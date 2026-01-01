use async_trait::async_trait;
use redis::{AsyncCommands, aio::ConnectionManager};

use crate::{
    app_error::{AppError, AppResult},
    application::use_cases::domain_auth::{
        MarkStateResult, OAuthCompletionData, OAuthLinkConfirmationData, OAuthStateData,
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

    async fn mark_state_in_use(
        &self,
        state: &str,
        retry_window_secs: i64,
    ) -> AppResult<MarkStateResult> {
        let mut conn = self.manager.clone();
        let key = Self::state_key(state);

        let min_ttl = retry_window_secs + 30;

        let script = redis::Script::new(
            r#"
            local value = redis.call('GET', KEYS[1])
            if not value then
                return {1, nil}
            end

            local data = cjson.decode(value)
            local retry_window = tonumber(ARGV[1])
            local min_ttl = tonumber(ARGV[2])

            local time_result = redis.call('TIME')
            local now = tonumber(time_result[1])

            if data.status == 'in_use' then
                local marked_at = data.marked_at or 0
                if (now - marked_at) > retry_window then
                    return {2, nil}
                end
                redis.call('EXPIRE', KEYS[1], min_ttl)
                return {0, value}
            end

            data.status = 'in_use'
            data.marked_at = now
            local new_value = cjson.encode(data)
            redis.call('SET', KEYS[1], new_value, 'EX', min_ttl)
            return {0, new_value}
            "#,
        );

        let result: (i32, Option<String>) = script
            .key(&key)
            .arg(retry_window_secs)
            .arg(min_ttl)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to mark OAuth state in-use: {e}")))?;

        match result.0 {
            0 => {
                let json = result.1.ok_or_else(|| {
                    AppError::Internal("Lua returned success but no data".into())
                })?;
                let data: OAuthStateData = serde_json::from_str(&json)
                    .map_err(|e| AppError::Internal(format!("Failed to parse OAuth state: {e}")))?;
                Ok(MarkStateResult::Success(data))
            }
            1 => Ok(MarkStateResult::NotFound),
            2 => Ok(MarkStateResult::RetryWindowExpired),
            _ => Err(AppError::Internal(format!(
                "Unknown status code from Lua: {}",
                result.0
            ))),
        }
    }

    async fn complete_state(&self, state: &str) -> AppResult<()> {
        let mut conn = self.manager.clone();
        let key = Self::state_key(state);
        let _: () = conn
            .del(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to complete OAuth state: {e}")))?;
        Ok(())
    }

    async fn abort_state(&self, state: &str) -> AppResult<()> {
        self.complete_state(state).await
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
