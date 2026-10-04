//! The operation worker, admission edge, and lifecycle actions against real PostgreSQL.

use std::{error::Error, sync::Arc, time::Duration as StdDuration};

use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, PaginatorTrait};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use uuid::Uuid;

use super::{
    actions::{admit, cancel, claim_next, finalize, get, list_events, prune, recover_running},
    bulk::{BulkTarget, run_bulk},
    domain::{DurableOperation, OperationKind, OperationSnapshot, OperationState},
    entities::{idempotency_record, operation},
    executor::{ExecutionContext, ExecutionFailure, ExecutorRegistry, OperationExecutor},
    executors::noop::NoopExecutor,
    postgres_tests::{production_router, test_principal},
    worker::{Worker, WorkerSettings},
};
use crate::{
    auth::{actions::create_session, domain::PrincipalContext},
    test_support::PostgresTestDatabase,
};

fn fast_settings() -> WorkerSettings {
    WorkerSettings {
        reference: "controller:test".to_owned(),
        concurrency: 4,
        idle_min: StdDuration::from_millis(10),
        idle_max: StdDuration::from_millis(50),
        maintenance_every: StdDuration::from_secs(3600),
        grace: StdDuration::from_secs(2),
    }
}

fn start_worker(
    database: &sea_orm::DatabaseConnection,
    registry: ExecutorRegistry,
) -> (CancellationToken, tokio::task::JoinHandle<()>) {
    let draining = CancellationToken::new();
    let worker = Worker {
        database: database.clone(),
        registry: Arc::new(registry),
        agents: None,
        manager: None,
        settings: fast_settings(),
    };
    let task = tokio::spawn(worker.run(draining.clone()));
    (draining, task)
}

async fn admit_kind(
    database: &sea_orm::DatabaseConnection,
    principal: Uuid,
    kind: OperationKind,
    payload: serde_json::Value,
) -> Result<Uuid, Box<dyn Error>> {
    let request = sea_orm_admission(principal, kind, &payload)?;
    Ok(admit::execute(database, request).await?.operation.id)
}

fn sea_orm_admission(
    principal: Uuid,
    kind: OperationKind,
    payload: &serde_json::Value,
) -> Result<super::domain::AdmitOperationRequest, Box<dyn Error>> {
    Ok(super::domain::AdmitOperationRequest::new(
        PrincipalContext::new(principal),
        "operations.worker",
        Uuid::now_v7().to_string(),
        kind.as_str(),
        kind.as_str(),
        None,
        serde_json::to_vec(payload)?,
        Utc::now() + Duration::hours(1),
    )?
    .with_payload(payload.clone())
    .with_request_id(Some("req-test-01".to_owned())))
}

/// Wait until the operation is terminal or `uncertain`; neither moves again on its own.
async fn wait_settled(
    database: &sea_orm::DatabaseConnection,
    principal: Uuid,
    id: Uuid,
) -> Result<OperationSnapshot, Box<dyn Error>> {
    let principal = PrincipalContext::new(principal);
    for _ in 0..400 {
        let snapshot = get::execute(database, &principal, id).await?;
        if snapshot.operation.state.is_terminal() || snapshot.operation.state.is_uncertain() {
            return Ok(snapshot);
        }
        tokio::time::sleep(StdDuration::from_millis(10)).await;
    }
    Err("operation did not settle".into())
}

