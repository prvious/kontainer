package controllers

import (
	"context"
	"os"
	"runtime"
	"time"

	"github.com/shirou/gopsutil/v4/cpu"
	"github.com/shirou/gopsutil/v4/disk"
	"github.com/shirou/gopsutil/v4/mem"

	pb "github.com/prvious/kontainer/server/proto/agent"
)

func (c *AgentController) Metrics(ctx context.Context, req *pb.MetricsRequest) (*pb.MetricsResponse, error) {
	hostname, _ := os.Hostname()

	// Collect node metrics
	nodeMetrics, err := collectNodeMetrics()
	if err != nil {
		return nil, err
	}

	// Collect container metrics via orchestrator
	containerStats, err := c.orchestrator.TaskStats(ctx)
	if err != nil {
		return nil, err
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
		Timestamp: time.Now().UTC().Format(time.RFC3339),
		NodeId:    c.nodeID,
		Hostname:  hostname,
		Node:      nodeMetrics,
		Containers: containers,
	}, nil
}

func collectNodeMetrics() (*pb.NodeMetrics, error) {
	if _, err := os.Stat("/host/proc"); err == nil {
		os.Setenv("HOST_PROC", "/host/proc")
	}

	v, err := mem.VirtualMemory()
	if err != nil {
		return nil, err
	}

	cpuPercent, err := cpu.Percent(0, false)
	if err != nil {
		return nil, err
	}

	d, err := disk.Usage("/")
	if err != nil {
		return nil, err
	}

	cpuPct := 0.0
	if len(cpuPercent) > 0 {
		cpuPct = cpuPercent[0]
	}

	return &pb.NodeMetrics{
		Cpus:          int32(runtime.NumCPU()),
		CpuPercent:    cpuPct,
		MemoryUsed:    int64(v.Used),
		MemoryTotal:   int64(v.Total),
		MemoryPercent: v.UsedPercent,
		DiskUsed:      int64(d.Used),
		DiskTotal:     int64(d.Total),
		DiskPercent:   d.UsedPercent,
	}, nil
}
