//! The supervised check-in task: sample, call `CheckIn`, adopt the controller's cadence.
//!
//! The agent never exits because the controller is away: failures log at warn and back off,
//! capped at 30 seconds, until the controller answers or shutdown cancels the loop.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use kontainer_agent_proto::v1::{
    CheckInRequest, CheckInResponse, NodeSample, controller_intake_client::ControllerIntakeClient,
};
use kontainer_engine::NodeInfo;
use kontainer_rpc_auth::MutualTlsIdentityFiles;
use tokio_util::sync::CancellationToken;
use tonic::transport::Endpoint;

use crate::sampling::{NodeReading, NodeSampler};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const RPC_TIMEOUT: Duration = Duration::from_secs(3);
/// Cadence until the controller sets one.
pub const DEFAULT_CHECK_IN_INTERVAL: Duration = Duration::from_secs(5);
/// Accepted controller cadences; anything outside keeps the current interval.
const INTERVAL_BOUNDS: std::ops::RangeInclusive<u32> = 1..=300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckInError {
    Unavailable,
    TimedOut,
    Rejected,
    Cancelled,
}

impl std::fmt::Display for CheckInError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "controller is unavailable",
            Self::TimedOut => "controller did not answer in time",
            Self::Rejected => "controller rejected the check-in",
            Self::Cancelled => "check-in was cancelled",
        })
    }
}

/// The controller end of check-in; production dials over mTLS, tests fake it.
#[async_trait]
pub trait ControllerLink: Send + Sync {
    async fn check_in(
        &self,
        request: CheckInRequest,
        cancellation: &CancellationToken,
    ) -> Result<CheckInResponse, CheckInError>;
}

/// Connect-per-call mTLS client for `ControllerIntake`. The controller must present a controller
/// identity of this agent's cluster; the agent presents its own certificate.
pub struct MtlsControllerLink {
    endpoint: Endpoint,
}

impl MtlsControllerLink {
    /// Validate settings and TLS material without connecting.
    pub fn new(
        endpoint: &str,
        server_name: &str,
        cluster_id: &str,
        tls_files: &MutualTlsIdentityFiles,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if !endpoint.starts_with("https://") {
            return Err("KONTAINER_CONTROLLER_RPC_ENDPOINT must be an https:// URL".into());
        }
        let tls = tls_files.client_config_for_custom_verifier(server_name.to_owned())?;
        let verifier = tls_files.controller_verifier(cluster_id.to_owned())?;
        let endpoint = Endpoint::from_shared(endpoint.to_owned())
            .map_err(|_| "KONTAINER_CONTROLLER_RPC_ENDPOINT is not a valid URL")?
            .timeout(RPC_TIMEOUT)
            .tls_config_with_verifier(tls, verifier)
            .map_err(|_| "controller client TLS files are invalid")?;
        Ok(Self { endpoint })
    }
}

#[async_trait]
impl ControllerLink for MtlsControllerLink {
    async fn check_in(
        &self,
        request: CheckInRequest,
        cancellation: &CancellationToken,
    ) -> Result<CheckInResponse, CheckInError> {
        let call = async {
            let channel = tokio::time::timeout(CONNECT_TIMEOUT, self.endpoint.connect())
                .await
                .map_err(|_| CheckInError::TimedOut)?
                .map_err(|_| CheckInError::Unavailable)?;
            let mut client = ControllerIntakeClient::new(channel);
            let mut request = tonic::Request::new(request);
            request.set_timeout(RPC_TIMEOUT);
            match tokio::time::timeout(RPC_TIMEOUT, client.check_in(request)).await {
                Ok(Ok(response)) => Ok(response.into_inner()),
                Ok(Err(status)) => Err(match status.code() {
                    tonic::Code::Unavailable => CheckInError::Unavailable,
                    tonic::Code::DeadlineExceeded => CheckInError::TimedOut,
                    _ => CheckInError::Rejected,
                }),
                Err(_) => Err(CheckInError::TimedOut),
            }
        };
        tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(CheckInError::Cancelled),
            result = call => result,
        }
    }
}

/// What the agent says about itself on every check-in, besides the sample.
#[derive(Debug, Clone)]
pub struct AgentFacts {
    pub node_id: String,
    pub advertise_address: String,
}

