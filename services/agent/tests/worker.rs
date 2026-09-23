//! Agent worker ordering and evidence-integrity contracts.

use std::{collections::VecDeque, fs, path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use bytes::Bytes;
use lastro_agent::{
    api_client::ApiClient,
    command::StationCommand,
    error::AgentError,
    serial::{
        StationTransport,
        frame::{Frame, MessageType},
        payload::{decode_command, decode_event_ready},
    },
    spool::Spool,
    worker::{process_capture, replay_durable_acks, retry_pending_once, run},
};
use serde_json::Value;
use sqlx::SqlitePool;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
    task::JoinHandle,
};
use uuid::Uuid;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name)).expect("read fixture")
}

fn command() -> StationCommand {
    decode_command(&fixture("serial-command.bin")).unwrap()
}

fn event_ready_frame() -> Frame {
    Frame {
        message_type: MessageType::EventReady,
        payload: Bytes::from(fixture("serial-event-ready.bin")),
    }
}

fn station_pubkey() -> [u8; 33] {
    decode_event_ready(&fixture("serial-event-ready.bin"))
        .unwrap()
        .station_pubkey33
}

fn database_url() -> (String, PathBuf) {
    let path = std::env::temp_dir().join(format!("lastro-agent-worker-{}.sqlite", Uuid::new_v4()));
    (format!("sqlite://{}", path.display()), path)
}

struct FakeTransport {
    incoming: VecDeque<Frame>,
    sent: Vec<Frame>,
    sqlite_url: Option<String>,
    ack_observed_after_persist: bool,
}

impl FakeTransport {
    fn new(incoming: Vec<Frame>) -> Self {
        Self {
            incoming: incoming.into(),
            sent: Vec::new(),
            sqlite_url: None,
            ack_observed_after_persist: false,
        }
    }

    fn with_sqlite_check(mut self, sqlite_url: String) -> Self {
        self.sqlite_url = Some(sqlite_url);
        self
    }
}

#[async_trait]
impl StationTransport for FakeTransport {
    async fn send(&mut self, frame: Frame) -> Result<(), AgentError> {
        if frame.message_type == MessageType::Ack {
            if let Some(url) = &self.sqlite_url {
                let pool = SqlitePool::connect(url)
                    .await
                    .map_err(|error| AgentError::Spool(error.to_string()))?;
                let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox")
                    .fetch_one(&pool)
                    .await
                    .map_err(|error| AgentError::Spool(error.to_string()))?;
                pool.close().await;
                self.ack_observed_after_persist = count == 1;
            }
        }
        self.sent.push(frame);
        Ok(())
    }

    async fn receive(&mut self) -> Result<Frame, AgentError> {
        if self.sent.len() != 1 || self.sent[0].message_type != MessageType::Command {
            return Err(AgentError::Contract(
                "worker must send exactly one COMMAND before waiting for Station response".into(),
            ));
        }
        self.incoming
            .pop_front()
            .ok_or_else(|| AgentError::Serial("test Station has no response".into()))
    }
}

struct StopAfterRecoveredPollTransport {
    sent: Vec<Frame>,
}

#[async_trait]
impl StationTransport for StopAfterRecoveredPollTransport {
    async fn send(&mut self, frame: Frame) -> Result<(), AgentError> {
        self.sent.push(frame);
        Ok(())
    }

    async fn receive(&mut self) -> Result<Frame, AgentError> {
        Err(AgentError::Contract(
            "test stop after recovered command poll".into(),
        ))
    }
}

struct HangingTransport {
    sent: Vec<Frame>,
}

#[async_trait]
impl StationTransport for HangingTransport {
    async fn send(&mut self, frame: Frame) -> Result<(), AgentError> {
        self.sent.push(frame);
        Ok(())
    }

    async fn receive(&mut self) -> Result<Frame, AgentError> {
        std::future::pending::<Result<Frame, AgentError>>().await
    }
}

async fn spawn_capture_server() -> (String, Arc<Mutex<Vec<u8>>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let output = Arc::clone(&captured);
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut chunk = [0u8; 4096];
        let mut expected_len = None;
        loop {
            let count = socket.read(&mut chunk).await.unwrap();
            if count == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..count]);
            if expected_len.is_none() {
                if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                    let headers_end = end + 4;
                    let headers = String::from_utf8_lossy(&request[..headers_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length: ")
                                .or_else(|| line.strip_prefix("Content-Length: "))
                        })
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    expected_len = Some(headers_end + content_length);
                }
            }
            if expected_len.is_some_and(|length| request.len() >= length) {
                break;
            }
        }
        *output.lock().await = request;
        let response = b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let _ = socket.write_all(response).await;
    });
    (format!("http://{address}/"), captured, task)
}

