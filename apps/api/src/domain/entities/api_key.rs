use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ApiKey {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub key_prefix: String,
    pub key_hash: String,
    pub name: String,
    pub last_used_at: Option<chrono::NaiveDateTime>,
    pub revoked_at: Option<chrono::NaiveDateTime>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub created_by_end_user_id: Uuid,
}
