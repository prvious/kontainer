//! Login: one active attempt, pair binding, and the ordered commit.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::session::controller::Redemption;
use crate::session::errors::SessionError;
use crate::session::models::{AnchorState, SessionDocument, SessionLoginInput};

use super::fakes::{
    future_ms, issued, login_input, past_ms, redeemed, redeemed_for, Harness, CLUSTER, ORIGIN,
    OTHER_ORIGIN, OTHER_PRINCIPAL_ID, PRINCIPAL_ID,
};

#[tokio::test]
async fn login_commits_record_metadata_and_anchor_before_publishing() {
    let harness = Harness::new();
    harness
        .controllers
        .script()
        .redeem
        .push_back(Ok(Redemption::Ready(redeemed(1))));
    let view = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect("login succeeds");

    assert_eq!(view.origin, ORIGIN);
    assert_eq!(view.cluster_id, CLUSTER);
    assert_eq!(view.principal_id, PRINCIPAL_ID);
    assert_eq!(view.github_login, "owner");
    assert_eq!(view.label.as_deref(), Some("Default"));
    assert!(view.access_expires_at_unix_ms > past_ms());

    // The credential record and the non-secret summary are durable before the
    // anchor commits, and the anchor commits before anything is published.
    assert_eq!(
        harness.entries(),
        vec![
            "read_anchor",
            "start_login",
            "open_browser",
            "redeem_begin",
            "redeem_end",
            "read_anchor",
            "write_anchor:commit_pending",
            "write_profile:1",
            "write_document",
            "write_anchor:ready",
        ]
    );
    let anchor = harness.vault.anchor().expect("anchor exists");
    assert_eq!(anchor.state, AnchorState::Ready);
    assert_eq!(anchor.generation, 1);
    assert_eq!(anchor.profile_id, view.profile_id);
    assert_eq!(anchor.origin, ORIGIN);
    assert!(harness.vault.has_profile(view.profile_id));
    let document = harness.metadata.document().expect("document exists");
    assert!(document.hint.is_none());
    assert!(document.session.is_some());

    // Rust opened the authorization URL itself; it never crossed IPC.
    assert_eq!(
        harness.browser.opened(),
        vec!["https://github.com/login/oauth/authorize".to_owned()]
    );

    // A restore right after a login must not consume another credential.
    let entries = harness.entries();
    assert_eq!(
        harness.manager.restore().await.expect("restore"),
        Some(view)
    );
    assert_eq!(harness.entries(), entries);
}

#[tokio::test]
async fn login_reuses_the_profile_uuid_for_the_same_pair_and_updates_origin() {
    let harness = Harness::new();
    let first = harness.login_ok().await;

    assert_eq!(
        harness
            .manager
            .login(login_input(OTHER_ORIGIN))
            .await
            .expect_err("active session blocks login"),
        SessionError::ProfileConflict
    );
    assert_eq!(harness.count("start_login"), 1);

    harness.controllers.script().logout.push_back(Ok(()));
    harness.manager.logout().await.expect("logout");
    harness
        .controllers
        .script()
        .redeem
        .push_back(Ok(Redemption::Ready(redeemed(2))));
    let second = harness
        .manager
        .login(login_input(OTHER_ORIGIN))
        .await
        .expect("re-login succeeds");

    assert_eq!(second.profile_id, first.profile_id);
    assert_eq!(second.origin, OTHER_ORIGIN);
    let anchor = harness.vault.anchor().expect("anchor");
    assert_eq!(anchor.origin, OTHER_ORIGIN);
    assert_eq!(anchor.generation, 2);
    let record = harness
        .vault
        .profile(first.profile_id)
        .expect("re-login record");
    assert_eq!(
        record.generation, 2,
        "the record commits the new generation"
    );
    assert!(harness.vault.has_profile(first.profile_id));
}

#[tokio::test]
async fn different_pair_conflicts_without_overwriting_records() {
    let harness = Harness::new();
    harness.seed_ready_session();
    let before_anchor = harness.vault.anchor().expect("anchor");
    let before_record = harness
        .vault
        .profile(before_anchor.profile_id)
        .expect("record");

    let error = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect_err("conflict");
    assert_eq!(error, SessionError::ProfileConflict);

    assert_eq!(harness.vault.anchor(), Some(before_anchor.clone()));
    let after_record = harness
        .vault
        .profile(before_anchor.profile_id)
        .expect("record");
    assert_eq!(after_record.generation, before_record.generation);
    assert_eq!(
        after_record.refresh_credential,
        before_record.refresh_credential
    );
    assert_eq!(harness.count("write_anchor:commit_pending"), 0);
    assert_eq!(harness.count("write_document"), 0);
    // The existing summary and record set are untouched.
    let document = harness.metadata.document().expect("document");
    assert_eq!(
        document.session.expect("session").principal_id,
        before_anchor.principal_id
    );
    assert_eq!(harness.count("start_login"), 0);
    assert_eq!(harness.count("logout"), 0);
}