async fn spawn_poll_recovery_server(command: &StationCommand) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let body = serde_json::json!({
        "captureId": command.capture_id,
        "action": match command.action {
            1 => "ORIGIN",
            2 => "TRANSFER",
            3 => "REIDENTIFY",
            _ => unreachable!("validated test command action"),
        },
        "deploymentId": hex::encode(command.deployment_id),
        "animalId": hex::encode(command.animal_id),
        "eventSequence": command.event_sequence,
        "identityRevision": command.identity_revision,
        "previousEventHash": hex::encode(command.previous_event_hash),
        "expectedOldRfidHash": hex::encode(command.expected_old_rfid_hash),
        "fromCustodian": hex::encode(command.from_custodian),
        "toCustodian": hex::encode(command.to_custodian),
    })
    .to_string();

    let task = tokio::spawn(async move {
        for (index, (status, response_body)) in [
            ("500 Internal Server Error", String::new()),
            ("200 OK", body.clone()),
        ]
        .into_iter()
        .enumerate()
        {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 4096];
            let count = socket.read(&mut request).await.unwrap();
            let text = String::from_utf8_lossy(&request[..count]);
            assert!(text.starts_with("GET /api/agent/commands "));
            let wire = format!(
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                response_body.len(),
                response_body,
            );
            socket.write_all(wire.as_bytes()).await.unwrap();
            socket.shutdown().await.unwrap();
            assert!(index < 2);
        }
    });
    (format!("http://{address}/"), task)
}

async fn spawn_status_server(status: &'static str) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 4096];
        let _ = socket.read(&mut request).await;
        let response =
            format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        let _ = socket.write_all(response.as_bytes()).await;
    });
    (format!("http://{address}/"), task)
}

fn request_json(request: &[u8]) -> Value {
    let body_start = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap()
        + 4;
    serde_json::from_slice(&request[body_start..]).unwrap()
}

