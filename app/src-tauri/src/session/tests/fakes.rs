//! Scripted doubles for the custody ports, with one shared operation log so
//! tests can assert exact ordering across vault, controller, and metadata.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use kontainer_native_client::{
    AccessCredential, ContainerPage, ContainerPageRequest, LoginVerifier, NativeClientError,
    NodeId, RefreshCredential,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::session::browser::BrowserOpener;
use crate::session::controller::{
    Controller, ControllerFactory, ControllerFuture, IssuedCredentials, LoginStart,
    RedeemedSession, Redemption,
};
use crate::session::errors::SessionError;
use crate::session::manager::SessionManager;
use crate::session::metadata::{MetadataError, SessionMetadataStore};
use crate::session::models::{
    decode_anchor, decode_profile, encode_anchor, encode_profile, AnchorRecord, AnchorState,
    LoginHint, MetadataSession, ProfileRecord, SessionDocument, SessionLoginInput, SessionView,
    ANCHOR_VERSION,
};
use crate::session::vault::{CredentialVault, VaultError, VaultFuture};

pub(crate) type OpLog = Arc<Mutex<Vec<String>>>;

pub(crate) const ORIGIN: &str = "https://controller.example/";
pub(crate) const OTHER_ORIGIN: &str = "https://other.example/";
pub(crate) const CLUSTER: &str = "test";
pub(crate) const PRINCIPAL_ID: Uuid = Uuid::from_u128(0xaa);
pub(crate) const OTHER_PRINCIPAL_ID: Uuid = Uuid::from_u128(0xab);
pub(crate) const PROFILE_ID: Uuid = Uuid::from_u128(0xbb);

pub(crate) fn new_log() -> OpLog {
    Arc::new(Mutex::new(Vec::new()))
}

fn record(log: &OpLog, entry: impl Into<String>) {
    log.lock().unwrap().push(entry.into());
}

