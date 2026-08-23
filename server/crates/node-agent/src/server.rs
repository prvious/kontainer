use std::error::Error;

use kontainer_agent_proto::v1::node_agent_server::NodeAgentServer;
use kontainer_rpc_auth::{MutualTlsIdentityFiles, PeerIdentityInterceptor, WorkloadKind};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;

use crate::NodeAgentService;

pub async fn serve_node_agent_rpc(
    listener: TcpListener,
    cluster_id: String,
    tls_files: MutualTlsIdentityFiles,
    service: NodeAgentService,
    cancellation: CancellationToken,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let tls_config = tls_files.server_config()?;
    let peer_identity = PeerIdentityInterceptor::new(cluster_id, WorkloadKind::Controller);

    Server::builder()
        .tls_config(tls_config)?
        .add_service(NodeAgentServer::with_interceptor(service, peer_identity))
        .serve_with_incoming_shutdown(
            TcpListenerStream::new(listener),
            cancellation.cancelled_owned(),
        )
        .await?;

    Ok(())
}
