package metrics

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"time"

	"github.com/clovismuneza/devoper/agent/cluster"
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
	snapshot, err := h.collect(r.Context())
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

func (h *InternalMetricsHandler) collect(ctx context.Context) (AgentMetricsSnapshot, error) {
	nodeMetrics, err := h.nodeAction.Execute(ctx)
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
}

func NewClusterMetricsHandler(
	local *InternalMetricsHandler,
	registry *cluster.Registry,
	internalSecret string,
) *ClusterMetricsHandler {
	return &ClusterMetricsHandler{
		local:          local,
		registry:       registry,
		internalSecret: internalSecret,
	}
}

func (h *ClusterMetricsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	localSnapshot, err := h.local.collect(r.Context())
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

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(ClusterMetricsResponse{
		Agents: agents,
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

func fetchFollowerMetrics(ctx context.Context, addr string, secret string) (AgentMetricsSnapshot, error) {
	url := fmt.Sprintf("http://%s/internal/metrics", addr)

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return AgentMetricsSnapshot{}, fmt.Errorf("create request: %w", err)
	}
	req.Header.Set(cluster.InternalSecretHeader, secret)

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return AgentMetricsSnapshot{}, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return AgentMetricsSnapshot{}, fmt.Errorf("unexpected status %d: %s", resp.StatusCode, string(body))
	}

	var snapshot AgentMetricsSnapshot
	if err := json.NewDecoder(resp.Body).Decode(&snapshot); err != nil {
		return AgentMetricsSnapshot{}, fmt.Errorf("decode response: %w", err)
	}

	return snapshot, nil
}
