use dotenvy::dotenv;
use tracing::info;

use dokustatus::infra::{app::create_app, setup::init_app_state};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let app_state = init_app_state().await?;

    // Read bind address from config before moving app_state
    let bind_addr = app_state.config.bind_addr;

    spawn_pass_status_poll(app_state.clone());

    let app = create_app(app_state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    info!("Backend listening at {}", &listener.local_addr()?);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

fn spawn_pass_status_poll(app_state: dokustatus::adapters::http::app_state::AppState) {
    tokio::spawn(async move {
        loop {
            match app_state.pass_status_use_cases.check_all_and_notify().await {
                Ok(res) => !res.is_empty(),
                Err(err) => {
                    tracing::error!(error = ?err, "pass status poll failed");
                    false
                }
            };
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
}
