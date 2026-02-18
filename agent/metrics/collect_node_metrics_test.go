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
	action := NewCollectNodeMetricsAction(nil, true)
	_, err := action.Execute(context.Background())
	if err == nil {
		t.Error("expected error with nil docker client in dev mode")
	}
}

func TestCollectNodeMetrics_NonDevMode(t *testing.T) {
	action := NewCollectNodeMetricsAction(nil, false)
	m, err := action.Execute(context.Background())
	if err != nil {
		t.Fatalf("non-dev mode should use /proc, got error: %v", err)
	}
	if m.CPUs <= 0 {
		t.Errorf("expected CPUs > 0, got %d", m.CPUs)
	}
}
