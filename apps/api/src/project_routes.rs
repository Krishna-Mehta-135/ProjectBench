use crate::*;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use url::Url;
use uuid::Uuid;

pub(super) async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProjectSummary>>, ApiError> {
    match state.repository {
        Some(repository) => Ok(Json(repository.list_projects().await?)),
        None => Ok(Json(Vec::new())),
    }
}

pub(super) async fn create_project(
    State(state): State<AppState>,
    Json(request): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectDetail>), ApiError> {
    validate_name(&request.name)?;
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let project = repository
        .create_project(&request.name, &request.description)
        .await?;
    repository
        .record_audit_event(
            "project.created",
            "project",
            Some(project.id),
            &serde_json::json!({}),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(project)))
}

pub(super) async fn get_project(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ProjectDetail>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    repository
        .get_project(project_id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

pub(super) async fn update_project(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectDetail>, ApiError> {
    validate_name(&request.name)?;
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let project = repository
        .update_project(project_id, &request.name, &request.description)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)?;
    repository
        .record_audit_event(
            "project.updated",
            "project",
            Some(project.0.id),
            &serde_json::json!({}),
        )
        .await?;
    Ok(project)
}

pub(super) async fn export_project(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<RegistryExport>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    Ok(Json(
        repository
            .export_project(project_id)
            .await?
            .ok_or(ApiError::NotFound)?,
    ))
}

pub(super) async fn import_project(
    State(state): State<AppState>,
    Json(request): Json<RegistryExport>,
) -> Result<(StatusCode, Json<RegistryExport>), ApiError> {
    if request.schema_version != "1" {
        return Err(ApiError::Execution(
            "unsupported registry schema version".into(),
        ));
    }
    let mut normalized_request = request;
    validate_name(&normalized_request.project.name)?;
    for environment in &mut normalized_request.environments {
        validate_name(&environment.name)?;
        environment.base_url = normalize_origin(&environment.base_url)?;
        environment.allowed_origins = environment
            .allowed_origins
            .iter()
            .map(|origin| normalize_origin(origin))
            .collect::<Result<Vec<_>, _>>()?;
        if environment.allowed_origins.is_empty()
            || !environment
                .allowed_origins
                .iter()
                .any(|origin| origin == &environment.base_url)
        {
            return Err(ApiError::Policy(PolicyError::TargetNotAllowlisted));
        }
    }
    for scenario in &normalized_request.scenarios {
        validate_name(&scenario.name)?;
        validate_secret_refs(&scenario.secret_refs)?;
        if scenario.version.trim().is_empty()
            || scenario.script_path.trim().is_empty()
            || scenario.script_path.contains("..")
            || scenario.script_path.starts_with('/')
            || contains_secret_key(&scenario.config_snapshot)
        {
            return Err(ApiError::Execution(
                "scenario contains an invalid path or secret-like configuration".into(),
            ));
        }
        let expected_hash = format!(
            "sha256:{:x}",
            Sha256::digest(
                serde_json::to_vec(&scenario.config_snapshot).expect("config serializes")
            )
        );
        if expected_hash != scenario.config_sha256 {
            return Err(ApiError::Execution(
                "scenario configuration hash does not match".into(),
            ));
        }
    }
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let schema_version = normalized_request.schema_version.clone();
    let imported = repository.import_project(&normalized_request).await?;
    repository
        .record_audit_event(
            "project.imported",
            "project",
            None,
            &serde_json::json!({"schema_version": schema_version}),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(imported)))
}

fn contains_secret_key(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => map.iter().any(|(key, child)| {
            let key = key.to_ascii_lowercase();
            ["secret", "password", "token", "cookie", "credential"]
                .iter()
                .any(|term| key.contains(term))
                || contains_secret_key(child)
        }),
        serde_json::Value::Array(values) => values.iter().any(contains_secret_key),
        _ => false,
    }
}

fn validate_secret_refs(secret_refs: &[String]) -> Result<(), ApiError> {
    let mut unique = HashSet::new();
    for secret_ref in secret_refs {
        if secret_ref.is_empty()
            || secret_ref.len() > 64
            || !secret_ref
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
            || !unique.insert(secret_ref)
        {
            return Err(ApiError::Execution(
                "secret references must be unique uppercase environment names".into(),
            ));
        }
    }
    Ok(())
}

