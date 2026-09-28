//! Restore: metadata as the tamper check, ordered rotation, and the
//! non-restorable pending states.

use std::sync::atomic::Ordering;

use crate::session::errors::SessionError;
use crate::session::models::{AnchorState, ConnectionSummary, SessionDocument};

use super::fakes::{
    future_ms, issued, past_ms, Harness, CLUSTER, OTHER_ORIGIN, OTHER_PRINCIPAL_ID,
};

#[tokio::test]
async fn restore_rotates_once_in_order_and_commits_the_successor() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    let access_expires_at_unix_ms = future_ms();
    harness.controllers.script().refresh.push_back(Ok(issued(
        2,
        access_expires_at_unix_ms,
        future_ms(),
    )));

    let view = harness
        .manager
        .restore()
        .await
        .expect("restore succeeds")
        .expect("session restored");

    assert_eq!(view.profile_id, anchor.profile_id);
    assert_eq!(view.origin, anchor.origin);
    assert_eq!(view.cluster_id, anchor.cluster_id);
    assert_eq!(view.principal_id, anchor.principal_id);
    assert_eq!(view.email, super::fakes::EMAIL);
    assert_eq!(view.display_name, super::fakes::DISPLAY_NAME);
    assert_eq!(view.access_expires_at_unix_ms, access_expires_at_unix_ms);

    // Metadata is checked before the keyed credential is read; the anchor is
    // marked pending before the refresh; the successor commits before use.
    assert_eq!(
        harness.entries(),
        vec![
            "read_anchor",
            "read_document",
            "read_profile",
            "write_anchor:commit_pending",
            "refresh:controller.example:01010101",
            "write_profile:2",
            "read_document",
            "write_document",
            "write_anchor:ready",
        ]
    );
    let anchor = harness.vault.anchor().expect("anchor");
    assert_eq!(anchor.state, AnchorState::Ready);
    assert_eq!(anchor.generation, 2);
    let record = harness
        .vault
        .profile(anchor.profile_id)
        .expect("successor record");
    assert_eq!(record.generation, 2);
    assert!(record.refresh_credential.starts_with("ktr_"));
}

#[tokio::test]
async fn restore_is_idempotent_after_a_committed_rotation() {
    let harness = Harness::new();
    harness.seed_ready_session();
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    let first = harness
        .manager
        .restore()
        .await
        .expect("restore")
        .expect("view");
    let entries = harness.entries();
    let second = harness
        .manager
        .restore()
        .await
        .expect("restore")
        .expect("view");
    assert_eq!(first, second);
    assert_eq!(harness.entries(), entries);
}

#[tokio::test]
async fn restore_metadata_tamper_never_reads_the_profile_or_the_network() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();

    let mut document = harness.metadata.document().expect("document");
    document.session.as_mut().expect("session").origin = OTHER_ORIGIN.to_owned();
    harness.metadata.set_document(document);

    let error = harness.manager.restore().await.expect_err("tamper");
    assert_eq!(error, SessionError::ReloginRequired);
    assert_eq!(harness.count("read_profile"), 0);
    assert_eq!(harness.count("refresh"), 0);
    assert_eq!(harness.count("logout"), 0);
    // Local invalidation only: the keyed credential is deleted by anchor ID.
    assert!(!harness.vault.has_profile(anchor.profile_id));
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
}

#[tokio::test]
async fn restore_rejects_tampered_uuid_pair_and_cluster_without_a_keyed_read() {
    for tamper in ["uuid", "principal", "cluster"] {
        let harness = Harness::new();
        let anchor = harness.seed_ready_session();
        let mut document = harness.metadata.document().expect("document");
        let session = document.session.as_mut().expect("session");
        match tamper {
            "uuid" => session.profile_id = uuid::Uuid::from_u128(0xee),
            "principal" => session.principal_id = OTHER_PRINCIPAL_ID,
            "cluster" => session.cluster_id = "other".to_owned(),
            _ => unreachable!(),
        }
        harness.metadata.set_document(document);

        let error = harness.manager.restore().await.expect_err("tamper");
        assert_eq!(error, SessionError::ReloginRequired, "{tamper}");
        assert_eq!(harness.count("read_profile"), 0, "{tamper}");
        assert_eq!(harness.count("refresh"), 0, "{tamper}");
        assert!(!harness.vault.has_profile(anchor.profile_id), "{tamper}");
    }
}