#[tokio::test]
async fn conflicting_login_reports_cleanup_uncertain_when_revoke_fails() {
    let harness = Harness::new();
    let ready = harness.seed_ready_session();
    let before = crate::session::models::AnchorRecord {
        state: AnchorState::LoggedOut,
        ..ready
    };
    harness.vault.seed_anchor(&before);

    harness
        .controllers
        .script()
        .redeem
        .push_back(Ok(Redemption::Ready(redeemed_for(9, OTHER_PRINCIPAL_ID))));
    harness
        .controllers
        .script()
        .logout
        .push_back(Err(SessionError::ControllerUnavailable));
    let error = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect_err("conflict cleanup fails");
    assert_eq!(error, SessionError::CleanupUncertain);
    assert_eq!(harness.vault.anchor(), Some(before));

    harness
        .controllers
        .script()
        .logout
        .push_back(Err(SessionError::ControllerUnavailable));
    assert_eq!(
        harness
            .manager
            .logout()
            .await
            .expect_err("orphan cleanup fails"),
        SessionError::RevokeFailed
    );
    assert_eq!(
        harness.vault.anchor_state(),
        Some(AnchorState::LogoutPending)
    );
    assert_eq!(
        harness
            .restarted_manager()
            .restore()
            .await
            .expect_err("restart stays pending"),
        SessionError::CleanupUncertain
    );
}

#[tokio::test]
async fn login_uses_the_stored_label_and_clears_the_hint_once_committed() {
    let harness = Harness::new();
    harness.metadata.set_document(SessionDocument::new(
        Some(crate::session::models::LoginHint {
            origin: ORIGIN.to_owned(),
            label: Some("Hinted".to_owned()),
        }),
        None,
    ));
    harness
        .controllers
        .script()
        .redeem
        .push_back(Ok(Redemption::Ready(redeemed(1))));
    let view = harness
        .manager
        .login(SessionLoginInput {
            origin: ORIGIN.to_owned(),
            label: None,
        })
        .await
        .expect("login succeeds");
    assert_eq!(view.label.as_deref(), Some("Hinted"));
    assert!(harness.hint().is_none());
}

#[tokio::test(start_paused = true)]
async fn replacement_login_cancels_and_joins_the_previous_attempt() {
    let harness = Arc::new(Harness::new());
    {
        let mut script = harness.controllers.script();
        script.redeem.push_back(Ok(Redemption::Pending));
        script.redeem.push_back(Ok(Redemption::Ready(redeemed(1))));
    }

    let first = {
        let harness = harness.clone();
        tokio::spawn(async move { harness.manager.login(login_input(ORIGIN)).await })
    };
    while harness.count("redeem_begin") < 1 {
        tokio::task::yield_now().await;
    }

    let second = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect("replacement login succeeds");
    assert_eq!(second.principal_id, PRINCIPAL_ID);
    assert_eq!(
        first.await.expect("task joins").unwrap_err(),
        SessionError::LoginCancelled
    );
    assert_eq!(harness.count("redeem_begin"), 2);
}

#[tokio::test(start_paused = true)]
async fn cancel_login_joins_the_attempt_and_reports_cancellation() {
    let harness = Arc::new(Harness::new());
    harness
        .controllers
        .script()
        .redeem
        .push_back(Ok(Redemption::Pending));
    let task = {
        let harness = harness.clone();
        tokio::spawn(async move { harness.manager.login(login_input(ORIGIN)).await })
    };
    while harness.count("redeem_begin") < 1 {
        tokio::task::yield_now().await;
    }
    harness.manager.cancel_login().await.expect("cancel");
    assert_eq!(
        task.await.expect("task joins").unwrap_err(),
        SessionError::LoginCancelled
    );
}

#[tokio::test]
async fn browser_failure_is_reported_without_starting_redemption() {
    let harness = Harness::new();
    harness.browser.fail.store(true, Ordering::SeqCst);
    let error = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect_err("browser fails");
    assert_eq!(error, SessionError::BrowserOpenFailed);
    assert_eq!(harness.count("redeem_begin"), 0);
}

