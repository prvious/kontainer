//! Startup invalidation of legacy plaintext credential stores.
//!
//! Pre-custody desktop builds wrote complete bearer tokens into
//! `profiles.json` and used flat keys in `session.json`. Those values are
//! discarded, never imported into the vault. Both writes must succeed before
//! the app becomes usable, so a failed invalidation keeps the plaintext from
//! being read by a later run that only partially completes.

use tauri_plugin_store::StoreExt;

use super::metadata::{MetadataError, SessionDocumentStore, SessionMetadataStore};
use super::models::SessionDocument;

/// Legacy profiles file that mixed metadata with bearer tokens.
const LEGACY_PROFILES_STORE: &str = "profiles.json";

/// Invalidate legacy plaintext and establish the metadata document.
pub(crate) fn sanitize(
    app: &tauri::AppHandle,
    documents: &SessionDocumentStore,
) -> Result<(), MetadataError> {
    // Preserve a versioned document, replace missing or corrupt metadata, and
    // abort if the store itself is unavailable.
    let document = sanitized_document(documents.read_document())?;
    documents.clear_foreign_keys();
    documents.write_document(&document)?;

    // Clearing is only complete once the profiles file is saved empty; its
    // values must never be loadable by a later run.
    let profiles = app
        .store(LEGACY_PROFILES_STORE)
        .map_err(|_| MetadataError::Unavailable)?;
    profiles.clear();
    profiles.save().map_err(|_| MetadataError::Unavailable)?;
    Ok(())
}

fn sanitized_document(
    stored: Result<Option<SessionDocument>, MetadataError>,
) -> Result<SessionDocument, MetadataError> {
    match stored {
        Ok(Some(document)) => Ok(document),
        Ok(None) | Err(MetadataError::Corrupt) => Ok(SessionDocument::new(None)),
        Err(MetadataError::Unavailable) => Err(MetadataError::Unavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_metadata_aborts_sanitization() {
        assert_eq!(
            sanitized_document(Err(MetadataError::Unavailable)),
            Err(MetadataError::Unavailable)
        );
    }
}
