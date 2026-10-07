//! Container start and stop times for the list, from inspect.
//!
//! The list API has no start time, so each container is inspected once and the answer kept while
//! its state is unchanged, for at most a minute (an external restart is picked up within that).
//! Actions through this agent drop the entry at once.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use futures_util::{StreamExt, stream};
use kontainer_engine::{ContainerEngine, ContainerState, ContainerSummary, ContainerTimes};
use tokio_util::sync::CancellationToken;

const INSPECT_CONCURRENCY: usize = 8;
const ENTRY_TTL: Duration = Duration::from_secs(60);
/// Inspects still pending after this are left out of the page (times read as unknown).
const INSPECT_DEADLINE: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Copy)]
struct Entry {
    state: ContainerState,
    read_at: Instant,
    times: ContainerTimes,
}

#[derive(Debug, Clone, Default)]
pub struct ContainerTimesCache(Arc<Mutex<HashMap<String, Entry>>>);

impl ContainerTimesCache {
    /// Times for every container in the page, inspecting the unknown or stale ones.
    pub async fn times_for(
        &self,
        engine: &dyn ContainerEngine,
        containers: &[ContainerSummary],
        cancellation: &CancellationToken,
    ) -> HashMap<String, ContainerTimes> {
        let now = Instant::now();
        let (mut known, stale): (HashMap<_, _>, Vec<_>) = {
            let entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            let mut known = HashMap::new();
            let mut stale = Vec::new();
            for container in containers {
                match entries.get(&container.container_id) {
                    Some(entry)
                        if entry.state == container.state
                            && now.duration_since(entry.read_at) < ENTRY_TTL =>
                    {
                        known.insert(container.container_id.clone(), entry.times);
                    }
                    _ => stale.push((container.container_id.clone(), container.state)),
                }
            }
            (known, stale)
        };
        if stale.is_empty() {
            return known;
        }
        let inspects = cancellation.child_token();
        let _cancel_pending = inspects.clone().drop_guard();
        let inspects = &inspects;
        let mut results = stream::iter(stale)
            .map(|(id, state)| async move {
                let times = engine.container_times(&id, inspects).await;
                (id, state, times)
            })
            .buffer_unordered(INSPECT_CONCURRENCY);
        let expiry = tokio::time::sleep(INSPECT_DEADLINE);
        tokio::pin!(expiry);
        let mut fresh = Vec::new();
        loop {
            tokio::select! {
                () = &mut expiry => break,
                next = results.next() => match next {
                    Some((id, state, Ok(times))) => fresh.push((id, state, times)),
                    Some((_, _, Err(_))) => {}
                    None => break,
                },
            }
        }
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        // Removed containers age out; pages other than this one keep their entries.
        entries.retain(|_, entry| now.duration_since(entry.read_at) < ENTRY_TTL * 5);
        for (id, state, times) in fresh {
            entries.insert(
                id.clone(),
                Entry {
                    state,
                    read_at: now,
                    times,
                },
            );
            known.insert(id, times);
        }
        known
    }

    /// Drop one container's entry after an action changed it.
    pub fn forget(&self, container_id: &str) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(container_id);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use async_trait::async_trait;
    use kontainer_engine::{
        ContainerEngine, ContainerState, ContainerSummary, ContainerTimes, EngineError,
    };
    use tokio_util::sync::CancellationToken;

    use super::ContainerTimesCache;

    struct CountingEngine(AtomicUsize);

    #[async_trait]
    impl ContainerEngine for CountingEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }

        async fn container_times(
            &self,
            _id: &str,
            _cancellation: &CancellationToken,
        ) -> Result<ContainerTimes, EngineError> {
            let read = self.0.fetch_add(1, Ordering::SeqCst) as i64;
            Ok(ContainerTimes {
                started_at_seconds: Some(1_700_000_000 + read),
                finished_at_seconds: None,
            })
        }
    }

    fn container(id: char, state: ContainerState) -> ContainerSummary {
        ContainerSummary {
            container_id: id.to_string().repeat(64),
            state,
            ..ContainerSummary::default()
        }
    }

    #[tokio::test]
    async fn inspects_once_per_state_and_again_after_an_action() {
        let engine = Arc::new(CountingEngine(AtomicUsize::new(0)));
        let cache = ContainerTimesCache::default();
        let page = vec![
            container('a', ContainerState::Running),
            container('b', ContainerState::Exited),
        ];
        let token = CancellationToken::new();
        let first = cache.times_for(engine.as_ref(), &page, &token).await;
        assert_eq!(first.len(), 2);
        cache.times_for(engine.as_ref(), &page, &token).await;
        assert_eq!(
            engine.0.load(Ordering::SeqCst),
            2,
            "cached while the state holds"
        );

        let changed = vec![
            container('a', ContainerState::Exited),
            container('b', ContainerState::Exited),
        ];
        cache.times_for(engine.as_ref(), &changed, &token).await;
        assert_eq!(
            engine.0.load(Ordering::SeqCst),
            3,
            "a state change re-inspects"
        );

        cache.forget(&"b".repeat(64));
        cache.times_for(engine.as_ref(), &changed, &token).await;
        assert_eq!(engine.0.load(Ordering::SeqCst), 4, "an action re-inspects");
    }
}
