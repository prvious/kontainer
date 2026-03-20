package controllers

import (
	"context"
	"time"

	pb "github.com/prvious/kontainer/server/proto/agent"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

func (c *AgentController) Metrics(ctx context.Context, req *pb.MetricsRequest) (*pb.MetricsResponse, error) {
	nodeMetrics, err := c.nodeMetrics.Execute()
	if err != nil {
		return nil, status.Errorf(codes.Internal, "collect node metrics: %v", err)
	}

	containerStats, err := c.orchestrator.TaskStats(ctx)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "collect container stats: %v", err)
	}

	containers := make([]*pb.ContainerMetrics, 0, len(containerStats))
	for _, s := range containerStats {
		containers = append(containers, &pb.ContainerMetrics{
			ContainerId:   s.ContainerID,
			Name:          s.Name,
			ServiceId:     s.ServiceID,
			CpuPercent:    s.CPUPercent,
			MemoryUsage:   s.MemUsage,
			MemoryLimit:   s.MemLimit,
			MemoryPercent: s.MemPercent,
		})
	}

	return &pb.MetricsResponse{
		Timestamp:  time.Now().UTC().Format(time.RFC3339),
		NodeId:     c.nodeID,
		Hostname:   c.hostname,
		Node: &pb.NodeMetrics{
			Cpus:          int32(nodeMetrics.CPUs),
			CpuPercent:    nodeMetrics.CPUPercent,
			MemoryUsed:    int64(nodeMetrics.MemoryUsed),
			MemoryTotal:   int64(nodeMetrics.MemoryTotal),
			MemoryPercent: nodeMetrics.MemoryPercent,
			DiskUsed:      int64(nodeMetrics.DiskUsed),
			DiskTotal:     int64(nodeMetrics.DiskTotal),
			DiskPercent:   nodeMetrics.DiskPercent,
		},
		Containers: containers,
	}, nil
}
