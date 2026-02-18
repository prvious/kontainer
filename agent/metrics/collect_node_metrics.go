package metrics

import (
	"context"
	"errors"
	"os"
	"runtime"

	"github.com/docker/docker/api/types/system"
	"github.com/shirou/gopsutil/v4/cpu"
	"github.com/shirou/gopsutil/v4/disk"
	"github.com/shirou/gopsutil/v4/mem"
)

// NodeMetricsDockerClient defines the Docker API subset needed for node metrics in dev mode.
type NodeMetricsDockerClient interface {
	Info(ctx context.Context) (system.Info, error)
}

type CollectNodeMetricsAction struct {
	dockerClient NodeMetricsDockerClient
	devMode      bool
}

func NewCollectNodeMetricsAction(dockerClient NodeMetricsDockerClient, devMode bool) *CollectNodeMetricsAction {
	if _, err := os.Stat("/host/proc"); err == nil {
		os.Setenv("HOST_PROC", "/host/proc")
	}
	return &CollectNodeMetricsAction{dockerClient: dockerClient, devMode: devMode}
}

func (a *CollectNodeMetricsAction) Execute(ctx context.Context) (NodeMetrics, error) {
	if a.devMode {
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
