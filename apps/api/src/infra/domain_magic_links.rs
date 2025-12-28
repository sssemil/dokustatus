use async_trait::async_trait;
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    app_error::{AppError, AppResult},
    application::use_cases::domain_auth::{
        DomainMagicLinkData, DomainMagicLinkStore as DomainMagicLinkStoreTrait,
    },
};

#[derive(Clone)]
pub struct DomainMagicLinkStore {
    manager: ConnectionManager,
}

#[derive(Serialize, Deserialize)]
struct StoredData {
    end_user_id: Uuid,
    domain_id: Uuid,
    session_id: String,
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
    async fn save(
        &self,
        token_hash: &str,
        end_user_id: Uuid,
        domain_id: Uuid,
        session_id: &str,
        ttl_minutes: i64,
    ) -> AppResult<()> {
        let mut conn = self.manager.clone();
        let key = Self::key(token_hash);
        let ttl_secs: u64 = (ttl_minutes.max(1) * 60) as u64;

        let data = StoredData {
            end_user_id,
            domain_id,
            session_id: session_id.to_string(),
        };
        let json = serde_json::to_string(&data)
            .map_err(|e| AppError::Internal(format!("Failed to serialize magic link data: {e}")))?;

        let _: () = conn
            .set_ex(key, json, ttl_secs)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn consume(
        &self,
        token_hash: &str,
        session_id: &str,
    ) -> AppResult<Option<DomainMagicLinkData>> {
        let mut conn = self.manager.clone();
        let key = Self::key(token_hash);

        // First GET to check if exists and verify session (don't delete yet)
        let raw: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        match raw {
            Some(value) => {
                let data: StoredData = serde_json::from_str(&value).map_err(|e| {
                    AppError::Internal(format!("Failed to parse magic link data: {e}"))
                })?;

                // Check if session matches
                if data.session_id != session_id {
                    // Token exists but different browser/device - don't consume, return error
                    return Err(AppError::SessionMismatch);
                }

                // Session matches, now delete the token
                let _: () = conn
                    .del(&key)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                Ok(Some(DomainMagicLinkData {
                    end_user_id: data.end_user_id,
                    domain_id: data.domain_id,
                }))
            }
            None => Ok(None), // Token not found (expired or never existed)
        }
    }
}
