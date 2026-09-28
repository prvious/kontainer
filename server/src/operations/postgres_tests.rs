use std::{error::Error, io, path::Path, time::Duration as StdDuration};

use axum::{
    Router,
    body::Body,
    body::to_bytes,
    http::{Request, StatusCode, header},
};
use chrono::{Duration, Utc};
use loco_rs::{
    app::{AppContext, Hooks},
    config::Config,
    environment::Environment,
};
use migration::{Migrator, MigratorTrait, SchemaManager};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    TransactionTrait,
};
use tower::ServiceExt;
use uuid::Uuid;

use super::{
    actions::{admit, claim_next, get, mark_uncertain},
    domain::{AdmitOperationRequest, OperationState},
    entities::{audit_intent, idempotency_record, operation, operation_event},
};
use crate::{
    app::App,
    auth::{actions::create_session, domain::PrincipalContext, entities::principal},
    test_support::PostgresTestDatabase,
};

#[tokio::test]
async fn readiness_tracks_postgres_and_shutdown_without_reporting_dependency_details()
-> Result<(), Box<dyn Error>> {
    use crate::{app::App, controllers::health};
    use axum::{extract::State, http::StatusCode};
    use loco_rs::app::Hooks;
    use tokio_util::sync::CancellationToken;

    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let context = App::after_context(
            AppContext::builder(
                Environment::Test,
                database.connection.clone(),
                Config::from_folder(&Environment::Test, Path::new("config"))?,
            )
            .build(),
        )
        .await?;
        let healthy = health::ready(State(context.clone())).await.status();
        context
            .shared_store
            .get::<CancellationToken>()
            .ok_or_else(|| io::Error::other("controller lifecycle is not initialized"))?
            .cancel();
        let draining = health::ready(State(context.clone())).await.status();

        let context = App::after_context(context).await?;
        let healthy_after_reinitialization = health::ready(State(context.clone())).await.status();
        database.connection.clone().close().await?;
        let unavailable = health::ready(State(context)).await.status();

        Ok::<_, Box<dyn Error>>((
            healthy,
            draining,
            healthy_after_reinitialization,
            unavailable,
        ))
    }
    .await;
    let removed = database.cleanup().await;
    let (healthy, draining, healthy_after_reinitialization, unavailable) = evidence?;
    let removed = removed?;

    assert_eq!(healthy, StatusCode::OK);
    assert_eq!(draining, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(healthy_after_reinitialization, StatusCode::OK);
    assert_eq!(unavailable, StatusCode::SERVICE_UNAVAILABLE);
    assert!(removed);
    Ok(())
}

fn admission_request(
    principal_id: Uuid,
    idempotency_key: &str,
    canonical_request: &[u8],
) -> Result<AdmitOperationRequest, Box<dyn Error>> {
    Ok(AdmitOperationRequest::new(
        PrincipalContext::new(principal_id),
        "operations.integration",
        idempotency_key,
        "test.operation",
        "operation.admit",
        Some("test-target".to_owned()),
        canonical_request.to_vec(),
        Utc::now() + Duration::hours(1),
    )?)
}

