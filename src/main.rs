use dotenvy::dotenv;
use tracing::info;

use dokustatus::infra::{app::create_app, setup::init_app_state};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let app_state = init_app_state().await?;

    // Read bind address from config before moving app_state
    let bind_addr = app_state.config.bind_addr;

    let app = create_app(app_state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    info!("Backend listening at {}", &listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}
