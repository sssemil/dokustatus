use dotenvy::dotenv;
use std::process::ExitCode;
use tracing::{error, info};

use reauth_api::infra::{
    InfraError, app::create_app, domain_verifier::run_domain_verification_loop,
    setup::init_app_state, webhook_delivery_worker::run_webhook_delivery_loop,
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> ExitCode {
    dotenv().ok();

    if let Err(e) = run().await {
        // Log sanitized error via Display (safe for logs)
        // Note: Debug would expose full error chain including potential secrets
        error!(error = %e, "Startup failed");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

async fn run() -> Result<(), InfraError> {
    let app_state = init_app_state().await?;

    let bind_addr = app_state.config.bind_addr;

    let app = create_app(app_state.clone());

    // Spawn domain verification background task (after tracing is initialized)
    let domain_use_cases = app_state.domain_use_cases.clone();
    tokio::spawn(async move {
        run_domain_verification_loop(domain_use_cases).await;
    });

    // Spawn webhook delivery background worker
    let webhook_use_cases = app_state.webhook_use_cases.clone();
    tokio::spawn(async move {
        run_webhook_delivery_loop(webhook_use_cases).await;
    });

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(InfraError::TcpBind)?;

    info!(
        "Backend listening at {}",
        &listener.local_addr().map_err(InfraError::Server)?
    );

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(InfraError::Server)?;

    Ok(())
}
