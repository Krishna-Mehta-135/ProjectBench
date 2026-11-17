use crate::storage::{PgRepository, StorageError};
use crate::*;

impl PgRepository {
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

    pub async fn update_project(
        &self,
        project_id: uuid::Uuid,
        name: &str,
        description: &str,
    ) -> Result<Option<ProjectDetail>, StorageError> {
        let result = sqlx::query(
            "UPDATE projects SET name = $1, description = $2, updated_at = NOW() WHERE id = $3",
        )
        .bind(name.trim())
        .bind(description.trim())
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.get_project(project_id).await
    }

    pub async fn export_project(
        &self,
        project_id: uuid::Uuid,
    ) -> Result<Option<crate::RegistryExport>, StorageError> {
        let project = match self.get_project(project_id).await? {
            Some(project) => project,
            None => return Ok(None),
        };
        let environments = self.list_environments(project_id).await?;
        let scenarios = self.list_scenarios(project_id).await?;
        Ok(Some(crate::RegistryExport {
            schema_version: "1".into(),
            project: crate::ProjectConfig {
                name: project.name,
                description: project.description,
            },
            environments: environments
                .into_iter()
                .map(|environment| crate::EnvironmentConfig {
                    name: environment.name,
                    base_url: environment.base_url,
                    allowed_origins: serde_json::from_value(environment.allowed_origins)
                        .unwrap_or_default(),
                    enabled: environment.enabled,
                })
                .collect(),
            scenarios: scenarios
                .into_iter()
                .map(|scenario| crate::ScenarioConfig {
                    name: scenario.name,
                    version: scenario.version,
                    script_path: scenario.script_path,
                    config_snapshot: scenario.config_snapshot,
                    config_sha256: scenario.config_sha256,
                    secret_refs: serde_json::from_value(scenario.secret_refs).unwrap_or_default(),
                })
                .collect(),
        }))
    }

    pub async fn import_project(
        &self,
        export: &crate::RegistryExport,
    ) -> Result<crate::RegistryExport, StorageError> {
        let mut transaction = self.pool.begin().await?;
        let project_id = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO projects (id, name, description, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())")
            .bind(project_id)
            .bind(export.project.name.trim())
            .bind(export.project.description.trim())
            .execute(&mut *transaction)
            .await?;
        for environment in &export.environments {
            sqlx::query("INSERT INTO environments (id, project_id, name, base_url, allowed_origins, enabled, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())")
                .bind(uuid::Uuid::new_v4())
                .bind(project_id)
                .bind(environment.name.trim())
                .bind(&environment.base_url)
                .bind(serde_json::to_value(&environment.allowed_origins).expect("origins serialize"))
                .bind(environment.enabled)
                .execute(&mut *transaction)
                .await?;
        }
        for scenario in &export.scenarios {
            sqlx::query("INSERT INTO scenarios (id, project_id, name, version, script_path, config_snapshot, config_sha256, secret_refs, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())")
                .bind(uuid::Uuid::new_v4())
                .bind(project_id)
                .bind(scenario.name.trim())
                .bind(scenario.version.trim())
                .bind(&scenario.script_path)
                .bind(&scenario.config_snapshot)
                .bind(&scenario.config_sha256)
                .bind(serde_json::to_value(&scenario.secret_refs).expect("secret refs serialize"))
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        self.export_project(project_id).await?.ok_or_else(|| {
            StorageError::Configuration("imported project could not be loaded".into())
        })
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

    pub async fn set_environment_enabled(
        &self,
        project_id: uuid::Uuid,
        environment_id: uuid::Uuid,
        enabled: bool,
    ) -> Result<Option<EnvironmentSummary>, StorageError> {
        let result = sqlx::query("UPDATE environments SET enabled = $1, updated_at = NOW() WHERE id = $2 AND project_id = $3")
            .bind(enabled)
            .bind(environment_id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.get_environment(environment_id).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_scenario(
        &self,
        project_id: uuid::Uuid,
        name: &str,
        version: &str,
        script_path: &str,
        config_snapshot: &serde_json::Value,
        config_sha256: &str,
        secret_refs: &[String],
    ) -> Result<ScenarioSummary, StorageError> {
        let id = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO scenarios (id, project_id, name, version, script_path, config_snapshot, config_sha256, secret_refs, created_at) SELECT $1, $2, $3, $4, $5, $6, $7, $8, NOW() WHERE EXISTS (SELECT 1 FROM projects WHERE id = $2)")
            .bind(id).bind(project_id).bind(name.trim()).bind(version.trim()).bind(script_path).bind(config_snapshot).bind(config_sha256).bind(serde_json::to_value(secret_refs).expect("secret refs serialize")).execute(&self.pool).await?;
        self.get_scenario(id)
            .await?
            .ok_or_else(|| StorageError::Configuration("project does not exist".into()))
    }

    async fn get_scenario(&self, id: uuid::Uuid) -> Result<Option<ScenarioSummary>, StorageError> {
        Ok(sqlx::query_as::<_, ScenarioSummary>("SELECT id, project_id, name, version, script_path, config_snapshot, config_sha256, secret_refs FROM scenarios WHERE id = $1").bind(id).fetch_optional(&self.pool).await?)
    }

    pub async fn list_scenarios(
        &self,
        project_id: uuid::Uuid,
    ) -> Result<Vec<ScenarioSummary>, StorageError> {
        Ok(sqlx::query_as::<_, ScenarioSummary>("SELECT id, project_id, name, version, script_path, config_snapshot, config_sha256, secret_refs FROM scenarios WHERE project_id = $1 ORDER BY name, version").bind(project_id).fetch_all(&self.pool).await?)
    }
}