/// Position of the first logged entry starting with `needle`, panicking when
/// it is absent so an ordering assertion cannot pass on a missing step.
pub(crate) fn position(entries: &[String], needle: &str) -> usize {
    entries
        .iter()
        .position(|entry| entry.starts_with(needle))
        .unwrap_or_else(|| panic!("missing operation {needle} in {entries:?}"))
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

pub(crate) fn future_ms() -> i64 {
    now_ms() + 60 * 60 * 1000
}

pub(crate) fn past_ms() -> i64 {
    now_ms() - 60 * 1000
}

fn hex(seed: u8, groups: usize) -> String {
    format!("{seed:02x}").repeat(groups)
}

pub(crate) fn fake_access(seed: u8) -> AccessCredential {
    AccessCredential::parse(&format!("kta_{}.{}", hex(seed, 16), hex(seed, 32)))
        .expect("canonical fake access credential")
}

pub(crate) fn fake_refresh(seed: u8) -> RefreshCredential {
    RefreshCredential::parse(&format!("ktr_{}.{}", hex(seed, 16), hex(seed, 32)))
        .expect("canonical fake refresh credential")
}

pub(crate) fn issued(
    seed: u8,
    access_expires_at_unix_ms: i64,
    refresh_expires_at_unix_ms: i64,
) -> IssuedCredentials {
    IssuedCredentials {
        access: fake_access(seed),
        refresh: fake_refresh(seed),
        access_expires_at_unix_ms,
        refresh_expires_at_unix_ms,
    }
}

/// A successful redemption for pair `(test, PRINCIPAL_ID)`.
pub(crate) fn redeemed(seed: u8) -> RedeemedSession {
    redeemed_for(seed, PRINCIPAL_ID)
}

pub(crate) fn redeemed_for(seed: u8, principal_id: Uuid) -> RedeemedSession {
    RedeemedSession {
        cluster_id: CLUSTER.to_owned(),
        principal_id,
        github_login: "owner".to_owned(),
        issued: issued(seed, future_ms(), future_ms()),
    }
}

pub(crate) fn login_input(origin: &str) -> SessionLoginInput {
    SessionLoginInput {
        origin: origin.to_owned(),
        label: Some("Default".to_owned()),
    }
}

pub(crate) struct FakeVault {
    log: OpLog,
    anchor: Mutex<Option<Vec<u8>>>,
    profiles: Mutex<HashMap<Uuid, Vec<u8>>>,
    pub(crate) fail_reads: AtomicBool,
    pub(crate) fail_anchor_read_number: AtomicUsize,
    anchor_reads: AtomicUsize,
    pub(crate) fail_profile_writes: AtomicBool,
    /// Fails only `Ready` anchor writes, so a test can stop the commit step
    /// without breaking the earlier pending-anchor write.
    pub(crate) fail_ready_anchor_writes: AtomicBool,
    /// Fails only the non-restorable anchor markers (`CommitPending`,
    /// `LogoutPending`), so a test can prove that a failed marker write stops
    /// the network attempt without breaking the terminal `Ready` write.
    pub(crate) fail_pending_anchor_writes: AtomicBool,
    pub(crate) fail_deletes: AtomicBool,
}

impl FakeVault {
    fn new(log: OpLog) -> Self {
        Self {
            log,
            anchor: Mutex::new(None),
            profiles: Mutex::new(HashMap::new()),
            fail_reads: AtomicBool::new(false),
            fail_anchor_read_number: AtomicUsize::new(0),
            anchor_reads: AtomicUsize::new(0),
            fail_profile_writes: AtomicBool::new(false),
            fail_ready_anchor_writes: AtomicBool::new(false),
            fail_pending_anchor_writes: AtomicBool::new(false),
            fail_deletes: AtomicBool::new(false),
        }
    }

    pub(crate) fn seed_anchor(&self, record: &AnchorRecord) {
        *self.anchor.lock().unwrap() = Some(encode_anchor(record).expect("anchor encodes"));
    }

    pub(crate) fn seed_profile(&self, record: &ProfileRecord) {
        self.profiles.lock().unwrap().insert(
            record.profile_id,
            encode_profile(record).expect("record encodes").to_vec(),
        );
    }

    pub(crate) fn anchor(&self) -> Option<AnchorRecord> {
        self.anchor
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|bytes| decode_anchor(bytes).ok())
    }

    pub(crate) fn anchor_state(&self) -> Option<AnchorState> {
        self.anchor().map(|anchor| anchor.state)
    }

    pub(crate) fn profile(&self, profile_id: Uuid) -> Option<ProfileRecord> {
        self.profiles
            .lock()
            .unwrap()
            .get(&profile_id)
            .and_then(|bytes| decode_profile(bytes).ok())
    }

    pub(crate) fn has_profile(&self, profile_id: Uuid) -> bool {
        self.profiles.lock().unwrap().contains_key(&profile_id)
    }

    pub(crate) fn corrupt_anchor(&self) {
        *self.anchor.lock().unwrap() = Some(b"not an anchor".to_vec());
    }

    pub(crate) fn corrupt_profile(&self, profile_id: Uuid) {
        self.profiles
            .lock()
            .unwrap()
            .insert(profile_id, b"not a record".to_vec());
    }

    /// Remove a record without logging, for crash-state fixtures.
    pub(crate) fn delete_profile_silently(&self, profile_id: Uuid) {
        self.profiles.lock().unwrap().remove(&profile_id);
    }

    /// Remove the anchor without logging, for crash-state fixtures.
    pub(crate) fn remove_anchor_silently(&self) {
        *self.anchor.lock().unwrap() = None;
    }
}

