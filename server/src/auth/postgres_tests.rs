use std::{error::Error, path::Path};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderValue, Request, StatusCode, header},
    routing::{delete, post},
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
    actions::{create_session, revoke_session_family, rotate_refresh, validate_access},
    domain::{IssuedSession, RevocationReason, actions},
    entities::{principal, refresh_credential, security_audit_intent, session_family},
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
    ] {
        assert!(
            document["paths"].get(path).is_some(),
            "{path} is not documented"
        );
    }
    let serialized = serde_json::to_string(&document)?;
    assert!(!serialized.contains("kta_") && !serialized.contains("ktr_"));
    assert!(!serialized.contains("secret-canary"));
    // Credentials travel in the JSON body or the Authorization header only.
    assert!(
        !serialized.contains("\"in\":\"query\""),
        "no query credential transport"
    );
    assert!(
        !serialized.contains("\"in\":\"cookie\""),
        "no cookie credential transport"
    );
    Ok(())
}
