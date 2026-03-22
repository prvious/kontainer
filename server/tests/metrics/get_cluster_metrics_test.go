package metrics_test

import (
	"context"
	"errors"
	"os"
	"testing"
	"time"

	"github.com/goravel/framework/facades"

	metricsActions "github.com/prvious/kontainer/server/app/actions/metrics"
	"github.com/prvious/kontainer/server/app/models"
	_ "github.com/prvious/kontainer/server/tests"
)

func TestMain(m *testing.M) {
	facades.Artisan().Call("migrate")
	os.Exit(m.Run())
}

func cleanup(t *testing.T) {
	t.Helper()
	if _, err := facades.Orm().Query().Where("1 = 1").Delete(&models.Metric{}); err != nil {
		t.Fatalf("cleanup failed: %v", err)
	}
}

func seedNodeMetric(t *testing.T, nodeID, hostname string, ts time.Time, cpuPct float64) {
	t.Helper()
	m := &models.Metric{
		Timestamp: ts, Kind: models.MetricKindNode,
		NodeID: nodeID, SubjectID: nodeID, SubjectName: hostname,
		CPUPercent: cpuPct, MemUsed: 4_000_000_000, MemTotal: 8_000_000_000, MemPercent: 50.0,
		CPUs: 4, DiskUsed: 50_000_000_000, DiskTotal: 100_000_000_000, DiskPercent: 50.0,
	}
	if err := facades.Orm().Query().Create(m); err != nil {
		t.Fatalf("seedNodeMetric failed: %v", err)
	}
}

func seedContainerMetric(t *testing.T, nodeID, containerID, name, serviceID string, ts time.Time, cpuPct float64) {
	t.Helper()
	m := &models.Metric{
		Timestamp: ts, Kind: models.MetricKindContainer,
		NodeID: nodeID, SubjectID: containerID, SubjectName: name,
		CPUPercent: cpuPct, MemUsed: 100_000_000, MemTotal: 500_000_000, MemPercent: 20.0,
		ServiceID: serviceID,
	}
	if err := facades.Orm().Query().Create(m); err != nil {
		t.Fatalf("seedContainerMetric failed: %v", err)
	}
}

func TestGetClusterMetrics_ReturnsLatestPerNode(t *testing.T) {
	cleanup(t)

	now := time.Now().Truncate(time.Second).UTC()
	old := now.Add(-30 * time.Second)

	// Two snapshots for node-A: old and new
	seedNodeMetric(t, "node-A", "host-a", old, 10.0)
	seedNodeMetric(t, "node-A", "host-a", now, 25.0)

	// One snapshot for node-B
	seedNodeMetric(t, "node-B", "host-b", now, 50.0)

	action := metricsActions.NewGetClusterMetricsAction()
	resp, err := action.Execute(context.Background(), 1*time.Hour)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(resp.Agents) != 2 {
		t.Fatalf("expected 2 agents, got %d", len(resp.Agents))
	}

	// Find node-A and verify it has the latest CPU value
	for _, agent := range resp.Agents {
		if agent.NodeID == "node-A" {
			if agent.Node.CPUPercent != 25.0 {
				t.Errorf("expected node-A cpu 25.0, got %.1f", agent.Node.CPUPercent)
			}
		}
	}
}

func TestGetClusterMetrics_AttachesContainersToNodes(t *testing.T) {
	cleanup(t)

	now := time.Now().Truncate(time.Second).UTC()

	seedNodeMetric(t, "node-A", "host-a", now, 30.0)
	seedContainerMetric(t, "node-A", "ctr-1", "web.1", "svc-web", now, 5.0)
	seedContainerMetric(t, "node-A", "ctr-2", "api.1", "svc-api", now, 8.0)

	action := metricsActions.NewGetClusterMetricsAction()
	resp, err := action.Execute(context.Background(), 1*time.Hour)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(resp.Agents) != 1 {
		t.Fatalf("expected 1 agent, got %d", len(resp.Agents))
	}

	agent := resp.Agents[0]
	if len(agent.Containers) != 2 {
		t.Fatalf("expected 2 containers, got %d", len(agent.Containers))
	}

	// Verify container fields are mapped correctly
	for _, c := range agent.Containers {
		if c.ContainerID != "ctr-1" && c.ContainerID != "ctr-2" {
			t.Errorf("unexpected container ID: %s", c.ContainerID)
		}
		if c.MemoryLimit != 500_000_000 {
			t.Errorf("expected MemoryLimit 500000000, got %d", c.MemoryLimit)
		}
	}
}

