use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainStatus {
    PendingDns,
    Verifying,
    Verified,
    Failed,
}

impl DomainStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DomainStatus::PendingDns => "pending_dns",
            DomainStatus::Verifying => "verifying",
            DomainStatus::Verified => "verified",
            DomainStatus::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "pending_dns" => DomainStatus::PendingDns,
            "verifying" => DomainStatus::Verifying,
            "verified" => DomainStatus::Verified,
            "failed" => DomainStatus::Failed,
            _ => DomainStatus::PendingDns,
        }
    }
}

#[derive(Debug)]
pub struct Domain {
    pub id: Uuid,
    pub user_id: Uuid,
    pub domain: String,
    pub status: DomainStatus,
    pub verification_started_at: Option<chrono::NaiveDateTime>,
    pub verified_at: Option<chrono::NaiveDateTime>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}
