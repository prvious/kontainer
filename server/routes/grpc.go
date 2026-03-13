package routes

import (
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/grpc/controllers"
	"github.com/prvious/kontainer/server/app/services/docker"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

func Grpc() {
	orchestratorInstance, err := facades.App().Make("docker.orchestrator")
	if err != nil {
		facades.Log().Errorf("Failed to resolve docker.orchestrator for gRPC: %v", err)
		return
	}
	orchestrator, ok := orchestratorInstance.(docker.Orchestrator)
	if !ok {
		facades.Log().Errorf("docker.orchestrator is not docker.Orchestrator")
		return
	}

	nodeIDInstance, _ := facades.App().Make("docker.node_id")
	nodeID, _ := nodeIDInstance.(string)

	agentCtrl := controllers.NewAgentController(nodeID, orchestrator)
	pb.RegisterAgentServiceServer(facades.Grpc().Server(), agentCtrl)
}