/// Timing knobs; production uses [`CheckInTiming::default`], tests shrink them.
#[derive(Debug, Clone, Copy)]
pub struct CheckInTiming {
    pub initial_interval: Duration,
    pub max_backoff: Duration,
}

impl Default for CheckInTiming {
    fn default() -> Self {
        Self {
            initial_interval: DEFAULT_CHECK_IN_INTERVAL,
            // Inside the controller's 15 s online window: after a controller restart, agents
            // check back in within 10 s instead of up to 30 s.
            max_backoff: Duration::from_secs(10),
        }
    }
}

/// Run until `cancellation`. Returns only on shutdown.
pub async fn run_check_in(
    link: Arc<dyn ControllerLink>,
    mut sampler: NodeSampler,
    facts: AgentFacts,
    timing: CheckInTiming,
    cancellation: CancellationToken,
) {
    let mut interval = timing.initial_interval;
    let mut failures: u32 = 0;
    let mut last_info: Option<NodeInfo> = None;
    while !cancellation.is_cancelled() {
        let started = tokio::time::Instant::now();
        // Stats get 80 % of the interval so the call still lands on cadence.
        let stats_deadline = interval.mul_f64(0.8);
        let reading = match sampler.sample(stats_deadline, &cancellation).await {
            Ok(reading) => {
                last_info = Some(reading.info.clone());
                Some(reading)
            }
            Err(error) => {
                tracing::warn!(%error, "node sample failed; checking in without one");
                None
            }
        };
        let request = check_in_request(&facts, last_info.as_ref(), reading.as_ref());
        let delay = match link.check_in(request, &cancellation).await {
            Ok(response) => {
                if failures > 0 {
                    tracing::info!(failures, "controller check-in recovered");
                }
                failures = 0;
                if INTERVAL_BOUNDS.contains(&response.check_in_interval_seconds) {
                    interval = Duration::from_secs(u64::from(response.check_in_interval_seconds));
                }
                interval
            }
            Err(CheckInError::Cancelled) => break,
            Err(error) => {
                failures = failures.saturating_add(1);
                let backoff = backoff(interval, failures, timing.max_backoff);
                tracing::warn!(%error, failures, retry_in_ms = backoff.as_millis(), "controller check-in failed");
                backoff
            }
        };
        tokio::select! {
            () = cancellation.cancelled() => break,
            () = tokio::time::sleep_until(started + delay) => {}
        }
    }
}

/// Doubling from the interval, capped.
fn backoff(interval: Duration, failures: u32, cap: Duration) -> Duration {
    let factor = 2_u32.saturating_pow(failures.saturating_sub(1).min(16));
    interval.saturating_mul(factor).min(cap)
}

