package metrics

import (
	"os"
	"runtime"

	"github.com/shirou/gopsutil/v4/cpu"
	"github.com/shirou/gopsutil/v4/disk"
	"github.com/shirou/gopsutil/v4/mem"
)

type NodeMetrics struct {
	CPUs          int     `json:"cpus"`
	CPUPercent    float64 `json:"cpuPercent"`
	MemoryTotal   uint64  `json:"memoryTotal"`
	MemoryUsed    uint64  `json:"memoryUsed"`
	MemoryPercent float64 `json:"memoryPercent"`
	DiskTotal     uint64  `json:"diskTotal"`
	DiskUsed      uint64  `json:"diskUsed"`
	DiskPercent   float64 `json:"diskPercent"`
}

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
		CPUPercent:    cpuPct,
		MemoryTotal:   v.Total,
		MemoryUsed:    v.Used,
		MemoryPercent: v.UsedPercent,
		DiskTotal:     d.Total,
		DiskUsed:      d.Used,
		DiskPercent:   d.UsedPercent,
	}, nil
}
