use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::{
    app_error::AppResult,
    application::use_cases::webhook::{
        WebhookDeliveryProfile, WebhookDeliveryRepoTrait, WebhookDeliveryWithDetails,
        WebhookEndpointProfile, WebhookEndpointRepoTrait, WebhookEventProfile,
        WebhookEventRepoTrait,
    },
};

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

    async fn claim_pending_batch(
        &self,
        _limit: i64,
    ) -> AppResult<Vec<WebhookDeliveryWithDetails>> {
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
