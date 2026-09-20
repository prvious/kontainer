//! Rust session ownership: login, ordered rotation, logout, and recovery.
//!
//! Failure ordering is the whole point of this module:
//!
//! - A credential-consuming rotation marks the trusted anchor pending before
//!   the request leaves the process, so a crash can never re-send a possibly
//!   consumed credential.
//! - A successor becomes durable before the access credential is published in
//!   memory: the credential record, then the non-secret summary, then the
//!   anchor (a rotation leaves an already-matching summary untouched).
//! - Logout revokes remotely before deleting locally, and a rejected access
//!   credential triggers exactly one persisted rotation before the family is
//!   treated as gone.
//! - Ambiguous rotation failures refuse automatic recovery and report
//!   [`SessionError::CleanupUncertain`]. Logout ambiguity preserves custody for
//!   an explicit retry and reports [`SessionError::RevokeFailed`]; a restart
//!   while either is pending can never restore as authenticated.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kontainer_native_client::{AccessCredential, LoginVerifier, RefreshCredential};
use tokio::sync::{watch, Mutex as AsyncMutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::browser::BrowserOpener;
use super::controller::{
    Controller, ControllerFactory, IssuedCredentials, RedeemedSession, Redemption,
};
use super::errors::SessionError;
use super::metadata::{MetadataError, SessionMetadataStore};
use super::models::{
    decode_anchor, decode_profile, encode_anchor, encode_profile, sanitize_label, AnchorRecord,
    AnchorState, LoginHint, MetadataSession, ProfileRecord, SessionDocument, SessionLoginInput,
    SessionView, ANCHOR_VERSION, PROFILE_VERSION,
};
use super::vault::CredentialVault;

/// Poll interval between redemption attempts while the browser step is open.
const REDEEM_POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Local ceiling for one login attempt, independent of the controller clock.
const LOGIN_ATTEMPT_CEILING: Duration = Duration::from_secs(10 * 60);

/// The one published session plus whatever work is in flight.
struct SessionSlots {
    active: Option<ActiveSession>,
    /// A successor that rotated successfully but could not be committed. It is
    /// retained only in this process, and only so the operator can retry the
    /// commit or log out.
    held: Option<HeldCommit>,
    login: Option<Arc<LoginSlot>>,
}

struct ActiveSession {
    view: SessionView,
    access: AccessCredential,
}

struct HeldCommit {
    view: SessionView,
    /// Ready-state anchor carrying the successor's generation.
    anchor: AnchorRecord,
    credentials: IssuedCredentials,
    /// The failure that stopped the commit. It is reported by `restore` while
    /// the commit stays held; a failed `retry_commit` derives a fresh code.
    error: SessionError,
}

/// Registration of one in-flight login attempt.
struct LoginSlot {
    cancel: CancellationToken,
    done: watch::Sender<bool>,
}

/// Removes the login registration and signals joiners on every exit path,
/// including a dropped command future.
struct LoginGuard<'a> {
    manager: &'a SessionManager,
    slot: Arc<LoginSlot>,
}

impl Drop for LoginGuard<'_> {
    fn drop(&mut self) {
        {
            let mut slots = self.manager.slots();
            if slots
                .login
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &self.slot))
            {
                slots.login = None;
            }
        }
        let _ = self.slot.done.send(true);
    }
}

/// Owns the desktop's one logical profile.
pub(crate) struct SessionManager {
    vault: Arc<dyn CredentialVault>,
    metadata: Arc<dyn SessionMetadataStore>,
    controllers: Arc<dyn ControllerFactory>,
    browser: Arc<dyn BrowserOpener>,
    slots: Mutex<SessionSlots>,
    /// Serializes state-changing operations that touch the vault or publish a
    /// session. Login polling deliberately stays outside this lock.
    ops: AsyncMutex<()>,
}

