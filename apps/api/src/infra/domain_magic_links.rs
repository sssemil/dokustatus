use async_trait::async_trait;
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    app_error::{AppError, AppResult},
    application::use_cases::domain_auth::{DomainMagicLinkData, DomainMagicLinkStore as DomainMagicLinkStoreTrait},
};

#[derive(Clone)]
pub struct DomainMagicLinkStore {
    manager: ConnectionManager,
}

#[derive(Serialize, Deserialize)]
struct StoredData {
    end_user_id: Uuid,
    domain_id: Uuid,
}

impl DomainMagicLinkStore {
    pub fn new(manager: ConnectionManager) -> Self {
        Self { manager }
    }

    fn key(token_hash: &str) -> String {
        format!("magic:domain:{token_hash}")
    }
}

#[async_trait]
impl DomainMagicLinkStoreTrait for DomainMagicLinkStore {
    async fn save(&self, token_hash: &str, end_user_id: Uuid, domain_id: Uuid, ttl_minutes: i64) -> AppResult<()> {
        let mut conn = self.manager.clone();
        let key = Self::key(token_hash);
        let ttl_secs: u64 = (ttl_minutes.max(1) * 60) as u64;

        let data = StoredData { end_user_id, domain_id };
        let json = serde_json::to_string(&data)
            .map_err(|e| AppError::Internal(format!("Failed to serialize magic link data: {e}")))?;

        let _: () = conn
            .set_ex(key, json, ttl_secs)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn consume(&self, token_hash: &str) -> AppResult<Option<DomainMagicLinkData>> {
        let mut conn = self.manager.clone();
        let key = Self::key(token_hash);

        let raw: Option<String> = redis::cmd("GETDEL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        raw.map(|value| {
            let data: StoredData = serde_json::from_str(&value)
                .map_err(|e| AppError::Internal(format!("Failed to parse magic link data: {e}")))?;
            Ok(DomainMagicLinkData {
                end_user_id: data.end_user_id,
                domain_id: data.domain_id,
            })
        })
        .transpose()
    }
}