#[tokio::test]
async fn worker_times_out_a_station_that_never_responds() {
    // PURPOSE: One silent Station must not block command processing forever.
    // ARRANGE: A transport accepts COMMAND but never resolves receive().
    // ACTION: Process one capture with a 25 ms Station response timeout.
    // ASSERT: The worker returns a serial timeout, persists no evidence, and sends no ACK.
    // FAILURE MEANS: one unplugged or wedged serial peer can hang the Agent indefinitely.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let api = ApiClient::new(
        "http://127.0.0.1:9/".into(),
        TOKEN.into(),
        Duration::from_millis(20),
    )
    .unwrap();
    let mut transport = HangingTransport { sent: Vec::new() };

    let error = tokio::time::timeout(
        Duration::from_secs(1),
        process_capture(
            &mut transport,
            &spool,
            &api,
            &station_pubkey(),
            Duration::from_millis(25),
            &command(),
        ),
    )
    .await
    .expect("worker-level Station timeout must resolve")
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("timed out waiting for Station response")
    );
    assert!(spool.pending().await.unwrap().is_empty());
    assert_eq!(transport.sent.len(), 1);
    assert_eq!(transport.sent[0].message_type, MessageType::Command);

    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn worker_sends_one_command_per_station_capture() {
    // PURPOSE: One active capture must map to exactly one Station COMMAND before a response is consumed.
    // ASSERT: The transport observes one COMMAND and then one ACK; no second COMMAND is emitted during the same capture.
    // FAILURE MEANS: The Station could mix immutable context from concurrent capture sessions.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let (base_url, _, server) = spawn_capture_server().await;
    let api = ApiClient::new(base_url, TOKEN.into(), Duration::from_secs(1)).unwrap();
    let mut transport = FakeTransport::new(vec![event_ready_frame()]);

    process_capture(
        &mut transport,
        &spool,
        &api,
        &station_pubkey(),
        Duration::from_secs(1),
        &command(),
    )
    .await
    .unwrap();
    assert_eq!(transport.sent.len(), 2);
    assert_eq!(transport.sent[0].message_type, MessageType::Command);
    assert_eq!(transport.sent[1].message_type, MessageType::Ack);
    assert_eq!(
        transport
            .sent
            .iter()
            .filter(|frame| frame.message_type == MessageType::Command)
            .count(),
        1
    );

    server.await.unwrap();
    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn worker_does_not_ack_before_sqlite_persist() {
    // PURPOSE: ACK is allowed only after evidence crosses the durable LOCAL SQLite boundary.
    // ASSERT: At the exact ACK send operation, the outbox already contains the captured evidence row.
    // FAILURE MEANS: The Station could discard its pending event while the Agent still has no durable copy.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let (base_url, _, server) = spawn_capture_server().await;
    let api = ApiClient::new(base_url, TOKEN.into(), Duration::from_secs(1)).unwrap();
    let mut transport =
        FakeTransport::new(vec![event_ready_frame()]).with_sqlite_check(url.clone());

    process_capture(
        &mut transport,
        &spool,
        &api,
        &station_pubkey(),
        Duration::from_secs(1),
        &command(),
    )
    .await
    .unwrap();
    assert!(transport.ack_observed_after_persist);

    server.await.unwrap();
    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn worker_rejects_capture_id_mismatch() {
    // PURPOSE: EVENT_READY must belong to the exact capture that produced its COMMAND.
    // ASSERT: A different capture id is rejected before persistence, ACK, or HTTP submission.
    // FAILURE MEANS: Evidence from another Station session could be attached to the active capture context.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let mut ready = decode_event_ready(&fixture("serial-event-ready.bin")).unwrap();
    ready.capture_id = Uuid::new_v4();
    let payload = lastro_agent::serial::payload::encode_event_ready(&ready);
    let mut transport = FakeTransport::new(vec![Frame {
        message_type: MessageType::EventReady,
        payload: Bytes::copy_from_slice(&payload),
    }]);
    let api = ApiClient::new(
        "http://127.0.0.1:9/".into(),
        TOKEN.into(),
        Duration::from_millis(20),
    )
    .unwrap();

    let error = process_capture(
        &mut transport,
        &spool,
        &api,
        &station_pubkey(),
        Duration::from_secs(1),
        &command(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("capture_id"));
    assert!(spool.pending().await.unwrap().is_empty());
    assert_eq!(transport.sent.len(), 1);
    assert_eq!(transport.sent[0].message_type, MessageType::Command);

    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn worker_rejects_station_pubkey_change() {
    // PURPOSE: The Agent must pin the configured Station key before accepting serial evidence.
    // ASSERT: A different supplied key is rejected before persistence, ACK, or HTTP submission.
    // FAILURE MEANS: A physically substituted Station could silently enter the evidence pipeline.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let mut unexpected_key = station_pubkey();
    unexpected_key[0] = if unexpected_key[0] == 0x02 {
        0x03
    } else {
        0x02
    };
    let mut transport = FakeTransport::new(vec![event_ready_frame()]);
    let api = ApiClient::new(
        "http://127.0.0.1:9/".into(),
        TOKEN.into(),
        Duration::from_millis(20),
    )
    .unwrap();

    let error = process_capture(
        &mut transport,
        &spool,
        &api,
        &unexpected_key,
        Duration::from_secs(1),
        &command(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("public key"));
    assert!(spool.pending().await.unwrap().is_empty());
    assert_eq!(transport.sent.len(), 1);

    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn worker_never_modifies_event_bytes() {
    // PURPOSE: The same signed 276 bytes must cross serial, SQLite, and HTTP without reserialization.
    // ASSERT: Serial EVENT_READY bytes equal the durable blob and the decoded HTTP eventBytesBase64 payload exactly.
    // FAILURE MEANS: Any bridge transformation could break the Station signature or Secp256r1 message binding.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let expected_ready = decode_event_ready(&fixture("serial-event-ready.bin")).unwrap();
    let (base_url, request, server) = spawn_capture_server().await;
    let api = ApiClient::new(base_url, TOKEN.into(), Duration::from_secs(1)).unwrap();
    let mut transport = FakeTransport::new(vec![event_ready_frame()]);

    process_capture(
        &mut transport,
        &spool,
        &api,
        &station_pubkey(),
        Duration::from_secs(1),
        &command(),
    )
    .await
    .unwrap();
    let stored = spool.pending().await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].event_bytes, expected_ready.event_bytes);

    server.await.unwrap();
    let captured = request.lock().await;
    let json = request_json(&captured);
    let http_event = BASE64
        .decode(json["eventBytesBase64"].as_str().unwrap())
        .unwrap();
    assert_eq!(http_event.as_slice(), expected_ready.event_bytes);
    drop(captured);

    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn worker_replays_ack_for_durable_local_evidence_after_restart() {
    // PURPOSE: Close the crash window after SQLite LOCAL commit but before the original ACK reaches the Station.
    // ARRANGE: Persist the frozen EVENT_READY evidence as LOCAL, simulating an Agent restart before ACK/HTTP delivery.
    // ACTION: Run the startup durable-ACK recovery against a fresh transport.
    // ASSERT: Exactly one ACK is sent with the durable capture_id/event_hash and the outbox row remains byte-identical LOCAL.
    // FAILURE MEANS: a restart can leave the Station permanently in WAIT_ACK despite the Agent already owning durable evidence.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let ready = decode_event_ready(&fixture("serial-event-ready.bin")).unwrap();
    let event = lastro_protocol::StationEvent::decode(&ready.event_bytes).unwrap();
    let row = lastro_agent::spool::model::OutboxRow {
        capture_id: ready.capture_id,
        event_hash: event.event_hash(),
        event_bytes: ready.event_bytes,
        observed_rfid: ready.observed_rfid,
        station_pubkey: ready.station_pubkey33,
        station_signature: ready.station_signature64,
        state: lastro_agent::spool::model::OutboxState::Local,
        attempts: 0,
    };
    spool.persist_local(&row).await.unwrap();
    let mut transport = FakeTransport::new(vec![]);

    assert_eq!(
        replay_durable_acks(&mut transport, &spool, &station_pubkey())
            .await
            .unwrap(),
        1
    );
    assert_eq!(transport.sent.len(), 1);
    assert_eq!(transport.sent[0].message_type, MessageType::Ack);
    let ack = lastro_agent::serial::payload::decode_ack(&transport.sent[0].payload).unwrap();
    assert_eq!(ack.capture_id, row.capture_id);
    assert_eq!(ack.event_hash, row.event_hash);

    let pending = spool.pending().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].event_bytes, row.event_bytes);
    assert_eq!(
        pending[0].state,
        lastro_agent::spool::model::OutboxState::Local
    );

    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn worker_replays_ack_for_server_evidence_after_restart() {
    // PURPOSE: Close the ACK-loss window after the API already accepted durable evidence.
    // ARRANGE: Persist one exact EVENT_READY row and advance it to SERVER as if HTTP acceptance
    //          succeeded after the host wrote an ACK that the Station never processed.
    // ACTION: Run startup durable-ACK recovery against a fresh transport.
    // ASSERT: The same capture/event ACK is replayed once and the immutable row remains SERVER.
    // FAILURE MEANS: a lost ACK after API acceptance can leave the Station in WAIT_ACK forever.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let ready = decode_event_ready(&fixture("serial-event-ready.bin")).unwrap();
    let event = lastro_protocol::StationEvent::decode(&ready.event_bytes).unwrap();
    let row = lastro_agent::spool::model::OutboxRow {
        capture_id: ready.capture_id,
        event_hash: event.event_hash(),
        event_bytes: ready.event_bytes,
        observed_rfid: ready.observed_rfid,
        station_pubkey: ready.station_pubkey33,
        station_signature: ready.station_signature64,
        state: lastro_agent::spool::model::OutboxState::Local,
        attempts: 0,
    };
    spool.persist_local(&row).await.unwrap();
    spool
        .advance(
            row.capture_id,
            lastro_agent::spool::model::OutboxState::Server,
        )
        .await
        .unwrap();
    let mut transport = FakeTransport::new(vec![]);

    assert_eq!(
        replay_durable_acks(&mut transport, &spool, &station_pubkey())
            .await
            .unwrap(),
        1
    );
    assert_eq!(transport.sent.len(), 1);
    let ack = lastro_agent::serial::payload::decode_ack(&transport.sent[0].payload).unwrap();
    assert_eq!(ack.capture_id, row.capture_id);
    assert_eq!(ack.event_hash, row.event_hash);
    let pending = spool.pending().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0].state,
        lastro_agent::spool::model::OutboxState::Server
    );
    assert_eq!(pending[0].event_bytes, row.event_bytes);

    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn transient_command_poll_failure_does_not_terminate_worker() {
    // PURPOSE: A temporary API outage while polling commands must not kill the long-running Agent.
    // ARRANGE: The loopback API returns HTTP 500 once, then one valid command.
    //          The transport deliberately returns a Contract error only after that recovered command is sent.
    // ACTION: Run the production worker with zero poll delay.
    // ASSERT: The returned error is the deliberate post-recovery Contract error, not the first transient API error,
    //         and exactly one Station COMMAND was sent after recovery.
    // FAILURE MEANS: one transient polling failure still requires process supervision/restart for recovery.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let expected = command();
    let (base_url, server) = spawn_poll_recovery_server(&expected).await;
    let api = ApiClient::new(base_url, TOKEN.into(), Duration::from_secs(1)).unwrap();
    let transport = StopAfterRecoveredPollTransport { sent: Vec::new() };

    let error = tokio::time::timeout(
        Duration::from_secs(2),
        run(
            transport,
            &spool,
            &api,
            station_pubkey(),
            Duration::ZERO,
            Duration::from_secs(1),
        ),
    )
    .await
    .expect("worker must progress beyond the transient poll failure")
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("test stop after recovered command poll")
    );
    server.await.unwrap();

    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn terminal_api_rejection_quarantines_durable_evidence_without_stopping_retry_processing() {
    // PURPOSE: A deterministic 4xx response must not crash the Agent or retry forever after restart.
    // ARRANGE: Persist immutable LOCAL evidence and make the API reject its upload with HTTP 409.
    // ACTION: Run one durable retry pass.
    // ASSERT: The pass succeeds, the row leaves the automatic pending queue, its bytes remain unchanged,
    //         and SQLite records a QUARANTINED terminal state with diagnostic context.
    // FAILURE MEANS: an expired or semantically rejected capture can permanently wedge the Agent outbox.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let ready = decode_event_ready(&fixture("serial-event-ready.bin")).unwrap();
    let event = lastro_protocol::StationEvent::decode(&ready.event_bytes).unwrap();
    let row = lastro_agent::spool::model::OutboxRow {
        capture_id: ready.capture_id,
        event_hash: event.event_hash(),
        event_bytes: ready.event_bytes,
        observed_rfid: ready.observed_rfid,
        station_pubkey: ready.station_pubkey33,
        station_signature: ready.station_signature64,
        state: lastro_agent::spool::model::OutboxState::Local,
        attempts: 0,
    };
    spool.persist_local(&row).await.unwrap();
    let (base_url, server) = spawn_status_server("409 Conflict").await;
    let api = ApiClient::new(base_url, TOKEN.into(), Duration::from_secs(1)).unwrap();

    assert!(
        !retry_pending_once(&spool, &api, Duration::ZERO)
            .await
            .unwrap()
    );
    server.await.unwrap();
    assert!(spool.pending().await.unwrap().is_empty());

    let direct = SqlitePool::connect(&url).await.unwrap();
    let state: String = sqlx::query_scalar("SELECT state FROM outbox WHERE capture_id = ?")
        .bind(row.capture_id.to_string())
        .fetch_one(&direct)
        .await
        .unwrap();
    let stored_bytes: Vec<u8> =
        sqlx::query_scalar("SELECT event_bytes FROM outbox WHERE capture_id = ?")
            .bind(row.capture_id.to_string())
            .fetch_one(&direct)
            .await
            .unwrap();
    let last_error: String =
        sqlx::query_scalar("SELECT last_error FROM outbox WHERE capture_id = ?")
            .bind(row.capture_id.to_string())
            .fetch_one(&direct)
            .await
            .unwrap();
    assert_eq!(state, "QUARANTINED");
    assert_eq!(stored_bytes, row.event_bytes);
    assert!(last_error.contains("409"));
    direct.close().await;

    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn worker_refuses_to_ack_durable_evidence_from_an_unexpected_station_key() {
    // PURPOSE: Restart recovery must not turn a stale/substituted SQLite row into an ACK for an untrusted Station identity.
    // ARRANGE: Persist one LOCAL row then configure the worker with a different compressed Station key.
    // ACTION: Run durable-ACK recovery.
    // ASSERT: Recovery fails before serial output and leaves the durable row unchanged.
    // FAILURE MEANS: local-disk tampering or Station substitution could make the Agent acknowledge evidence for the wrong device.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let ready = decode_event_ready(&fixture("serial-event-ready.bin")).unwrap();
    let event = lastro_protocol::StationEvent::decode(&ready.event_bytes).unwrap();
    let row = lastro_agent::spool::model::OutboxRow {
        capture_id: ready.capture_id,
        event_hash: event.event_hash(),
        event_bytes: ready.event_bytes,
        observed_rfid: ready.observed_rfid,
        station_pubkey: ready.station_pubkey33,
        station_signature: ready.station_signature64,
        state: lastro_agent::spool::model::OutboxState::Local,
        attempts: 0,
    };
    spool.persist_local(&row).await.unwrap();
    let mut unexpected = station_pubkey();
    unexpected[0] = if unexpected[0] == 0x02 { 0x03 } else { 0x02 };
    let mut transport = FakeTransport::new(vec![]);

    let error = replay_durable_acks(&mut transport, &spool, &unexpected)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("public key"));
    assert!(transport.sent.is_empty());
    assert_eq!(spool.pending().await.unwrap().len(), 1);

    drop(spool);
    let _ = fs::remove_file(path);
}
