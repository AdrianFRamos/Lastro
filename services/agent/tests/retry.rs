//! Agent retry and durable-outbox contracts.
//!
//! These tests use the real SQLite migrations and real `reqwest` transport against a loopback TCP
//! server. Only the remote API behavior is controlled; retry state and evidence bytes use production code.

use std::{fs, path::PathBuf, time::Duration};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use lastro_agent::{
    api_client::ApiClient,
    spool::{
        model::{OutboxRow, OutboxState},
        Spool,
    },
    worker::{retry_delay, retry_pending_once},
};
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use uuid::Uuid;

#[derive(Clone)]
struct ResponseSpec {
    status: u16,
    body: &'static str,
    delay: Duration,
}

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name)).expect("read fixture")
}

fn row(capture_id: Uuid) -> OutboxRow {
    OutboxRow {
        capture_id,
        event_hash: hex::decode("5845dc20fd6b266ec98399f0aa93c736ec9aa778bf038af5291e5df81334b531")
            .unwrap()
            .try_into()
            .unwrap(),
        event_bytes: fixture("origin.bin").try_into().unwrap(),
        observed_rfid: hex::decode("8000130000000001").unwrap().try_into().unwrap(),
        station_pubkey: hex::decode("036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296")
            .unwrap()
            .try_into()
            .unwrap(),
        station_signature: hex::decode("09579de39fbd8108dea2fc89262240c45b2043d85e6e16854d9bed737e6426e109be548b3eb9d3ba3252aeba61af230a18115aeaf0b74734df88c90955f9184f")
            .unwrap()
            .try_into()
            .unwrap(),
        state: OutboxState::Local,
        attempts: 0,
    }
}

fn database_url() -> (String, PathBuf) {
    let path = std::env::temp_dir().join(format!("lastro-agent-retry-{}.sqlite", Uuid::new_v4()));
    (format!("sqlite://{}", path.display()), path)
}

fn cleanup_database(path: &PathBuf) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}-wal", path.display()));
    let _ = fs::remove_file(format!("{}-shm", path.display()));
}

fn api(base_url: String, timeout: Duration) -> ApiClient {
    ApiClient::new(base_url, "test-agent-token-0123456789abcdef".into(), timeout).unwrap()
}

async fn spawn_server(responses: Vec<ResponseSpec>) -> (String, JoinHandle<Vec<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_http_request(&mut socket).await;
            requests.push(request);
            if !response.delay.is_zero() {
                tokio::time::sleep(response.delay).await;
            }
            let reason = match response.status {
                200 => "OK",
                201 => "Created",
                429 => "Too Many Requests",
                500 => "Internal Server Error",
                _ => "Test Response",
            };
            let wire = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.status,
                reason,
                response.body.len(),
                response.body,
            );
            let _ = socket.write_all(wire.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
        requests
    });
    (format!("http://{address}/"), handle)
}

async fn read_http_request(socket: &mut tokio::net::TcpStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let count = socket.read(&mut chunk).await.unwrap_or(0);
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(header_end) = find_subslice(&bytes, b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: ").or_else(|| line.strip_prefix("Content-Length: ")))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if bytes.len() >= header_end + 4 + content_length {
                break;
            }
        }
        assert!(bytes.len() < 1_000_000, "test HTTP request exceeded safety bound");
    }
    bytes
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn request_body(request: &[u8]) -> &[u8] {
    let start = find_subslice(request, b"\r\n\r\n").expect("HTTP header terminator") + 4;
    &request[start..]
}

#[tokio::test]
async fn api_timeout_keeps_outbox_local() {
    // PURPOSE: A timed-out POST must never lose already durable evidence.
    // ARRANGE: Persist one LOCAL row and let the loopback API delay beyond the real reqwest timeout.
    // ACTION: Execute one production retry pass.
    // ASSERT: Failure is retryable; row stays LOCAL with immutable event hash/bytes and one recorded attempt.
    // FAILURE MEANS: A network failure can destroy or rewrite the only durable evidence copy.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let expected = row(Uuid::new_v4());
    spool.persist_local(&expected).await.unwrap();
    let (base_url, server) = spawn_server(vec![ResponseSpec {
        status: 201,
        body: "",
        delay: Duration::from_millis(200),
    }]).await;

    assert!(retry_pending_once(&spool, &api(base_url, Duration::from_millis(25)), Duration::ZERO).await.unwrap());
    let pending = spool.pending().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].state, OutboxState::Local);
    assert_eq!(pending[0].attempts, 1);
    assert_eq!(pending[0].event_hash, expected.event_hash);
    assert_eq!(pending[0].event_bytes, expected.event_bytes);
    server.abort();
    drop(spool);
    cleanup_database(&path);
}

