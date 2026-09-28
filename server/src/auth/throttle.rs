//! Sign-in throttling per lowercased email.
//!
//! Ten failures inside a fifteen-minute window lock the email for fifteen
//! minutes. The counter lives in controller memory behind one mutex, so
//! concurrent failures never undercount (ADR 0014). A password reset voids an
//! active lock: sign-in passes the credential's `updated_at`, and an entry
//! older than the latest reset is discarded.

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

    /// `Some(retry_after_seconds)` while the email is locked.
    pub fn locked(
        &self,
        key: &str,
        now: DateTime<Utc>,
        credential_changed_at: Option<DateTime<Utc>>,
    ) -> Option<u32> {
        let mut entries = self.entries();
        let entry = *entries.get(key)?;
        let superseded =
            credential_changed_at.is_some_and(|changed| changed > entry.first_failure_at);
        if entry.expired(now) || superseded {
            entries.remove(key);
            return None;
        }
        entry.locked_until.map(|until| seconds_until(until, now))
    }

    /// Count one failure. The tenth inside the window locks the email.
    pub fn record_failure(&self, key: &str, now: DateTime<Utc>) -> FailureOutcome {
        let mut entries = self.entries();
        if entries.len() > PRUNE_ABOVE {
            entries.retain(|_, entry| !entry.expired(now));
        }
        let entry = entries
            .entry(key.to_owned())
            .and_modify(|entry| {
                if entry.expired(now) {
                    *entry = Entry {
                        failures: 0,
                        first_failure_at: now,
                        locked_until: None,
                    };
                }
            })
            .or_insert(Entry {
                failures: 0,
                first_failure_at: now,
                locked_until: None,
            });
        entry.failures += 1;
        if entry.failures >= MAX_FAILURES {
            let until = now + LOCK_DURATION;
            entry.locked_until = Some(until);
            FailureOutcome::Locked {
                retry_after_seconds: seconds_until(until, now),
            }
        } else {
            FailureOutcome::Remaining(MAX_FAILURES - entry.failures)
        }
    }

    pub fn clear(&self, key: &str) {
        self.entries().remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_failures_lock_and_the_lock_expires() {
        let throttle = LoginThrottle::default();
        let start = Utc::now();
        for attempt in 1..MAX_FAILURES {
            assert_eq!(
                throttle.record_failure("owner@example.com", start),
                FailureOutcome::Remaining(MAX_FAILURES - attempt)
            );
            assert_eq!(throttle.locked("owner@example.com", start, None), None);
        }
        assert_eq!(
            throttle.record_failure("owner@example.com", start),
            FailureOutcome::Locked {
                retry_after_seconds: 900
            }
        );
        assert_eq!(
            throttle.locked("owner@example.com", start + Duration::minutes(5), None),
            Some(600)
        );
        assert_eq!(throttle.locked("other@example.com", start, None), None);
        assert_eq!(
            throttle.locked("owner@example.com", start + LOCK_DURATION, None),
            None
        );
        assert_eq!(
            throttle.record_failure("owner@example.com", start + LOCK_DURATION),
            FailureOutcome::Remaining(9)
        );
    }

    #[test]
    fn the_window_resets_the_count_and_success_clears_it() {
        let throttle = LoginThrottle::default();
        let start = Utc::now();
        throttle.record_failure("owner@example.com", start);
        assert_eq!(
            throttle.record_failure("owner@example.com", start + WINDOW),
            FailureOutcome::Remaining(9)
        );
        throttle.record_failure("owner@example.com", start + WINDOW);
        throttle.clear("owner@example.com");
        assert_eq!(
            throttle.record_failure("owner@example.com", start + WINDOW),
            FailureOutcome::Remaining(9)
        );
    }

    #[test]
    fn a_password_reset_after_the_first_failure_voids_the_lock() {
        let throttle = LoginThrottle::default();
        let start = Utc::now();
        for _ in 0..MAX_FAILURES {
            throttle.record_failure("owner@example.com", start);
        }
        assert!(
            throttle
                .locked("owner@example.com", start, Some(start - Duration::days(1)))
                .is_some()
        );
        assert_eq!(
            throttle.locked(
                "owner@example.com",
                start,
                Some(start + Duration::seconds(1))
            ),
            None
        );
    }
}
