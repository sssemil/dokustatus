//! In-memory mock implementations for domain-related repository traits.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::{
    app_error::{AppError, AppResult},
    application::use_cases::domain::{DomainProfile, DomainRepoTrait},
    domain::entities::{domain::DomainStatus, payment_mode::PaymentMode},
};

/// In-memory implementation of DomainRepoTrait for testing.
#[derive(Default)]
pub struct InMemoryDomainRepo {
    pub domains: Mutex<HashMap<Uuid, DomainProfile>>,
}

impl InMemoryDomainRepo {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed the repo with initial domains for testing.
    pub fn with_domains(domains: Vec<DomainProfile>) -> Self {
        let map: HashMap<Uuid, DomainProfile> = domains.into_iter().map(|d| (d.id, d)).collect();
        Self {
            domains: Mutex::new(map),
        }
    }

    /// Get all domains (for test assertions).
    pub fn get_all(&self) -> Vec<DomainProfile> {
        self.domains.lock().unwrap().values().cloned().collect()
    }
}

#[async_trait]
impl DomainRepoTrait for InMemoryDomainRepo {
    async fn create(&self, owner_end_user_id: Uuid, domain: &str) -> AppResult<DomainProfile> {
        let mut domains = self.domains.lock().unwrap();

        // Check for duplicate domain
        if domains.values().any(|d| d.domain == domain) {
            return Err(AppError::InvalidInput("Domain already exists".into()));
        }

        let now = chrono::Utc::now().naive_utc();
        let profile = DomainProfile {
            id: Uuid::new_v4(),
            owner_end_user_id: Some(owner_end_user_id),
            domain: domain.to_string(),
            status: DomainStatus::PendingDns,
            active_payment_mode: PaymentMode::Test,
            verification_started_at: None,
            verified_at: None,
            created_at: Some(now),
            updated_at: Some(now),
        };

        domains.insert(profile.id, profile.clone());
        Ok(profile)
    }

    async fn get_by_id(&self, domain_id: Uuid) -> AppResult<Option<DomainProfile>> {
        Ok(self.domains.lock().unwrap().get(&domain_id).cloned())
    }

    async fn get_by_domain(&self, domain: &str) -> AppResult<Option<DomainProfile>> {
        Ok(self
            .domains
            .lock()
            .unwrap()
            .values()
            .find(|d| d.domain == domain)
            .cloned())
    }

    async fn list_by_owner(&self, owner_end_user_id: Uuid) -> AppResult<Vec<DomainProfile>> {
        Ok(self
            .domains
            .lock()
            .unwrap()
            .values()
            .filter(|d| d.owner_end_user_id == Some(owner_end_user_id))
            .cloned()
            .collect())
    }

    async fn update_status(&self, domain_id: Uuid, status: &str) -> AppResult<DomainProfile> {
        let mut domains = self.domains.lock().unwrap();
        let domain = domains.get_mut(&domain_id).ok_or(AppError::NotFound)?;

        domain.status = DomainStatus::from_str(status);
        domain.updated_at = Some(chrono::Utc::now().naive_utc());

        Ok(domain.clone())
    }

    async fn set_verifying(&self, domain_id: Uuid) -> AppResult<DomainProfile> {
        let mut domains = self.domains.lock().unwrap();
        let domain = domains.get_mut(&domain_id).ok_or(AppError::NotFound)?;

        domain.status = DomainStatus::Verifying;
        domain.verification_started_at = Some(chrono::Utc::now().naive_utc());
        domain.updated_at = Some(chrono::Utc::now().naive_utc());

        Ok(domain.clone())
    }

    async fn set_verified(&self, domain_id: Uuid) -> AppResult<DomainProfile> {
        let mut domains = self.domains.lock().unwrap();
        let domain = domains.get_mut(&domain_id).ok_or(AppError::NotFound)?;

        domain.status = DomainStatus::Verified;
        domain.verified_at = Some(chrono::Utc::now().naive_utc());
        domain.updated_at = Some(chrono::Utc::now().naive_utc());

        Ok(domain.clone())
    }

    async fn set_failed(&self, domain_id: Uuid) -> AppResult<DomainProfile> {
        let mut domains = self.domains.lock().unwrap();
        let domain = domains.get_mut(&domain_id).ok_or(AppError::NotFound)?;

        domain.status = DomainStatus::Failed;
        domain.updated_at = Some(chrono::Utc::now().naive_utc());

        Ok(domain.clone())
    }

    async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
        let mut domains = self.domains.lock().unwrap();
        domains.remove(&domain_id).ok_or(AppError::NotFound)?;
        Ok(())
    }

    async fn get_verifying_domains(&self) -> AppResult<Vec<DomainProfile>> {
        Ok(self
            .domains
            .lock()
            .unwrap()
            .values()
            .filter(|d| d.status == DomainStatus::Verifying)
            .cloned()
            .collect())
    }

    async fn set_active_payment_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
    ) -> AppResult<DomainProfile> {
        let mut domains = self.domains.lock().unwrap();
        let domain = domains.get_mut(&domain_id).ok_or(AppError::NotFound)?;

        domain.active_payment_mode = mode;
        domain.updated_at = Some(chrono::Utc::now().naive_utc());

        Ok(domain.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_domain;

    #[tokio::test]
    async fn test_create_domain() {
        let repo = InMemoryDomainRepo::new();
        let owner_id = Uuid::new_v4();

        let domain = repo.create(owner_id, "test.com").await.unwrap();

        assert_eq!(domain.domain, "test.com");
        assert_eq!(domain.owner_end_user_id, Some(owner_id));
        assert_eq!(domain.status, DomainStatus::PendingDns);
    }

    #[tokio::test]
    async fn test_duplicate_domain_fails() {
        let repo = InMemoryDomainRepo::new();
        let owner_id = Uuid::new_v4();

        repo.create(owner_id, "test.com").await.unwrap();
        let result = repo.create(owner_id, "test.com").await;

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let domain = create_test_domain(|d| {
            d.domain = "test.com".to_string();
        });
        let domain_id = domain.id;

        let repo = InMemoryDomainRepo::with_domains(vec![domain]);

        let found = repo.get_by_id(domain_id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().domain, "test.com");
    }

    #[tokio::test]
    async fn test_set_verified() {
        let domain = create_test_domain(|d| {
            d.status = DomainStatus::Verifying;
        });
        let domain_id = domain.id;

        let repo = InMemoryDomainRepo::with_domains(vec![domain]);

        let updated = repo.set_verified(domain_id).await.unwrap();
        assert_eq!(updated.status, DomainStatus::Verified);
        assert!(updated.verified_at.is_some());
    }
}