impl SessionManager {
    pub(crate) fn new(
        vault: Arc<dyn CredentialVault>,
        metadata: Arc<dyn SessionMetadataStore>,
        controllers: Arc<dyn ControllerFactory>,
        browser: Arc<dyn BrowserOpener>,
    ) -> Self {
        Self {
            vault,
            metadata,
            controllers,
            browser,
            slots: Mutex::new(SessionSlots {
                active: None,
                held: None,
                login: None,
            }),
            ops: AsyncMutex::new(()),
        }
    }

    /// Run one browser login. Any earlier attempt is cancelled and joined
    /// before a new verifier is generated.
    pub(crate) async fn login(
        &self,
        input: SessionLoginInput,
    ) -> Result<SessionView, SessionError> {
        let slot = Arc::new(LoginSlot {
            cancel: CancellationToken::new(),
            done: watch::channel(false).0,
        });
        let previous = {
            let mut slots = self.slots();
            slots.login.replace(slot.clone())
        };
        if let Some(previous) = previous {
            previous.cancel.cancel();
            wait_for_done(&previous).await;
        }
        let guard = LoginGuard {
            manager: self,
            slot: slot.clone(),
        };
        let result = self.run_login(input, &slot).await;
        drop(guard);
        result
    }

    /// Cancel the active attempt and join its cleanup.
    pub(crate) async fn cancel_login(&self) -> Result<(), SessionError> {
        if let Some(slot) = self.take_login() {
            slot.cancel.cancel();
            wait_for_done(&slot).await;
        }
        Ok(())
    }

    /// Restore the stored session, rotating once through the vault.
    ///
    /// An already-published in-process session is returned unchanged so a
    /// repeated call never consumes another credential.
    pub(crate) async fn restore(&self) -> Result<Option<SessionView>, SessionError> {
        if let Some(view) = self.active_view() {
            return Ok(Some(view));
        }
        if let Some(error) = self.held_error() {
            return Err(error);
        }
        let _ops = self.ops.lock().await;
        if let Some(view) = self.active_view() {
            return Ok(Some(view));
        }
        if let Some(error) = self.held_error() {
            return Err(error);
        }
        let Some(anchor) = self.load_anchor().await? else {
            self.clear_stale_session_document()?;
            return Ok(None);
        };
        match anchor.state {
            AnchorState::Ready => self.restore_ready(anchor).await.map(Some),
            AnchorState::LoggedOut => {
                let label = self.stored_label();
                self.finish_logged_out_document(&anchor, label)?;
                Ok(None)
            }
            // A rotation or logout stopped mid-flight: the stored credential
            // may already be consumed, so it is never replayed automatically.
            AnchorState::CommitPending
            | AnchorState::LogoutPending
            | AnchorState::CleanupPending => Err(SessionError::CleanupUncertain),
        }
    }

    /// Finish a login or rotation whose successor was issued but never became
    /// durable. Local writes only; no controller call.
    pub(crate) async fn retry_commit(&self) -> Result<SessionView, SessionError> {
        let _ops = self.ops.lock().await;
        let Some(held) = self.take_held() else {
            return Err(SessionError::LocalStateFailed);
        };
        // A held successor is only valid while the vault still sits in the
        // pending generation it was issued from; a newer login supersedes it.
        let anchor = match self.load_anchor().await {
            Ok(anchor) => anchor,
            Err(error) => {
                self.hold(held);
                return Err(error);
            }
        };
        let still_pending = matches!(
            anchor,
            Some(anchor)
                if anchor.state == AnchorState::CommitPending
                    && anchor.generation + 1 == held.anchor.generation
        );
        if !still_pending {
            // A newer login superseded this successor. Revoke it rather than
            // leaving a live family with no local record; a failed revoke is
            // still better than a silent drop, and the family dies at expiry.
            if let Ok(controller) = self.controllers.for_origin(&held.anchor.origin) {
                let _ = self
                    .revoke_issued(controller.as_ref(), &held.credentials)
                    .await;
            }
            return Err(SessionError::LocalStateFailed);
        }
        self.commit_successor(held.view, held.anchor, held.credentials)
            .await
    }