func TestGetClusterMetrics_ContainersWithOldTimestampNotAttached(t *testing.T) {
	cleanup(t)

	now := time.Now().Truncate(time.Second).UTC()
	old := now.Add(-30 * time.Second)

	// Node at latest timestamp, container at older timestamp
	seedNodeMetric(t, "node-A", "host-a", now, 30.0)
	seedContainerMetric(t, "node-A", "ctr-old", "stale.1", "svc-stale", old, 1.0)

	action := metricsActions.NewGetClusterMetricsAction()
	resp, err := action.Execute(context.Background(), 1*time.Hour)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(resp.Agents) != 1 {
		t.Fatalf("expected 1 agent, got %d", len(resp.Agents))
	}

	if len(resp.Agents[0].Containers) != 0 {
		t.Errorf("expected 0 containers (old timestamp), got %d", len(resp.Agents[0].Containers))
	}
}

func TestGetClusterMetrics_HistoryReturnsNodeMetricsOnly(t *testing.T) {
	cleanup(t)

	now := time.Now().Truncate(time.Second).UTC()
	t1 := now.Add(-30 * time.Minute)
	t2 := now.Add(-15 * time.Minute)

	seedNodeMetric(t, "node-A", "host-a", t1, 10.0)
	seedNodeMetric(t, "node-A", "host-a", t2, 20.0)
	seedContainerMetric(t, "node-A", "ctr-1", "web.1", "svc-web", t1, 5.0)

	action := metricsActions.NewGetClusterMetricsAction()
	resp, err := action.Execute(context.Background(), 1*time.Hour)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(resp.History) != 2 {
		t.Fatalf("expected 2 history entries, got %d", len(resp.History))
	}

	// History entries should have empty containers
	for _, h := range resp.History {
		if len(h.Containers) != 0 {
			t.Errorf("expected empty containers in history, got %d", len(h.Containers))
		}
	}

	// Verify history is ordered ascending
	if resp.History[0].Node.CPUPercent != 10.0 {
		t.Errorf("expected first history entry cpu 10.0, got %.1f", resp.History[0].Node.CPUPercent)
	}
	if resp.History[1].Node.CPUPercent != 20.0 {
		t.Errorf("expected second history entry cpu 20.0, got %.1f", resp.History[1].Node.CPUPercent)
	}
}

func TestGetClusterMetrics_HistoryRespectsRangeDuration(t *testing.T) {
	cleanup(t)

	now := time.Now().Truncate(time.Second).UTC()
	recent := now.Add(-30 * time.Minute)
	old := now.Add(-3 * time.Hour)

	seedNodeMetric(t, "node-A", "host-a", recent, 20.0)
	seedNodeMetric(t, "node-A", "host-a", old, 10.0)

	action := metricsActions.NewGetClusterMetricsAction()

	// 1h range should only include the recent entry
	resp, err := action.Execute(context.Background(), 1*time.Hour)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(resp.History) != 1 {
		t.Fatalf("expected 1 history entry for 1h range, got %d", len(resp.History))
	}

	// 6h range should include both
	resp, err = action.Execute(context.Background(), 6*time.Hour)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(resp.History) != 2 {
		t.Fatalf("expected 2 history entries for 6h range, got %d", len(resp.History))
	}
}

func TestGetClusterMetrics_EmptyDB(t *testing.T) {
	cleanup(t)

	action := metricsActions.NewGetClusterMetricsAction()
	resp, err := action.Execute(context.Background(), 1*time.Hour)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(resp.Agents) != 0 {
		t.Errorf("expected 0 agents, got %d", len(resp.Agents))
	}
	if len(resp.History) != 0 {
		t.Errorf("expected 0 history, got %d", len(resp.History))
	}
}

