use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::{
    app_error::{AppError, AppResult},
    application::use_cases::webhook::{
        WebhookDeliveryProfile, WebhookDeliveryRepoTrait, WebhookDeliveryWithDetails,
        WebhookEndpointProfile, WebhookEndpointRepoTrait, WebhookEventProfile,
        WebhookEventRepoTrait,
    },
};

// ============================================================================
// In-Memory Webhook Endpoint Repo
// ============================================================================

#[derive(Default)]
pub struct InMemoryWebhookEndpointRepo {
    endpoints: Mutex<HashMap<Uuid, WebhookEndpointProfile>>,
}

impl InMemoryWebhookEndpointRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WebhookEndpointRepoTrait for InMemoryWebhookEndpointRepo {
    async fn create(
        &self,
        domain_id: Uuid,
        url: &str,
        description: Option<&str>,
        secret_encrypted: &str,
        event_types: &JsonValue,
    ) -> AppResult<WebhookEndpointProfile> {
        let now = chrono::Utc::now().naive_utc();
        let endpoint = WebhookEndpointProfile {
            id: Uuid::new_v4(),
            domain_id,
            url: url.to_string(),
            description: description.map(|d| d.to_string()),
            secret_encrypted: secret_encrypted.to_string(),
            event_types: event_types.clone(),
            is_active: true,
            consecutive_failures: 0,
            last_success_at: None,
            last_failure_at: None,
            created_at: Some(now),
            updated_at: Some(now),
        };
        self.endpoints
            .lock()
            .unwrap()
            .insert(endpoint.id, endpoint.clone());
        Ok(endpoint)
    }

    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<WebhookEndpointProfile>> {
        Ok(self.endpoints.lock().unwrap().get(&id).cloned())
    }

    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<WebhookEndpointProfile>> {
        Ok(self
            .endpoints
            .lock()
            .unwrap()
            .values()
            .filter(|e| e.domain_id == domain_id)
            .cloned()
            .collect())
    }

    async fn list_active_for_event(
        &self,
        domain_id: Uuid,
        _event_type: &str,
    ) -> AppResult<Vec<WebhookEndpointProfile>> {
        Ok(self
            .endpoints
            .lock()
            .unwrap()
            .values()
            .filter(|e| e.domain_id == domain_id && e.is_active)
            .cloned()
            .collect())
    }

    async fn update(
        &self,
        id: Uuid,
        url: Option<&str>,
        description: Option<Option<&str>>,
        event_types: Option<&JsonValue>,
        is_active: Option<bool>,
    ) -> AppResult<WebhookEndpointProfile> {
        let mut endpoints = self.endpoints.lock().unwrap();
        let endpoint = endpoints.get_mut(&id).ok_or(AppError::NotFound)?;
        if let Some(u) = url {
            endpoint.url = u.to_string();
        }
        if let Some(d) = description {
            endpoint.description = d.map(|s| s.to_string());
        }
        if let Some(et) = event_types {
            endpoint.event_types = et.clone();
        }
        if let Some(a) = is_active {
            endpoint.is_active = a;
        }
        endpoint.updated_at = Some(chrono::Utc::now().naive_utc());
        Ok(endpoint.clone())
    }

    async fn update_secret(&self, id: Uuid, secret_encrypted: &str) -> AppResult<()> {
        let mut endpoints = self.endpoints.lock().unwrap();
        let endpoint = endpoints.get_mut(&id).ok_or(AppError::NotFound)?;
        endpoint.secret_encrypted = secret_encrypted.to_string();
        Ok(())
    }

    async fn record_success(&self, id: Uuid) -> AppResult<()> {
        let mut endpoints = self.endpoints.lock().unwrap();
        if let Some(ep) = endpoints.get_mut(&id) {
            ep.consecutive_failures = 0;
            ep.last_success_at = Some(chrono::Utc::now().naive_utc());
        }
        Ok(())
    }

    async fn record_failure(&self, id: Uuid) -> AppResult<()> {
        let mut endpoints = self.endpoints.lock().unwrap();
        if let Some(ep) = endpoints.get_mut(&id) {
            ep.consecutive_failures += 1;
            ep.last_failure_at = Some(chrono::Utc::now().naive_utc());
        }
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> AppResult<()> {
        self.endpoints
            .lock()
            .unwrap()
            .remove(&id)
            .ok_or(AppError::NotFound)?;
        Ok(())
    }

    async fn count_by_domain(&self, domain_id: Uuid) -> AppResult<i64> {
        Ok(self
            .endpoints
            .lock()
            .unwrap()
            .values()
            .filter(|e| e.domain_id == domain_id)
            .count() as i64)
    }
}

// ============================================================================
// In-Memory Webhook Event Repo
// ============================================================================

#[derive(Default)]
pub struct InMemoryWebhookEventRepo {
    events: Mutex<HashMap<Uuid, WebhookEventProfile>>,
}

impl InMemoryWebhookEventRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WebhookEventRepoTrait for InMemoryWebhookEventRepo {
    async fn create(
        &self,
        domain_id: Uuid,
        event_type: &str,
        payload: &JsonValue,
        payload_raw: &str,
    ) -> AppResult<WebhookEventProfile> {
        let now = chrono::Utc::now().naive_utc();
        let event = WebhookEventProfile {
            id: Uuid::new_v4(),
            domain_id,
            event_type: event_type.to_string(),
            payload: payload.clone(),
            payload_raw: payload_raw.to_string(),
            created_at: Some(now),
        };
        self.events.lock().unwrap().insert(event.id, event.clone());
        Ok(event)
    }

    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<WebhookEventProfile>> {
        Ok(self.events.lock().unwrap().get(&id).cloned())
    }

