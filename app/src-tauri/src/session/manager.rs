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
use std::time::{SystemTime, UNIX_EPOCH};

use kontainer_native_client::{AccessCredential, ProbeFailure, RefreshCredential};
use tokio::sync::Mutex as AsyncMutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::controller::{Controller, ControllerFactory, IssuedCredentials, RedeemedSession};
use super::errors::{LoginFailure, SessionError};
use super::metadata::{MetadataError, SessionMetadataStore};
use super::models::{
    decode_anchor, decode_profile, encode_anchor, encode_profile, sanitize_label, AnchorRecord,
    AnchorState, BootstrapView, ConnectionSummary, LoginHint, MetadataSession, PasswordLoginInput,
    ProbeDiagnostic, ProbeStatus, ProbeView, ProfileRecord, RegisterOwnerInput, SavedConnection,
    SessionDocument, SessionView, ANCHOR_VERSION, PROFILE_VERSION,
};
use super::vault::CredentialVault;

#[path = "manager_inventory.rs"]
mod inventory;
pub(crate) use inventory::InventoryError;

#[path = "manager_containers.rs"]
mod containers;
pub(crate) use containers::{ContainerActionOptionsInput, ContainerListView, ContainerTargetInput};

#[path = "manager_images.rs"]
mod images;
pub(crate) use images::{ImageListView, ImageNodesInput};

#[path = "manager_volumes.rs"]
mod volumes;
pub(crate) use volumes::{CreateVolumeInput, VolumeListView};

#[path = "manager_nodes.rs"]
mod nodes;
pub(crate) use nodes::{ControllerSettingsView, NodeListView, SampleSeriesView};

#[path = "manager_operations.rs"]
mod operations;
pub(crate) use operations::{AdmittedView, OperationEventView, OperationView};

/// The one published session plus whatever work is in flight.
struct SessionSlots {
    active: Option<ActiveSession>,
    /// A successor that rotated successfully but could not be committed. It is
    /// retained only in this process, and only so the operator can retry the
    /// commit or log out.
    held: Option<HeldCommit>,
    /// Issued credentials that could not be committed or conclusively revoked.
    orphaned: Vec<OrphanedCredentials>,
    /// A logout owns the retained credentials; they are not authenticated state.
    cleanup_only: bool,
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

struct OrphanedCredentials {
    origin: String,
    credentials: IssuedCredentials,
}

/// Retains newly issued credentials if their command future exits before they
/// are either committed or conclusively revoked.
struct IssuedGuard<'a> {
    manager: &'a SessionManager,
    issued: Option<OrphanedCredentials>,
}

impl IssuedGuard<'_> {
    fn credentials(&self) -> &IssuedCredentials {
        &self
            .issued
            .as_ref()
            .expect("issued credentials")
            .credentials
    }

    fn discard(&mut self) {
        self.issued = None;
    }

    fn take(&mut self) -> IssuedCredentials {
        self.issued.take().expect("issued credentials").credentials
    }
}

impl Drop for IssuedGuard<'_> {
    fn drop(&mut self) {
        if let Some(issued) = self.issued.take() {
            self.manager.slots().orphaned.push(issued);
        }
    }
}

/// Owns in-memory custody during logout and restores anything not consumed.
struct MemoryCustody<'a> {
    manager: &'a SessionManager,
    active: Option<ActiveSession>,
    held: Option<HeldCommit>,
    orphaned: Vec<OrphanedCredentials>,
    cleanup_only: bool,
}

impl MemoryCustody<'_> {
    fn finish(&mut self, result: Result<(), SessionError>) -> Result<(), SessionError> {
        if result.is_ok() {
            self.cleanup_only = false;
        }
        result
    }
}