#[tokio::test]
async fn restore_rejects_a_rebound_or_missing_credential_record() {
    // Rebound: the record no longer repeats the anchor binding.
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    let mut record = harness.vault.profile(anchor.profile_id).expect("record");
    record.origin = OTHER_ORIGIN.to_owned();
    harness.vault.seed_profile(&record);
    let error = harness.manager.restore().await.expect_err("rebound");
    assert_eq!(error, SessionError::ReloginRequired);
    assert_eq!(harness.count("refresh"), 0);

    // Missing: the anchor promises a record that is not there.
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness.vault.delete_profile_silently(anchor.profile_id);
    let error = harness.manager.restore().await.expect_err("missing");
    assert_eq!(error, SessionError::ReloginRequired);
    assert_eq!(harness.count("refresh"), 0);
}

#[tokio::test]
async fn restore_rejects_a_wrong_generation_record() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    let mut record = harness.vault.profile(anchor.profile_id).expect("record");
    record.generation = anchor.generation + 1;
    harness.vault.seed_profile(&record);
    let error = harness
        .manager
        .restore()
        .await
        .expect_err("generation drift");
    assert_eq!(error, SessionError::ReloginRequired);
    assert_eq!(harness.count("refresh"), 0);
}

#[tokio::test]
async fn restore_expired_record_requires_relogin_without_a_network_call() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    let mut record = harness.vault.profile(anchor.profile_id).expect("record");
    record.refresh_expires_at_unix_ms = past_ms();
    harness.vault.seed_profile(&record);

    let error = harness.manager.restore().await.expect_err("expired");
    assert_eq!(error, SessionError::ReloginRequired);
    assert_eq!(harness.count("refresh"), 0);
    assert!(!harness.vault.has_profile(anchor.profile_id));
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
}

#[tokio::test]
async fn restore_refresh_rejection_cleans_up_and_requires_relogin() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness
        .controllers
        .script()
        .refresh
        .push_back(Err(SessionError::ReloginRequired));

    let error = harness.manager.restore().await.expect_err("rejected");
    assert_eq!(error, SessionError::ReloginRequired);
    assert_eq!(harness.count("refresh"), 1);
    assert!(!harness.vault.has_profile(anchor.profile_id));
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
    let hint = harness.hint().expect("hint survives");
    assert_eq!(hint.origin, anchor.origin);
}

#[tokio::test]
async fn restore_ambiguous_refresh_is_never_replayed() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness
        .controllers
        .script()
        .refresh
        .push_back(Err(SessionError::ControllerUnavailable));

    let error = harness.manager.restore().await.expect_err("ambiguous");
    assert_eq!(error, SessionError::CleanupUncertain);
    // The anchor stays pending and the consumed-or-not credential is untouched.
    assert_eq!(
        harness.vault.anchor_state(),
        Some(AnchorState::CommitPending)
    );
    assert!(harness.vault.has_profile(anchor.profile_id));

    let error = harness
        .manager
        .restore()
        .await
        .expect_err("still uncertain");
    assert_eq!(error, SessionError::CleanupUncertain);
    assert_eq!(
        harness.count("refresh"),
        1,
        "the credential is never replayed"
    );
    assert_eq!(harness.count("read_profile"), 1);
}

#[tokio::test]
async fn restore_metadata_store_failure_preserves_custody() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness.metadata.fail_reads.store(true, Ordering::SeqCst);

    let error = harness.manager.restore().await.expect_err("store down");
    assert_eq!(error, SessionError::LocalStateFailed);
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::Ready));
    assert!(harness.vault.has_profile(anchor.profile_id));
    assert_eq!(harness.count("refresh"), 0);
}

#[tokio::test]
async fn restore_corrupt_metadata_forces_relogin() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness.metadata.corrupt_reads.store(true, Ordering::SeqCst);

    let error = harness.manager.restore().await.expect_err("corrupt");
    assert_eq!(error, SessionError::ReloginRequired);
    assert_eq!(harness.count("read_profile"), 0);
    assert!(!harness.vault.has_profile(anchor.profile_id));
}

#[tokio::test]
async fn restore_successor_write_failure_holds_the_commit_for_local_retry() {
    let harness = Harness::new();
    harness.seed_ready_session();
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    harness
        .vault
        .fail_profile_writes
        .store(true, Ordering::SeqCst);

    let error = harness.manager.restore().await.expect_err("write fails");
    assert_eq!(error, SessionError::VaultWriteFailed);
    assert_eq!(harness.count("refresh"), 1);

    // Nothing was published: a fresh restore reports the held state and makes
    // no further controller or vault request.
    let entries = harness.entries();
    assert_eq!(
        harness.manager.restore().await.expect_err("held"),
        SessionError::VaultWriteFailed
    );
    assert_eq!(harness.entries(), entries);

    harness
        .vault
        .fail_profile_writes
        .store(false, Ordering::SeqCst);
    // A retry that fails again keeps the successor for the next attempt.
    harness
        .vault
        .fail_ready_anchor_writes
        .store(true, Ordering::SeqCst);
    assert_eq!(
        harness
            .manager
            .retry_commit()
            .await
            .expect_err("commit fails"),
        SessionError::VaultWriteFailed
    );
    harness
        .vault
        .fail_ready_anchor_writes
        .store(false, Ordering::SeqCst);
    // The explicit retry finishes the commit locally; the consumed credential
    // is never replayed.
    let view = harness.manager.retry_commit().await.expect("commit retry");
    assert_eq!(
        harness.count("refresh"),
        1,
        "the consumed credential is not replayed"
    );
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::Ready));
    assert_eq!(view.cluster_id, CLUSTER);
    assert_eq!(
        harness.manager.restore().await.expect("restore"),
        Some(view)
    );
}

