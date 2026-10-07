//! Argon2id hashing off the async runtime, plus the timing equalizer.
//!
//! Every hash or verify needs an [`ArgonPermit`]. One run holds about 19 MiB and a CPU, so the
//! permits bound how much unauthenticated sign-in work can run at once; a request that can't get
//! one quickly is told the controller is busy instead of queueing without limit.

use std::{num::NonZero, sync::LazyLock, thread::available_parallelism, time::Duration};

use tokio::sync::{Semaphore, SemaphorePermit};

/// Verified against when the email is unknown, so an unknown email costs the
/// same Argon2id work as a wrong password.
static EQUALIZER_HASH: LazyLock<Option<String>> =
    LazyLock::new(|| loco_rs::hash::hash_password("kontainer-sign-in-timing-equalizer").ok());

/// One Argon2id run per CPU.
static PERMITS: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(available_parallelism().map_or(4, NonZero::get)));

/// How long a request waits for a permit. A run takes tens of milliseconds, so a wait this long
/// means the controller is saturated rather than briefly contended.
const PERMIT_WAIT: Duration = Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
#[error("password hashing failed")]
pub struct HashError;

/// Every Argon2id permit is in use; the caller answers 429 `busy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("too many password checks are running")]
pub struct Busy;

/// The right to run one Argon2id hash or verification. It moves into the blocking task, so it is
/// released when the work finishes, not when a cancelled request stops waiting for it.
#[derive(Debug)]
pub struct ArgonPermit {
    _permit: SemaphorePermit<'static>,
}

pub async fn permit() -> Result<ArgonPermit, Busy> {
    acquire(&PERMITS, PERMIT_WAIT).await
}

async fn acquire(permits: &'static Semaphore, wait: Duration) -> Result<ArgonPermit, Busy> {
    match tokio::time::timeout(wait, permits.acquire()).await {
        Ok(Ok(permit)) => Ok(ArgonPermit { _permit: permit }),
        Ok(Err(_)) | Err(_) => Err(Busy),
    }
}

/// Argon2id (Loco's default parameters) on a blocking thread.
pub async fn hash(permit: ArgonPermit, password: String) -> Result<String, HashError> {
    tokio::task::spawn_blocking(move || {
        let _held = permit;
        loco_rs::hash::hash_password(&password)
    })
    .await
    .map_err(|_| HashError)?
    .map_err(|_| HashError)
}

/// Verify on a blocking thread. `None` runs the equalizer and always fails.
pub async fn verify(permit: ArgonPermit, password: String, stored_hash: Option<String>) -> bool {
    tokio::task::spawn_blocking(move || {
        let _held = permit;
        match stored_hash {
            Some(stored) => loco_rs::hash::verify_password(&password, &stored),
            None => {
                if let Some(equalizer) = EQUALIZER_HASH.as_deref() {
                    let _ = loco_rs::hash::verify_password(&password, equalizer);
                }
                false
            }
        }
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::Semaphore;

    use super::{acquire, permit};

    #[tokio::test]
    async fn hashes_are_argon2id_and_verify_only_the_original() {
        let stored = super::hash(permit().await.unwrap(), "correct horse battery".to_owned())
            .await
            .unwrap();
        assert!(stored.starts_with("$argon2id$"));
        assert!(
            super::verify(
                permit().await.unwrap(),
                "correct horse battery".to_owned(),
                Some(stored.clone())
            )
            .await
        );
        assert!(
            !super::verify(
                permit().await.unwrap(),
                "wrong horse battery".to_owned(),
                Some(stored)
            )
            .await
        );
        assert!(
            !super::verify(
                permit().await.unwrap(),
                "correct horse battery".to_owned(),
                None
            )
            .await
        );
    }

    #[tokio::test]
    async fn a_saturated_pool_answers_busy_and_frees_up_when_work_finishes() {
        // Its own pool, so other tests' Argon2 work isn't starved.
        static ONE: Semaphore = Semaphore::const_new(1);
        let wait = Duration::from_millis(10);
        let held = acquire(&ONE, wait).await.unwrap();
        assert_eq!(acquire(&ONE, wait).await.unwrap_err(), super::Busy);
        // The permit lives inside the blocking work and is released when it finishes.
        assert!(!super::verify(held, "x".to_owned(), None).await);
        assert!(acquire(&ONE, wait).await.is_ok());
    }
}
