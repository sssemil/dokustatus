use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DomainAuthConfig {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub magic_link_enabled: bool,
    pub google_oauth_enabled: bool,
    pub redirect_url: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}
