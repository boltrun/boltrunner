use boltrunner_profile_api::{
    auth::Tokens,
    http::{AppState, router},
    logging::RollingLog,
    service::UserService,
};
use std::{
    env,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tracing_subscriber::{EnvFilter, prelude::*};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = env::var("BOLTRUNNER_JWT_SECRET")
        .map_err(|_| "Set BOLTRUNNER_JWT_SECRET to a random secret of at least 32 bytes")?;
    let tokens = Tokens::new(key.as_bytes())?;
    let log_path =
        PathBuf::from(env::var("BOLTRUNNER_LOG_FILE").unwrap_or_else(|_| "log/log.txt".into()));
    let limit = env::var("BOLTRUNNER_LOG_MAX_BYTES")
        .unwrap_or_else(|_| "65536".into())
        .parse()?;
    let writer = RollingLog::new(log_path, limit)?;
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(writer.clone()),
        )
        .init();
    let data = env::var("BOLTRUNNER_PROFILE_DATA").unwrap_or_else(|_| "data/users.json".into());
    let service = UserService::open(data).map_err(|_| "Cannot open profile data")?;
    let state = AppState {
        service: Arc::new(Mutex::new(service)),
        tokens,
    };
    let bind = env::var("BOLTRUNNER_PROFILE_BIND").unwrap_or_else(|_| "127.0.0.1:8768".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(address=%bind,"profile API started");
    axum::serve(listener, router(state)).await?;
    Ok(())
}
