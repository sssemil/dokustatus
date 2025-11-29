use crate::{adapters::persistence::PostgresPersistence, infra::db::init_db};

pub mod app;
pub mod config;
pub mod db;
pub mod rate_limit;
pub mod setup;

pub async fn postgres_persistence() -> anyhow::Result<PostgresPersistence> {
    let pool = init_db().await?;
    let persistence = PostgresPersistence::new(pool);
    Ok(persistence)
}
