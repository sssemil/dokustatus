use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    domain::entities::{payment_mode::PaymentMode, payment_provider::PaymentProvider},
};

// ============================================================================
// Profile Types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct EnabledPaymentProviderProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub provider: PaymentProvider,
    pub mode: PaymentMode,
    pub is_active: bool,
    pub display_order: i32,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

// ============================================================================
// Repository Trait
// ============================================================================

#[async_trait]
pub trait EnabledPaymentProvidersRepoTrait: Send + Sync {
    /// List all enabled providers for a domain
    async fn list_by_domain(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Vec<EnabledPaymentProviderProfile>>;

    /// List only active providers for a domain
    async fn list_active_by_domain(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Vec<EnabledPaymentProviderProfile>>;

    /// Get a specific provider config
    async fn get(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<Option<EnabledPaymentProviderProfile>>;

    /// Enable a provider for a domain
    async fn enable(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
        display_order: i32,
    ) -> AppResult<EnabledPaymentProviderProfile>;

    /// Disable a provider for a domain
    async fn disable(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<()>;

    /// Set active status for a provider
    async fn set_active(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
        is_active: bool,
    ) -> AppResult<()>;

    /// Update display order for a provider
    async fn set_display_order(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
        display_order: i32,
    ) -> AppResult<()>;

    /// Check if a provider is enabled for a domain
    async fn is_enabled(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<bool>;
}

// ============================================================================
// Implementation
// ============================================================================

fn row_to_profile(row: sqlx::postgres::PgRow) -> EnabledPaymentProviderProfile {
    EnabledPaymentProviderProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        provider: row.get("provider"),
        mode: row.get("mode"),
        is_active: row.get("is_active"),
        display_order: row.get("display_order"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

const SELECT_COLS: &str = r#"
    id, domain_id, provider, mode, is_active, display_order, created_at, updated_at
"#;

#[async_trait]
impl EnabledPaymentProvidersRepoTrait for PostgresPersistence {
    async fn list_by_domain(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Vec<EnabledPaymentProviderProfile>> {
        let rows = sqlx::query(&format!(
            "SELECT {} FROM domain_enabled_payment_providers WHERE domain_id = $1 ORDER BY display_order, created_at",
            SELECT_COLS
        ))
        .bind(domain_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn list_active_by_domain(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Vec<EnabledPaymentProviderProfile>> {
        let rows = sqlx::query(&format!(
            "SELECT {} FROM domain_enabled_payment_providers WHERE domain_id = $1 AND is_active = true ORDER BY display_order, created_at",
            SELECT_COLS
        ))
        .bind(domain_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn get(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<Option<EnabledPaymentProviderProfile>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM domain_enabled_payment_providers WHERE domain_id = $1 AND provider = $2 AND mode = $3",
            SELECT_COLS
        ))
        .bind(domain_id)
        .bind(provider)
        .bind(mode)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(row.map(row_to_profile))
    }

    async fn enable(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
        display_order: i32,
    ) -> AppResult<EnabledPaymentProviderProfile> {
        let id = Uuid::new_v4();
        let row = sqlx::query(&format!(
            r#"
            INSERT INTO domain_enabled_payment_providers
                (id, domain_id, provider, mode, is_active, display_order)
            VALUES ($1, $2, $3, $4, true, $5)
            ON CONFLICT (domain_id, provider, mode) DO UPDATE SET
                is_active = true,
                display_order = EXCLUDED.display_order,
                updated_at = CURRENT_TIMESTAMP
            RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(id)
        .bind(domain_id)
        .bind(provider)
        .bind(mode)
        .bind(display_order)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(row_to_profile(row))
    }

    async fn disable(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<()> {
        sqlx::query(
            "DELETE FROM domain_enabled_payment_providers WHERE domain_id = $1 AND provider = $2 AND mode = $3"
        )
        .bind(domain_id)
        .bind(provider)
        .bind(mode)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(())
    }

    async fn set_active(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
        is_active: bool,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE domain_enabled_payment_providers
            SET is_active = $4, updated_at = CURRENT_TIMESTAMP
            WHERE domain_id = $1 AND provider = $2 AND mode = $3
            "#,
        )
        .bind(domain_id)
        .bind(provider)
        .bind(mode)
        .bind(is_active)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(())
    }

    async fn set_display_order(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
        display_order: i32,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE domain_enabled_payment_providers
            SET display_order = $4, updated_at = CURRENT_TIMESTAMP
            WHERE domain_id = $1 AND provider = $2 AND mode = $3
            "#,
        )
        .bind(domain_id)
        .bind(provider)
        .bind(mode)
        .bind(display_order)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(())
    }

    async fn is_enabled(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM domain_enabled_payment_providers WHERE domain_id = $1 AND provider = $2 AND mode = $3 AND is_active = true"
        )
        .bind(domain_id)
        .bind(provider)
        .bind(mode)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(count > 0)
    }
}
