use async_trait::async_trait;
use redis::{AsyncCommands, aio::ConnectionManager};

use crate::{
    app_error::{AppError, AppResult},
    application::use_cases::domain_auth::{
        OAuthCompletionData, OAuthLinkConfirmationData, OAuthStateData,
        OAuthStateStore as OAuthStateStoreTrait,
    },
};

#[derive(Clone)]
pub struct OAuthStateStore {
    manager: ConnectionManager,
}

impl OAuthStateStore {
    pub fn new(manager: ConnectionManager) -> Self {
        Self { manager }
    }

    fn state_key(state: &str) -> String {
        format!("oauth_state:{state}")
    }

    fn completion_key(token: &str) -> String {
        format!("oauth_completion:{token}")
    }

    fn link_confirmation_key(token: &str) -> String {
        format!("oauth_link_confirm:{token}")
    }
}

#[async_trait]
impl OAuthStateStoreTrait for OAuthStateStore {
    async fn store_state(
        &self,
        state: &str,
        data: &OAuthStateData,
        ttl_minutes: i64,
    ) -> AppResult<()> {
        let mut conn = self.manager.clone();
        let key = Self::state_key(state);
        let ttl_secs: u64 = (ttl_minutes.max(1) * 60) as u64;

        let json = serde_json::to_string(data)
            .map_err(|e| AppError::Internal(format!("Failed to serialize OAuth state: {e}")))?;

        let _: () = conn
            .set_ex(key, json, ttl_secs)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn consume_state(&self, state: &str) -> AppResult<Option<OAuthStateData>> {
        let mut conn = self.manager.clone();
        let key = Self::state_key(state);

        // Use Lua script for atomic GET + DELETE (single-use consumption)
        // This prevents race conditions where two parallel requests could both succeed
        let script = redis::Script::new(
            r#"
            local value = redis.call('GET', KEYS[1])
            if value then
                redis.call('DEL', KEYS[1])
            end
            return value
            "#,
        );

        let raw: Option<String> = script
            .key(&key)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to consume OAuth state: {e}")))?;

        match raw {
            Some(value) => {
                let data: OAuthStateData = serde_json::from_str(&value)
                    .map_err(|e| AppError::Internal(format!("Failed to parse OAuth state: {e}")))?;
                Ok(Some(data))
            }
            None => Ok(None), // State not found (expired, consumed, or never existed)
        }
    }

    async fn store_completion(
        &self,
        token: &str,
        data: &OAuthCompletionData,
        ttl_minutes: i64,
    ) -> AppResult<()> {
        let mut conn = self.manager.clone();
        let key = Self::completion_key(token);
        let ttl_secs: u64 = (ttl_minutes.max(1) * 60) as u64;

        let json = serde_json::to_string(data).map_err(|e| {
            AppError::Internal(format!("Failed to serialize OAuth completion: {e}"))
        })?;

        let _: () = conn
            .set_ex(key, json, ttl_secs)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn consume_completion(&self, token: &str) -> AppResult<Option<OAuthCompletionData>> {
        let mut conn = self.manager.clone();
        let key = Self::completion_key(token);

        // Use Lua script for atomic GET + DELETE (single-use consumption)
        let script = redis::Script::new(
            r#"
            local value = redis.call('GET', KEYS[1])
            if value then
                redis.call('DEL', KEYS[1])
            end
            return value
            "#,
        );

        let raw: Option<String> = script
            .key(&key)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to consume OAuth completion: {e}")))?;

        match raw {
            Some(value) => {
                let data: OAuthCompletionData = serde_json::from_str(&value).map_err(|e| {
                    AppError::Internal(format!("Failed to parse OAuth completion: {e}"))
                })?;
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }

    async fn store_link_confirmation(
        &self,
        token: &str,
        data: &OAuthLinkConfirmationData,
        ttl_minutes: i64,
    ) -> AppResult<()> {
        let mut conn = self.manager.clone();
        let key = Self::link_confirmation_key(token);
        let ttl_secs: u64 = (ttl_minutes.max(1) * 60) as u64;

        let json = serde_json::to_string(data).map_err(|e| {
            AppError::Internal(format!("Failed to serialize link confirmation: {e}"))
        })?;

        let _: () = conn
            .set_ex(key, json, ttl_secs)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn consume_link_confirmation(
        &self,
        token: &str,
    ) -> AppResult<Option<OAuthLinkConfirmationData>> {
        let mut conn = self.manager.clone();
        let key = Self::link_confirmation_key(token);

        // Use Lua script for atomic GET + DELETE (single-use consumption)
        let script = redis::Script::new(
            r#"
            local value = redis.call('GET', KEYS[1])
            if value then
                redis.call('DEL', KEYS[1])
            end
            return value
            "#,
        );

        let raw: Option<String> = script
            .key(&key)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to consume link confirmation: {e}")))?;

        match raw {
            Some(value) => {
                let data: OAuthLinkConfirmationData =
                    serde_json::from_str(&value).map_err(|e| {
                        AppError::Internal(format!("Failed to parse link confirmation: {e}"))
                    })?;
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }
}
