use env_helpers::get_env;

use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing::info;

pub async fn init_db() -> anyhow::Result<PgPool> {
    let database_url: String = get_env("DATABASE_URL");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    info!("Connected to database!");
    Ok(pool)
}
