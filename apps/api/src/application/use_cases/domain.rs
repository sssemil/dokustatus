use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDateTime;
use tracing::instrument;
use uuid::Uuid;

use crate::app_error::{AppError, AppResult};
use crate::domain::entities::domain::DomainStatus;

#[async_trait]
pub trait DomainRepo: Send + Sync {
    async fn create(&self, user_id: Uuid, domain: &str) -> AppResult<DomainProfile>;
    async fn get_by_id(&self, domain_id: Uuid) -> AppResult<Option<DomainProfile>>;
    async fn list_by_user(&self, user_id: Uuid) -> AppResult<Vec<DomainProfile>>;
    async fn update_status(&self, domain_id: Uuid, status: &str) -> AppResult<DomainProfile>;
    async fn set_verifying(&self, domain_id: Uuid) -> AppResult<DomainProfile>;
    async fn set_verified(&self, domain_id: Uuid) -> AppResult<DomainProfile>;
    async fn set_failed(&self, domain_id: Uuid) -> AppResult<DomainProfile>;
    async fn delete(&self, domain_id: Uuid) -> AppResult<()>;
    async fn get_verifying_domains(&self) -> AppResult<Vec<DomainProfile>>;
}

#[async_trait]
pub trait DnsVerifier: Send + Sync {
    async fn check_cname(&self, domain: &str, expected_target: &str) -> AppResult<bool>;
    async fn check_txt(&self, domain: &str, expected_value: &str) -> AppResult<bool>;
}

#[derive(Clone)]
pub struct DomainUseCases {
    repo: Arc<dyn DomainRepo>,
    dns_verifier: Arc<dyn DnsVerifier>,
    ingress_domain: String,
}

impl DomainUseCases {
    pub fn new(
        repo: Arc<dyn DomainRepo>,
        dns_verifier: Arc<dyn DnsVerifier>,
        ingress_domain: String,
    ) -> Self {
        Self {
            repo,
            dns_verifier,
            ingress_domain,
        }
    }

    #[instrument(skip(self))]
    pub async fn add_domain(&self, user_id: Uuid, domain: &str) -> AppResult<DomainProfile> {
        let normalized = domain.to_lowercase().trim().to_string();
        self.repo.create(user_id, &normalized).await
    }

    #[instrument(skip(self))]
    pub async fn list_domains(&self, user_id: Uuid) -> AppResult<Vec<DomainProfile>> {
        self.repo.list_by_user(user_id).await
    }

    #[instrument(skip(self))]
    pub async fn get_domain(&self, user_id: Uuid, domain_id: Uuid) -> AppResult<DomainProfile> {
        let domain = self
            .repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if domain.user_id != user_id {
            return Err(AppError::InvalidCredentials);
        }
        Ok(domain)
    }

    #[instrument(skip(self))]
    pub async fn start_verification(&self, user_id: Uuid, domain_id: Uuid) -> AppResult<DomainProfile> {
        let domain = self.get_domain(user_id, domain_id).await?;
        self.repo.set_verifying(domain.id).await
    }

    #[instrument(skip(self))]
    pub async fn check_domain_dns(&self, domain_id: Uuid) -> AppResult<bool> {
        let domain = self
            .repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;

        let cname_ok = self
            .dns_verifier
            .check_cname(&domain.domain, &self.ingress_domain)
            .await
            .unwrap_or(false);

        let txt_record = format!("_reauth.{}", domain.domain);
        let expected_txt = format!("project={}", domain.id);
        let txt_ok = self
            .dns_verifier
            .check_txt(&txt_record, &expected_txt)
            .await
            .unwrap_or(false);

        if cname_ok && txt_ok {
            self.repo.set_verified(domain_id).await?;
            return Ok(true);
        }

        Ok(false)
    }

    #[instrument(skip(self))]
    pub async fn mark_failed(&self, domain_id: Uuid) -> AppResult<DomainProfile> {
        self.repo.set_failed(domain_id).await
    }

    #[instrument(skip(self))]
    pub async fn delete_domain(&self, user_id: Uuid, domain_id: Uuid) -> AppResult<()> {
        let domain = self.get_domain(user_id, domain_id).await?;
        self.repo.delete(domain.id).await
    }

    pub fn get_dns_records(&self, domain: &str, domain_id: Uuid) -> DnsRecords {
        DnsRecords {
            cname_name: domain.to_string(),
            cname_value: self.ingress_domain.clone(),
            txt_name: format!("_reauth.{}", domain),
            txt_value: format!("project={}", domain_id),
        }
    }

    pub async fn get_verifying_domains(&self) -> AppResult<Vec<DomainProfile>> {
        self.repo.get_verifying_domains().await
    }
}

#[derive(Debug, Clone)]
pub struct DomainProfile {
    pub id: Uuid,
    pub user_id: Uuid,
    pub domain: String,
    pub status: DomainStatus,
    pub verification_started_at: Option<NaiveDateTime>,
    pub verified_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct DnsRecords {
    pub cname_name: String,
    pub cname_value: String,
    pub txt_name: String,
    pub txt_value: String,
}