func TestGetNodeMetrics_ReturnsLatestSnapshot(t *testing.T) {
	cleanup(t)

	now := time.Now().Truncate(time.Second).UTC()

	seedNodeMetric(t, "node-A", "host-a", now, 42.0)
	seedContainerMetric(t, "node-A", "ctr-1", "web.1", "svc-web", now, 5.0)

	action := metricsActions.NewGetNodeMetricsAction()
	snap, err := action.Execute(context.Background(), "node-A")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if snap.NodeID != "node-A" {
		t.Errorf("expected node-A, got %s", snap.NodeID)
	}
	if snap.Hostname != "host-a" {
		t.Errorf("expected host-a, got %s", snap.Hostname)
	}
	if snap.Node.CPUPercent != 42.0 {
		t.Errorf("expected cpu 42.0, got %.1f", snap.Node.CPUPercent)
	}
	if len(snap.Containers) != 1 {
		t.Fatalf("expected 1 container, got %d", len(snap.Containers))
	}
	if snap.Containers[0].ContainerID != "ctr-1" {
		t.Errorf("expected ctr-1, got %s", snap.Containers[0].ContainerID)
	}
}

func TestGetNodeMetrics_ReturnsErrNoRecentMetrics(t *testing.T) {
	cleanup(t)

	action := metricsActions.NewGetNodeMetricsAction()
	_, err := action.Execute(context.Background(), "nonexistent-node")
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if !errors.Is(err, metricsActions.ErrNoRecentMetrics) {
		t.Errorf("expected ErrNoRecentMetrics, got: %v", err)
	}
}

func TestGetNodeMetrics_IgnoresStaleData(t *testing.T) {
	cleanup(t)

	// Insert a metric older than 2 minutes
	stale := time.Now().Add(-5 * time.Minute).Truncate(time.Second).UTC()
	seedNodeMetric(t, "node-A", "host-a", stale, 10.0)

	action := metricsActions.NewGetNodeMetricsAction()
	_, err := action.Execute(context.Background(), "node-A")
	if !errors.Is(err, metricsActions.ErrNoRecentMetrics) {
		t.Errorf("expected ErrNoRecentMetrics for stale data, got: %v", err)
	}
}

func TestNodeSnapshotFieldMapping(t *testing.T) {
	cleanup(t)

	now := time.Now().Truncate(time.Second).UTC()
	m := &models.Metric{
		Timestamp: now, Kind: models.MetricKindNode,
		NodeID: "n1", SubjectID: "n1", SubjectName: "myhost",
		CPUPercent: 12.5, MemUsed: 2_000_000_000, MemTotal: 8_000_000_000, MemPercent: 25.0,
		CPUs: 8, DiskUsed: 100_000_000_000, DiskTotal: 500_000_000_000, DiskPercent: 20.0,
	}
	if err := facades.Orm().Query().Create(m); err != nil {
		t.Fatalf("seed failed: %v", err)
	}

	action := metricsActions.NewGetNodeMetricsAction()
	snap, err := action.Execute(context.Background(), "n1")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if snap.Node.CPUs != 8 {
		t.Errorf("CPUs: expected 8, got %d", snap.Node.CPUs)
	}
	if snap.Node.MemoryTotal != 8_000_000_000 {
		t.Errorf("MemoryTotal: expected 8000000000, got %d", snap.Node.MemoryTotal)
	}
	if snap.Node.MemoryUsed != 2_000_000_000 {
		t.Errorf("MemoryUsed: expected 2000000000, got %d", snap.Node.MemoryUsed)
	}
	if snap.Node.DiskTotal != 500_000_000_000 {
		t.Errorf("DiskTotal: expected 500000000000, got %d", snap.Node.DiskTotal)
	}
	if snap.Node.DiskPercent != 20.0 {
		t.Errorf("DiskPercent: expected 20.0, got %.1f", snap.Node.DiskPercent)
	}
}
