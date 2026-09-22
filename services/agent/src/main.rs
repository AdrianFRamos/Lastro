use lastro_agent::{
    api_client::ApiClient,
    config::AgentConfig,
    serial::SerialStationTransport,
    spool::Spool,
    worker,
};

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
    let transport = SerialStationTransport::open(&config.station_serial_path, config.station_baud)?;

    worker::run(
        transport,
        spool,
        api,
        config.station_pubkey33,
        config.poll_interval,
    )
    .await?;
    Ok(())
}
