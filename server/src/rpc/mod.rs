use kontainer_agent_proto::{
    PROTOCOL_PACKAGE,
    v1::{
        CheckInRequest, CheckInResponse,
        controller_intake_server::{ControllerIntake, ControllerIntakeServer},
    },
};
use tonic::{Request, Response, Status};

#[derive(Debug, Default)]
pub struct ControllerIntakeService;

#[tonic::async_trait]
impl ControllerIntake for ControllerIntakeService {
    async fn check_in(
        &self,
        request: Request<CheckInRequest>,
    ) -> Result<Response<CheckInResponse>, Status> {
        let check_in = request.into_inner();
        if check_in.agent_id.trim().is_empty() {
            return Err(Status::invalid_argument("agent_id is required"));
        }

        Ok(Response::new(CheckInResponse {
            controller_version: env!("CARGO_PKG_VERSION").to_owned(),
            protocol_package: PROTOCOL_PACKAGE.to_owned(),
        }))
    }
}

#[must_use]
pub fn controller_intake_service() -> ControllerIntakeServer<ControllerIntakeService> {
    ControllerIntakeServer::new(ControllerIntakeService)
}

#[cfg(test)]
mod tests {
    use super::{CheckInRequest, ControllerIntake, ControllerIntakeService, Request};
    use kontainer_agent_proto::PROTOCOL_PACKAGE;

    #[tokio::test]
    async fn check_in_returns_the_controller_protocol_identity() {
        let response = ControllerIntakeService
            .check_in(Request::new(CheckInRequest {
                agent_id: "node-1".to_owned(),
                agent_version: "0.1.0".to_owned(),
                architecture: "aarch64".to_owned(),
            }))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(response.controller_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(response.protocol_package, PROTOCOL_PACKAGE);
    }

    #[tokio::test]
    async fn check_in_rejects_an_empty_agent_identity() {
        let error = ControllerIntakeService
            .check_in(Request::new(CheckInRequest::default()))
            .await
            .unwrap_err();

        assert_eq!(error.code(), tonic::Code::InvalidArgument);
    }
}
