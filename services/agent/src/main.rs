use lastro_agent::{
    api_client::ApiClient, config::AgentConfig, error::AgentError, serial::SerialStationTransport,
    spool::Spool, worker,
};
use tokio::time::sleep;

/// How long the Station firmware waits for a tag after a command (station.c,
/// LASTRO_RFID_WAIT_TIMEOUT_MS).
const STATION_RFID_WAIT: std::time::Duration = std::time::Duration::from_secs(180);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = AgentConfig::from_env()?;
    if config.station_response_timeout < STATION_RFID_WAIT + std::time::Duration::from_secs(5) {
        tracing::warn!(
            timeout_ms = config.station_response_timeout.as_millis() as u64,
            "LASTRO_AGENT_STATION_RESPONSE_TIMEOUT_MS is shorter than the Station's 180 s RFID wait; \
             a late tag read can arrive while the next command runs"
        );
    }
    let spool = Spool::connect_and_migrate(&config.sqlite_url).await?;
    let api = ApiClient::new(
        config.api_base_url.clone(),
        config.agent_token.clone(),
        config.request_timeout,
    )?;

    loop {
        let transport =
            match SerialStationTransport::open(&config.station_serial_path, config.station_baud) {
                Ok(transport) => transport,
                Err(AgentError::Serial(message)) => {
                    tracing::warn!(
                        error = %message,
                        "Station serial open failed; retrying"
                    );
                    sleep(config.poll_interval).await;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };

        match worker::run(
            transport,
            &spool,
            &api,
            config.station_pubkey33,
            config.poll_interval,
            config.station_response_timeout,
        )
        .await
        {
            Ok(()) => return Ok(()),
            Err(AgentError::Serial(message)) => {
                tracing::warn!(
                    error = %message,
                    "Station serial transport failed; reconnecting"
                );
                sleep(config.poll_interval).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
}
