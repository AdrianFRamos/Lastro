use lastro_api::{config::AppConfig, db, routes, solana::rpc::HttpSolanaRpc, state::AppState};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Startup order is deliberate: config -> tracing -> DB+migrations -> RPC handle -> bind.
    // Do not start accepting HTTP requests after partial initialization.
    let config = Arc::new(AppConfig::from_env()?);
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let db = db::connect_and_migrate(&config.database_url).await?;
    let rpc = Arc::new(HttpSolanaRpc::new(
        config.solana_rpc_url.clone(),
        config.lastro_program_id.clone(),
    ));
    let state = AppState {
        db,
        config: config.clone(),
        rpc,
    };
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, routes::router(state)).await?;
    Ok(())
}
