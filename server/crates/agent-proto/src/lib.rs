#![forbid(unsafe_code)]

pub const PROTOCOL_PACKAGE: &str = "kontainer.agent.v1";

pub mod v1 {
    tonic::include_proto!("kontainer.agent.v1");
}

#[cfg(test)]
mod tests {
    use super::{PROTOCOL_PACKAGE, v1::CheckInRequest};

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
}
