//! The closed IPC contract, checked against the fixture the TypeScript tests
//! also read. Rust owns the serialization; TypeScript owns the interface.

use uuid::Uuid;

use crate::session::errors::{FieldErrorsView, FieldProblem, LoginFailure, SessionError};
use crate::session::models::{
    ConnectionSummary, ProbeDiagnostic, ProbeStatus, ProbeView, SavedConnection, SessionView,
};

use super::fakes::Harness;

const FIXTURE: &str = include_str!("../../../../tests/fixtures/session-ipc.json");
/// The registered command surface; `generate_handler!` in `lib.rs` is checked
/// against this list so the fixture cannot describe commands that no longer
/// exist.
const LIB_SOURCE: &str = include_str!("../../lib.rs");

fn fixture() -> serde_json::Value {
    serde_json::from_str(FIXTURE).expect("fixture parses")
}

#[test]
fn session_view_matches_the_shared_fixture() {
    let view = SessionView {
        profile_id: Uuid::parse_str("018f2e34-0000-7000-8000-0000000000bb").expect("uuid"),
        origin: "https://controller.example/".to_owned(),
        cluster_id: "test".to_owned(),
        principal_id: Uuid::parse_str("018f2e34-0000-7000-8000-0000000000aa").expect("uuid"),
        email: "owner@example.com".to_owned(),
        display_name: "Ada Owner".to_owned(),
        label: Some("Default".to_owned()),
        access_expires_at_unix_ms: 1_760_000_000_000,
    };
    let serialized = serde_json::to_value(&view).expect("serializes");
    assert_eq!(serialized, fixture()["sessionView"]);

    // Optionality must stay optional on the wire.
    let without_label = SessionView {
        label: None,
        ..view
    };
    let serialized = serde_json::to_value(&without_label).expect("serializes");
    assert!(serialized["label"].is_null());
}

#[test]
fn connection_probe_and_failure_shapes_match_the_shared_fixture() {
    let connection = SavedConnection {
        id: Uuid::parse_str("018f2e34-0000-7000-8000-0000000000cc").expect("uuid"),
        origin: "https://controller.example/".to_owned(),
        label: Some("Default".to_owned()),
        last_used_at_unix_ms: 1_760_000_000_000,
        last_summary: Some(ConnectionSummary {
            nodes: 3,
            services: 12,
        }),
    };
    assert_eq!(
        serde_json::to_value(&connection).expect("serializes"),
        fixture()["savedConnection"]
    );
    let probe = ProbeView {
        status: ProbeStatus::Unreachable,
        diagnostic: Some(ProbeDiagnostic::Tcp),
    };
    assert_eq!(
        serde_json::to_value(probe).expect("serializes"),
        fixture()["probeView"]
    );
    let failure = LoginFailure {
        attempts_remaining: Some(7),
        retry_after_seconds: None,
        fields: Some(FieldErrorsView {
            password: Some(FieldProblem::TooShort),
            ..FieldErrorsView::default()
        }),
        request_id: Some("01J9ZK4F3Q".to_owned()),
        ..SessionError::InvalidCredentials.into()
    };
    assert_eq!(
        serde_json::to_value(&failure).expect("serializes"),
        fixture()["loginFailure"]
    );
    let setup_code_failure = LoginFailure {
        fields: Some(FieldErrorsView {
            setup_code: Some(FieldProblem::Invalid),
            ..FieldErrorsView::default()
        }),
        ..SessionError::ValidationFailed.into()
    };
    assert_eq!(
        serde_json::to_value(&setup_code_failure).expect("serializes"),
        fixture()["setupCodeFailure"]
    );
}

#[test]
fn error_codes_match_the_shared_fixture() {
    let serialized: Vec<String> = SessionError::ALL
        .iter()
        .map(|error| {
            serde_json::to_value(error)
                .expect("serializes")
                .as_str()
                .expect("code is a string")
                .to_owned()
        })
        .collect();
    let expected: Vec<String> = fixture()["errorCodes"]
        .as_array()
        .expect("array")
        .iter()
        .map(|value| value.as_str().expect("string").to_owned())
        .collect();
    assert_eq!(serialized, expected);
}

#[test]
fn the_fixture_lists_exactly_the_registered_commands() {
    let commands: Vec<String> = fixture()["commands"]
        .as_array()
        .expect("array")
        .iter()
        .map(|value| value.as_str().expect("string").to_owned())
        .collect();
    for command in &commands {
        assert!(
            LIB_SOURCE.contains(&format!("actions::session::{command}::{command}")),
            "{command} is missing from the invoke handler"
        );
    }
    // And every registered custody command is described by the fixture.
    for line in LIB_SOURCE.lines() {
        let Some(rest) = line.trim().strip_prefix("actions::session::") else {
            continue;
        };
        let name = rest
            .trim_end_matches(',')
            .rsplit("::")
            .next()
            .expect("command name");
        assert!(commands.iter().any(|command| command == name), "{name}");
    }
}

#[tokio::test]
async fn custody_outputs_never_carry_credentials() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    let output = format!(
        "{} {:?}",
        serde_json::to_string(&view).expect("view serializes"),
        SessionError::CleanupUncertain,
    );
    for canary in [
        "kta_",
        "ktr_",
        "access_credential",
        "refresh_credential",
        "Bearer",
    ] {
        assert!(!output.contains(canary), "{canary} leaked into IPC output");
    }
}
