//! Sign-in throttling per lowercased email.
//!
//! Ten failures inside a fifteen-minute window lock the email for fifteen
//! minutes. The counter lives in controller memory behind one mutex (ADR 0014).
//! Each attempt is counted *before* its password is verified and refunded on
//! success, so concurrent requests can't all slip under the cap while their
//! verifications run. A password reset voids an active lock: sign-in passes the
//! credential's `updated_at`, and an entry older than the latest reset is
//! discarded.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Duration, Utc};

pub const MAX_FAILURES: u32 = 10;
pub const LOCK_DURATION: Duration = Duration::minutes(15);
const WINDOW: Duration = Duration::minutes(15);
/// Above this many tracked emails, expired entries are pruned on write.
const PRUNE_ABOVE: usize = 1024;

#[derive(Debug, Clone, Copy)]
struct Entry {
    failures: u32,
    first_failure_at: DateTime<Utc>,
    locked_until: Option<DateTime<Utc>>,
}

impl Entry {
    fn expired(&self, now: DateTime<Utc>) -> bool {
        match self.locked_until {
            Some(until) => now >= until,
            None => now >= self.first_failure_at + WINDOW,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureOutcome {
    Remaining(u32),
    Locked { retry_after_seconds: u32 },
}

#[derive(Clone, Default)]
pub struct LoginThrottle {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
}

fn seconds_until(until: DateTime<Utc>, now: DateTime<Utc>) -> u32 {
    u32::try_from((until - now).num_seconds().max(1)).unwrap_or(u32::MAX)
}

impl LoginThrottle {
    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Reserve one attempt before the password is verified. `Err(retry_after_seconds)` while the
    /// email is locked; otherwise the attempt already counts as a failure and `Ok` says what to
    /// report if it does fail. A successful sign-in calls [`Self::clear`].
    pub fn begin_attempt(
        &self,
        key: &str,
        now: DateTime<Utc>,
        credential_changed_at: Option<DateTime<Utc>>,
    ) -> Result<FailureOutcome, u32> {
        let mut entries = self.entries();
        if entries.len() > PRUNE_ABOVE {
            entries.retain(|_, entry| !entry.expired(now));
        }
        let fresh = Entry {
            failures: 0,
            first_failure_at: now,
            locked_until: None,
        };
        let entry = entries.entry(key.to_owned()).or_insert(fresh);
        let superseded =
            credential_changed_at.is_some_and(|changed| changed > entry.first_failure_at);
        if entry.expired(now) || superseded {
            *entry = fresh;
        }
        if let Some(until) = entry.locked_until {
            return Err(seconds_until(until, now));
        }
        entry.failures += 1;
        if entry.failures >= MAX_FAILURES {
            let until = now + LOCK_DURATION;
            entry.locked_until = Some(until);
            Ok(FailureOutcome::Locked {
                retry_after_seconds: seconds_until(until, now),
            })
        } else {
            Ok(FailureOutcome::Remaining(MAX_FAILURES - entry.failures))
        }
    }

    pub fn clear(&self, key: &str) {
        self.entries().remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "owner@example.com";

    #[test]
    fn ten_attempts_lock_and_the_lock_expires() {
        let throttle = LoginThrottle::default();
        let start = Utc::now();
        for attempt in 1..MAX_FAILURES {
            assert_eq!(
                throttle.begin_attempt(KEY, start, None),
                Ok(FailureOutcome::Remaining(MAX_FAILURES - attempt))
            );
        }
        assert_eq!(
            throttle.begin_attempt(KEY, start, None),
            Ok(FailureOutcome::Locked {
                retry_after_seconds: 900
            })
        );
        assert_eq!(
            throttle.begin_attempt(KEY, start + Duration::minutes(5), None),
            Err(600)
        );
        assert_eq!(
            throttle.begin_attempt("other@example.com", start, None),
            Ok(FailureOutcome::Remaining(9))
        );
        assert_eq!(
            throttle.begin_attempt(KEY, start + LOCK_DURATION, None),
            Ok(FailureOutcome::Remaining(9))
        );
    }

    #[test]
    fn the_window_resets_the_count_and_success_clears_it() {
        let throttle = LoginThrottle::default();
        let start = Utc::now();
        throttle.begin_attempt(KEY, start, None).unwrap();
        assert_eq!(
            throttle.begin_attempt(KEY, start + WINDOW, None),
            Ok(FailureOutcome::Remaining(9))
        );
        throttle.begin_attempt(KEY, start + WINDOW, None).unwrap();
        throttle.clear(KEY);
        assert_eq!(
            throttle.begin_attempt(KEY, start + WINDOW, None),
            Ok(FailureOutcome::Remaining(9))
        );
    }

    #[test]
    fn a_password_reset_after_the_first_failure_voids_the_lock() {
        let throttle = LoginThrottle::default();
        let start = Utc::now();
        for _ in 0..MAX_FAILURES {
            throttle.begin_attempt(KEY, start, None).unwrap();
        }
        assert!(
            throttle
                .begin_attempt(KEY, start, Some(start - Duration::days(1)))
                .is_err()
        );
        assert_eq!(
            throttle.begin_attempt(KEY, start, Some(start + Duration::seconds(1))),
            Ok(FailureOutcome::Remaining(9))
        );
    }

    #[test]
    fn concurrent_attempts_get_at_most_ten_verifications() {
        let throttle = LoginThrottle::default();
        let start = Utc::now();
        let reserved = std::thread::scope(|scope| {
            let attempts: Vec<_> = (0..50)
                .map(|_| scope.spawn(|| throttle.begin_attempt(KEY, start, None).is_ok()))
                .collect();
            attempts
                .into_iter()
                .filter_map(|attempt| attempt.join().unwrap().then_some(()))
                .count()
        });
        assert_eq!(reserved, MAX_FAILURES as usize);
    }
}
