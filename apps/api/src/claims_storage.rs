use crate::{
    claims::Claim,
    results::ResultDocument,
    storage::{PgRepository, StorageError},
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct StoredResult {
    pub id: Uuid,
    pub result_set_id: Uuid,
    pub claim_id: String,
    pub document: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl PgRepository {
    pub async fn sync_claims(&self, claims: &[Claim]) -> Result<(), StorageError> {
        let mut transaction = self.pool.begin().await?;
        for claim in claims {
            let definition = serde_json::to_value(claim)
                .map_err(|error| StorageError::Configuration(error.to_string()))?;
            sqlx::query("INSERT INTO claims (id, project, definition, loaded_at) VALUES ($1, $2, $3, NOW()) ON CONFLICT (id) DO UPDATE SET project = EXCLUDED.project, definition = EXCLUDED.definition, loaded_at = NOW()")
                .bind(&claim.id)
                .bind(&claim.project)
                .bind(definition)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn list_claims(&self) -> Result<Vec<Claim>, StorageError> {
        let definitions: Vec<serde_json::Value> =
            sqlx::query_scalar("SELECT definition FROM claims ORDER BY project, id")
                .fetch_all(&self.pool)
                .await?;
        definitions
            .into_iter()
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|error| StorageError::Configuration(error.to_string()))
            })
            .collect()
    }

    pub async fn get_claim(&self, id: &str) -> Result<Option<Claim>, StorageError> {
        let definition: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT definition FROM claims WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        definition
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|error| StorageError::Configuration(error.to_string()))
            })
            .transpose()
    }

    pub async fn insert_result(
        &self,
        result_set_id: Uuid,
        result: &ResultDocument,
    ) -> Result<Uuid, StorageError> {
        let id = Uuid::new_v4();
        let started_at = DateTime::parse_from_rfc3339(&result.started_at)
            .map_err(|error| StorageError::Configuration(error.to_string()))?
            .with_timezone(&Utc);
        let document = serde_json::to_value(result)
            .map_err(|error| StorageError::Configuration(error.to_string()))?;
        let inserted: Option<Uuid> = sqlx::query_scalar("INSERT INTO bench_results (id, result_set_id, claim_id, project, variant, workload, repetition, started_at, document) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (result_set_id, claim_id, variant, workload, repetition) DO NOTHING RETURNING id")
            .bind(id)
            .bind(result_set_id)
            .bind(&result.claim_id)
            .bind(&result.project)
            .bind(&result.variant)
            .bind(&result.workload)
            .bind(i32::try_from(result.repetition).map_err(|error| StorageError::Configuration(error.to_string()))?)
            .bind(started_at)
            .bind(&document)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(id) = inserted {
            return Ok(id);
        }
        let existing: (Uuid, serde_json::Value) = sqlx::query_as("SELECT id, document FROM bench_results WHERE result_set_id = $1 AND claim_id = $2 AND variant = $3 AND workload = $4 AND repetition = $5")
            .bind(result_set_id)
            .bind(&result.claim_id)
            .bind(&result.variant)
            .bind(&result.workload)
            .bind(i32::try_from(result.repetition).map_err(|error| StorageError::Configuration(error.to_string()))?)
            .fetch_one(&self.pool)
            .await?;
        if existing.1 != document {
            return Err(StorageError::Configuration(
                "result set already contains a different value for this repetition".into(),
            ));
        }
        Ok(existing.0)
    }

    pub async fn list_claim_results(
        &self,
        claim_id: &str,
    ) -> Result<Vec<StoredResult>, StorageError> {
        Ok(sqlx::query_as::<_, StoredResult>("SELECT id, result_set_id, claim_id, document, created_at FROM bench_results WHERE claim_id = $1 ORDER BY created_at DESC, id DESC")
            .bind(claim_id)
            .fetch_all(&self.pool)
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::{ResultEnvironment, ResultMetric};
    use std::collections::BTreeMap;

    #[sqlx::test(migrations = "../../migrations")]
    async fn reingesting_identical_result_keeps_one_row(pool: sqlx::PgPool) {
        let repository = PgRepository::from_pool(pool);
        let claim = crate::claims::bundled_claims()
            .expect("ledger")
            .into_iter()
            .find(|claim| claim.id == "canvassync-redis-cold-start-latency")
            .expect("claim");
        repository.sync_claims(&[claim]).await.expect("sync claim");
        let result = ResultDocument {
            schema_version: "1".into(),
            claim_id: "canvassync-redis-cold-start-latency".into(),
            project: "canvassync".into(),
            variant: "baseline".into(),
            workload: "cold-start-100-shapes".into(),
            repetition: 1,
            started_at: "2026-11-17T10:00:00Z".into(),
            duration_seconds: 0.01,
            metrics: vec![ResultMetric {
                name: "join_sync_latency_ms".into(),
                unit: "milliseconds".into(),
                value: 10.0,
            }],
            environment: ResultEnvironment {
                commit: "abc123".into(),
                images: BTreeMap::from([("ws".into(), format!("sha256:{}", "a".repeat(64)))]),
                host_cpu: "Apple M5".into(),
                sut_cpuset: "0-1".into(),
                sut_memory_mb: 384,
                generator_cpu_peak_percent: 40.0,
                docker: "29.9.3".into(),
                os: "Darwin".into(),
            },
            limitations: vec![],
        };
        let set = Uuid::new_v4();
        let first = repository
            .insert_result(set, &result)
            .await
            .expect("first insert");
        let again = repository.insert_result(set, &result).await.expect("retry");
        assert_eq!(first, again);
        assert_eq!(
            repository
                .list_claim_results(&result.claim_id)
                .await
                .expect("results")
                .len(),
            1
        );
    }
}
