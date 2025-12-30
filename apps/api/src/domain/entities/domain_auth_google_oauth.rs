use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DomainAuthGoogleOAuth {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub client_id: String,
    pub client_secret_encrypted: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}
