use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

use crate::app_error::{AppError, AppResult};
use crate::infra::http_client;

const STRIPE_API_BASE: &str = "https://api.stripe.com/v1";

#[derive(Clone)]
pub struct StripeClient {
    client: Client,
    secret_key: String,
}

impl StripeClient {
    pub fn new(secret_key: String) -> Self {
        Self {
            client: http_client::build_client(),
            secret_key,
        }
    }

    fn auth_header(&self) -> String {
        use base64::Engine;
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(format!("{}:", self.secret_key));
        format!("Basic {}", encoded)
    }

    // ========================================================================
    // Products
    // ========================================================================

    pub async fn create_product(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> AppResult<StripeProduct> {
        let mut params = HashMap::new();
        params.insert("name", name.to_string());
        if let Some(desc) = description {
            params.insert("description", desc.to_string());
        }

        let response = self
            .client
            .post(format!("{}/products", STRIPE_API_BASE))
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        self.handle_response(response).await
    }

    // ========================================================================
    // Prices
    // ========================================================================

    pub async fn create_price(
        &self,
        product_id: &str,
        unit_amount: i64,
        currency: &str,
        interval: &str,
        interval_count: i32,
    ) -> AppResult<StripePrice> {
        let params: Vec<(&str, String)> = vec![
            ("product", product_id.to_string()),
            ("unit_amount", unit_amount.to_string()),
            ("currency", currency.to_lowercase()),
            ("recurring[interval]", interval.to_string()),
            ("recurring[interval_count]", interval_count.to_string()),
        ];

        let response = self
            .client
            .post(format!("{}/prices", STRIPE_API_BASE))
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        self.handle_response(response).await
    }

    // ========================================================================
    // Customers
    // ========================================================================

    pub async fn create_customer(
        &self,
        email: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> AppResult<StripeCustomer> {
        let mut params: Vec<(String, String)> = vec![("email".to_string(), email.to_string())];

        if let Some(meta) = metadata {
            for (key, value) in meta {
                params.push((format!("metadata[{}]", key), value));
            }
        }

        let response = self
            .client
            .post(format!("{}/customers", STRIPE_API_BASE))
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        self.handle_response(response).await
    }

    pub async fn get_or_create_customer(
        &self,
        email: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> AppResult<StripeCustomer> {
        // Search for existing customer by email
        let response = self
            .client
            .get(format!("{}/customers", STRIPE_API_BASE))
            .header("Authorization", self.auth_header())
            .query(&[("email", email), ("limit", "1")])
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        let list: StripeCustomerList = self.handle_response(response).await?;
        if let Some(customer) = list.data.into_iter().next() {
            return Ok(customer);
        }

        // Create new customer
        self.create_customer(email, metadata).await
    }

    // ========================================================================
    // Checkout Sessions
    // ========================================================================

    pub async fn create_checkout_session(
        &self,
        customer_id: &str,
        price_id: &str,
        success_url: &str,
        cancel_url: &str,
        client_reference_id: Option<&str>,
        trial_days: Option<i32>,
    ) -> AppResult<StripeCheckoutSession> {
        let mut params: Vec<(String, String)> = vec![
            ("customer".to_string(), customer_id.to_string()),
            ("mode".to_string(), "subscription".to_string()),
            ("line_items[0][price]".to_string(), price_id.to_string()),
            ("line_items[0][quantity]".to_string(), "1".to_string()),
            ("success_url".to_string(), success_url.to_string()),
            ("cancel_url".to_string(), cancel_url.to_string()),
        ];

        if let Some(ref_id) = client_reference_id {
            params.push(("client_reference_id".to_string(), ref_id.to_string()));
        }

        if let Some(days) = trial_days {
            if days > 0 {
                params.push((
                    "subscription_data[trial_period_days]".to_string(),
                    days.to_string(),
                ));
            }
        }

        let response = self
            .client
            .post(format!("{}/checkout/sessions", STRIPE_API_BASE))
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        self.handle_response(response).await
    }

    // ========================================================================
    // Customer Portal
    // ========================================================================

    pub async fn create_portal_session(
        &self,
        customer_id: &str,
        return_url: &str,
    ) -> AppResult<StripePortalSession> {
        let params = vec![("customer", customer_id), ("return_url", return_url)];

        let response = self
            .client
            .post(format!("{}/billing_portal/sessions", STRIPE_API_BASE))
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        self.handle_response(response).await
    }

    // ========================================================================
    // Subscriptions
    // ========================================================================

    pub async fn get_subscription(&self, subscription_id: &str) -> AppResult<StripeSubscription> {
        let response = self
            .client
            .get(format!(
                "{}/subscriptions/{}",
                STRIPE_API_BASE, subscription_id
            ))
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        self.handle_response(response).await
    }

    pub async fn cancel_subscription(
        &self,
        subscription_id: &str,
        at_period_end: bool,
    ) -> AppResult<StripeSubscription> {
        if at_period_end {
            // Update to cancel at period end
            let response = self
                .client
                .post(format!(
                    "{}/subscriptions/{}",
                    STRIPE_API_BASE, subscription_id
                ))
                .header("Authorization", self.auth_header())
                .form(&[("cancel_at_period_end", "true")])
                .send()
                .await
                .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

            self.handle_response(response).await
        } else {
            // Cancel immediately
            let response = self
                .client
                .delete(format!(
                    "{}/subscriptions/{}",
                    STRIPE_API_BASE, subscription_id
                ))
                .header("Authorization", self.auth_header())
                .send()
                .await
                .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

            self.handle_response(response).await
        }
    }

    /// Preview the cost of upgrading/changing a subscription using Stripe's upcoming invoice API
    pub async fn preview_subscription_change(
        &self,
        customer_id: &str,
        subscription_id: &str,
        subscription_item_id: &str,
        new_price_id: &str,
    ) -> AppResult<StripeUpcomingInvoice> {
        let params: Vec<(&str, String)> = vec![
            ("customer", customer_id.to_string()),
            ("subscription", subscription_id.to_string()),
            (
                "subscription_items[0][id]",
                subscription_item_id.to_string(),
            ),
            ("subscription_items[0][price]", new_price_id.to_string()),
            (
                "subscription_proration_behavior",
                "always_invoice".to_string(),
            ),
        ];

        let response = self
            .client
            .get(format!("{}/invoices/upcoming", STRIPE_API_BASE))
            .header("Authorization", self.auth_header())
            .query(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        self.handle_response(response).await
    }

    /// Upgrade subscription immediately with proration
    pub async fn upgrade_subscription(
        &self,
        subscription_id: &str,
        subscription_item_id: &str,
        new_price_id: &str,
        quantity: i64,
        end_trial: bool,
        idempotency_key: &str,
    ) -> AppResult<StripeSubscription> {
        let mut params: Vec<(String, String)> = vec![
            ("items[0][id]".to_string(), subscription_item_id.to_string()),
            ("items[0][price]".to_string(), new_price_id.to_string()),
            ("items[0][quantity]".to_string(), quantity.to_string()),
            (
                "proration_behavior".to_string(),
                "always_invoice".to_string(),
            ),
            (
                "payment_behavior".to_string(),
                "default_incomplete".to_string(),
            ),
            ("cancel_at_period_end".to_string(), "false".to_string()),
            // Expand latest_invoice and payment_intent to get client_secret for SCA
            (
                "expand[0]".to_string(),
                "latest_invoice.payment_intent".to_string(),
            ),
        ];

        if end_trial {
            params.push(("trial_end".to_string(), "now".to_string()));
        }

        let response = self
            .client
            .post(format!(
                "{}/subscriptions/{}",
                STRIPE_API_BASE, subscription_id
            ))
            .header("Authorization", self.auth_header())
            .header("Idempotency-Key", idempotency_key)
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        self.handle_response(response).await
    }

    /// Schedule a downgrade for the end of the current billing period using Subscription Schedules
    pub async fn schedule_downgrade(
        &self,
        subscription_id: &str,
        current_price_id: &str,
        new_price_id: &str,
        current_period_end: i64,
        idempotency_key: &str,
    ) -> AppResult<StripeSubscriptionSchedule> {
        // First, create a schedule from the existing subscription
        let create_response = self
            .client
            .post(format!("{}/subscription_schedules", STRIPE_API_BASE))
            .header("Authorization", self.auth_header())
            .header("Idempotency-Key", format!("{}-create", idempotency_key))
            .form(&[("from_subscription", subscription_id)])
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        let schedule: StripeSubscriptionSchedule = self.handle_response(create_response).await?;

        // Now update the schedule with the phase change
        let params: Vec<(String, String)> = vec![
            // End behavior: release means the subscription continues after the schedule ends
            ("end_behavior".to_string(), "release".to_string()),
            // Phase 1: current plan until period end (already handled by from_subscription)
            // Phase 2: new plan starting at period end
            (
                "phases[0][items][0][price]".to_string(),
                current_price_id.to_string(),
            ),
            (
                "phases[0][end_date]".to_string(),
                current_period_end.to_string(),
            ),
            (
                "phases[1][items][0][price]".to_string(),
                new_price_id.to_string(),
            ),
            ("phases[1][iterations]".to_string(), "1".to_string()),
        ];

        let update_response = self
            .client
            .post(format!(
                "{}/subscription_schedules/{}",
                STRIPE_API_BASE, schedule.id
            ))
            .header("Authorization", self.auth_header())
            .header("Idempotency-Key", format!("{}-update", idempotency_key))
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        // If update fails, cancel the orphaned schedule to clean up
        match self
            .handle_response::<StripeSubscriptionSchedule>(update_response)
            .await
        {
            Ok(updated_schedule) => Ok(updated_schedule),
            Err(e) => {
                // Best-effort cleanup: cancel the schedule we just created
                let _ = self.cancel_schedule(&schedule.id).await;
                Err(e)
            }
        }
    }

    /// Cancel a pending subscription schedule (releases the subscription back to normal)
    pub async fn cancel_schedule(
        &self,
        schedule_id: &str,
    ) -> AppResult<StripeSubscriptionSchedule> {
        let response = self
            .client
            .post(format!(
                "{}/subscription_schedules/{}/cancel",
                STRIPE_API_BASE, schedule_id
            ))
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        self.handle_response(response).await
    }

    /// Get a customer to check for default payment method
    pub async fn get_customer(&self, customer_id: &str) -> AppResult<StripeCustomerFull> {
        let response = self
            .client
            .get(format!("{}/customers/{}", STRIPE_API_BASE, customer_id))
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        self.handle_response(response).await
    }

    // ========================================================================
    // Refunds
    // ========================================================================

    pub async fn create_refund(
        &self,
        payment_intent_id: &str,
        amount: Option<i64>,
        reason: Option<&str>,
    ) -> AppResult<StripeRefund> {
        let mut params: Vec<(String, String)> =
            vec![("payment_intent".to_string(), payment_intent_id.to_string())];

        if let Some(amt) = amount {
            params.push(("amount".to_string(), amt.to_string()));
        }

        if let Some(r) = reason {
            params.push(("reason".to_string(), r.to_string()));
        }

        let response = self
            .client
            .post(format!("{}/refunds", STRIPE_API_BASE))
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        self.handle_response(response).await
    }

    // ========================================================================
    // Invoices
    // ========================================================================

    pub async fn list_invoices(
        &self,
        customer_id: &str,
        limit: Option<i32>,
    ) -> AppResult<Vec<StripeInvoice>> {
        let mut query = vec![("customer", customer_id.to_string())];
        if let Some(l) = limit {
            query.push(("limit", l.to_string()));
        }

        let response = self
            .client
            .get(format!("{}/invoices", STRIPE_API_BASE))
            .header("Authorization", self.auth_header())
            .query(&query)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

        let list: StripeInvoiceList = self.handle_response(response).await?;
        Ok(list.data)
    }

    // ========================================================================
    // Webhook Signature Verification
    // ========================================================================

    pub fn verify_webhook_signature(
        payload: &str,
        signature_header: &str,
        webhook_secret: &str,
    ) -> AppResult<()> {
        let now = chrono::Utc::now().timestamp();
        Self::verify_webhook_signature_at(payload, signature_header, webhook_secret, now)
    }

    fn verify_webhook_signature_at(
        payload: &str,
        signature_header: &str,
        webhook_secret: &str,
        now: i64,
    ) -> AppResult<()> {
        let payload = payload.as_bytes();
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        // Parse signature header: "t=timestamp,v1=signature,..."
        let mut timestamp: Option<&str> = None;
        let mut signatures: Vec<&str> = Vec::new();

        for part in signature_header.split(',') {
            let kv: Vec<&str> = part.splitn(2, '=').collect();
            if kv.len() != 2 {
                continue;
            }
            match kv[0] {
                "t" => timestamp = Some(kv[1]),
                "v1" => signatures.push(kv[1]),
                _ => {}
            }
        }

        let timestamp = timestamp
            .ok_or_else(|| AppError::InvalidInput("Missing timestamp in signature".into()))?;

        if signatures.is_empty() {
            return Err(AppError::InvalidInput("Missing signature".into()));
        }

        // Compute signed payload once (outside loop).
        let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));

        // Check if any v1 signature matches (constant-time via verify_slice).
        // Note: per-signature MAC recomputation is required because verify_slice consumes the MAC.
        for sig in &signatures {
            let sig_bytes = match hex::decode(sig) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };

            let mut mac = Hmac::<Sha256>::new_from_slice(webhook_secret.as_bytes())
                .map_err(|_| AppError::Internal("HMAC error".into()))?;
            mac.update(signed_payload.as_bytes());

            if mac.verify_slice(&sig_bytes).is_ok() {
                // Verify timestamp is not too old (5 minutes tolerance)
                let ts: i64 = timestamp
                    .parse()
                    .map_err(|_| AppError::InvalidInput("Invalid timestamp".into()))?;
                if (now - ts).abs() > 300 {
                    return Err(AppError::InvalidInput("Timestamp too old".into()));
                }
                return Ok(());
            }
        }

        Err(AppError::InvalidInput("Invalid signature".into()))
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    async fn handle_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> AppResult<T> {
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            tracing::error!(status = %status, body = %body, "Stripe API error");

            // Try to parse Stripe error
            if let Ok(error) = serde_json::from_str::<StripeErrorResponse>(&body) {
                return Err(AppError::InvalidInput(format!(
                    "Stripe error: {}",
                    error
                        .error
                        .message
                        .unwrap_or_else(|| error.error.error_type)
                )));
            }

            return Err(AppError::Internal(format!(
                "Stripe API error: {} - {}",
                status, body
            )));
        }

        serde_json::from_str(&body).map_err(|e| {
            tracing::error!(body = %body, error = %e, "Failed to parse Stripe response");
            AppError::Internal(format!("Failed to parse Stripe response: {}", e))
        })
    }
}

// ============================================================================
// Stripe Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct StripeProduct {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StripePrice {
    pub id: String,
    pub product: String,
    pub unit_amount: Option<i64>,
    pub currency: String,
    pub recurring: Option<StripePriceRecurring>,
}

