use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing::info;

use super::InfraError;

pub async fn init_db(database_url: &str) -> Result<PgPool, InfraError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?; // Uses From<sqlx::Error> impl

    info!("Connected to database!");
    Ok(pool)
}
