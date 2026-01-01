use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::domain_billing::{
        CreateSubscriptionInput, StripeSubscriptionUpdate, SubscriptionPlanProfile,
        UserSubscriptionProfile, UserSubscriptionRepo, UserSubscriptionWithPlan,
    },
    domain::entities::billing_state::BillingState,
    domain::entities::payment_mode::PaymentMode,
    domain::entities::payment_provider::PaymentProvider,
    domain::entities::stripe_mode::StripeMode,
    domain::entities::user_subscription::SubscriptionStatus,
};

fn row_to_profile(row: &sqlx::postgres::PgRow) -> UserSubscriptionProfile {
    UserSubscriptionProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        stripe_mode: row.get("stripe_mode"),
        payment_provider: row.get::<Option<PaymentProvider>, _>("payment_provider"),
        payment_mode: row.get::<Option<PaymentMode>, _>("payment_mode"),
        billing_state: row.get::<Option<BillingState>, _>("billing_state"),
        end_user_id: row.get("end_user_id"),
        plan_id: row.get("plan_id"),
        status: row.get("status"),
        stripe_customer_id: row.get("stripe_customer_id"),
        stripe_subscription_id: row.get("stripe_subscription_id"),
        current_period_start: row.get("current_period_start"),
        current_period_end: row.get("current_period_end"),
        trial_start: row.get("trial_start"),
        trial_end: row.get("trial_end"),
        cancel_at_period_end: row.get("cancel_at_period_end"),
        canceled_at: row.get("canceled_at"),
        manually_granted: row.get("manually_granted"),
        granted_by: row.get("granted_by"),
        granted_at: row.get("granted_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

const SELECT_COLS: &str = r#"
    id, domain_id, stripe_mode, payment_provider, payment_mode, billing_state,
    end_user_id, plan_id, status, stripe_customer_id, stripe_subscription_id,
    current_period_start, current_period_end, trial_start, trial_end,
    cancel_at_period_end, canceled_at, manually_granted, granted_by, granted_at,
    created_at, updated_at
"#;

#[async_trait]
impl UserSubscriptionRepo for PostgresPersistence {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<UserSubscriptionProfile>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM user_subscriptions WHERE id = $1",
            SELECT_COLS
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.as_ref().map(row_to_profile))
    }

    async fn get_by_user_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        end_user_id: Uuid,
    ) -> AppResult<Option<UserSubscriptionProfile>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM user_subscriptions WHERE domain_id = $1 AND stripe_mode = $2 AND end_user_id = $3",
            SELECT_COLS
        ))
        .bind(domain_id)
        .bind(mode)
        .bind(end_user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.as_ref().map(row_to_profile))
    }

    async fn get_by_stripe_subscription_id(
        &self,
        stripe_subscription_id: &str,
    ) -> AppResult<Option<UserSubscriptionProfile>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM user_subscriptions WHERE stripe_subscription_id = $1",
            SELECT_COLS
        ))
        .bind(stripe_subscription_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.as_ref().map(row_to_profile))
    }

    async fn get_by_stripe_customer_id(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        stripe_customer_id: &str,
    ) -> AppResult<Option<UserSubscriptionProfile>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM user_subscriptions WHERE domain_id = $1 AND stripe_mode = $2 AND stripe_customer_id = $3",
            SELECT_COLS
        ))
        .bind(domain_id)
        .bind(mode)
        .bind(stripe_customer_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.as_ref().map(row_to_profile))
    }

    async fn list_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<Vec<UserSubscriptionWithPlan>> {
        let rows = sqlx::query(
            r#"
            SELECT
                s.id, s.domain_id, s.stripe_mode, s.payment_provider, s.payment_mode, s.billing_state,
                s.end_user_id, s.plan_id, s.status, s.stripe_customer_id, s.stripe_subscription_id,
                s.current_period_start, s.current_period_end, s.trial_start, s.trial_end,
                s.cancel_at_period_end, s.canceled_at, s.manually_granted, s.granted_by, s.granted_at,
                s.created_at, s.updated_at,
                p.id as p_id, p.domain_id as p_domain_id, p.stripe_mode as p_stripe_mode,
                p.payment_provider as p_payment_provider, p.payment_mode as p_payment_mode,
                p.code as p_code, p.name as p_name,
                p.description as p_description, p.price_cents as p_price_cents, p.currency as p_currency,
                p.interval as p_interval, p.interval_count as p_interval_count, p.trial_days as p_trial_days,
                p.features as p_features, p.is_public as p_is_public, p.display_order as p_display_order,
                p.stripe_product_id as p_stripe_product_id, p.stripe_price_id as p_stripe_price_id,
                p.is_archived as p_is_archived, p.archived_at as p_archived_at,
                p.created_at as p_created_at, p.updated_at as p_updated_at,
                u.email as user_email
            FROM user_subscriptions s
            JOIN subscription_plans p ON s.plan_id = p.id
            JOIN domain_end_users u ON s.end_user_id = u.id
            WHERE s.domain_id = $1 AND s.stripe_mode = $2
            ORDER BY s.created_at DESC
            "#
        )
        .bind(domain_id)
        .bind(mode)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows
            .iter()
            .map(|row| {
                let plan_id: uuid::Uuid = row.get("p_id");
                let features_json: serde_json::Value = row.get("p_features");
                let features: Vec<String> = super::parse_json_with_fallback(
                    &features_json,
                    "features",
                    "subscription_plan",
                    &plan_id.to_string(),
                );

                UserSubscriptionWithPlan {
                    subscription: row_to_profile(row),
                    plan: SubscriptionPlanProfile {
                        id: plan_id,
                        domain_id: row.get("p_domain_id"),
                        stripe_mode: row.get("p_stripe_mode"),
                        payment_provider: row
                            .get::<Option<PaymentProvider>, _>("p_payment_provider"),
                        payment_mode: row.get::<Option<PaymentMode>, _>("p_payment_mode"),
                        code: row.get("p_code"),
                        name: row.get("p_name"),
                        description: row.get("p_description"),
                        price_cents: row.get("p_price_cents"),
                        currency: row.get("p_currency"),
                        interval: row.get("p_interval"),
                        interval_count: row.get("p_interval_count"),
                        trial_days: row.get("p_trial_days"),
                        features,
                        is_public: row.get("p_is_public"),
                        display_order: row.get("p_display_order"),
                        stripe_product_id: row.get("p_stripe_product_id"),
                        stripe_price_id: row.get("p_stripe_price_id"),
                        is_archived: row.get("p_is_archived"),
                        archived_at: row.get("p_archived_at"),
                        created_at: row.get("p_created_at"),
                        updated_at: row.get("p_updated_at"),
                    },
                    user_email: row.get("user_email"),
                }
            })
            .collect())
    }

    async fn list_by_plan(&self, plan_id: Uuid) -> AppResult<Vec<UserSubscriptionProfile>> {
        let rows = sqlx::query(&format!(
            "SELECT {} FROM user_subscriptions WHERE plan_id = $1 ORDER BY created_at DESC",
            SELECT_COLS
        ))
        .bind(plan_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows.iter().map(row_to_profile).collect())
    }

    async fn create(&self, input: &CreateSubscriptionInput) -> AppResult<UserSubscriptionProfile> {
        let id = Uuid::new_v4();
        let row = sqlx::query(&format!(
            r#"
            INSERT INTO user_subscriptions
                (id, domain_id, stripe_mode, end_user_id, plan_id, status, stripe_customer_id, stripe_subscription_id,
                 current_period_start, current_period_end, trial_start, trial_end)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(id)
        .bind(input.domain_id)
        .bind(input.stripe_mode)
        .bind(input.end_user_id)
        .bind(input.plan_id)
        .bind(input.status)
        .bind(&input.stripe_customer_id)
        .bind(&input.stripe_subscription_id)
        .bind(input.current_period_start)
        .bind(input.current_period_end)
        .bind(input.trial_start)
        .bind(input.trial_end)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(&row))
    }

    async fn update_from_stripe(
        &self,
        id: Uuid,
        update: &StripeSubscriptionUpdate,
    ) -> AppResult<UserSubscriptionProfile> {
        let row = sqlx::query(&format!(
            r#"
            UPDATE user_subscriptions SET
                status = $2,
                plan_id = COALESCE($3, plan_id),
                stripe_subscription_id = COALESCE($4, stripe_subscription_id),
                current_period_start = $5,
                current_period_end = $6,
                cancel_at_period_end = $7,
                canceled_at = $8,
                trial_start = $9,
                trial_end = $10,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(id)
        .bind(update.status)
        .bind(update.plan_id)
        .bind(&update.stripe_subscription_id)
        .bind(update.current_period_start)
        .bind(update.current_period_end)
        .bind(update.cancel_at_period_end)
        .bind(update.canceled_at)
        .bind(update.trial_start)
        .bind(update.trial_end)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(&row))
    }

    async fn update_plan(&self, id: Uuid, plan_id: Uuid) -> AppResult<()> {
        sqlx::query(
            "UPDATE user_subscriptions SET plan_id = $2, updated_at = CURRENT_TIMESTAMP WHERE id = $1"
        )
        .bind(id)
        .bind(plan_id)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn grant_manually(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        end_user_id: Uuid,
        plan_id: Uuid,
        granted_by: Uuid,
        stripe_customer_id: &str,
    ) -> AppResult<UserSubscriptionProfile> {
        let id = Uuid::new_v4();
        let row = sqlx::query(&format!(
            r#"
            INSERT INTO user_subscriptions
                (id, domain_id, stripe_mode, end_user_id, plan_id, status, stripe_customer_id,
                 manually_granted, granted_by, granted_at)
            VALUES ($1, $2, $3, $4, $5, 'active', $6, true, $7, CURRENT_TIMESTAMP)
            ON CONFLICT (domain_id, stripe_mode, end_user_id) DO UPDATE SET
                plan_id = EXCLUDED.plan_id,
                status = 'active',
                manually_granted = true,
                granted_by = EXCLUDED.granted_by,
                granted_at = CURRENT_TIMESTAMP,
                cancel_at_period_end = false,
                canceled_at = NULL,
                updated_at = CURRENT_TIMESTAMP
            RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(id)
        .bind(domain_id)
        .bind(mode)
        .bind(end_user_id)
        .bind(plan_id)
        .bind(stripe_customer_id)
        .bind(granted_by)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(&row))
    }

    async fn revoke(&self, id: Uuid) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE user_subscriptions SET
                status = 'canceled',
                canceled_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM user_subscriptions WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn count_active_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_subscriptions WHERE domain_id = $1 AND stripe_mode = $2 AND status IN ('active', 'trialing')"
        )
        .bind(domain_id)
        .bind(mode)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(count)
    }

    async fn count_by_status_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        status: SubscriptionStatus,
    ) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_subscriptions WHERE domain_id = $1 AND stripe_mode = $2 AND status = $3"
        )
        .bind(domain_id)
        .bind(mode)
        .bind(status)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(count)
    }

    async fn count_by_domain_and_mode(&self, domain_id: Uuid, mode: StripeMode) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_subscriptions WHERE domain_id = $1 AND stripe_mode = $2",
        )
        .bind(domain_id)
        .bind(mode)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(count)
    }
}