    async fn list_by_domain(
        &self,
        domain_id: Uuid,
        event_type_filter: Option<&str>,
        limit: i64,
        _offset: i64,
    ) -> AppResult<Vec<WebhookEventProfile>> {
        Ok(self
            .events
            .lock()
            .unwrap()
            .values()
            .filter(|e| {
                e.domain_id == domain_id
                    && event_type_filter
                        .map_or(true, |f| e.event_type == f)
            })
            .take(limit as usize)
            .cloned()
            .collect())
    }
}

// ============================================================================
// In-Memory Webhook Delivery Repo
// ============================================================================

#[derive(Default)]
pub struct InMemoryWebhookDeliveryRepo {
    deliveries: Mutex<HashMap<Uuid, WebhookDeliveryProfile>>,
}

impl InMemoryWebhookDeliveryRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WebhookDeliveryRepoTrait for InMemoryWebhookDeliveryRepo {
    async fn create(
        &self,
        event_id: Uuid,
        endpoint_id: Uuid,
    ) -> AppResult<WebhookDeliveryProfile> {
        let now = chrono::Utc::now().naive_utc();
        let delivery = WebhookDeliveryProfile {
            id: Uuid::new_v4(),
            webhook_event_id: event_id,
            webhook_endpoint_id: endpoint_id,
            status: "pending".to_string(),
            attempt_count: 0,
            next_attempt_at: Some(now),
            locked_at: None,
            last_response_status: None,
            last_response_body: None,
            last_error: None,
            completed_at: None,
            created_at: Some(now),
        };
        self.deliveries
            .lock()
            .unwrap()
            .insert(delivery.id, delivery.clone());
        Ok(delivery)
    }

    async fn claim_pending_batch(&self, _limit: i64) -> AppResult<Vec<WebhookDeliveryWithDetails>> {
        Ok(vec![])
    }

    async fn mark_succeeded(&self, id: Uuid, response_status: i32) -> AppResult<()> {
        let mut deliveries = self.deliveries.lock().unwrap();
        if let Some(d) = deliveries.get_mut(&id) {
            d.status = "succeeded".to_string();
            d.last_response_status = Some(response_status);
            d.completed_at = Some(chrono::Utc::now().naive_utc());
        }
        Ok(())
    }

    async fn mark_failed(
        &self,
        id: Uuid,
        attempt_count: i32,
        next_attempt_at: NaiveDateTime,
        response_status: Option<i32>,
        response_body: Option<&str>,
        error: Option<&str>,
    ) -> AppResult<()> {
        let mut deliveries = self.deliveries.lock().unwrap();
        if let Some(d) = deliveries.get_mut(&id) {
            d.status = "failed".to_string();
            d.attempt_count = attempt_count;
            d.next_attempt_at = Some(next_attempt_at);
            d.last_response_status = response_status;
            d.last_response_body = response_body.map(|s| s.to_string());
            d.last_error = error.map(|s| s.to_string());
        }
        Ok(())
    }

    async fn mark_abandoned(
        &self,
        id: Uuid,
        response_status: Option<i32>,
        response_body: Option<&str>,
        error: Option<&str>,
    ) -> AppResult<()> {
        let mut deliveries = self.deliveries.lock().unwrap();
        if let Some(d) = deliveries.get_mut(&id) {
            d.status = "abandoned".to_string();
            d.last_response_status = response_status;
            d.last_response_body = response_body.map(|s| s.to_string());
            d.last_error = error.map(|s| s.to_string());
            d.completed_at = Some(chrono::Utc::now().naive_utc());
        }
        Ok(())
    }

    async fn release_stale(&self, _threshold_secs: i64) -> AppResult<i64> {
        Ok(0)
    }

