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

    pub async fn list_projects(&self) -> Result<Vec<ProjectSummary>, StorageError> {
        Ok(sqlx::query_as::<_, ProjectSummary>("SELECT id, name, '' AS environment, NULL::uuid AS latest_run_id FROM projects ORDER BY name").fetch_all(&self.pool).await?)
    }

    pub async fn list_runs(&self) -> Result<Vec<RunSummary>, StorageError> {
        Ok(sqlx::query_as::<_, RunSummary>("SELECT id, status, TRUE AS synthetic_traffic, (report IS NOT NULL) AS report_available, created_at, started_at, finished_at, failure_reason FROM runs ORDER BY created_at DESC").fetch_all(&self.pool).await?)
    }

    pub async fn get_run(
        &self,
        run_id: uuid::Uuid,
    ) -> Result<Option<crate::RunDetail>, StorageError> {
        Ok(sqlx::query_as::<_, crate::RunDetail>("SELECT id, status, target_origin, workload, scenario_snapshot, report, created_at, started_at, finished_at, failure_reason FROM runs WHERE id = $1").bind(run_id).fetch_optional(&self.pool).await?)
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    async fn seed_run(pool: &PgPool, status: &str, started_at: Option<&str>) -> uuid::Uuid {
        let project_id = uuid::Uuid::new_v4();
        let environment_id = uuid::Uuid::new_v4();
        let scenario_id = uuid::Uuid::new_v4();
        let run_id = uuid::Uuid::new_v4();
        let now = Utc::now();
        sqlx::query("INSERT INTO projects (id, name, created_at, updated_at) VALUES ($1, 'integration project', $2, $2)")
            .bind(project_id)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO environments (id, project_id, name, base_url, allowed_origins, created_at, updated_at) VALUES ($1, $2, 'local', 'https://example.test', '[]', $3, $3)")
            .bind(environment_id)
            .bind(project_id)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO scenarios (id, project_id, name, version, script_path, config_snapshot, config_sha256, created_at) VALUES ($1, $2, 'integration', '1', 'scenario.js', '{}', 'sha256:test', $3)")
            .bind(scenario_id)
            .bind(project_id)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        let started = started_at.unwrap_or("NOW()");
        let query = format!("INSERT INTO runs (id, project_id, environment_id, scenario_id, status, target_origin, workload, scenario_snapshot, scenario_sha256, started_at, created_at) VALUES ($1, $2, $3, $4, $5, 'https://example.test', '{{}}', '{{}}', 'sha256:test', {}, NOW())", started);
        sqlx::query(&query)
            .bind(run_id)
            .bind(project_id)
            .bind(environment_id)
            .bind(scenario_id)
            .bind(status)
            .execute(pool)
            .await
            .unwrap();
        run_id
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn queued_acquisition_has_one_winner(pool: PgPool) {
        let run_id = seed_run(&pool, "queued", None).await;
        let repository = PgRepository::from_pool(pool.clone());
        let first = repository.clone();
        let second = repository.clone();
        let (left, right) = tokio::join!(
            first.acquire_queued_run(run_id),
            second.acquire_queued_run(run_id)
        );
        assert_eq!(left.unwrap() as u8 + right.unwrap() as u8, 1);
        let status: String = sqlx::query_scalar("SELECT status FROM runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "running");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn stale_running_run_is_retained_as_failed(pool: PgPool) {
        let run_id = seed_run(&pool, "running", Some("NOW() - INTERVAL '10 minutes'")).await;
        let repository = PgRepository::from_pool(pool.clone());
        assert_eq!(repository.recover_stale_runs(60).await.unwrap(), 1);
        let row: (String, String) =
            sqlx::query_as("SELECT status, failure_reason FROM runs WHERE id = $1")
                .bind(run_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row, ("failed".into(), "runner lease expired".into()));
    }
}