impl Drop for MemoryCustody<'_> {
    fn drop(&mut self) {
        let mut slots = self.manager.slots();
        debug_assert!(slots.active.is_none());
        debug_assert!(slots.held.is_none());
        if slots.active.is_none() {
            slots.active = self.active.take();
        }
        if slots.held.is_none() {
            slots.held = self.held.take();
        }
        slots.orphaned.append(&mut self.orphaned);
        slots.cleanup_only = self.cleanup_only;
    }
}

/// Owns the desktop's one logical profile.
pub(crate) struct SessionManager {
    vault: Arc<dyn CredentialVault>,
    metadata: Arc<dyn SessionMetadataStore>,
    controllers: Arc<dyn ControllerFactory>,
    slots: Mutex<SessionSlots>,
    /// Serializes state-changing operations that touch the vault or publish a
    /// session. The sign-in request itself stays outside this lock.
    ops: AsyncMutex<()>,
}

impl SessionManager {
    pub(crate) fn new(
        vault: Arc<dyn CredentialVault>,
        metadata: Arc<dyn SessionMetadataStore>,
        controllers: Arc<dyn ControllerFactory>,
    ) -> Self {
        Self {
            vault,
            metadata,
            controllers,
            slots: Mutex::new(SessionSlots {
                active: None,
                held: None,
                orphaned: Vec::new(),
                cleanup_only: false,
            }),
            ops: AsyncMutex::new(()),
        }
    }

    /// Sign in with email and password, then commit the issued session
    /// through the same ordered custody path as every other session.
    pub(crate) async fn login(
        &self,
        input: PasswordLoginInput,
    ) -> Result<SessionView, LoginFailure> {
        let controller = self.controllers.for_origin(&input.origin)?;
        self.ensure_login_allowed().await?;
        let redeemed = controller
            .password_login(&input.email, &input.password, &CancellationToken::new())
            .await?;
        drop(input.password);
        Ok(self
            .commit_login(controller.as_ref(), redeemed, input.label)
            .await?)
    }

    /// Create the owner on a fresh controller and sign in.
    pub(crate) async fn register(
        &self,
        input: RegisterOwnerInput,
    ) -> Result<SessionView, LoginFailure> {
        let controller = self.controllers.for_origin(&input.origin)?;
        self.ensure_login_allowed().await?;
        let redeemed = controller
            .register_owner(
                &input.display_name,
                &input.email,
                &input.password,
                &CancellationToken::new(),
            )
            .await?;
        drop(input.password);
        Ok(self
            .commit_login(controller.as_ref(), redeemed, input.label)
            .await?)
    }

    /// Whether the controller at `origin` already has its owner.
    pub(crate) async fn bootstrap(&self, origin: &str) -> Result<BootstrapView, LoginFailure> {
        let controller = self.controllers.for_origin(origin)?;
        let owner_exists = controller
            .bootstrap_status(&CancellationToken::new())
            .await?;
        Ok(BootstrapView { owner_exists })
    }

    /// Two-second readiness probe of an arbitrary origin.
    pub(crate) async fn probe(&self, origin: &str) -> Result<ProbeView, SessionError> {
        let controller = self.controllers.for_origin(origin)?;
        Ok(probe_view(
            controller.probe(&CancellationToken::new()).await,
        ))
    }

    /// The signed-in controller's origin, if any.
    pub(crate) fn active_origin(&self) -> Option<String> {
        self.active_view().map(|view| view.origin)
    }

    /// Readiness of the signed-in controller, or `None` when signed out.
    pub(crate) async fn health(&self) -> Option<ProbeView> {
        let origin = self.active_view()?.origin;
        self.probe(&origin).await.ok()
    }

    /// Saved connections, most recent first.
    pub(crate) fn connections(&self) -> Vec<SavedConnection> {
        self.metadata
            .read_document()
            .ok()
            .flatten()
            .map(|document| document.connections)
            .unwrap_or_default()
    }

    /// Drop one saved connection. Its origin stays usable; only the row goes.
    pub(crate) fn forget_connection(&self, id: Uuid) -> Result<(), SessionError> {
        self.update_connections(|connections| connections.retain(|connection| connection.id != id))
    }