    /// Commit an issued successor: credential record, non-secret summary,
    /// trusted anchor, then in-memory publication.
    ///
    /// Any failure keeps the successor held for [`Self::retry_commit`] or an
    /// explicit logout, and no part of it becomes observable as a session.
    async fn commit_successor(
        &self,
        view: SessionView,
        anchor: AnchorRecord,
        credentials: IssuedCredentials,
    ) -> Result<SessionView, SessionError> {
        let held = |credentials: IssuedCredentials, error: SessionError| HeldCommit {
            view: view.clone(),
            anchor: anchor.clone(),
            credentials,
            error,
        };
        let record = build_profile_record(&anchor, anchor.generation, &credentials);
        let payload = match encode_profile(&record) {
            Ok(payload) => payload,
            Err(_) => {
                self.hold(held(credentials, SessionError::VaultWriteFailed));
                return Err(SessionError::VaultWriteFailed);
            }
        };
        if self
            .vault
            .write_profile(anchor.profile_id, &payload)
            .await
            .is_err()
        {
            self.hold(held(credentials, SessionError::VaultWriteFailed));
            return Err(SessionError::VaultWriteFailed);
        }
        drop(payload);
        let document = SessionDocument::new(None, Some(metadata_session_from(&view)));
        if self.metadata.write_document(&document).is_err() {
            self.hold(held(credentials, SessionError::LocalStateFailed));
            return Err(SessionError::LocalStateFailed);
        }
        if self.write_anchor(&anchor).await.is_err() {
            self.hold(held(credentials, SessionError::VaultWriteFailed));
            return Err(SessionError::VaultWriteFailed);
        }
        let access = credentials.access;
        self.publish(view.clone(), access);
        Ok(view)
    }

    /// Revoke the server session before clearing local custody.
    pub(crate) async fn logout(&self) -> Result<(), SessionError> {
        if let Some(slot) = self.take_login() {
            slot.cancel.cancel();
            wait_for_done(&slot).await;
        }
        let _ops = self.ops.lock().await;

        // The anchor is read before any in-memory credential is taken so a
        // transient vault failure leaves the retryable state untouched.
        let anchor = self.load_anchor().await?;
        let active = self.take_active();
        let held = self.take_held();
        let label = active
            .as_ref()
            .and_then(|session| session.view.label.clone())
            .or_else(|| self.stored_label());

        let Some(anchor) = anchor else {
            // No anchor claims a session: revoke whatever this process still
            // holds, best effort, since there is no durable state to retry.
            if let Some(held) = held {
                if let Ok(controller) = self.controllers.for_origin(&held.anchor.origin) {
                    let _ = self
                        .revoke_issued(controller.as_ref(), &held.credentials)
                        .await;
                }
            }
            if let Some(session) = active {
                if let Ok(controller) = self.controllers.for_origin(&session.view.origin) {
                    let _ = controller
                        .logout(&session.access, &CancellationToken::new())
                        .await;
                }
            }
            self.clear_stale_session_document()?;
            return Ok(());
        };

        match anchor.state {
            AnchorState::LoggedOut | AnchorState::CleanupPending => {
                self.finish_logout_local(&anchor, label).await
            }
            AnchorState::Ready | AnchorState::CommitPending | AnchorState::LogoutPending => {
                // The anchor must be non-restorable before any network attempt.
                if anchor.state != AnchorState::LogoutPending {
                    self.write_anchor(&AnchorRecord {
                        state: AnchorState::LogoutPending,
                        ..anchor.clone()
                    })
                    .await?;
                }
                let Ok(controller) = self.controllers.for_origin(&anchor.origin) else {
                    // An un-validatable stored origin can never be reached;
                    // local custody is cleared without a network attempt.
                    return self.finish_logout_local(&anchor, label).await;
                };

                // A successor still held by this process is the newest live
                // family; its predecessor, when one existed, is represented by
                // the vault record handled below.
                if let Some(held) = held {
                    self.revoke_issued(controller.as_ref(), &held.credentials)
                        .await?;
                }

                let record = self
                    .load_profile(anchor.profile_id)
                    .await?
                    .filter(|record| {
                        record.matches_binding(&anchor) && record_refresh(record).is_some()
                    });
                match record {
                    Some(record) => {
                        // The live access credential is the cheapest honest
                        // revocation; only a rejected access credential needs
                        // the persisted rotation.
                        if let Some(session) = active {
                            match controller
                                .logout(&session.access, &CancellationToken::new())
                                .await
                            {
                                Ok(()) => {
                                    return self.finish_logout_local(&anchor, label).await;
                                }
                                Err(SessionError::ReloginRequired) => {}
                                Err(_) => return Err(SessionError::RevokeFailed),
                            }
                        }
                        self.logout_record(controller.as_ref(), &anchor, &record, label)
                            .await
                    }
                    None => {
                        // No usable record. A live access credential may still
                        // revoke the family; a rejected one proves nothing, and
                        // there is no refresh credential left to rotate.
                        if let Some(session) = active {
                            match controller
                                .logout(&session.access, &CancellationToken::new())
                                .await
                            {
                                Ok(()) => return self.finish_logout_local(&anchor, label).await,
                                Err(SessionError::ReloginRequired) => {
                                    self.finish_logout_local(&anchor, label).await?;
                                    return Err(SessionError::CleanupUncertain);
                                }
                                Err(_) => return Err(SessionError::RevokeFailed),
                            }
                        }
                        // Nothing reachable can revoke a session the anchor
                        // still claims: clean up locally and do not report a
                        // verified logout.
                        self.finish_logout_local(&anchor, label).await?;
                        Err(SessionError::CleanupUncertain)
                    }
                }
            }
        }
    }

