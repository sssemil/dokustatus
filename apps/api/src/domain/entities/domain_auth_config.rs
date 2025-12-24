use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DomainAuthConfig {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub magic_link_enabled: bool,
    pub google_oauth_enabled: bool,
    pub redirect_url: Option<String>,
    pub access_token_ttl_secs: i32,
    pub refresh_token_ttl_days: i32,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}
