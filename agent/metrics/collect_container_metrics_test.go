package metrics

import (
	"testing"

	"github.com/docker/docker/api/types/container"
)

func TestCalculateCPUPercent(t *testing.T) {
	stats := &container.StatsResponse{}
	stats.CPUStats.CPUUsage.TotalUsage = 200
	stats.PreCPUStats.CPUUsage.TotalUsage = 100
	stats.CPUStats.SystemUsage = 2000
	stats.PreCPUStats.SystemUsage = 1000
	stats.CPUStats.OnlineCPUs = 2

	pct := calculateCPUPercent(stats)
	if pct < 19.9 || pct > 20.1 {
		t.Errorf("expected ~20%%, got %f", pct)
	}
}

func TestCalculateCPUPercent_ZeroDelta(t *testing.T) {
	stats := &container.StatsResponse{}
	pct := calculateCPUPercent(stats)
	if pct != 0 {
		t.Errorf("expected 0%%, got %f", pct)
	}
}
