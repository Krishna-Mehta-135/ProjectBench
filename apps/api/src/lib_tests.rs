use super::*;
use axum::{body::Body, http::Request};
use chrono::Utc;
use sqlx::PgPool;
use tower::ServiceExt;

fn policy() -> RunPolicy {
    RunPolicy {
        allowed_origins: vec!["https://api.canvassync.tech".into()],
        max_vus: 10,
        max_duration_seconds: 300,
        max_runtime_seconds: 360,
    }
}
fn request() -> PreflightRequest {
    PreflightRequest {
        target_origin: "https://api.canvassync.tech/".into(),
        scenario: ScenarioSnapshot {
            name: "Canvassync room read".into(),
            version: "1".into(),
            script_path: "k6/scenarios/canvassync-room-read.js".into(),
            routes: vec!["POST /api/v1/auth/signin".into()],
            secret_refs: vec![
                "TEST_ACCOUNT_EMAIL".into(),
                "TEST_ACCOUNT_PASSWORD".into(),
                "SEEDED_ROOM_ID".into(),
            ],
        },
        workload: Workload {
            vus: 1,
            duration_seconds: 60,
            think_time_seconds: 2,
        },
    }
}

#[test]
fn accepts_allowlisted_https_origin_and_normalizes_it() {
    assert_eq!(
        policy().validate(&request()).unwrap().target_origin,
        "https://api.canvassync.tech"
    );
}
#[test]
fn rejects_non_allowlisted_target() {
    let mut input = request();
    input.target_origin = "https://example.com/".into();
    assert!(matches!(
        policy().validate(&input),
        Err(PolicyError::TargetNotAllowlisted)
    ));
}
#[test]
fn rejects_workload_over_caps() {
    let mut input = request();
    input.workload.vus = 11;
    assert!(matches!(
        policy().validate(&input),
        Err(PolicyError::VusOutOfRange { .. })
    ));
}
#[tokio::test]
async fn health_endpoint_is_available() {
    let response = router(AppState::new(policy(), None))
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn project_list_is_explicitly_empty_until_persistence_is_configured() {
    let response = router(AppState::new(policy(), None))
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), b"[]");
}

#[tokio::test]
async fn create_run_requires_confirmation_before_storage() {
    let body = serde_json::json!({
        "project_id": Uuid::new_v4(),
        "environment_id": Uuid::new_v4(),
        "scenario_id": Uuid::new_v4(),
        "target_origin": "https://api.canvassync.tech/",
        "scenario": {"name":"read","version":"1","script_path":"scenario.js","routes":["GET /health"]},
        "workload": {"vus":1,"duration_seconds":1,"think_time_seconds":0},
        "confirmed": false
    });
    let response = router(AppState::new(policy(), None))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
}

async fn seed_run_dependencies(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let project_id = Uuid::new_v4();
    let environment_id = Uuid::new_v4();
    let scenario_id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query("INSERT INTO projects (id, name, created_at, updated_at) VALUES ($1, 'e2e project', $2, $2)")
            .bind(project_id)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO environments (id, project_id, name, base_url, allowed_origins, created_at, updated_at) VALUES ($1, $2, 'local', 'https://api.canvassync.tech', '[\"https://api.canvassync.tech\"]', $3, $3)")
            .bind(environment_id)
            .bind(project_id)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO scenarios (id, project_id, name, version, script_path, config_snapshot, config_sha256, created_at) VALUES ($1, $2, 'e2e', '1', 'scenario.js', '{}', 'sha256:e2e', $3)")
            .bind(scenario_id)
            .bind(project_id)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
    (project_id, environment_id, scenario_id)
}

async fn create_e2e_run(
    app: &axum::Router,
    project_id: Uuid,
    environment_id: Uuid,
    scenario_id: Uuid,
) -> Uuid {
    let body = serde_json::json!({
        "project_id": project_id,
        "environment_id": environment_id,
        "scenario_id": scenario_id,
        "target_origin": "https://api.canvassync.tech/",
        "scenario": {"name":"e2e","version":"1","script_path":"scenario.js","routes":["GET /health"]},
        "workload": {"vus":1,"duration_seconds":1,"think_time_seconds":0},
        "confirmed": true
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice::<CreatedRun>(&body).unwrap().id
}

async fn execute_e2e_run(app: &axum::Router, run_id: Uuid) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/runs/{run_id}/execute"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn fake_k6_covers_success_failure_cancellation_and_artifacts(pool: PgPool) {
    let fixture =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/bin/k6");
    let artifact_root = std::env::temp_dir().join(format!("metrics-e2e-{}", Uuid::new_v4()));
    std::env::set_var("METRICS_K6_EXECUTABLE", fixture);
    std::env::set_var("ARTIFACT_ROOT", &artifact_root);
    let repository = Arc::new(storage::PgRepository::from_pool(pool.clone()));
    let app = router(AppState::new(policy(), Some(repository)));
    let (project_id, environment_id, scenario_id) = seed_run_dependencies(&pool).await;

    std::env::set_var("K6_FIXTURE_MODE", "success");
    let successful_run = create_e2e_run(&app, project_id, environment_id, scenario_id).await;
    let response = execute_e2e_run(&app, successful_run).await;
    assert_eq!(response.status(), StatusCode::OK);
    let report: RunReport = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report.run_id, successful_run);
    assert!(report.synthetic_traffic);
    assert_eq!(report.artifacts.len(), 2);
    let artifact = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/runs/{successful_run}/artifacts/k6-raw-jsonl"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(artifact.status(), StatusCode::OK);
    assert!(axum::body::to_bytes(artifact.into_body(), usize::MAX)
        .await
        .unwrap()
        .windows(9)
        .any(|window| window == b"http_reqs"));

    std::env::set_var("K6_FIXTURE_MODE", "fail");
    let failed_run = create_e2e_run(&app, project_id, environment_id, scenario_id).await;
    assert_eq!(
        execute_e2e_run(&app, failed_run).await.status(),
        StatusCode::BAD_GATEWAY
    );
    let failed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/runs/{failed_run}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let failed: RunDetail = serde_json::from_slice(
        &axum::body::to_bytes(failed.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(failed.status, "failed");
    assert!(failed.failure_reason.unwrap().contains("exit code"));
    assert!(failed.report.is_none());

    std::env::set_var("K6_FIXTURE_MODE", "sleep");
    let canceled_run = create_e2e_run(&app, project_id, environment_id, scenario_id).await;
    let executing = {
        let app = app.clone();
        tokio::spawn(async move { execute_e2e_run(&app, canceled_run).await })
    };
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    let cancel = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/runs/{canceled_run}/cancel"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        cancel.status(),
        StatusCode::ACCEPTED | StatusCode::OK
    ));
    assert_eq!(executing.await.unwrap().status(), StatusCode::BAD_GATEWAY);
    let canceled = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/runs/{canceled_run}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let canceled: RunDetail = serde_json::from_slice(
        &axum::body::to_bytes(canceled.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(canceled.status, "canceled");
    assert_eq!(
        canceled.failure_reason.as_deref(),
        Some("canceled by operator")
    );
    assert!(canceled.report.is_none());
    let _ = std::fs::remove_dir_all(artifact_root);
}
