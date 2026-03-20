package routes

import (
	metricsActions "github.com/prvious/kontainer/server/app/actions/metrics"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/grpc/controllers"
	"github.com/prvious/kontainer/server/app/services/docker"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

func Grpc() {
	orchestrator, ok := resolve[docker.Orchestrator]("docker.orchestrator")
	if !ok {
		facades.Log().Warningf("gRPC AgentService not registered — Docker orchestrator unavailable")
		return
	}

	nodeID, ok := resolve[string]("docker.node_id")
	if !ok {
		facades.Log().Warningf("gRPC AgentService not registered — docker.node_id unavailable")
		return
	}

	hostname, ok := resolve[string]("docker.hostname")
	if !ok {
		facades.Log().Warningf("gRPC AgentService not registered — docker.hostname unavailable")
		return
	}

	nodeMetrics := metricsActions.NewCollectNodeMetricsAction()
	agentCtrl := controllers.NewAgentController(nodeID, hostname, orchestrator, nodeMetrics)
	pb.RegisterAgentServiceServer(facades.Grpc().Server(), agentCtrl)
}
