//! Node-local volumes in Kontainer's own terms.

use std::collections::BTreeMap;

use crate::image::bounded_message;

/// Longest name the controller accepts; Docker has no limit of its own.
pub const MAX_VOLUME_NAME_LEN: usize = 255;
/// Labels one volume may carry across the boundary.
pub const MAX_VOLUME_LABELS: usize = 64;
/// Container names listed for an in-use volume.
pub const MAX_VOLUME_USERS: usize = 16;
const MAX_LABEL_LEN: usize = 256;

/// One volume on this node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VolumeSummary {
    pub name: String,
    pub driver: String,
    /// Host path, for `local` volumes under the daemon's data root.
    pub mountpoint: String,
    pub created_at_seconds: Option<i64>,
    pub labels: BTreeMap<String, String>,
    /// Containers on this node (any state) that mount it.
    pub used_by: u32,
    /// Their names, at most [`MAX_VOLUME_USERS`].
    pub users: Vec<String>,
    /// From the last `docker system df`; `None` until it ran or when the driver can't say.
    pub size_bytes: Option<u64>,
}

/// What `create_volume` makes. Only the `local` driver is offered for now.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VolumeCreate {
    pub name: String,
    pub driver: String,
    pub labels: BTreeMap<String, String>,
    pub driver_opts: BTreeMap<String, String>,
}

/// Docker's volume name rule, `[a-zA-Z0-9][a-zA-Z0-9_.-]+`, capped at 255 characters. Docker
/// refuses one-character names, so the controller does too.
#[must_use]
pub fn is_valid_volume_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    (2..=MAX_VOLUME_NAME_LEN).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

/// Label keys and values that may cross the boundary: at most 64 pairs, keys 1–256 printable
/// characters, values up to 256.
#[must_use]
pub fn are_valid_volume_labels(labels: &BTreeMap<String, String>) -> bool {
    labels.len() <= MAX_VOLUME_LABELS
        && labels.iter().all(|(key, value)| {
            !key.is_empty()
                && key.len() <= MAX_LABEL_LEN
                && value.len() <= MAX_LABEL_LEN
                && !key
                    .chars()
                    .any(|character| character.is_control() || character == '=')
                && !value.chars().any(char::is_control)
        })
}

/// A daemon-reported label set bounded for the boundary: oversized pairs are dropped.
pub(crate) fn bounded_labels(
    labels: impl IntoIterator<Item = (String, String)>,
) -> BTreeMap<String, String> {
    labels
        .into_iter()
        .filter(|(key, value)| {
            !key.is_empty() && key.len() <= MAX_LABEL_LEN && value.len() <= MAX_LABEL_LEN
        })
        .map(|(key, value)| (bounded_message(&key), bounded_message(&value)))
        .take(MAX_VOLUME_LABELS)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{are_valid_volume_labels, bounded_labels, is_valid_volume_name};

    #[test]
    fn volume_names_follow_dockers_rule() {
        for name in [
            "pg",
            "postgres-data",
            "a.b_c-d",
            "0volume",
            &"a".repeat(255),
        ] {
            assert!(is_valid_volume_name(name), "{name}");
        }
        for name in [
            "",
            "a",
            "-data",
            ".data",
            "_data",
            "has space",
            "sl/ash",
            "ü-data",
            &"a".repeat(256),
        ] {
            assert!(!is_valid_volume_name(name), "{name}");
        }
    }

    #[test]
    fn labels_are_bounded() {
        let ok = BTreeMap::from([
            ("app".to_owned(), "billing".to_owned()),
            ("empty".to_owned(), String::new()),
        ]);
        assert!(are_valid_volume_labels(&ok));
        for (key, value) in [("", "x"), ("a=b", "x"), ("k", "line\nbreak")] {
            assert!(
                !are_valid_volume_labels(&BTreeMap::from([(key.to_owned(), value.to_owned())])),
                "{key:?}"
            );
        }
        let many: BTreeMap<_, _> = (0..65)
            .map(|index| (format!("k{index}"), String::new()))
            .collect();
        assert!(!are_valid_volume_labels(&many));
        let bounded = bounded_labels([
            ("k".to_owned(), "v".repeat(300)),
            ("ok".to_owned(), "fine".to_owned()),
        ]);
        assert_eq!(
            bounded,
            BTreeMap::from([("ok".to_owned(), "fine".to_owned())])
        );
    }
}
