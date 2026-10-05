//! Logout: revoke before delete, explicit local cleanup failure, and
//! convergence from every non-restorable anchor state.

use std::sync::atomic::Ordering;

use crate::session::errors::SessionError;
use crate::session::models::{AnchorRecord, AnchorState};

use super::fakes::{future_ms, issued, login_input, position, Harness, ORIGIN};

#[tokio::test]
async fn logout_revokes_before_deleting_and_reports_logged_out() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness.controllers.script().logout.push_back(Ok(()));

    harness.manager.logout().await.expect("logout");

    let entries = harness.entries();
    assert!(position(&entries, "logout") < position(&entries, "delete_profile"));
    assert!(position(&entries, "delete_profile") < position(&entries, "write_anchor:logged_out"));
    // The live access credential is preferred over a needless rotation.
    assert_eq!(harness.count("logout"), 1);
    assert_eq!(harness.count("refresh"), 0);
    assert!(!harness.vault.has_profile(view.profile_id));
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));

    let document = harness.metadata.document().expect("document");
    assert!(document.session.is_none());
    assert_eq!(document.connections[0].origin, ORIGIN);
}

#[tokio::test]
async fn revoke_failure_preserves_custody_for_retry() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness
        .controllers
        .script()
        .logout
        .push_back(Err(SessionError::ControllerUnavailable));

    let error = harness.manager.logout().await.expect_err("revoke fails");
    assert_eq!(error, SessionError::RevokeFailed);
    assert!(harness.vault.has_profile(view.profile_id));
    assert_eq!(
        harness.vault.anchor_state(),
        Some(AnchorState::LogoutPending)
    );
    assert_eq!(harness.count("delete_profile"), 0);
    // A restart in this state never restores as authenticated.
    assert_eq!(
        harness.manager.restore().await.expect_err("pending"),
        SessionError::CleanupUncertain
    );
    // The retry converges through rotation.
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    harness.controllers.script().logout.push_back(Ok(()));
    harness.manager.logout().await.expect("retry");
    assert!(!harness.vault.has_profile(view.profile_id));
}

#[tokio::test]
async fn failed_logout_keeps_memory_cleanup_only_until_retry() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness.vault.delete_profile_silently(view.profile_id);
    harness
        .controllers
        .script()
        .logout
        .push_back(Err(SessionError::ControllerUnavailable));

    assert_eq!(
        harness.manager.logout().await.expect_err("revoke fails"),
        SessionError::RevokeFailed
    );
    assert_eq!(
        harness.manager.restore().await.expect_err("cleanup only"),
        SessionError::CleanupUncertain
    );
    let starts = harness.count("start_login");
    assert_eq!(
        harness
            .manager
            .login(login_input(ORIGIN))
            .await
            .expect_err("cleanup blocks login"),
        SessionError::CleanupUncertain
    );
    assert_eq!(harness.count("start_login"), starts);

    harness.controllers.script().logout.push_back(Ok(()));
    harness.manager.logout().await.expect("logout retry");
    assert_eq!(harness.manager.restore().await.expect("logged out"), None);
    harness.login_ok().await;
}

#[tokio::test]
async fn failed_logout_without_an_anchor_preserves_active_custody_for_retry() {
    let harness = Harness::new();
    harness.login_ok().await;
    harness.vault.remove_anchor_silently();
    harness
        .controllers
        .script()
        .logout
        .push_back(Err(SessionError::ControllerUnavailable));

    assert_eq!(
        harness.manager.logout().await.expect_err("revoke fails"),
        SessionError::RevokeFailed
    );
    assert_eq!(
        harness.manager.restore().await.expect_err("cleanup only"),
        SessionError::CleanupUncertain
    );
    let starts = harness.count("start_login");
    assert_eq!(
        harness
            .manager
            .login(login_input(ORIGIN))
            .await
            .expect_err("cleanup blocks login"),
        SessionError::CleanupUncertain
    );
    assert_eq!(harness.count("start_login"), starts);

    harness.controllers.script().logout.push_back(Ok(()));
    harness.manager.logout().await.expect("logout retry");
    assert_eq!(harness.manager.restore().await.expect("logged out"), None);
}

