//! Per-session caps on long-lived streams: eight log connections and four exec sessions per
//! access-session family (ADR 0025, ADR 0026).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use uuid::Uuid;

/// Open streams of one kind, counted per access-session family.
#[derive(Clone)]
pub struct SessionSlots {
    limit: usize,
    open: Arc<Mutex<HashMap<Uuid, usize>>>,
}

impl SessionSlots {
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            open: Arc::default(),
        }
    }

    fn counts(&self) -> MutexGuard<'_, HashMap<Uuid, usize>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A slot for one more stream of `family`, or `None` at the limit.
    #[must_use]
    pub fn try_acquire(&self, family: Uuid) -> Option<SessionSlot> {
        let mut counts = self.counts();
        let count = counts.entry(family).or_insert(0);
        if *count >= self.limit {
            return None;
        }
        *count += 1;
        Some(SessionSlot {
            slots: self.clone(),
            family,
        })
    }

    /// Streams `family` holds now; tests check that dropped slots are released.
    #[cfg(test)]
    #[must_use]
    pub fn open(&self, family: Uuid) -> usize {
        self.counts().get(&family).copied().unwrap_or(0)
    }
}

/// Releases its slot when the stream ends.
pub struct SessionSlot {
    slots: SessionSlots,
    family: Uuid,
}

impl Drop for SessionSlot {
    fn drop(&mut self) {
        let mut counts = self.slots.counts();
        if let Some(count) = counts.get_mut(&self.family) {
            *count -= 1;
            if *count == 0 {
                counts.remove(&self.family);
            }
        }
    }
}