async fn test_principal(database: &sea_orm::DatabaseConnection) -> Result<Uuid, Box<dyn Error>> {
    let now = Utc::now();
    let id = Uuid::now_v7();
    principal::ActiveModel {
        id: Set(id),
        email: Set(None),
        display_name: Set(None),
        kind: Set("human".to_owned()),
        status: Set("active".to_owned()),
        authorization_epoch: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(database)
    .await?;
    Ok(id)
}

/// Production route composition: lifecycle routes unlayered, application
/// routes behind access-session middleware. Tests must exercise this exact
/// composition rather than hand-built single-route routers.
async fn production_router(
    connection: &sea_orm::DatabaseConnection,
) -> Result<Router, Box<dyn Error>> {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
    let config = Config::from_folder(&Environment::Test, &config_path)?;
    let context = App::after_context(
        AppContext::builder(Environment::Test, connection.clone(), config).build(),
    )
    .await?;
    Ok(App::routes(&context).to_router::<App>(context, Router::new())?)
}

#[tokio::test]
async fn postgres_migration_up_down_status_and_cleanup() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(false).await? else {
        return Ok(());
    };

    let evidence = async {
        let pending_before = Migrator::get_pending_migrations(&test_database.connection)
            .await?
            .len();
        Migrator::status(&test_database.connection).await?;
        Migrator::up(&test_database.connection, None).await?;
        let applied_after_up = Migrator::get_applied_migrations(&test_database.connection)
            .await?
            .len();
        let schema = SchemaManager::new(&test_database.connection);
        let tables_exist_after_up = [
            schema.has_table("operations").await?,
            schema.has_table("idempotency_records").await?,
            schema.has_table("operation_events").await?,
            schema.has_table("audit_intents").await?,
            schema.has_table("principals").await?,
            schema.has_table("session_families").await?,
            schema.has_table("refresh_credentials").await?,
            schema.has_table("security_audit_intents").await?,
            schema.has_table("github_login_attempts").await?,
            schema.has_table("github_identities").await?,
            schema.has_table("bootstrap_state").await?,
            schema.has_table("password_credentials").await?,
        ];

        Migrator::down(&test_database.connection, None).await?;
        Migrator::status(&test_database.connection).await?;
        let pending_after_down = Migrator::get_pending_migrations(&test_database.connection)
            .await?
            .len();
        let tables_exist_after_down = [
            schema.has_table("operations").await?,
            schema.has_table("idempotency_records").await?,
            schema.has_table("operation_events").await?,
            schema.has_table("audit_intents").await?,
            schema.has_table("principals").await?,
            schema.has_table("session_families").await?,
            schema.has_table("refresh_credentials").await?,
            schema.has_table("security_audit_intents").await?,
            schema.has_table("github_login_attempts").await?,
            schema.has_table("github_identities").await?,
            schema.has_table("bootstrap_state").await?,
            schema.has_table("password_credentials").await?,
        ];

        Ok::<_, sea_orm::DbErr>((
            pending_before,
            applied_after_up,
            pending_after_down,
            tables_exist_after_up,
            tables_exist_after_down,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (pending_before, applied_after_up, pending_after_down, tables_after_up, tables_after_down) =
        evidence?;

    let total = Migrator::migrations().len();
    assert_eq!(pending_before, total);
    assert_eq!(applied_after_up, total);
    assert_eq!(pending_after_down, total);
    assert!(tables_after_up.into_iter().all(|exists| exists));
    assert!(tables_after_down.into_iter().all(|exists| !exists));
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_admission_is_atomic_and_idempotent() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let owner = Uuid::now_v7();
        let principal = PrincipalContext::new(owner);
        let first_request = admission_request(owner, "admit-once", br#"{"replicas":2}"#)?;
        let second_request = admission_request(owner, "admit-once", br#"{"replicas":2}"#)?;
        let (first, second) = tokio::join!(
            admit::execute(&test_database.connection, first_request),
            admit::execute(&test_database.connection, second_request),
        );
        let (first, second) = (first?, second?);
        let (first, replay) = if first.replayed {
            (second, first)
        } else {
            (first, second)
        };
        let conflict = admit::execute(
            &test_database.connection,
            admission_request(owner, "admit-once", br#"{"replicas":3}"#)?,
        )
        .await;
        let snapshot =
            get::execute(&test_database.connection, &principal, first.operation.id).await?;

        Ok::<_, Box<dyn Error>>((
            first,
            replay,
            conflict,
            snapshot,
            operation::Entity::find()
                .count(&test_database.connection)
                .await?,
            idempotency_record::Entity::find()
                .count(&test_database.connection)
                .await?,
            operation_event::Entity::find()
                .count(&test_database.connection)
                .await?,
            audit_intent::Entity::find()
                .count(&test_database.connection)
                .await?,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (first, replay, conflict, snapshot, operations, idempotency, events, audit) = evidence?;

    assert!(!first.replayed);
    assert!(replay.replayed);
    assert_eq!(first.operation.id, replay.operation.id);
    assert!(matches!(
        conflict,
        Err(admit::AdmitOperationError::IdempotencyConflict { operation_id })
            if operation_id == first.operation.id
    ));
    assert_eq!(snapshot.operation.state, OperationState::Recoverable);
    assert_eq!(snapshot.last_event_sequence, 1);
    assert_eq!((operations, idempotency, events, audit), (1, 1, 1, 1));
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_operation_is_polled_through_the_versioned_http_route()
-> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let owner = test_principal(&test_database.connection).await?;
        let admitted = admit::execute(
            &test_database.connection,
            admission_request(owner, "poll-through-http", b"poll")?,
        )
        .await?;
        let session = create_session::execute(&test_database.connection, owner).await?;
        let application = production_router(&test_database.connection).await?;
        let response = application
            .oneshot(
                Request::get(format!("/api/v1/operations/{}", admitted.operation.id))
                    .header(
                        axum::http::header::AUTHORIZATION,
                        format!("Bearer {}", session.access_plaintext),
                    )
                    .body(Body::empty())?,
            )
            .await?;
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await?;
        let document: serde_json::Value = serde_json::from_slice(&body)?;

        Ok::<_, Box<dyn Error>>((admitted.operation.id, status, document))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (operation_id, status, document) = evidence?;

    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(document["id"], operation_id.to_string());
    assert_eq!(document["state"], OperationState::Recoverable.as_str());
    assert_eq!(document["lastEventSequence"], 1);
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_broken_operation_history_is_an_internal_error() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let owner = test_principal(&test_database.connection).await?;
        let principal = PrincipalContext::new(owner);
        let admitted = admit::execute(
            &test_database.connection,
            admission_request(owner, "broken-history", b"history")?,
        )
        .await?;
        operation_event::Entity::delete_many()
            .filter(operation_event::Column::OperationId.eq(admitted.operation.id))
            .exec(&test_database.connection)
            .await?;
        let action_error =
            get::execute(&test_database.connection, &principal, admitted.operation.id).await;

        let session = create_session::execute(&test_database.connection, owner).await?;
        let response = production_router(&test_database.connection)
            .await?
            .oneshot(
                Request::get(format!("/api/v1/operations/{}", admitted.operation.id))
                    .header(
                        axum::http::header::AUTHORIZATION,
                        format!("Bearer {}", session.access_plaintext),
                    )
                    .body(Body::empty())?,
            )
            .await?;

        Ok::<_, Box<dyn Error>>((action_error, response.status()))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (action_error, status) = evidence?;

    assert!(matches!(
        action_error,
        Err(get::GetOperationError::MissingEvent)
    ));
    assert_eq!(status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_skip_locked_prevents_a_double_claim() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let owner = Uuid::now_v7();
        let admitted = admit::execute(
            &test_database.connection,
            admission_request(owner, "claim-once", b"claim")?,
        )
        .await?;

        let locking_transaction = test_database.connection.begin().await?;
        let locked = claim_next::claimable_query()
            .one(&locking_transaction)
            .await?;
        let skipped = tokio::time::timeout(
            StdDuration::from_secs(2),
            claim_next::execute(&test_database.connection, "controller:skipped"),
        )
        .await??;
        locking_transaction.rollback().await?;

        let (first_worker, second_worker) = tokio::join!(
            claim_next::execute(&test_database.connection, "controller:first"),
            claim_next::execute(&test_database.connection, "controller:second")
        );
        let first_worker = first_worker?;
        let second_worker = second_worker?;
        let claimed = [first_worker, second_worker]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        Ok::<_, Box<dyn Error>>((admitted.operation.id, locked, skipped, claimed))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (operation_id, locked, skipped, claimed) = evidence?;

    assert_eq!(locked.map(|model| model.id), Some(operation_id));
    assert!(skipped.is_none());
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].id, operation_id);
    assert_eq!(claimed[0].state, OperationState::Running);
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_rollback_and_uncertain_recovery_are_explicit() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let owner = Uuid::now_v7();
        let principal = PrincipalContext::new(owner);
        let injected = admit::execute_with_injected_rollback(
            &test_database.connection,
            admission_request(owner, "must-rollback", b"rollback")?,
        )
        .await;
        let counts_after_rollback = (
            operation::Entity::find()
                .count(&test_database.connection)
                .await?,
            idempotency_record::Entity::find()
                .count(&test_database.connection)
                .await?,
            operation_event::Entity::find()
                .count(&test_database.connection)
                .await?,
            audit_intent::Entity::find()
                .count(&test_database.connection)
                .await?,
        );

        let admitted = admit::execute(
            &test_database.connection,
            admission_request(owner, "becomes-uncertain", b"timeout")?,
        )
        .await?;
        let recoverable =
            get::execute(&test_database.connection, &principal, admitted.operation.id).await?;
        claim_next::execute(&test_database.connection, "controller:worker")
            .await?
            .ok_or_else(|| io::Error::other("expected admitted operation to be claimed"))?;
        mark_uncertain::execute(
            &test_database.connection,
            admitted.operation.id,
            "engine_timeout",
        )
        .await?;
        let uncertain =
            get::execute(&test_database.connection, &principal, admitted.operation.id).await?;

        Ok::<_, Box<dyn Error>>((injected, counts_after_rollback, recoverable, uncertain))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (injected, counts_after_rollback, recoverable, uncertain) = evidence?;

    assert!(matches!(
        injected,
        Err(admit::AdmitOperationError::InjectedFailure)
    ));
    assert_eq!(counts_after_rollback, (0, 0, 0, 0));
    assert_eq!(recoverable.operation.state, OperationState::Recoverable);
    assert_eq!(uncertain.operation.state, OperationState::Uncertain);
    assert_eq!(uncertain.last_event_sequence, 3);
    assert_eq!(uncertain.last_error_code.as_deref(), Some("engine_timeout"));
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_transitions_reject_missing_or_exhausted_history() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let owner = Uuid::now_v7();
        let mut outcomes = Vec::new();
        for state in [OperationState::Recoverable, OperationState::Running] {
            for exhausted in [false, true] {
                let admitted = admit::execute(
                    &test_database.connection,
                    admission_request(
                        owner,
                        &format!("history-{state:?}-{exhausted}"),
                        b"history",
                    )?,
                )
                .await?;
                let operation_id = admitted.operation.id;
                if state == OperationState::Running {
                    claim_next::execute(&test_database.connection, "controller:worker")
                        .await?
                        .ok_or_else(|| io::Error::other("expected operation to be claimed"))?;
                }
                operation_event::Entity::delete_many()
                    .filter(operation_event::Column::OperationId.eq(operation_id))
                    .exec(&test_database.connection)
                    .await?;
                if exhausted {
                    operation_event::ActiveModel {
                        operation_id: Set(operation_id),
                        sequence: Set(i32::MAX),
                        state: Set(state.as_str().to_owned()),
                        error_code: Set(None),
                        created_at: Set(Utc::now()),
                        ..Default::default()
                    }
                    .insert(&test_database.connection)
                    .await?;
                }
                let rejected = if state == OperationState::Recoverable {
                    matches!(
                        claim_next::execute(&test_database.connection, "controller:worker").await,
                        Err(claim_next::ClaimOperationError::Persistence(_))
                    )
                } else {
                    matches!(
                        mark_uncertain::execute(&test_database.connection, operation_id, "timeout")
                            .await,
                        Err(mark_uncertain::MarkUncertainError::Persistence(_))
                    )
                };
                let model = operation::Entity::find_by_id(operation_id)
                    .one(&test_database.connection)
                    .await?
                    .ok_or_else(|| io::Error::other("operation disappeared"))?;
                let events = operation_event::Entity::find()
                    .filter(operation_event::Column::OperationId.eq(operation_id))
                    .count(&test_database.connection)
                    .await?;
                outcomes.push((
                    rejected,
                    model.state == state.as_str(),
                    events == u64::from(exhausted),
                ));

                // Exclude this intentionally corrupt row from the next claim.
                let mut model: operation::ActiveModel = model.into();
                model.state = Set(OperationState::Failed.as_str().to_owned());
                model.update(&test_database.connection).await?;
            }
        }
        Ok::<_, Box<dyn Error>>(outcomes)
    }
    .await;
    let removed = test_database.cleanup().await?;

    assert!(
        evidence?
            .into_iter()
            .all(|outcome| outcome == (true, true, true))
    );
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_application_routes_deny_anonymous_requests() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let router = production_router(&test_database.connection).await?;
        let mut anonymous = Vec::new();
        for request in [
            Request::get("/api/v1/status").body(Body::empty())?,
            Request::get("/api/v1/openapi.json").body(Body::empty())?,
            Request::get(format!("/api/v1/operations/{}", Uuid::now_v7())).body(Body::empty())?,
            Request::get("/api/v1/nodes/node-1/containers").body(Body::empty())?,
        ] {
            let response = router.clone().oneshot(request).await?;
            let challenge = response.headers().get(header::WWW_AUTHENTICATE).cloned();
            anonymous.push((response.status(), challenge));
        }
        let mut probes = Vec::new();
        for path in ["/health/live", "/health/startup", "/health/ready"] {
            let status = router
                .clone()
                .oneshot(Request::get(path).body(Body::empty())?)
                .await?
                .status();
            probes.push(status);
        }

        Ok::<_, Box<dyn Error>>((anonymous, probes))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (anonymous, probes) = evidence?;

    for (status, challenge) in anonymous {
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            challenge,
            Some(header::HeaderValue::from_static(
                "Bearer error=\"invalid_token\""
            ))
        );
    }
    assert!(
        probes.into_iter().all(|status| status == StatusCode::OK),
        "canonical probes remain the only anonymous routes"
    );
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_authenticated_sessions_reach_protected_routes() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let owner = test_principal(&test_database.connection).await?;
        let session = create_session::execute(&test_database.connection, owner).await?;
        let authorization = format!("Bearer {}", session.access_plaintext);
        let router = production_router(&test_database.connection).await?;

        let mut protected = Vec::new();
        for request in [
            Request::get("/api/v1/status")
                .header(header::AUTHORIZATION, &authorization)
                .body(Body::empty())?,
            Request::get("/api/v1/openapi.json")
                .header(header::AUTHORIZATION, &authorization)
                .body(Body::empty())?,
        ] {
            protected.push(router.clone().oneshot(request).await?.status());
        }

        // Refresh carries its own refresh credential and no access header:
        // the lifecycle exemption must survive protected composition. It
        // runs last because rotation invalidates the access credential above.
        let refresh = router
            .clone()
            .oneshot(
                Request::post("/api/v1/auth/tokens/refresh")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({ "refreshCredential": session.refresh_plaintext })
                            .to_string(),
                    ))?,
            )
            .await?;
        let refresh_status = refresh.status();

        Ok::<_, Box<dyn Error>>((refresh_status, protected))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (refresh_status, protected) = evidence?;

    assert_eq!(refresh_status, StatusCode::OK);
    assert!(
        protected.into_iter().all(|status| status == StatusCode::OK),
        "valid sessions reach status, OpenAPI, and polling"
    );
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_operation_polling_is_scoped_to_the_admission_principal()
-> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let owner_a = test_principal(&test_database.connection).await?;
        let owner_b = test_principal(&test_database.connection).await?;
        let admitted = admit::execute(
            &test_database.connection,
            admission_request(owner_a, "scoped-poll", b"scoped")?,
        )
        .await?;
        let session_a = create_session::execute(&test_database.connection, owner_a).await?;
        let session_b = create_session::execute(&test_database.connection, owner_b).await?;
        let router = production_router(&test_database.connection).await?;

        let get_operation = |credential: &str, operation_id: Uuid| {
            Request::get(format!("/api/v1/operations/{operation_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                .body(Body::empty())
        };
        let own = router
            .clone()
            .oneshot(get_operation(
                &session_a.access_plaintext,
                admitted.operation.id,
            )?)
            .await?;
        let own_status = own.status();
        let own_body = to_bytes(own.into_body(), 1024 * 1024).await?;

        let foreign = router
            .clone()
            .oneshot(get_operation(
                &session_b.access_plaintext,
                admitted.operation.id,
            )?)
            .await?;
        let foreign_status = foreign.status();
        let foreign_body = to_bytes(foreign.into_body(), 1024 * 1024).await?;

        let missing = router
            .clone()
            .oneshot(get_operation(&session_b.access_plaintext, Uuid::now_v7())?)
            .await?;
        let missing_status = missing.status();
        let missing_body = to_bytes(missing.into_body(), 1024 * 1024).await?;

        // Header-only transport: the same valid credential in a query string
        // or cookie, without the header, must not authenticate.
        let mut bypass = Vec::new();
        bypass.push(
            router
                .clone()
                .oneshot(
                    Request::get(format!(
                        "/api/v1/operations/{}?accessCredential={}",
                        admitted.operation.id, session_a.access_plaintext
                    ))
                    .body(Body::empty())?,
                )
                .await?
                .status(),
        );
        bypass.push(
            router
                .clone()
                .oneshot(
                    Request::get(format!("/api/v1/operations/{}", admitted.operation.id))
                        .header(
                            header::COOKIE,
                            format!("session={}", session_a.access_plaintext),
                        )
                        .body(Body::empty())?,
                )
                .await?
                .status(),
        );

        Ok::<_, Box<dyn Error>>((
            own_status,
            own_body,
            foreign_status,
            foreign_body,
            missing_status,
            missing_body,
            bypass,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (own_status, own_body, foreign_status, foreign_body, missing_status, missing_body, bypass) =
        evidence?;

    assert_eq!(own_status, StatusCode::OK);
    let document: serde_json::Value = serde_json::from_slice(&own_body)?;
    assert_eq!(document["state"], OperationState::Recoverable.as_str());
    assert_eq!(foreign_status, StatusCode::NOT_FOUND);
    assert_eq!(missing_status, StatusCode::NOT_FOUND);
    assert_eq!(
        foreign_body, missing_body,
        "cross-principal and nonexistent operations are indistinguishable"
    );
    assert!(
        bypass
            .into_iter()
            .all(|status| status == StatusCode::UNAUTHORIZED),
        "query and cookie credentials must not authenticate"
    );
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_authentication_database_failure_denies_closed() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let owner = test_principal(&test_database.connection).await?;
    let session = create_session::execute(&test_database.connection, owner).await?;
    let router = production_router(&test_database.connection).await?;
    test_database.connection.clone().close().await?;

    // A well-formed credential against an unavailable database must fail as
    // a server-side outage, never as an authentication decision, and the
    // handler must not run (a lookup miss would be 401, a poll would be 200).
    let response = router
        .oneshot(
            Request::get(format!("/api/v1/operations/{}", Uuid::now_v7()))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", session.access_plaintext),
                )
                .body(Body::empty())?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(test_database.cleanup().await?);
    Ok(())
}
