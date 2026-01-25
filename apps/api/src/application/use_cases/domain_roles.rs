use std::sync::Arc;

use tracing::instrument;
use uuid::Uuid;

use crate::adapters::persistence::domain_role::{DomainRoleRepoTrait, DomainRoleWithCount};
use crate::app_error::{AppError, AppResult};
use crate::application::use_cases::domain::DomainRepoTrait;
use crate::application::use_cases::domain_auth::DomainEndUserRepoTrait;
use crate::domain::entities::domain_role::DomainRole;

// ============================================================================
// Use Cases
// ============================================================================

#[derive(Clone)]
pub struct DomainRolesUseCases {
    domain_repo: Arc<dyn DomainRepoTrait>,
    role_repo: Arc<dyn DomainRoleRepoTrait>,
    end_user_repo: Arc<dyn DomainEndUserRepoTrait>,
}

impl DomainRolesUseCases {
    pub fn new(
        domain_repo: Arc<dyn DomainRepoTrait>,
        role_repo: Arc<dyn DomainRoleRepoTrait>,
        end_user_repo: Arc<dyn DomainEndUserRepoTrait>,
    ) -> Self {
        Self {
            domain_repo,
            role_repo,
            end_user_repo,
        }
    }

    // ========================================================================
    // Role CRUD Operations
    // ========================================================================

    /// Create a new role for a domain
    #[instrument(skip(self))]
    pub async fn create_role(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        name: &str,
    ) -> AppResult<DomainRole> {
        self.verify_domain_ownership(owner_end_user_id, domain_id)
            .await?;

        // Validate role name
        let name = name.trim().to_lowercase();
        if !is_valid_role_name(&name) {
            return Err(AppError::InvalidInput(
                "Role name must be lowercase alphanumeric with hyphens, start with a letter, max 50 chars".into()
            ));
        }

        // Check if role already exists
        if self.role_repo.exists(domain_id, &name).await? {
            return Err(AppError::InvalidInput("Role already exists".into()));
        }

        self.role_repo.create(domain_id, &name).await
    }

    /// List all roles for a domain with user counts
    #[instrument(skip(self))]
    pub async fn list_roles(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<Vec<DomainRoleWithCount>> {
        self.verify_domain_ownership(owner_end_user_id, domain_id)
            .await?;
        self.role_repo.list_by_domain_with_counts(domain_id).await
    }

    /// Delete a role and remove it from all users
    #[instrument(skip(self))]
    pub async fn delete_role(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        role_name: &str,
    ) -> AppResult<()> {
        self.verify_domain_ownership(owner_end_user_id, domain_id)
            .await?;

        // Check if role exists
        if !self.role_repo.exists(domain_id, role_name).await? {
            return Err(AppError::NotFound);
        }

        // Remove role from all users first
        self.end_user_repo
            .remove_role_from_all_users(domain_id, role_name)
            .await?;

        // Delete the role
        self.role_repo.delete(domain_id, role_name).await
    }

    /// Get the count of users with a specific role (for confirmation dialogs)
    #[instrument(skip(self))]
    pub async fn count_users_with_role(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        role_name: &str,
    ) -> AppResult<i64> {
        self.verify_domain_ownership(owner_end_user_id, domain_id)
            .await?;
        self.end_user_repo
            .count_users_with_role(domain_id, role_name)
            .await
    }

    // ========================================================================
    // User Role Assignment
    // ========================================================================

    /// Set roles for a user (replaces all existing roles)
    #[instrument(skip(self))]
    pub async fn set_user_roles(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        user_id: Uuid,
        roles: Vec<String>,
    ) -> AppResult<()> {
        self.verify_domain_ownership(owner_end_user_id, domain_id)
            .await?;

        // Verify user belongs to this domain
        let user = self
            .end_user_repo
            .get_by_id(user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if user.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        // Validate all roles exist in the domain
        for role_name in &roles {
            if !self.role_repo.exists(domain_id, role_name).await? {
                return Err(AppError::InvalidInput(format!(
                    "Role '{}' does not exist",
                    role_name
                )));
            }
        }

        self.end_user_repo.set_roles(user_id, &roles).await
    }

    /// Get list of roles available for a domain (for role selection UI)
    #[instrument(skip(self))]
    pub async fn list_available_roles(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<Vec<String>> {
        self.verify_domain_ownership(owner_end_user_id, domain_id)
            .await?;
        let roles = self.role_repo.list_by_domain(domain_id).await?;
        Ok(roles.into_iter().map(|r| r.name).collect())
    }

    // ========================================================================
    // Private Helpers
    // ========================================================================

    async fn verify_domain_ownership(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<()> {
        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if domain.owner_end_user_id != Some(owner_end_user_id) {
            return Err(AppError::InvalidCredentials);
        }

        Ok(())
    }
}

// ============================================================================
// Validation
// ============================================================================

/// Validate role name: lowercase alphanumeric with hyphens, start with letter, max 50 chars
fn is_valid_role_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 50 {
        return false;
    }

    let mut chars = name.chars();

    // First character must be a lowercase letter
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }

    // Rest must be lowercase letters, digits, or hyphens
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_role_name() {
        // Valid names
        assert!(is_valid_role_name("admin"));
        assert!(is_valid_role_name("super-admin"));
        assert!(is_valid_role_name("user123"));
        assert!(is_valid_role_name("a"));
        assert!(is_valid_role_name("admin-user-role"));

        // Invalid names
        assert!(!is_valid_role_name("")); // empty
        assert!(!is_valid_role_name("Admin")); // uppercase
        assert!(!is_valid_role_name("123admin")); // starts with number
        assert!(!is_valid_role_name("-admin")); // starts with hyphen
        assert!(!is_valid_role_name("admin_user")); // underscore
        assert!(!is_valid_role_name("admin user")); // space
        assert!(!is_valid_role_name(&"a".repeat(51))); // too long
    }
}
