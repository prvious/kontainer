//! Non-secret session metadata over `session.json`.
//!
//! The document is strict (`deny_unknown_fields`, exact schema version) but it
//! is never authority for anything security-relevant: the trusted vault anchor
//! must agree with it before any stored credential is read.

use std::sync::Arc;

use tauri_plugin_store::StoreExt;

use super::models::{SessionDocument, METADATA_VERSION};

/// Store key holding the single metadata document.
pub(crate) const DOCUMENT_KEY: &str = "document";
/// File name of the metadata store.
pub(crate) const DOCUMENT_STORE: &str = "session.json";

/// Closed metadata failure taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataError {
    /// The store could not be read or saved.
    Unavailable,
    /// The stored value did not match the strict document schema, including
    /// its schema version.
    Corrupt,
}

/// Read and write the one metadata document.
pub(crate) trait SessionMetadataStore: Send + Sync + 'static {
    fn read_document(&self) -> Result<Option<SessionDocument>, MetadataError>;
    fn write_document(&self, document: &SessionDocument) -> Result<(), MetadataError>;
}

/// Production store backed by `tauri-plugin-store`.
pub(crate) struct SessionDocumentStore {
    store: Arc<tauri_plugin_store::Store<tauri::Wry>>,
}

impl SessionDocumentStore {
    /// Open the `session.json` store, creating the file handle when absent.
    pub(crate) fn open(app: &tauri::AppHandle) -> Result<Self, MetadataError> {
        let store = app
            .store(DOCUMENT_STORE)
            .map_err(|_| MetadataError::Unavailable)?;
        Ok(Self { store })
    }

    /// Drop every key other than the versioned document, so legacy flat
    /// records cannot be read back by any code path.
    pub(crate) fn clear_foreign_keys(&self) {
        for key in self.store.keys() {
            if key != DOCUMENT_KEY {
                self.store.delete(key);
            }
        }
    }
}

impl SessionMetadataStore for SessionDocumentStore {
    fn read_document(&self) -> Result<Option<SessionDocument>, MetadataError> {
        let Some(value) = self.store.get(DOCUMENT_KEY) else {
            return Ok(None);
        };
        let document: SessionDocument =
            serde_json::from_value(value).map_err(|_| MetadataError::Corrupt)?;
        if document.version != METADATA_VERSION {
            return Err(MetadataError::Corrupt);
        }
        Ok(Some(document))
    }

    fn write_document(&self, document: &SessionDocument) -> Result<(), MetadataError> {
        let value = serde_json::to_value(document).map_err(|_| MetadataError::Unavailable)?;
        self.store.set(DOCUMENT_KEY, value);
        self.store.save().map_err(|_| MetadataError::Unavailable)?;
        Ok(())
    }
}