#[tokio::test]
async fn invalid_origin_is_rejected_before_any_attention() {
    let harness = Harness::new();
    harness.controllers.fail_origin();
    let error = harness
        .manager
        .login(login_input("http://controller.example"))
        .await
        .expect_err("origin rejected");
    assert_eq!(error, SessionError::InvalidOrigin);
    assert_eq!(harness.count("start_login"), 0);
}

#[tokio::test]
async fn expired_attempt_reports_login_expired() {
    let harness = Harness::new();
    harness
        .controllers
        .script()
        .start
        .push_back(Ok(crate::session::controller::LoginStart {
            attempt_id: uuid::Uuid::from_u128(1),
            authorize_url: "https://github.com/login/oauth/authorize".to_owned(),
            expires_at_unix_ms: past_ms(),
        }));
    let error = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect_err("expired");
    assert_eq!(error, SessionError::LoginExpired);
    assert_eq!(harness.count("redeem_begin"), 0);
}

#[tokio::test(start_paused = true)]
async fn redemption_polls_only_on_not_ready() {
    let harness = Harness::new();
    {
        let mut script = harness.controllers.script();
        script.redeem.push_back(Ok(Redemption::Pending));
        script.redeem.push_back(Ok(Redemption::Pending));
        script.redeem.push_back(Ok(Redemption::Ready(redeemed(1))));
    }
    harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect("login succeeds after two pending polls");
    assert_eq!(harness.count("redeem_begin"), 3);

    // A denial stops immediately instead of polling.
    let harness = Harness::new();
    harness
        .controllers
        .script()
        .redeem
        .push_back(Err(SessionError::LoginDenied));
    let error = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect_err("denied");
    assert_eq!(error, SessionError::LoginDenied);
    assert_eq!(harness.count("redeem_begin"), 1);
}

#[tokio::test]
async fn login_writes_no_credential_outside_the_vault() {
    let harness = Harness::new();
    let view = harness.login_ok().await;

    let document = serde_json::to_string(&harness.metadata.document().expect("document"))
        .expect("serialize document");
    let anchor =
        serde_json::to_string(&harness.vault.anchor().expect("anchor")).expect("serialize anchor");
    let view = serde_json::to_string(&view).expect("serialize view");
    for (name, serialized) in [("document", document), ("anchor", anchor), ("view", view)] {
        assert!(
            !serialized.contains("kta_"),
            "{name} carries an access credential"
        );
        assert!(
            !serialized.contains("ktr_"),
            "{name} carries a refresh credential"
        );
    }

    // The vault record is the one place the refresh credential is retained;
    // the access credential never leaves memory.
    let profile = harness
        .vault
        .profile(harness.vault.anchor().expect("anchor").profile_id)
        .expect("record");
    assert!(profile.refresh_credential.starts_with("ktr_"));
    let serialized = serde_json::to_string(&profile).expect("serialize record");
    assert!(
        !serialized.contains("kta_"),
        "the record must not carry an access credential"
    );
}

#[tokio::test]
async fn login_profile_write_failure_holds_the_commit_for_local_retry() {
    let harness = Harness::new();
    harness
        .controllers
        .script()
        .redeem
        .push_back(Ok(Redemption::Ready(redeemed(1))));
    harness
        .vault
        .fail_profile_writes
        .store(true, Ordering::SeqCst);

    let error = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect_err("vault write fails");
    assert_eq!(error, SessionError::VaultWriteFailed);
    assert_eq!(harness.count("write_document"), 0);
    assert_eq!(
        harness.vault.anchor_state(),
        Some(AnchorState::CommitPending)
    );

    // Restore reports the held failure and performs no new work.
    let entries = harness.entries();
    assert_eq!(
        harness.manager.restore().await.expect_err("held failure"),
        SessionError::VaultWriteFailed
    );
    assert_eq!(harness.entries(), entries);

    // The retry is local: no new controller traffic, and the session commits.
    harness
        .vault
        .fail_profile_writes
        .store(false, Ordering::SeqCst);
    let view = harness.manager.retry_commit().await.expect("commit retry");
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::Ready));
    assert_eq!(harness.count("start_login"), 1);
    assert_eq!(harness.count("refresh"), 0);
    assert_eq!(
        harness.manager.restore().await.expect("restore"),
        Some(view)
    );
}