    /// The non-secret login hint for the login screen.
    pub(crate) fn login_hint(&self) -> Option<LoginHint> {
        self.metadata
            .read_document()
            .ok()
            .flatten()
            .and_then(|document| document.hint)
    }

    async fn run_login(
        &self,
        input: SessionLoginInput,
        slot: &LoginSlot,
    ) -> Result<SessionView, SessionError> {
        if slot.cancel.is_cancelled() {
            return Err(SessionError::LoginCancelled);
        }
        let controller = self.controllers.for_origin(&input.origin)?;
        let verifier = LoginVerifier::generate();
        let start = controller.start_login(&verifier, &slot.cancel).await?;
        self.browser.open(&start.authorize_url)?;
        let deadline = login_deadline(start.expires_at_unix_ms);
        let redeemed = loop {
            if slot.cancel.is_cancelled() {
                return Err(SessionError::LoginCancelled);
            }
            if now_ms() >= deadline {
                return Err(SessionError::LoginExpired);
            }
            match controller
                .redeem_login(start.attempt_id, &verifier, &slot.cancel)
                .await?
            {
                Redemption::Ready(redeemed) => break redeemed,
                Redemption::Pending => {
                    tokio::select! {
                        () = slot.cancel.cancelled() => return Err(SessionError::LoginCancelled),
                        () = tokio::time::sleep(REDEEM_POLL_INTERVAL) => {}
                    }
                }
            }
        };
        // The verifier exists only for this loop and is dropped with it.
        drop(verifier);
        self.commit_login(controller.as_ref(), redeemed, input, slot)
            .await
    }

