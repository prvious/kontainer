package metrics

import (
	"testing"
)

func TestCollectNodeMetrics(t *testing.T) {
	action := NewCollectNodeMetricsAction()
	m, err := action.Execute()
	if err != nil {
		t.Fatalf("Execute error: %v", err)
	}

	if m.CPUs <= 0 {
		t.Errorf("expected CPUs > 0, got %d", m.CPUs)
	}
	if m.MemoryTotal == 0 {
		t.Error("expected MemoryTotal > 0")
	}
	if m.MemoryUsed == 0 {
		t.Error("expected MemoryUsed > 0")
	}
	if m.DiskTotal == 0 {
		t.Error("expected DiskTotal > 0")
	}
}
