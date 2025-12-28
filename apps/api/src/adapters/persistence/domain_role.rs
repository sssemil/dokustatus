use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    domain::entities::domain_role::DomainRole,
};

#[derive(Debug, Clone)]
pub struct DomainRoleWithCount {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub name: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub user_count: i64,
}

#[async_trait]
pub trait DomainRoleRepo: Send + Sync {
    async fn create(&self, domain_id: Uuid, name: &str) -> AppResult<DomainRole>;
    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<DomainRole>>;
    async fn list_by_domain_with_counts(&self, domain_id: Uuid) -> AppResult<Vec<DomainRoleWithCount>>;
    async fn get_by_name(&self, domain_id: Uuid, name: &str) -> AppResult<Option<DomainRole>>;
    async fn delete(&self, domain_id: Uuid, name: &str) -> AppResult<()>;
    async fn exists(&self, domain_id: Uuid, name: &str) -> AppResult<bool>;
}

fn row_to_role(row: sqlx::postgres::PgRow) -> DomainRole {
    DomainRole {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        name: row.get("name"),
        created_at: row.get("created_at"),
    }
}

fn row_to_role_with_count(row: sqlx::postgres::PgRow) -> DomainRoleWithCount {
    DomainRoleWithCount {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        name: row.get("name"),
        created_at: row.get("created_at"),
        user_count: row.get("user_count"),
    }
}

#[async_trait]
impl DomainRoleRepo for PostgresPersistence {
    async fn create(&self, domain_id: Uuid, name: &str) -> AppResult<DomainRole> {
        let row = sqlx::query(
            r#"
            INSERT INTO domain_roles (domain_id, name)
            VALUES ($1, $2)
            RETURNING id, domain_id, name, created_at
            "#,
        )
        .bind(domain_id)
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(row_to_role(row))
    }

    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<DomainRole>> {
        let rows = sqlx::query(
            r#"
            SELECT id, domain_id, name, created_at
            FROM domain_roles
            WHERE domain_id = $1
            ORDER BY name ASC
            "#,
        )
        .bind(domain_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(row_to_role).collect())
    }

    async fn list_by_domain_with_counts(&self, domain_id: Uuid) -> AppResult<Vec<DomainRoleWithCount>> {
        let rows = sqlx::query(
            r#"
            SELECT
                r.id,
                r.domain_id,
                r.name,
                r.created_at,
                COALESCE(
                    (SELECT COUNT(*) FROM domain_end_users u
                     WHERE u.domain_id = r.domain_id
                     AND u.roles @> to_jsonb(r.name)),
                    0
                ) as user_count
            FROM domain_roles r
            WHERE r.domain_id = $1
            ORDER BY r.name ASC
            "#,
        )
        .bind(domain_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(row_to_role_with_count).collect())
    }

    async fn get_by_name(&self, domain_id: Uuid, name: &str) -> AppResult<Option<DomainRole>> {
        let row = sqlx::query(
            r#"
            SELECT id, domain_id, name, created_at
            FROM domain_roles
            WHERE domain_id = $1 AND name = $2
            "#,
        )
        .bind(domain_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(row.map(row_to_role))
    }

    async fn delete(&self, domain_id: Uuid, name: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM domain_roles WHERE domain_id = $1 AND name = $2")
            .bind(domain_id)
            .bind(name)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(())
    }

    async fn exists(&self, domain_id: Uuid, name: &str) -> AppResult<bool> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM domain_roles WHERE domain_id = $1 AND name = $2",
        )
        .bind(domain_id)
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(row.0 > 0)
    }
}
