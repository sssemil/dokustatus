use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing::info;

pub async fn init_db(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .map_err(|e| {
            anyhow::anyhow!("Postgres connection failed (check DATABASE_URL/password): {e}")
        })?;

    info!("Connected to database!");
    Ok(pool)
}