#[tokio::test]
async fn login_anchor_commit_failure_holds_the_successor() {
    let harness = Harness::new();
    harness
        .controllers
        .script()
        .redeem
        .push_back(Ok(Redemption::Ready(redeemed(1))));
    harness
        .vault
        .fail_ready_anchor_writes
        .store(true, Ordering::SeqCst);

    let error = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect_err("anchor commit fails");
    assert_eq!(error, SessionError::VaultWriteFailed);
    assert_eq!(
        harness.vault.anchor_state(),
        Some(AnchorState::CommitPending)
    );
    // The record was written before the commit step; the retry reuses it.
    assert!(harness.vault.has_profile(harness.anchor_profile_id()));

    harness
        .vault
        .fail_ready_anchor_writes
        .store(false, Ordering::SeqCst);
    let view = harness.manager.retry_commit().await.expect("commit retry");
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::Ready));
    assert_eq!(harness.count("redeem_begin"), 1);
    assert_eq!(view.principal_id, PRINCIPAL_ID);
}

#[tokio::test]
async fn login_metadata_write_failure_holds_the_commit() {
    let harness = Harness::new();
    harness
        .controllers
        .script()
        .redeem
        .push_back(Ok(Redemption::Ready(redeemed(1))));
    harness.metadata.fail_writes.store(true, Ordering::SeqCst);

    let error = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect_err("metadata write fails");
    assert_eq!(error, SessionError::LocalStateFailed);
    assert_eq!(
        harness.vault.anchor_state(),
        Some(AnchorState::CommitPending)
    );
    assert_eq!(
        harness.manager.restore().await.expect_err("held failure"),
        SessionError::LocalStateFailed
    );

    harness.metadata.fail_writes.store(false, Ordering::SeqCst);
    harness.manager.retry_commit().await.expect("commit retry");
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::Ready));
    assert!(harness
        .metadata
        .document()
        .expect("document")
        .session
        .is_some());
}

#[tokio::test]
async fn login_failure_before_commit_leaves_no_session() {
    let harness = Harness::new();
    harness
        .controllers
        .script()
        .redeem
        .push_back(Err(SessionError::ControllerUnavailable));
    let error = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect_err("unavailable");
    assert_eq!(error, SessionError::ControllerUnavailable);
    assert!(harness.vault.anchor().is_none());
    assert_eq!(harness.manager.restore().await.expect("restore"), None);
}

#[tokio::test]
async fn pending_anchor_write_failure_revokes_the_issued_family() {
    let harness = Harness::new();
    harness
        .controllers
        .script()
        .redeem
        .push_back(Ok(Redemption::Ready(redeemed(1))));
    harness.controllers.script().logout.push_back(Ok(()));
    harness
        .vault
        .fail_pending_anchor_writes
        .store(true, Ordering::SeqCst);

    let error = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect_err("pending anchor fails");
    assert_eq!(error, SessionError::VaultWriteFailed);
    // The freshly issued family is revoked rather than dropped alive.
    assert_eq!(harness.count("logout"), 1);
    assert_eq!(harness.count("write_profile"), 0);
    assert!(harness.vault.anchor().is_none());
}

#[tokio::test]
async fn pending_anchor_write_failure_with_failed_revoke_is_uncertain() {
    let harness = Harness::new();
    harness
        .controllers
        .script()
        .redeem
        .push_back(Ok(Redemption::Ready(redeemed(1))));
    harness
        .controllers
        .script()
        .logout
        .push_back(Err(SessionError::ControllerUnavailable));
    harness
        .vault
        .fail_pending_anchor_writes
        .store(true, Ordering::SeqCst);

    let error = harness
        .manager
        .login(login_input(ORIGIN))
        .await
        .expect_err("revoke fails too");
    assert_eq!(error, SessionError::CleanupUncertain);
    assert!(harness.vault.anchor().is_none());

    harness
        .vault
        .fail_pending_anchor_writes
        .store(false, Ordering::SeqCst);
    harness.controllers.script().logout.push_back(Ok(()));
    harness.manager.logout().await.expect("cleanup retry");
    assert_eq!(harness.count("logout"), 2);
}

#[tokio::test]
async fn durable_pending_anchors_block_login_commit() {
    for (state, expected) in [
        (AnchorState::Ready, SessionError::ProfileConflict),
        (AnchorState::CommitPending, SessionError::CleanupUncertain),
        (AnchorState::LogoutPending, SessionError::CleanupUncertain),
        (AnchorState::CleanupPending, SessionError::CleanupUncertain),
    ] {
        let harness = Harness::new();
        let anchor = harness.seed_ready_session();
        harness
            .vault
            .seed_anchor(&crate::session::models::AnchorRecord {
                state,
                ..anchor.clone()
            });
        assert_eq!(
            harness
                .manager
                .login(login_input(ORIGIN))
                .await
                .expect_err("pending custody blocks commit"),
            expected
        );
        assert_eq!(harness.count("start_login"), 0);
        assert_eq!(harness.vault.anchor_state(), Some(state));
        assert_eq!(
            harness
                .vault
                .profile(anchor.profile_id)
                .expect("original record")
                .generation,
            anchor.generation
        );
    }
}

