use dotenvy::dotenv;
use tracing::info;

use reauth_api::infra::{
    app::create_app, domain_verifier::run_domain_verification_loop, setup::init_app_state,
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let app_state = init_app_state().await?;

    let bind_addr = app_state.config.bind_addr;

    let app = create_app(app_state.clone());

    // Spawn domain verification background task (after tracing is initialized)
    let domain_use_cases = app_state.domain_use_cases.clone();
    tokio::spawn(async move {
        run_domain_verification_loop(domain_use_cases).await;
    });

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    info!("Backend listening at {}", &listener.local_addr()?);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
