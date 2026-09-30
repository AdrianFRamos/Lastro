use thiserror::Error;

/// Stable Agent error classes. Retry policy depends on preserving the distinction between
/// malformed Station data, local durability failures, transient API failures and terminal API responses.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("invalid configuration: {0}")]
    Config(String),
    #[error("serial protocol error: {0}")]
    Serial(String),
    #[error("station capture failed: {0}")]
    Station(String),
    #[error("station command/evidence contract violation: {0}")]
    Contract(String),
    #[error("local outbox failure: {0}")]
    Spool(String),
    #[error("api transport failure: {0}")]
    Api(String),
    #[error("terminal api response: {0}")]
    ApiTerminal(String),
}
