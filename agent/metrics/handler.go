package metrics

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"time"

	"github.com/prvious/kontainer/agent/cluster"
	"github.com/prvious/kontainer/agent/storage"
	"github.com/go-chi/chi/v5"
)

// InternalMetricsHandler serves metrics for this single agent.
// Mounted on /internal/metrics by both follower and leader.
type InternalMetricsHandler struct {
	nodeAction      *CollectNodeMetricsAction
	containerAction *CollectContainerMetricsAction
	nodeID          string
	hostname        string
}

func NewInternalMetricsHandler(
	nodeAction *CollectNodeMetricsAction,
	containerAction *CollectContainerMetricsAction,
	nodeID string,
	hostname string,
) *InternalMetricsHandler {
	return &InternalMetricsHandler{
		nodeAction:      nodeAction,
		containerAction: containerAction,
		nodeID:          nodeID,
		hostname:        hostname,
	}
}

func (h *InternalMetricsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	snapshot, err := h.Collect(r.Context())
	if err != nil {
		log.Printf("internal metrics error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to collect metrics"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(snapshot)
}

func (h *InternalMetricsHandler) Collect(ctx context.Context) (AgentMetricsSnapshot, error) {
	nodeMetrics, err := h.nodeAction.Execute()
	if err != nil {
		return AgentMetricsSnapshot{}, fmt.Errorf("node metrics: %w", err)
	}

	containerMetrics, err := h.containerAction.Execute(ctx)
	if err != nil {
		return AgentMetricsSnapshot{}, fmt.Errorf("container metrics: %w", err)
	}

	return AgentMetricsSnapshot{
		Timestamp:  time.Now().UTC().Format(time.RFC3339),
		NodeID:     h.nodeID,
		Hostname:   h.hostname,
		Node:       nodeMetrics,
		Containers: containerMetrics,
	}, nil
}

// ClusterMetricsHandler aggregates metrics from all agents.
// Leader only, mounted on /api/metrics.
type ClusterMetricsHandler struct {
	local          *InternalMetricsHandler
	registry       *cluster.Registry
	internalSecret string
	store          *storage.MetricsStore
}

func NewClusterMetricsHandler(
	local *InternalMetricsHandler,
	registry *cluster.Registry,
	internalSecret string,
	store *storage.MetricsStore,
) *ClusterMetricsHandler {
	return &ClusterMetricsHandler{
		local:          local,
		registry:       registry,
		internalSecret: internalSecret,
		store:          store,
	}
}

func (h *ClusterMetricsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	localSnapshot, err := h.local.Collect(r.Context())
	if err != nil {
		log.Printf("cluster metrics (local) error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to collect local metrics"})
		return
	}

	agents := []AgentMetricsSnapshot{localSnapshot}

	for _, follower := range h.registry.All() {
		snapshot, err := fetchFollowerMetrics(r.Context(), follower.Address, h.internalSecret)
		if err != nil {
			log.Printf("cluster metrics: failed to fetch from %s (%s): %v", follower.NodeID, follower.Address, err)
			continue
		}
		agents = append(agents, snapshot)
	}

	// Parse range query param (default 1h, allowed: 1h, 6h, 24h)
	rangeDuration := 1 * time.Hour
	switch r.URL.Query().Get("range") {
	case "6h":
		rangeDuration = 6 * time.Hour
	case "24h":
		rangeDuration = 24 * time.Hour
	}

	since := time.Now().Add(-rangeDuration)
	rows, err := h.store.Query(since, "")
	if err != nil {
		log.Printf("cluster metrics: history query error: %v", err)
	}

	history := make([]AgentMetricsSnapshot, 0, len(rows))
	for _, row := range rows {
		history = append(history, rowToSnapshot(row))
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(ClusterMetricsResponse{
		Agents:  agents,
		History: history,
	})
}

// NodeMetricsHandler proxies to a specific node's metrics.
// Leader only, mounted on /api/metrics/{nodeID}.
type NodeMetricsHandler struct {
	local          *InternalMetricsHandler
	registry       *cluster.Registry
	internalSecret string
}

func NewNodeMetricsHandler(
	local *InternalMetricsHandler,
	registry *cluster.Registry,
	internalSecret string,
) *NodeMetricsHandler {
	return &NodeMetricsHandler{
		local:          local,
		registry:       registry,
		internalSecret: internalSecret,
	}
}

func (h *NodeMetricsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	nodeID := chi.URLParam(r, "nodeID")

	if nodeID == h.local.nodeID {
		h.local.ServeHTTP(w, r)
		return
	}

	addr, ok := h.registry.ByNodeID(nodeID)
	if !ok {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusNotFound)
		json.NewEncoder(w).Encode(map[string]string{"error": "node not found"})
		return
	}

	snapshot, err := fetchFollowerMetrics(r.Context(), addr, h.internalSecret)
	if err != nil {
		log.Printf("node metrics: failed to fetch from %s (%s): %v", nodeID, addr, err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadGateway)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to fetch node metrics"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(snapshot)
}

func rowToSnapshot(r storage.MetricsRow) AgentMetricsSnapshot {
	return AgentMetricsSnapshot{
		Timestamp: r.Timestamp,
		NodeID:    r.NodeID,
		Hostname:  r.Hostname,
		Node: NodeMetrics{
			CPUs:          r.CPUs,
			CPUPercent:    r.CPUPercent,
			MemoryUsed:    r.MemUsed,
			MemoryTotal:   r.MemTotal,
			MemoryPercent: r.MemPercent,
			DiskUsed:      r.DiskUsed,
			DiskTotal:     r.DiskTotal,
			DiskPercent:   r.DiskPercent,
		},
		Containers: []ContainerMetrics{},
	}
}

var metricsHTTPClient = &http.Client{Timeout: 5 * time.Second}

func fetchFollowerMetrics(ctx context.Context, addr string, secret string) (AgentMetricsSnapshot, error) {
	url := fmt.Sprintf("http://%s/internal/metrics", addr)

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return AgentMetricsSnapshot{}, fmt.Errorf("create request: %w", err)
	}
	req.Header.Set(cluster.InternalSecretHeader, secret)

	resp, err := metricsHTTPClient.Do(req)
	if err != nil {
		return AgentMetricsSnapshot{}, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, 4096))
		return AgentMetricsSnapshot{}, fmt.Errorf("unexpected status %d: %s", resp.StatusCode, string(body))
	}

	var snapshot AgentMetricsSnapshot
	if err := json.NewDecoder(resp.Body).Decode(&snapshot); err != nil {
		return AgentMetricsSnapshot{}, fmt.Errorf("decode response: %w", err)
	}

	return snapshot, nil
}
