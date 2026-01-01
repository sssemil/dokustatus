use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::domain_billing::{
        CreatePlanInput, SubscriptionPlanProfile, SubscriptionPlanRepo, UpdatePlanInput,
    },
    domain::entities::payment_mode::PaymentMode,
    domain::entities::payment_provider::PaymentProvider,
    domain::entities::stripe_mode::StripeMode,
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> SubscriptionPlanProfile {
    let features_json: serde_json::Value = row.get("features");
    let features: Vec<String> = serde_json::from_value(features_json).unwrap_or_default();

    SubscriptionPlanProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        stripe_mode: row.get("stripe_mode"),
        payment_provider: row.get::<Option<PaymentProvider>, _>("payment_provider"),
        payment_mode: row.get::<Option<PaymentMode>, _>("payment_mode"),
        code: row.get("code"),
        name: row.get("name"),
        description: row.get("description"),
        price_cents: row.get("price_cents"),
        currency: row.get("currency"),
        interval: row.get("interval"),
        interval_count: row.get("interval_count"),
        trial_days: row.get("trial_days"),
        features,
        is_public: row.get("is_public"),
        display_order: row.get("display_order"),
        stripe_product_id: row.get("stripe_product_id"),
        stripe_price_id: row.get("stripe_price_id"),
        is_archived: row.get("is_archived"),
        archived_at: row.get("archived_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

const SELECT_COLS: &str = r#"
    id, domain_id, stripe_mode, payment_provider, payment_mode,
    code, name, description, price_cents, currency,
    interval, interval_count, trial_days, features, is_public, display_order,
    stripe_product_id, stripe_price_id, is_archived, archived_at, created_at, updated_at
"#;

#[async_trait]
impl SubscriptionPlanRepo for PostgresPersistence {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<SubscriptionPlanProfile>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM subscription_plans WHERE id = $1",
            SELECT_COLS
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }

    async fn get_by_domain_and_code(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        code: &str,
    ) -> AppResult<Option<SubscriptionPlanProfile>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM subscription_plans WHERE domain_id = $1 AND stripe_mode = $2 AND code = $3",
            SELECT_COLS
        ))
        .bind(domain_id)
        .bind(mode)
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }

    async fn list_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        include_archived: bool,
    ) -> AppResult<Vec<SubscriptionPlanProfile>> {
        let query = if include_archived {
            format!(
                "SELECT {} FROM subscription_plans WHERE domain_id = $1 AND stripe_mode = $2 ORDER BY display_order, created_at",
                SELECT_COLS
            )
        } else {
            format!(
                "SELECT {} FROM subscription_plans WHERE domain_id = $1 AND stripe_mode = $2 AND is_archived = false ORDER BY display_order, created_at",
                SELECT_COLS
            )
        };
        let rows = sqlx::query(&query)
            .bind(domain_id)
            .bind(mode)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn list_public_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<Vec<SubscriptionPlanProfile>> {
        let rows = sqlx::query(&format!(
            "SELECT {} FROM subscription_plans WHERE domain_id = $1 AND stripe_mode = $2 AND is_public = true AND is_archived = false ORDER BY display_order, created_at",
            SELECT_COLS
        ))
        .bind(domain_id)
        .bind(mode)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn create(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        input: &CreatePlanInput,
    ) -> AppResult<SubscriptionPlanProfile> {
        let id = Uuid::new_v4();
        let features_json = serde_json::to_value(&input.features).unwrap_or(serde_json::json!([]));

        // Get max display_order for this domain and mode
        let max_order: Option<i32> = sqlx::query_scalar(
            "SELECT MAX(display_order) FROM subscription_plans WHERE domain_id = $1 AND stripe_mode = $2"
        )
        .bind(domain_id)
        .bind(mode)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        let display_order = max_order.unwrap_or(-1) + 1;

        let row = sqlx::query(&format!(
            r#"
            INSERT INTO subscription_plans
                (id, domain_id, stripe_mode, code, name, description, price_cents, currency,
                 interval, interval_count, trial_days, features, is_public, display_order)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(id)
        .bind(domain_id)
        .bind(mode)
        .bind(&input.code)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.price_cents)
        .bind(&input.currency)
        .bind(&input.interval)
        .bind(input.interval_count)
        .bind(input.trial_days)
        .bind(features_json)
        .bind(input.is_public)
        .bind(display_order)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn update(
        &self,
        id: Uuid,
        input: &UpdatePlanInput,
    ) -> AppResult<SubscriptionPlanProfile> {
        // For simplicity, we'll use a simpler approach with COALESCE
        let row = sqlx::query(&format!(
            r#"
            UPDATE subscription_plans SET
                name = COALESCE($2, name),
                description = COALESCE($3, description),
                price_cents = COALESCE($4, price_cents),
                interval = COALESCE($5, interval),
                interval_count = COALESCE($6, interval_count),
                trial_days = COALESCE($7, trial_days),
                features = COALESCE($8, features),
                is_public = COALESCE($9, is_public),
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.price_cents)
        .bind(&input.interval)
        .bind(input.interval_count)
        .bind(input.trial_days)
        .bind(
            input
                .features
                .as_ref()
                .map(|f| serde_json::to_value(f).unwrap_or(serde_json::json!([]))),
        )
        .bind(input.is_public)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn set_stripe_ids(&self, id: Uuid, product_id: &str, price_id: &str) -> AppResult<()> {
        sqlx::query(
            "UPDATE subscription_plans SET stripe_product_id = $2, stripe_price_id = $3, updated_at = CURRENT_TIMESTAMP WHERE id = $1"
        )
        .bind(id)
        .bind(product_id)
        .bind(price_id)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn set_display_order(&self, id: Uuid, order: i32) -> AppResult<()> {
        sqlx::query(
            "UPDATE subscription_plans SET display_order = $2, updated_at = CURRENT_TIMESTAMP WHERE id = $1"
        )
        .bind(id)
        .bind(order)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn archive(&self, id: Uuid) -> AppResult<()> {
        sqlx::query(
            "UPDATE subscription_plans SET is_archived = true, archived_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = $1"
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM subscription_plans WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn count_subscribers(&self, plan_id: Uuid) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_subscriptions WHERE plan_id = $1 AND status IN ('active', 'trialing', 'past_due')"
        )
        .bind(plan_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(count)
    }

    async fn count_by_domain_and_mode(&self, domain_id: Uuid, mode: StripeMode) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM subscription_plans WHERE domain_id = $1 AND stripe_mode = $2",
        )
        .bind(domain_id)
        .bind(mode)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(count)
    }

    async fn get_by_stripe_price_id(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        stripe_price_id: &str,
    ) -> AppResult<Option<SubscriptionPlanProfile>> {
        // Search ALL plans in the mode (including archived) to handle plans whose visibility changed after purchase
        let row = sqlx::query(&format!(
            "SELECT {} FROM subscription_plans WHERE domain_id = $1 AND stripe_mode = $2 AND stripe_price_id = $3",
            SELECT_COLS
        ))
        .bind(domain_id)
        .bind(mode)
        .bind(stripe_price_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }
}
