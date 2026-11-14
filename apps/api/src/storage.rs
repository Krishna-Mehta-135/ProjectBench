use crate::{
    timeseries::TimeBucket, EnvironmentSummary, ProjectDetail, ProjectSummary, RunSummary,
    ScenarioSummary,
};
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
    pool: PgPool,
}

#[derive(sqlx::FromRow)]
struct DbTimeBucket {
    start_at: chrono::DateTime<chrono::Utc>,
    request_count: i64,
    failed_count: i64,
    duration_count: i64,
    duration_sum_ms: f64,
    duration_max_ms: Option<f64>,
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
        Ok(sqlx::query_as::<_, ProjectSummary>("SELECT p.id, p.name, COALESCE((SELECT e.name FROM environments e WHERE e.project_id = p.id AND e.enabled ORDER BY e.name LIMIT 1), '') AS environment, (SELECT r.id FROM runs r WHERE r.project_id = p.id ORDER BY r.created_at DESC LIMIT 1) AS latest_run_id FROM projects p ORDER BY p.name").fetch_all(&self.pool).await?)
    }

    pub async fn create_project(
        &self,
        name: &str,
        description: &str,
    ) -> Result<ProjectDetail, StorageError> {
        let id = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO projects (id, name, description, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())")
            .bind(id).bind(name.trim()).bind(description.trim()).execute(&self.pool).await?;
        self.get_project(id).await?.ok_or_else(|| {
            StorageError::Configuration("created project could not be loaded".into())
        })
    }

    pub async fn get_project(&self, id: uuid::Uuid) -> Result<Option<ProjectDetail>, StorageError> {
        Ok(sqlx::query_as::<_, ProjectDetail>(
            "SELECT id, name, description, created_at, updated_at FROM projects WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn create_environment(
        &self,
        project_id: uuid::Uuid,
        name: &str,
        base_url: &str,
        allowed_origins: &[String],
    ) -> Result<EnvironmentSummary, StorageError> {
        let id = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO environments (id, project_id, name, base_url, allowed_origins, created_at, updated_at) SELECT $1, $2, $3, $4, $5, NOW(), NOW() WHERE EXISTS (SELECT 1 FROM projects WHERE id = $2)")
            .bind(id).bind(project_id).bind(name.trim()).bind(base_url).bind(serde_json::to_value(allowed_origins).expect("origins serialize")).execute(&self.pool).await?;
        self.get_environment(id)
            .await?
            .ok_or_else(|| StorageError::Configuration("project does not exist".into()))
    }

    async fn get_environment(
        &self,
        id: uuid::Uuid,
    ) -> Result<Option<EnvironmentSummary>, StorageError> {
        Ok(sqlx::query_as::<_, EnvironmentSummary>("SELECT id, project_id, name, base_url, allowed_origins, enabled FROM environments WHERE id = $1").bind(id).fetch_optional(&self.pool).await?)
    }

    pub async fn list_environments(
        &self,
        project_id: uuid::Uuid,
    ) -> Result<Vec<EnvironmentSummary>, StorageError> {
        Ok(sqlx::query_as::<_, EnvironmentSummary>("SELECT id, project_id, name, base_url, allowed_origins, enabled FROM environments WHERE project_id = $1 ORDER BY name").bind(project_id).fetch_all(&self.pool).await?)
    }

    pub async fn create_scenario(
        &self,
        project_id: uuid::Uuid,
        name: &str,
        version: &str,
        script_path: &str,
        config_snapshot: &serde_json::Value,
        config_sha256: &str,
    ) -> Result<ScenarioSummary, StorageError> {
        let id = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO scenarios (id, project_id, name, version, script_path, config_snapshot, config_sha256, created_at) SELECT $1, $2, $3, $4, $5, $6, $7, NOW() WHERE EXISTS (SELECT 1 FROM projects WHERE id = $2)")
            .bind(id).bind(project_id).bind(name.trim()).bind(version.trim()).bind(script_path).bind(config_snapshot).bind(config_sha256).execute(&self.pool).await?;
        self.get_scenario(id)
            .await?
            .ok_or_else(|| StorageError::Configuration("project does not exist".into()))
    }

    async fn get_scenario(&self, id: uuid::Uuid) -> Result<Option<ScenarioSummary>, StorageError> {
        Ok(sqlx::query_as::<_, ScenarioSummary>("SELECT id, project_id, name, version, script_path, config_snapshot, config_sha256 FROM scenarios WHERE id = $1").bind(id).fetch_optional(&self.pool).await?)
    }

    pub async fn list_scenarios(
        &self,
        project_id: uuid::Uuid,
    ) -> Result<Vec<ScenarioSummary>, StorageError> {
        Ok(sqlx::query_as::<_, ScenarioSummary>("SELECT id, project_id, name, version, script_path, config_snapshot, config_sha256 FROM scenarios WHERE project_id = $1 ORDER BY name, version").bind(project_id).fetch_all(&self.pool).await?)
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

    pub async fn register_artifact(
        &self,
        run_id: uuid::Uuid,
        kind: &str,
        path: &str,
        sha256: &str,
        size_bytes: i64,
        partial: bool,
    ) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO run_artifacts (run_id, kind, path, sha256, size_bytes, partial, created_at) VALUES ($1, $2, $3, $4, $5, $6, NOW()) ON CONFLICT (run_id, kind) DO UPDATE SET path = EXCLUDED.path, sha256 = EXCLUDED.sha256, size_bytes = EXCLUDED.size_bytes, partial = EXCLUDED.partial")
            .bind(run_id)
            .bind(kind)
            .bind(path)
            .bind(sha256)
            .bind(size_bytes)
            .bind(partial)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_artifacts(
        &self,
        run_id: uuid::Uuid,
    ) -> Result<Vec<crate::ArtifactRef>, StorageError> {
        Ok(sqlx::query_as::<_, crate::ArtifactRef>(
            "SELECT kind, path, sha256 FROM run_artifacts WHERE run_id = $1 ORDER BY kind",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn replace_time_buckets(
        &self,
        run_id: uuid::Uuid,
        buckets: &[TimeBucket],
    ) -> Result<(), StorageError> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("DELETE FROM run_time_buckets WHERE run_id = $1")
            .bind(run_id)
            .execute(&mut *transaction)
            .await?;
        for bucket in buckets {
            sqlx::query("INSERT INTO run_time_buckets (run_id, bucket_start, request_count, failed_count, duration_count, duration_sum_ms, duration_max_ms) VALUES ($1, $2, $3, $4, $5, $6, $7)")
                .bind(run_id)
                .bind(bucket.start_at)
                .bind(bucket.request_count as i64)
                .bind(bucket.failed_count as i64)
                .bind(bucket.duration_count as i64)
                .bind(bucket.duration_sum_ms)
                .bind(bucket.duration_max_ms)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn list_time_buckets(
        &self,
        run_id: uuid::Uuid,
    ) -> Result<Vec<TimeBucket>, StorageError> {
        let rows = sqlx::query_as::<_, DbTimeBucket>("SELECT bucket_start AS start_at, request_count, failed_count, duration_count, duration_sum_ms, duration_max_ms FROM run_time_buckets WHERE run_id = $1 ORDER BY bucket_start")
            .bind(run_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| TimeBucket {
                start_at: row.start_at,
                request_count: row.request_count.max(0) as u64,
                failed_count: row.failed_count.max(0) as u64,
                duration_count: row.duration_count.max(0) as u64,
                duration_sum_ms: row.duration_sum_ms,
                duration_max_ms: row.duration_max_ms,
            })
            .collect())
    }

    pub async fn get_artifact(
        &self,
        run_id: uuid::Uuid,
        kind: &str,
    ) -> Result<Option<crate::ArtifactRef>, StorageError> {
        Ok(sqlx::query_as::<_, crate::ArtifactRef>(
            "SELECT kind, path, sha256 FROM run_artifacts WHERE run_id = $1 AND kind = $2",
        )
        .bind(run_id)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn create_queued_run(
        &self,
        request: &crate::CreateRunRequest,
        target_origin: &str,
        scenario_hash: &str,
    ) -> Result<uuid::Uuid, StorageError> {
        let id = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO runs (id, project_id, environment_id, scenario_id, status, target_origin, workload, scenario_snapshot, scenario_sha256, created_at) VALUES ($1, $2, $3, $4, 'queued', $5, $6, $7, $8, NOW())")
            .bind(id)
            .bind(request.project_id)
            .bind(request.environment_id)
            .bind(request.scenario_id)
            .bind(target_origin)
            .bind(serde_json::to_value(&request.workload).expect("workload serializes"))
            .bind(serde_json::to_value(&request.scenario).expect("scenario serializes"))
            .bind(scenario_hash)
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    pub async fn validate_run_configuration(
        &self,
        request: &crate::CreateRunRequest,
        target_origin: &str,
    ) -> Result<(), StorageError> {
        let row = sqlx::query_as::<_, (uuid::Uuid, bool, serde_json::Value, uuid::Uuid, String, String, String)>(
            "SELECT e.project_id, e.enabled, e.allowed_origins, s.project_id, s.name, s.version, s.script_path
             FROM environments e JOIN scenarios s ON s.id = $2
             WHERE e.id = $1",
        )
        .bind(request.environment_id)
        .bind(request.scenario_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::Configuration("environment or scenario does not exist".into()))?;

        if row.0 != request.project_id || row.3 != request.project_id {
            return Err(StorageError::Configuration(
                "environment and scenario must belong to the project".into(),
            ));
        }
        if !row.1 {
            return Err(StorageError::Configuration(
                "environment is disabled".into(),
            ));
        }

        let origins: Vec<String> = serde_json::from_value(row.2).map_err(|error| {
            StorageError::Configuration(format!("environment allowlist is invalid: {error}"))
        })?;
        if !origins.iter().any(|origin| origin == target_origin) {
            return Err(StorageError::Configuration(
                "target origin is not approved for the environment".into(),
            ));
        }

        let scenario = &request.scenario;
        if scenario.name != row.4 || scenario.version != row.5 || scenario.script_path != row.6 {
            return Err(StorageError::Configuration(
                "scenario snapshot does not match the persisted scenario version".into(),
            ));
        }
        Ok(())
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

    pub async fn fail_run(&self, run_id: uuid::Uuid, reason: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("UPDATE runs SET status = 'failed', failure_reason = $1, finished_at = NOW() WHERE id = $2 AND status = 'running'")
            .bind(reason)
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn cancel_run(&self, run_id: uuid::Uuid) -> Result<bool, StorageError> {
        let result = sqlx::query("UPDATE runs SET status = 'canceled', failure_reason = 'canceled by operator', finished_at = NOW() WHERE id = $1 AND status IN ('queued', 'running')")
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
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
    ) -> Result<bool, StorageError> {
        let result = sqlx::query("UPDATE runs SET status = $1, report = $2, failure_reason = NULL, finished_at = NOW() WHERE id = $3 AND status = 'running'")
            .bind(status)
            .bind(report)
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
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
        sqlx::query("INSERT INTO environments (id, project_id, name, base_url, allowed_origins, created_at, updated_at) VALUES ($1, $2, 'local', 'https://example.test', '[\"https://example.test\"]', $3, $3)")
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn run_configuration_is_bound_to_enabled_environment_and_scenario(pool: PgPool) {
        let project_id = uuid::Uuid::new_v4();
        let environment_id = uuid::Uuid::new_v4();
        let scenario_id = uuid::Uuid::new_v4();
        let now = Utc::now();
        sqlx::query("INSERT INTO projects (id, name, created_at, updated_at) VALUES ($1, 'policy project', $2, $2)")
            .bind(project_id)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO environments (id, project_id, name, base_url, allowed_origins, created_at, updated_at) VALUES ($1, $2, 'local', 'https://example.test', '[\"https://example.test\"]', $3, $3)")
            .bind(environment_id)
            .bind(project_id)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO scenarios (id, project_id, name, version, script_path, config_snapshot, config_sha256, created_at) VALUES ($1, $2, 'read', '1', 'scenario.js', '{}', 'sha256:test', $3)")
            .bind(scenario_id)
            .bind(project_id)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();

        let repository = PgRepository::from_pool(pool.clone());
        let request = crate::CreateRunRequest {
            project_id,
            environment_id,
            scenario_id,
            target_origin: "https://example.test/".into(),
            scenario: crate::ScenarioSnapshot {
                name: "read".into(),
                version: "1".into(),
                script_path: "scenario.js".into(),
                routes: vec!["GET /health".into()],
            },
            workload: crate::Workload {
                vus: 1,
                duration_seconds: 1,
                think_time_seconds: 0,
            },
            confirmed: true,
        };
        repository
            .validate_run_configuration(&request, "https://example.test")
            .await
            .unwrap();

        let mut cross_project = request.clone();
        cross_project.project_id = uuid::Uuid::new_v4();
        assert!(matches!(
            repository
                .validate_run_configuration(&cross_project, "https://example.test")
                .await,
            Err(StorageError::Configuration(message)) if message.contains("belong")
        ));

        sqlx::query("UPDATE environments SET enabled = FALSE WHERE id = $1")
            .bind(environment_id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(matches!(
            repository
                .validate_run_configuration(&request, "https://example.test")
                .await,
            Err(StorageError::Configuration(message)) if message.contains("disabled")
        ));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn terminal_writes_cannot_overwrite_canceled_runs(pool: PgPool) {
        let run_id = seed_run(&pool, "running", None).await;
        let repository = PgRepository::from_pool(pool.clone());
        assert!(repository.cancel_run(run_id).await.unwrap());
        assert!(!repository
            .fail_run(run_id, "late process error")
            .await
            .unwrap());
        assert!(!repository
            .save_report(run_id, "completed", Some(&serde_json::json!({"ok": true})))
            .await
            .unwrap());
        let row: (String, Option<String>) =
            sqlx::query_as("SELECT status, failure_reason FROM runs WHERE id = $1")
                .bind(run_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            row,
            ("canceled".into(), Some("canceled by operator".into()))
        );
    }
}