impl CredentialVault for FakeVault {
    fn read_anchor(&self) -> VaultFuture<'_, Result<Option<Zeroizing<Vec<u8>>>, VaultError>> {
        record(&self.log, "read_anchor");
        let read_number = self.anchor_reads.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_reads.load(Ordering::SeqCst)
            || self.fail_anchor_read_number.load(Ordering::SeqCst) == read_number
        {
            return Box::pin(async { Err(VaultError::Unavailable) });
        }
        let value = self.anchor.lock().unwrap().clone().map(Zeroizing::new);
        Box::pin(async move { Ok(value) })
    }

    fn write_anchor(&self, payload: Vec<u8>) -> VaultFuture<'_, Result<(), VaultError>> {
        let state = decode_anchor(&payload).ok().map(|anchor| anchor.state);
        record(
            &self.log,
            format!(
                "write_anchor:{}",
                state.map(state_name).unwrap_or("unreadable")
            ),
        );
        if state == Some(AnchorState::Ready) && self.fail_ready_anchor_writes.load(Ordering::SeqCst)
        {
            return Box::pin(async { Err(VaultError::Write) });
        }
        if matches!(
            state,
            Some(AnchorState::CommitPending) | Some(AnchorState::LogoutPending)
        ) && self.fail_pending_anchor_writes.load(Ordering::SeqCst)
        {
            return Box::pin(async { Err(VaultError::Write) });
        }
        *self.anchor.lock().unwrap() = Some(payload);
        Box::pin(async { Ok(()) })
    }

    fn read_profile(
        &self,
        profile_id: Uuid,
    ) -> VaultFuture<'_, Result<Option<Zeroizing<Vec<u8>>>, VaultError>> {
        record(&self.log, "read_profile");
        if self.fail_reads.load(Ordering::SeqCst) {
            return Box::pin(async { Err(VaultError::Unavailable) });
        }
        let value = self
            .profiles
            .lock()
            .unwrap()
            .get(&profile_id)
            .cloned()
            .map(Zeroizing::new);
        Box::pin(async move { Ok(value) })
    }

    fn write_profile<'a>(
        &'a self,
        profile_id: Uuid,
        payload: &'a [u8],
    ) -> VaultFuture<'a, Result<(), VaultError>> {
        let generation = decode_profile(payload)
            .map(|record| record.generation)
            .map(|generation| generation.to_string())
            .unwrap_or_else(|_| "unreadable".to_owned());
        record(&self.log, format!("write_profile:{generation}"));
        if self.fail_profile_writes.load(Ordering::SeqCst) {
            return Box::pin(async { Err(VaultError::Write) });
        }
        self.profiles
            .lock()
            .unwrap()
            .insert(profile_id, payload.to_vec());
        Box::pin(async { Ok(()) })
    }

    fn delete_profile(&self, profile_id: Uuid) -> VaultFuture<'_, Result<(), VaultError>> {
        record(&self.log, "delete_profile");
        if self.fail_deletes.load(Ordering::SeqCst) {
            return Box::pin(async { Err(VaultError::Delete) });
        }
        self.profiles.lock().unwrap().remove(&profile_id);
        Box::pin(async { Ok(()) })
    }
}

fn state_name(state: AnchorState) -> &'static str {
    match state {
        AnchorState::Ready => "ready",
        AnchorState::CommitPending => "commit_pending",
        AnchorState::LogoutPending => "logout_pending",
        AnchorState::CleanupPending => "cleanup_pending",
        AnchorState::LoggedOut => "logged_out",
    }
}

pub(crate) struct FakeMetadata {
    log: OpLog,
    document: Mutex<Option<SessionDocument>>,
    pub(crate) fail_reads: AtomicBool,
    /// Simulates a stored document that no longer matches the strict schema.
    pub(crate) corrupt_reads: AtomicBool,
    pub(crate) fail_writes: AtomicBool,
}

impl FakeMetadata {
    fn new(log: OpLog) -> Self {
        Self {
            log,
            document: Mutex::new(None),
            fail_reads: AtomicBool::new(false),
            corrupt_reads: AtomicBool::new(false),
            fail_writes: AtomicBool::new(false),
        }
    }

    pub(crate) fn set_document(&self, document: SessionDocument) {
        *self.document.lock().unwrap() = Some(document);
    }

    pub(crate) fn document(&self) -> Option<SessionDocument> {
        self.document.lock().unwrap().clone()
    }
}

impl SessionMetadataStore for FakeMetadata {
    fn read_document(&self) -> Result<Option<SessionDocument>, MetadataError> {
        record(&self.log, "read_document");
        if self.corrupt_reads.load(Ordering::SeqCst) {
            return Err(MetadataError::Corrupt);
        }
        if self.fail_reads.load(Ordering::SeqCst) {
            return Err(MetadataError::Unavailable);
        }
        Ok(self.document.lock().unwrap().clone())
    }

    fn write_document(&self, document: &SessionDocument) -> Result<(), MetadataError> {
        record(&self.log, "write_document");
        if self.fail_writes.load(Ordering::SeqCst) {
            return Err(MetadataError::Unavailable);
        }
        *self.document.lock().unwrap() = Some(document.clone());
        Ok(())
    }
}

pub(crate) struct FakeBrowser {
    log: OpLog,
    opened: Mutex<Vec<String>>,
    pub(crate) fail: AtomicBool,
}

impl FakeBrowser {
    fn new(log: OpLog) -> Self {
        Self {
            log,
            opened: Mutex::new(Vec::new()),
            fail: AtomicBool::new(false),
        }
    }

    pub(crate) fn opened(&self) -> Vec<String> {
        self.opened.lock().unwrap().clone()
    }
}