#[tokio::test]
async fn rejected_access_rotates_once_then_revokes() {
    let harness = Harness::new();
    harness.login_ok().await;
    {
        let mut script = harness.controllers.script();
        script.logout.push_back(Err(SessionError::ReloginRequired));
        script
            .refresh
            .push_back(Ok(issued(2, future_ms(), future_ms())));
        script.logout.push_back(Ok(()));
    }

    harness.manager.logout().await.expect("logout");

    let entries = harness.entries();
    let first_logout = position(&entries, "logout");
    let refresh = position(&entries, "refresh");
    let successor = position(&entries, "write_profile:2");
    let second_logout = entries
        .iter()
        .rposition(|entry| entry.starts_with("logout"))
        .expect("second logout");
    let delete = position(&entries, "delete_profile");
    assert!(first_logout < refresh);
    assert!(refresh < successor);
    assert!(successor < second_logout);
    assert!(second_logout < delete);
    // The rejected access credential is retried with the rotated successor,
    // never with the credential the controller just refused.
    assert!(entries
        .iter()
        .any(|entry| entry == "logout:controller.example:01010101"));
    assert!(entries
        .iter()
        .any(|entry| entry == "logout:controller.example:02020202"));
    assert_eq!(harness.count("refresh"), 1);
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
}

#[tokio::test]
async fn rotation_rejection_means_the_family_is_gone() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    {
        let mut script = harness.controllers.script();
        script.logout.push_back(Err(SessionError::ReloginRequired));
        script.refresh.push_back(Err(SessionError::ReloginRequired));
    }
    harness.manager.logout().await.expect("logout");
    assert!(!harness.vault.has_profile(view.profile_id));
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
    assert_eq!(harness.count("refresh"), 1);
}

#[tokio::test]
async fn ambiguous_rotation_preserves_custody() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    {
        let mut script = harness.controllers.script();
        script.logout.push_back(Err(SessionError::ReloginRequired));
        script
            .refresh
            .push_back(Err(SessionError::ControllerUnavailable));
    }
    let error = harness.manager.logout().await.expect_err("ambiguous");
    assert_eq!(error, SessionError::RevokeFailed);
    assert!(harness.vault.has_profile(view.profile_id));
    assert_eq!(
        harness.vault.anchor_state(),
        Some(AnchorState::LogoutPending)
    );
    assert_eq!(harness.count("refresh"), 1);
    assert_eq!(harness.count("delete_profile"), 0);
}

#[tokio::test]
async fn delete_failure_reports_local_cleanup_failed() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness.controllers.script().logout.push_back(Ok(()));
    harness.vault.fail_deletes.store(true, Ordering::SeqCst);

    let error = harness.manager.logout().await.expect_err("delete fails");
    assert_eq!(error, SessionError::LocalCleanupFailed);
    assert!(harness.vault.has_profile(view.profile_id));
    assert_eq!(
        harness.vault.anchor_state(),
        Some(AnchorState::CleanupPending)
    );

    // The cleanup retry is local only and completes the logout.
    harness.vault.fail_deletes.store(false, Ordering::SeqCst);
    harness.manager.logout().await.expect("cleanup retry");
    assert!(!harness.vault.has_profile(view.profile_id));
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
    assert_eq!(harness.count("logout"), 1);
    assert_eq!(harness.count("refresh"), 0);
}

#[tokio::test]
async fn cleanup_pending_is_local_only_without_a_memory_session() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness.vault.seed_anchor(&AnchorRecord {
        state: AnchorState::CleanupPending,
        ..anchor.clone()
    });

    harness.manager.logout().await.expect("logout");
    assert_eq!(harness.count("refresh"), 0);
    assert_eq!(harness.count("logout"), 0);
    assert!(!harness.vault.has_profile(anchor.profile_id));
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
}

