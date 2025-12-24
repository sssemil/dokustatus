use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DomainAuthMagicLink {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub resend_api_key_encrypted: String,
    pub from_email: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}
