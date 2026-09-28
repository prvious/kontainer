//! Node-level facts and container resource figures, in Kontainer's own types.

use std::collections::HashMap;

/// `docker info`, reduced to what check-in reports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeInfo {
    pub hostname: String,
    /// Swarm node id; `None` when the daemon is not part of a Swarm.
    pub swarm_node_id: Option<String>,
    pub docker_version: String,
    pub os: String,
    pub architecture: String,
    pub cpu_cores: u32,
    pub memory_total_bytes: u64,
    pub containers_running: u32,
    pub containers_paused: u32,
    pub containers_stopped: u32,
}

/// One stats reading for a running container. CPU is percent of one core times cores
/// used (Docker's convention: 0 to 100 × cores). Network counters are cumulative.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ContainerStats {
    pub cpu_percent: Option<f64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_limit_bytes: Option<u64>,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
}

/// `docker system df`: images, containers, volumes, and build cache summed. `None` when the
/// daemon reported no category totals (old API versions only send per-item sizes).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiskUsage {
    pub total_bytes: Option<u64>,
    /// Bytes per volume name, for the volumes whose driver reports a size.
    pub volume_sizes: std::collections::BTreeMap<String, u64>,
}

/// CPU counters from one side of a stats reading.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CpuCounters {
    pub total_usage: u64,
    pub system_usage: u64,
    pub online_cpus: u32,
}

/// Docker's CPU percent: container delta over system delta, times online CPUs, times 100.
/// `None` when either delta is missing or the system counter did not advance.
#[must_use]
pub fn cpu_percent(current: CpuCounters, previous: CpuCounters) -> Option<f64> {
    let container_delta = current.total_usage.checked_sub(previous.total_usage)?;
    let system_delta = current.system_usage.checked_sub(previous.system_usage)?;
    if system_delta == 0 || previous.system_usage == 0 || current.online_cpus == 0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    Some(container_delta as f64 / system_delta as f64 * f64::from(current.online_cpus) * 100.0)
}

/// Memory in use as `docker stats` shows it: usage minus reclaimable page cache
/// (`inactive_file` on cgroup v2, `cache` on v1).
#[must_use]
pub fn memory_used(usage: u64, stats: &HashMap<String, u64>) -> u64 {
    let cache = stats
        .get("inactive_file")
        .or_else(|| stats.get("cache"))
        .copied()
        .unwrap_or(0);
    usage.saturating_sub(cache)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{CpuCounters, cpu_percent, memory_used};

    #[test]
    fn cpu_percent_follows_docker_stats() {
        let previous = CpuCounters {
            total_usage: 1_000_000,
            system_usage: 10_000_000,
            online_cpus: 4,
        };
        let current = CpuCounters {
            total_usage: 1_500_000,
            system_usage: 20_000_000,
            online_cpus: 4,
        };
        // 0.5M of 10M system ticks across 4 CPUs = 20 % of one core.
        assert_eq!(cpu_percent(current, previous), Some(20.0));
        assert_eq!(
            cpu_percent(
                current,
                CpuCounters {
                    system_usage: 0,
                    ..previous
                }
            ),
            None,
            "no previous reading"
        );
        assert_eq!(
            cpu_percent(previous, previous),
            None,
            "system counter did not move"
        );
        assert_eq!(
            cpu_percent(previous, current),
            None,
            "counters went backwards"
        );
    }

    #[test]
    fn memory_excludes_reclaimable_cache() {
        let v2 = HashMap::from([("inactive_file".to_owned(), 100)]);
        let v1 = HashMap::from([("cache".to_owned(), 300)]);
        assert_eq!(memory_used(1000, &v2), 900);
        assert_eq!(memory_used(1000, &v1), 700);
        assert_eq!(memory_used(1000, &HashMap::new()), 1000);
        assert_eq!(memory_used(10, &v1), 0);
    }
}