#[tokio::test]
async fn logout_without_memory_rotates_and_revokes() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    {
        let mut script = harness.controllers.script();
        script
            .refresh
            .push_back(Ok(issued(2, future_ms(), future_ms())));
        script.logout.push_back(Ok(()));
    }

    harness.manager.logout().await.expect("logout");
    let entries = harness.entries();
    assert!(position(&entries, "refresh") < position(&entries, "logout"));
    assert!(position(&entries, "logout") < position(&entries, "delete_profile"));
    assert!(!harness.vault.has_profile(anchor.profile_id));
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
    let document = harness.metadata.document().expect("document");
    assert!(document.session.is_none());
}

#[tokio::test]
async fn logout_without_any_state_is_idempotent() {
    let harness = Harness::new();
    harness.manager.logout().await.expect("logout");
    assert_eq!(harness.count("logout"), 0);
    assert_eq!(harness.count("refresh"), 0);
}

#[tokio::test]
async fn logout_from_logged_out_anchor_is_idempotent() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness.vault.seed_anchor(&AnchorRecord {
        state: AnchorState::LoggedOut,
        ..anchor.clone()
    });
    harness.manager.logout().await.expect("logout");
    assert_eq!(harness.count("logout"), 0);
    assert!(!harness.vault.has_profile(anchor.profile_id));
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
}

#[tokio::test]
async fn logout_from_commit_pending_converges() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness.vault.seed_anchor(&AnchorRecord {
        state: AnchorState::CommitPending,
        ..anchor.clone()
    });
    {
        let mut script = harness.controllers.script();
        script
            .refresh
            .push_back(Ok(issued(2, future_ms(), future_ms())));
        script.logout.push_back(Ok(()));
    }

    harness.manager.logout().await.expect("logout");
    assert!(!harness.vault.has_profile(anchor.profile_id));
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
}

#[tokio::test]
async fn logout_clears_a_held_successor() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    harness
        .vault
        .fail_profile_writes
        .store(true, Ordering::SeqCst);
    let _ = harness.manager.restore().await.expect_err("held commit");
    harness
        .vault
        .fail_profile_writes
        .store(false, Ordering::SeqCst);

    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(3, future_ms(), future_ms())));
    // Two families exist: the held successor and the record's predecessor.
    harness.controllers.script().logout.push_back(Ok(()));
    harness.controllers.script().logout.push_back(Ok(()));
    harness.manager.logout().await.expect("logout");
    assert_eq!(harness.count("logout"), 2);
    assert_eq!(
        harness
            .manager
            .retry_commit()
            .await
            .expect_err("nothing held"),
        SessionError::LocalStateFailed
    );
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
    assert!(!harness.vault.has_profile(anchor.profile_id));
}

#[tokio::test]
async fn restart_during_logout_pending_cannot_restore() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness.vault.seed_anchor(&AnchorRecord {
        state: AnchorState::LogoutPending,
        ..anchor.clone()
    });
    assert_eq!(
        harness.manager.restore().await.expect_err("pending"),
        SessionError::CleanupUncertain
    );
    assert_eq!(harness.count("read_profile"), 0);
    assert_eq!(harness.count("refresh"), 0);
    assert_eq!(harness.count("logout"), 0);
}

#[tokio::test]
async fn logout_pending_write_failure_makes_no_network_call() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness
        .vault
        .fail_pending_anchor_writes
        .store(true, Ordering::SeqCst);

    let error = harness.manager.logout().await.expect_err("pending fails");
    assert_eq!(error, SessionError::VaultWriteFailed);
    assert_eq!(harness.count("logout"), 0);
    assert_eq!(harness.count("refresh"), 0);
    assert_eq!(harness.count("delete_profile"), 0);
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::Ready));
    assert!(harness.vault.has_profile(anchor.profile_id));
}

