//! Node sampling for check-in, entirely through the engine trait.
//!
//! ponytail: node figures are container-scoped (the sum of container stats), so development (TCP
//! to DinD) and production (the local socket) report the same thing. Host-level sampling (procfs
//! or a node exporter) is the upgrade when node overhead outside containers matters.

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, PoisonError, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::{StreamExt, stream};
use kontainer_engine::{ContainerEngine, ContainerStats, EngineError, NodeInfo, cpu_percent};
use tokio_util::sync::CancellationToken;

/// Stats reads in flight at once.
const STATS_CONCURRENCY: usize = 8;
/// `docker system df` runs on every twelfth sample; the controller carries the value forward.
pub const DISK_SAMPLE_EVERY: u64 = 12;

/// The last tick's per-container stats. `ListContainers` reads it so the list carries CPU and
/// memory without extra Docker calls.
#[derive(Debug, Clone, Default)]
pub struct ContainerStatsCache(Arc<RwLock<HashMap<String, ContainerStats>>>);

impl ContainerStatsCache {
    #[must_use]
    pub fn get(&self, container_id: &str) -> Option<ContainerStats> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(container_id)
            .copied()
    }

    fn snapshot(&self) -> HashMap<String, ContainerStats> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn replace(&self, stats: HashMap<String, ContainerStats>) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = stats;
    }
}

/// Volume sizes from the last `docker system df`, which runs every twelfth sample. `None` until
/// the first one; `ListVolumes` reads it so listing never walks volume contents.
#[derive(Debug, Clone, Default)]
pub struct VolumeSizeCache(Arc<RwLock<Option<BTreeMap<String, u64>>>>);

impl VolumeSizeCache {
    /// The size of one volume, `None` while unknown.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<u64> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .and_then(|sizes| sizes.get(name).copied())
    }

    fn replace(&self, sizes: BTreeMap<String, u64>) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = Some(sizes);
    }
}

/// One node reading in Kontainer's own terms; `check_in` maps it to protobuf.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeReading {
    pub info: NodeInfo,
    pub sampled_at_unix_ms: i64,
    /// 0–100 of the whole node.
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub disk_used_bytes: Option<u64>,
    pub net_rx_bytes_per_sec: Option<f64>,
    pub net_tx_bytes_per_sec: Option<f64>,
}

pub struct NodeSampler {
    engine: Arc<dyn ContainerEngine>,
    cache: ContainerStatsCache,
    volume_sizes: VolumeSizeCache,
    ticks: u64,
    previous_at: Option<Instant>,
}

impl NodeSampler {
    #[must_use]
    pub fn new(engine: Arc<dyn ContainerEngine>, cache: ContainerStatsCache) -> Self {
        Self {
            engine,
            cache,
            volume_sizes: VolumeSizeCache::default(),
            ticks: 0,
            previous_at: None,
        }
    }

    /// Publish each `docker system df`'s volume sizes for `ListVolumes`.
    #[must_use]
    pub fn with_volume_sizes(mut self, volume_sizes: VolumeSizeCache) -> Self {
        self.volume_sizes = volume_sizes;
        self
    }

