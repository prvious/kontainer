package metrics

import (
	"context"
	"errors"
	"os"
	"runtime"

	"github.com/docker/docker/client"
	"github.com/shirou/gopsutil/v4/cpu"
	"github.com/shirou/gopsutil/v4/disk"
	"github.com/shirou/gopsutil/v4/mem"
)

type CollectNodeMetricsAction struct {
	dockerClient *client.Client
}

func NewCollectNodeMetricsAction(dockerClient *client.Client) *CollectNodeMetricsAction {
	return &CollectNodeMetricsAction{dockerClient: dockerClient}
}

func (a *CollectNodeMetricsAction) Execute(ctx context.Context) (NodeMetrics, error) {
	if os.Getenv("DEVOPER_DEV") == "true" {
		return a.collectFromDocker(ctx)
	}
	return collectFromProc()
}

func (a *CollectNodeMetricsAction) collectFromDocker(ctx context.Context) (NodeMetrics, error) {
	if a.dockerClient == nil {
		return NodeMetrics{}, errors.New("docker client is nil")
	}

	info, err := a.dockerClient.Info(ctx)
	if err != nil {
		return NodeMetrics{}, err
	}

	return NodeMetrics{
		CPUs:        info.NCPU,
		MemoryTotal: uint64(info.MemTotal),
	}, nil
}

func collectFromProc() (NodeMetrics, error) {
	if _, err := os.Stat("/host/proc"); err == nil {
		os.Setenv("HOST_PROC", "/host/proc")
	}

	v, err := mem.VirtualMemory()
	if err != nil {
		return NodeMetrics{}, err
	}

	cpuPercent, err := cpu.Percent(0, false)
	if err != nil {
		return NodeMetrics{}, err
	}

	d, err := disk.Usage("/")
	if err != nil {
		return NodeMetrics{}, err
	}

	cpuPct := 0.0
	if len(cpuPercent) > 0 {
		cpuPct = cpuPercent[0]
	}

	return NodeMetrics{
		CPUs:          runtime.NumCPU(),
		MemoryTotal:   v.Total,
		MemoryUsed:    v.Used,
		MemoryPercent: v.UsedPercent,
		CPUPercent:    cpuPct,
		DiskTotal:     d.Total,
		DiskUsed:      d.Used,
		DiskPercent:   d.UsedPercent,
	}, nil
}