    async fn commit_login(
        &self,
        controller: &dyn Controller,
        redeemed: RedeemedSession,
        input: SessionLoginInput,
        slot: &LoginSlot,
    ) -> Result<SessionView, SessionError> {
        let _ops = self.ops.lock().await;
        if slot.cancel.is_cancelled() {
            // The browser step completed just as the operator cancelled: the
            // family exists and must be revoked before it is forgotten.
            if self
                .revoke_issued(controller, &redeemed.issued)
                .await
                .is_ok()
            {
                return Err(SessionError::LoginCancelled);
            }
            return Err(SessionError::CleanupUncertain);
        }

        let existing = self.load_anchor().await?;
        let profile_id = match &existing {
            None => Uuid::new_v4(),
            Some(anchor)
                if anchor.cluster_id == redeemed.cluster_id
                    && anchor.principal_id == redeemed.principal_id =>
            {
                anchor.profile_id
            }
            Some(_) => {
                // A different pair never overwrites the existing profile or
                // vault entry; the newly issued family is revoked instead.
                if self
                    .revoke_issued(controller, &redeemed.issued)
                    .await
                    .is_ok()
                {
                    return Err(SessionError::ProfileConflict);
                }
                return Err(SessionError::CleanupUncertain);
            }
        };

        let old_generation = existing.as_ref().map_or(0, |anchor| anchor.generation);
        let new_generation = old_generation + 1;
        let label = sanitize_label(input.label.clone()).or_else(|| self.stored_label());
        let pending = AnchorRecord {
            version: ANCHOR_VERSION,
            profile_id,
            origin: controller.canonical_origin().to_owned(),
            cluster_id: redeemed.cluster_id.clone(),
            principal_id: redeemed.principal_id,
            generation: old_generation,
            state: AnchorState::CommitPending,
        };
        let ready = AnchorRecord {
            generation: new_generation,
            state: AnchorState::Ready,
            ..pending.clone()
        };
        let view = SessionView {
            profile_id,
            origin: pending.origin.clone(),
            cluster_id: pending.cluster_id.clone(),
            principal_id: pending.principal_id,
            github_login: redeemed.github_login.clone(),
            label: label.clone(),
            access_expires_at_unix_ms: redeemed.issued.access_expires_at_unix_ms,
        };

        // 1. A successor held from an earlier failed commit is superseded by
        // this login; revoke it through its *own* origin, because the incoming
        // login may target a different controller and no credential may follow
        // an origin change.
        if let Some(held) = self.take_held() {
            if let Ok(held_controller) = self.controllers.for_origin(&held.anchor.origin) {
                let _ = self
                    .revoke_issued(held_controller.as_ref(), &held.credentials)
                    .await;
            }
        }

        // 2. The anchor becomes non-restorable before the credential exists.
        if self.write_anchor(&pending).await.is_err() {
            // The credentials were issued but cannot be committed; revoke them
            // rather than dropping a live family.
            if self
                .revoke_issued(controller, &redeemed.issued)
                .await
                .is_ok()
            {
                return Err(SessionError::VaultWriteFailed);
            }
            return Err(SessionError::CleanupUncertain);
        }

        // 3-6. Credential record, non-secret summary, anchor, publication.
        self.commit_successor(view, ready, redeemed.issued).await
    }