    /// Cache the cluster size on a saved connection for the switcher subtitle.
    pub(crate) fn record_summary(
        &self,
        origin: &str,
        summary: ConnectionSummary,
    ) -> Result<(), SessionError> {
        self.update_connections(|connections| {
            if let Some(connection) = connections
                .iter_mut()
                .find(|connection| connection.origin == origin)
            {
                connection.last_summary = Some(summary);
            }
        })
    }

    fn update_connections(
        &self,
        change: impl FnOnce(&mut Vec<SavedConnection>),
    ) -> Result<(), SessionError> {
        let mut document = match self.metadata.read_document() {
            Ok(Some(document)) => document,
            Ok(None) => return Ok(()),
            Err(_) => return Err(SessionError::LocalStateFailed),
        };
        change(&mut document.connections);
        self.metadata
            .write_document(&document)
            .map_err(|_| SessionError::LocalStateFailed)
    }

    /// Saved connections carried into every document this manager rewrites.
    fn stored_connections(&self) -> Vec<SavedConnection> {
        self.connections()
    }

    async fn ensure_login_allowed(&self) -> Result<(), SessionError> {
        let _ops = self.ops.lock().await;
        if let Some(error) = self.login_error() {
            return Err(error);
        }
        match self.load_anchor().await? {
            None
            | Some(AnchorRecord {
                state: AnchorState::LoggedOut,
                ..
            }) => Ok(()),
            Some(AnchorRecord {
                state: AnchorState::Ready,
                ..
            }) => Err(SessionError::ProfileConflict),
            Some(_) => Err(SessionError::CleanupUncertain),
        }
    }

