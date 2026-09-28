//! Rust-owned desktop session custody.
//!
//! The desktop keeps one logical profile whose access credential lives only in
//! Rust memory and whose refresh credential lives only in the OS credential
//! vault. This module owns:
//!
//! - the trusted vault anchor that binds one local profile UUID to its
//!   canonical controller origin and authenticated `(cluster_id,
//!   principal_id)` pair;
//! - the UUID-keyed refresh record that repeats that binding;
//! - the non-secret `session.json` metadata document, which is never trusted
//!   without a matching anchor;
//! - startup invalidation of legacy plaintext credential stores;
//! - password sign-in and owner registration, restart restore with ordered
//!   rotation, commit retry, and logout;
//! - the saved-connections list and readiness probes for the connect card.
//!
//! No custody command returns a credential. Sign-in and registration take the
//! password only as a request argument (zeroed after use), return
//! [`models::SessionView`], and fail with [`errors::LoginFailure`]; the other
//! commands fail with closed [`errors::SessionError`] codes.

pub(crate) mod controller;
pub(crate) mod errors;
pub(crate) mod legacy;
pub(crate) mod manager;
pub(crate) mod metadata;
pub(crate) mod models;
pub(crate) mod vault;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::session::controller::NativeControllerFactory;
use crate::session::manager::SessionManager;
use crate::session::metadata::{MetadataError, SessionDocumentStore};
use crate::session::vault::KeyringVault;

/// Build the managed session custody, failing the app startup closed when the
/// non-secret metadata store or legacy invalidation cannot be completed.
pub(crate) fn initialize(app: &tauri::AppHandle) -> Result<SessionManager, String> {
    let metadata = SessionDocumentStore::open(app).map_err(describe_metadata_error)?;
    legacy::sanitize(app, &metadata).map_err(describe_metadata_error)?;
    Ok(SessionManager::new(
        Arc::new(KeyringVault::spawn()),
        Arc::new(metadata),
        Arc::new(NativeControllerFactory),
    ))
}

fn describe_metadata_error(error: MetadataError) -> String {
    match error {
        MetadataError::Unavailable => "session metadata store is unavailable".to_string(),
        MetadataError::Corrupt => "session metadata store is corrupt".to_string(),
    }
}
