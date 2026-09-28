use std::{
    error::Error,
    io,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderValue, Request, StatusCode, header},
    middleware,
    routing::{delete, get, post},
};
use chrono::{Duration, Utc};
use loco_rs::{
    app::{AppContext, Hooks},
    config::Config,
    environment::Environment,
};
use migration::SchemaManager;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    sea_query::{Alias, Table},
};
use tower::ServiceExt;
use uuid::Uuid;

use super::{
    actions::{
        accept_github_callback, create_session, redeem_github_login, revoke_session_family,
        rotate_refresh, start_github_login, validate_access,
    },
    domain::{IssuedSession, PrincipalContext, RevocationReason, actions},
    entities::{
        bootstrap_state, github_identity, github_login_attempt, principal, refresh_credential,
        security_audit_intent, session_family,
    },
    github::{
        self, GITHUB_AUTHORIZE_URL, GITHUB_TOKEN_URL, GITHUB_USER_URL, GitHubAuthService,
        GitHubLoginConfig, fake::FakeGitHubProvider,
    },
    request::require_access_session,
};
use crate::{app::App, test_support::PostgresTestDatabase};

async fn test_principal(
    database: &sea_orm::DatabaseConnection,
    status: &str,
) -> Result<Uuid, Box<dyn Error>> {
    let now = Utc::now();
    let id = Uuid::now_v7();
    principal::ActiveModel {
        id: Set(id),
        email: Set(None),
        display_name: Set(None),
        kind: Set("human".to_owned()),
        status: Set(status.to_owned()),
        authorization_epoch: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(database)
    .await?;
    Ok(id)
}

async fn issued(database: &sea_orm::DatabaseConnection) -> Result<IssuedSession, Box<dyn Error>> {
    let principal = test_principal(database, "active").await?;
    Ok(create_session::execute(database, principal).await?)
}

async fn counts(
    database: &sea_orm::DatabaseConnection,
) -> Result<(u64, u64, u64, u64), sea_orm::DbErr> {
    Ok((
        principal::Entity::find().count(database).await?,
        session_family::Entity::find().count(database).await?,
        refresh_credential::Entity::find().count(database).await?,
        security_audit_intent::Entity::find()
            .count(database)
            .await?,
    ))
}

async fn family(database: &sea_orm::DatabaseConnection, id: Uuid) -> session_family::Model {
    session_family::Entity::find_by_id(id)
        .one(database)
        .await
        .unwrap()
        .unwrap()
}

fn session_router(context: AppContext) -> Router {
    Router::new()
        .route(
            "/api/v1/auth/tokens/refresh",
            post(crate::controllers::sessions::refresh),
        )
        .route(
            "/api/v1/auth/sessions/current",
            delete(crate::controllers::sessions::logout_current),
        )
        .with_state(context)
}

async fn test_context(database: sea_orm::DatabaseConnection) -> Result<AppContext, Box<dyn Error>> {
    let config = Config::from_folder(&Environment::Test, Path::new("config"))?;
    let context = AppContext::builder(Environment::Test, database, config).build();
    Ok(App::after_context(context).await?)
}

async fn authorized_get(
    router: &Router,
    path: &str,
    access: &str,
) -> Result<(StatusCode, Option<String>, serde_json::Value), Box<dyn Error>> {
    let response = router
        .clone()
        .oneshot(
            Request::get(path)
                .header(header::AUTHORIZATION, format!("Bearer {access}"))
                .body(Body::empty())?,
        )
        .await?;
    let status = response.status();
    let cache = response
        .headers()
        .get(header::CACHE_CONTROL)
        .map(|value| value.to_str().unwrap_or_default().to_owned());
    let body = serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await?)?;
    Ok((status, cache, body))
}

async fn assert_challenged(
    router: &Router,
    path: &str,
    access: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let mut request = Request::get(path);
    if let Some(access) = access {
        request = request.header(header::AUTHORIZATION, format!("Bearer {access}"));
    }
    let response = router.clone().oneshot(request.body(Body::empty())?).await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    assert_eq!(
        response.headers().get(header::WWW_AUTHENTICATE).unwrap(),
        "Bearer error=\"invalid_token\""
    );
    Ok(())
}

