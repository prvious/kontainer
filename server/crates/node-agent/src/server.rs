use std::{error::Error, future::Future};

use kontainer_agent_proto::v1::node_agent_server::NodeAgentServer;
use kontainer_rpc_auth::{MutualTlsIdentityFiles, PeerIdentityInterceptor, WorkloadKind};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;

use crate::NodeAgentService;

/// Validate TLS eagerly; polling the returned future begins RPC admission.
pub fn serve_node_agent_rpc(
    listener: TcpListener,
    cluster_id: String,
    tls_files: MutualTlsIdentityFiles,
    service: NodeAgentService,
    cancellation: CancellationToken,
) -> Result<
    impl Future<Output = Result<(), tonic::transport::Error>> + Send,
    Box<dyn Error + Send + Sync>,
> {
    let peer_identity = PeerIdentityInterceptor::new(cluster_id, WorkloadKind::Controller)?;
    let tls_config = tls_files.server_config()?;

    Ok(Server::builder()
        .tls_config(tls_config)?
        .add_service(NodeAgentServer::with_interceptor(service, peer_identity))
        .serve_with_incoming_shutdown(
            TcpListenerStream::new(listener),
            cancellation.cancelled_owned(),
        ))
}
