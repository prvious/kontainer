package metrics

import (
	"context"
	"testing"
)

func TestCollectFromProc(t *testing.T) {
	m, err := collectFromProc()
	if err != nil {
		t.Fatalf("collectFromProc error: %v", err)
	}

	if m.CPUs <= 0 {
		t.Errorf("expected CPUs > 0, got %d", m.CPUs)
	}
	if m.MemoryTotal == 0 {
		t.Error("expected MemoryTotal > 0")
	}
	if m.DiskTotal == 0 {
		t.Error("expected DiskTotal > 0")
	}
}

func TestCollectNodeMetrics_DevMode(t *testing.T) {
	t.Setenv("KONTAINER_DEV", "true")

	action := NewCollectNodeMetricsAction(nil)
	_, err := action.Execute(context.Background())
	if err == nil {
		t.Error("expected error with nil docker client in dev mode")
	}
}