pub(super) async fn list_environments(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<EnvironmentSummary>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    Ok(Json(repository.list_environments(project_id).await?))
}

pub(super) async fn create_environment(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateEnvironmentRequest>,
) -> Result<(StatusCode, Json<EnvironmentSummary>), ApiError> {
    validate_name(&request.name)?;
    let base_url = normalize_origin(&request.base_url)?;
    if request.allowed_origins.is_empty() {
        return Err(ApiError::Policy(PolicyError::EmptyScenario));
    }
    let mut allowed = Vec::with_capacity(request.allowed_origins.len());
    for origin in request.allowed_origins {
        allowed.push(normalize_origin(&origin)?);
    }
    if !allowed.iter().any(|origin| origin == &base_url) {
        return Err(ApiError::Policy(PolicyError::TargetNotAllowlisted));
    }
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let environment = repository
        .create_environment(project_id, &request.name, &base_url, &allowed)
        .await?;
    repository
        .record_audit_event(
            "environment.created",
            "environment",
            Some(environment.id),
            &serde_json::json!({"enabled": environment.enabled}),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(environment)))
}

pub(super) async fn update_environment(
    State(state): State<AppState>,
    Path((project_id, environment_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateEnvironmentRequest>,
) -> Result<Json<EnvironmentSummary>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let environment = repository
        .set_environment_enabled(project_id, environment_id, request.enabled)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)?;
    repository
        .record_audit_event(
            "environment.updated",
            "environment",
            Some(environment.0.id),
            &serde_json::json!({"enabled": environment.0.enabled}),
        )
        .await?;
    Ok(environment)
}

pub(super) async fn list_scenarios(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<ScenarioSummary>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    Ok(Json(repository.list_scenarios(project_id).await?))
}

pub(super) async fn get_baseline(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<BaselineSummary>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    repository
        .get_baseline(project_id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

pub(super) async fn select_baseline(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<SelectBaselineRequest>,
) -> Result<Json<BaselineSummary>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    Ok(Json(
        repository
            .select_baseline(
                project_id,
                request.environment_id,
                request.scenario_id,
                request.run_id,
                &request.selected_by,
            )
            .await?,
    ))
}

pub(super) async fn create_scenario(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateScenarioRequest>,
) -> Result<(StatusCode, Json<ScenarioSummary>), ApiError> {
    validate_name(&request.name)?;
    if request.version.trim().is_empty()
        || request.script_path.trim().is_empty()
        || request.script_path.contains("..")
        || request.script_path.starts_with('/')
    {
        return Err(ApiError::Policy(PolicyError::EmptyScenario));
    }
    validate_secret_refs(&request.secret_refs)?;
    let config_sha256 = format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&request.config_snapshot).expect("config serializes"))
    );
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let scenario = repository
        .create_scenario(
            project_id,
            &request.name,
            &request.version,
            &request.script_path,
            &request.config_snapshot,
            &config_sha256,
            &request.secret_refs,
        )
        .await?;
    repository
        .record_audit_event(
            "scenario.created",
            "scenario",
            Some(scenario.id),
            &serde_json::json!({}),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(scenario)))
}

fn validate_name(name: &str) -> Result<(), ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 120 {
        return Err(ApiError::Policy(PolicyError::EmptyScenario));
    }
    Ok(())
}

fn normalize_origin(value: &str) -> Result<String, ApiError> {
    let parsed = Url::parse(value).map_err(|_| ApiError::Policy(PolicyError::InvalidOrigin))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || parsed.path() != "/"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(ApiError::Policy(PolicyError::InvalidOrigin));
    }
    Ok(parsed.origin().ascii_serialization())
}

#[cfg(test)]
mod registry_config_tests {
    use super::contains_secret_key;
    use serde_json::json;

    #[test]
    fn rejects_secret_like_configuration_keys() {
        assert!(contains_secret_key(
            &json!({"credentials": {"password": "redacted"}})
        ));
        assert!(contains_secret_key(&json!({"api_token": "redacted"})));
        assert!(!contains_secret_key(
            &json!({"headers": ["accept", "content-type"]})
        ));
    }
}