#[tokio::test]
async fn api_transport_errors_do_not_persist_url_credentials() {
    // PURPOSE: Reqwest transport failures may carry the full request URL; durable retry diagnostics must not retain URL credentials.
    // ARRANGE: Use a loopback endpoint with synthetic URL userinfo and delay the response beyond the production client timeout.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let expected = row(Uuid::new_v4());
    spool.persist_local(&expected).await.unwrap();
    let (base_url, server) = spawn_server(vec![ResponseSpec {
        status: 201,
        body: "",
        delay: Duration::from_millis(200),
    }]).await;
    let credentialed = base_url.replacen(
        "http://",
        "http://synthetic-user:synthetic-password@",
        1,
    );

    assert!(
        retry_pending_once(
            &spool,
            &api(credentialed, Duration::from_millis(25)),
            Duration::ZERO,
        )
        .await
        .unwrap()
    );

    let direct = sqlx::SqlitePool::connect(&url).await.unwrap();
    let last_error: Option<String> =
        sqlx::query_scalar("SELECT last_error FROM outbox WHERE capture_id = ?")
            .bind(expected.capture_id.to_string())
            .fetch_one(&direct)
            .await
            .unwrap();
    let last_error = last_error.expect("retry failure is recorded");
    assert!(!last_error.contains("synthetic-user"));
    assert!(!last_error.contains("synthetic-password"));
    direct.close().await;

    server.abort();
    drop(spool);
    cleanup_database(&path);

    // ASSERT: retry state retains a useful error class without retaining URL credentials.
    // FAILURE MEANS: a deployment URL secret can survive in SQLite/log-adjacent diagnostics.
}

#[tokio::test]
async fn api_500_retries_with_bounded_backoff() {
    // PURPOSE: Transient API failures retry without duplicate durable rows or unbounded delay growth.
    // ARRANGE: One LOCAL row; loopback API returns 500 twice then 201.
    // ACTION: Run three production retry passes and inspect the pure durable-attempt backoff policy.
    // ASSERT: One row remains; attempts advance deterministically; success moves it to SERVER; backoff caps at 30 seconds.
    // FAILURE MEANS: Agent can overload the API, duplicate evidence, or delay retries without a bound.
    assert_eq!(retry_delay(Duration::from_millis(100), 0), Duration::ZERO);
    assert_eq!(retry_delay(Duration::from_millis(100), 1), Duration::from_millis(100));
    assert_eq!(retry_delay(Duration::from_millis(100), 2), Duration::from_millis(200));
    assert_eq!(retry_delay(Duration::from_millis(100), 3), Duration::from_millis(400));
    assert_eq!(retry_delay(Duration::from_secs(20), 4), Duration::from_secs(30));

    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let expected = row(Uuid::new_v4());
    spool.persist_local(&expected).await.unwrap();
    let (base_url, server) = spawn_server(vec![
        ResponseSpec { status: 500, body: "", delay: Duration::ZERO },
        ResponseSpec { status: 500, body: "", delay: Duration::ZERO },
        ResponseSpec { status: 201, body: "", delay: Duration::ZERO },
    ]).await;
    let client = api(base_url, Duration::from_secs(1));

    assert!(retry_pending_once(&spool, &client, Duration::ZERO).await.unwrap());
    assert!(retry_pending_once(&spool, &client, Duration::ZERO).await.unwrap());
    assert!(!retry_pending_once(&spool, &client, Duration::ZERO).await.unwrap());
    let pending = spool.pending().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].state, OutboxState::Server);
    assert_eq!(pending[0].attempts, 3);
    assert_eq!(pending[0].event_bytes, expected.event_bytes);
    assert_eq!(server.await.unwrap().len(), 3);
    drop(spool);
    cleanup_database(&path);
}

