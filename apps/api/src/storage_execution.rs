use crate::storage::{PgRepository, StorageError};

#[derive(Debug, sqlx::FromRow)]
struct DbExecutionConfiguration {
    project_id: uuid::Uuid,
    environment_id: uuid::Uuid,
    scenario_id: uuid::Uuid,
    target_origin: String,
    workload: serde_json::Value,
    scenario_snapshot: serde_json::Value,
    scenario_sha256: String,
    environment_project_id: uuid::Uuid,
    environment_enabled: bool,
    allowed_origins: serde_json::Value,
    scenario_project_id: uuid::Uuid,
    scenario_name: String,
    scenario_version: String,
    scenario_script_path: String,
    scenario_secret_refs: serde_json::Value,
}

impl PgRepository {
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
        let row = sqlx::query_as::<_, (uuid::Uuid, bool, serde_json::Value, uuid::Uuid, String, String, String, serde_json::Value)>(
            "SELECT e.project_id, e.enabled, e.allowed_origins, s.project_id, s.name, s.version, s.script_path, s.secret_refs
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
        let persisted_secret_refs: Vec<String> =
            serde_json::from_value(row.7).map_err(|error| {
                StorageError::Configuration(format!(
                    "scenario secret references are invalid: {error}"
                ))
            })?;
        if scenario.name != row.4
            || scenario.version != row.5
            || scenario.script_path != row.6
            || scenario.secret_refs != persisted_secret_refs
        {
            return Err(StorageError::Configuration(
                "scenario snapshot does not match the persisted scenario version".into(),
            ));
        }
        Ok(())
    }

    pub async fn validate_execution_configuration(
        &self,
        run_id: uuid::Uuid,
    ) -> Result<(), StorageError> {
        let row = sqlx::query_as::<_, DbExecutionConfiguration>(
            "SELECT r.project_id, r.environment_id, r.scenario_id, r.target_origin,
                    r.workload, r.scenario_snapshot, r.scenario_sha256,
                    e.project_id AS environment_project_id, e.enabled AS environment_enabled,
                    e.allowed_origins, s.project_id AS scenario_project_id,
                    s.name AS scenario_name, s.version AS scenario_version,
                    s.script_path AS scenario_script_path, s.secret_refs AS scenario_secret_refs
             FROM runs r
             JOIN environments e ON e.id = r.environment_id
             JOIN scenarios s ON s.id = r.scenario_id
             WHERE r.id = $1 AND r.status = 'queued'",
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            StorageError::Configuration("queued run configuration is unavailable".into())
        })?;

        if row.project_id != row.environment_project_id
            || row.project_id != row.scenario_project_id
            || row.environment_id.is_nil()
            || row.scenario_id.is_nil()
        {
            return Err(StorageError::Configuration(
                "queued run ownership no longer matches its registry records".into(),
            ));
        }
        if !row.environment_enabled {
            return Err(StorageError::Configuration(
                "environment is disabled".into(),
            ));
        }
        let origins: Vec<String> =
            serde_json::from_value(row.allowed_origins).map_err(|error| {
                StorageError::Configuration(format!("environment allowlist is invalid: {error}"))
            })?;
        if !origins.iter().any(|origin| origin == &row.target_origin) {
            return Err(StorageError::Configuration(
                "queued run target origin is no longer approved".into(),
            ));
        }
        let scenario: crate::ScenarioSnapshot = serde_json::from_value(row.scenario_snapshot)
            .map_err(|error| {
                StorageError::Configuration(format!("queued scenario snapshot is invalid: {error}"))
            })?;
        if scenario.name != row.scenario_name
            || scenario.version != row.scenario_version
            || scenario.script_path != row.scenario_script_path
            || scenario.secret_refs
                != serde_json::from_value::<Vec<String>>(row.scenario_secret_refs).map_err(
                    |error| {
                        StorageError::Configuration(format!(
                            "scenario secret references are invalid: {error}"
                        ))
                    },
                )?
            || crate::scenario_hash(&scenario) != row.scenario_sha256
        {
            return Err(StorageError::Configuration(
                "queued scenario snapshot no longer matches the approved version".into(),
            ));
        }
        serde_json::from_value::<crate::Workload>(row.workload).map_err(|error| {
            StorageError::Configuration(format!("queued workload is invalid: {error}"))
        })?;
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
