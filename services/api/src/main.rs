use lastro_api::{
    config::AppConfig, db, orgs::LocalKeyVault, routes, solana::rpc::HttpSolanaRpc,
    sponsor::Sponsor, state::AppState,
};
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
    let sponsor = config
        .sponsor_keypair_path
        .as_deref()
        .map(|path| Sponsor::from_keypair_file(path, config.sponsor_limits))
        .transpose()?
        .map(Arc::new);
    if let Some(sponsor) = &sponsor {
        tracing::info!(sponsor = %sponsor.pubkey(), "fee sponsorship enabled");
    }
    let vault = config
        .org_key_encryption_key
        .as_ref()
        .map(|key| Arc::new(LocalKeyVault::new(key)));
    if vault.is_some() {
        tracing::info!("managed organization wallets enabled");
    }
    let state = AppState {
        db,
        config: config.clone(),
        rpc,
        sponsor,
        vault,
    };
    let _reconciliation_worker = lastro_api::reconciliation::spawn(state.clone());
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, routes::router(state)).await?;
    Ok(())
}