    async fn restore_ready(&self, anchor: AnchorRecord) -> Result<SessionView, SessionError> {
        let controller = match self.controllers.for_origin(&anchor.origin) {
            Ok(controller) => controller,
            Err(_) => return Err(self.invalidate_to_relogin(&anchor, None).await),
        };

        // The metadata document must agree with the trusted anchor before the
        // UUID-keyed credential is read at all. A store that is merely
        // unavailable keeps custody; a document that no longer matches the
        // strict schema is treated as tampering.
        let document = match self.metadata.read_document() {
            Ok(document) => document,
            Err(MetadataError::Unavailable) => return Err(SessionError::LocalStateFailed),
            Err(MetadataError::Corrupt) => None,
        };
        let Some(session) = document
            .as_ref()
            .and_then(|document| document.session.clone())
        else {
            let label = document
                .as_ref()
                .and_then(|document| document.hint.as_ref())
                .and_then(|hint| hint.label.clone());
            return Err(self.invalidate_to_relogin(&anchor, label).await);
        };
        if !session.matches_anchor(&anchor) {
            return Err(self
                .invalidate_to_relogin(&anchor, session.label.clone())
                .await);
        }

        let record = match self.load_profile(anchor.profile_id).await? {
            Some(record)
                if record.matches_binding(&anchor) && record.generation == anchor.generation =>
            {
                record
            }
            _ => {
                return Err(self
                    .invalidate_to_relogin(&anchor, session.label.clone())
                    .await);
            }
        };
        if record.refresh_expires_at_unix_ms <= now_ms() {
            return Err(self
                .invalidate_to_relogin(&anchor, session.label.clone())
                .await);
        }

        // Ordered rotation: pending anchor, one refresh, successor, commit.
        self.write_anchor(&AnchorRecord {
            state: AnchorState::CommitPending,
            ..anchor.clone()
        })
        .await?;
        let Some(refresh) = record_refresh(&record) else {
            return Err(self
                .invalidate_to_relogin(&anchor, session.label.clone())
                .await);
        };
        let issued = match controller
            .refresh_session(&refresh, &CancellationToken::new())
            .await
        {
            Ok(issued) => issued,
            Err(SessionError::ReloginRequired) => {
                return Err(self
                    .invalidate_to_relogin(&anchor, session.label.clone())
                    .await);
            }
            // Whether or not the controller consumed the credential is
            // unknowable; restore never sends it again. Only an explicit
            // logout may present it through rotation, where reuse detection
            // revokes the family.
            Err(_) => return Err(SessionError::CleanupUncertain),
        };

        let new_generation = anchor.generation + 1;
        let ready = AnchorRecord {
            generation: new_generation,
            state: AnchorState::Ready,
            ..anchor.clone()
        };
        let view = SessionView {
            profile_id: anchor.profile_id,
            origin: anchor.origin.clone(),
            cluster_id: anchor.cluster_id.clone(),
            principal_id: anchor.principal_id,
            github_login: session.github_login.clone(),
            label: sanitize_label(session.label.clone()),
            access_expires_at_unix_ms: issued.access_expires_at_unix_ms,
        };
        self.commit_successor(view, ready, issued).await
    }

    /// Rotate once and revoke the family behind a trusted record.
    async fn logout_record(
        &self,
        controller: &dyn Controller,
        anchor: &AnchorRecord,
        record: &ProfileRecord,
        label: Option<String>,
    ) -> Result<(), SessionError> {
        let Some(refresh) = record_refresh(record) else {
            // The caller already verified the record parses; this is a guard
            // rather than a reachable state.
            return Err(SessionError::CleanupUncertain);
        };
        let cancellation = CancellationToken::new();
        match controller.refresh_session(&refresh, &cancellation).await {
            Ok(issued) => {
                // Persist the successor so a crash between rotation and
                // revocation converges on retry. A failed write is safe: a
                // retry of the consumed credential revokes the family through
                // reuse detection, which is the logout outcome anyway.
                let successor = build_profile_record(anchor, anchor.generation + 1, &issued);
                if let Ok(payload) = encode_profile(&successor) {
                    let _ = self.vault.write_profile(anchor.profile_id, &payload).await;
                }
                match controller.logout(&issued.access, &cancellation).await {
                    Ok(()) | Err(SessionError::ReloginRequired) => {
                        self.finish_logout_local(anchor, label).await
                    }
                    Err(_) => Err(SessionError::RevokeFailed),
                }
            }
            Err(SessionError::ReloginRequired) => self.finish_logout_local(anchor, label).await,
            Err(_) => Err(SessionError::RevokeFailed),
        }
    }

    /// Delete local custody and report an explicit cleanup failure if the
    /// platform store refuses the deletion.
    async fn finish_logout_local(
        &self,
        anchor: &AnchorRecord,
        label: Option<String>,
    ) -> Result<(), SessionError> {
        if self.vault.delete_profile(anchor.profile_id).await.is_err() {
            let cleanup_pending = AnchorRecord {
                state: AnchorState::CleanupPending,
                ..anchor.clone()
            };
            let _ = self.write_anchor(&cleanup_pending).await;
            return Err(SessionError::LocalCleanupFailed);
        }
        let logged_out = AnchorRecord {
            state: AnchorState::LoggedOut,
            ..anchor.clone()
        };
        self.write_anchor(&logged_out).await?;
        self.finish_logged_out_document(&logged_out, label)
    }

