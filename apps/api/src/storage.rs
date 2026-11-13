use crate::{ProjectSummary, RunSummary};
use sqlx::{postgres::PgPoolOptions, PgPool};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database connection failed: {0}")]
    Connection(#[from] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

#[derive(Clone)]
pub struct PgRepository {
    pool: PgPool,
}

impl PgRepository {
    pub async fn connect(database_url: &str) -> Result<Self, StorageError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        sqlx::migrate!("../../migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectSummary>, StorageError> {
        Ok(sqlx::query_as::<_, ProjectSummary>("SELECT id, name, '' AS environment, NULL::uuid AS latest_run_id FROM projects ORDER BY name").fetch_all(&self.pool).await?)
    }

    pub async fn list_runs(&self) -> Result<Vec<RunSummary>, StorageError> {
        Ok(sqlx::query_as::<_, RunSummary>("SELECT id, status, TRUE AS synthetic_traffic, (report IS NOT NULL) AS report_available FROM runs ORDER BY created_at DESC").fetch_all(&self.pool).await?)
    }

    pub async fn get_run(
        &self,
        run_id: uuid::Uuid,
    ) -> Result<Option<crate::RunDetail>, StorageError> {
        Ok(sqlx::query_as::<_, crate::RunDetail>("SELECT id, status, target_origin, workload, scenario_snapshot, report FROM runs WHERE id = $1").bind(run_id).fetch_optional(&self.pool).await?)
    }

    pub async fn get_report(
        &self,
        run_id: uuid::Uuid,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        Ok(sqlx::query_scalar::<_, Option<serde_json::Value>>(
            "SELECT report FROM runs WHERE id = $1",
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?
        .flatten())
    }

    pub async fn create_queued_run(
        &self,
        request: &crate::CreateRunRequest,
        scenario_hash: &str,
    ) -> Result<uuid::Uuid, StorageError> {
        let id = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO runs (id, project_id, environment_id, scenario_id, status, target_origin, workload, scenario_snapshot, scenario_sha256, created_at) VALUES ($1, $2, $3, $4, 'queued', $5, $6, $7, $8, NOW())")
            .bind(id)
            .bind(request.project_id)
            .bind(request.environment_id)
            .bind(request.scenario_id)
            .bind(&request.target_origin)
            .bind(serde_json::to_value(&request.workload).expect("workload serializes"))
            .bind(serde_json::to_value(&request.scenario).expect("scenario serializes"))
            .bind(scenario_hash)
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    pub async fn update_status(
        &self,
        run_id: uuid::Uuid,
        status: &str,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE runs SET status = $1, started_at = CASE WHEN $1 = 'running' THEN NOW() ELSE started_at END, finished_at = CASE WHEN $1 IN ('completed', 'failed', 'canceled', 'aborted') THEN NOW() ELSE finished_at END WHERE id = $2")
            .bind(status)
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn acquire_queued_run(&self, run_id: uuid::Uuid) -> Result<bool, StorageError> {
        let result = sqlx::query("UPDATE runs SET status = 'running', started_at = NOW(), execution_attempts = execution_attempts + 1, failure_reason = NULL WHERE id = $1 AND status = 'queued'")
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn fail_run(&self, run_id: uuid::Uuid, reason: &str) -> Result<(), StorageError> {
        sqlx::query("UPDATE runs SET status = 'failed', failure_reason = $1, finished_at = NOW() WHERE id = $2")
            .bind(reason)
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn recover_stale_runs(&self, timeout_seconds: i64) -> Result<u64, StorageError> {
        let result = sqlx::query("UPDATE runs SET status = 'failed', failure_reason = 'runner lease expired', finished_at = NOW() WHERE status = 'running' AND started_at < NOW() - make_interval(secs => $1)")
            .bind(timeout_seconds as f64)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn save_report(
        &self,
        run_id: uuid::Uuid,
        status: &str,
        report: Option<&serde_json::Value>,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE runs SET status = $1, report = $2, failure_reason = NULL, finished_at = NOW() WHERE id = $3")
            .bind(status)
            .bind(report)
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
