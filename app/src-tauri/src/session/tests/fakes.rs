//! Scripted doubles for the custody ports, with one shared operation log so
//! tests can assert exact ordering across vault, controller, and metadata.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use kontainer_native_client::{
    AccessCredential, AdmittedOperation, ContainerAction, ContainerActionOptions, ContainerList,
    ContainerTarget, ControllerSettings, IdempotencyKey, ImageList, ImageNodes, NativeClientError,
    NodeId, NodeList, OperationEvent, OperationSnapshot, ProbeFailure, RefreshCredential,
    SampleSeries, VolumeList, VolumeName, VolumeQuery, VolumeSpec,
};
use kontainer_native_client::{
    AttachedContainer, NetworkId, NetworkList, NetworkPlacement, NetworkSpec,
};
use kontainer_native_client::{ServiceAction, ServiceDetail, ServiceId, ServiceList};
use kontainer_native_client::{StackList, StackName, StackRemovalPlan};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::session::controller::{
    Controller, ControllerFactory, ControllerFuture, IssuedCredentials, RedeemedSession,
};
use crate::session::errors::{LoginFailure, SessionError};
use crate::session::manager::SessionManager;
use crate::session::metadata::{MetadataError, SessionMetadataStore};
use crate::session::models::{
    decode_anchor, decode_profile, encode_anchor, encode_profile, AnchorRecord, AnchorState,
    LoginHint, MetadataSession, PasswordLoginInput, ProfileRecord, SessionDocument, SessionView,
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
pub(crate) const EMAIL: &str = "owner@example.com";
pub(crate) const DISPLAY_NAME: &str = "Ada Owner";
pub(crate) const PASSWORD: &str = "correct horse battery staple";

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
        email: EMAIL.to_owned(),
        display_name: DISPLAY_NAME.to_owned(),
        issued: issued(seed, future_ms(), future_ms()),
    }
}