    /// Local-only invalidation after a session the desktop will not trust:
    /// tampering, expiry, or a refresh the controller rejected. It never reads
    /// the UUID-keyed credential and never makes a network call.
    async fn invalidate_to_relogin(
        &self,
        anchor: &AnchorRecord,
        label: Option<String>,
    ) -> SessionError {
        if self.vault.delete_profile(anchor.profile_id).await.is_err() {
            // The credential may still be readable: keep custody
            // non-restorable rather than claiming the invalidation succeeded.
            let cleanup_pending = AnchorRecord {
                state: AnchorState::CleanupPending,
                ..anchor.clone()
            };
            let stored = self.write_anchor(&cleanup_pending).await.is_ok();
            let _ = self.finish_logged_out_document(&cleanup_pending, label);
            return if stored {
                SessionError::CleanupUncertain
            } else {
                SessionError::LocalStateFailed
            };
        }
        let logged_out = AnchorRecord {
            state: AnchorState::LoggedOut,
            ..anchor.clone()
        };
        if self.write_anchor(&logged_out).await.is_err() {
            return SessionError::VaultWriteFailed;
        }
        match self.finish_logged_out_document(&logged_out, label) {
            Ok(()) => SessionError::ReloginRequired,
            Err(error) => error,
        }
    }

    fn finish_logged_out_document(
        &self,
        anchor: &AnchorRecord,
        label: Option<String>,
    ) -> Result<(), SessionError> {
        let desired = SessionDocument::new(
            Some(LoginHint {
                origin: anchor.origin.clone(),
                label,
            }),
            None,
        );
        match self.metadata.read_document() {
            Ok(Some(existing)) if existing == desired => Ok(()),
            _ => self
                .metadata
                .write_document(&desired)
                .map_err(|_| SessionError::LocalStateFailed),
        }
    }

    fn clear_stale_session_document(&self) -> Result<(), SessionError> {
        let existing = match self.metadata.read_document() {
            Ok(document) => document,
            Err(MetadataError::Unavailable) => return Err(SessionError::LocalStateFailed),
            // A document that no longer matches the strict schema is replaced.
            Err(MetadataError::Corrupt) => None,
        };
        match existing {
            Some(document) if document.session.is_some() => {
                let cleared = SessionDocument::new(document.hint, None);
                self.metadata
                    .write_document(&cleared)
                    .map_err(|_| SessionError::LocalStateFailed)
            }
            _ => Ok(()),
        }
    }

    /// Revoke a credential pair custody still holds, rotating once when the
    /// access credential alone is rejected. `Ok` means the family is revoked
    /// or provably dead.
    async fn revoke_issued(
        &self,
        controller: &dyn Controller,
        issued: &IssuedCredentials,
    ) -> Result<(), SessionError> {
        let cancellation = CancellationToken::new();
        match controller.logout(&issued.access, &cancellation).await {
            Ok(()) => Ok(()),
            // A rejected access credential does not prove the family is gone.
            Err(SessionError::ReloginRequired) => {
                match controller
                    .refresh_session(&issued.refresh, &cancellation)
                    .await
                {
                    Ok(successor) => {
                        match controller.logout(&successor.access, &cancellation).await {
                            Ok(()) | Err(SessionError::ReloginRequired) => Ok(()),
                            Err(_) => Err(SessionError::RevokeFailed),
                        }
                    }
                    Err(SessionError::ReloginRequired) => Ok(()),
                    Err(_) => Err(SessionError::RevokeFailed),
                }
            }
            Err(_) => Err(SessionError::RevokeFailed),
        }
    }

    async fn load_anchor(&self) -> Result<Option<AnchorRecord>, SessionError> {
        match self.vault.read_anchor().await {
            Ok(None) => Ok(None),
            // An undecodable anchor supplies no trusted UUID, so it is treated
            // as absent rather than as a usable record; a fresh login replaces
            // it and any orphaned entry stays unreachable.
            Ok(Some(bytes)) => Ok(decode_anchor(&bytes).ok()),
            Err(_) => Err(SessionError::VaultUnavailable),
        }
    }