#[tokio::test]
async fn post_redemption_read_failure_retains_credentials_for_logout() {
    let harness = Harness::new();
    harness
        .controllers
        .script()
        .redeem
        .push_back(Ok(Redemption::Ready(redeemed(1))));
    harness
        .controllers
        .script()
        .logout
        .push_back(Err(SessionError::ControllerUnavailable));
    harness
        .vault
        .fail_anchor_read_number
        .store(2, Ordering::SeqCst);

    assert_eq!(
        harness
            .manager
            .login(login_input(ORIGIN))
            .await
            .expect_err("anchor read and cleanup fail"),
        SessionError::CleanupUncertain
    );
    assert_eq!(
        harness.manager.restore().await.expect_err("held cleanup"),
        SessionError::CleanupUncertain
    );

    harness.controllers.script().logout.push_back(Ok(()));
    harness.manager.logout().await.expect("logout retry");
    assert_eq!(harness.manager.restore().await.expect("logged out"), None);
}

#[tokio::test]
async fn a_held_successor_blocks_login_until_committed() {
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
    assert_eq!(
        harness.manager.restore().await.expect_err("held"),
        SessionError::VaultWriteFailed
    );

    harness
        .vault
        .fail_profile_writes
        .store(false, Ordering::SeqCst);
    let starts = harness.count("start_login");
    assert_eq!(
        harness
            .manager
            .login(login_input(ORIGIN))
            .await
            .expect_err("held custody blocks login"),
        SessionError::CleanupUncertain
    );
    assert_eq!(harness.count("start_login"), starts);

    let view = harness.manager.retry_commit().await.expect("commit retry");
    let record = harness.vault.profile(anchor.profile_id).expect("record");
    assert_eq!(record.generation, 2);
    assert_eq!(harness.vault.anchor().expect("anchor").generation, 2);
    assert_eq!(
        harness.manager.restore().await.expect("restore"),
        Some(view)
    );
}

#[tokio::test]
async fn a_different_pair_after_logout_still_conflicts() {
    let harness = Harness::new();
    let anchor = harness.seed_ready_session();
    harness
        .vault
        .seed_anchor(&crate::session::models::AnchorRecord {
            state: AnchorState::LoggedOut,
            ..anchor.clone()
        });
    harness
        .controllers
        .script()
        .redeem
        .push_back(Ok(Redemption::Ready(redeemed_for(9, OTHER_PRINCIPAL_ID))));
    harness.controllers.script().logout.push_back(Ok(()));

    // Single-profile custody keeps the retained identity: a different pair
    // never overwrites it, even while no credential is held.
    assert_eq!(
        harness
            .manager
            .login(login_input(ORIGIN))
            .await
            .expect_err("conflict"),
        SessionError::ProfileConflict
    );
    assert_eq!(
        harness.vault.anchor().expect("anchor").profile_id,
        anchor.profile_id
    );
    assert_eq!(harness.count("logout"), 1);
}

#[tokio::test]
async fn failed_held_revocation_is_preserved_for_logout_retry() {
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
    let _ = harness.manager.restore().await.expect_err("held commit");
    harness
        .vault
        .fail_profile_writes
        .store(false, Ordering::SeqCst);

    harness
        .controllers
        .script()
        .logout
        .push_back(Err(SessionError::ControllerUnavailable));
    assert_eq!(
        harness.manager.logout().await.expect_err("revoke fails"),
        SessionError::RevokeFailed
    );

    {
        let mut script = harness.controllers.script();
        script.logout.push_back(Ok(()));
        script
            .refresh
            .push_back(Ok(issued(3, future_ms(), future_ms())));
        script.logout.push_back(Ok(()));
    }
    harness.manager.logout().await.expect("logout retry");

    let entries = harness.entries();
    assert!(
        entries
            .iter()
            .any(|entry| entry == "logout:controller.example:02020202"),
        "the held successor must remain available for retry"
    );
    assert!(
        !entries
            .iter()
            .any(|entry| entry.starts_with("logout:other.example:")),
        "no credential may follow an origin change"
    );
}
