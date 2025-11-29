use uuid::Uuid;

#[derive(Debug)]
pub struct User {
    pub id: Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub email: String,
}
