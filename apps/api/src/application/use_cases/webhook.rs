use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use chrono::NaiveDateTime;
use rand::RngCore;
use serde::Serialize;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::app_error::{AppError, AppResult};
use crate::application::use_cases::domain::DomainRepoTrait;
use crate::domain::entities::webhook::WebhookEventType;
use crate::infra::crypto::ProcessCipher;

// ============================================================================
// Constants
// ============================================================================

pub const MAX_WEBHOOK_ENDPOINTS_PER_DOMAIN: i64 = 10;
pub const MAX_DELIVERY_ATTEMPTS: i32 = 5;
pub const STALE_LOCK_THRESHOLD_SECS: i64 = 300; // 5 minutes
pub const WEBHOOK_API_VERSION: &str = "2026-01-29";

// ============================================================================
// Repository Traits
// ============================================================================

#[async_trait]
pub trait WebhookEndpointRepoTrait: Send + Sync {
    async fn create(
        &self,
        domain_id: Uuid,
        url: &str,
        description: Option<&str>,
        secret_encrypted: &str,
        event_types: &JsonValue,
    ) -> AppResult<WebhookEndpointProfile>;

    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<WebhookEndpointProfile>>;

    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<WebhookEndpointProfile>>;

    async fn list_active_for_event(
        &self,
        domain_id: Uuid,
        event_type: &str,
    ) -> AppResult<Vec<WebhookEndpointProfile>>;

    async fn update(
        &self,
        id: Uuid,
        url: Option<&str>,
        description: Option<Option<&str>>,
        event_types: Option<&JsonValue>,
        is_active: Option<bool>,
    ) -> AppResult<WebhookEndpointProfile>;

    async fn update_secret(&self, id: Uuid, secret_encrypted: &str) -> AppResult<()>;

    async fn record_success(&self, id: Uuid) -> AppResult<()>;

    async fn record_failure(&self, id: Uuid) -> AppResult<()>;

    async fn delete(&self, id: Uuid) -> AppResult<()>;

    async fn count_by_domain(&self, domain_id: Uuid) -> AppResult<i64>;
}

#[async_trait]
pub trait WebhookEventRepoTrait: Send + Sync {
    async fn create(
        &self,
        domain_id: Uuid,
        event_type: &str,
        payload: &JsonValue,
        payload_raw: &str,
    ) -> AppResult<WebhookEventProfile>;

    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<WebhookEventProfile>>;

    async fn list_by_domain(
        &self,
        domain_id: Uuid,
        event_type_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<WebhookEventProfile>>;
}

#[async_trait]
pub trait WebhookDeliveryRepoTrait: Send + Sync {
    async fn create(&self, event_id: Uuid, endpoint_id: Uuid) -> AppResult<WebhookDeliveryProfile>;

    async fn claim_pending_batch(&self, limit: i64) -> AppResult<Vec<WebhookDeliveryWithDetails>>;

    async fn mark_succeeded(&self, id: Uuid, response_status: i32) -> AppResult<()>;

    async fn mark_failed(
        &self,
        id: Uuid,
        attempt_count: i32,
        next_attempt_at: NaiveDateTime,
        response_status: Option<i32>,
        response_body: Option<&str>,
        error: Option<&str>,
    ) -> AppResult<()>;

    async fn mark_abandoned(
        &self,
        id: Uuid,
        response_status: Option<i32>,
        response_body: Option<&str>,
        error: Option<&str>,
    ) -> AppResult<()>;

    async fn release_stale(&self, threshold_secs: i64) -> AppResult<i64>;

