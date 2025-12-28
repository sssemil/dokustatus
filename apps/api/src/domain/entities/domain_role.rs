use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DomainRole {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub name: String,
    pub created_at: Option<chrono::NaiveDateTime>,
}
