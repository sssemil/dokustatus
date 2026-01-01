use async_trait::async_trait;
use chrono::NaiveDateTime;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::domain_billing::{
        BillingPaymentProfile, BillingPaymentRepo, BillingPaymentWithUser, CreatePaymentInput,
        PaginatedPayments, PaymentListFilters, PaymentSummary,
    },
    domain::entities::{
        payment_mode::PaymentMode, payment_provider::PaymentProvider,
        payment_status::PaymentStatus, stripe_mode::StripeMode,
    },
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> BillingPaymentProfile {
    BillingPaymentProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        stripe_mode: row.get("stripe_mode"),
        payment_provider: row.get::<Option<PaymentProvider>, _>("payment_provider"),
        payment_mode: row.get::<Option<PaymentMode>, _>("payment_mode"),
        end_user_id: row.get("end_user_id"),
        subscription_id: row.get("subscription_id"),
        stripe_invoice_id: row.get("stripe_invoice_id"),
        stripe_payment_intent_id: row.get("stripe_payment_intent_id"),
        stripe_customer_id: row.get("stripe_customer_id"),
        amount_cents: row.get("amount_cents"),
        amount_paid_cents: row.get("amount_paid_cents"),
        amount_refunded_cents: row.get("amount_refunded_cents"),
        currency: row.get("currency"),
        status: row.get("status"),
        plan_id: row.get("plan_id"),
        plan_code: row.get("plan_code"),
        plan_name: row.get("plan_name"),
        hosted_invoice_url: row.get("hosted_invoice_url"),
        invoice_pdf_url: row.get("invoice_pdf_url"),
        invoice_number: row.get("invoice_number"),
        billing_reason: row.get("billing_reason"),
        failure_message: row.get("failure_message"),
        invoice_created_at: row.get("invoice_created_at"),
        payment_date: row.get("payment_date"),
        refunded_at: row.get("refunded_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_payment_with_user(row: sqlx::postgres::PgRow) -> BillingPaymentWithUser {
    let user_email: String = row.get("user_email");
    BillingPaymentWithUser {
        payment: BillingPaymentProfile {
            id: row.get("id"),
            domain_id: row.get("domain_id"),
            stripe_mode: row.get("stripe_mode"),
            payment_provider: row.get::<Option<PaymentProvider>, _>("payment_provider"),
            payment_mode: row.get::<Option<PaymentMode>, _>("payment_mode"),
            end_user_id: row.get("end_user_id"),
            subscription_id: row.get("subscription_id"),
            stripe_invoice_id: row.get("stripe_invoice_id"),
            stripe_payment_intent_id: row.get("stripe_payment_intent_id"),
            stripe_customer_id: row.get("stripe_customer_id"),
            amount_cents: row.get("amount_cents"),
            amount_paid_cents: row.get("amount_paid_cents"),
            amount_refunded_cents: row.get("amount_refunded_cents"),
            currency: row.get("currency"),
            status: row.get("status"),
            plan_id: row.get("plan_id"),
            plan_code: row.get("plan_code"),
            plan_name: row.get("plan_name"),
            hosted_invoice_url: row.get("hosted_invoice_url"),
            invoice_pdf_url: row.get("invoice_pdf_url"),
            invoice_number: row.get("invoice_number"),
            billing_reason: row.get("billing_reason"),
            failure_message: row.get("failure_message"),
            invoice_created_at: row.get("invoice_created_at"),
            payment_date: row.get("payment_date"),
            refunded_at: row.get("refunded_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        },
        user_email,
    }
}

const SELECT_COLS: &str = r#"
    bp.id, bp.domain_id, bp.stripe_mode, bp.payment_provider, bp.payment_mode,
    bp.end_user_id, bp.subscription_id,
    bp.stripe_invoice_id, bp.stripe_payment_intent_id, bp.stripe_customer_id,
    bp.amount_cents, bp.amount_paid_cents, bp.amount_refunded_cents, bp.currency, bp.status,
    bp.plan_id, bp.plan_code, bp.plan_name,
    bp.hosted_invoice_url, bp.invoice_pdf_url, bp.invoice_number, bp.billing_reason,
    bp.failure_message, bp.invoice_created_at, bp.payment_date, bp.refunded_at,
    bp.created_at, bp.updated_at
"#;

#[async_trait]
impl BillingPaymentRepo for PostgresPersistence {
    async fn upsert_from_stripe(
        &self,
        input: &CreatePaymentInput,
    ) -> AppResult<BillingPaymentProfile> {
        let id = Uuid::new_v4();
        let row = sqlx::query(&format!(
            r#"
            INSERT INTO billing_payments (
                id, domain_id, stripe_mode, end_user_id, subscription_id,
                stripe_invoice_id, stripe_payment_intent_id, stripe_customer_id,
                amount_cents, amount_paid_cents, currency, status,
                plan_id, plan_code, plan_name,
                hosted_invoice_url, invoice_pdf_url, invoice_number, billing_reason,
                failure_message, invoice_created_at, payment_date
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22)
            ON CONFLICT (domain_id, stripe_mode, stripe_invoice_id) DO UPDATE SET
                stripe_payment_intent_id = COALESCE(EXCLUDED.stripe_payment_intent_id, billing_payments.stripe_payment_intent_id),
                amount_cents = EXCLUDED.amount_cents,
                amount_paid_cents = EXCLUDED.amount_paid_cents,
                -- Only update status if current status is not terminal (paid, refunded, partial_refund, void)
                status = CASE
                    WHEN billing_payments.status IN ('paid', 'refunded', 'partial_refund', 'void')
                    THEN billing_payments.status
                    ELSE EXCLUDED.status
                END,
                hosted_invoice_url = COALESCE(EXCLUDED.hosted_invoice_url, billing_payments.hosted_invoice_url),
                invoice_pdf_url = COALESCE(EXCLUDED.invoice_pdf_url, billing_payments.invoice_pdf_url),
                invoice_number = COALESCE(EXCLUDED.invoice_number, billing_payments.invoice_number),
                billing_reason = COALESCE(EXCLUDED.billing_reason, billing_payments.billing_reason),
                payment_date = COALESCE(EXCLUDED.payment_date, billing_payments.payment_date),
                updated_at = CURRENT_TIMESTAMP
            RETURNING {}
            "#,
            SELECT_COLS.replace("bp.", "")
        ))
        .bind(id)
        .bind(input.domain_id)
        .bind(input.stripe_mode)
        .bind(input.end_user_id)
        .bind(input.subscription_id)
        .bind(&input.stripe_invoice_id)
        .bind(&input.stripe_payment_intent_id)
        .bind(&input.stripe_customer_id)
        .bind(input.amount_cents)
        .bind(input.amount_paid_cents)
        .bind(&input.currency)
        .bind(input.status)
        .bind(input.plan_id)
        .bind(&input.plan_code)
        .bind(&input.plan_name)
        .bind(&input.hosted_invoice_url)
        .bind(&input.invoice_pdf_url)
        .bind(&input.invoice_number)
        .bind(&input.billing_reason)
        .bind(&input.failure_message)
        .bind(input.invoice_created_at)
        .bind(input.payment_date)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(row_to_profile(row))
    }

    async fn get_by_stripe_invoice_id(
        &self,
        stripe_invoice_id: &str,
    ) -> AppResult<Option<BillingPaymentProfile>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM billing_payments bp WHERE bp.stripe_invoice_id = $1",
            SELECT_COLS
        ))
        .bind(stripe_invoice_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(row.map(row_to_profile))
    }

    async fn list_by_user(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        end_user_id: Uuid,
        page: i32,
        per_page: i32,
    ) -> AppResult<PaginatedPayments> {
        let offset = (page - 1) * per_page;

        // Get total count
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM billing_payments WHERE domain_id = $1 AND stripe_mode = $2 AND end_user_id = $3",
        )
        .bind(domain_id)
        .bind(mode)
        .bind(end_user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        // Get paginated payments with user email
        let rows = sqlx::query(&format!(
            r#"
            SELECT {}, deu.email as user_email
            FROM billing_payments bp
            JOIN domain_end_users deu ON bp.end_user_id = deu.id
            WHERE bp.domain_id = $1 AND bp.stripe_mode = $2 AND bp.end_user_id = $3
            ORDER BY bp.payment_date DESC NULLS LAST, bp.created_at DESC
            LIMIT $4 OFFSET $5
            "#,
            SELECT_COLS
        ))
        .bind(domain_id)
        .bind(mode)
        .bind(end_user_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        let payments: Vec<BillingPaymentWithUser> =
            rows.into_iter().map(row_to_payment_with_user).collect();
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as i32;

        Ok(PaginatedPayments {
            payments,
            total,
            page,
            per_page,
            total_pages,
        })
    }

    async fn list_by_domain(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        filters: &PaymentListFilters,
        page: i32,
        per_page: i32,
    ) -> AppResult<PaginatedPayments> {
        let offset = (page - 1) * per_page;

        // Build dynamic WHERE clause
        let mut conditions: Vec<String> = vec![
            "bp.domain_id = $1".to_string(),
            "bp.stripe_mode = $2".to_string(),
        ];
        let mut param_count = 2;

        if filters.status.is_some() {
            param_count += 1;
            conditions.push(format!("bp.status = ${}", param_count));
        }
        if filters.date_from.is_some() {
            param_count += 1;
            conditions.push(format!(
                "(bp.payment_date >= ${} OR bp.created_at >= ${})",
                param_count, param_count
            ));
        }
        if filters.date_to.is_some() {
            param_count += 1;
            conditions.push(format!(
                "(bp.payment_date <= ${} OR bp.created_at <= ${})",
                param_count, param_count
            ));
        }
        if filters.plan_code.is_some() {
            param_count += 1;
            conditions.push(format!("bp.plan_code = ${}", param_count));
        }
        if filters.user_email.is_some() {
            param_count += 1;
            conditions.push(format!("deu.email ILIKE ${}", param_count));
        }

        let where_clause = conditions.join(" AND ");

        // Build and execute count query
        let count_query = format!(
            r#"
            SELECT COUNT(*)
            FROM billing_payments bp
            JOIN domain_end_users deu ON bp.end_user_id = deu.id
            WHERE {}
            "#,
            where_clause
        );

        let mut count_q = sqlx::query_scalar::<_, i64>(&count_query)
            .bind(domain_id)
            .bind(mode);

        if let Some(status) = &filters.status {
            count_q = count_q.bind(status);
        }
        if let Some(date_from) = &filters.date_from {
            count_q = count_q.bind(date_from);
        }
        if let Some(date_to) = &filters.date_to {
            count_q = count_q.bind(date_to);
        }
        if let Some(plan_code) = &filters.plan_code {
            count_q = count_q.bind(plan_code);
        }
        if let Some(user_email) = &filters.user_email {
            count_q = count_q.bind(format!("%{}%", user_email));
        }

        let total: i64 = count_q
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::from)?;

        // Build and execute paginated query
        let data_query = format!(
            r#"
            SELECT {}, deu.email as user_email
            FROM billing_payments bp
            JOIN domain_end_users deu ON bp.end_user_id = deu.id
            WHERE {}
            ORDER BY bp.payment_date DESC NULLS LAST, bp.created_at DESC
            LIMIT ${} OFFSET ${}
            "#,
            SELECT_COLS,
            where_clause,
            param_count + 1,
            param_count + 2
        );

        let mut data_q = sqlx::query(&data_query).bind(domain_id).bind(mode);

        if let Some(status) = &filters.status {
            data_q = data_q.bind(status);
        }
        if let Some(date_from) = &filters.date_from {
            data_q = data_q.bind(date_from);
        }
        if let Some(date_to) = &filters.date_to {
            data_q = data_q.bind(date_to);
        }
        if let Some(plan_code) = &filters.plan_code {
            data_q = data_q.bind(plan_code);
        }
        if let Some(user_email) = &filters.user_email {
            data_q = data_q.bind(format!("%{}%", user_email));
        }

        data_q = data_q.bind(per_page).bind(offset);

        let rows = data_q.fetch_all(&self.pool).await.map_err(AppError::from)?;
        let payments: Vec<BillingPaymentWithUser> =
            rows.into_iter().map(row_to_payment_with_user).collect();
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as i32;

        Ok(PaginatedPayments {
            payments,
            total,
            page,
            per_page,
            total_pages,
        })
    }

    async fn update_status(
        &self,
        stripe_invoice_id: &str,
        status: PaymentStatus,
        amount_refunded_cents: Option<i32>,
        failure_message: Option<String>,
    ) -> AppResult<()> {
        let refunded_at = if status.is_refunded() {
            Some(chrono::Utc::now().naive_utc())
        } else {
            None
        };

        // Update with terminal state protection:
        // - 'refunded' and 'void' are fully terminal, never update
        // - 'paid' can only transition to 'refunded' or 'partial_refund'
        // - 'partial_refund' can only transition to 'refunded'
        // - Other states (pending, failed, uncollectible) can transition to anything
        let result = sqlx::query(
            r#"
            UPDATE billing_payments SET
                status = $2,
                amount_refunded_cents = COALESCE($3, amount_refunded_cents),
                failure_message = COALESCE($4, failure_message),
                refunded_at = COALESCE($5, refunded_at),
                updated_at = CURRENT_TIMESTAMP
            WHERE stripe_invoice_id = $1
              AND (
                -- Fully terminal states: never update
                status NOT IN ('refunded', 'void')
                -- 'paid' can only go to refund states
                AND (status != 'paid' OR $2 IN ('refunded', 'partial_refund'))
                -- 'partial_refund' can only go to 'refunded'
                AND (status != 'partial_refund' OR $2 = 'refunded')
              )
            "#,
        )
        .bind(stripe_invoice_id)
        .bind(status)
        .bind(amount_refunded_cents)
        .bind(&failure_message)
        .bind(refunded_at)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;

        if result.rows_affected() == 0 {
            // Check if invoice exists to distinguish between "not found" and "blocked by terminal state"
            let exists: Option<i64> =
                sqlx::query_scalar("SELECT 1 FROM billing_payments WHERE stripe_invoice_id = $1")
                    .bind(stripe_invoice_id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(AppError::from)?;

            if exists.is_some() {
                tracing::debug!(
                    "Payment status update skipped for {} - current status is terminal or transition not allowed to {:?}",
                    stripe_invoice_id,
                    status
                );
            } else {
                tracing::warn!(
                    "Payment status update failed - invoice {} not found",
                    stripe_invoice_id
                );
            }
        }

        Ok(())
    }

    async fn get_payment_summary(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        date_from: Option<NaiveDateTime>,
        date_to: Option<NaiveDateTime>,
    ) -> AppResult<PaymentSummary> {
        let mut conditions: Vec<String> =
            vec!["domain_id = $1".to_string(), "stripe_mode = $2".to_string()];
        let mut param_count = 2;

        if date_from.is_some() {
            param_count += 1;
            conditions.push(format!(
                "(payment_date >= ${} OR created_at >= ${})",
                param_count, param_count
            ));
        }
        if date_to.is_some() {
            param_count += 1;
            conditions.push(format!(
                "(payment_date <= ${} OR created_at <= ${})",
                param_count, param_count
            ));
        }

        let where_clause = conditions.join(" AND ");

        let query = format!(
            r#"
            SELECT
                COALESCE(SUM(CASE WHEN status = 'paid' THEN amount_paid_cents ELSE 0 END), 0) as total_revenue_cents,
                COALESCE(SUM(amount_refunded_cents), 0) as total_refunded_cents,
                COUNT(*) as payment_count,
                COUNT(*) FILTER (WHERE status = 'paid') as successful_payments,
                COUNT(*) FILTER (WHERE status IN ('failed', 'uncollectible', 'void')) as failed_payments
            FROM billing_payments
            WHERE {}
            "#,
            where_clause
        );

        let mut q = sqlx::query(&query).bind(domain_id).bind(mode);

        if let Some(df) = &date_from {
            q = q.bind(df);
        }
        if let Some(dt) = &date_to {
            q = q.bind(dt);
        }

        let row = q.fetch_one(&self.pool).await.map_err(AppError::from)?;

        Ok(PaymentSummary {
            total_revenue_cents: row.get("total_revenue_cents"),
            total_refunded_cents: row.get("total_refunded_cents"),
            payment_count: row.get("payment_count"),
            successful_payments: row.get("successful_payments"),
            failed_payments: row.get("failed_payments"),
        })
    }

    async fn list_all_for_export(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        filters: &PaymentListFilters,
    ) -> AppResult<Vec<BillingPaymentWithUser>> {
        // Build dynamic WHERE clause
        let mut conditions: Vec<String> = vec![
            "bp.domain_id = $1".to_string(),
            "bp.stripe_mode = $2".to_string(),
        ];
        let mut param_count = 2;

        if filters.status.is_some() {
            param_count += 1;
            conditions.push(format!("bp.status = ${}", param_count));
        }
        if filters.date_from.is_some() {
            param_count += 1;
            conditions.push(format!(
                "(bp.payment_date >= ${} OR bp.created_at >= ${})",
                param_count, param_count
            ));
        }
        if filters.date_to.is_some() {
            param_count += 1;
            conditions.push(format!(
                "(bp.payment_date <= ${} OR bp.created_at <= ${})",
                param_count, param_count
            ));
        }
        if filters.plan_code.is_some() {
            param_count += 1;
            conditions.push(format!("bp.plan_code = ${}", param_count));
        }
        if filters.user_email.is_some() {
            param_count += 1;
            conditions.push(format!("deu.email ILIKE ${}", param_count));
        }

        let where_clause = conditions.join(" AND ");

        let query = format!(
            r#"
            SELECT {}, deu.email as user_email
            FROM billing_payments bp
            JOIN domain_end_users deu ON bp.end_user_id = deu.id
            WHERE {}
            ORDER BY bp.payment_date DESC NULLS LAST, bp.created_at DESC
            "#,
            SELECT_COLS, where_clause
        );

        let mut q = sqlx::query(&query).bind(domain_id).bind(mode);

        if let Some(status) = &filters.status {
            q = q.bind(status);
        }
        if let Some(date_from) = &filters.date_from {
            q = q.bind(date_from);
        }
        if let Some(date_to) = &filters.date_to {
            q = q.bind(date_to);
        }
        if let Some(plan_code) = &filters.plan_code {
            q = q.bind(plan_code);
        }
        if let Some(user_email) = &filters.user_email {
            q = q.bind(format!("%{}%", user_email));
        }

        let rows = q.fetch_all(&self.pool).await.map_err(AppError::from)?;
        Ok(rows.into_iter().map(row_to_payment_with_user).collect())
    }
}