    /// Restore the stored session, rotating once through the vault.
    ///
    /// An already-published in-process session is returned unchanged so a
    /// repeated call never consumes another credential.
    pub(crate) async fn restore(&self) -> Result<Option<SessionView>, SessionError> {
        if let Some(error) = self.custody_error() {
            return Err(error);
        }
        if let Some(view) = self.active_view() {
            return Ok(Some(view));
        }
        let _ops = self.ops.lock().await;
        if let Some(error) = self.custody_error() {
            return Err(error);
        }
        if let Some(view) = self.active_view() {
            return Ok(Some(view));
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
    /// durable. Cleanup of a mismatched anchor is attempted through its
    /// controller.
    pub(crate) async fn retry_commit(&self) -> Result<SessionView, SessionError> {
        let _ops = self.ops.lock().await;
        {
            let slots = self.slots();
            if slots.cleanup_only || !slots.orphaned.is_empty() {
                return Err(SessionError::CleanupUncertain);
            }
        }
        let Some(held) = self.take_held() else {
            return Err(SessionError::LocalStateFailed);
        };
        let HeldCommit {
            view,
            anchor: held_anchor,
            credentials,
            error: held_error,
        } = held;
        let mut issued = IssuedGuard {
            manager: self,
            issued: Some(OrphanedCredentials {
                origin: held_anchor.origin.clone(),
                credentials,
            }),
        };
        let held = |credentials: IssuedCredentials| HeldCommit {
            view: view.clone(),
            anchor: held_anchor.clone(),
            credentials,
            error: held_error,
        };
        // A held successor is valid only while the vault still sits in the
        // pending generation it was issued from.
        let anchor = match self.load_anchor().await {
            Ok(anchor) => anchor,
            Err(error) => {
                self.hold(held(issued.take()));
                return Err(error);
            }
        };
        let still_pending = matches!(
            anchor,
            Some(anchor)
                if anchor.state == AnchorState::CommitPending
                    && anchor.generation + 1 == held_anchor.generation
        );
        if !still_pending {
            let controller = match self.controllers.for_origin(&held_anchor.origin) {
                Ok(controller) => controller,
                Err(_) => {
                    self.hold(held(issued.take()));
                    return Err(SessionError::CleanupUncertain);
                }
            };
            if self
                .revoke_issued(controller.as_ref(), issued.credentials())
                .await
                .is_err()
            {
                self.hold(held(issued.take()));
                return Err(SessionError::RevokeFailed);
            }
            issued.discard();
            return Err(SessionError::LocalStateFailed);
        }
        self.commit_successor(view, held_anchor, issued).await
    }

    /// Commit an issued successor: credential record, non-secret summary,
    /// trusted anchor, then in-memory publication.
    ///
    /// Reported commit failures keep the successor held for
    /// [`Self::retry_commit`] or logout. A dropped future leaves it in
    /// cleanup-only custody for logout.
    async fn commit_successor(
        &self,
        view: SessionView,
        anchor: AnchorRecord,
        mut issued: IssuedGuard<'_>,
    ) -> Result<SessionView, SessionError> {
        let held = |credentials: IssuedCredentials, error: SessionError| HeldCommit {
            view: view.clone(),
            anchor: anchor.clone(),
            credentials,
            error,
        };
        let record = build_profile_record(&anchor, anchor.generation, issued.credentials());
        let payload = match encode_profile(&record) {
            Ok(payload) => payload,
            Err(_) => {
                self.hold(held(issued.take(), SessionError::VaultWriteFailed));
                return Err(SessionError::VaultWriteFailed);
            }
        };
        if self
            .vault
            .write_profile(anchor.profile_id, &payload)
            .await
            .is_err()
        {
            self.hold(held(issued.take(), SessionError::VaultWriteFailed));
            return Err(SessionError::VaultWriteFailed);
        }
        drop(payload);
        let mut document = SessionDocument::new(None, Some(metadata_session_from(&view)))
            .with_connections(self.stored_connections());
        document.touch_connection(&view.origin, view.label.clone(), now_ms());
        if self.metadata.write_document(&document).is_err() {
            self.hold(held(issued.take(), SessionError::LocalStateFailed));
            return Err(SessionError::LocalStateFailed);
        }
        if self.write_anchor(&anchor).await.is_err() {
            self.hold(held(issued.take(), SessionError::VaultWriteFailed));
            return Err(SessionError::VaultWriteFailed);
        }
        self.publish(view.clone(), issued)?;
        Ok(view)
    }

    /// Revoke the server session before clearing local custody.
    pub(crate) async fn logout(&self) -> Result<(), SessionError> {
        let _ops = self.ops.lock().await;

        // The anchor is read before any in-memory credential is taken so a
        // transient vault failure leaves the retryable state untouched.
        let anchor = self.load_anchor().await?;
        let mut memory = self.take_memory();
        let label = memory
            .active
            .as_ref()
            .and_then(|session| session.view.label.clone())
            .or_else(|| self.stored_label());

        let memory_cleanup_marked = match anchor.as_ref() {
            Some(anchor) if anchor.state == AnchorState::LogoutPending => true,
            Some(anchor) if memory.held.is_some() || !memory.orphaned.is_empty() => {
                self.write_anchor(&AnchorRecord {
                    state: AnchorState::LogoutPending,
                    ..anchor.clone()
                })
                .await?;
                true
            }
            _ => false,
        };

        self.revoke_orphaned(&mut memory).await?;

        let Some(anchor) = anchor else {
            self.revoke_held(&mut memory).await?;
            if let Some(session) = memory.active.as_ref() {
                let controller = self
                    .controllers
                    .for_origin(&session.view.origin)
                    .map_err(|_| SessionError::CleanupUncertain)?;
                match controller
                    .logout(&session.access, &CancellationToken::new())
                    .await
                {
                    Ok(()) => memory.active = None,
                    Err(SessionError::ReloginRequired) => {
                        return Err(SessionError::CleanupUncertain);
                    }
                    Err(_) => return Err(SessionError::RevokeFailed),
                }
            }
            self.clear_stale_session_document()?;
            return memory.finish(Ok(()));
        };

        match anchor.state {
            AnchorState::LoggedOut | AnchorState::CleanupPending => {
                self.revoke_held(&mut memory).await?;
                memory.active = None;
                let result = self.finish_logout_local(&anchor, label).await;
                memory.finish(result)
            }
            AnchorState::Ready | AnchorState::CommitPending | AnchorState::LogoutPending => {
                // The anchor must be non-restorable before any network attempt.
                if !memory_cleanup_marked {
                    self.write_anchor(&AnchorRecord {
                        state: AnchorState::LogoutPending,
                        ..anchor.clone()
                    })
                    .await?;
                }
                self.revoke_held(&mut memory).await?;
                let controller = match self.controllers.for_origin(&anchor.origin) {
                    Ok(controller) => controller,
                    Err(_) if memory.active.is_none() => {
                        self.finish_uncertain_logout(&anchor, label).await?;
                        return Err(SessionError::CleanupUncertain);
                    }
                    Err(_) => return Err(SessionError::CleanupUncertain),
                };

                let record = self
                    .load_profile(anchor.profile_id)
                    .await?
                    .filter(|record| {
                        record.matches_binding(&anchor) && record_refresh(record).is_some()
                    });
                match record {
                    Some(record) => {
                        if let Some(session) = memory.active.as_ref() {
                            match controller
                                .logout(&session.access, &CancellationToken::new())
                                .await
                            {
                                Ok(()) => {
                                    memory.active = None;
                                    let result = self.finish_logout_local(&anchor, label).await;
                                    return memory.finish(result);
                                }
                                Err(SessionError::ReloginRequired) => memory.active = None,
                                Err(_) => {
                                    // The durable refresh record is sufficient
                                    // for retry; the rejected access session is
                                    // no longer published as authenticated.
                                    memory.active = None;
                                    return Err(SessionError::RevokeFailed);
                                }
                            }
                        }
                        let result = self
                            .logout_record(controller.as_ref(), &anchor, &record, label)
                            .await;
                        memory.finish(result)
                    }
                    None => {
                        if let Some(session) = memory.active.as_ref() {
                            match controller
                                .logout(&session.access, &CancellationToken::new())
                                .await
                            {
                                Ok(()) => {
                                    memory.active = None;
                                    let result = self.finish_logout_local(&anchor, label).await;
                                    return memory.finish(result);
                                }
                                Err(SessionError::ReloginRequired) => {
                                    memory.active = None;
                                    self.finish_uncertain_logout(&anchor, label).await?;
                                    return Err(SessionError::CleanupUncertain);
                                }
                                Err(_) => return Err(SessionError::RevokeFailed),
                            }
                        }
                        self.finish_uncertain_logout(&anchor, label).await?;
                        Err(SessionError::CleanupUncertain)
                    }
                }
            }
        }
    }

    async fn commit_login(
        &self,
        controller: &dyn Controller,
        redeemed: RedeemedSession,
        label: Option<String>,
    ) -> Result<SessionView, SessionError> {
        let RedeemedSession {
            cluster_id,
            principal_id,
            email,
            display_name,
            issued: credentials,
        } = redeemed;
        let mut issued = IssuedGuard {
            manager: self,
            issued: Some(OrphanedCredentials {
                origin: controller.canonical_origin().to_owned(),
                credentials,
            }),
        };
        let _ops = self.ops.lock().await;
        if let Some(error) = self.login_error() {
            return self.reject_issued(controller, &mut issued, error).await;
        }

        let existing = match self.load_anchor().await {
            Ok(existing) => existing,
            Err(error) => return self.reject_issued(controller, &mut issued, error).await,
        };
        let (profile_id, old_generation) = match &existing {
            None => (Uuid::new_v4(), 0),
            Some(anchor)
                if anchor.state == AnchorState::LoggedOut
                    && anchor.cluster_id == cluster_id
                    && anchor.principal_id == principal_id =>
            {
                (anchor.profile_id, anchor.generation)
            }
            Some(anchor)
                if matches!(
                    anchor.state,
                    AnchorState::CommitPending
                        | AnchorState::LogoutPending
                        | AnchorState::CleanupPending
                ) =>
            {
                return self
                    .reject_issued(controller, &mut issued, SessionError::CleanupUncertain)
                    .await;
            }
            Some(_) => {
                return self
                    .reject_issued(controller, &mut issued, SessionError::ProfileConflict)
                    .await;
            }
        };

        let new_generation = old_generation + 1;
        let label = sanitize_label(label).or_else(|| self.stored_label());
        let pending = AnchorRecord {
            version: ANCHOR_VERSION,
            profile_id,
            origin: controller.canonical_origin().to_owned(),
            cluster_id: cluster_id.clone(),
            principal_id,
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
            email,
            display_name,
            label: label.clone(),
            access_expires_at_unix_ms: issued.credentials().access_expires_at_unix_ms,
        };

        // The anchor becomes non-restorable before the issued credential is
        // committed.
        if self.write_anchor(&pending).await.is_err() {
            return self
                .reject_issued(controller, &mut issued, SessionError::VaultWriteFailed)
                .await;
        }

        self.commit_successor(view, ready, issued).await
    }

    async fn reject_issued<T>(
        &self,
        controller: &dyn Controller,
        issued: &mut IssuedGuard<'_>,
        error: SessionError,
    ) -> Result<T, SessionError> {
        if self
            .revoke_issued(controller, issued.credentials())
            .await
            .is_ok()
        {
            issued.discard();
            Err(error)
        } else {
            Err(SessionError::CleanupUncertain)
        }
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
        let credentials = match controller
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
        let issued = IssuedGuard {
            manager: self,
            issued: Some(OrphanedCredentials {
                origin: anchor.origin.clone(),
                credentials,
            }),
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
            email: session.email.clone(),
            display_name: session.display_name.clone(),
            label: sanitize_label(session.label.clone()),
            access_expires_at_unix_ms: issued.credentials().access_expires_at_unix_ms,
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

    /// Remove unusable local material without claiming remote revocation.
    async fn finish_uncertain_logout(
        &self,
        anchor: &AnchorRecord,
        label: Option<String>,
    ) -> Result<(), SessionError> {
        self.vault
            .delete_profile(anchor.profile_id)
            .await
            .map_err(|_| SessionError::LocalCleanupFailed)?;
        self.finish_logged_out_document(anchor, label)
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
        )
        .with_connections(self.stored_connections());
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
                let cleared = SessionDocument::new(document.hint, None)
                    .with_connections(document.connections);
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
            // as absent rather than as a usable record.
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

    fn publish(&self, view: SessionView, mut issued: IssuedGuard<'_>) -> Result<(), SessionError> {
        let mut slots = self.slots();
        if slots.cleanup_only || !slots.orphaned.is_empty() {
            drop(slots);
            return Err(SessionError::CleanupUncertain);
        }
        let access = issued.take().access;
        // A published session supersedes any successor still held from an
        // earlier failed commit.
        slots.active = Some(ActiveSession { view, access });
        slots.held = None;
        Ok(())
    }

    fn active_view(&self) -> Option<SessionView> {
        let slots = self.slots();
        if slots.cleanup_only || !slots.orphaned.is_empty() {
            None
        } else {
            slots.active.as_ref().map(|session| session.view.clone())
        }
    }

    fn login_error(&self) -> Option<SessionError> {
        let slots = self.slots();
        if slots.cleanup_only || slots.held.is_some() || !slots.orphaned.is_empty() {
            Some(SessionError::CleanupUncertain)
        } else if slots.active.is_some() {
            Some(SessionError::ProfileConflict)
        } else {
            None
        }
    }

    fn custody_error(&self) -> Option<SessionError> {
        let slots = self.slots();
        if slots.cleanup_only || !slots.orphaned.is_empty() {
            Some(SessionError::CleanupUncertain)
        } else {
            slots.held.as_ref().map(|held| held.error)
        }
    }

    fn hold(&self, held: HeldCommit) {
        self.slots().held = Some(held);
    }

    fn take_held(&self) -> Option<HeldCommit> {
        self.slots().held.take()
    }

    fn take_memory(&self) -> MemoryCustody<'_> {
        let mut slots = self.slots();
        slots.cleanup_only = true;
        MemoryCustody {
            manager: self,
            active: slots.active.take(),
            held: slots.held.take(),
            orphaned: std::mem::take(&mut slots.orphaned),
            cleanup_only: true,
        }
    }

    async fn revoke_orphaned(&self, memory: &mut MemoryCustody<'_>) -> Result<(), SessionError> {
        while let Some(issued) = memory.orphaned.last() {
            let controller = self
                .controllers
                .for_origin(&issued.origin)
                .map_err(|_| SessionError::CleanupUncertain)?;
            self.revoke_issued(controller.as_ref(), &issued.credentials)
                .await
                .map_err(|_| SessionError::RevokeFailed)?;
            memory.orphaned.pop();
        }
        Ok(())
    }

    async fn revoke_held(&self, memory: &mut MemoryCustody<'_>) -> Result<(), SessionError> {
        let Some(held) = memory.held.as_ref() else {
            return Ok(());
        };
        let controller = self
            .controllers
            .for_origin(&held.anchor.origin)
            .map_err(|_| SessionError::CleanupUncertain)?;
        self.revoke_issued(controller.as_ref(), &held.credentials)
            .await
            .map_err(|_| SessionError::RevokeFailed)?;
        memory.held = None;
        Ok(())
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
        email: view.email.clone(),
        display_name: view.display_name.clone(),
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

/// The probe outcome as the connect card and titlebar dot read it.
fn probe_view(outcome: Result<(), ProbeFailure>) -> ProbeView {
    match outcome {
        Ok(()) => ProbeView {
            status: ProbeStatus::Ready,
            diagnostic: None,
        },
        Err(failure) => ProbeView {
            status: ProbeStatus::Unreachable,
            diagnostic: Some(match failure {
                ProbeFailure::Dns => ProbeDiagnostic::Dns,
                ProbeFailure::Tcp => ProbeDiagnostic::Tcp,
                ProbeFailure::Tls => ProbeDiagnostic::Tls,
                ProbeFailure::Http => ProbeDiagnostic::Http,
                ProbeFailure::Timeout => ProbeDiagnostic::Timeout,
            }),
        },
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod probe_tests {
    use super::*;

    #[test]
    fn probe_failures_name_the_step_that_failed() {
        assert_eq!(probe_view(Ok(())).status, ProbeStatus::Ready);
        for (failure, diagnostic) in [
            (ProbeFailure::Dns, ProbeDiagnostic::Dns),
            (ProbeFailure::Tcp, ProbeDiagnostic::Tcp),
            (ProbeFailure::Tls, ProbeDiagnostic::Tls),
            (ProbeFailure::Http, ProbeDiagnostic::Http),
            (ProbeFailure::Timeout, ProbeDiagnostic::Timeout),
        ] {
            let view = probe_view(Err(failure));
            assert_eq!(view.status, ProbeStatus::Unreachable);
            assert_eq!(view.diagnostic, Some(diagnostic));
        }
        assert_eq!(
            serde_json::to_value(probe_view(Err(ProbeFailure::Tls))).unwrap(),
            serde_json::json!({ "status": "unreachable", "diagnostic": "tls" })
        );
    }
}