#[tokio::test]
async fn logout_without_a_usable_record_is_cleanup_uncertain() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness.vault.delete_profile_silently(anchor.profile_id);

    let error = harness
        .manager
        .logout()
        .await
        .expect_err("nothing to revoke");
    // No reachable credential exists, so the desktop cannot prove the server
    // session is gone; it must not report a verified logout.
    assert_eq!(error, SessionError::CleanupUncertain);
    assert_eq!(harness.count("logout"), 0);
    assert_eq!(harness.count("refresh"), 0);
    // Local material is cleared, but the durable marker stays pending because
    // remote revocation was never proved.
    assert_eq!(
        harness.vault.anchor_state(),
        Some(AnchorState::LogoutPending)
    );
    assert!(!harness.vault.has_profile(anchor.profile_id));
    assert!(harness
        .metadata
        .document()
        .expect("document")
        .session
        .is_none());
    assert_eq!(
        harness.manager.logout().await.expect_err("still uncertain"),
        SessionError::CleanupUncertain
    );
    let starts = harness.count("start_login");
    assert_eq!(
        harness
            .manager
            .login(login_input(ORIGIN))
            .await
            .expect_err("pending cleanup blocks login"),
        SessionError::CleanupUncertain
    );
    assert_eq!(harness.count("start_login"), starts);
}

#[tokio::test]
async fn logout_with_an_unvalidatable_stored_origin_stays_uncertain() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness.controllers.fail_origin();

    assert_eq!(
        harness.manager.logout().await.expect_err("unreachable"),
        SessionError::CleanupUncertain
    );
    assert_eq!(harness.count("logout"), 0);
    assert_eq!(harness.count("refresh"), 0);
    assert!(!harness.vault.has_profile(anchor.profile_id));
    assert_eq!(
        harness.vault.anchor_state(),
        Some(AnchorState::LogoutPending)
    );
}

#[tokio::test]
async fn restore_after_logout_returns_none_without_network() {
    let harness = Harness::new();
    harness.login_ok().await;
    harness.controllers.script().logout.push_back(Ok(()));
    harness.manager.logout().await.expect("logout");

    let refresh = harness.count("refresh");
    let logout = harness.count("logout");
    let read_profile = harness.count("read_profile");
    assert_eq!(harness.manager.restore().await.expect("restore"), None);
    // Only local, non-secret state is read; no credential is touched or sent.
    assert_eq!(harness.count("refresh"), refresh);
    assert_eq!(harness.count("logout"), logout);
    assert_eq!(harness.count("read_profile"), read_profile);
}

#[tokio::test]
async fn logout_with_a_rejected_access_and_no_record_is_uncertain() {
    let harness = Harness::new();
    harness.seed_ready_session();
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    let view = harness
        .manager
        .restore()
        .await
        .expect("restore")
        .expect("view");
    // The record disappears while the session is published in memory.
    harness.vault.delete_profile_silently(view.profile_id);
    harness
        .controllers
        .script()
        .logout
        .push_back(Err(SessionError::ReloginRequired));

    let error = harness.manager.logout().await.expect_err("rejected");
    // A rejected access credential does not prove the family is gone.
    assert_eq!(error, SessionError::CleanupUncertain);
    assert_eq!(
        harness.vault.anchor_state(),
        Some(AnchorState::LogoutPending)
    );
}

#[tokio::test]
async fn logout_without_an_anchor_still_revokes_a_held_successor() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    harness
        .vault
        .fail_profile_writes
        .store(true, Ordering::SeqCst);
    let _ = harness.manager.restore().await.expect_err("held commit");
    harness.vault.remove_anchor_silently();
    harness.controllers.script().logout.push_back(Ok(()));

    harness.manager.logout().await.expect("logout");
    // The held family is the only reachable credential and is revoked even
    // though the anchor disappeared.
    assert!(
        harness
            .entries()
            .iter()
            .any(|entry| entry == "logout:controller.example:02020202"),
        "the held successor must be revoked"
    );
    let _ = anchor;
}

#[tokio::test]
async fn logout_retries_after_a_transient_anchor_read_failure() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness.vault.fail_reads.store(true, Ordering::SeqCst);

    assert_eq!(
        harness.manager.logout().await.expect_err("vault down"),
        SessionError::VaultUnavailable
    );
    // The state is untouched, so the retry can revoke normally.
    harness.vault.fail_reads.store(false, Ordering::SeqCst);
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    harness.controllers.script().logout.push_back(Ok(()));
    harness.manager.logout().await.expect("retry");
    assert!(!harness.vault.has_profile(anchor.profile_id));
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
}