    /// Take one reading. Container stats that miss `stats_deadline` are left out of this tick;
    /// `docker info` or the container listing failing fails the reading, so check-in reports no
    /// sample rather than zeros.
    pub async fn sample(
        &mut self,
        stats_deadline: Duration,
        cancellation: &CancellationToken,
    ) -> Result<NodeReading, EngineError> {
        let tick = self.ticks;
        self.ticks += 1;
        let info = self.engine.node_info(cancellation).await?;
        let running = self.engine.running_container_ids(cancellation).await?;
        let mut current = self.read_stats(running, stats_deadline, cancellation).await;
        let now = Instant::now();
        let elapsed = self.previous_at.map(|previous| now - previous);
        let previous = self.cache.snapshot();
        self.previous_at = Some(now);
        container_cpu_percents(&previous, &mut current);

        let (rx_rate, tx_rate) = network_rates(&previous, &current, elapsed);
        let cpu_percent = node_cpu_percent(&current, info.cpu_cores);
        let memory_used_bytes = current
            .values()
            .filter_map(|stats| stats.memory_used_bytes)
            .fold(0_u64, u64::saturating_add);
        let disk_used_bytes = if tick.is_multiple_of(DISK_SAMPLE_EVERY) {
            match self.engine.disk_usage(cancellation).await {
                Ok(usage) => {
                    self.volume_sizes.replace(usage.volume_sizes);
                    usage.total_bytes
                }
                Err(_) => None,
            }
        } else {
            None
        };
        self.cache.replace(current);

        Ok(NodeReading {
            info,
            sampled_at_unix_ms: unix_ms(SystemTime::now()),
            cpu_percent,
            memory_used_bytes,
            disk_used_bytes,
            net_rx_bytes_per_sec: rx_rate,
            net_tx_bytes_per_sec: tx_rate,
        })
    }

    async fn read_stats(
        &self,
        container_ids: Vec<String>,
        deadline: Duration,
        cancellation: &CancellationToken,
    ) -> HashMap<String, ContainerStats> {
        // Reads still pending at the deadline are cancelled, not detached.
        let reads = cancellation.child_token();
        let _cancel_pending = reads.clone().drop_guard();
        let engine = &self.engine;
        let reads = &reads;
        let mut results = stream::iter(container_ids)
            .map(|id| async move {
                let stats = engine.container_stats(&id, reads).await;
                (id, stats)
            })
            .buffer_unordered(STATS_CONCURRENCY);
        let mut collected = HashMap::new();
        let expiry = tokio::time::sleep(deadline);
        tokio::pin!(expiry);
        loop {
            tokio::select! {
                () = &mut expiry => break,
                next = results.next() => match next {
                    Some((id, Ok(stats))) => {
                        collected.insert(id, stats);
                    }
                    Some((_, Err(_))) => {}
                    None => break,
                },
            }
        }
        collected
    }
}

/// Each container's CPU percent from its counters' delta since the previous tick, like
/// `network_rates`. Engine reads are one-shot, so a container without a previous reading (new,
/// or missed last tick's deadline) has none this tick; on the agent's first tick that leaves
/// every container out and the node reads 0 % once.
fn container_cpu_percents(
    previous: &HashMap<String, ContainerStats>,
    current: &mut HashMap<String, ContainerStats>,
) {
    for (id, stats) in current.iter_mut() {
        stats.cpu_percent = previous
            .get(id)
            .and_then(|before| cpu_percent(stats.cpu, before.cpu));
    }
}

/// Sum of container CPU percent over cores: Docker reports 100 per fully used core.
fn node_cpu_percent(stats: &HashMap<String, ContainerStats>, cpu_cores: u32) -> f64 {
    if cpu_cores == 0 {
        return 0.0;
    }
    // A fold from +0.0: an empty f64 `sum()` is -0.0, which would serialize as "-0".
    let total = stats
        .values()
        .filter_map(|stats| stats.cpu_percent)
        .fold(0.0, |total, percent| total + percent);
    (total / f64::from(cpu_cores)).clamp(0.0, 100.0)
}

/// Bytes per second from per-container counter deltas. Containers that started or stopped between
/// samples, or whose counters reset, are left out. `None` on the first sample.
fn network_rates(
    previous: &HashMap<String, ContainerStats>,
    current: &HashMap<String, ContainerStats>,
    elapsed: Option<Duration>,
) -> (Option<f64>, Option<f64>) {
    let Some(seconds) = elapsed
        .map(|elapsed| elapsed.as_secs_f64())
        .filter(|s| *s > 0.0)
    else {
        return (None, None);
    };
    let (rx, tx) = current
        .iter()
        .filter_map(|(id, now)| previous.get(id).map(|before| (before, now)))
        .fold((0_u64, 0_u64), |(rx, tx), (before, now)| {
            (
                rx.saturating_add(now.net_rx_bytes.saturating_sub(before.net_rx_bytes)),
                tx.saturating_add(now.net_tx_bytes.saturating_sub(before.net_tx_bytes)),
            )
        });
    #[allow(clippy::cast_precision_loss)]
    (Some(rx as f64 / seconds), Some(tx as f64 / seconds))
}

