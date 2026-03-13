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
	orchestrator docker.Orchestrator
}

func NewAgentController(nodeID string, orchestrator docker.Orchestrator) *AgentController {
	return &AgentController{nodeID: nodeID, orchestrator: orchestrator}
}

func (c *AgentController) Health(ctx context.Context, req *pb.HealthRequest) (*pb.HealthResponse, error) {
	hostname, _ := os.Hostname()
	return &pb.HealthResponse{
		Status:   "ok",
		NodeId:   c.nodeID,
		Hostname: hostname,
	}, nil
}

func (c *AgentController) Info(ctx context.Context, req *pb.InfoRequest) (*pb.InfoResponse, error) {
	hostname, _ := os.Hostname()
	role := facades.Config().GetString("cluster.role", "follower")
	return &pb.InfoResponse{
		NodeId:   c.nodeID,
		Hostname: hostname,
		Role:     role,
	}, nil
}