#[tokio::test]
async fn restore_anchor_commit_failure_holds_the_successor() {
    let harness = Harness::new();
    harness.seed_ready_session();
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    harness
        .vault
        .fail_ready_anchor_writes
        .store(true, Ordering::SeqCst);

    let error = harness.manager.restore().await.expect_err("commit fails");
    assert_eq!(error, SessionError::VaultWriteFailed);
    assert_eq!(harness.count("refresh"), 1);

    harness
        .vault
        .fail_ready_anchor_writes
        .store(false, Ordering::SeqCst);
    let view = harness.manager.retry_commit().await.expect("retry");
    assert_eq!(harness.count("refresh"), 1);
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::Ready));
    assert!(view.access_expires_at_unix_ms > past_ms());
}

#[tokio::test]
async fn restore_from_a_pending_anchor_is_not_restorable() {
    for state in [AnchorState::CommitPending, AnchorState::LogoutPending] {
        let harness = Harness::new();
        let anchor = harness.seed_ready_session();
        harness
            .vault
            .seed_anchor(&crate::session::models::AnchorRecord { state, ..anchor });

        let error = harness.manager.restore().await.expect_err("pending");
        assert_eq!(error, SessionError::CleanupUncertain, "{state:?}");
        assert_eq!(harness.count("read_profile"), 0, "{state:?}");
        assert_eq!(harness.count("refresh"), 0, "{state:?}");
    }
}

#[tokio::test]
async fn restore_from_cleanup_pending_requires_explicit_action() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness
        .vault
        .seed_anchor(&crate::session::models::AnchorRecord {
            state: AnchorState::CleanupPending,
            ..anchor
        });
    let error = harness
        .manager
        .restore()
        .await
        .expect_err("cleanup pending");
    assert_eq!(error, SessionError::CleanupUncertain);
    assert_eq!(harness.count("refresh"), 0);
}

#[tokio::test]
async fn restore_logged_out_anchor_returns_none_and_clears_stale_metadata() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness
        .vault
        .seed_anchor(&crate::session::models::AnchorRecord {
            state: AnchorState::LoggedOut,
            ..anchor.clone()
        });

    assert_eq!(harness.manager.restore().await.expect("restore"), None);
    let document = harness.metadata.document().expect("document");
    assert!(document.session.is_none());
    let hint = document.hint.expect("hint");
    assert_eq!(hint.origin, anchor.origin);
    assert_eq!(hint.label.as_deref(), Some("Default"));
}

#[tokio::test]
async fn restore_without_anchor_returns_none_and_clears_stale_metadata() {
    let harness = Harness::new();
    harness.seed_ready_session();
    harness.vault.remove_anchor_silently();
    assert_eq!(harness.manager.restore().await.expect("restore"), None);
    let document = harness.metadata.document().expect("document");
    assert!(document.session.is_none());
}

#[tokio::test]
async fn retry_commit_without_a_held_commit_is_a_local_state_failure() {
    let harness = Harness::new();
    let error = harness
        .manager
        .retry_commit()
        .await
        .expect_err("nothing held");
    assert_eq!(error, SessionError::LocalStateFailed);
    assert!(harness.entries().is_empty());
}

#[tokio::test]
async fn restore_skips_the_vault_when_a_session_is_already_published() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    let entries = harness.entries();
    assert_eq!(
        harness.manager.restore().await.expect("restore"),
        Some(view)
    );
    assert_eq!(harness.entries(), entries);
}

#[tokio::test]
async fn ipc_inputs_reject_identity_fields() {
    // Identity can only come from the controller response: the one structured
    // IPC input (the sign-out summary) is strictly two counts.
    let parsed: Result<ConnectionSummary, _> = serde_json::from_value(serde_json::json!({
        "nodes": 3,
        "services": 12,
        "principalId": "018f2e34-0000-7000-8000-0000000000aa",
    }));
    assert!(parsed.is_err());
}

