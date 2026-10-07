//! Node-local images in Kontainer's own terms, and the typed progress of a pull.

use std::pin::Pin;

use futures_util::Stream;

/// One image on this node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageSummary {
    /// `sha256:<64 hex>`.
    pub image_id: String,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    pub size_bytes: u64,
    pub created_at_seconds: i64,
    /// Containers on this node (any state) created from this image.
    pub used_by: u32,
}

/// One progress message from the daemon, e.g. `Downloading` for a layer with bytes so far.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PullProgress {
    pub layer_id: Option<String>,
    pub status: String,
    pub current: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullEvent {
    Progress(PullProgress),
    /// The registry's or daemon's own message, bounded and stripped of control characters. It is
    /// passed through on purpose: "manifest unknown" or "unauthorized" is what the user needs.
    Failed(String),
}

pub type PullStream = Pin<Box<dyn Stream<Item = PullEvent> + Send>>;

const MAX_MESSAGE_LEN: usize = 512;
const MAX_STATUS_LEN: usize = 128;
const MAX_REFERENCE_LEN: usize = 512;

/// A daemon message fit to show: control characters become spaces, at most 512 bytes.
#[must_use]
pub fn bounded_message(value: &str) -> String {
    bounded(value, MAX_MESSAGE_LEN)
}

pub(crate) fn bounded_status(value: &str) -> String {
    bounded(value, MAX_STATUS_LEN)
}

fn bounded(value: &str, limit: usize) -> String {
    let cleaned: String = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    if cleaned.len() <= limit {
        return cleaned;
    }
    let mut end = limit;
    while !cleaned.is_char_boundary(end) {
        end -= 1;
    }
    cleaned[..end].to_owned()
}

/// `sha256:<64 lowercase hex>`, the only image id shape that crosses the boundary.
#[must_use]
pub fn is_valid_image_id(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

/// Coarse shape check for a pull reference; the controller validates the full grammar.
#[must_use]
pub fn is_plausible_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REFERENCE_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'"' && byte != b'\\')
}

#[cfg(test)]
mod tests {
    use super::{bounded_message, is_plausible_reference, is_valid_image_id};

    #[test]
    fn image_ids_and_references_are_bounded() {
        assert!(is_valid_image_id(&format!("sha256:{}", "a".repeat(64))));
        for bad in [
            "a".repeat(64),
            format!("sha256:{}", "A".repeat(64)),
            "sha256:abc".to_owned(),
        ] {
            assert!(!is_valid_image_id(&bad), "{bad}");
        }
        assert!(is_plausible_reference("ghcr.io/acme/billing:2.4.0"));
        assert!(!is_plausible_reference(""));
        assert!(!is_plausible_reference("has space"));
        assert!(!is_plausible_reference(&"a".repeat(513)));
    }

    #[test]
    fn messages_are_bounded_and_single_line() {
        assert_eq!(
            bounded_message(" manifest unknown\\n "),
            "manifest unknown\\n"
        );
        assert_eq!(bounded_message("a\nb"), "a b");
        assert_eq!(bounded_message(&"x".repeat(600)).len(), 512);
    }
}
