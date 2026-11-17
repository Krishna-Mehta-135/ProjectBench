use crate::{
    claims::Claim, claims_storage::StoredResult, results::ResultDocument, ApiError, AppState,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

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
}
