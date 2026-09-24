//! Startup invalidation of legacy plaintext credential stores.
//!
//! Pre-custody desktop builds wrote complete bearer tokens into
//! `profiles.json` and used flat keys in `session.json`. Those values are
//! discarded, never imported into the vault, and at most an active label and a
//! validated canonical origin survive as a login hint. Both writes must
//! succeed before the app becomes usable, so a failed invalidation keeps the
//! plaintext from being read by a later run that only partially completes.

use serde::Deserialize;
use serde_json::Value;
use tauri_plugin_store::StoreExt;

use kontainer_native_client::{NativeClient, NativeClientConfig};

use super::metadata::{MetadataError, SessionDocumentStore, SessionMetadataStore};
use super::models::{sanitize_label, LoginHint, SessionDocument};

/// Legacy profiles file that mixed metadata with bearer tokens.
const LEGACY_PROFILES_STORE: &str = "profiles.json";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyProfile {
    label: Option<String>,
    agent_url: Option<String>,
}

/// Invalidate legacy plaintext and establish the metadata document.
pub(crate) fn sanitize(
    app: &tauri::AppHandle,
    documents: &SessionDocumentStore,
) -> Result<(), MetadataError> {
    let hint = legacy_hint(app);

    // Preserve a versioned document, replace missing or corrupt metadata, and
    // abort if the store itself is unavailable.
    let document = sanitized_document(documents.read_document(), hint)?;
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
    hint: Option<LoginHint>,
) -> Result<SessionDocument, MetadataError> {
    match stored {
        Ok(Some(document)) => Ok(document),
        Ok(None) | Err(MetadataError::Corrupt) => Ok(SessionDocument::new(hint, None)),
        Err(MetadataError::Unavailable) => Err(MetadataError::Unavailable),
    }
}

/// Best-effort read of the previously active label and origin. Nothing here is
/// trusted: the origin is re-validated and stored only in canonical form, and
/// the label is bounded display text.
fn legacy_hint(app: &tauri::AppHandle) -> Option<LoginHint> {
    let profiles = app.store(LEGACY_PROFILES_STORE).ok()?;
    let stored = profiles.get("profiles")?;
    let active_id = profiles
        .get("activeProfileId")
        .and_then(|value| value.as_str().map(str::to_owned));
    hint_from_legacy_profiles(&stored, active_id.as_deref())
}

/// The pure part of [`legacy_hint`]: choose the active legacy profile, keep
/// only a canonical origin and a bounded label, and never carry a credential.
fn hint_from_legacy_profiles(profiles: &Value, active_id: Option<&str>) -> Option<LoginHint> {
    let entries = profiles.as_array()?;
    let active = active_id
        .and_then(|id| {
            entries.iter().find(|entry| {
                entry
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|entry_id| entry_id == id)
            })
        })
        .or_else(|| entries.first())?;
    let profile: LegacyProfile = serde_json::from_value(active.clone()).ok()?;

    Some(LoginHint {
        origin: profile.agent_url.as_deref().and_then(canonical_origin)?,
        label: sanitize_label(profile.label),
    })
}

/// Validate a legacy controller URL and return its canonical origin.
fn canonical_origin(raw: &str) -> Option<String> {
    NativeClient::new(NativeClientConfig::new(raw.trim()))
        .ok()
        .map(|client| client.controller_origin().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_json(entries: serde_json::Value, active: Option<&str>) -> Option<LoginHint> {
        hint_from_legacy_profiles(&entries, active)
    }

    #[test]
    fn a_legacy_profile_yields_only_a_canonical_origin_and_bounded_label() {
        let entries = serde_json::json!([
            {
                "id": "profile-1",
                "label": "  Production  ",
                "agentUrl": "https://controller.example",
                "token": "header.payload.signature",
                "userName": "owner"
            },
            {
                "id": "profile-2",
                "label": "Other",
                "agentUrl": "http://10.0.0.9:5150"
            }
        ]);
        let hint = profile_json(entries.clone(), Some("profile-1")).expect("hint");
        assert_eq!(hint.origin, "https://controller.example/");
        assert_eq!(hint.label.as_deref(), Some("Production"));
        let serialized = serde_json::to_string(&hint).expect("serializes");
        assert!(!serialized.contains("token"));
        assert!(!serialized.contains("signature"));
        assert!(!serialized.contains("owner"));

        // Without an active pointer the first entry is the hint, and the
        // label still never exceeds the display bound.
        let hint = profile_json(entries, None).expect("hint");
        assert_eq!(hint.label.as_deref(), Some("Production"));
    }

    #[test]
    fn a_non_canonical_or_missing_origin_yields_no_hint() {
        let cases = [
            serde_json::json!([{ "id": "p", "agentUrl": "http://10.0.0.9:5150" }]),
            serde_json::json!([{ "id": "p", "agentUrl": "not a url" }]),
            serde_json::json!([{ "id": "p", "agentUrl": "https://controller.example/api" }]),
            serde_json::json!([{ "id": "p", "label": "No origin" }]),
            serde_json::json!([]),
            serde_json::Value::Null,
        ];
        for entries in cases {
            assert!(
                profile_json(entries.clone(), None).is_none(),
                "{entries} must not yield a hint"
            );
        }
    }

    #[test]
    fn a_whitespace_label_is_dropped() {
        let entries = serde_json::json!([
            { "id": "p", "label": "   ", "agentUrl": "https://controller.example" }
        ]);
        let hint = profile_json(entries, Some("p")).expect("hint");
        assert!(hint.label.is_none());
    }

    #[test]
    fn unavailable_metadata_aborts_sanitization() {
        assert_eq!(
            sanitized_document(Err(MetadataError::Unavailable), None),
            Err(MetadataError::Unavailable)
        );
    }
}