impl BrowserOpener for FakeBrowser {
    fn open(&self, url: &str) -> Result<(), SessionError> {
        record(&self.log, "open_browser");
        if self.fail.load(Ordering::SeqCst) {
            return Err(SessionError::BrowserOpenFailed);
        }
        self.opened.lock().unwrap().push(url.to_owned());
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct Script {
    pub(crate) start: VecDeque<Result<LoginStart, SessionError>>,
    pub(crate) redeem: VecDeque<Result<Redemption, SessionError>>,
    pub(crate) refresh: VecDeque<Result<IssuedCredentials, SessionError>>,
    pub(crate) logout: VecDeque<Result<(), SessionError>>,
    pub(crate) discover: VecDeque<Result<Option<NodeId>, NativeClientError>>,
    pub(crate) discover_gate: Option<Arc<Notify>>,
    pub(crate) containers: VecDeque<Result<ContainerPage, NativeClientError>>,
}

pub(crate) struct FakeControllerFactory {
    log: OpLog,
    script: Arc<Mutex<Script>>,
    fail_origin: AtomicBool,
}

impl FakeControllerFactory {
    fn new(log: OpLog) -> Self {
        Self {
            log,
            script: Arc::new(Mutex::new(Script::default())),
            fail_origin: AtomicBool::new(false),
        }
    }

    pub(crate) fn script(&self) -> MutexGuard<'_, Script> {
        self.script.lock().unwrap()
    }

    pub(crate) fn fail_origin(&self) {
        self.fail_origin.store(true, Ordering::SeqCst);
    }
}

impl ControllerFactory for FakeControllerFactory {
    fn for_origin(&self, origin: &str) -> Result<Arc<dyn Controller>, SessionError> {
        if self.fail_origin.load(Ordering::SeqCst) {
            return Err(SessionError::InvalidOrigin);
        }
        Ok(Arc::new(FakeController {
            origin: origin.to_owned(),
            log: self.log.clone(),
            script: Arc::clone(&self.script),
        }))
    }
}

struct FakeController {
    origin: String,
    log: OpLog,
    script: Arc<Mutex<Script>>,
}

impl Controller for FakeController {
    fn canonical_origin(&self) -> &str {
        &self.origin
    }

    fn discover_node<'a>(
        &'a self,
        access: &'a AccessCredential,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<Option<NodeId>, NativeClientError>> {
        record(
            &self.log,
            format!("discover:{}", public_tag(access.expose())),
        );
        let (outcome, gate) = {
            let mut script = self.script.lock().unwrap();
            (
                script
                    .discover
                    .pop_front()
                    .unwrap_or(Err(NativeClientError::InvalidResponse)),
                script.discover_gate.clone(),
            )
        };
        let log = self.log.clone();
        Box::pin(async move {
            if let Some(gate) = gate {
                gate.notified().await;
            }
            record(&log, "discover_done");
            outcome
        })
    }

    fn list_containers<'a>(
        &'a self,
        access: &'a AccessCredential,
        node_id: &'a NodeId,
        page: &'a ContainerPageRequest,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ContainerPage, NativeClientError>> {
        record(
            &self.log,
            format!(
                "containers:{}:{}:{}:{}",
                public_tag(access.expose()),
                node_id,
                page.limit,
                page.cursor.as_deref().unwrap_or("")
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .containers
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn start_login<'a>(
        &'a self,
        _verifier: &'a LoginVerifier,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<LoginStart, SessionError>> {
        record(&self.log, "start_login");
        let outcome = self
            .script
            .lock()
            .unwrap()
            .start
            .pop_front()
            .unwrap_or_else(|| {
                Ok(LoginStart {
                    attempt_id: Uuid::from_u128(0x11),
                    authorize_url: "https://github.com/login/oauth/authorize".to_owned(),
                    expires_at_unix_ms: future_ms() + 10 * 60 * 1000,
                })
            });
        Box::pin(async move { outcome })
    }

    fn redeem_login<'a>(
        &'a self,
        _attempt_id: Uuid,
        _verifier: &'a LoginVerifier,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<Redemption, SessionError>> {
        record(&self.log, "redeem_begin");
        let log = self.log.clone();
        let outcome = self
            .script
            .lock()
            .unwrap()
            .redeem
            .pop_front()
            .unwrap_or(Err(SessionError::InvalidResponse));
        Box::pin(async move {
            record(&log, "redeem_end");
            outcome
        })
    }