pub(crate) fn login_input(origin: &str) -> PasswordLoginInput {
    PasswordLoginInput {
        origin: origin.to_owned(),
        email: EMAIL.to_owned(),
        password: Zeroizing::new(PASSWORD.to_owned()),
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

#[derive(Default)]
pub(crate) struct Script {
    pub(crate) login: VecDeque<Result<RedeemedSession, LoginFailure>>,
    pub(crate) register: VecDeque<Result<RedeemedSession, LoginFailure>>,
    pub(crate) bootstrap: VecDeque<Result<bool, LoginFailure>>,
    pub(crate) probe: VecDeque<Result<(), ProbeFailure>>,
    pub(crate) refresh: VecDeque<Result<IssuedCredentials, SessionError>>,
    pub(crate) logout: VecDeque<Result<(), SessionError>>,
    pub(crate) nodes: VecDeque<Result<NodeList, NativeClientError>>,
    pub(crate) nodes_gate: Option<Arc<Notify>>,
    pub(crate) samples: VecDeque<Result<SampleSeries, NativeClientError>>,
    pub(crate) settings: VecDeque<Result<ControllerSettings, NativeClientError>>,
    pub(crate) operations: VecDeque<Result<OperationSnapshot, NativeClientError>>,
    pub(crate) events: VecDeque<Result<Vec<OperationEvent>, NativeClientError>>,
    pub(crate) admissions: VecDeque<Result<AdmittedOperation, NativeClientError>>,
    pub(crate) containers: VecDeque<Result<ContainerList, NativeClientError>>,
    pub(crate) images: VecDeque<Result<ImageList, NativeClientError>>,
    pub(crate) volumes: VecDeque<Result<VolumeList, NativeClientError>>,
    pub(crate) networks: VecDeque<Result<NetworkList, NativeClientError>>,
    pub(crate) attached: VecDeque<Result<Vec<AttachedContainer>, NativeClientError>>,
    pub(crate) services: VecDeque<Result<ServiceList, NativeClientError>>,
    pub(crate) service_details: VecDeque<Result<ServiceDetail, NativeClientError>>,
    pub(crate) stacks: VecDeque<Result<StackList, NativeClientError>>,
    pub(crate) plans: VecDeque<Result<StackRemovalPlan, NativeClientError>>,
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

    fn list_nodes<'a>(
        &'a self,
        access: &'a AccessCredential,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<NodeList, NativeClientError>> {
        record(&self.log, format!("nodes:{}", public_tag(access.expose())));
        let (outcome, gate) = {
            let mut script = self.script.lock().unwrap();
            (
                script
                    .nodes
                    .pop_front()
                    .unwrap_or(Err(NativeClientError::InvalidResponse)),
                script.nodes_gate.clone(),
            )
        };
        let log = self.log.clone();
        Box::pin(async move {
            if let Some(gate) = gate {
                gate.notified().await;
            }
            record(&log, "nodes_done");
            outcome
        })
    }

    fn samples<'a>(
        &'a self,
        access: &'a AccessCredential,
        node_id: Option<&'a NodeId>,
        window: &'a str,
        bucket: &'a str,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<SampleSeries, NativeClientError>> {
        record(
            &self.log,
            format!(
                "samples:{}:{}:{window}:{bucket}",
                public_tag(access.expose()),
                node_id.map_or_else(|| "cluster".to_owned(), ToString::to_string),
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .samples
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn get_settings<'a>(
        &'a self,
        access: &'a AccessCredential,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ControllerSettings, NativeClientError>> {
        record(
            &self.log,
            format!("get_settings:{}", public_tag(access.expose())),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .settings
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn update_settings<'a>(
        &'a self,
        access: &'a AccessCredential,
        metrics_retention_days: u32,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ControllerSettings, NativeClientError>> {
        record(
            &self.log,
            format!(
                "update_settings:{}:{metrics_retention_days}",
                public_tag(access.expose())
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .settings
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn list_containers<'a>(
        &'a self,
        access: &'a AccessCredential,
        node: Option<&'a NodeId>,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ContainerList, NativeClientError>> {
        record(
            &self.log,
            format!(
                "containers:{}:{}",
                public_tag(access.expose()),
                node.map_or_else(|| "all".to_owned(), ToString::to_string)
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

    fn container_action<'a>(
        &'a self,
        access: &'a AccessCredential,
        target: &'a ContainerTarget,
        action: ContainerAction,
        options: ContainerActionOptions,
        key: &'a IdempotencyKey,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        record(
            &self.log,
            format!(
                "container_action:{}:{}:{}:{}:{}:{:?}:{}",
                public_tag(access.expose()),
                target.node_id,
                target.container_id,
                action.as_str(),
                options.force,
                options.timeout_seconds,
                key.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .admissions
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn containers_bulk_action<'a>(
        &'a self,
        access: &'a AccessCredential,
        action: ContainerAction,
        targets: &'a [ContainerTarget],
        force: bool,
        key: &'a IdempotencyKey,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        record(
            &self.log,
            format!(
                "containers_bulk:{}:{}:{}:{force}:{}",
                public_tag(access.expose()),
                action.as_str(),
                targets.len(),
                key.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .admissions
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn list_images<'a>(
        &'a self,
        access: &'a AccessCredential,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ImageList, NativeClientError>> {
        record(&self.log, format!("images:{}", public_tag(access.expose())));
        let outcome = self
            .script
            .lock()
            .unwrap()
            .images
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn remove_image<'a>(
        &'a self,
        access: &'a AccessCredential,
        image_id: &'a str,
        nodes: &'a ImageNodes,
        force: bool,
        key: &'a IdempotencyKey,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        let nodes = match nodes {
            ImageNodes::All => "all".to_owned(),
            ImageNodes::Only(nodes) => nodes
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        };
        record(
            &self.log,
            format!(
                "remove_image:{}:{image_id}:{nodes}:{force}:{}",
                public_tag(access.expose()),
                key.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .admissions
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn pull_image<'a>(
        &'a self,
        access: &'a AccessCredential,
        reference: &'a str,
        nodes: &'a [NodeId],
        key: &'a IdempotencyKey,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        record(
            &self.log,
            format!(
                "pull_image:{}:{reference}:{}:{}",
                public_tag(access.expose()),
                nodes
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
                key.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .admissions
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn list_volumes<'a>(
        &'a self,
        access: &'a AccessCredential,
        query: &'a VolumeQuery,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<VolumeList, NativeClientError>> {
        record(
            &self.log,
            format!(
                "volumes:{}:{}:{}",
                public_tag(access.expose()),
                query.node.as_ref().map_or("all", NodeId::as_str),
                query.name.as_ref().map_or("any", VolumeName::as_str),
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .volumes
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn create_volume<'a>(
        &'a self,
        access: &'a AccessCredential,
        spec: &'a VolumeSpec,
        key: &'a IdempotencyKey,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        let labels: Vec<_> = spec
            .labels
            .iter()
            .map(|(label, value)| format!("{label}={value}"))
            .collect();
        record(
            &self.log,
            format!(
                "create_volume:{}:{}:{}:{}:{}",
                public_tag(access.expose()),
                spec.node_id,
                spec.name.as_str(),
                labels.join(","),
                key.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .admissions
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn remove_volume<'a>(
        &'a self,
        access: &'a AccessCredential,
        node_id: &'a NodeId,
        name: &'a VolumeName,
        key: &'a IdempotencyKey,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        record(
            &self.log,
            format!(
                "remove_volume:{}:{node_id}:{}:{}",
                public_tag(access.expose()),
                name.as_str(),
                key.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .admissions
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn list_networks<'a>(
        &'a self,
        access: &'a AccessCredential,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<NetworkList, NativeClientError>> {
        record(
            &self.log,
            format!("networks:{}", public_tag(access.expose())),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .networks
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn inspect_network<'a>(
        &'a self,
        access: &'a AccessCredential,
        network_id: &'a NetworkId,
        node: Option<&'a NodeId>,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<Vec<AttachedContainer>, NativeClientError>> {
        record(
            &self.log,
            format!(
                "inspect_network:{}:{}:{}",
                public_tag(access.expose()),
                network_id.as_str(),
                node.map_or("swarm", NodeId::as_str)
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .attached
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn create_network<'a>(
        &'a self,
        access: &'a AccessCredential,
        spec: &'a NetworkSpec,
        key: &'a IdempotencyKey,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        let placement = match &spec.placement {
            NetworkPlacement::Overlay { attachable } => format!("overlay:{attachable}"),
            NetworkPlacement::Bridge { node_id } => format!("bridge:{node_id}"),
        };
        record(
            &self.log,
            format!(
                "create_network:{}:{}:{placement}:{}:{}",
                public_tag(access.expose()),
                spec.name,
                spec.subnet.as_deref().unwrap_or("none"),
                key.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .admissions
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn remove_network<'a>(
        &'a self,
        access: &'a AccessCredential,
        network_id: &'a NetworkId,
        node: Option<&'a NodeId>,
        key: &'a IdempotencyKey,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        record(
            &self.log,
            format!(
                "remove_network:{}:{}:{}:{}",
                public_tag(access.expose()),
                network_id.as_str(),
                node.map_or("swarm", NodeId::as_str),
                key.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .admissions
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn list_services<'a>(
        &'a self,
        access: &'a AccessCredential,
        stack: Option<&'a str>,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ServiceList, NativeClientError>> {
        record(
            &self.log,
            format!(
                "services:{}:{}",
                public_tag(access.expose()),
                stack.unwrap_or("all")
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .services
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn get_service<'a>(
        &'a self,
        access: &'a AccessCredential,
        service_id: &'a ServiceId,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ServiceDetail, NativeClientError>> {
        record(
            &self.log,
            format!(
                "service:{}:{}",
                public_tag(access.expose()),
                service_id.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .service_details
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn service_action<'a>(
        &'a self,
        access: &'a AccessCredential,
        service_id: &'a ServiceId,
        action: ServiceAction,
        version: u64,
        key: &'a IdempotencyKey,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        let action = match action {
            ServiceAction::Scale { replicas } => format!("scale={replicas}"),
            ServiceAction::Restart => "restart".to_owned(),
            ServiceAction::Rollback => "rollback".to_owned(),
        };
        record(
            &self.log,
            format!(
                "service_action:{}:{}:{action}:{version}:{}",
                public_tag(access.expose()),
                service_id.as_str(),
                key.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .admissions
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn list_stacks<'a>(
        &'a self,
        access: &'a AccessCredential,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<StackList, NativeClientError>> {
        record(&self.log, format!("stacks:{}", public_tag(access.expose())));
        let outcome = self
            .script
            .lock()
            .unwrap()
            .stacks
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn stack_removal_plan<'a>(
        &'a self,
        access: &'a AccessCredential,
        name: &'a StackName,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<StackRemovalPlan, NativeClientError>> {
        record(
            &self.log,
            format!(
                "stack_plan:{}:{}",
                public_tag(access.expose()),
                name.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .plans
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn remove_stack<'a>(
        &'a self,
        access: &'a AccessCredential,
        name: &'a StackName,
        key: &'a IdempotencyKey,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        record(
            &self.log,
            format!(
                "remove_stack:{}:{}:{}",
                public_tag(access.expose()),
                name.as_str(),
                key.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .admissions
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn get_operation<'a>(
        &'a self,
        access: &'a AccessCredential,
        operation_id: Uuid,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<OperationSnapshot, NativeClientError>> {
        record(
            &self.log,
            format!(
                "get_operation:{}:{operation_id}",
                public_tag(access.expose())
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .operations
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn list_operation_events<'a>(
        &'a self,
        access: &'a AccessCredential,
        operation_id: Uuid,
        after: u32,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<Vec<OperationEvent>, NativeClientError>> {
        record(
            &self.log,
            format!(
                "operation_events:{}:{operation_id}:{after}",
                public_tag(access.expose())
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .events
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn cancel_operation<'a>(
        &'a self,
        access: &'a AccessCredential,
        operation_id: Uuid,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<OperationSnapshot, NativeClientError>> {
        record(
            &self.log,
            format!(
                "cancel_operation:{}:{operation_id}",
                public_tag(access.expose())
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .operations
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn admit<'a>(
        &'a self,
        access: &'a AccessCredential,
        path: &'a str,
        key: &'a IdempotencyKey,
        body: &'a serde_json::Value,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        record(
            &self.log,
            format!(
                "admit:{}:{path}:{}:{body}",
                public_tag(access.expose()),
                key.as_str()
            ),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .admissions
            .pop_front()
            .unwrap_or(Err(NativeClientError::InvalidResponse));
        Box::pin(async move { outcome })
    }

    fn bootstrap_status<'a>(
        &'a self,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<bool, LoginFailure>> {
        record(&self.log, "bootstrap");
        let outcome = self
            .script
            .lock()
            .unwrap()
            .bootstrap
            .pop_front()
            .unwrap_or(Ok(true));
        Box::pin(async move { outcome })
    }

    fn password_login<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<RedeemedSession, LoginFailure>> {
        // The password never reaches the log; only its presence is recorded.
        record(
            &self.log,
            format!("password_login:{email}:{}", !password.is_empty()),
        );
        let outcome = self
            .script
            .lock()
            .unwrap()
            .login
            .pop_front()
            .unwrap_or(Err(LoginFailure::from(SessionError::InvalidResponse)));
        Box::pin(async move { outcome })
    }

    fn register_owner<'a>(
        &'a self,
        display_name: &'a str,
        email: &'a str,
        _password: &'a str,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<RedeemedSession, LoginFailure>> {
        record(&self.log, format!("register_owner:{display_name}:{email}"));
        let outcome = self
            .script
            .lock()
            .unwrap()
            .register
            .pop_front()
            .unwrap_or(Err(LoginFailure::from(SessionError::InvalidResponse)));
        Box::pin(async move { outcome })
    }

    fn probe<'a>(
        &'a self,
        _cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<(), ProbeFailure>> {
        record(&self.log, format!("probe:{}", origin_label(&self.origin)));
        let outcome = self
            .script
            .lock()
            .unwrap()
            .probe
            .pop_front()
            .unwrap_or(Ok(()));
        Box::pin(async move { outcome })
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
    pub(crate) log: OpLog,
}

impl Harness {
    pub(crate) fn new() -> Self {
        let log = new_log();
        let vault = Arc::new(FakeVault::new(log.clone()));
        let metadata = Arc::new(FakeMetadata::new(log.clone()));
        let controllers = Arc::new(FakeControllerFactory::new(log.clone()));
        let manager = SessionManager::new(vault.clone(), metadata.clone(), controllers.clone());
        Self {
            manager,
            vault,
            metadata,
            controllers,
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
        )
    }

    /// Complete one login against the default origin and pair.
    pub(crate) async fn login_ok(&self) -> SessionView {
        self.controllers.script().login.push_back(Ok(redeemed(1)));
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
                email: EMAIL.to_owned(),
                display_name: DISPLAY_NAME.to_owned(),
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
