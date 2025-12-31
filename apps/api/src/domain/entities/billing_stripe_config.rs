use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BillingStripeConfig {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub stripe_secret_key_encrypted: String,
    pub stripe_publishable_key: String,
    pub stripe_webhook_secret_encrypted: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}
