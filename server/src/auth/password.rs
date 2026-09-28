//! Argon2id hashing off the async runtime, plus the timing equalizer.

use std::sync::LazyLock;

/// Verified against when the email is unknown, so an unknown email costs the
/// same Argon2id work as a wrong password.
static EQUALIZER_HASH: LazyLock<Option<String>> =
    LazyLock::new(|| loco_rs::hash::hash_password("kontainer-sign-in-timing-equalizer").ok());

#[derive(Debug, thiserror::Error)]
#[error("password hashing failed")]
pub struct HashError;

/// Argon2id (Loco's default parameters) on a blocking thread.
pub async fn hash(password: String) -> Result<String, HashError> {
    tokio::task::spawn_blocking(move || loco_rs::hash::hash_password(&password))
        .await
        .map_err(|_| HashError)?
        .map_err(|_| HashError)
}

/// Verify on a blocking thread. `None` runs the equalizer and always fails.
pub async fn verify(password: String, stored_hash: Option<String>) -> bool {
    tokio::task::spawn_blocking(move || match stored_hash {
        Some(stored) => loco_rs::hash::verify_password(&password, &stored),
        None => {
            if let Some(equalizer) = EQUALIZER_HASH.as_deref() {
                let _ = loco_rs::hash::verify_password(&password, equalizer);
            }
            false
        }
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn hashes_are_argon2id_and_verify_only_the_original() {
        let stored = super::hash("correct horse battery".to_owned())
            .await
            .unwrap();
        assert!(stored.starts_with("$argon2id$"));
        assert!(super::verify("correct horse battery".to_owned(), Some(stored.clone())).await);
        assert!(!super::verify("wrong horse battery".to_owned(), Some(stored)).await);
        assert!(!super::verify("correct horse battery".to_owned(), None).await);
    }
}
