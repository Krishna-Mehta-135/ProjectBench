use crate::ProjectSummary;
use sqlx::{postgres::PgPoolOptions, PgPool};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database connection failed: {0}")]
    Connection(#[from] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("run configuration is not approved: {0}")]
    Configuration(String),
}

#[derive(Clone)]
pub struct PgRepository {
    pub(crate) pool: PgPool,
}

impl PgRepository {
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn connect(database_url: &str) -> Result<Self, StorageError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        sqlx::migrate!("../../migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn ready(&self) -> Result<(), StorageError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectSummary>, StorageError> {
        Ok(sqlx::query_as::<_, ProjectSummary>("SELECT p.id, p.name, COALESCE((SELECT e.name FROM environments e WHERE e.project_id = p.id AND e.enabled ORDER BY e.name LIMIT 1), '') AS environment, (SELECT r.id FROM runs r WHERE r.project_id = p.id ORDER BY r.created_at DESC LIMIT 1) AS latest_run_id FROM projects p ORDER BY p.name").fetch_all(&self.pool).await?)
    }

    pub async fn record_audit_event(
        &self,
        action: &str,
        resource_type: &str,
        resource_id: Option<uuid::Uuid>,
        metadata: &serde_json::Value,
    ) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO audit_events (id, action, resource_type, resource_id, metadata, created_at) VALUES ($1, $2, $3, $4, $5, NOW())")
            .bind(uuid::Uuid::new_v4())
            .bind(action)
            .bind(resource_type)
            .bind(resource_id)
            .bind(metadata)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_audit_events(&self) -> Result<Vec<crate::AuditEvent>, StorageError> {
        Ok(sqlx::query_as::<_, crate::AuditEvent>(
            "SELECT id, action, resource_type, resource_id, metadata, created_at FROM audit_events ORDER BY created_at DESC, id DESC",
        )
        .fetch_all(&self.pool)
        .await?)
    }
}

pub use crate::storage_run_queries::CompareRunRecord;

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