#[tokio::test]
async fn restore_handles_a_corrupt_anchor_as_absent() {
    let harness = Harness::new();
    harness.seed_ready_session();
    harness.vault.corrupt_anchor();
    assert_eq!(harness.manager.restore().await.expect("restore"), None);
    assert_eq!(harness.count("refresh"), 0);
}

#[tokio::test]
async fn restore_handles_a_corrupt_record_as_invalid() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness.vault.corrupt_profile(anchor.profile_id);
    let error = harness.manager.restore().await.expect_err("corrupt record");
    assert_eq!(error, SessionError::ReloginRequired);
    assert_eq!(harness.count("refresh"), 0);
}

#[tokio::test]
async fn restore_survives_a_vault_read_failure() {
    let harness = Harness::new();
    harness.seed_ready_session();
    harness.vault.fail_reads.store(true, Ordering::SeqCst);
    let error = harness.manager.restore().await.expect_err("vault down");
    assert_eq!(error, SessionError::VaultUnavailable);
    assert_eq!(harness.count("refresh"), 0);
}

#[tokio::test]
async fn session_document_keeps_the_same_shape_after_a_failed_restore() {
    let harness = Harness::new();
    harness.seed_ready_session();
    harness
        .controllers
        .script()
        .refresh
        .push_back(Err(SessionError::ReloginRequired));
    let _ = harness.manager.restore().await;
    let document = harness.metadata.document().expect("document");
    assert_eq!(document.version, crate::session::models::METADATA_VERSION);
    assert!(document.hint.is_some());
    assert!(document.session.is_none());
}

#[tokio::test]
async fn restore_without_an_anchor_keeps_the_login_hint() {
    let harness = Harness::new();
    let hint_origin = OTHER_ORIGIN.to_owned();
    harness.metadata.set_document(SessionDocument::new(
        Some(crate::session::models::LoginHint {
            origin: hint_origin.clone(),
            label: Some("Hinted".to_owned()),
        }),
        None,
    ));
    assert_eq!(harness.manager.restore().await.expect("restore"), None);
    let document = harness.metadata.document().expect("document");
    assert_eq!(document.hint.expect("hint").origin, hint_origin);
}

#[tokio::test]
async fn restore_pending_anchor_write_failure_makes_no_network_call() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    let record_before = harness.vault.profile(anchor.profile_id).expect("record");
    harness
        .vault
        .fail_pending_anchor_writes
        .store(true, Ordering::SeqCst);

    let error = harness
        .manager
        .restore()
        .await
        .expect_err("pending anchor fails");
    // The non-restorable marker could not be persisted, so the credential is
    // never sent and the durable state stays restorable-but-untouched.
    assert_eq!(error, SessionError::VaultWriteFailed);
    assert_eq!(harness.count("refresh"), 0);
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::Ready));
    assert_eq!(
        harness
            .vault
            .profile(anchor.profile_id)
            .expect("record")
            .refresh_credential,
        record_before.refresh_credential
    );
}

#[tokio::test]
async fn restore_delete_failure_keeps_custody_non_restorable() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    let mut document = harness.metadata.document().expect("document");
    document.session.as_mut().expect("session").origin = OTHER_ORIGIN.to_owned();
    harness.metadata.set_document(document);
    harness.vault.fail_deletes.store(true, Ordering::SeqCst);

    let error = harness.manager.restore().await.expect_err("delete fails");
    // The rejected credential may still be readable: no clean re-login claim.
    assert_eq!(error, SessionError::CleanupUncertain);
    assert!(harness.vault.has_profile(anchor.profile_id));
    assert_eq!(
        harness.vault.anchor_state(),
        Some(AnchorState::CleanupPending)
    );
    assert_eq!(harness.count("refresh"), 0);
}

#[tokio::test]
async fn restore_with_an_unvalidatable_stored_origin_relogins() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness.controllers.fail_origin();

    let error = harness.manager.restore().await.expect_err("origin invalid");
    assert_eq!(error, SessionError::ReloginRequired);
    assert_eq!(harness.count("refresh"), 0);
    assert_eq!(harness.count("read_profile"), 0);
    assert!(!harness.vault.has_profile(anchor.profile_id));
}

#[tokio::test]
async fn two_concurrent_restores_rotate_once() {
    let harness = Harness::new();
    harness.seed_ready_session();
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));

    let (first, second) = tokio::join!(harness.manager.restore(), harness.manager.restore());
    let first = first.expect("first restore").expect("view");
    let second = second.expect("second restore").expect("view");
    assert_eq!(first, second);
    assert_eq!(harness.count("refresh"), 1, "one rotation, not two");
    assert_eq!(harness.count("read_profile"), 1);
}