    async fn list_by_event(
        &self,
        event_id: Uuid,
        limit: i64,
        _offset: i64,
    ) -> AppResult<Vec<WebhookDeliveryProfile>> {
        Ok(self
            .deliveries
            .lock()
            .unwrap()
            .values()
            .filter(|d| d.webhook_event_id == event_id)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn list_by_endpoint(
        &self,
        endpoint_id: Uuid,
        limit: i64,
        _offset: i64,
    ) -> AppResult<Vec<WebhookDeliveryProfile>> {
        Ok(self
            .deliveries
            .lock()
            .unwrap()
            .values()
            .filter(|d| d.webhook_endpoint_id == endpoint_id)
            .take(limit as usize)
            .cloned()
            .collect())
    }
}

// ============================================================================
// Stub Implementations (kept for backward compatibility with tests that
// don't need functional webhook repos)
// ============================================================================

#[derive(Default)]
pub struct StubWebhookEndpointRepo;

#[async_trait]
impl WebhookEndpointRepoTrait for StubWebhookEndpointRepo {
    async fn create(
        &self,
        _domain_id: Uuid,
        _url: &str,
        _description: Option<&str>,
        _secret_encrypted: &str,
        _event_types: &JsonValue,
    ) -> AppResult<WebhookEndpointProfile> {
        unimplemented!("not needed for tests")
    }

    async fn get_by_id(&self, _id: Uuid) -> AppResult<Option<WebhookEndpointProfile>> {
        Ok(None)
    }

    async fn list_by_domain(&self, _domain_id: Uuid) -> AppResult<Vec<WebhookEndpointProfile>> {
        Ok(vec![])
    }

    async fn list_active_for_event(
        &self,
        _domain_id: Uuid,
        _event_type: &str,
    ) -> AppResult<Vec<WebhookEndpointProfile>> {
        Ok(vec![])
    }

    async fn update(
        &self,
        _id: Uuid,
        _url: Option<&str>,
        _description: Option<Option<&str>>,
        _event_types: Option<&JsonValue>,
        _is_active: Option<bool>,
    ) -> AppResult<WebhookEndpointProfile> {
        unimplemented!("not needed for tests")
    }

    async fn update_secret(&self, _id: Uuid, _secret_encrypted: &str) -> AppResult<()> {
        Ok(())
    }

    async fn record_success(&self, _id: Uuid) -> AppResult<()> {
        Ok(())
    }

    async fn record_failure(&self, _id: Uuid) -> AppResult<()> {
        Ok(())
    }

    async fn delete(&self, _id: Uuid) -> AppResult<()> {
        Ok(())
    }

    async fn count_by_domain(&self, _domain_id: Uuid) -> AppResult<i64> {
        Ok(0)
    }
}

#[derive(Default)]
pub struct StubWebhookEventRepo;

#[async_trait]
impl WebhookEventRepoTrait for StubWebhookEventRepo {
    async fn create(
        &self,
        _domain_id: Uuid,
        _event_type: &str,
        _payload: &JsonValue,
        _payload_raw: &str,
    ) -> AppResult<WebhookEventProfile> {
        unimplemented!("not needed for tests")
    }

    async fn get_by_id(&self, _id: Uuid) -> AppResult<Option<WebhookEventProfile>> {
        Ok(None)
    }

    async fn list_by_domain(
        &self,
        _domain_id: Uuid,
        _event_type_filter: Option<&str>,
        _limit: i64,
        _offset: i64,
    ) -> AppResult<Vec<WebhookEventProfile>> {
        Ok(vec![])
    }
}

#[derive(Default)]
pub struct StubWebhookDeliveryRepo;

#[async_trait]
impl WebhookDeliveryRepoTrait for StubWebhookDeliveryRepo {
    async fn create(
        &self,
        _event_id: Uuid,
        _endpoint_id: Uuid,
    ) -> AppResult<WebhookDeliveryProfile> {
        unimplemented!("not needed for tests")
    }

    async fn claim_pending_batch(&self, _limit: i64) -> AppResult<Vec<WebhookDeliveryWithDetails>> {
        Ok(vec![])
    }

    async fn mark_succeeded(&self, _id: Uuid, _response_status: i32) -> AppResult<()> {
        Ok(())
    }

    async fn mark_failed(
        &self,
        _id: Uuid,
        _attempt_count: i32,
        _next_attempt_at: NaiveDateTime,
        _response_status: Option<i32>,
        _response_body: Option<&str>,
        _error: Option<&str>,
    ) -> AppResult<()> {
        Ok(())
    }

    async fn mark_abandoned(
        &self,
        _id: Uuid,
        _response_status: Option<i32>,
        _response_body: Option<&str>,
        _error: Option<&str>,
    ) -> AppResult<()> {
        Ok(())
    }

    async fn release_stale(&self, _threshold_secs: i64) -> AppResult<i64> {
        Ok(0)
    }

    async fn list_by_event(
        &self,
        _event_id: Uuid,
        _limit: i64,
        _offset: i64,
    ) -> AppResult<Vec<WebhookDeliveryProfile>> {
        Ok(vec![])
    }

    async fn list_by_endpoint(
        &self,
        _endpoint_id: Uuid,
        _limit: i64,
        _offset: i64,
    ) -> AppResult<Vec<WebhookDeliveryProfile>> {
        Ok(vec![])
    }
}
