use crate::{adapters::persistence::PostgresPersistence, infra::db::init_db};

pub mod app;
pub mod config;
pub mod crypto;
pub mod db;
pub mod domain_email;
pub mod domain_magic_links;
pub mod domain_verifier;
pub mod dummy_payment_client;
pub mod oauth_state;
pub mod rate_limit;
pub mod setup;
pub mod stripe_client;
pub mod stripe_payment_adapter;

pub async fn postgres_persistence(database_url: &str) -> anyhow::Result<PostgresPersistence> {
    let pool = init_db(database_url).await?;
    let persistence = PostgresPersistence::new(pool);
    Ok(persistence)
}