pub(crate) fn unix_ms(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        collections::HashMap,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use kontainer_engine::{
        ContainerEngine, ContainerStats, CpuCounters, DiskUsage, EngineError, EngineErrorKind,
        EngineOperation, NodeInfo,
    };
    use tokio_util::sync::CancellationToken;

    use super::{
        ContainerStatsCache, NodeSampler, VolumeSizeCache, container_cpu_percents, network_rates,
        node_cpu_percent,
    };

    /// System CPU nanoseconds per round across the fake's 4 online CPUs.
    const SYSTEM_STEP: u64 = 4_000_000_000;

    /// Two running containers with known stats; per round of two reads, network counters grow
    /// by `step` and each container's CPU counter by its `cpu.total_usage` (one-shot reads, like
    /// the real engine: `cpu_percent` is left `None`).
    pub(crate) struct FakeNodeEngine {
        pub stats: HashMap<String, ContainerStats>,
        pub disk_reads: AtomicUsize,
        pub reads: AtomicUsize,
        pub step: u64,
        pub slow: Mutex<Option<String>>,
        pub info_fails: bool,
        pub listing_fails: bool,
    }

    impl FakeNodeEngine {
        pub(crate) fn two_containers() -> Self {
            // `cores_busy` of the 4 CPUs per round: 1.5 cores is 150 % in Docker's terms.
            let stats = |cores_busy: f64, memory| ContainerStats {
                cpu_percent: None,
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                cpu: CpuCounters {
                    total_usage: (cores_busy / 4.0 * SYSTEM_STEP as f64) as u64,
                    system_usage: SYSTEM_STEP,
                    online_cpus: 4,
                },
                memory_used_bytes: Some(memory),
                memory_limit_bytes: Some(1 << 30),
                net_rx_bytes: 0,
                net_tx_bytes: 0,
            };
            Self {
                stats: HashMap::from([
                    ("a".repeat(64), stats(1.5, 100 << 20)),
                    ("b".repeat(64), stats(0.5, 50 << 20)),
                ]),
                disk_reads: AtomicUsize::new(0),
                reads: AtomicUsize::new(0),
                step: 1000,
                slow: Mutex::new(None),
                info_fails: false,
                listing_fails: false,
            }
        }
    }

    #[async_trait]
    impl ContainerEngine for FakeNodeEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }

        async fn node_info(&self, _: &CancellationToken) -> Result<NodeInfo, EngineError> {
            if self.info_fails {
                return Err(EngineError::new(
                    EngineErrorKind::ConnectionUnavailable,
                    EngineOperation::ReadSystemInfo,
                ));
            }
            Ok(NodeInfo {
                hostname: "manager".to_owned(),
                swarm_node_id: Some("tj42rq5nzwf8".to_owned()),
                docker_version: "29.8.0".to_owned(),
                os: "Alpine Linux".to_owned(),
                architecture: "aarch64".to_owned(),
                cpu_cores: 4,
                memory_total_bytes: 8 << 30,
                containers_running: 2,
                containers_paused: 0,
                containers_stopped: 1,
            })
        }

        async fn running_container_ids(
            &self,
            _: &CancellationToken,
        ) -> Result<Vec<String>, EngineError> {
            if self.listing_fails {
                return Err(EngineError::new(
                    EngineErrorKind::ConnectionUnavailable,
                    EngineOperation::ListContainers,
                ));
            }
            let mut ids: Vec<_> = self.stats.keys().cloned().collect();
            ids.sort();
            Ok(ids)
        }

        async fn container_stats(
            &self,
            container_id: &str,
            cancellation: &CancellationToken,
        ) -> Result<ContainerStats, EngineError> {
            if self.slow.lock().unwrap().as_deref() == Some(container_id) {
                cancellation.cancelled().await;
                return Err(EngineError::new(
                    EngineErrorKind::Cancelled,
                    EngineOperation::ReadContainerStats,
                ));
            }
            let read = self.reads.fetch_add(1, Ordering::SeqCst) as u64;
            let mut stats = self.stats[container_id];
            // Both containers' counters advance per round of two reads.
            let round = read / 2;
            stats.net_rx_bytes = round * self.step;
            stats.net_tx_bytes = round * self.step / 2;
            stats.cpu.total_usage *= round + 1;
            stats.cpu.system_usage *= round + 1;
            Ok(stats)
        }

        async fn disk_usage(&self, _: &CancellationToken) -> Result<DiskUsage, EngineError> {
            self.disk_reads.fetch_add(1, Ordering::SeqCst);
            Ok(DiskUsage {
                total_bytes: Some(5 << 30),
                volume_sizes: std::collections::BTreeMap::from([("pg-data".to_owned(), 4096)]),
            })
        }
    }

    #[tokio::test]
    async fn node_figures_sum_container_stats_over_cores() {
        let engine = Arc::new(FakeNodeEngine::two_containers());
        let cache = ContainerStatsCache::default();
        let mut sampler = NodeSampler::new(engine.clone(), cache.clone());

        let first = sampler
            .sample(Duration::from_secs(1), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(first.memory_used_bytes, 150 << 20);
        assert_eq!(first.info.containers_running, 2);
        assert_eq!(first.net_rx_bytes_per_sec, None, "no previous sample");
        assert_eq!(
            cache.get(&"a".repeat(64)).unwrap().cpu_percent,
            None,
            "CPU needs a previous reading"
        );

        let reading = sampler
            .sample(Duration::from_secs(1), &CancellationToken::new())
            .await
            .unwrap();
        // (150 + 50) % of one core across 4 cores = 50 % of the node.
        assert_eq!(reading.cpu_percent, 50.0);
        assert_eq!(
            cache.get(&"a".repeat(64)).unwrap().cpu_percent,
            Some(150.0),
            "ListContainers reads the last tick"
        );
    }

    #[test]
    fn container_cpu_comes_from_the_previous_readings_counters() {
        let counters = |total_usage, system_usage| ContainerStats {
            cpu: CpuCounters {
                total_usage,
                system_usage,
                online_cpus: 2,
            },
            ..ContainerStats::default()
        };
        let previous = HashMap::from([
            ("a".to_owned(), counters(1_000_000, 20_000_000)),
            ("gone".to_owned(), counters(5, 5)),
        ]);
        let mut current = HashMap::from([
            ("a".to_owned(), counters(3_000_000, 40_000_000)),
            ("new".to_owned(), counters(9_000_000, 40_000_000)),
        ]);
        container_cpu_percents(&previous, &mut current);
        // 2M of 20M system ticks across 2 CPUs = 20 % of one core, as `docker stats` shows.
        assert_eq!(current["a"].cpu_percent, Some(20.0));
        assert_eq!(current["new"].cpu_percent, None, "no previous reading");

        // A restarted container's counters went backwards: unknown, never negative.
        let mut restarted = HashMap::from([("a".to_owned(), counters(10, 50_000_000))]);
        container_cpu_percents(&previous, &mut restarted);
        assert_eq!(restarted["a"].cpu_percent, None);
    }

    #[tokio::test]
    async fn disk_usage_is_read_on_every_twelfth_sample_only() {
        let engine = Arc::new(FakeNodeEngine::two_containers());
        let sizes = VolumeSizeCache::default();
        let mut sampler = NodeSampler::new(engine.clone(), ContainerStatsCache::default())
            .with_volume_sizes(sizes.clone());
        assert_eq!(sizes.get("pg-data"), None, "unknown before the first df");
        let mut with_disk = Vec::new();
        for tick in 0..25 {
            let reading = sampler
                .sample(Duration::from_secs(1), &CancellationToken::new())
                .await
                .unwrap();
            if reading.disk_used_bytes.is_some() {
                with_disk.push(tick);
            }
        }
        assert_eq!(with_disk, vec![0, 12, 24]);
        assert_eq!(engine.disk_reads.load(Ordering::SeqCst), 3);
        assert_eq!(sizes.get("pg-data"), Some(4096));
        assert_eq!(sizes.get("other"), None);
    }

    #[tokio::test]
    async fn stats_past_the_deadline_are_left_out_and_cancelled() {
        let engine = Arc::new(FakeNodeEngine::two_containers());
        *engine.slow.lock().unwrap() = Some("b".repeat(64));
        let cache = ContainerStatsCache::default();
        let mut sampler = NodeSampler::new(engine.clone(), cache.clone());

        let started = std::time::Instant::now();
        let reading = sampler
            .sample(Duration::from_millis(50), &CancellationToken::new())
            .await
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(
            reading.memory_used_bytes,
            100 << 20,
            "only the fast container"
        );
        assert!(cache.get(&"b".repeat(64)).is_none());
    }

    #[tokio::test]
    async fn a_failed_docker_info_fails_the_reading() {
        let mut engine = FakeNodeEngine::two_containers();
        engine.info_fails = true;
        let mut sampler = NodeSampler::new(Arc::new(engine), ContainerStatsCache::default());
        assert!(
            sampler
                .sample(Duration::from_secs(1), &CancellationToken::new())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_failed_container_listing_fails_the_reading_rather_than_reading_idle() {
        let mut engine = FakeNodeEngine::two_containers();
        engine.listing_fails = true;
        let cache = ContainerStatsCache::default();
        let mut sampler = NodeSampler::new(Arc::new(engine), cache.clone());
        assert!(
            sampler
                .sample(Duration::from_secs(1), &CancellationToken::new())
                .await
                .is_err()
        );
        assert!(
            cache.get(&"a".repeat(64)).is_none(),
            "no zeroed stats either"
        );
    }

    #[test]
    fn network_rates_use_per_container_deltas() {
        let counters = |rx, tx| ContainerStats {
            net_rx_bytes: rx,
            net_tx_bytes: tx,
            ..ContainerStats::default()
        };
        let previous = HashMap::from([
            ("a".to_owned(), counters(1000, 100)),
            ("gone".to_owned(), counters(9000, 9000)),
        ]);
        let current = HashMap::from([
            ("a".to_owned(), counters(3000, 600)),
            ("new".to_owned(), counters(7000, 7000)),
        ]);
        assert_eq!(
            network_rates(&previous, &current, Some(Duration::from_secs(2))),
            (Some(1000.0), Some(250.0))
        );
        assert_eq!(network_rates(&previous, &current, None), (None, None));
        // A counter reset (restart) reads as zero, never negative.
        let reset = HashMap::from([("a".to_owned(), counters(10, 10))]);
        assert_eq!(
            network_rates(&previous, &reset, Some(Duration::from_secs(1))),
            (Some(0.0), Some(0.0))
        );
    }

    #[test]
    fn cpu_is_bounded_and_safe_without_cores() {
        let busy = HashMap::from([(
            "a".to_owned(),
            ContainerStats {
                cpu_percent: Some(900.0),
                ..ContainerStats::default()
            },
        )]);
        assert_eq!(node_cpu_percent(&busy, 4), 100.0);
        assert_eq!(node_cpu_percent(&busy, 0), 0.0);
        assert_eq!(node_cpu_percent(&HashMap::new(), 4), 0.0);
        assert!(
            node_cpu_percent(&HashMap::new(), 4).is_sign_positive(),
            "never -0"
        );
    }
}
