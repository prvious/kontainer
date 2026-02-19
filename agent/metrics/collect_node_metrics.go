package metrics

import (
	"os"
	"runtime"

	"github.com/shirou/gopsutil/v4/cpu"
	"github.com/shirou/gopsutil/v4/disk"
	"github.com/shirou/gopsutil/v4/mem"
)

type CollectNodeMetricsAction struct{}

func NewCollectNodeMetricsAction() *CollectNodeMetricsAction {
	if _, err := os.Stat("/host/proc"); err == nil {
		os.Setenv("HOST_PROC", "/host/proc")
	}
	return &CollectNodeMetricsAction{}
}

func (a *CollectNodeMetricsAction) Execute() (NodeMetrics, error) {
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