    fn refresh_session<'a>(
        &'a self,
        refresh: &'a RefreshCredential,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<IssuedCredentials, SessionError>> {
        record(
            &self.log,
            format!(
                "refresh:{}:{}",
                origin_label(&self.origin),
                public_tag(refresh.expose())
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .refresh
            .pop_front()
            .unwrap_or(Err(SessionError::ControllerUnavailable));
        Box::pin(async move { outcome })
    }

    fn logout<'a>(
        &'a self,
        access: &'a AccessCredential,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<(), SessionError>> {
        record(
            &self.log,
            format!(
                "logout:{}:{}",
                origin_label(&self.origin),
                public_tag(access.expose())
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .logout
            .pop_front()
            .unwrap_or(Err(SessionError::ControllerUnavailable));
        Box::pin(async move { outcome })
    }
}

/// The host part of an origin, so a test can prove which controller received a
/// credential.
fn origin_label(origin: &str) -> String {
    origin
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_owned()
}

/// The eight leading characters of a credential's public identifier: enough to
/// tell two scripted credentials apart in the operation log without carrying
/// any secret material.
fn public_tag(credential: &str) -> String {
    credential
        .split_once('.')
        .map(|(public_id, _)| public_id)
        .unwrap_or(credential)
        .rsplit_once('_')
        .map(|(_, hex)| hex[..8.min(hex.len())].to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}

pub(crate) struct Harness {
    pub(crate) manager: SessionManager,
    pub(crate) vault: Arc<FakeVault>,
    pub(crate) metadata: Arc<FakeMetadata>,
    pub(crate) controllers: Arc<FakeControllerFactory>,
    pub(crate) browser: Arc<FakeBrowser>,
    pub(crate) log: OpLog,
}

impl Harness {
    pub(crate) fn new() -> Self {
        let log = new_log();
        let vault = Arc::new(FakeVault::new(log.clone()));
        let metadata = Arc::new(FakeMetadata::new(log.clone()));
        let controllers = Arc::new(FakeControllerFactory::new(log.clone()));
        let browser = Arc::new(FakeBrowser::new(log.clone()));
        let manager = SessionManager::new(
            vault.clone(),
            metadata.clone(),
            controllers.clone(),
            browser.clone(),
        );
        Self {
            manager,
            vault,
            metadata,
            controllers,
            browser,
            log,
        }
    }

    pub(crate) fn entries(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    pub(crate) fn count(&self, prefix: &str) -> usize {
        self.entries()
            .iter()
            .filter(|item| item.starts_with(prefix))
            .count()
    }

    pub(crate) fn restarted_manager(&self) -> SessionManager {
        SessionManager::new(
            self.vault.clone(),
            self.metadata.clone(),
            self.controllers.clone(),
            self.browser.clone(),
        )
    }

    /// Complete one login against the default origin and pair.
    pub(crate) async fn login_ok(&self) -> SessionView {
        self.controllers
            .script()
            .redeem
            .push_back(Ok(Redemption::Ready(redeemed(1))));
        self.manager
            .login(login_input(ORIGIN))
            .await
            .expect("login succeeds")
    }

    /// Seed a durable ready session as a previous process would have left it.
    pub(crate) fn seed_ready_session(&self) -> AnchorRecord {
        let anchor = AnchorRecord {
            version: ANCHOR_VERSION,
            profile_id: PROFILE_ID,
            origin: ORIGIN.to_owned(),
            cluster_id: CLUSTER.to_owned(),
            principal_id: PRINCIPAL_ID,
            generation: 1,
            state: AnchorState::Ready,
        };
        let record = ProfileRecord {
            version: crate::session::models::PROFILE_VERSION,
            profile_id: anchor.profile_id,
            origin: anchor.origin.clone(),
            cluster_id: anchor.cluster_id.clone(),
            principal_id: anchor.principal_id,
            generation: anchor.generation,
            refresh_expires_at_unix_ms: future_ms(),
            refresh_credential: fake_refresh(1).expose().to_owned(),
        };
        self.vault.seed_anchor(&anchor);
        self.vault.seed_profile(&record);
        self.metadata.set_document(SessionDocument::new(
            None,
            Some(MetadataSession {
                profile_id: anchor.profile_id,
                origin: anchor.origin.clone(),
                cluster_id: anchor.cluster_id.clone(),
                principal_id: anchor.principal_id,
                github_login: "owner".to_owned(),
                label: Some("Default".to_owned()),
            }),
        ));
        anchor
    }

    /// The login hint currently stored for the login screen.
    pub(crate) fn hint(&self) -> Option<LoginHint> {
        self.metadata.document().and_then(|document| document.hint)
    }

    /// The trusted profile UUID currently held by the vault anchor.
    pub(crate) fn anchor_profile_id(&self) -> Uuid {
        self.vault.anchor().expect("anchor present").profile_id
    }
}
