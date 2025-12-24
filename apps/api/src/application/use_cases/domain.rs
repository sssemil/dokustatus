use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDateTime;
use tracing::instrument;
use uuid::Uuid;

use crate::app_error::{AppError, AppResult};
use crate::domain::entities::domain::DomainStatus;

#[async_trait]
pub trait DomainRepo: Send + Sync {
    async fn create(&self, owner_end_user_id: Uuid, domain: &str) -> AppResult<DomainProfile>;
    async fn get_by_id(&self, domain_id: Uuid) -> AppResult<Option<DomainProfile>>;
    async fn get_by_domain(&self, domain: &str) -> AppResult<Option<DomainProfile>>;
    async fn list_by_owner(&self, owner_end_user_id: Uuid) -> AppResult<Vec<DomainProfile>>;
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
    pub async fn add_domain(&self, owner_end_user_id: Uuid, domain: &str) -> AppResult<DomainProfile> {
        let normalized = domain.to_lowercase().trim().to_string();

        // Validate that this is a root domain (not a subdomain)
        if !is_root_domain(&normalized) {
            return Err(AppError::InvalidInput(
                "Please enter your root domain (e.g., example.com), not a subdomain".into()
            ));
        }

        self.repo.create(owner_end_user_id, &normalized).await
    }

    #[instrument(skip(self))]
    pub async fn list_domains(&self, owner_end_user_id: Uuid) -> AppResult<Vec<DomainProfile>> {
        self.repo.list_by_owner(owner_end_user_id).await
    }

    #[instrument(skip(self))]
    pub async fn get_domain(&self, owner_end_user_id: Uuid, domain_id: Uuid) -> AppResult<DomainProfile> {
        let domain = self
            .repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if domain.owner_end_user_id != Some(owner_end_user_id) {
            return Err(AppError::InvalidCredentials);
        }
        Ok(domain)
    }

    #[instrument(skip(self))]
    pub async fn start_verification(&self, owner_end_user_id: Uuid, domain_id: Uuid) -> AppResult<DomainProfile> {
        let domain = self.get_domain(owner_end_user_id, domain_id).await?;
        self.repo.set_verifying(domain.id).await
    }

    #[instrument(skip(self))]
    pub async fn check_domain_dns(&self, domain_id: Uuid) -> AppResult<bool> {
        let domain = self
            .repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;

        // Check CNAME for reauth.{domain} (not the root domain itself)
        let cname_record = format!("reauth.{}", domain.domain);
        let cname_ok = self
            .dns_verifier
            .check_cname(&cname_record, &self.ingress_domain)
            .await
            .unwrap_or(false);

        // Check TXT for _reauth.{domain}
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
    pub async fn delete_domain(&self, owner_end_user_id: Uuid, domain_id: Uuid) -> AppResult<()> {
        let domain = self.get_domain(owner_end_user_id, domain_id).await?;

        // Prevent deletion of system domains (those without an owner)
        if domain.owner_end_user_id.is_none() {
            return Err(AppError::InvalidInput("System domains cannot be deleted".into()));
        }

        self.repo.delete(domain.id).await
    }

    pub fn get_dns_records(&self, domain: &str, domain_id: Uuid) -> DnsRecords {
        DnsRecords {
            cname_name: format!("reauth.{}", domain),
            cname_value: self.ingress_domain.clone(),
            txt_name: format!("_reauth.{}", domain),
            txt_value: format!("project={}", domain_id),
        }
    }

    pub async fn get_verifying_domains(&self) -> AppResult<Vec<DomainProfile>> {
        self.repo.get_verifying_domains().await
    }

    /// Check if a domain is allowed for SSL provisioning (used by Caddy on_demand_tls)
    /// Caddy will ask about "reauth.example.com", we need to look up "example.com"
    #[instrument(skip(self))]
    pub async fn is_domain_allowed(&self, hostname: &str) -> AppResult<bool> {
        // Extract root domain from reauth.* hostname
        let root_domain = extract_root_from_reauth_hostname(hostname);
        let domain = self.repo.get_by_domain(&root_domain).await?;
        match domain {
            Some(d) if d.status == DomainStatus::Verified => Ok(true),
            _ => Ok(false),
        }
    }
}

/// Check if a domain is a root domain (not a subdomain)
/// Handles multi-part TLDs like .co.uk, .com.au, etc.
fn is_root_domain(domain: &str) -> bool {
    let parts: Vec<&str> = domain.split('.').collect();

    // Must have at least 2 parts (name + TLD)
    if parts.len() < 2 {
        return false;
    }

    // Check for multi-part TLDs
    let multi_part_tlds = ["co.uk", "com.au", "co.nz", "com.br", "co.jp", "org.uk", "net.au"];
    let domain_lower = domain.to_lowercase();

    for tld in &multi_part_tlds {
        if domain_lower.ends_with(tld) {
            // For multi-part TLDs, root domain has exactly 3 parts
            return parts.len() == 3;
        }
    }

    // For standard TLDs, root domain has exactly 2 parts
    parts.len() == 2
}

/// Extract root domain from a reauth.* hostname
/// e.g., "reauth.example.com" -> "example.com"
/// Special case: "reauth.dev" stays as "reauth.dev" (it's the actual domain)
pub fn extract_root_from_reauth_hostname(hostname: &str) -> String {
    if hostname.starts_with("reauth.") {
        let remainder = hostname.strip_prefix("reauth.").unwrap_or(hostname);
        // Only strip if remainder is a valid domain (contains at least one dot)
        // This prevents "reauth.dev" from becoming "dev"
        if remainder.contains('.') {
            remainder.to_string()
        } else {
            hostname.to_string()
        }
    } else {
        hostname.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct DomainProfile {
    pub id: Uuid,
    pub owner_end_user_id: Option<Uuid>, // NULL for system domains like reauth.dev
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
