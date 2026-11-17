use crate::storage::{PgRepository, StorageError};
use crate::*;
use crate::timeseries::TimeBucket;

#[derive(sqlx::FromRow)]
struct DbTimeBucket {
    start_at: chrono::DateTime<chrono::Utc>,
    request_count: i64,
    failed_count: i64,
    duration_count: i64,
    duration_sum_ms: f64,
    duration_max_ms: Option<f64>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct CompareRunRecord {
    pub project_id: uuid::Uuid,
    pub environment_id: uuid::Uuid,
    pub scenario_id: uuid::Uuid,
    pub target_origin: String,
    pub workload: serde_json::Value,
    pub scenario_sha256: String,
    pub status: String,
    pub report: Option<serde_json::Value>,
}

impl PgRepository {
    pub async fn list_runs(
        &self,
        project_id: Option<uuid::Uuid>,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RunSummary>, StorageError> {
        Ok(sqlx::query_as::<_, RunSummary>("SELECT id, project_id, environment_id, scenario_id, status, TRUE AS synthetic_traffic, (report IS NOT NULL) AS report_available, created_at, started_at, finished_at, failure_reason FROM runs WHERE ($1::uuid IS NULL OR project_id = $1) AND ($2::text IS NULL OR status = $2) ORDER BY created_at DESC, id DESC LIMIT $3 OFFSET $4")
            .bind(project_id)
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn get_run(
        &self,
        run_id: uuid::Uuid,
    ) -> Result<Option<crate::RunDetail>, StorageError> {
        Ok(sqlx::query_as::<_, crate::RunDetail>("SELECT id, status, target_origin, workload, scenario_snapshot, report, created_at, started_at, finished_at, failure_reason FROM runs WHERE id = $1").bind(run_id).fetch_optional(&self.pool).await?)
    }

    pub async fn get_compare_runs(
        &self,
        baseline_id: uuid::Uuid,
        candidate_id: uuid::Uuid,
    ) -> Result<Option<(CompareRunRecord, CompareRunRecord)>, StorageError> {
        let query = "SELECT project_id, environment_id, scenario_id, target_origin, workload, scenario_sha256, status, report FROM runs WHERE id = $1";
        let baseline = sqlx::query_as::<_, CompareRunRecord>(query)
            .bind(baseline_id)
            .fetch_optional(&self.pool)
            .await?;
        let candidate = sqlx::query_as::<_, CompareRunRecord>(query)
            .bind(candidate_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(baseline.zip(candidate))
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

    pub async fn get_baseline(
        &self,
        project_id: uuid::Uuid,
    ) -> Result<Option<crate::BaselineSummary>, StorageError> {
        Ok(sqlx::query_as::<_, crate::BaselineSummary>("SELECT project_id, environment_id, scenario_id, run_id, selected_at, selected_by FROM baselines WHERE project_id = $1")
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn select_baseline(
        &self,
        project_id: uuid::Uuid,
        environment_id: uuid::Uuid,
        scenario_id: uuid::Uuid,
        run_id: uuid::Uuid,
        selected_by: &str,
    ) -> Result<crate::BaselineSummary, StorageError> {
        let run = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, uuid::Uuid, String, bool)>(
            "SELECT project_id, environment_id, scenario_id, status, report IS NOT NULL FROM runs WHERE id = $1",
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::Configuration("run does not exist".into()))?;
        if run.0 != project_id || run.1 != environment_id || run.2 != scenario_id {
            return Err(StorageError::Configuration(
                "run identity does not match the requested baseline scope".into(),
            ));
        }
        if run.3 != "completed" || !run.4 {
            return Err(StorageError::Configuration(
                "only completed runs with reports can become baselines".into(),
            ));
        }
        let mut transaction = self.pool.begin().await?;
        let now = chrono::Utc::now();
        sqlx::query("INSERT INTO baseline_history (id, project_id, environment_id, scenario_id, run_id, selected_at, selected_by) VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(uuid::Uuid::new_v4())
            .bind(project_id)
            .bind(environment_id)
            .bind(scenario_id)
            .bind(run_id)
            .bind(now)
            .bind(selected_by)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("INSERT INTO baselines (project_id, environment_id, scenario_id, run_id, selected_at, selected_by) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (project_id, environment_id, scenario_id) DO UPDATE SET run_id = EXCLUDED.run_id, selected_at = EXCLUDED.selected_at, selected_by = EXCLUDED.selected_by")
            .bind(project_id)
            .bind(environment_id)
            .bind(scenario_id)
            .bind(run_id)
            .bind(now)
            .bind(selected_by)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        self.get_baseline(project_id).await?.ok_or_else(|| {
            StorageError::Configuration("selected baseline could not be loaded".into())
        })
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
}
