
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
            secret_refs: Vec::new(),
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
    assert!(
        !repository
            .set_environment_enabled(project_id, environment_id, false)
            .await
            .unwrap()
            .unwrap()
            .enabled
    );
    assert!(
        repository
            .set_environment_enabled(project_id, environment_id, true)
            .await
            .unwrap()
            .unwrap()
            .enabled
    );

    let run_id = repository
        .create_queued_run(
            &request,
            "https://example.test",
            &crate::scenario_hash(&request.scenario),
        )
        .await
        .unwrap();
    repository
        .validate_execution_configuration(run_id)
        .await
        .unwrap();

    sqlx::query("UPDATE runs SET scenario_snapshot = jsonb_build_object('name', 'changed', 'version', '1', 'script_path', 'scenario.js', 'routes', jsonb_build_array('GET /health')) WHERE id = $1")
            .bind(run_id)
            .execute(&pool)
            .await
            .unwrap();
    assert!(matches!(
        repository.validate_execution_configuration(run_id).await,
        Err(StorageError::Configuration(message)) if message.contains("snapshot")
    ));

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
