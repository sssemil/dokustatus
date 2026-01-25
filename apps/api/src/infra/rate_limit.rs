use async_trait::async_trait;
use redis::{Script, aio::ConnectionManager};

use super::InfraError;
use crate::app_error::{AppError, AppResult};

/// Trait for rate limiting implementations.
#[async_trait]
pub trait RateLimiterTrait: Send + Sync {
    /// Check rate limits for an IP and optional email.
    /// Returns Ok(()) if within limits, Err(AppError::RateLimited) if exceeded.
    async fn check(&self, ip: &str, email: Option<&str>) -> AppResult<()>;
}

/// Lua script for atomic increment with TTL.
/// Returns the new count after increment.
/// If the key doesn't exist, it's created with TTL.
/// If the key exists but has no TTL (edge case from old bug), TTL is set.
const INCR_WITH_TTL_SCRIPT: &str = r#"
local current = redis.call('INCR', KEYS[1])
if current == 1 then
    redis.call('EXPIRE', KEYS[1], ARGV[1])
elseif redis.call('TTL', KEYS[1]) == -1 then
    -- Key exists but has no TTL (shouldn't happen, but fix it)
    redis.call('EXPIRE', KEYS[1], ARGV[1])
end
return current
"#;

/// Redis-backed rate limiter for production use.
#[derive(Clone)]
pub struct RedisRateLimiter {
    manager: ConnectionManager,
    window_secs: u64,
    max_per_ip: u64,
    max_per_email: u64,
    script: Script,
}

impl RedisRateLimiter {
    pub async fn new(
        redis_url: &str,
        window_secs: u64,
        max_per_ip: u64,
        max_per_email: u64,
    ) -> Result<Self, InfraError> {
        let client = redis::Client::open(redis_url).map_err(InfraError::RedisConnection)?;
        let manager = ConnectionManager::new(client)
            .await
            .map_err(InfraError::RedisConnection)?;
        let script = Script::new(INCR_WITH_TTL_SCRIPT);
        Ok(Self {
            manager,
            window_secs,
            max_per_ip,
            max_per_email,
            script,
        })
    }

    async fn bump(&self, conn: &mut ConnectionManager, key: &str, limit: u64) -> AppResult<()> {
        let current: u64 = self
            .script
            .key(key)
            .arg(self.window_secs)
            .invoke_async(conn)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if current > limit {
            return Err(AppError::RateLimited);
        }

        Ok(())
    }
}

#[async_trait]
impl RateLimiterTrait for RedisRateLimiter {
    async fn check(&self, ip: &str, email: Option<&str>) -> AppResult<()> {
        let mut conn = self.manager.clone();
        self.bump(&mut conn, &format!("rate:ip:{ip}"), self.max_per_ip)
            .await?;

        if let Some(email) = email {
            let normalized = email.to_lowercase();
            self.bump(
                &mut conn,
                &format!("rate:email:{normalized}"),
                self.max_per_email,
            )
            .await?;
        }
        Ok(())
    }
}
