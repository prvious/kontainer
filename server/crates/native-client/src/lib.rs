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
//! This crate has no Tauri, WebView, controller, Docker, or database
//! dependency. The log and exec streams are the one exception to "spawns no work": its
//! WebSocket is the transport crate's, framed with the stream protocol's
//! protobuf messages, and [`LogFollower`] is driven by the caller's task
//! (ADR 0006, ADR 0025).

mod auth;
mod client;
mod containers;
mod credentials;
mod error;
mod exec;
mod images;
mod logs;
mod networks;
mod nodes;
mod operations;
mod probe;
mod services;
mod settings;
mod stacks;
mod status;
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
    ContainerTarget, NodeId, NodeOutcome, NodeOutcomeStatus,
};
pub use credentials::{AccessCredential, CredentialError, LoginVerifier, RefreshCredential};
pub use error::{AccountFieldErrors, FieldProblem, NativeClientError, Traced};
pub use exec::{
    ExecClosedReason, ExecConnection, ExecEvent, ExecEventFuture, ExecEventSource, ExecInputFrame,
    ExecInputSink, ExecOpenRequest, ExecSendFuture,
};
pub use images::{ImageItem, ImageList, ImageNodes};
pub use logs::{
    LogConnectError, LogConnectFuture, LogConnector, LogDisconnect, LogEndKind, LogEvent,
    LogFollower, LogFrame, LogFrameFuture, LogLineItem, LogOpenRequest, LogReceiver, LogStream,
};
pub use networks::{
    AttachedContainer, ManagerStatus, NetworkId, NetworkItem, NetworkList, NetworkPlacement,
    NetworkScope, NetworkSpec,
};
pub use nodes::{
    AgentStatus, ClusterSummary, NodeAvailability, NodeHealth, NodeList, NodeRole, NodeSample,
    NodeSummary, QuorumState, SamplePoint, SampleSeries,
};
pub use operations::{AdmittedOperation, IdempotencyKey, OperationSnapshot, OperationState};
pub use probe::ProbeFailure;
pub use services::{
    ServiceAction, ServiceDetail, ServiceId, ServiceItem, ServiceList, ServiceMode,
};
pub use settings::ControllerSettings;
pub use stacks::{StackItem, StackList, StackName, StackRemovalPlan, StackStatus};
pub use status::ControllerStatus;
pub use volumes::{VolumeItem, VolumeList, VolumeName, VolumeQuery, VolumeSpec};