#[tokio::test]
async fn restart_resumes_local_rows() {
    // PURPOSE: Process restart must resume work from durable SQLite without another Station read.
    // ARRANGE: Persist LOCAL, drop the Spool, then reopen the exact same SQLite file and start a 201 loopback API.
    // ACTION: Run one production retry pass after reopening.
    // ASSERT: The same bytes are discovered and submitted; the single row advances to SERVER.
    // FAILURE MEANS: Host reboot can orphan locally captured evidence.
    let (url, path) = database_url();
    let expected = row(Uuid::new_v4());
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    spool.persist_local(&expected).await.unwrap();
    drop(spool);

    let reopened = Spool::connect_and_migrate(&url).await.unwrap();
    let before = reopened.pending().await.unwrap();
    assert_eq!(before.len(), 1);
    assert_eq!(before[0].event_bytes, expected.event_bytes);
    let (base_url, server) = spawn_server(vec![ResponseSpec { status: 201, body: "", delay: Duration::ZERO }]).await;
    assert!(!retry_pending_once(&reopened, &api(base_url, Duration::from_secs(1)), Duration::ZERO).await.unwrap());
    let after = reopened.pending().await.unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].state, OutboxState::Server);
    assert_eq!(after[0].event_bytes, expected.event_bytes);
    assert_eq!(server.await.unwrap().len(), 1);
    drop(reopened);
    cleanup_database(&path);
}

#[tokio::test]
async fn server_ack_moves_local_to_server() {
    // PURPOSE: Backend acceptance advances lifecycle without mutating proof bytes.
    // ARRANGE: One LOCAL row and a loopback API that returns idempotent HTTP 200.
    // ACTION: Execute one production retry pass and capture the actual HTTP body.
    // ASSERT: State becomes SERVER and the submitted base64/hex fields reproduce immutable SQLite evidence exactly.
    // FAILURE MEANS: Lifecycle can alter evidence or remain stuck after backend acceptance.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let expected = row(Uuid::new_v4());
    spool.persist_local(&expected).await.unwrap();
    let (base_url, server) = spawn_server(vec![ResponseSpec { status: 200, body: "", delay: Duration::ZERO }]).await;

    assert!(!retry_pending_once(&spool, &api(base_url, Duration::from_secs(1)), Duration::ZERO).await.unwrap());
    let pending = spool.pending().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].state, OutboxState::Server);
    assert_eq!(pending[0].event_bytes, expected.event_bytes);
    assert_eq!(pending[0].observed_rfid, expected.observed_rfid);
    assert_eq!(pending[0].station_pubkey, expected.station_pubkey);
    assert_eq!(pending[0].station_signature, expected.station_signature);

    let requests = server.await.unwrap();
    let body: Value = serde_json::from_slice(request_body(&requests[0])).unwrap();
    assert_eq!(body["captureId"], expected.capture_id.to_string());
    assert_eq!(body["eventBytesBase64"], BASE64.encode(expected.event_bytes));
    assert_eq!(body["observedRfidHex"], hex::encode(expected.observed_rfid));
    assert_eq!(body["stationPubkeyHex"], hex::encode(expected.station_pubkey));
    assert_eq!(body["stationSignatureHex"], hex::encode(expected.station_signature));
    drop(spool);
    cleanup_database(&path);
}

#[tokio::test]
async fn finalization_moves_server_to_finalized() {
    // PURPOSE: Canonical finalization must close only the durable item represented by its event hash.
    // ARRANGE: Advance one row to SERVER and return FINALIZED from the real evidence-status HTTP endpoint.
    // ACTION: Run one production retry pass.
    // ASSERT: The row leaves the pending set, and lifecycle regression is rejected afterward.
    // FAILURE MEANS: Agent can leave finalized evidence retrying or reopen terminal state.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let expected = row(Uuid::new_v4());
    spool.persist_local(&expected).await.unwrap();
    spool.advance(expected.capture_id, OutboxState::Server).await.unwrap();
    let (base_url, server) = spawn_server(vec![ResponseSpec {
        status: 200,
        body: "{\"status\":\"FINALIZED\"}",
        delay: Duration::ZERO,
    }]).await;

    assert!(!retry_pending_once(&spool, &api(base_url, Duration::from_secs(1)), Duration::ZERO).await.unwrap());
    assert!(spool.pending().await.unwrap().is_empty());
    assert!(spool.advance(expected.capture_id, OutboxState::Server).await.is_err());
    let requests = server.await.unwrap();
    let request_text = String::from_utf8_lossy(&requests[0]);
    assert!(request_text.starts_with("GET /api/agent/evidence/5845dc20fd6b266ec98399f0aa93c736ec9aa778bf038af5291e5df81334b531 "));
    drop(spool);
    cleanup_database(&path);
}
