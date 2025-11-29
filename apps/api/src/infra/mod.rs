use crate::{adapters::persistence::PostgresPersistence, infra::db::init_db};

pub mod app;
pub mod config;
pub mod crypto;
pub mod db;
pub mod magic_links;
pub mod rate_limit;
pub mod setup;

pub async fn postgres_persistence(
    cipher: std::sync::Arc<crypto::ProcessCipher>,
    database_url: &str,
) -> anyhow::Result<PostgresPersistence> {
    let pool = init_db(database_url).await?;
    let persistence = PostgresPersistence::new(pool, cipher);
    Ok(persistence)
}
