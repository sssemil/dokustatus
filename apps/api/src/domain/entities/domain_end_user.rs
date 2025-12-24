use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DomainEndUser {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub email: String,
    pub email_verified_at: Option<chrono::NaiveDateTime>,
    pub last_login_at: Option<chrono::NaiveDateTime>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}