#[derive(Debug, Deserialize)]
pub struct StripePriceRecurring {
    pub interval: String,
    pub interval_count: i32,
}

#[derive(Debug, Deserialize)]
pub struct StripeCustomer {
    pub id: String,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StripeCustomerList {
    pub data: Vec<StripeCustomer>,
}

#[derive(Debug, Deserialize)]
pub struct StripeCheckoutSession {
    pub id: String,
    pub url: Option<String>,
    pub customer: Option<String>,
    pub subscription: Option<String>,
    pub client_reference_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StripePortalSession {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct StripeSubscription {
    pub id: String,
    pub customer: String,
    pub status: String,
    pub current_period_start: i64,
    pub current_period_end: i64,
    pub cancel_at_period_end: bool,
    pub canceled_at: Option<i64>,
    pub trial_start: Option<i64>,
    pub trial_end: Option<i64>,
    pub items: StripeSubscriptionItems,
    pub latest_invoice: Option<StripeLatestInvoice>,
    pub schedule: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StripeLatestInvoice {
    Id(String),
    Expanded(Box<StripeLatestInvoiceExpanded>),
}

#[derive(Debug, Deserialize)]
pub struct StripeLatestInvoiceExpanded {
    pub id: String,
    pub status: Option<String>,
    pub amount_due: i64,
    pub amount_paid: i64,
    pub currency: String,
    pub hosted_invoice_url: Option<String>,
    pub payment_intent: Option<StripePaymentIntentRef>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StripePaymentIntentRef {
    Id(String),
    Expanded(Box<StripePaymentIntent>),
}

#[derive(Debug, Deserialize)]
pub struct StripePaymentIntent {
    pub id: String,
    pub status: String,
    pub client_secret: Option<String>,
    pub amount: i64,
    pub currency: String,
}

impl StripeSubscription {
    /// Get the first price ID from the subscription items
    pub fn price_id(&self) -> String {
        self.items
            .data
            .first()
            .map(|item| item.price.id.clone())
            .unwrap_or_default()
    }

    /// Get the first subscription item ID
    pub fn subscription_item_id(&self) -> Option<String> {
        self.items.data.first().map(|item| item.id.clone())
    }

    /// Get the quantity from the first subscription item
    pub fn quantity(&self) -> i64 {
        self.items
            .data
            .first()
            .and_then(|item| item.quantity)
            .unwrap_or(1)
    }

    /// Get the payment intent status from the latest invoice (for upgrades)
    pub fn payment_intent_status(&self) -> Option<String> {
        match &self.latest_invoice {
            Some(StripeLatestInvoice::Expanded(invoice)) => match &invoice.payment_intent {
                Some(StripePaymentIntentRef::Expanded(pi)) => Some(pi.status.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    /// Get the client secret for SCA confirmation
    pub fn client_secret(&self) -> Option<String> {
        match &self.latest_invoice {
            Some(StripeLatestInvoice::Expanded(invoice)) => match &invoice.payment_intent {
                Some(StripePaymentIntentRef::Expanded(pi)) => pi.client_secret.clone(),
                _ => None,
            },
            _ => None,
        }
    }

    /// Get the hosted invoice URL for fallback SCA flow
    pub fn hosted_invoice_url(&self) -> Option<String> {
        match &self.latest_invoice {
            Some(StripeLatestInvoice::Expanded(invoice)) => invoice.hosted_invoice_url.clone(),
            _ => None,
        }
    }

    /// Get the invoice ID from the latest invoice
    pub fn invoice_id(&self) -> Option<String> {
        match &self.latest_invoice {
            Some(StripeLatestInvoice::Id(id)) => Some(id.clone()),
            Some(StripeLatestInvoice::Expanded(invoice)) => Some(invoice.id.clone()),
            None => None,
        }
    }

    /// Get the amount charged from the latest invoice
    pub fn amount_charged(&self) -> Option<(i64, String)> {
        match &self.latest_invoice {
            Some(StripeLatestInvoice::Expanded(invoice)) => {
                Some((invoice.amount_due, invoice.currency.clone()))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct StripeSubscriptionItems {
    pub data: Vec<StripeSubscriptionItem>,
}

#[derive(Debug, Deserialize)]
pub struct StripeSubscriptionItem {
    pub id: String,
    pub price: StripePrice,
    pub quantity: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct StripeRefund {
    pub id: String,
    pub amount: i64,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct StripeInvoice {
    pub id: String,
    pub customer: String,
    pub amount_due: i64,
    pub amount_paid: i64,
    pub currency: String,
    pub status: Option<String>,
    pub hosted_invoice_url: Option<String>,
    pub invoice_pdf: Option<String>,
    pub number: Option<String>,
    pub billing_reason: Option<String>,
    pub created: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct StripeInvoiceList {
    pub data: Vec<StripeInvoice>,
}

#[derive(Debug, Deserialize)]
pub struct StripeErrorResponse {
    pub error: StripeError,
}

#[derive(Debug, Deserialize)]
pub struct StripeError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: Option<String>,
    pub code: Option<String>,
}

// ============================================================================
// Webhook Event Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct StripeWebhookEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: StripeWebhookEventData,
}

#[derive(Debug, Deserialize)]
pub struct StripeWebhookEventData {
    pub object: serde_json::Value,
}

impl StripeWebhookEvent {
    pub fn get_checkout_session(&self) -> Option<StripeCheckoutSession> {
        serde_json::from_value(self.data.object.clone()).ok()
    }

    pub fn get_subscription(&self) -> Option<StripeSubscription> {
        serde_json::from_value(self.data.object.clone()).ok()
    }

    pub fn get_invoice(&self) -> Option<StripeInvoice> {
        serde_json::from_value(self.data.object.clone()).ok()
    }
}

// ============================================================================
// Upcoming Invoice (for proration preview)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct StripeUpcomingInvoice {
    pub amount_due: i64,
    pub amount_remaining: i64,
    pub currency: String,
    pub customer: String,
    pub subscription: Option<String>,
    pub lines: StripeInvoiceLines,
    pub total: i64,
    pub subtotal: i64,
}

#[derive(Debug, Deserialize)]
pub struct StripeInvoiceLines {
    pub data: Vec<StripeInvoiceLine>,
}

#[derive(Debug, Deserialize)]
pub struct StripeInvoiceLine {
    pub id: String,
    pub amount: i64,
    pub description: Option<String>,
    pub proration: bool,
    pub price: Option<StripePrice>,
}

// ============================================================================
// Subscription Schedule (for downgrades)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct StripeSubscriptionSchedule {
    pub id: String,
    pub status: String,
    pub subscription: Option<String>,
    pub current_phase: Option<StripeSchedulePhase>,
    pub phases: Vec<StripeSchedulePhase>,
}

#[derive(Debug, Deserialize)]
pub struct StripeSchedulePhase {
    pub start_date: i64,
    pub end_date: Option<i64>,
    pub items: Vec<StripeSchedulePhaseItem>,
}

#[derive(Debug, Deserialize)]
pub struct StripeSchedulePhaseItem {
    pub price: String,
    pub quantity: Option<i64>,
}

// ============================================================================
// Customer (full details including payment method)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct StripeCustomerFull {
    pub id: String,
    pub email: Option<String>,
    pub invoice_settings: Option<StripeInvoiceSettings>,
    pub default_source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StripeInvoiceSettings {
    pub default_payment_method: Option<String>,
}

impl StripeCustomerFull {
    pub fn has_payment_method(&self) -> bool {
        self.default_source.is_some()
            || self
                .invoice_settings
                .as_ref()
                .and_then(|s| s.default_payment_method.as_ref())
                .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    const TEST_SECRET: &str = "whsec_test_secret_key";
    const NOW_TS: i64 = 1_700_000_000;
    const TOLERANCE_SECS: i64 = 300;

    fn compute_signature(payload: &str, timestamp: i64, secret: &str) -> String {
        let signed_payload = format!("{}.{}", timestamp, payload);
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(signed_payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    fn verify(payload: &str, header: &str) -> AppResult<()> {
        StripeClient::verify_webhook_signature_at(payload, header, TEST_SECRET, NOW_TS)
    }

    // -------------------------------------------------------------------------
    // Basic signature verification
    // -------------------------------------------------------------------------

    #[test]
    fn test_valid_signature() {
        let payload = r#"{"type":"checkout.session.completed"}"#;
        let ts = NOW_TS;
        let sig = compute_signature(payload, ts, TEST_SECRET);
        let header = format!("t={},v1={}", ts, sig);

        let result = verify(payload, &header);
        assert!(result.is_ok(), "Valid signature should pass");
    }

    #[test]
    fn test_invalid_signature() {
        let payload = r#"{"type":"checkout.session.completed"}"#;
        let ts = NOW_TS;
        let header = format!(
            "t={},v1=0000000000000000000000000000000000000000000000000000000000000000",
            ts
        );

        let result = verify(payload, &header);
        assert!(result.is_err(), "Invalid signature should fail");
    }

    // -------------------------------------------------------------------------
    // Timestamp validation (deterministic)
    // -------------------------------------------------------------------------

    #[test]
    fn test_expired_timestamp() {
        let payload = r#"{"type":"checkout.session.completed"}"#;
        let ts = NOW_TS - TOLERANCE_SECS - 100;
        let sig = compute_signature(payload, ts, TEST_SECRET);
        let header = format!("t={},v1={}", ts, sig);

        let result = verify(payload, &header);
        assert!(result.is_err(), "Expired timestamp should fail");
    }

    #[test]
    fn test_future_timestamp_within_tolerance() {
        let payload = r#"{"type":"test"}"#;
        let ts = NOW_TS + 60;
        let sig = compute_signature(payload, ts, TEST_SECRET);
        let header = format!("t={},v1={}", ts, sig);

        let result = verify(payload, &header);
        assert!(
            result.is_ok(),
            "Future timestamp within tolerance should pass"
        );
    }

    #[test]
    fn test_future_timestamp_beyond_tolerance() {
        let payload = r#"{"type":"test"}"#;
        let ts = NOW_TS + TOLERANCE_SECS + 100;
        let sig = compute_signature(payload, ts, TEST_SECRET);
        let header = format!("t={},v1={}", ts, sig);

        let result = verify(payload, &header);
        assert!(
            result.is_err(),
            "Future timestamp beyond tolerance should fail"
        );
    }

    // -------------------------------------------------------------------------
    // Header parsing edge cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_missing_timestamp() {
        let result = verify(
            "payload",
            "v1=abc123def456abc123def456abc123def456abc123def456abc123def456abcd",
        );
        assert!(result.is_err(), "Missing timestamp should fail");
    }

    #[test]
    fn test_missing_signature() {
        let ts = NOW_TS;
        let result = verify("payload", &format!("t={}", ts));
        assert!(result.is_err(), "Missing signature should fail");
    }

    #[test]
    fn test_multiple_v1_signatures_second_valid() {
        let payload = r#"{"type":"test"}"#;
        let ts = NOW_TS;
        let valid_sig = compute_signature(payload, ts, TEST_SECRET);
        let invalid_sig = "0000000000000000000000000000000000000000000000000000000000000000";

        let header = format!("t={},v1={},v1={}", ts, invalid_sig, valid_sig);

        let result = verify(payload, &header);
        assert!(result.is_ok(), "Second valid signature should pass");
    }

    #[test]
    fn test_non_v1_signatures_ignored() {
        let payload = r#"{"type":"test"}"#;
        let ts = NOW_TS;
        let valid_sig = compute_signature(payload, ts, TEST_SECRET);

        let header = format!("t={},v0=ignored,v1={}", ts, valid_sig);

        let result = verify(payload, &header);
        assert!(result.is_ok(), "Non-v1 signatures should be ignored");
    }

    // -------------------------------------------------------------------------
    // Malformed signature handling
    // -------------------------------------------------------------------------

    #[test]
    fn test_malformed_hex_non_hex_chars() {
        let payload = r#"{"type":"test"}"#;
        let ts = NOW_TS;
        let header = format!(
            "t={},v1=ZZZZ0000000000000000000000000000000000000000000000000000000000",
            ts
        );

        let result = verify(payload, &header);
        assert!(result.is_err(), "Malformed hex should fail");
    }

    #[test]
    fn test_malformed_hex_odd_length() {
        let payload = r#"{"type":"test"}"#;
        let ts = NOW_TS;
        let header = format!("t={},v1=abc", ts);

        let result = verify(payload, &header);
        assert!(result.is_err(), "Odd-length hex should fail");
    }

    #[test]
    fn test_empty_signature_value() {
        let payload = r#"{"type":"test"}"#;
        let ts = NOW_TS;
        let header = format!("t={},v1=", ts);

        let result = verify(payload, &header);
        assert!(result.is_err(), "Empty signature should fail");
    }

    #[test]
    fn test_wrong_length_signature() {
        let payload = r#"{"type":"test"}"#;
        let ts = NOW_TS;
        let header = format!("t={},v1=abcd1234", ts);

        let result = verify(payload, &header);
        assert!(result.is_err(), "Wrong-length signature should fail");
    }

    // -------------------------------------------------------------------------
    // Header whitespace handling
    // -------------------------------------------------------------------------

    #[test]
    fn test_header_with_spaces_around_comma() {
        let payload = r#"{"type":"test"}"#;
        let ts = NOW_TS;
        let sig = compute_signature(payload, ts, TEST_SECRET);
        let header = format!("t={}, v1={}", ts, sig);

        let result = verify(payload, &header);
        assert!(
            result.is_err(),
            "Header with spaces should fail (documents current behavior)"
        );
    }

    #[test]
    fn test_header_standard_stripe_format() {
        let payload = r#"{"type":"test"}"#;
        let ts = NOW_TS;
        let sig = compute_signature(payload, ts, TEST_SECRET);
        let header = format!("t={},v1={}", ts, sig);

        let result = verify(payload, &header);
        assert!(result.is_ok(), "Standard Stripe format should pass");
    }
}