fn check_in_request(
    facts: &AgentFacts,
    info: Option<&NodeInfo>,
    reading: Option<&NodeReading>,
) -> CheckInRequest {
    let info = info.cloned().unwrap_or_default();
    CheckInRequest {
        agent_id: facts.node_id.clone(),
        agent_version: env!("CARGO_PKG_VERSION").to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        advertise_address: facts.advertise_address.clone(),
        hostname: info.hostname,
        swarm_node_id: info.swarm_node_id.unwrap_or_default(),
        docker_version: info.docker_version,
        os: info.os,
        cpu_cores: info.cpu_cores,
        memory_total_bytes: info.memory_total_bytes,
        sample: reading.map(|reading| NodeSample {
            sampled_at_unix_ms: reading.sampled_at_unix_ms,
            cpu_percent: reading.cpu_percent,
            memory_used_bytes: reading.memory_used_bytes,
            containers_running: reading.info.containers_running,
            containers_paused: reading.info.containers_paused,
            containers_exited: reading.info.containers_stopped,
            net_rx_bytes_per_sec: reading.net_rx_bytes_per_sec,
            net_tx_bytes_per_sec: reading.net_tx_bytes_per_sec,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use kontainer_agent_proto::v1::{CheckInRequest, CheckInResponse};
    use tokio_util::sync::CancellationToken;

    use super::{AgentFacts, CheckInError, CheckInTiming, ControllerLink, backoff, run_check_in};
    use crate::sampling::{ContainerStatsCache, NodeSampler, tests::FakeNodeEngine};

    /// Fails the first `down_for` calls, then answers with `interval_seconds`.
    struct FlakyController {
        down_for: usize,
        calls: AtomicUsize,
        interval_seconds: u32,
        seen: Mutex<Vec<CheckInRequest>>,
    }

    #[async_trait]
    impl ControllerLink for FlakyController {
        async fn check_in(
            &self,
            request: CheckInRequest,
            _: &CancellationToken,
        ) -> Result<CheckInResponse, CheckInError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            self.seen.lock().unwrap().push(request);
            if call < self.down_for {
                return Err(CheckInError::Unavailable);
            }
            Ok(CheckInResponse {
                controller_version: "test".to_owned(),
                protocol_package: kontainer_agent_proto::PROTOCOL_PACKAGE.to_owned(),
                check_in_interval_seconds: self.interval_seconds,
            })
        }
    }

    fn facts() -> AgentFacts {
        AgentFacts {
            node_id: "manager".to_owned(),
            advertise_address: "127.0.0.1:50052".to_owned(),
        }
    }

    #[test]
    fn backoff_doubles_from_the_interval_and_caps_inside_the_online_window() {
        let cap = CheckInTiming::default().max_backoff;
        assert_eq!(cap, Duration::from_secs(10));
        let interval = Duration::from_secs(5);
        let delays: Vec<_> = (1..=4)
            .map(|n| backoff(interval, n, cap).as_secs())
            .collect();
        assert_eq!(delays, vec![5, 10, 10, 10]);
        assert_eq!(backoff(interval, u32::MAX, cap), cap);
    }

    #[tokio::test]
    async fn the_task_survives_controller_downtime_and_stops_on_shutdown() {
        let controller = Arc::new(FlakyController {
            down_for: 3,
            calls: AtomicUsize::new(0),
            interval_seconds: 0,
            seen: Mutex::new(Vec::new()),
        });
        let sampler = NodeSampler::new(
            Arc::new(FakeNodeEngine::two_containers()),
            ContainerStatsCache::default(),
        );
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(run_check_in(
            controller.clone(),
            sampler,
            facts(),
            CheckInTiming {
                initial_interval: Duration::from_millis(10),
                max_backoff: Duration::from_millis(40),
            },
            cancellation.clone(),
        ));

        tokio::time::timeout(Duration::from_secs(5), async {
            while controller.calls.load(Ordering::SeqCst) < 6 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("check-in kept retrying through downtime and resumed");
        cancellation.cancel();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("shutdown stops the loop")
            .unwrap();

        let seen = controller.seen.lock().unwrap();
        let first = &seen[0];
        assert_eq!(first.agent_id, "manager");
        assert_eq!(first.advertise_address, "127.0.0.1:50052");
        assert_eq!(first.hostname, "manager");
        assert_eq!(first.swarm_node_id, "tj42rq5nzwf8");
        assert_eq!(first.cpu_cores, 4);
        let sample = first.sample.as_ref().unwrap();
        assert_eq!(
            seen[1].sample.as_ref().unwrap().cpu_percent,
            50.0,
            "CPU from the second tick on"
        );
        assert_eq!(sample.containers_running, 2);
        assert_eq!(sample.containers_exited, 1);
    }

    #[tokio::test]
    async fn a_docker_outage_still_checks_in_without_a_sample() {
        let controller = Arc::new(FlakyController {
            down_for: 0,
            calls: AtomicUsize::new(0),
            interval_seconds: 0,
            seen: Mutex::new(Vec::new()),
        });
        let mut engine = FakeNodeEngine::two_containers();
        engine.info_fails = true;
        let sampler = NodeSampler::new(Arc::new(engine), ContainerStatsCache::default());
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(run_check_in(
            controller.clone(),
            sampler,
            facts(),
            CheckInTiming {
                initial_interval: Duration::from_millis(10),
                max_backoff: Duration::from_millis(40),
            },
            cancellation.clone(),
        ));
        tokio::time::timeout(Duration::from_secs(5), async {
            while controller.calls.load(Ordering::SeqCst) < 1 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        cancellation.cancel();
        task.await.unwrap();
        let seen = controller.seen.lock().unwrap();
        assert_eq!(seen[0].agent_id, "manager");
        assert!(seen[0].sample.is_none());
    }
}