#[tokio::test]
async fn postgres_noop_runs_end_to_end_with_progress_and_one_terminal_event()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        let principal = test_principal(db).await?;
        let (draining, worker) = start_worker(db, ExecutorRegistry::default().with(NoopExecutor));
        let id = admit_kind(
            db,
            principal,
            OperationKind::Noop,
            serde_json::json!({"steps": 3, "stepMs": 5}),
        )
        .await?;
        let snapshot = wait_settled(db, principal, id).await?;
        let failed_id = admit_kind(
            db,
            principal,
            OperationKind::Noop,
            serde_json::json!({"steps": 1, "stepMs": 1, "failWith": "no_eligible_nodes"}),
        )
        .await?;
        let failed = wait_settled(db, principal, failed_id).await?;
        draining.cancel();
        worker.await?;

        assert_eq!(snapshot.operation.state, OperationState::Succeeded);
        assert_eq!(
            snapshot.progress,
            Some(serde_json::json!({"total": 3, "done": 3}))
        );
        assert!(snapshot.operation.started_at.is_some());
        assert!(snapshot.operation.finished_at >= snapshot.operation.started_at);
        assert_eq!(
            snapshot.operation.request_id.as_deref(),
            Some("req-test-01")
        );
        let events =
            list_events::execute(db, &PrincipalContext::new(principal), id, 0, 100).await?;
        let states: Vec<_> = events.iter().map(|event| event.state).collect();
        assert_eq!(
            states,
            vec![
                OperationState::Recoverable,
                OperationState::Running,
                OperationState::Running,
                OperationState::Running,
                OperationState::Running,
                OperationState::Succeeded,
            ]
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.state.is_terminal())
                .count(),
            1
        );
        assert_eq!(
            events[2].detail,
            Some(serde_json::json!({"total": 3, "done": 1}))
        );

        assert_eq!(failed.operation.state, OperationState::Failed);
        assert_eq!(failed.last_error_code.as_deref(), Some("no_eligible_nodes"));
        assert_eq!(
            failed.error_message.as_deref(),
            Some("The test operation failed as requested.")
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_a_failed_finalize_is_retried_until_the_outcome_lands()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        let principal = test_principal(db).await?;
        // Admitted and armed before the worker starts, so the first finalize fails.
        let id = admit_kind(
            db,
            principal,
            OperationKind::Noop,
            serde_json::json!({"steps": 1, "stepMs": 1}),
        )
        .await?;
        finalize::injected::fail_next(id);
        let (draining, worker) = start_worker(db, ExecutorRegistry::default().with(NoopExecutor));
        let snapshot = wait_settled(db, principal, id).await?;
        draining.cancel();
        worker.await?;

        assert_eq!(snapshot.operation.state, OperationState::Succeeded);
        let took = snapshot.operation.finished_at.unwrap() - snapshot.operation.started_at.unwrap();
        assert!(
            took >= Duration::milliseconds(250),
            "recorded on the retry after the first backoff, took {took}"
        );
        let events =
            list_events::execute(db, &PrincipalContext::new(principal), id, 0, 100).await?;
        assert_eq!(
            events
                .iter()
                .filter(|event| event.state.is_terminal())
                .count(),
            1
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

struct StuckExecutor;

#[async_trait]
impl OperationExecutor for StuckExecutor {
    fn kind(&self) -> OperationKind {
        OperationKind::ServiceRestart
    }

    fn timeout(&self, _payload: &serde_json::Value) -> StdDuration {
        StdDuration::from_millis(50)
    }

    async fn execute(
        &self,
        _operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        context.cancellation.cancelled().await;
        Err(ExecutionFailure::new("cancelled", "interrupted"))
    }
}

struct PanickingExecutor;

#[async_trait]
impl OperationExecutor for PanickingExecutor {
    fn kind(&self) -> OperationKind {
        OperationKind::StackRemove
    }

    async fn execute(
        &self,
        _operation: &DurableOperation,
        _context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        panic!("executor bug");
    }
}

/// A timeout leaves the outcome unknown (the node may still finish), so it is `uncertain`, never a
/// guessed `failed`; panics and unknown kinds fail with stable codes.
#[tokio::test]
async fn postgres_timeouts_are_uncertain_and_panics_and_unknown_kinds_fail_with_stable_codes()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        let principal = test_principal(db).await?;
        let registry = ExecutorRegistry::default()
            .with(StuckExecutor)
            .with(PanickingExecutor);
        let (draining, worker) = start_worker(db, registry);
        let stuck = admit_kind(
            db,
            principal,
            OperationKind::ServiceRestart,
            serde_json::json!({}),
        )
        .await?;
        let panicking = admit_kind(
            db,
            principal,
            OperationKind::StackRemove,
            serde_json::json!({}),
        )
        .await?;
        let unregistered = admit_kind(
            db,
            principal,
            OperationKind::ImagePull,
            serde_json::json!({}),
        )
        .await?;
        let results = [
            wait_settled(db, principal, stuck).await?,
            wait_settled(db, principal, panicking).await?,
            wait_settled(db, principal, unregistered).await?,
        ];
        draining.cancel();
        worker.await?;
        let codes: Vec<_> = results
            .iter()
            .map(|snapshot| (snapshot.operation.state, snapshot.last_error_code.clone()))
            .collect();
        assert_eq!(
            codes,
            vec![
                (OperationState::Uncertain, Some("timeout".to_owned())),
                (OperationState::Failed, Some("internal_error".to_owned())),
                (OperationState::Failed, Some("unsupported_kind".to_owned())),
            ]
        );
        assert!(!format!("{results:?}").contains("executor bug"));
        assert_eq!(
            results[0].operation.finished_at, None,
            "it may still finish"
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

struct BulkExecutor;

#[async_trait]
impl OperationExecutor for BulkExecutor {
    fn kind(&self) -> OperationKind {
        OperationKind::ContainersBulk
    }

    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        let targets: Vec<BulkTarget> = serde_json::from_value(operation.payload["targets"].clone())
            .map_err(|_| ExecutionFailure::invalid_payload())?;
        run_bulk(targets, context, |target| async move {
            if target.container_id.starts_with('b') {
                Err("health_check_timeout".to_owned())
            } else {
                Ok(())
            }
        })
        .await
    }
}

#[tokio::test]
async fn postgres_bulk_partial_failure_keeps_every_result() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        let principal = test_principal(db).await?;
        let (draining, worker) = start_worker(db, ExecutorRegistry::default().with(BulkExecutor));
        let targets = |ids: &[&str]| {
            serde_json::json!({
                "action": "restart",
                "targets": ids.iter().map(|id| serde_json::json!({"nodeId": "manager", "containerId": id.repeat(64)})).collect::<Vec<_>>(),
            })
        };
        let partial = admit_kind(db, principal, OperationKind::ContainersBulk, targets(&["a", "b", "c"])).await?;
        let complete = admit_kind(db, principal, OperationKind::ContainersBulk, targets(&["a", "c"])).await?;
        let partial = wait_settled(db, principal, partial).await?;
        let complete = wait_settled(db, principal, complete).await?;
        draining.cancel();
        worker.await?;

        assert_eq!(partial.operation.state, OperationState::Failed);
        assert_eq!(partial.last_error_code.as_deref(), Some("partial_failure"));
        assert_eq!(partial.error_message.as_deref(), Some("1 of 3 targets failed."));
        let detail = partial.progress.unwrap();
        assert_eq!((detail["total"].clone(), detail["done"].clone(), detail["failed"].clone()), (3.into(), 3.into(), 1.into()));
        let results = detail["results"].as_array().unwrap();
        assert_eq!(results.len(), 3);
        let failed: Vec<_> = results.iter().filter(|result| result["ok"] == false).collect();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0]["error"], "health_check_timeout");
        assert_eq!(failed[0]["containerId"], "b".repeat(64));

        assert_eq!(complete.operation.state, OperationState::Succeeded);
        assert_eq!(complete.progress.unwrap()["failed"], 0);
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_recovery_cancel_prune_and_event_pages() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        let principal_id = test_principal(db).await?;
        let principal = PrincipalContext::new(principal_id);

        // Restart recovery: a claimed (running) operation becomes uncertain.
        let running =
            admit_kind(db, principal_id, OperationKind::Noop, serde_json::json!({})).await?;
        assert_eq!(
            claim_next::execute(db, "controller:old").await?.unwrap().id,
            running
        );
        assert_eq!(recover_running::execute(db).await?, 1);
        let recovered = get::execute(db, &principal, running).await?;
        assert_eq!(recovered.operation.state, OperationState::Uncertain);
        assert_eq!(
            recovered.last_error_code.as_deref(),
            Some("recovered_after_restart")
        );
        assert_eq!(recover_running::execute(db).await?, 0, "idempotent");

        // Cancel only while recoverable; another principal cannot see it at all.
        let queued =
            admit_kind(db, principal_id, OperationKind::Noop, serde_json::json!({})).await?;
        let stranger = PrincipalContext::new(test_principal(db).await?);
        assert!(matches!(
            cancel::execute(db, &stranger, queued).await,
            Err(cancel::CancelOperationError::NotFound)
        ));
        assert_eq!(
            cancel::execute(db, &principal, queued).await?.state,
            OperationState::Cancelled
        );
        assert!(matches!(
            cancel::execute(db, &principal, queued).await,
            Err(cancel::CancelOperationError::NotCancellable)
        ));
        assert!(matches!(
            cancel::execute(db, &principal, running).await,
            Err(cancel::CancelOperationError::NotCancellable)
        ));
        assert!(
            claim_next::execute(db, "controller:new").await?.is_none(),
            "cancelled never runs"
        );

        // Event pages.
        let all = list_events::execute(db, &principal, queued, 0, 100).await?;
        assert_eq!(all.len(), 2);
        let after_first = list_events::execute(db, &principal, queued, 1, 100).await?;
        assert_eq!(after_first.len(), 1);
        assert_eq!(after_first[0].state, OperationState::Cancelled);
        assert_eq!(
            list_events::execute(db, &principal, queued, 0, 1)
                .await?
                .len(),
            1
        );
        assert!(
            list_events::execute(db, &principal, queued, 0, 0)
                .await
                .is_err()
        );

        // Prune: only terminal operations older than seven days go.
        let old_terminal = queued;
        let old_uncertain = running;
        for id in [old_terminal, old_uncertain] {
            let model = operation::Entity::find_by_id(id).one(db).await?.unwrap();
            let mut model: operation::ActiveModel = model.into();
            model.updated_at = Set(Utc::now() - Duration::days(8));
            model.update(db).await?;
        }
        let fresh =
            admit_kind(db, principal_id, OperationKind::Noop, serde_json::json!({})).await?;
        cancel::execute(db, &principal, fresh).await?;
        let report = prune::execute(db).await?;
        assert_eq!(report.operations, 1);
        assert!(
            operation::Entity::find_by_id(old_terminal)
                .one(db)
                .await?
                .is_none()
        );
        assert!(
            operation::Entity::find_by_id(old_uncertain)
                .one(db)
                .await?
                .is_some()
        );
        assert!(
            operation::Entity::find_by_id(fresh)
                .one(db)
                .await?
                .is_some()
        );
        assert_eq!(idempotency_record::Entity::find().count(db).await?, 2);
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_http_admission_requires_a_key_and_replays_or_conflicts()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        let principal = test_principal(db).await?;
        let session = create_session::execute(db, principal).await?;
        let authorization = format!("Bearer {}", session.access_plaintext);
        let router = production_router(db).await?;
        let post = |key: Option<&str>, body: &str| {
            let mut request = Request::post("/api/v1/operations/noop")
                .header(header::AUTHORIZATION, &authorization)
                .header(header::CONTENT_TYPE, "application/json");
            if let Some(key) = key {
                request = request.header("idempotency-key", key);
            }
            request.body(Body::from(body.to_owned())).unwrap()
        };
        let read = |response: axum::response::Response| async move {
            let status = response.status();
            let body: serde_json::Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap())
                    .unwrap();
            (status, body)
        };
        let key = "018f2e34-0000-7000-8000-0000000000aa";

        let (missing, missing_body) = read(router.clone().oneshot(post(None, "{}")).await?).await;
        let (short, _) = read(router.clone().oneshot(post(Some("short"), "{}")).await?).await;
        let (first, first_body) = read(
            router
                .clone()
                .oneshot(post(Some(key), r#"{"steps":2}"#))
                .await?,
        )
        .await;
        let (replay, replay_body) = read(
            router
                .clone()
                .oneshot(post(Some(key), r#"{"steps":2}"#))
                .await?,
        )
        .await;
        let (conflict, conflict_body) = read(
            router
                .clone()
                .oneshot(post(Some(key), r#"{"steps":3}"#))
                .await?,
        )
        .await;
        let (invalid, invalid_body) = read(
            router
                .clone()
                .oneshot(post(
                    Some("018f2e34-0000-7000-8000-0000000000bb"),
                    r#"{"steps":99}"#,
                ))
                .await?,
        )
        .await;
        let operation_id: Uuid = first_body["operationId"].as_str().unwrap().parse()?;
        let events = router
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/operations/{operation_id}/events?after=0&limit=10"
                ))
                .header(header::AUTHORIZATION, &authorization)
                .body(Body::empty())?,
            )
            .await?;
        let (events_status, events_body) = read(events).await;
        let cancel = router
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/operations/{operation_id}/cancel"))
                    .header(header::AUTHORIZATION, &authorization)
                    .body(Body::empty())?,
            )
            .await?;
        let (cancel_status, cancel_body) = read(cancel).await;
        let cancel_again = router
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/operations/{operation_id}/cancel"))
                    .header(header::AUTHORIZATION, &authorization)
                    .body(Body::empty())?,
            )
            .await?;
        let (again_status, again_body) = read(cancel_again).await;

        assert_eq!(missing, StatusCode::BAD_REQUEST);
        assert_eq!(
            missing_body,
            serde_json::json!({"error": "idempotency_key_required"})
        );
        assert_eq!(short, StatusCode::BAD_REQUEST);
        assert_eq!(first, StatusCode::ACCEPTED);
        assert_eq!(first_body["replayed"], false);
        assert_eq!(replay, StatusCode::ACCEPTED);
        assert_eq!(replay_body["replayed"], true);
        assert_eq!(replay_body["operationId"], first_body["operationId"]);
        assert_eq!(conflict, StatusCode::CONFLICT);
        assert_eq!(
            conflict_body,
            serde_json::json!({"error": "idempotency_conflict"})
        );
        assert_eq!(invalid, StatusCode::BAD_REQUEST);
        assert_eq!(
            invalid_body,
            serde_json::json!({"error": "invalid_request"})
        );
        assert_eq!(events_status, StatusCode::OK);
        assert_eq!(events_body["items"][0]["state"], "recoverable");
        assert_eq!(cancel_status, StatusCode::ACCEPTED);
        assert_eq!(cancel_body["state"], "cancelled");
        assert!(cancel_body["finishedAt"].is_string());
        assert!(
            cancel_body["requestId"].is_string(),
            "the admitting request id is kept"
        );
        assert_eq!(again_status, StatusCode::CONFLICT);
        assert_eq!(again_body, serde_json::json!({"error": "not_cancellable"}));
        assert_eq!(
            operation::Entity::find().count(db).await?,
            1,
            "replay admitted nothing new"
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}
