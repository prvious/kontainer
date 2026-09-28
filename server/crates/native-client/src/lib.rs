#![forbid(unsafe_code)]

//! UI-neutral Rust client for the Kontainer controller public API.
//!
//! This crate implements the controller contracts: password sign-in and owner
//! registration, GitHub bootstrap login attempt/redemption, refresh and logout, node membership
//! and samples, controller settings, the cluster container list and container actions, the image list, removal, and pulls, volumes, networks, services, stacks, and
//! owner-scoped operation polling and admission. It is the native
//! networking boundary that Tauri (and any future native caller) reuses
//! instead of the WebView.
//!
//! Boundaries enforced here:
//!
//! - HTTPS is mandatory except for explicit loopback development origins.
//! - Redirects, ambient proxies, transparent compression, cookies, and
//!   implicit transport retries are disabled.
//! - Access, refresh, and PKCE credentials are per-call arguments held in
//!   redacted, non-serializable, non-cloneable wrappers. [`NativeClient`]
//!   stores no credential and sets no default authorization header.
//! - Every call takes one absolute deadline plus an explicit
//!   [`CancellationToken`](tokio_util::sync::CancellationToken) and spawns no
//!   work of its own; dropping a call drops the connection.
//! - Response bodies stream into bounded buffers, and only the container and
//!   operation GETs repeat once after a classified pre-response transient
//!   failure under the original deadline.
//!
//! This crate has no Tauri, WebView, controller, Docker, database, or
//! protobuf dependency.

mod auth;
mod client;
mod containers;
mod credentials;
mod error;
mod images;
mod networks;
mod nodes;
mod operations;
mod probe;
mod services;
mod settings;
mod stacks;
mod volumes;

#[cfg(test)]
mod tests;

pub use auth::{Account, AuthenticatedSession, LoginAttempt, PasswordSession, SessionCredentials};
pub use client::{
    DEFAULT_COLLECTION_BODY_LIMIT, DEFAULT_CONNECT_TIMEOUT, DEFAULT_JSON_BODY_LIMIT,
    DEFAULT_REQUEST_DEADLINE, NativeClient, NativeClientConfig, TrustPolicy,
};
pub use containers::{
    ContainerAction, ContainerActionOptions, ContainerItem, ContainerList, ContainerState,
    ContainerTarget, MAX_BULK_TARGETS, MAX_CONTAINERS, MAX_STOP_TIMEOUT_SECONDS, NodeId,
    NodeOutcome, NodeOutcomeStatus,
};
pub use credentials::{AccessCredential, CredentialError, LoginVerifier, RefreshCredential};
pub use error::{AccountFieldErrors, FieldProblem, NativeClientError, Traced};
pub use images::{
    ImageItem, ImageList, ImageNodes, MAX_IMAGE_REFERENCE_LEN, MAX_IMAGE_TARGET_NODES, MAX_IMAGES,
};
pub use networks::{
    AttachedContainer, MAX_NETWORKS, ManagerStatus, NetworkId, NetworkItem, NetworkList,
    NetworkPlacement, NetworkScope, NetworkSpec,
};
pub use nodes::{
    MAX_SAMPLE_POINTS, NodeList, NodeSample, NodeStatus, NodeSummary, SAMPLE_WINDOWS, SamplePoint,
    SampleSeries,
};
pub use operations::{
    AdmittedOperation, IdempotencyKey, MAX_EVENT_PAGE, OperationEvent, OperationSnapshot,
    OperationState,
};
pub use probe::ProbeFailure;
pub use services::{
    MAX_REPLICAS, MAX_SERVICES, ServiceAction, ServiceDetail, ServiceId, ServiceItem, ServiceList,
    ServiceMode, TaskItem,
};
pub use settings::{ControllerSettings, RETENTION_DAYS};
pub use stacks::{StackItem, StackList, StackName, StackRemovalPlan, StackStatus};
pub use volumes::{
    LOCAL_VOLUME_DRIVER, MAX_VOLUME_LABELS, MAX_VOLUMES, VolumeItem, VolumeList, VolumeName,
    VolumeQuery, VolumeSpec,
};
