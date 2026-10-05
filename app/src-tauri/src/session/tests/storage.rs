//! Record codecs, strict schemas, and redaction hygiene.

use serde_json::json;

use crate::session::models::{
    decode_anchor, decode_profile, encode_anchor, encode_profile, sanitize_label, AnchorRecord,
    AnchorState, ProfileRecord, SessionDocument, ANCHOR_VERSION, MAX_LABEL_CHARS, MAX_RECORD_BYTES,
    METADATA_VERSION, PROFILE_VERSION,
};

use super::fakes::{fake_refresh, ORIGIN, PRINCIPAL_ID, PROFILE_ID};

fn sample_anchor() -> AnchorRecord {
    AnchorRecord {
        version: ANCHOR_VERSION,
        profile_id: PROFILE_ID,
        origin: ORIGIN.to_owned(),
        cluster_id: "test".to_owned(),
        principal_id: PRINCIPAL_ID,
        generation: 7,
        state: AnchorState::CommitPending,
    }
}

fn sample_record() -> ProfileRecord {
    ProfileRecord {
        version: PROFILE_VERSION,
        profile_id: PROFILE_ID,
        origin: ORIGIN.to_owned(),
        cluster_id: "test".to_owned(),
        principal_id: PRINCIPAL_ID,
        generation: 7,
        refresh_expires_at_unix_ms: 1_760_000_000_000,
        refresh_credential: fake_refresh(1).expose().to_owned(),
    }
}

#[test]
fn anchor_records_round_trip_and_reject_drift() {
    let anchor = sample_anchor();
    let bytes = encode_anchor(&anchor).expect("encodes");
    assert_eq!(decode_anchor(&bytes).expect("decodes"), anchor);

    let mut value = serde_json::to_value(&anchor).expect("serializes");
    value["extra"] = json!(true);
    let drifted = serde_json::to_vec(&value).expect("serializes");
    assert!(
        decode_anchor(&drifted).is_err(),
        "unknown fields are rejected"
    );

    let mut value = serde_json::to_value(&anchor).expect("serializes");
    value["generation"] = json!("seven");
    let wrong_type = serde_json::to_vec(&value).expect("serializes");
    assert!(decode_anchor(&wrong_type).is_err());
}

#[test]
fn profile_records_round_trip_and_reject_drift() {
    let record = sample_record();
    let bytes = encode_profile(&record).expect("encodes");
    let decoded = decode_profile(&bytes).expect("decodes");
    assert_eq!(decoded.profile_id, record.profile_id);
    assert_eq!(decoded.generation, record.generation);
    assert_eq!(decoded.refresh_credential, record.refresh_credential);

    let mut value = serde_json::to_value(&record).expect("serializes");
    value["credential"] = json!("extra");
    let drifted = serde_json::to_vec(&value).expect("serializes");
    assert!(decode_profile(&drifted).is_err());
}

#[test]
fn oversized_records_are_rejected_before_parsing() {
    let anchor = sample_anchor();
    let mut value = serde_json::to_value(&anchor).expect("serializes");
    value["clusterId"] = json!("x".repeat(MAX_RECORD_BYTES));
    let bytes = serde_json::to_vec(&value).expect("serializes");
    assert!(bytes.len() > MAX_RECORD_BYTES);
    assert!(decode_anchor(&bytes).is_err());
    assert!(encode_anchor(&AnchorRecord {
        cluster_id: "x".repeat(MAX_RECORD_BYTES),
        ..anchor
    })
    .is_err());
}

#[test]
fn records_from_another_schema_version_are_rejected() {
    let anchor = sample_anchor();
    let mut value = serde_json::to_value(&anchor).expect("serializes");
    value["version"] = json!(ANCHOR_VERSION + 1);
    let drifted = serde_json::to_vec(&value).expect("serializes");
    assert!(decode_anchor(&drifted).is_err());

    let record = sample_record();
    let mut value = serde_json::to_value(&record).expect("serializes");
    value["version"] = json!(PROFILE_VERSION + 1);
    let drifted = serde_json::to_vec(&value).expect("serializes");
    assert!(decode_profile(&drifted).is_err());
}

#[test]
fn profile_record_debug_redacts_the_credential() {
    let record = sample_record();
    let rendered = format!("{record:?}");
    assert!(!rendered.contains("ktr_"), "{rendered}");
    assert!(rendered.contains("<redacted>"), "{rendered}");
}

#[test]
fn metadata_document_uses_a_strict_versioned_schema() {
    let mut document = SessionDocument::new(None);
    document.touch_connection(ORIGIN, Some("Default".to_owned()), 1);
    let value = serde_json::to_value(&document).expect("serializes");
    assert_eq!(value["version"], json!(METADATA_VERSION));
    assert_eq!(
        serde_json::from_value::<SessionDocument>(value.clone()).expect("round trips"),
        document
    );

    let mut unknown = value.clone();
    unknown["credentials"] = json!("never");
    assert!(serde_json::from_value::<SessionDocument>(unknown).is_err());

    let mut missing = value;
    missing.as_object_mut().expect("object").remove("version");
    assert!(serde_json::from_value::<SessionDocument>(missing).is_err());
}

#[test]
fn a_document_with_the_removed_hint_still_decodes_and_is_rewritten_without_it() {
    for hint in [
        json!({ "origin": ORIGIN, "label": "Default" }),
        serde_json::Value::Null,
    ] {
        let stored = json!({
            "version": METADATA_VERSION,
            "hint": hint,
            "session": null,
            "connections": [{
                "id": "018f2e34-0000-7000-8000-0000000000c1",
                "origin": ORIGIN,
                "label": "Default",
                "lastUsedAtUnixMs": 1
            }]
        });
        let document: SessionDocument =
            serde_json::from_value(stored).expect("an older document is not corrupt");
        assert_eq!(document.connections.len(), 1);
        assert_eq!(document.connections[0].origin, ORIGIN);

        let rewritten = serde_json::to_value(&document).expect("serializes");
        assert!(rewritten.get("hint").is_none(), "{rewritten}");
        assert_eq!(rewritten["connections"][0]["origin"], json!(ORIGIN));
        assert_eq!(
            serde_json::from_value::<SessionDocument>(rewritten).expect("round trips"),
            SessionDocument::new(None).with_connections(document.connections)
        );
    }
}

#[test]
fn label_sanitization_trims_bounds_and_drops_empty_values() {
    assert_eq!(
        sanitize_label(Some("  Default  ".to_owned())).as_deref(),
        Some("Default")
    );
    assert_eq!(sanitize_label(Some("   ".to_owned())), None);
    assert_eq!(sanitize_label(None), None);
    let bounded = sanitize_label(Some("x".repeat(MAX_LABEL_CHARS * 3))).expect("label");
    assert_eq!(bounded.chars().count(), MAX_LABEL_CHARS);
}

#[test]
fn anchor_states_serialize_as_snake_case_names() {
    for (state, name) in [
        (AnchorState::Ready, "ready"),
        (AnchorState::CommitPending, "commit_pending"),
        (AnchorState::LogoutPending, "logout_pending"),
        (AnchorState::CleanupPending, "cleanup_pending"),
        (AnchorState::LoggedOut, "logged_out"),
    ] {
        assert_eq!(
            serde_json::to_value(state).expect("serializes"),
            json!(name)
        );
    }
}
