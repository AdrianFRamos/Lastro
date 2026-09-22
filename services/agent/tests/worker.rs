//! Agent worker ordering and evidence-integrity contracts.

use std::{collections::VecDeque, fs, path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bytes::Bytes;
use lastro_agent::{
    api_client::ApiClient,
    command::StationCommand,
    error::AgentError,
    serial::{
        frame::{Frame, MessageType},
        payload::{decode_command, decode_event_ready},
        StationTransport,
    },
    spool::Spool,
    worker::{process_capture, replay_durable_local_acks},
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
                        .find_map(|line| line.strip_prefix("content-length: ").or_else(|| line.strip_prefix("Content-Length: ")))
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

fn request_json(request: &[u8]) -> Value {
    let body_start = request.windows(4).position(|window| window == b"\r\n\r\n").unwrap() + 4;
    serde_json::from_slice(&request[body_start..]).unwrap()
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

    process_capture(&mut transport, &spool, &api, &station_pubkey(), &command())
        .await
        .unwrap();
    assert_eq!(transport.sent.len(), 2);
    assert_eq!(transport.sent[0].message_type, MessageType::Command);
    assert_eq!(transport.sent[1].message_type, MessageType::Ack);
    assert_eq!(transport.sent.iter().filter(|frame| frame.message_type == MessageType::Command).count(), 1);

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
    let mut transport = FakeTransport::new(vec![event_ready_frame()]).with_sqlite_check(url.clone());

    process_capture(&mut transport, &spool, &api, &station_pubkey(), &command())
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
    let api = ApiClient::new("http://127.0.0.1:9/".into(), TOKEN.into(), Duration::from_millis(20)).unwrap();

    let error = process_capture(&mut transport, &spool, &api, &station_pubkey(), &command())
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
    unexpected_key[0] = if unexpected_key[0] == 0x02 { 0x03 } else { 0x02 };
    let mut transport = FakeTransport::new(vec![event_ready_frame()]);
    let api = ApiClient::new("http://127.0.0.1:9/".into(), TOKEN.into(), Duration::from_millis(20)).unwrap();

    let error = process_capture(&mut transport, &spool, &api, &unexpected_key, &command())
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

    process_capture(&mut transport, &spool, &api, &station_pubkey(), &command())
        .await
        .unwrap();
    let stored = spool.pending().await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].event_bytes, expected_ready.event_bytes);

    server.await.unwrap();
    let captured = request.lock().await;
    let json = request_json(&captured);
    let http_event = BASE64.decode(json["eventBytesBase64"].as_str().unwrap()).unwrap();
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
        replay_durable_local_acks(&mut transport, &spool, &station_pubkey())
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
    assert_eq!(pending[0].state, lastro_agent::spool::model::OutboxState::Local);

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

    let error = replay_durable_local_acks(&mut transport, &spool, &unexpected)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("public key"));
    assert!(transport.sent.is_empty());
    assert_eq!(spool.pending().await.unwrap().len(), 1);

    drop(spool);
    let _ = fs::remove_file(path);
}
