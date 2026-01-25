use crate::{adapters::persistence::PostgresPersistence, infra::db::init_db};

pub mod app;
pub mod config;
pub mod crypto;
pub mod db;
pub mod domain_email;
pub mod domain_magic_links;
pub mod domain_verifier;
pub mod dummy_payment_client;
pub mod error;
pub mod http_client;
pub mod key_derivation;
pub mod oauth_state;
pub mod rate_limit;
pub mod setup;
pub mod stripe_client;
pub mod stripe_payment_adapter;

pub use error::InfraError;

pub async fn postgres_persistence(database_url: &str) -> Result<PostgresPersistence, InfraError> {
    let pool = init_db(database_url).await?;
    let persistence = PostgresPersistence::new(pool);
    Ok(persistence)
}
