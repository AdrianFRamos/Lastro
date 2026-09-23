use lastro_agent::{
    api_client::ApiClient, config::AgentConfig, error::AgentError, serial::SerialStationTransport,
    spool::Spool, worker,
};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = AgentConfig::from_env()?;
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