    async fn list_by_event(
        &self,
        event_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<WebhookDeliveryProfile>>;

    async fn list_by_endpoint(
        &self,
        endpoint_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<WebhookDeliveryProfile>>;
}

// ============================================================================
// Profile Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct WebhookEndpointProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub url: String,
    pub description: Option<String>,
    pub secret_encrypted: String,
    pub event_types: JsonValue,
    pub is_active: bool,
    pub consecutive_failures: i32,
    pub last_success_at: Option<NaiveDateTime>,
    pub last_failure_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebhookEventProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub event_type: String,
    pub payload: JsonValue,
    pub payload_raw: String,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WebhookDeliveryProfile {
    pub id: Uuid,
    pub webhook_event_id: Uuid,
    pub webhook_endpoint_id: Uuid,
    pub status: String,
    pub attempt_count: i32,
    pub next_attempt_at: Option<NaiveDateTime>,
    pub locked_at: Option<NaiveDateTime>,
    pub last_response_status: Option<i32>,
    pub last_response_body: Option<String>,
    pub last_error: Option<String>,
    pub completed_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct WebhookDeliveryWithDetails {
    pub delivery_id: Uuid,
    pub event_id: Uuid,
    pub endpoint_id: Uuid,
    pub attempt_count: i32,
    pub endpoint_url: String,
    pub secret_encrypted: String,
    pub payload_raw: String,
    pub event_type: String,
    pub event_created_at: Option<NaiveDateTime>,
}

// ============================================================================
// Use Cases
// ============================================================================

#[derive(Clone)]
pub struct WebhookUseCases {
    domain_repo: Arc<dyn DomainRepoTrait>,
    endpoint_repo: Arc<dyn WebhookEndpointRepoTrait>,
    event_repo: Arc<dyn WebhookEventRepoTrait>,
    delivery_repo: Arc<dyn WebhookDeliveryRepoTrait>,
    cipher: ProcessCipher,
}

impl WebhookUseCases {
    pub fn new(
        domain_repo: Arc<dyn DomainRepoTrait>,
        endpoint_repo: Arc<dyn WebhookEndpointRepoTrait>,
        event_repo: Arc<dyn WebhookEventRepoTrait>,
        delivery_repo: Arc<dyn WebhookDeliveryRepoTrait>,
        cipher: ProcessCipher,
    ) -> Self {
        Self {
            domain_repo,
            endpoint_repo,
            event_repo,
            delivery_repo,
            cipher,
        }
    }

    // ========================================================================
    // Endpoint CRUD
    // ========================================================================

    pub async fn create_endpoint(
        &self,
        domain_id: Uuid,
        url: &str,
        description: Option<&str>,
        event_types: Option<Vec<String>>,
    ) -> AppResult<(WebhookEndpointProfile, String)> {
        let count = self.endpoint_repo.count_by_domain(domain_id).await?;
        if count >= MAX_WEBHOOK_ENDPOINTS_PER_DOMAIN {
            return Err(AppError::InvalidInput(format!(
                "Maximum of {} webhook endpoints per domain",
                MAX_WEBHOOK_ENDPOINTS_PER_DOMAIN
            )));
        }

        self.validate_url(url, domain_id).await?;

        let event_types_value = self.validate_event_types(event_types)?;

        let (secret_plaintext, secret_encrypted) = self.generate_secret()?;

        let endpoint = self
            .endpoint_repo
            .create(
                domain_id,
                url,
                description,
                &secret_encrypted,
                &event_types_value,
            )
            .await?;

        Ok((endpoint, secret_plaintext))
    }

    pub async fn list_endpoints(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Vec<WebhookEndpointProfile>> {
        self.endpoint_repo.list_by_domain(domain_id).await
    }

    pub async fn get_endpoint(
        &self,
        endpoint_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<WebhookEndpointProfile> {
        let endpoint = self
            .endpoint_repo
            .get_by_id(endpoint_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if endpoint.domain_id != domain_id {
            return Err(AppError::Forbidden);
        }
        Ok(endpoint)
    }

    pub async fn update_endpoint(
        &self,
        endpoint_id: Uuid,
        domain_id: Uuid,
        url: Option<&str>,
        description: Option<Option<&str>>,
        event_types: Option<Vec<String>>,
        is_active: Option<bool>,
    ) -> AppResult<WebhookEndpointProfile> {
        let existing = self.get_endpoint(endpoint_id, domain_id).await?;

        if let Some(new_url) = url {
            self.validate_url(new_url, existing.domain_id).await?;
        }

        let event_types_value = event_types
            .map(|et| self.validate_event_types(Some(et)))
            .transpose()?;

        self.endpoint_repo
            .update(
                endpoint_id,
                url,
                description,
                event_types_value.as_ref(),
                is_active,
            )
            .await
    }

    pub async fn delete_endpoint(
        &self,
        endpoint_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<()> {
        self.get_endpoint(endpoint_id, domain_id).await?;
        self.endpoint_repo.delete(endpoint_id).await
    }

    pub async fn rotate_secret(
        &self,
        endpoint_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<String> {
        self.get_endpoint(endpoint_id, domain_id).await?;
        let (secret_plaintext, secret_encrypted) = self.generate_secret()?;
        self.endpoint_repo
            .update_secret(endpoint_id, &secret_encrypted)
            .await?;
        Ok(secret_plaintext)
    }

    // ========================================================================
    // Event Emission
    // ========================================================================

    pub async fn emit_event(
        &self,
        domain_id: Uuid,
        event_type: WebhookEventType,
        data: JsonValue,
    ) -> AppResult<WebhookEventProfile> {
        let event_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let payload = serde_json::json!({
            "id": format!("evt_{}", event_id),
            "type": event_type.as_str(),
            "api_version": WEBHOOK_API_VERSION,
            "created_at": now.to_rfc3339(),
            "domain_id": domain_id.to_string(),
            "data": data,
        });

        let payload_raw = serde_json::to_string(&payload).map_err(|e| {
            AppError::Internal(format!("failed to serialize webhook payload: {}", e))
        })?;

        let event = self
            .event_repo
            .create(domain_id, event_type.as_str(), &payload, &payload_raw)
            .await?;

        let endpoints = self
            .endpoint_repo
            .list_active_for_event(domain_id, event_type.as_str())
            .await?;

        for endpoint in &endpoints {
            if let Err(e) = self.delivery_repo.create(event.id, endpoint.id).await {
                tracing::error!(
                    endpoint_id = %endpoint.id,
                    event_id = %event.id,
                    error = %e,
                    "Failed to create webhook delivery"
                );
            }
        }

        Ok(event)
    }

    pub async fn send_test_event(
        &self,
        endpoint_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<WebhookEventProfile> {
        self.get_endpoint(endpoint_id, domain_id).await?;

        let data = serde_json::json!({
            "endpoint_id": endpoint_id.to_string(),
        });

        let event = self
            .emit_event(domain_id, WebhookEventType::WebhookTest, data)
            .await?;

        Ok(event)
    }

    // ========================================================================
    // Event & Delivery Queries
    // ========================================================================

    pub async fn list_events(
        &self,
        domain_id: Uuid,
        event_type_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<WebhookEventProfile>> {
        self.event_repo
            .list_by_domain(domain_id, event_type_filter, limit, offset)
            .await
    }

    pub async fn get_event(
        &self,
        event_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<WebhookEventProfile> {
        let event = self
            .event_repo
            .get_by_id(event_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if event.domain_id != domain_id {
            return Err(AppError::Forbidden);
        }
        Ok(event)
    }

    pub async fn list_deliveries_for_event(
        &self,
        event_id: Uuid,
        domain_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<WebhookDeliveryProfile>> {
        self.get_event(event_id, domain_id).await?;
        self.delivery_repo.list_by_event(event_id, limit, offset).await
    }

    pub async fn list_deliveries_for_endpoint(
        &self,
        endpoint_id: Uuid,
        domain_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<WebhookDeliveryProfile>> {
        self.get_endpoint(endpoint_id, domain_id).await?;
        self.delivery_repo
            .list_by_endpoint(endpoint_id, limit, offset)
            .await
    }

    // ========================================================================
    // Delivery Processing (called by background worker)
    // ========================================================================

    pub async fn claim_pending_deliveries(
        &self,
        batch_size: i64,
    ) -> AppResult<Vec<WebhookDeliveryWithDetails>> {
        self.delivery_repo.claim_pending_batch(batch_size).await
    }

    pub async fn record_delivery_success(
        &self,
        delivery_id: Uuid,
        endpoint_id: Uuid,
        response_status: i32,
    ) -> AppResult<()> {
        self.delivery_repo
            .mark_succeeded(delivery_id, response_status)
            .await?;
        self.endpoint_repo.record_success(endpoint_id).await?;
        Ok(())
    }

    pub async fn record_delivery_failure(
        &self,
        delivery_id: Uuid,
        endpoint_id: Uuid,
        attempt_count: i32,
        response_status: Option<i32>,
        response_body: Option<&str>,
        error: Option<&str>,
        is_terminal: bool,
    ) -> AppResult<()> {
        self.endpoint_repo.record_failure(endpoint_id).await?;

        if is_terminal || attempt_count >= MAX_DELIVERY_ATTEMPTS {
            let truncated_body = response_body.map(|b| &b[..b.len().min(1024)]);
            self.delivery_repo
                .mark_abandoned(delivery_id, response_status, truncated_body, error)
                .await?;
        } else {
            let delay = calculate_backoff_delay(attempt_count);
            let next_attempt =
                chrono::Utc::now().naive_utc() + chrono::Duration::seconds(delay);
            let truncated_body = response_body.map(|b| &b[..b.len().min(1024)]);
            self.delivery_repo
                .mark_failed(
                    delivery_id,
                    attempt_count,
                    next_attempt,
                    response_status,
                    truncated_body,
                    error,
                )
                .await?;
        }

        Ok(())
    }

    pub async fn release_stale_deliveries(&self) -> AppResult<i64> {
        self.delivery_repo
            .release_stale(STALE_LOCK_THRESHOLD_SECS)
            .await
    }

    pub fn decrypt_secret(&self, encrypted: &str) -> AppResult<String> {
        self.cipher
            .decrypt(encrypted)
            .map_err(|e| AppError::Internal(format!("failed to decrypt webhook secret: {}", e)))
    }

    // ========================================================================
    // Private Helpers
    // ========================================================================

    async fn validate_url(&self, url: &str, domain_id: Uuid) -> AppResult<()> {
        let parsed = url::Url::parse(url)
            .map_err(|_| AppError::InvalidInput("invalid URL format".to_string()))?;

        if parsed.scheme() != "https" {
            return Err(AppError::InvalidInput(
                "webhook URL must use HTTPS".to_string(),
            ));
        }

        let host = parsed
            .host_str()
            .ok_or_else(|| AppError::InvalidInput("webhook URL must have a host".to_string()))?;

        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;

        let verified_domain = &domain.domain;
        let is_under_domain = host == verified_domain
            || host.ends_with(&format!(".{}", verified_domain));

        if !is_under_domain {
            return Err(AppError::InvalidInput(format!(
                "webhook URL must be under your verified domain ({})",
                verified_domain
            )));
        }

        Ok(())
    }

    fn validate_event_types(&self, event_types: Option<Vec<String>>) -> AppResult<JsonValue> {
        let types = event_types.unwrap_or_else(|| vec!["*".to_string()]);

        if types.is_empty() {
            return Err(AppError::InvalidInput(
                "at least one event type is required".to_string(),
            ));
        }

        if types.contains(&"*".to_string()) {
            return Ok(serde_json::json!(["*"]));
        }

        let valid_types = WebhookEventType::all_type_strings();
        for t in &types {
            if !valid_types.contains(&t.as_str()) {
                return Err(AppError::InvalidInput(format!(
                    "unknown event type: {}",
                    t
                )));
            }
        }

        Ok(serde_json::json!(types))
    }

    fn generate_secret(&self) -> AppResult<(String, String)> {
        let mut secret_bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut secret_bytes);
        let secret_plaintext = format!(
            "whsec_{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(secret_bytes)
        );
        let secret_encrypted = self.cipher.encrypt(&secret_plaintext).map_err(|e| {
            AppError::Internal(format!("failed to encrypt webhook secret: {}", e))
        })?;
        Ok((secret_plaintext, secret_encrypted))
    }
}

// ============================================================================
// Backoff Calculation
// ============================================================================

pub fn calculate_backoff_delay(attempt_count: i32) -> i64 {
    let base_delay: i64 = 300; // 5 minutes in seconds
    let max_delay: i64 = 20_000; // ~5.5 hours cap
    let exponential = base_delay.saturating_mul(4i64.saturating_pow(attempt_count as u32));
    let capped = exponential.min(max_delay);
    let jitter = (rand::random::<u64>() % 60) as i64;
    capped + jitter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_delay_increases_exponentially() {
        let d1 = calculate_backoff_delay(0);
        let d2 = calculate_backoff_delay(1);
        let d3 = calculate_backoff_delay(2);

        assert!(d1 >= 300 && d1 < 360);
        assert!(d2 >= 1200 && d2 < 1260);
        assert!(d3 >= 4800 && d3 < 4860);
    }

    #[test]
    fn backoff_delay_is_capped() {
        let d = calculate_backoff_delay(10);
        assert!(d <= 20_060);
    }
}
