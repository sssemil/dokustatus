use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

use crate::app_error::{AppError, AppResult};

const STRIPE_API_BASE: &str = "https://api.stripe.com/v1";

#[derive(Clone)]
pub struct StripeClient {
    client: Client,
    secret_key: String,
}

impl StripeClient {
    pub fn new(secret_key: String) -> Self {
        Self {
            client: Client::new(),
            secret_key,
        }
    }

    fn auth_header(&self) -> String {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{}:", self.secret_key));
        format!("Basic {}", encoded)
    }

    // ========================================================================
    // Products
    // ========================================================================

    pub async fn create_product(&self, name: &str, description: Option<&str>) -> AppResult<StripeProduct> {
        let mut params = HashMap::new();
        params.insert("name", name.to_string());
        if let Some(desc) = description {
            params.insert("description", desc.to_string());
        }

        let response = self.client
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

        let response = self.client
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
        let mut params: Vec<(String, String)> = vec![
            ("email".to_string(), email.to_string()),
        ];

        if let Some(meta) = metadata {
            for (key, value) in meta {
                params.push((format!("metadata[{}]", key), value));
            }
        }

        let response = self.client
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
        let response = self.client
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
                params.push(("subscription_data[trial_period_days]".to_string(), days.to_string()));
            }
        }

        let response = self.client
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
        let params = vec![
            ("customer", customer_id),
            ("return_url", return_url),
        ];

        let response = self.client
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
        let response = self.client
            .get(format!("{}/subscriptions/{}", STRIPE_API_BASE, subscription_id))
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
            let response = self.client
                .post(format!("{}/subscriptions/{}", STRIPE_API_BASE, subscription_id))
                .header("Authorization", self.auth_header())
                .form(&[("cancel_at_period_end", "true")])
                .send()
                .await
                .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

            self.handle_response(response).await
        } else {
            // Cancel immediately
            let response = self.client
                .delete(format!("{}/subscriptions/{}", STRIPE_API_BASE, subscription_id))
                .header("Authorization", self.auth_header())
                .send()
                .await
                .map_err(|e| AppError::Internal(format!("Stripe request failed: {}", e)))?;

            self.handle_response(response).await
        }
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
        let mut params: Vec<(String, String)> = vec![
            ("payment_intent".to_string(), payment_intent_id.to_string()),
        ];

        if let Some(amt) = amount {
            params.push(("amount".to_string(), amt.to_string()));
        }

        if let Some(r) = reason {
            params.push(("reason".to_string(), r.to_string()));
        }

        let response = self.client
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

        let response = self.client
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

        let timestamp = timestamp.ok_or_else(|| {
            AppError::InvalidInput("Missing timestamp in signature".into())
        })?;

        if signatures.is_empty() {
            return Err(AppError::InvalidInput("Missing signature".into()));
        }

        // Compute expected signature
        let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));
        let mut mac = Hmac::<Sha256>::new_from_slice(webhook_secret.as_bytes())
            .map_err(|_| AppError::Internal("HMAC error".into()))?;
        mac.update(signed_payload.as_bytes());
        let expected = hex::encode(mac.finalize().into_bytes());

        // Check if any signature matches
        for sig in signatures {
            if constant_time_compare(sig, &expected) {
                // Verify timestamp is not too old (5 minutes tolerance)
                let ts: i64 = timestamp.parse().map_err(|_| {
                    AppError::InvalidInput("Invalid timestamp".into())
                })?;
                let now = chrono::Utc::now().timestamp();
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
        let body = response.text().await.map_err(|e| {
            AppError::Internal(format!("Failed to read response: {}", e))
        })?;

        if !status.is_success() {
            tracing::error!(status = %status, body = %body, "Stripe API error");

            // Try to parse Stripe error
            if let Ok(error) = serde_json::from_str::<StripeErrorResponse>(&body) {
                return Err(AppError::InvalidInput(format!(
                    "Stripe error: {}",
                    error.error.message.unwrap_or_else(|| error.error.error_type)
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

fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
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
}

impl StripeSubscription {
    /// Get the first price ID from the subscription items
    pub fn price_id(&self) -> String {
        self.items.data
            .first()
            .map(|item| item.price.id.clone())
            .unwrap_or_default()
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
    pub status: Option<String>,
    pub hosted_invoice_url: Option<String>,
    pub invoice_pdf: Option<String>,
    pub created: i64,
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