#[tokio::test]
async fn postgres_deep_status_enforces_session_and_drain_before_diagnostics()
-> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let session = issued(&test_database.connection).await?;
        let context = test_context(test_database.connection.clone()).await?;
        let drain = context
            .shared_store
            .get::<tokio_util::sync::CancellationToken>()
            .unwrap();
        let router = App::routes(&context)
            .to_router::<App>(context.clone(), Router::new())?
            .layer(middleware::from_fn_with_state(
                drain.clone(),
                crate::controllers::health::admit_request,
            ));
        let path = "/api/v1/status/deep";
        let nodes_path = "/api/v1/nodes";
        let access = session.access_plaintext.clone();

        let (status, cache, body) = authorized_get(&router, path, &access).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(cache.as_deref(), Some("no-store"));
        assert_eq!(
            body,
            serde_json::json!({
                "build": {"version": env!("CARGO_PKG_VERSION")},
                "migration": "current",
                "database": "reachable",
                "nodesOnline": 0,
                "nodesTotal": 0
            })
        );
        let (status, cache, body) = authorized_get(&router, nodes_path, &access).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(cache.as_deref(), Some("no-store"));
        assert_eq!(body["items"], serde_json::json!([]));

        crate::nodes::actions::record_check_in::execute(
            &test_database.connection,
            "test",
            crate::nodes::domain::CheckIn {
                node_id: "manager".to_owned(),
                agent_version: "0.1.0".to_owned(),
                architecture: "aarch64".to_owned(),
                advertise_address: "127.0.0.1:50052".to_owned(),
                hostname: "manager".to_owned(),
                swarm_node_id: None,
                docker_version: "29.8.0".to_owned(),
                os: "linux".to_owned(),
                cpu_cores: 4,
                memory_total_bytes: 8 << 30,
                agent_started_at: chrono::Utc::now(),
                sample: None,
            },
        )
        .await?;
        let (_, _, body) = authorized_get(&router, path, &access).await?;
        assert_eq!(body["nodesOnline"], 1);
        assert_eq!(body["nodesTotal"], 1);
        let (_, _, body) = authorized_get(&router, nodes_path, &access).await?;
        assert_eq!(body["items"][0]["id"], "manager");
        assert_eq!(body["items"][0]["status"], "online");
        assert!(body["items"][0]["latestSample"].is_null());

        assert_challenged(&router, path, None).await?;
        assert_challenged(&router, nodes_path, None).await?;

        revoke_session_family::execute(
            &test_database.connection,
            session.family_id,
            RevocationReason::Logout,
        )
        .await?;
        assert_challenged(&router, path, Some(&access)).await?;
        assert_challenged(&router, nodes_path, Some(&access)).await?;

        drain.cancel();
        for draining_path in [path, nodes_path] {
            let draining = router
                .clone()
                .oneshot(Request::get(draining_path).body(Body::empty())?)
                .await?;
            assert_eq!(draining.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(
                draining.headers().get(header::CACHE_CONTROL).unwrap(),
                "no-store"
            );
            let draining_body: serde_json::Value =
                serde_json::from_slice(&to_bytes(draining.into_body(), 4096).await?)?;
            assert_eq!(draining_body, serde_json::json!({"ok": false}));
        }

        let manager = SchemaManager::new(&test_database.connection);
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("session_families"))
                    .cascade()
                    .to_owned(),
            )
            .await?;
        let broken_context = test_context(test_database.connection.clone()).await?;
        let broken_router =
            App::routes(&broken_context).to_router::<App>(broken_context, Router::new())?;
        for broken_path in [path, nodes_path] {
            let (status, cache, body) =
                authorized_get(&broken_router, broken_path, &access).await?;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(cache.as_deref(), Some("no-store"));
            assert_eq!(body, serde_json::json!({"error": "unavailable"}));
        }
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = test_database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_session_create_validate_then_logout_revokes() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let session = issued(&test_database.connection).await?;
        let validated =
            validate_access::execute(&test_database.connection, &session.access_plaintext).await?;
        revoke_session_family::execute(
            &test_database.connection,
            session.family_id,
            RevocationReason::Logout,
        )
        .await?;
        let access_after =
            validate_access::execute(&test_database.connection, &session.access_plaintext).await;
        let refresh_after =
            rotate_refresh::execute(&test_database.connection, &session.refresh_plaintext).await;
        let revoked = family(&test_database.connection, session.family_id).await;
        let audit_actions: Vec<String> = security_audit_intent::Entity::find()
            .filter(security_audit_intent::Column::FamilyId.eq(session.family_id))
            .all(&test_database.connection)
            .await?
            .into_iter()
            .map(|row| row.action)
            .collect();

        Ok::<_, Box<dyn Error>>((
            validated.family_id == session.family_id
                && validated.principal_id == session.principal_id,
            access_after.is_err(),
            refresh_after.is_err(),
            revoked.revoked_at.is_some(),
            revoked.revoked_reason,
            audit_actions,
            counts(&test_database.connection).await?,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (validated, access_denied, refresh_denied, revoked, reason, audit, counts) = evidence?;

    assert!(validated, "fresh access credential must validate");
    assert!(
        access_denied,
        "logout must deny the unexpired access credential"
    );
    assert!(refresh_denied, "logout must deny the refresh credential");
    assert!(revoked);
    assert_eq!(reason.as_deref(), Some("logout"));
    assert_eq!(audit, vec![actions::CREATE, actions::REVOKE]);
    assert_eq!(counts, (1, 1, 1, 2));
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_session_rejects_inactive_and_missing_principals() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let mut outcomes = Vec::new();
        for status in ["pending", "suspended", "deleted"] {
            let principal = test_principal(&test_database.connection, status).await?;
            outcomes.push(
                create_session::execute(&test_database.connection, principal)
                    .await
                    .is_err(),
            );
        }
        let missing = create_session::execute(&test_database.connection, Uuid::now_v7()).await;
        Ok::<_, Box<dyn Error>>((
            outcomes,
            missing.is_err(),
            counts(&test_database.connection).await?,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (outcomes, missing_rejected, (principals, families, refreshes, audits)) = evidence?;

    assert!(outcomes.into_iter().all(|rejected| rejected));
    assert!(missing_rejected);
    assert_eq!((principals, families, refreshes, audits), (3, 0, 0, 0));
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_access_validation_enforces_expiry_and_status() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        // Expired access credential.
        let session = issued(&test_database.connection).await?;
        let mut expired: session_family::ActiveModel =
            family(&test_database.connection, session.family_id)
                .await
                .into();
        expired.access_expires_at = Set(Utc::now() - Duration::minutes(1));
        expired.update(&test_database.connection).await?;
        let expired_denied =
            validate_access::execute(&test_database.connection, &session.access_plaintext)
                .await
                .is_err();

        // Expired family.
        let session = issued(&test_database.connection).await?;
        let mut absolute: session_family::ActiveModel =
            family(&test_database.connection, session.family_id)
                .await
                .into();
        absolute.absolute_expires_at = Set(Utc::now() - Duration::seconds(1));
        absolute.update(&test_database.connection).await?;
        let absolute_denied =
            validate_access::execute(&test_database.connection, &session.access_plaintext)
                .await
                .is_err();

        // Suspended principal.
        let principal = test_principal(&test_database.connection, "active").await?;
        let session = create_session::execute(&test_database.connection, principal).await?;
        let mut suspended: principal::ActiveModel = principal::Entity::find_by_id(principal)
            .one(&test_database.connection)
            .await?
            .ok_or("principal disappeared")?
            .into();
        suspended.status = Set("suspended".to_owned());
        suspended.update(&test_database.connection).await?;
        let suspended_denied =
            validate_access::execute(&test_database.connection, &session.access_plaintext)
                .await
                .is_err();

        // Malformed and unknown credentials.
        let malformed_denied =
            validate_access::execute(&test_database.connection, "kta_not-a-credential")
                .await
                .is_err();
        let unknown = super::credentials::AccessCredential::generate();
        let unknown_denied =
            validate_access::execute(&test_database.connection, &unknown.plaintext())
                .await
                .is_err();

        Ok::<_, Box<dyn Error>>((
            expired_denied,
            absolute_denied,
            suspended_denied,
            malformed_denied,
            unknown_denied,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (expired, absolute, suspended, malformed, unknown) = evidence?;

    assert!(expired && absolute && suspended && malformed && unknown);
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_refresh_rotation_replaces_both_credentials() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let first = issued(&test_database.connection).await?;
        let second =
            rotate_refresh::execute(&test_database.connection, &first.refresh_plaintext).await?;
        assert_ne!(first.access_plaintext, second.access_plaintext);
        assert_ne!(first.refresh_plaintext, second.refresh_plaintext);

        let old_access =
            validate_access::execute(&test_database.connection, &first.access_plaintext).await;
        let new_access =
            validate_access::execute(&test_database.connection, &second.access_plaintext).await;
        let successor_rows = refresh_credential::Entity::find()
            .filter(refresh_credential::Column::FamilyId.eq(first.family_id))
            .count(&test_database.connection)
            .await?;
        let audit: Vec<String> = security_audit_intent::Entity::find()
            .filter(security_audit_intent::Column::FamilyId.eq(first.family_id))
            .all(&test_database.connection)
            .await?
            .into_iter()
            .map(|row| row.action)
            .collect();

        Ok::<_, Box<dyn Error>>((
            old_access.is_err(),
            new_access.is_ok(),
            successor_rows,
            audit,
            second,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (old_denied, new_valid, rows, audit, second) = evidence?;

    assert!(
        old_denied,
        "rotation must invalidate the previous access credential"
    );
    assert!(new_valid);
    assert_eq!(rows, 2);
    assert_eq!(audit, vec![actions::CREATE, actions::ROTATE]);
    assert!(!second.access_plaintext.is_empty() && !second.refresh_plaintext.is_empty());
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_refresh_reuse_revokes_the_whole_family() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let first = issued(&test_database.connection).await?;
        let second =
            rotate_refresh::execute(&test_database.connection, &first.refresh_plaintext).await?;
        let reuse =
            rotate_refresh::execute(&test_database.connection, &first.refresh_plaintext).await;

        let successor_access =
            validate_access::execute(&test_database.connection, &second.access_plaintext).await;
        let successor_refresh =
            rotate_refresh::execute(&test_database.connection, &second.refresh_plaintext).await;
        let revoked = family(&test_database.connection, first.family_id).await;
        let revoked_refreshes = refresh_credential::Entity::find()
            .filter(refresh_credential::Column::FamilyId.eq(first.family_id))
            .filter(refresh_credential::Column::RevokedAt.is_not_null())
            .count(&test_database.connection)
            .await?;
        let audit: Vec<String> = security_audit_intent::Entity::find()
            .filter(security_audit_intent::Column::FamilyId.eq(first.family_id))
            .all(&test_database.connection)
            .await?
            .into_iter()
            .map(|row| row.action)
            .collect();

        Ok::<_, Box<dyn Error>>((
            matches!(
                reuse,
                Err(rotate_refresh::RotateRefreshError::ReuseDetected)
            ),
            successor_access.is_err(),
            successor_refresh.is_err(),
            revoked.revoked_reason,
            revoked_refreshes,
            audit,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (
        reuse_detected,
        successor_access_denied,
        successor_refresh_denied,
        reason,
        revoked_rows,
        audit,
    ) = evidence?;

    assert!(reuse_detected);
    assert!(
        successor_access_denied,
        "reuse must kill the rotated access credential"
    );
    assert!(
        successor_refresh_denied,
        "reuse must kill the rotated refresh credential"
    );
    assert_eq!(reason.as_deref(), Some("refresh_reuse"));
    assert_eq!(revoked_rows, 2);
    assert_eq!(
        audit,
        vec![actions::CREATE, actions::ROTATE, actions::REUSE]
    );
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_reuse_while_suspended_still_revokes_the_family() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let principal = test_principal(&test_database.connection, "active").await?;
        let first = create_session::execute(&test_database.connection, principal).await?;
        let second =
            rotate_refresh::execute(&test_database.connection, &first.refresh_plaintext).await?;

        let mut suspended: principal::ActiveModel = principal::Entity::find_by_id(principal)
            .one(&test_database.connection)
            .await?
            .ok_or("principal disappeared")?
            .into();
        suspended.status = Set("suspended".to_owned());
        suspended.update(&test_database.connection).await?;

        // Reuse while suspended must still revoke, not merely deny.
        let reuse =
            rotate_refresh::execute(&test_database.connection, &first.refresh_plaintext).await;

        let mut reactivated: principal::ActiveModel = principal::Entity::find_by_id(principal)
            .one(&test_database.connection)
            .await?
            .ok_or("principal disappeared")?
            .into();
        reactivated.status = Set("active".to_owned());
        reactivated.update(&test_database.connection).await?;

        // Reactivation must not resurrect the rotated successor.
        let resurrected =
            rotate_refresh::execute(&test_database.connection, &second.refresh_plaintext).await;
        let revoked = family(&test_database.connection, first.family_id).await;

        Ok::<_, Box<dyn Error>>((
            matches!(
                reuse,
                Err(rotate_refresh::RotateRefreshError::ReuseDetected)
            ),
            resurrected.is_err(),
            revoked.revoked_reason,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (reuse_detected, resurrection_denied, reason) = evidence?;

    assert!(
        reuse_detected,
        "suspension must not shelter a reused credential"
    );
    assert!(
        resurrection_denied,
        "reactivation must not resurrect the successor"
    );
    assert_eq!(reason.as_deref(), Some("refresh_reuse"));
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_credentials_are_bound_to_their_kind() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let session = issued(&test_database.connection).await?;
        let access_as_refresh =
            rotate_refresh::execute(&test_database.connection, &session.access_plaintext).await;
        let refresh_as_access =
            validate_access::execute(&test_database.connection, &session.refresh_plaintext).await;
        // The legitimate paths still work after the failed cross-kind attempts.
        let access_valid =
            validate_access::execute(&test_database.connection, &session.access_plaintext)
                .await
                .is_ok();
        let rotated =
            rotate_refresh::execute(&test_database.connection, &session.refresh_plaintext).await;

        Ok::<_, Box<dyn Error>>((
            matches!(
                access_as_refresh,
                Err(rotate_refresh::RotateRefreshError::Unauthorized)
            ),
            matches!(
                refresh_as_access,
                Err(validate_access::ValidateAccessError::Unauthorized)
            ),
            access_valid,
            rotated.is_ok(),
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (access_rejected, refresh_rejected, access_valid, rotated) = evidence?;

    assert!(access_rejected && refresh_rejected);
    assert!(access_valid && rotated);
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_concurrent_refresh_creates_at_most_one_successor() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let first = issued(&test_database.connection).await?;
        let presented = first.refresh_plaintext.clone();
        let (left, right) = tokio::join!(
            rotate_refresh::execute(&test_database.connection, &presented),
            rotate_refresh::execute(&test_database.connection, &presented),
        );
        let outcomes = [left.is_ok(), right.is_ok()];
        let reuse_count = [left, right]
            .into_iter()
            .filter(|result| {
                matches!(
                    result,
                    Err(rotate_refresh::RotateRefreshError::ReuseDetected)
                )
            })
            .count();
        let rows = refresh_credential::Entity::find()
            .filter(refresh_credential::Column::FamilyId.eq(first.family_id))
            .count(&test_database.connection)
            .await?;
        let revoked = family(&test_database.connection, first.family_id).await;

        Ok::<_, Box<dyn Error>>((outcomes, reuse_count, rows, revoked.revoked_at.is_some()))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (outcomes, reuse_count, rows, revoked) = evidence?;

    assert_eq!(outcomes.iter().filter(|ok| **ok).count(), 1);
    assert_eq!(reuse_count, 1);
    assert_eq!(rows, 2, "exactly one successor may exist");
    assert!(revoked, "the losing refresh must revoke the family");
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_expired_refresh_and_revoke_then_refresh_are_denied() -> Result<(), Box<dyn Error>>
{
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let session = issued(&test_database.connection).await?;
        let presented_id = super::credentials::RefreshCredential::parse(&session.refresh_plaintext)
            .unwrap()
            .public_id();
        let mut expired: refresh_credential::ActiveModel =
            refresh_credential::Entity::find_by_id(presented_id)
                .one(&test_database.connection)
                .await?
                .ok_or("refresh row disappeared")?
                .into();
        expired.expires_at = Set(Utc::now() - Duration::seconds(1));
        expired.update(&test_database.connection).await?;
        let expired_outcome =
            rotate_refresh::execute(&test_database.connection, &session.refresh_plaintext).await;
        let rows_after_expiry = refresh_credential::Entity::find()
            .filter(refresh_credential::Column::FamilyId.eq(session.family_id))
            .count(&test_database.connection)
            .await?;

        let session = issued(&test_database.connection).await?;
        revoke_session_family::execute(
            &test_database.connection,
            session.family_id,
            RevocationReason::Logout,
        )
        .await?;
        let revoked_outcome =
            rotate_refresh::execute(&test_database.connection, &session.refresh_plaintext).await;
        let rows_after_revoke = refresh_credential::Entity::find()
            .filter(refresh_credential::Column::FamilyId.eq(session.family_id))
            .count(&test_database.connection)
            .await?;

        Ok::<_, Box<dyn Error>>((
            matches!(
                expired_outcome,
                Err(rotate_refresh::RotateRefreshError::Unauthorized)
            ),
            rows_after_expiry,
            matches!(
                revoked_outcome,
                Err(rotate_refresh::RotateRefreshError::Unauthorized)
            ),
            rows_after_revoke,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (expired_denied, rows_after_expiry, revoked_denied, rows_after_revoke) = evidence?;

    assert!(expired_denied && revoked_denied);
    assert_eq!(
        rows_after_expiry, 1,
        "expired refresh must create no successor"
    );
    assert_eq!(
        rows_after_revoke, 1,
        "revoked family must create no successor"
    );
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_missing_audit_storage_fails_closed_without_partial_rows()
-> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let principal = test_principal(&test_database.connection, "active").await?;
        let session = create_session::execute(&test_database.connection, principal).await?;

        let manager = SchemaManager::new(&test_database.connection);
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("security_audit_intents"))
                    .to_owned(),
            )
            .await?;

        let create_outcome = create_session::execute(&test_database.connection, principal).await;
        let rotate_outcome =
            rotate_refresh::execute(&test_database.connection, &session.refresh_plaintext).await;
        let revoke_outcome = revoke_session_family::execute(
            &test_database.connection,
            session.family_id,
            RevocationReason::Logout,
        )
        .await;
        let family_after = family(&test_database.connection, session.family_id).await;
        let successor_rows = refresh_credential::Entity::find()
            .filter(refresh_credential::Column::FamilyId.eq(session.family_id))
            .count(&test_database.connection)
            .await?;
        let family_rows = session_family::Entity::find()
            .count(&test_database.connection)
            .await?;

        Ok::<_, Box<dyn Error>>((
            create_outcome.is_err(),
            rotate_outcome.is_err(),
            revoke_outcome.is_err(),
            family_after.revoked_at.is_none(),
            family_after.access_expires_at,
            successor_rows,
            family_rows,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (
        create_failed,
        rotate_failed,
        revoke_failed,
        still_active,
        access_expiry,
        successor_rows,
        family_rows,
    ) = evidence?;

    assert!(create_failed && rotate_failed && revoke_failed);
    assert!(still_active, "revocation must roll back with audit storage");
    assert_eq!(
        successor_rows, 1,
        "rotation must roll back with audit storage"
    );
    assert_eq!(family_rows, 1, "creation must roll back with audit storage");
    assert!(access_expiry > Utc::now());
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_database_failure_is_typed_and_omits_secrets() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let principal = test_principal(&test_database.connection, "active").await?;
    let session = create_session::execute(&test_database.connection, principal).await?;
    let canary_access = session.access_plaintext.clone();
    let canary_refresh = session.refresh_plaintext.clone();
    test_database.connection.clone().close().await?;

    let create = create_session::execute(&test_database.connection, principal).await;
    let validate = validate_access::execute(&test_database.connection, &canary_access).await;
    let rotate = rotate_refresh::execute(&test_database.connection, &canary_refresh).await;
    let revoke = revoke_session_family::execute(
        &test_database.connection,
        session.family_id,
        RevocationReason::Logout,
    )
    .await;

    for message in [
        format!("{create:?}"),
        format!("{validate:?}"),
        format!("{rotate:?}"),
        format!("{revoke:?}"),
    ] {
        assert!(
            !message.contains(&canary_access),
            "error leaked access credential"
        );
        assert!(
            !message.contains(&canary_refresh),
            "error leaked refresh credential"
        );
    }
    assert!(matches!(
        create,
        Err(create_session::CreateSessionError::Persistence(_))
    ));
    assert!(matches!(
        validate,
        Err(validate_access::ValidateAccessError::Persistence(_))
    ));
    assert!(matches!(
        rotate,
        Err(rotate_refresh::RotateRefreshError::Persistence(_))
    ));
    assert!(matches!(
        revoke,
        Err(revoke_session_family::RevokeSessionError::Persistence(_))
    ));
    assert!(test_database.cleanup().await?);
    Ok(())
}

#[tokio::test]
async fn postgres_persistence_holds_hashes_never_plaintext() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let first = issued(&test_database.connection).await?;
        let second =
            rotate_refresh::execute(&test_database.connection, &first.refresh_plaintext).await?;
        let families = session_family::Entity::find()
            .all(&test_database.connection)
            .await?;
        let refreshes = refresh_credential::Entity::find()
            .all(&test_database.connection)
            .await?;
        let audits = security_audit_intent::Entity::find()
            .all(&test_database.connection)
            .await?;

        Ok::<_, Box<dyn Error>>((first, second, families, refreshes, audits))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (first, second, families, refreshes, audits) = evidence?;

    for secret in [
        &first.access_plaintext,
        &first.refresh_plaintext,
        &second.access_plaintext,
        &second.refresh_plaintext,
    ] {
        for row in families
            .iter()
            .map(|model| model.access_hash.as_str())
            .chain(refreshes.iter().map(|model| model.credential_hash.as_str()))
        {
            assert!(!row.contains(secret));
        }
        for row in audits.iter() {
            // Audit rows carry only safe identifiers; Debug must never render
            // credential material.
            let rendered = format!("{row:?}");
            assert!(!rendered.contains(secret));
            assert!(!rendered.contains("kta_") && !rendered.contains("ktr_"));
            assert_eq!(row.state, actions::PENDING);
        }
    }
    for row in families
        .iter()
        .map(|model| model.access_hash.as_str())
        .chain(refreshes.iter().map(|model| model.credential_hash.as_str()))
    {
        assert_eq!(row.len(), 64);
        assert!(row.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_refresh_and_logout_behave_through_the_router() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let session = issued(&test_database.connection).await?;
        let context = test_context(test_database.connection.clone()).await?;
        let router = session_router(context);

        // Refresh over HTTP rotates both credentials with no-store.
        let response = router
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
        let refresh_status = response.status();
        let no_store = response.headers().get(header::CACHE_CONTROL).cloned();
        let document: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await?)?;
        let rotated_access = document["accessCredential"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let rotated_refresh = document["refreshCredential"]
            .as_str()
            .unwrap_or_default()
            .to_owned();

        // Duplicate Authorization headers are rejected even when one value is
        // valid: intermediaries may forward either one. The credential is
        // still live here, so 401 proves duplicate rejection rather than
        // revocation (the single-header logout below returns 204).
        let mut duplicate_denied = Vec::new();
        for headers in [
            vec![
                format!("Bearer {rotated_access}"),
                "Bearer not-a-credential".to_owned(),
            ],
            vec![
                "Bearer not-a-credential".to_owned(),
                format!("Bearer {rotated_access}"),
            ],
        ] {
            let mut request = Request::delete("/api/v1/auth/sessions/current");
            for value in headers {
                request = request.header(header::AUTHORIZATION, value);
            }
            duplicate_denied.push(
                router
                    .clone()
                    .oneshot(request.body(Body::empty())?)
                    .await?
                    .status(),
            );
        }

        // Logout with the rotated access credential revokes the family.
        let logout = router
            .clone()
            .oneshot(
                Request::delete("/api/v1/auth/sessions/current")
                    .header(header::AUTHORIZATION, format!("Bearer {rotated_access}"))
                    .body(Body::empty())?,
            )
            .await?;
        let logout_status = logout.status();

        // Both rotated credentials now fail; logout carries WWW-Authenticate.
        let refresh_after = router
            .clone()
            .oneshot(
                Request::post("/api/v1/auth/tokens/refresh")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({ "refreshCredential": rotated_refresh }).to_string(),
                    ))?,
            )
            .await?;
        let refresh_after_status = refresh_after.status();
        let logout_after = router
            .clone()
            .oneshot(
                Request::delete("/api/v1/auth/sessions/current")
                    .header(header::AUTHORIZATION, format!("Bearer {rotated_access}"))
                    .body(Body::empty())?,
            )
            .await?;
        let logout_after_status = logout_after.status();
        let www_authenticate = logout_after
            .headers()
            .get(header::WWW_AUTHENTICATE)
            .cloned();

        // Missing, malformed, cookie-only, and query-string credentials fail.
        let mut denied = Vec::new();
        denied.push(
            router
                .clone()
                .oneshot(Request::delete("/api/v1/auth/sessions/current").body(Body::empty())?)
                .await?
                .status(),
        );
        denied.push(
            router
                .clone()
                .oneshot(
                    Request::delete("/api/v1/auth/sessions/current")
                        .header(header::AUTHORIZATION, "Bearer not-a-credential")
                        .body(Body::empty())?,
                )
                .await?
                .status(),
        );
        denied.push(
            router
                .clone()
                .oneshot(
                    Request::delete("/api/v1/auth/sessions/current")
                        .header(header::COOKIE, format!("session={rotated_access}"))
                        .body(Body::empty())?,
                )
                .await?
                .status(),
        );
        // Duplicate Authorization headers are covered above while the
        // credential is live.
        denied.push(
            router
                .clone()
                .oneshot(
                    Request::delete(format!(
                        "/api/v1/auth/sessions/current?accessCredential={rotated_access}"
                    ))
                    .body(Body::empty())?,
                )
                .await?
                .status(),
        );

        Ok::<_, Box<dyn Error>>((
            refresh_status,
            no_store,
            rotated_access,
            rotated_refresh,
            duplicate_denied,
            logout_status,
            refresh_after_status,
            logout_after_status,
            www_authenticate,
            denied,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (
        refresh_status,
        no_store,
        rotated_access,
        rotated_refresh,
        duplicate_denied,
        logout_status,
        refresh_after_status,
        logout_after_status,
        www_authenticate,
        denied,
    ) = evidence?;

    assert_eq!(refresh_status, StatusCode::OK);
    assert_eq!(no_store, Some(HeaderValue::from_static("no-store")));
    assert!(rotated_access.starts_with("kta_") && rotated_refresh.starts_with("ktr_"));
    assert!(
        duplicate_denied
            .into_iter()
            .all(|status| status == StatusCode::UNAUTHORIZED),
        "duplicate Authorization headers must be rejected while the credential is live"
    );
    assert_eq!(logout_status, StatusCode::NO_CONTENT);
    assert_eq!(refresh_after_status, StatusCode::UNAUTHORIZED);
    assert_eq!(logout_after_status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        www_authenticate,
        Some(HeaderValue::from_static("Bearer error=\"invalid_token\""))
    );
    assert!(
        denied
            .into_iter()
            .all(|status| status == StatusCode::UNAUTHORIZED)
    );
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_session_openapi_registers_auth_routes_without_secrets()
-> Result<(), Box<dyn Error>> {
    use utoipa::OpenApi;

    let document = serde_json::to_value(crate::controllers::status::ApiDoc::openapi())?;
    for path in [
        "/api/v1/auth/tokens/refresh",
        "/api/v1/auth/sessions/current",
        "/api/v1/auth/github/login-attempts",
        "/api/v1/auth/github/callback",
        "/api/v1/auth/github/login-attempts/{attempt_id}/redeem",
    ] {
        assert!(
            document["paths"].get(path).is_some(),
            "{path} is not documented"
        );
    }
    let serialized = serde_json::to_string(&document)?;
    assert!(!serialized.contains("kta_") && !serialized.contains("ktr_"));
    assert!(!serialized.contains("secret-canary"));
    // Kontainer credentials travel in the JSON body or the Authorization
    // header only. The GitHub callback is the single credential-adjacent
    // exception: its query parameters carry only the provider `code`, opaque
    // `state`, and provider `error` signal, never a Kontainer credential or
    // verifier. Bounded pagination cursors are the only other query transport.
    assert!(
        !serialized.contains("\"in\":\"cookie\""),
        "no cookie credential transport"
    );
    {
        let mut violations = Vec::new();
        for (path, methods) in document["paths"].as_object().unwrap() {
            for (method, operation) in methods.as_object().unwrap() {
                for parameter in operation
                    .get("parameters")
                    .and_then(|parameters| parameters.as_array())
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
                {
                    if parameter.get("in") == Some(&serde_json::json!("query")) {
                        let name = parameter.get("name").cloned().unwrap_or_default();
                        let name = name.as_str().unwrap_or_default();
                        let allowed = (path == "/api/v1/auth/github/callback"
                            && ["code", "state", "error"].contains(&name))
                            || (path == "/api/v1/containers" && name == "node")
                            || (path == "/api/v1/volumes" && ["node", "name"].contains(&name))
                            || (["/api/v1/nodes/{nodeId}/samples", "/api/v1/cluster/samples"]
                                .contains(&path.as_str())
                                && ["window", "bucket"].contains(&name))
                            || (path == "/api/v1/operations/{operation_id}/events"
                                && ["after", "limit"].contains(&name));
                        if !allowed {
                            violations.push(format!("{method} {path} query {name}"));
                        }
                    }
                }
            }
        }
        assert!(
            violations.is_empty(),
            "unexpected query transport: {violations:?}"
        );
    }

    // Opaque access-session security is the default; probes and the refresh
    // lifecycle explicitly opt out.
    assert_eq!(
        document["components"]["securitySchemes"]["access_session"]["type"],
        "http"
    );
    assert_eq!(
        document["components"]["securitySchemes"]["access_session"]["scheme"],
        "bearer"
    );
    assert_eq!(
        document["components"]["securitySchemes"]["access_session"]["bearerFormat"],
        "opaque"
    );
    assert!(
        document["security"]
            .as_array()
            .is_some_and(|security| security
                .iter()
                .any(|requirement| requirement.get("access_session").is_some())),
        "protected operations inherit access-session security"
    );
    for path in ["/health/live", "/health/startup", "/health/ready"] {
        let operation = &document["paths"][path]["get"];
        assert_eq!(
            operation.get("security"),
            Some(&serde_json::json!([{}])),
            "{path} is an anonymous probe"
        );
    }
    assert_eq!(
        document["paths"]["/api/v1/auth/tokens/refresh"]["post"].get("security"),
        Some(&serde_json::json!([{}])),
        "refresh uses its own refresh credential, not the access session"
    );
    // GitHub lifecycle routes validate their own attempt credentials.
    for (path, method) in [
        ("/api/v1/auth/github/login-attempts", "post"),
        ("/api/v1/auth/github/callback", "get"),
        (
            "/api/v1/auth/github/login-attempts/{attempt_id}/redeem",
            "post",
        ),
    ] {
        assert_eq!(
            document["paths"][path][method].get("security"),
            Some(&serde_json::json!([{}])),
            "{method} {path} is an anonymous lifecycle route"
        );
    }
    // Logout and polling inherit the default; they document their own 401.
    assert!(
        document["paths"]["/api/v1/auth/sessions/current"]["delete"]
            .get("security")
            .is_none()
    );
    assert!(
        document["paths"]["/api/v1/operations/{operation_id}"]["get"]
            .get("security")
            .is_none()
    );
    Ok(())
}

/// Minimal guarded route proving the middleware/extractor contract: the
/// handler observes only the persisted principal, never request input.
fn guarded_probe(context: AppContext, calls: Arc<AtomicUsize>) -> Router {
    Router::new()
        .route(
            "/probe",
            get(move |principal: PrincipalContext| {
                calls.fetch_add(1, Ordering::SeqCst);
                let id = principal.principal_id();
                async move { id.to_string() }
            }),
        )
        .layer(middleware::from_fn_with_state(
            context.db.clone(),
            require_access_session,
        ))
        .with_state(context)
}

#[tokio::test]
async fn postgres_access_middleware_extracts_the_persisted_principal() -> Result<(), Box<dyn Error>>
{
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let session = issued(&test_database.connection).await?;
        let context = test_context(test_database.connection.clone()).await?;
        let calls = Arc::new(AtomicUsize::new(0));
        let router = guarded_probe(context, calls.clone());

        // Valid access reaches the handler with the persisted principal ID.
        // Spoofed query, cookie, and extra header input cannot change it.
        let valid = router
            .clone()
            .oneshot(
                Request::get(format!(
                    "/probe?principal_id={}&accessCredential={}",
                    uuid::Uuid::now_v7(),
                    session.access_plaintext
                ))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", session.access_plaintext),
                )
                .header(header::COOKIE, "principal_id=forged")
                .header("x-principal-id", session.principal_id.to_string())
                .body(Body::empty())?,
            )
            .await?;
        let valid_status = valid.status();
        let valid_body = to_bytes(valid.into_body(), 256).await?;

        // Header variants never reach the handler and share one challenge.
        let mut denied = Vec::new();
        denied.push(
            router
                .clone()
                .oneshot(Request::get("/probe").body(Body::empty())?)
                .await?,
        );
        denied.push(
            router
                .clone()
                .oneshot(
                    Request::get("/probe")
                        .header(header::AUTHORIZATION, "Basic abc")
                        .body(Body::empty())?,
                )
                .await?,
        );
        let mut duplicated = Request::get("/probe");
        duplicated = duplicated.header(
            header::AUTHORIZATION,
            format!("Bearer {}", session.access_plaintext),
        );
        duplicated = duplicated.header(header::AUTHORIZATION, "Bearer not-a-credential");
        denied.push(
            router
                .clone()
                .oneshot(duplicated.body(Body::empty())?)
                .await?,
        );
        let denied_statuses: Vec<StatusCode> =
            denied.iter().map(|response| response.status()).collect();
        let challenges: Vec<Option<HeaderValue>> = denied
            .iter()
            .map(|response| response.headers().get(header::WWW_AUTHENTICATE).cloned())
            .collect();

        Ok::<_, Box<dyn Error>>((
            valid_status,
            valid_body,
            session.principal_id,
            denied_statuses,
            challenges,
            calls.load(Ordering::SeqCst),
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (valid_status, valid_body, principal_id, denied_statuses, challenges, handler_calls) =
        evidence?;

    assert_eq!(valid_status, StatusCode::OK);
    assert_eq!(valid_body.as_ref(), principal_id.to_string().as_bytes());
    assert!(
        denied_statuses
            .into_iter()
            .all(|status| status == StatusCode::UNAUTHORIZED)
    );
    assert!(
        challenges.into_iter().all(|challenge| challenge
            == Some(HeaderValue::from_static("Bearer error=\"invalid_token\"")))
    );
    assert_eq!(
        handler_calls, 1,
        "denied requests must not invoke the handler"
    );
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_access_middleware_denies_closed_sessions_without_the_handler()
-> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let principal = test_principal(&test_database.connection, "active").await?;
        let session = create_session::execute(&test_database.connection, principal).await?;
        revoke_session_family::execute(
            &test_database.connection,
            session.family_id,
            RevocationReason::Logout,
        )
        .await?;
        let context = test_context(test_database.connection.clone()).await?;
        let calls = Arc::new(AtomicUsize::new(0));
        let router = guarded_probe(context, calls.clone());

        let revoked = router
            .clone()
            .oneshot(
                Request::get("/probe")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", session.access_plaintext),
                    )
                    .body(Body::empty())?,
            )
            .await?
            .status();

        // A well-formed credential against an unavailable database fails as
        // an outage, not an authentication decision.
        test_database.connection.clone().close().await?;
        let unavailable = router
            .clone()
            .oneshot(
                Request::get("/probe")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", session.access_plaintext),
                    )
                    .body(Body::empty())?,
            )
            .await?;
        let unavailable_status = unavailable.status();

        Ok::<_, Box<dyn Error>>((revoked, unavailable_status, calls.load(Ordering::SeqCst)))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (revoked, unavailable_status, handler_calls) = evidence?;

    assert_eq!(revoked, StatusCode::UNAUTHORIZED);
    assert_eq!(unavailable_status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(handler_calls, 0, "no denied request may invoke the handler");
    assert!(removed);
    Ok(())
}

// ---------------------------------------------------------------------------
// #79 GitHub bootstrap login.
// ---------------------------------------------------------------------------

const GITHUB_TEST_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const GITHUB_TEST_ORIGIN: &str = "http://127.0.0.1:5151";

/// Distinct valid verifiers per attempt: challenges are unique per row.
fn verifier_for(n: u8) -> String {
    format!("{}-{n:02}", &GITHUB_TEST_VERIFIER[..40])
}

fn test_service(allowlisted: &[i64], provider: FakeGitHubProvider) -> GitHubAuthService {
    GitHubAuthService {
        config: GitHubLoginConfig {
            enabled: true,
            client_id: "test-client".to_owned(),
            allowlisted: allowlisted.iter().copied().collect(),
            authorize_url: GITHUB_AUTHORIZE_URL.to_owned(),
            token_url: GITHUB_TOKEN_URL.to_owned(),
            user_url: GITHUB_USER_URL.to_owned(),
        },
        provider: Arc::new(provider),
    }
}

async fn login_router(
    connection: &sea_orm::DatabaseConnection,
    service: GitHubAuthService,
) -> Result<Router, Box<dyn Error>> {
    let context = test_context(connection.clone()).await?;
    context.shared_store.insert(service);
    Ok(App::routes(&context).to_router::<App>(context, Router::new())?)
}

fn authorize_state(url: &str) -> String {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned()
}

async fn full_login(
    database: &sea_orm::DatabaseConnection,
    service: &GitHubAuthService,
    origin: &str,
    verifier: String,
    code: &str,
) -> Result<redeem_github_login::RedeemedLogin, Box<dyn Error>> {
    let challenge = github::s256_challenge(&verifier);
    let started = start_github_login::execute(database, service, origin, &challenge).await?;
    accept_github_callback::execute(
        database,
        &authorize_state(&started.authorize_url),
        Some(code),
        None,
    )
    .await?;
    redeem_github_login::execute(database, service, origin, started.attempt_id, &verifier)
        .await
        .map_err(|error| -> Box<dyn Error> { format!("{error:?}").into() })
}

async fn login_counts(
    database: &sea_orm::DatabaseConnection,
) -> Result<(u64, u64, u64, u64, Option<Uuid>), sea_orm::DbErr> {
    Ok((
        principal::Entity::find().count(database).await?,
        github_login_attempt::Entity::find().count(database).await?,
        github_identity::Entity::find().count(database).await?,
        session_family::Entity::find().count(database).await?,
        bootstrap_state::Entity::find_by_id(1)
            .one(database)
            .await?
            .and_then(|state| state.owner_principal_id),
    ))
}

#[tokio::test]
async fn postgres_github_migration_seeds_singleton_bootstrap() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let manager = SchemaManager::new(&test_database.connection);
        let tables = [
            manager.has_table("github_login_attempts").await?,
            manager.has_table("github_identities").await?,
            manager.has_table("bootstrap_state").await?,
        ];
        let singleton = bootstrap_state::Entity::find_by_id(1)
            .one(&test_database.connection)
            .await?
            .ok_or_else(|| io::Error::other("bootstrap singleton is missing"))?;

        // A second singleton row violates the one-row invariant.
        let duplicate = bootstrap_state::ActiveModel {
            id: Set(2),
            owner_principal_id: Set(None),
            claimed_at: Set(None),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        }
        .insert(&test_database.connection)
        .await
        .is_err();

        // Claiming without a timestamp violates the consistency check.
        let owner = test_principal(&test_database.connection, "active").await?;
        let mut inconsistent: bootstrap_state::ActiveModel = singleton.clone().into();
        inconsistent.owner_principal_id = Set(Some(owner));
        inconsistent.claimed_at = Set(None);
        let inconsistent_rejected = inconsistent
            .update(&test_database.connection)
            .await
            .is_err();
        let unchanged = bootstrap_state::Entity::find_by_id(1)
            .one(&test_database.connection)
            .await?
            .ok_or_else(|| io::Error::other("bootstrap singleton disappeared"))?;

        Ok::<_, Box<dyn Error>>((
            tables,
            singleton,
            duplicate,
            inconsistent_rejected,
            unchanged,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (tables, singleton, duplicate, inconsistent_rejected, unchanged) = evidence?;

    assert!(tables.into_iter().all(|exists| exists));
    assert_eq!(singleton.id, 1);
    assert_eq!(singleton.owner_principal_id, None);
    assert_eq!(singleton.claimed_at, None);
    assert!(duplicate, "a second bootstrap row must be rejected");
    assert!(
        inconsistent_rejected,
        "owner without claimed_at must be rejected"
    );
    assert_eq!(unchanged.owner_principal_id, None);
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_github_login_attempt_state_machine() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let disabled = test_service(&[11], FakeGitHubProvider::succeed(11, "owner"));
        let mut disabled = disabled;
        disabled.config.enabled = false;
        let service = test_service(&[11], FakeGitHubProvider::succeed(11, "owner"));

        let disabled_start = start_github_login::execute(
            &test_database.connection,
            &disabled,
            GITHUB_TEST_ORIGIN,
            "x",
        )
        .await;
        let bad_challenge = start_github_login::execute(
            &test_database.connection,
            &service,
            GITHUB_TEST_ORIGIN,
            "too-short",
        )
        .await;

        let challenge = github::s256_challenge(&verifier_for(1));
        let started = start_github_login::execute(
            &test_database.connection,
            &service,
            GITHUB_TEST_ORIGIN,
            &challenge,
        )
        .await?;
        let state = authorize_state(&started.authorize_url);
        let row = github_login_attempt::Entity::find_by_id(started.attempt_id)
            .one(&test_database.connection)
            .await?
            .ok_or_else(|| io::Error::other("attempt was not persisted"))?;

        // Unknown states share the denial path without touching storage.
        let unknown = accept_github_callback::execute(
            &test_database.connection,
            &github::s256_challenge(&verifier_for(9)),
            Some("some-code"),
            None,
        )
        .await?;

        // Provider denial consumes the attempt terminally.
        let denied_attempt = start_github_login::execute(
            &test_database.connection,
            &service,
            GITHUB_TEST_ORIGIN,
            &github::s256_challenge(&verifier_for(2)),
        )
        .await?;
        let denied_state = authorize_state(&denied_attempt.authorize_url);
        let provider_denied = accept_github_callback::execute(
            &test_database.connection,
            &denied_state,
            None,
            Some("access_denied"),
        )
        .await?;
        let denied_row = github_login_attempt::Entity::find_by_id(denied_attempt.attempt_id)
            .one(&test_database.connection)
            .await?
            .ok_or_else(|| io::Error::other("denied attempt disappeared"))?;

        // A missing code consumes the attempt; the replay then denies.
        let missing_attempt = start_github_login::execute(
            &test_database.connection,
            &service,
            GITHUB_TEST_ORIGIN,
            &github::s256_challenge(&verifier_for(3)),
        )
        .await?;
        let missing_state = authorize_state(&missing_attempt.authorize_url);
        let missing_code =
            accept_github_callback::execute(&test_database.connection, &missing_state, None, None)
                .await?;
        let replay_missing = accept_github_callback::execute(
            &test_database.connection,
            &missing_state,
            Some("late-code"),
            None,
        )
        .await?;

        // The ready attempt rejects replays, wrong verifiers, and unknown
        // attempt IDs while staying redeemable with the true verifier.
        let ready = accept_github_callback::execute(
            &test_database.connection,
            &state,
            Some("provider-code-1"),
            None,
        )
        .await?;
        let replay_ready = accept_github_callback::execute(
            &test_database.connection,
            &state,
            Some("provider-code-1"),
            None,
        )
        .await?;
        let wrong_verifier = redeem_github_login::execute(
            &test_database.connection,
            &service,
            GITHUB_TEST_ORIGIN,
            started.attempt_id,
            &verifier_for(4),
        )
        .await;
        let malformed_verifier = redeem_github_login::execute(
            &test_database.connection,
            &service,
            GITHUB_TEST_ORIGIN,
            started.attempt_id,
            "short",
        )
        .await;
        let unknown_attempt = redeem_github_login::execute(
            &test_database.connection,
            &service,
            GITHUB_TEST_ORIGIN,
            Uuid::now_v7(),
            &verifier_for(1),
        )
        .await;
        let still_ready = github_login_attempt::Entity::find_by_id(started.attempt_id)
            .one(&test_database.connection)
            .await?
            .ok_or_else(|| io::Error::other("ready attempt disappeared"))?;

        // An expired attempt denies at both steps.
        let expiring = start_github_login::execute(
            &test_database.connection,
            &service,
            GITHUB_TEST_ORIGIN,
            &github::s256_challenge(&verifier_for(5)),
        )
        .await?;
        let mut expired: github_login_attempt::ActiveModel =
            github_login_attempt::Entity::find_by_id(expiring.attempt_id)
                .one(&test_database.connection)
                .await?
                .ok_or_else(|| io::Error::other("expiring attempt disappeared"))?
                .into();
        expired.expires_at = Set(Utc::now() - chrono::Duration::seconds(1));
        expired.update(&test_database.connection).await?;
        let expired_state = authorize_state(&expiring.authorize_url);
        let expired_callback = accept_github_callback::execute(
            &test_database.connection,
            &expired_state,
            Some("provider-code-5"),
            None,
        )
        .await?;
        let expired_redeem = redeem_github_login::execute(
            &test_database.connection,
            &service,
            GITHUB_TEST_ORIGIN,
            expiring.attempt_id,
            &verifier_for(5),
        )
        .await;

        Ok::<_, Box<dyn Error>>((
            disabled_start,
            bad_challenge,
            started,
            state,
            row,
            unknown,
            provider_denied,
            denied_row,
            missing_code,
            replay_missing,
            ready,
            replay_ready,
            wrong_verifier,
            malformed_verifier,
            unknown_attempt,
            still_ready,
            expired_callback,
            expired_redeem,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (
        disabled_start,
        bad_challenge,
        started,
        state,
        row,
        unknown,
        provider_denied,
        denied_row,
        missing_code,
        replay_missing,
        ready,
        replay_ready,
        wrong_verifier,
        malformed_verifier,
        unknown_attempt,
        still_ready,
        expired_callback,
        expired_redeem,
    ) = evidence?;

    assert!(matches!(
        disabled_start,
        Err(start_github_login::StartLoginError::Disabled)
    ));
    assert!(matches!(
        bad_challenge,
        Err(start_github_login::StartLoginError::InvalidChallenge)
    ));
    assert!(!started.authorize_url.contains("code_verifier"));
    assert!(started.authorize_url.contains("code_challenge_method=S256"));
    assert_eq!(row.status, "pending");
    assert_eq!(row.provider_code, None);
    assert!(
        !row.state_hash.contains(&state),
        "only the state hash is persisted"
    );
    assert_eq!(unknown, accept_github_callback::CallbackOutcome::Denied);
    assert_eq!(
        provider_denied,
        accept_github_callback::CallbackOutcome::Denied
    );
    assert_eq!(denied_row.status, "consumed");
    assert_eq!(
        missing_code,
        accept_github_callback::CallbackOutcome::Denied
    );
    assert_eq!(
        replay_missing,
        accept_github_callback::CallbackOutcome::Denied
    );
    assert!(matches!(
        ready,
        accept_github_callback::CallbackOutcome::Ready { .. }
    ));
    assert_eq!(
        replay_ready,
        accept_github_callback::CallbackOutcome::Denied
    );
    assert!(matches!(
        wrong_verifier,
        Err(redeem_github_login::RedeemError::Denied)
    ));
    assert!(matches!(
        malformed_verifier,
        Err(redeem_github_login::RedeemError::Denied)
    ));
    assert!(matches!(
        unknown_attempt,
        Err(redeem_github_login::RedeemError::Denied)
    ));
    assert_eq!(still_ready.status, "ready");
    assert_eq!(
        still_ready.provider_code.as_deref(),
        Some("provider-code-1")
    );
    assert_eq!(
        expired_callback,
        accept_github_callback::CallbackOutcome::Denied
    );
    assert!(matches!(
        expired_redeem,
        Err(redeem_github_login::RedeemError::Denied)
    ));
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_github_bootstrap_claims_single_owner() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let provider = FakeGitHubProvider::succeed(11, "owner-a")
            .route("code-a", 11, "owner-a")
            .route("code-b", 22, "owner-b");
        let service = test_service(&[11, 22], provider);
        let origin = GITHUB_TEST_ORIGIN;

        let challenge_a = github::s256_challenge(&verifier_for(11));
        let challenge_b = github::s256_challenge(&verifier_for(12));
        let attempt_a =
            start_github_login::execute(&test_database.connection, &service, origin, &challenge_a)
                .await?;
        let attempt_b =
            start_github_login::execute(&test_database.connection, &service, origin, &challenge_b)
                .await?;
        accept_github_callback::execute(
            &test_database.connection,
            &authorize_state(&attempt_a.authorize_url),
            Some("code-a"),
            None,
        )
        .await?;
        accept_github_callback::execute(
            &test_database.connection,
            &authorize_state(&attempt_b.authorize_url),
            Some("code-b"),
            None,
        )
        .await?;

        // Concurrent first logins race on the singleton row: exactly one
        // transaction observes the unclaimed state.
        let verifier_a = verifier_for(11);
        let verifier_b = verifier_for(12);
        let (first, second) = tokio::join!(
            redeem_github_login::execute(
                &test_database.connection,
                &service,
                origin,
                attempt_a.attempt_id,
                &verifier_a,
            ),
            redeem_github_login::execute(
                &test_database.connection,
                &service,
                origin,
                attempt_b.attempt_id,
                &verifier_b,
            ),
        );
        let (winner, loser) = match (first, second) {
            (Ok(winner), Err(redeem_github_login::RedeemError::Denied)) => (winner, 22),
            (Err(redeem_github_login::RedeemError::Denied), Ok(winner)) => (winner, 11),
            (first, second) => {
                return Err(io::Error::other(format!(
                    "bootstrap race must elect one owner, got {first:?} and {second:?}"
                ))
                .into());
            }
        };
        let counts_after_race = login_counts(&test_database.connection).await?;

        // The losing identity cannot bootstrap afterwards, even though it
        // is allowlisted: bootstrap never grants a second principal.
        let challenge_c = github::s256_challenge(&verifier_for(13));
        let attempt_c =
            start_github_login::execute(&test_database.connection, &service, origin, &challenge_c)
                .await?;
        accept_github_callback::execute(
            &test_database.connection,
            &authorize_state(&attempt_c.authorize_url),
            Some(if loser == 22 { "code-b" } else { "code-a" }),
            None,
        )
        .await?;
        // A fresh verifier proves the denial is about ownership, not replay.
        let denied = redeem_github_login::execute(
            &test_database.connection,
            &service,
            origin,
            attempt_c.attempt_id,
            &verifier_for(13),
        )
        .await;
        let counts_after_denial = login_counts(&test_database.connection).await?;

        Ok::<_, Box<dyn Error>>((
            winner,
            loser,
            counts_after_race,
            denied,
            counts_after_denial,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (winner, loser, counts_after_race, denied, counts_after_denial) = evidence?;

    assert!(winner.bootstrap_claimed);
    assert!(loser == 11 || loser == 22);
    assert_ne!(winner.principal_id, Uuid::nil());
    let (principals, _attempts, identities, families, owner) = counts_after_race;
    assert_eq!((principals, identities, families), (1, 1, 1));
    assert_eq!(owner, Some(winner.principal_id));
    assert!(matches!(
        denied,
        Err(redeem_github_login::RedeemError::Denied)
    ));
    // The denied bootstrap attempt consumed itself without creating rows.
    assert_eq!(counts_after_denial.0, 1, "no second principal");
    assert_eq!(counts_after_denial.2, 1, "no second identity");
    assert_eq!(counts_after_denial.3, 1, "no second session");
    assert_eq!(counts_after_denial.4, Some(winner.principal_id));
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_established_owner_relogs_in() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let service = test_service(&[11], FakeGitHubProvider::succeed(11, "owner"));
        let origin = GITHUB_TEST_ORIGIN;

        let code_canary = "provider-code-canary-bootstrap";
        let first = full_login(
            &test_database.connection,
            &service,
            origin,
            verifier_for(21),
            code_canary,
        )
        .await?;
        // The provider reports a new display login; identity stays the
        // numeric account.
        let relogin_service = test_service(&[11], FakeGitHubProvider::succeed(11, "owner-v2"));
        let relogin = full_login(
            &test_database.connection,
            &relogin_service,
            origin,
            verifier_for(22),
            "provider-code-canary-relogin",
        )
        .await?;
        let identity = github_identity::Entity::find_by_id(11)
            .one(&test_database.connection)
            .await?
            .ok_or_else(|| io::Error::other("github identity is missing"))?;

        // Suspension denies new logins without deleting the binding.
        let mut suspended: principal::ActiveModel =
            principal::Entity::find_by_id(first.principal_id)
                .one(&test_database.connection)
                .await?
                .ok_or_else(|| io::Error::other("bootstrap principal is missing"))?
                .into();
        suspended.status = Set("suspended".to_owned());
        suspended.update(&test_database.connection).await?;
        let suspended_login = full_login(
            &test_database.connection,
            &service,
            origin,
            verifier_for(23),
            "provider-code-canary-suspended",
        )
        .await;

        // Provider codes never persist: every attempt cleared its code and
        // no persisted field or Debug output holds a canary.
        let attempts = github_login_attempt::Entity::find()
            .all(&test_database.connection)
            .await?;
        let codes_cleared = attempts
            .iter()
            .all(|attempt| attempt.provider_code.is_none());
        let debug_clean = attempts
            .iter()
            .all(|attempt| !format!("{attempt:?}").contains("canary"));
        let counts = login_counts(&test_database.connection).await?;

        Ok::<_, Box<dyn Error>>((
            first,
            relogin,
            identity,
            suspended_login,
            codes_cleared,
            debug_clean,
            counts,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (first, relogin, identity, suspended_login, codes_cleared, debug_clean, counts) = evidence?;

    assert!(first.bootstrap_claimed);
    assert!(!relogin.bootstrap_claimed);
    assert_eq!(relogin.principal_id, first.principal_id);
    assert_ne!(
        relogin.issued.access_plaintext,
        first.issued.access_plaintext
    );
    assert_eq!(relogin.github_login, "owner-v2");
    assert_eq!(identity.principal_id, first.principal_id);
    assert_eq!(identity.login, "owner-v2");
    assert!(
        suspended_login.is_err(),
        "a suspended Owner must not receive a session"
    );
    assert!(
        codes_cleared,
        "provider codes must be cleared at redemption"
    );
    assert!(debug_clean, "attempt Debug must redact provider codes");
    assert_eq!(counts.0, 1, "relogin must reuse the principal");
    assert_eq!(counts.2, 1, "relogin must reuse the identity");
    assert_eq!(counts.3, 2, "relogin issues a second session family");
    assert_eq!(counts.4, Some(first.principal_id));
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_github_redemption_maps_provider_and_allowlist_failures()
-> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let origin = GITHUB_TEST_ORIGIN;
        // Provider denial consumes the attempt without creating rows.
        let denied_service = test_service(
            &[11],
            FakeGitHubProvider::fail(crate::auth::github::GitHubProviderError::Denied),
        );
        let denied = full_login(
            &test_database.connection,
            &denied_service,
            origin,
            verifier_for(51),
            "provider-code-denied",
        )
        .await;
        let counts_after_denial = login_counts(&test_database.connection).await?;

        // Provider outage surfaces as an outage, not a denial.
        let outage_service = test_service(
            &[11],
            FakeGitHubProvider::fail(crate::auth::github::GitHubProviderError::Unavailable),
        );
        let outage = full_login(
            &test_database.connection,
            &outage_service,
            origin,
            verifier_for(52),
            "provider-code-outage",
        )
        .await;
        let counts_after_outage = login_counts(&test_database.connection).await?;

        // An authenticated but non-allowlisted identity is denied even
        // though the provider exchange succeeded.
        let stranger_service = test_service(&[11], FakeGitHubProvider::succeed(22, "stranger"));
        let stranger = full_login(
            &test_database.connection,
            &stranger_service,
            origin,
            verifier_for(53),
            "provider-code-stranger",
        )
        .await;
        let counts_after_stranger = login_counts(&test_database.connection).await?;

        Ok::<_, Box<dyn Error>>((
            denied,
            counts_after_denial,
            outage,
            counts_after_outage,
            stranger,
            counts_after_stranger,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (denied, counts_after_denial, outage, counts_after_outage, stranger, counts_after_stranger) =
        evidence?;

    assert!(
        denied.unwrap_err().to_string().contains("Denied"),
        "provider denial must deny redemption"
    );
    assert_eq!(
        counts_after_denial.0, 0,
        "denial must not create a principal"
    );
    assert_eq!(
        counts_after_denial.4, None,
        "denial must not claim bootstrap"
    );
    assert!(
        outage
            .unwrap_err()
            .to_string()
            .contains("ProviderUnavailable"),
        "provider outage must surface as unavailable"
    );
    assert_eq!(counts_after_outage.0, 0);
    assert!(
        stranger.unwrap_err().to_string().contains("Denied"),
        "a non-allowlisted identity must be denied"
    );
    assert_eq!(counts_after_stranger.0, 0);
    assert_eq!(counts_after_stranger.2, 0);
    assert_eq!(counts_after_stranger.4, None);
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_github_adapter_exchanges_before_user_lookup() -> Result<(), Box<dyn Error>> {
    use crate::auth::github::GitHubIdentityProvider;
    use axum::extract::State;
    use axum::response::IntoResponse;

    const TOKEN_CANARY: &str = "stub-access-token-canary";
    const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

    #[derive(Default)]
    struct Calls {
        token_bodies: Vec<String>,
        user_authorization: Vec<String>,
        user_uris: Vec<String>,
        order: Vec<&'static str>,
    }

    #[derive(Clone)]
    struct Stub {
        calls: Arc<Mutex<Calls>>,
        token_mode: Arc<Mutex<String>>,
        user_mode: Arc<Mutex<String>>,
    }

    async fn token(State(stub): State<Stub>, body: String) -> axum::response::Response {
        let mut calls = stub.calls.lock().unwrap();
        calls.order.push("token");
        calls.token_bodies.push(body);
        match stub.token_mode.lock().unwrap().as_str() {
            "redirect" => (
                StatusCode::FOUND,
                [(header::LOCATION, "http://127.0.0.1:9/capture")],
                String::new(),
            )
                .into_response(),
            "unavailable" => (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({"message": "boom"})),
            )
                .into_response(),
            "denied" => axum::Json(serde_json::json!({
                "error": "incorrect_client_credentials",
            }))
            .into_response(),
            _ => axum::Json(serde_json::json!({
                "access_token": TOKEN_CANARY,
                "token_type": "bearer",
            }))
            .into_response(),
        }
    }

    async fn user(
        State(stub): State<Stub>,
        request: axum::extract::Request,
    ) -> axum::response::Response {
        let mut calls = stub.calls.lock().unwrap();
        calls.order.push("user");
        calls.user_authorization.push(
            request
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned(),
        );
        calls.user_uris.push(request.uri().to_string());
        if stub.user_mode.lock().unwrap().as_str() == "oversized" {
            return axum::http::Response::builder()
                .header("content-type", "application/json")
                .body(axum::body::Body::from(vec![b'x'; 70 * 1024]))
                .unwrap();
        }
        axum::Json(serde_json::json!({"id": 99, "login": "stub-user"})).into_response()
    }

    let stub = Stub {
        calls: Arc::new(Mutex::new(Calls::default())),
        token_mode: Arc::new(Mutex::new("ok".to_owned())),
        user_mode: Arc::new(Mutex::new("ok".to_owned())),
    };
    let router = Router::new()
        .route("/login/oauth/access_token", post(token))
        .route("/user", get(user))
        .with_state(stub.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    tokio::spawn(async move { axum::serve(listener, router.into_make_service()).await });

    let provider = crate::auth::github::ReqwestGitHubProvider::endpoint_for_tests(
        "test-client".to_owned(),
        crate::auth::github::ClientSecret::new("stub-secret".to_owned()),
        &base,
    )?;
    let account = provider
        .resolve_identity(
            "stub-code",
            VERIFIER,
            "http://127.0.0.1:5151/api/v1/auth/github/callback",
        )
        .await?;

    {
        let calls = stub.calls.lock().unwrap();
        assert_eq!(account.account_id, 99);
        assert_eq!(account.login, "stub-user");
        assert_eq!(calls.order, vec!["token", "user"]);
        assert_eq!(calls.token_bodies.len(), 1);
        let form: std::collections::HashMap<String, String> =
            url::form_urlencoded::parse(calls.token_bodies[0].as_bytes())
                .into_owned()
                .collect();
        assert_eq!(form.get("code").map(String::as_str), Some("stub-code"));
        assert_eq!(
            form.get("code_verifier").map(String::as_str),
            Some(VERIFIER)
        );
        assert_eq!(
            form.get("redirect_uri").map(String::as_str),
            Some("http://127.0.0.1:5151/api/v1/auth/github/callback")
        );
        assert!(!calls.token_bodies[0].contains(TOKEN_CANARY));
        assert_eq!(
            calls.user_authorization,
            vec![format!("Bearer {TOKEN_CANARY}")]
        );
        assert!(
            calls.user_uris.iter().all(|uri| !uri.contains('?')),
            "identity lookup sends no query parameters"
        );
    }

    // Redirects are disabled: a 302 never becomes an exchange.
    *stub.token_mode.lock().unwrap() = "redirect".to_owned();
    let redirected = provider
        .resolve_identity(
            "stub-code",
            VERIFIER,
            "http://127.0.0.1:5151/api/v1/auth/github/callback",
        )
        .await;
    assert_eq!(
        redirected.unwrap_err(),
        crate::auth::github::GitHubProviderError::Denied
    );

    // Provider denials and outages map to stable errors that never carry
    // the access token.
    *stub.token_mode.lock().unwrap() = "denied".to_owned();
    let denied = provider
        .resolve_identity(
            "stub-code",
            VERIFIER,
            "http://127.0.0.1:5151/api/v1/auth/github/callback",
        )
        .await;
    let denied_error = format!("{:?}", denied.unwrap_err());
    assert!(!denied_error.contains(TOKEN_CANARY));

    *stub.token_mode.lock().unwrap() = "ok".to_owned();
    *stub.user_mode.lock().unwrap() = "oversized".to_owned();
    let oversized = provider
        .resolve_identity(
            "stub-code",
            VERIFIER,
            "http://127.0.0.1:5151/api/v1/auth/github/callback",
        )
        .await;
    assert_eq!(
        oversized.unwrap_err(),
        crate::auth::github::GitHubProviderError::Unavailable
    );

    // Upstream 5xx is an outage, not an authentication denial.
    *stub.token_mode.lock().unwrap() = "unavailable".to_owned();
    *stub.user_mode.lock().unwrap() = "ok".to_owned();
    let outage = provider
        .resolve_identity(
            "stub-code",
            VERIFIER,
            "http://127.0.0.1:5151/api/v1/auth/github/callback",
        )
        .await;
    assert_eq!(
        outage.unwrap_err(),
        crate::auth::github::GitHubProviderError::Unavailable
    );
    Ok(())
}

#[tokio::test]
async fn postgres_github_lifecycle_router_flow() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let service = test_service(&[11], FakeGitHubProvider::succeed(11, "owner"));
        let router = login_router(&test_database.connection, service).await?;

        // Start is anonymous and returns the browser URL only.
        let challenge = github::s256_challenge(&verifier_for(31));
        let started = router
            .clone()
            .oneshot(
                Request::post("/api/v1/auth/github/login-attempts")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({ "codeChallenge": challenge }).to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(started.status(), StatusCode::CREATED);
        let started_body = to_bytes(started.into_body(), 16 * 1024).await?;
        let started_document: serde_json::Value = serde_json::from_slice(&started_body)?;
        let attempt_id = started_document["attemptId"]
            .as_str()
            .ok_or_else(|| io::Error::other("attemptId is missing"))?
            .to_owned();
        let authorize_url = started_document["authorizeUrl"]
            .as_str()
            .ok_or_else(|| io::Error::other("authorizeUrl is missing"))?
            .to_owned();
        let state = authorize_state(&authorize_url);

        // Early redemption reports pending without consuming the attempt.
        let early = router
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/v1/auth/github/login-attempts/{attempt_id}/redeem"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "codeVerifier": verifier_for(31) }).to_string(),
                ))?,
            )
            .await?;
        assert_eq!(early.status(), StatusCode::CONFLICT);

        // Spoofed identity query fields cannot influence the callback.
        let spoofed = router
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/auth/github/callback?code=provider-code-router&state={state}&login=attacker&email=attacker%40example.com&id=999"
                ))
                .body(Body::empty())?,
            )
            .await?;
        assert_eq!(spoofed.status(), StatusCode::OK);
        let spoofed_headers = spoofed.headers().clone();
        let spoofed_body = to_bytes(spoofed.into_body(), 16 * 1024).await?;
        let spoofed_text = String::from_utf8_lossy(&spoofed_body).into_owned();

        let redeemed = router
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/v1/auth/github/login-attempts/{attempt_id}/redeem"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "codeVerifier": verifier_for(31) }).to_string(),
                ))?,
            )
            .await?;
        assert_eq!(redeemed.status(), StatusCode::OK);
        let redeemed_body = to_bytes(redeemed.into_body(), 16 * 1024).await?;
        let redeemed_document: serde_json::Value = serde_json::from_slice(&redeemed_body)?;

        // The issued session authenticates protected routes.
        let access = redeemed_document["accessCredential"]
            .as_str()
            .ok_or_else(|| io::Error::other("accessCredential is missing"))?
            .to_owned();
        let status = router
            .clone()
            .oneshot(
                Request::get("/api/v1/status")
                    .header(header::AUTHORIZATION, format!("Bearer {access}"))
                    .body(Body::empty())?,
            )
            .await?;

        // Replay at both steps denies without new sessions.
        let redeem_replay = router
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/v1/auth/github/login-attempts/{attempt_id}/redeem"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "codeVerifier": verifier_for(31) }).to_string(),
                ))?,
            )
            .await?;
        let callback_replay = router
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/auth/github/callback?code=provider-code-router&state={state}"
                ))
                .body(Body::empty())?,
            )
            .await?;

        Ok::<_, Box<dyn Error>>((
            authorize_url,
            started_document,
            spoofed_headers,
            spoofed_text,
            redeemed_document,
            status.status(),
            redeem_replay.status(),
            callback_replay.status(),
            login_counts(&test_database.connection).await?,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (
        authorize_url,
        started_document,
        spoofed_headers,
        spoofed_text,
        redeemed_document,
        status,
        redeem_replay,
        callback_replay,
        counts,
    ) = evidence?;

    assert!(authorize_url.contains("code_challenge_method=S256"));
    assert!(!authorize_url.contains("kta_") && !started_document.to_string().contains("kta_"));
    assert_eq!(redeemed_document["clusterId"], "test");
    assert_eq!(redeemed_document["githubLogin"], "owner");
    assert!(!redeemed_document.to_string().contains("codeVerifier"));
    assert!(!redeemed_document.to_string().contains("state="));
    assert!(redeemed_document["principalId"].as_str().is_some());
    assert!(
        redeemed_document["accessCredential"]
            .as_str()
            .is_some_and(|credential| credential.starts_with("kta_"))
    );
    assert_eq!(
        spoofed_headers.get(header::CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );
    assert_eq!(
        spoofed_headers
            .get(header::CONTENT_SECURITY_POLICY)
            .unwrap(),
        "default-src 'none'; frame-ancestors 'none'; base-uri 'none'"
    );
    assert_eq!(
        spoofed_headers.get(header::REFERRER_POLICY).unwrap(),
        "no-referrer"
    );
    assert!(!spoofed_text.contains("provider-code-router"));
    assert!(!spoofed_text.contains("attacker"));
    assert_eq!(status, StatusCode::OK);
    assert_eq!(redeem_replay, StatusCode::FORBIDDEN);
    assert_eq!(callback_replay, StatusCode::BAD_REQUEST);
    assert_eq!(counts.0, 1);
    assert_eq!(counts.2, 1);
    assert_eq!(
        counts.4.map(|id| id.to_string()).as_deref(),
        redeemed_document["principalId"].as_str()
    );
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_github_lifecycle_denies_misuse() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let service = test_service(&[11], FakeGitHubProvider::succeed(11, "owner"));
        let router = login_router(&test_database.connection, service).await?;

        // Malformed challenges and identifiers fail as client errors.
        let bad_challenge = router
            .clone()
            .oneshot(
                Request::post("/api/v1/auth/github/login-attempts")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({ "codeChallenge": "short" }).to_string(),
                    ))?,
            )
            .await?
            .status();
        let bad_attempt = router
            .clone()
            .oneshot(
                Request::post("/api/v1/auth/github/login-attempts/not-a-uuid/redeem")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({ "codeVerifier": verifier_for(41) }).to_string(),
                    ))?,
            )
            .await?
            .status();
        let unknown_attempt = router
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/v1/auth/github/login-attempts/{}/redeem",
                    Uuid::now_v7()
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "codeVerifier": verifier_for(41) }).to_string(),
                ))?,
            )
            .await?;
        let unknown_status = unknown_attempt.status();
        let unknown_body = to_bytes(unknown_attempt.into_body(), 4096).await?;

        // Provider denial renders the generic failure page.
        let challenge = github::s256_challenge(&verifier_for(42));
        let started = router
            .clone()
            .oneshot(
                Request::post("/api/v1/auth/github/login-attempts")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({ "codeChallenge": challenge }).to_string(),
                    ))?,
            )
            .await?;
        let started_document: serde_json::Value =
            serde_json::from_slice(&to_bytes(started.into_body(), 16 * 1024).await?)?;
        let denied_state = authorize_state(
            started_document["authorizeUrl"]
                .as_str()
                .ok_or_else(|| io::Error::other("authorizeUrl is missing"))?,
        );
        let denied = router
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/auth/github/callback?error=access_denied&state={denied_state}"
                ))
                .body(Body::empty())?,
            )
            .await?;
        let denied_status = denied.status();
        let denied_body = to_bytes(denied.into_body(), 16 * 1024).await?;
        let denied_text = String::from_utf8_lossy(&denied_body).into_owned();

        // Missing state shares the same generic page.
        let missing_state = router
            .clone()
            .oneshot(
                Request::get("/api/v1/auth/github/callback?code=orphan-code").body(Body::empty())?,
            )
            .await?
            .status();

        // A disabled service fails closed without touching storage.
        let disabled_router = {
            let context = test_context(test_database.connection.clone()).await?;
            App::routes(&context).to_router::<App>(context, Router::new())?
        };
        let disabled = disabled_router
            .clone()
            .oneshot(
                Request::post("/api/v1/auth/github/login-attempts")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({ "codeChallenge": challenge }).to_string(),
                    ))?,
            )
            .await?
            .status();
        let disabled_callback = disabled_router
            .oneshot(
                Request::get("/api/v1/auth/github/callback?code=any&state=any")
                    .body(Body::empty())?,
            )
            .await?
            .status();

        Ok::<_, Box<dyn Error>>((
            bad_challenge,
            bad_attempt,
            unknown_status,
            unknown_body,
            denied_status,
            denied_text,
            missing_state,
            disabled,
            disabled_callback,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (
        bad_challenge,
        bad_attempt,
        unknown_status,
        unknown_body,
        denied_status,
        denied_text,
        missing_state,
        disabled,
        disabled_callback,
    ) = evidence?;

    assert_eq!(bad_challenge, StatusCode::BAD_REQUEST);
    assert_eq!(bad_attempt, StatusCode::BAD_REQUEST);
    assert_eq!(unknown_status, StatusCode::FORBIDDEN);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&unknown_body)?,
        serde_json::json!({"error": "login_denied"})
    );
    assert_eq!(denied_status, StatusCode::BAD_REQUEST);
    assert!(!denied_text.contains("access_denied"));
    assert!(!denied_text.contains("kta_"));
    assert_eq!(missing_state, StatusCode::BAD_REQUEST);
    assert_eq!(disabled, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        disabled_callback,
        StatusCode::SERVICE_UNAVAILABLE,
        "a disabled callback must fail closed before reading attempts"
    );
    assert!(removed);
    Ok(())
}
