use crate::{
    claims::Claim, claims_storage::StoredResult, results::ResultDocument, ApiError, AppState,
};
use crate::{
    publish,
    verdict::{self, Verdict},
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct VerdictHistoryEntry {
    pub result_set_id: Uuid,
    pub verdict: Verdict,
}

#[derive(serde::Deserialize)]
pub struct PublishRequest {
    pub confirmed: bool,
    pub result_set_id: Uuid,
}

#[derive(Serialize)]
pub struct PublishedEvidence {
    pub claim_id: String,
    pub result_set_id: Uuid,
    pub path: String,
}

#[derive(Serialize)]
pub struct CreatedResult {
    id: Uuid,
    result_set_id: Uuid,
}

fn result_set_id(headers: &HeaderMap) -> Result<Uuid, ApiError> {
    headers
        .get("x-projectbench-result-set")
        .map(|value| {
            value
                .to_str()
                .ok()
                .and_then(|text| Uuid::parse_str(text).ok())
                .ok_or_else(|| ApiError::InvalidResult("invalid result set UUID header".into()))
        })
        .unwrap_or_else(|| Ok(Uuid::new_v4()))
}

pub async fn list_claims(State(state): State<AppState>) -> Result<Json<Vec<Claim>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    Ok(Json(repository.list_claims().await?))
}

pub async fn get_claim(
    State(state): State<AppState>,
    Path(claim_id): Path<String>,
) -> Result<Json<Claim>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    Ok(Json(
        repository
            .get_claim(&claim_id)
            .await?
            .ok_or(ApiError::NotFound)?,
    ))
}

pub async fn post_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(result): Json<ResultDocument>,
) -> Result<(StatusCode, Json<CreatedResult>), ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let claim = repository
        .get_claim(&result.claim_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    result
        .validate(&claim)
        .map_err(|error| ApiError::InvalidResult(error.to_string()))?;
    let result_set_id = result_set_id(&headers)?;
    let id = repository.insert_result(result_set_id, &result).await?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedResult { id, result_set_id }),
    ))
}

pub async fn list_results(
    State(state): State<AppState>,
    Path(claim_id): Path<String>,
) -> Result<Json<Vec<StoredResult>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    if repository.get_claim(&claim_id).await?.is_none() {
        return Err(ApiError::NotFound);
    }
    Ok(Json(repository.list_claim_results(&claim_id).await?))
}

fn group_result_sets(
    claim: &Claim,
    rows: Vec<StoredResult>,
) -> Result<Vec<VerdictHistoryEntry>, ApiError> {
    let mut groups: Vec<(Uuid, Vec<ResultDocument>)> = Vec::new();
    for row in rows {
        let result: ResultDocument = serde_json::from_value(row.document).map_err(|error| {
            ApiError::InvalidResult(format!("stored result is invalid: {error}"))
        })?;
        if let Some((_, results)) = groups.iter_mut().find(|(id, _)| *id == row.result_set_id) {
            results.push(result);
        } else {
            groups.push((row.result_set_id, vec![result]));
        }
    }
    Ok(groups
        .into_iter()
        .map(|(result_set_id, results)| VerdictHistoryEntry {
            result_set_id,
            verdict: verdict::evaluate(claim, &results),
        })
        .collect())
}

pub async fn list_verdicts(
    State(state): State<AppState>,
    Path(claim_id): Path<String>,
) -> Result<Json<Vec<VerdictHistoryEntry>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let claim = repository
        .get_claim(&claim_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let rows = repository.list_claim_results(&claim_id).await?;
    Ok(Json(group_result_sets(&claim, rows)?))
}

pub async fn get_verdict(
    State(state): State<AppState>,
    Path(claim_id): Path<String>,
) -> Result<Json<VerdictHistoryEntry>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let claim = repository
        .get_claim(&claim_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let rows = repository.list_claim_results(&claim_id).await?;
    group_result_sets(&claim, rows)?
        .into_iter()
        .next()
        .map(Json)
        .ok_or(ApiError::NotFound)
}

pub async fn publish_claim(
    State(state): State<AppState>,
    Path(claim_id): Path<String>,
    Json(request): Json<PublishRequest>,
) -> Result<Json<PublishedEvidence>, ApiError> {
    if !request.confirmed {
        return Err(ApiError::ConfirmationRequired);
    }
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let claim = repository
        .get_claim(&claim_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let rows = repository.list_claim_results(&claim_id).await?;
    let results = rows
        .into_iter()
        .filter(|row| row.result_set_id == request.result_set_id)
        .map(|row| {
            serde_json::from_value::<ResultDocument>(row.document).map_err(|error| {
                ApiError::InvalidResult(format!("stored result is invalid: {error}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if results.is_empty() {
        return Err(ApiError::NotFound);
    }
    let verdict = verdict::evaluate(&claim, &results);
    let root = std::env::var("PROJECTBENCH_EVIDENCE_DIR").unwrap_or_else(|_| "evidence".into());
    publish::write_evidence(std::path::Path::new(&root), &claim, &verdict, &results)?;
    Ok(Json(PublishedEvidence {
        claim_id: claim.id.clone(),
        result_set_id: request.result_set_id,
        path: format!("evidence/{}/{}.json", claim.project, claim.id),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn result_set_header_groups_a_folder_and_rejects_invalid_values() {
        let id = Uuid::new_v4();
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-projectbench-result-set",
            HeaderValue::from_str(&id.to_string()).expect("UUID header"),
        );
        assert_eq!(result_set_id(&headers).expect("valid UUID"), id);
        headers.insert(
            "x-projectbench-result-set",
            HeaderValue::from_static("invalid"),
        );
        assert!(result_set_id(&headers).is_err());
    }

    #[tokio::test]
    async fn publishing_requires_explicit_confirmation() {
        let state = AppState::new(
            crate::RunPolicy {
                allowed_origins: vec![],
                max_vus: 1,
                max_duration_seconds: 1,
                max_runtime_seconds: 1,
            },
            None,
        );
        let result = publish_claim(
            State(state),
            Path("example".into()),
            Json(PublishRequest {
                confirmed: false,
                result_set_id: Uuid::new_v4(),
            }),
        )
        .await;
        assert!(matches!(result, Err(ApiError::ConfirmationRequired)));
    }
}
