#![forbid(unsafe_code)]

pub const PROTOCOL_PACKAGE: &str = "kontainer.agent.v1";

pub mod v1 {
    tonic::include_proto!("kontainer.agent.v1");
}

#[cfg(test)]
mod tests {
    use super::{
        PROTOCOL_PACKAGE,
        v1::{CheckInRequest, ContainerState, ContainerSummary, ListContainersRequest},
    };

    #[test]
    fn protocol_package_and_wire_fields_are_stable() {
        let request = CheckInRequest {
            agent_id: "node-1".to_owned(),
            agent_version: "0.1.0".to_owned(),
            architecture: "aarch64".to_owned(),
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
}
