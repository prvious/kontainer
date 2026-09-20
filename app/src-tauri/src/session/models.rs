//! Persisted session-custody models, the metadata document, and IPC views.
//!
//! Two vault records exist. The fixed anchor is addressed by a constant key
//! and supplies the trusted profile UUID plus its binding. The profile record
//! is addressed only by that UUID and repeats the binding so a mismatched or
//! partially overwritten record cannot be mistaken for the anchor's session.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

/// Schema version of the trusted anchor record.
pub(crate) const ANCHOR_VERSION: u32 = 1;
/// Schema version of the UUID-keyed profile credential record.
pub(crate) const PROFILE_VERSION: u32 = 1;
/// Schema version of the non-secret metadata document.
pub(crate) const METADATA_VERSION: u32 = 1;
/// Hard ceiling for any encoded vault record; larger blobs are corrupt.
pub(crate) const MAX_RECORD_BYTES: usize = 8 * 1024;
/// Longest display label the desktop persists or reports.
pub(crate) const MAX_LABEL_CHARS: usize = 64;

/// Lifecycle of the trusted anchor.
///
/// `CommitPending`, `LogoutPending`, and `CleanupPending` are all
/// non-restorable: a process that stopped inside one of them must not present
/// an authenticated session, because its credential may already have been
/// consumed server-side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnchorState {
    /// The profile record matches the anchor generation.
    Ready,
    /// A login or rotation is between "credentials issued" and "committed".
    CommitPending,
    /// A logout is between "revocation attempted" and "local deletion done".
    LogoutPending,
    /// Revocation is confirmed but the local credential still exists.
    CleanupPending,
    /// No credential is held for this profile.
    LoggedOut,
}

/// The fixed, integrity-bearing vault anchor for the one local profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AnchorRecord {
    pub version: u32,
    pub profile_id: Uuid,
    pub origin: String,
    pub cluster_id: String,
    pub principal_id: Uuid,
    pub generation: u64,
    pub state: AnchorState,
}

/// The UUID-keyed refresh record, integrity-bound to the anchor.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProfileRecord {
    pub version: u32,
    pub profile_id: Uuid,
    pub origin: String,
    pub cluster_id: String,
    pub principal_id: Uuid,
    pub generation: u64,
    pub refresh_expires_at_unix_ms: i64,
    pub refresh_credential: String,
}

impl std::fmt::Debug for ProfileRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProfileRecord")
            .field("version", &self.version)
            .field("profile_id", &self.profile_id)
            .field("origin", &self.origin)
            .field("cluster_id", &self.cluster_id)
            .field("principal_id", &self.principal_id)
            .field("generation", &self.generation)
            .field(
                "refresh_expires_at_unix_ms",
                &self.refresh_expires_at_unix_ms,
            )
            .field("refresh_credential", &"<redacted>")
            .finish()
    }
}

impl Drop for ProfileRecord {
    fn drop(&mut self) {
        // The encoded form is zeroed by the caller's `Zeroizing` buffer; this
        // clears the owned plaintext copy as well.
        self.refresh_credential.zeroize();
    }
}

/// The payload did not match the strict record schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordDecodeError;

/// Encode the anchor record for the vault.
pub(crate) fn encode_anchor(record: &AnchorRecord) -> Result<Vec<u8>, RecordDecodeError> {
    let bytes = serde_json::to_vec(record).map_err(|_| RecordDecodeError)?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(RecordDecodeError);
    }
    Ok(bytes)
}

/// Encode the profile record; the returned buffer is zeroed on drop.
pub(crate) fn encode_profile(
    record: &ProfileRecord,
) -> Result<Zeroizing<Vec<u8>>, RecordDecodeError> {
    let bytes = Zeroizing::new(serde_json::to_vec(record).map_err(|_| RecordDecodeError)?);
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(RecordDecodeError);
    }
    Ok(bytes)
}

/// Decode and strictly validate an anchor record.
pub(crate) fn decode_anchor(bytes: &[u8]) -> Result<AnchorRecord, RecordDecodeError> {
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(RecordDecodeError);
    }
    let record: AnchorRecord = serde_json::from_slice(bytes).map_err(|_| RecordDecodeError)?;
    if record.version != ANCHOR_VERSION {
        // A record written by another schema version is not a usable session.
        return Err(RecordDecodeError);
    }
    Ok(record)
}

/// Decode and strictly validate a profile record.
pub(crate) fn decode_profile(bytes: &[u8]) -> Result<ProfileRecord, RecordDecodeError> {
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(RecordDecodeError);
    }
    let record: ProfileRecord = serde_json::from_slice(bytes).map_err(|_| RecordDecodeError)?;
    if record.version != PROFILE_VERSION {
        return Err(RecordDecodeError);
    }
    Ok(record)
}

/// Non-secret metadata persisted in `session.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SessionDocument {
    pub version: u32,
    /// Unauthenticated login hint (previously used origin and label).
    pub hint: Option<LoginHint>,
    /// The session summary, valid only while it matches the vault anchor.
    pub session: Option<MetadataSession>,
}

impl SessionDocument {
    /// A document that always carries the current schema version.
    pub(crate) fn new(hint: Option<LoginHint>, session: Option<MetadataSession>) -> Self {
        Self {
            version: METADATA_VERSION,
            hint,
            session,
        }
    }
}

/// A non-secret origin/label hint for the login screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LoginHint {
    pub origin: String,
    pub label: Option<String>,
}

/// The non-secret session summary; never authority on its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MetadataSession {
    pub profile_id: Uuid,
    pub origin: String,
    pub cluster_id: String,
    pub principal_id: Uuid,
    pub github_login: String,
    pub label: Option<String>,
}

impl MetadataSession {
    /// Whether every security-relevant field matches the trusted anchor.
    pub(crate) fn matches_anchor(&self, anchor: &AnchorRecord) -> bool {
        self.profile_id == anchor.profile_id
            && self.origin == anchor.origin
            && self.cluster_id == anchor.cluster_id
            && self.principal_id == anchor.principal_id
    }
}

impl ProfileRecord {
    /// Whether the record's identity binding matches the trusted anchor.
    pub(crate) fn matches_binding(&self, anchor: &AnchorRecord) -> bool {
        self.profile_id == anchor.profile_id
            && self.origin == anchor.origin
            && self.cluster_id == anchor.cluster_id
            && self.principal_id == anchor.principal_id
    }
}

/// The credential-free session view returned over IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionView {
    pub profile_id: Uuid,
    pub origin: String,
    pub cluster_id: String,
    pub principal_id: Uuid,
    pub github_login: String,
    pub label: Option<String>,
    pub access_expires_at_unix_ms: i64,
}

/// Input for starting a login attempt.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SessionLoginInput {
    pub origin: String,
    pub label: Option<String>,
}

/// Trim, bound, and drop empty display labels.
pub(crate) fn sanitize_label(raw: Option<String>) -> Option<String> {
    raw.map(|label| label.trim().to_owned())
        .filter(|label| !label.is_empty())
        .map(|label| label.chars().take(MAX_LABEL_CHARS).collect())
}