    async fn load_profile(&self, profile_id: Uuid) -> Result<Option<ProfileRecord>, SessionError> {
        match self.vault.read_profile(profile_id).await {
            Ok(None) => Ok(None),
            Ok(Some(bytes)) => Ok(decode_profile(&bytes).ok()),
            Err(_) => Err(SessionError::VaultUnavailable),
        }
    }

    async fn write_anchor(&self, record: &AnchorRecord) -> Result<(), SessionError> {
        let payload = encode_anchor(record).map_err(|_| SessionError::VaultWriteFailed)?;
        self.vault
            .write_anchor(payload)
            .await
            .map_err(|_| SessionError::VaultWriteFailed)
    }

    /// Serialized access to the in-memory session slots. Poisoning is
    /// recovered because a panic in one command must not abort every later
    /// custody call (the login guard also runs during unwinding).
    fn slots(&self) -> std::sync::MutexGuard<'_, SessionSlots> {
        self.slots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn publish(&self, view: SessionView, access: AccessCredential) {
        let mut slots = self.slots();
        // A published session supersedes any successor still held from an
        // earlier failed commit.
        slots.active = Some(ActiveSession { view, access });
        slots.held = None;
    }

    fn active_view(&self) -> Option<SessionView> {
        self.slots()
            .active
            .as_ref()
            .map(|session| session.view.clone())
    }

    fn take_active(&self) -> Option<ActiveSession> {
        self.slots().active.take()
    }

    fn held_error(&self) -> Option<SessionError> {
        self.slots().held.as_ref().map(|held| held.error)
    }

    fn hold(&self, held: HeldCommit) {
        self.slots().held = Some(held);
    }

    fn take_held(&self) -> Option<HeldCommit> {
        self.slots().held.take()
    }

    fn take_login(&self) -> Option<Arc<LoginSlot>> {
        self.slots().login.take()
    }

    /// The last stored display label: the active session's first, then whatever
    /// the previous logout left as a login hint.
    fn stored_label(&self) -> Option<String> {
        self.metadata
            .read_document()
            .ok()
            .flatten()
            .and_then(|document| {
                document
                    .session
                    .and_then(|session| session.label)
                    .or_else(|| document.hint.and_then(|hint| hint.label))
            })
    }
}

/// The non-secret summary for a session view.
fn metadata_session_from(view: &SessionView) -> MetadataSession {
    MetadataSession {
        profile_id: view.profile_id,
        origin: view.origin.clone(),
        cluster_id: view.cluster_id.clone(),
        principal_id: view.principal_id,
        github_login: view.github_login.clone(),
        label: view.label.clone(),
    }
}

/// Build the UUID-keyed record for a freshly issued credential pair.
fn build_profile_record(
    anchor: &AnchorRecord,
    generation: u64,
    issued: &IssuedCredentials,
) -> ProfileRecord {
    ProfileRecord {
        version: PROFILE_VERSION,
        profile_id: anchor.profile_id,
        origin: anchor.origin.clone(),
        cluster_id: anchor.cluster_id.clone(),
        principal_id: anchor.principal_id,
        generation,
        refresh_expires_at_unix_ms: issued.refresh_expires_at_unix_ms,
        refresh_credential: issued.refresh.expose().to_owned(),
    }
}

/// Parse the record's credential, or `None` when it is not canonical.
fn record_refresh(record: &ProfileRecord) -> Option<RefreshCredential> {
    RefreshCredential::parse(&record.refresh_credential).ok()
}

async fn wait_for_done(slot: &LoginSlot) {
    let mut done = slot.done.subscribe();
    let _ = done.wait_for(|value| *value).await;
}

fn login_deadline(controller_expires_at_unix_ms: i64) -> i64 {
    controller_expires_at_unix_ms.min(now_ms() + LOGIN_ATTEMPT_CEILING.as_millis() as i64)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}
