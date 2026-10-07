#![forbid(unsafe_code)]

pub const PROTOCOL_PACKAGE: &str = "kontainer.agent.v1";

pub mod v1 {
    tonic::include_proto!("kontainer.agent.v1");
}

#[cfg(test)]
mod tests {
    use super::{
        PROTOCOL_PACKAGE,
        v1::{
            CheckInRequest, CheckInResponse, ContainerState, ContainerSummary,
            ListContainersRequest, NodeSample,
        },
    };
    use prost::Message;

    #[test]
    fn protocol_package_and_wire_fields_are_stable() {
        let request = CheckInRequest {
            agent_id: "node-1".to_owned(),
            agent_version: "0.1.0".to_owned(),
            architecture: "aarch64".to_owned(),
            ..CheckInRequest::default()
        };

        assert_eq!(PROTOCOL_PACKAGE, "kontainer.agent.v1");
        assert_eq!(request.agent_id, "node-1");
        assert_eq!(request.architecture, "aarch64");
    }

    #[test]
    fn container_inventory_fields_are_stable_and_bounded() {
        let request = ListContainersRequest {
            page_size: 50,
            page_token: String::new(),
        };
        assert_eq!(request.page_size, 50);

        let summary = ContainerSummary {
            container_id: "a".repeat(64),
            display_name: "web".to_owned(),
            image_reference: "nginx:latest".to_owned(),
            state: ContainerState::Running as i32,
            status: "Up 3s".to_owned(),
            created_at_unix_seconds: 1_700_000_000,
            node_id: "node-1".to_owned(),
            ..ContainerSummary::default()
        };
        assert_eq!(summary.container_id.len(), 64);
        assert_eq!(
            ContainerState::try_from(summary.state).unwrap(),
            ContainerState::Running
        );
        // The private summary has no environment, command, label, mount,
        // port, or raw inspect field by construction.
        let debug = format!("{summary:?}");
        assert!(!debug.contains("command"));
        assert!(!debug.contains("label"));
    }

    #[test]
    fn check_in_samples_round_trip_and_optional_fields_stay_absent() {
        let request = CheckInRequest {
            agent_id: "manager".to_owned(),
            agent_version: "0.1.0".to_owned(),
            architecture: "aarch64".to_owned(),
            advertise_address: "127.0.0.1:50052".to_owned(),
            hostname: "manager".to_owned(),
            swarm_node_id: "tj42rq5nzwf8".to_owned(),
            docker_version: "29.8.0".to_owned(),
            os: "linux".to_owned(),
            cpu_cores: 4,
            memory_total_bytes: 8 << 30,
            sample: Some(NodeSample {
                sampled_at_unix_ms: 1_760_000_005_000,
                cpu_percent: 12.5,
                memory_used_bytes: 1 << 30,
                containers_running: 5,
                containers_paused: 1,
                containers_exited: 2,
                net_rx_bytes_per_sec: Some(1024.0),
                net_tx_bytes_per_sec: None,
            }),
        };
        let decoded = CheckInRequest::decode(request.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded, request);
        let sample = decoded.sample.unwrap();
        assert_eq!(sample.net_tx_bytes_per_sec, None, "absent, not zero");

        let response = CheckInResponse {
            controller_version: "0.1.0".to_owned(),
            protocol_package: PROTOCOL_PACKAGE.to_owned(),
            check_in_interval_seconds: 5,
        };
        assert_eq!(
            CheckInResponse::decode(response.encode_to_vec().as_slice()).unwrap(),
            response
        );
    }

    #[test]
    fn a_check_in_from_an_agent_without_sampling_still_decodes() {
        // The pre-sampling message: only the first three fields exist.
        #[derive(Clone, PartialEq, prost::Message)]
        struct LegacyCheckIn {
            #[prost(string, tag = "1")]
            agent_id: String,
            #[prost(string, tag = "2")]
            agent_version: String,
            #[prost(string, tag = "3")]
            architecture: String,
        }
        let legacy = LegacyCheckIn {
            agent_id: "node-1".to_owned(),
            agent_version: "0.0.9".to_owned(),
            architecture: "x86_64".to_owned(),
        };
        let decoded = CheckInRequest::decode(legacy.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded.agent_id, "node-1");
        assert_eq!(decoded.sample, None);
        assert_eq!(decoded.cpu_cores, 0);

        let summary = ContainerSummary::decode(
            ContainerSummary {
                container_id: "a".repeat(64),
                ..ContainerSummary::default()
            }
            .encode_to_vec()
            .as_slice(),
        )
        .unwrap();
        assert_eq!(summary.cpu_percent, None);
    }

    #[test]
    fn a_check_in_still_carrying_the_reserved_fields_decodes() {
        // An agent built before ADR 0039 sends tag 11 and the sample's tag 4.
        #[derive(Clone, PartialEq, prost::Message)]
        struct OlderSample {
            #[prost(double, tag = "2")]
            cpu_percent: f64,
            #[prost(uint64, optional, tag = "4")]
            disk_used_bytes: Option<u64>,
        }
        #[derive(Clone, PartialEq, prost::Message)]
        struct OlderCheckIn {
            #[prost(string, tag = "1")]
            agent_id: String,
            #[prost(int64, tag = "11")]
            agent_started_at_unix_ms: i64,
            #[prost(message, optional, tag = "12")]
            sample: Option<OlderSample>,
        }
        let older = OlderCheckIn {
            agent_id: "node-1".to_owned(),
            agent_started_at_unix_ms: 1_760_000_000_000,
            sample: Some(OlderSample {
                cpu_percent: 12.5,
                disk_used_bytes: Some(5 << 30),
            }),
        };
        let decoded = CheckInRequest::decode(older.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded.agent_id, "node-1");
        assert_eq!(decoded.sample.unwrap().cpu_percent, 12.5);
    }
}
