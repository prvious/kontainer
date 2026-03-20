package routes

import (
	metricsActions "github.com/prvious/kontainer/server/app/actions/metrics"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/grpc/controllers"
	"github.com/prvious/kontainer/server/app/services/docker"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

func Grpc() {
	orchestratorInstance, err := facades.App().Make("docker.orchestrator")
	if err != nil {
		facades.Log().Warningf("gRPC AgentService not registered — Docker orchestrator unavailable: %v", err)
		return
	}
	orchestrator, ok := orchestratorInstance.(docker.Orchestrator)
	if !ok {
		facades.Log().Warningf("gRPC AgentService not registered — docker.orchestrator has unexpected type")
		return
	}

	nodeIDInstance, err := facades.App().Make("docker.node_id")
	if err != nil {
		facades.Log().Warningf("gRPC AgentService not registered — docker.node_id unavailable: %v", err)
		return
	}
	nodeID, ok := nodeIDInstance.(string)
	if !ok {
		facades.Log().Warningf("gRPC AgentService not registered — docker.node_id has unexpected type")
		return
	}

	hostnameInstance, err := facades.App().Make("docker.hostname")
	if err != nil {
		facades.Log().Warningf("gRPC AgentService not registered — docker.hostname unavailable: %v", err)
		return
	}
	hostname, ok := hostnameInstance.(string)
	if !ok {
		facades.Log().Warningf("gRPC AgentService not registered — docker.hostname has unexpected type")
		return
	}

	nodeMetrics := metricsActions.NewCollectNodeMetricsAction()
	agentCtrl := controllers.NewAgentController(nodeID, hostname, orchestrator, nodeMetrics)
	pb.RegisterAgentServiceServer(facades.Grpc().Server(), agentCtrl)
}
