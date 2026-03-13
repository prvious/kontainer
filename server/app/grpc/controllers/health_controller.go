package controllers

import (
	"context"
	"os"

	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

type AgentController struct {
	pb.UnimplementedAgentServiceServer
	nodeID       string
	hostname     string
	orchestrator docker.Orchestrator
}

func NewAgentController(nodeID string, orchestrator docker.Orchestrator) *AgentController {
	hostname, _ := os.Hostname() // Cached once at startup; never changes during process lifetime
	return &AgentController{nodeID: nodeID, hostname: hostname, orchestrator: orchestrator}
}

func (c *AgentController) Health(ctx context.Context, req *pb.HealthRequest) (*pb.HealthResponse, error) {
	return &pb.HealthResponse{
		Status:   "ok",
		NodeId:   c.nodeID,
		Hostname: c.hostname,
	}, nil
}

func (c *AgentController) Info(ctx context.Context, req *pb.InfoRequest) (*pb.InfoResponse, error) {
	role := facades.Config().GetString("cluster.role", "follower")
	return &pb.InfoResponse{
		NodeId:   c.nodeID,
		Hostname: c.hostname,
		Role:     role,
	}, nil
}
