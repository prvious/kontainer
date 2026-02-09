package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"
	"sort"

	"github.com/docker/docker/api/types"
)

type NodeItem struct {
	ID            string `json:"id"`
	Hostname      string `json:"hostname"`
	Role          string `json:"role"`
	Status        string `json:"status"`
	Availability  string `json:"availability"`
	EngineVersion string `json:"engineVersion"`
	OS            string `json:"os"`
	Architecture  string `json:"architecture"`
	IP            string `json:"ip"`
	Leader        bool   `json:"leader"`
	UpdatedAt     string `json:"updatedAt"`
}

type ListNodesAction struct {
	client DockerClient
}

func NewListNodesAction(client DockerClient) *ListNodesAction {
	return &ListNodesAction{client: client}
}

func (a *ListNodesAction) Execute(ctx context.Context) ([]NodeItem, error) {
	nodes, err := a.client.NodeList(ctx, types.NodeListOptions{})
	if err != nil {
		return nil, err
	}

	items := make([]NodeItem, 0, len(nodes))
	for _, node := range nodes {
		leader := node.ManagerStatus != nil && node.ManagerStatus.Leader

		items = append(items, NodeItem{
			ID:            node.ID,
			Hostname:      node.Description.Hostname,
			Role:          string(node.Spec.Role),
			Status:        string(node.Status.State),
			Availability:  string(node.Spec.Availability),
			EngineVersion: node.Description.Engine.EngineVersion,
			OS:            node.Description.Platform.OS,
			Architecture:  node.Description.Platform.Architecture,
			IP:            node.Status.Addr,
			Leader:        leader,
			UpdatedAt:     node.UpdatedAt.UTC().Format("2006-01-02T15:04:05Z"),
		})
	}

	// Managers first, then workers. Within each group, leaders before non-leaders.
	sort.Slice(items, func(i, j int) bool {
		if items[i].Role != items[j].Role {
			return items[i].Role == "manager"
		}
		return items[i].Leader && !items[j].Leader
	})

	return items, nil
}

type ListNodesHandler struct {
	action *ListNodesAction
}

func NewListNodesHandler(action *ListNodesAction) *ListNodesHandler {
	return &ListNodesHandler{action: action}
}

func (h *ListNodesHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	items, err := h.action.Execute(r.Context())
	if err != nil {
		log.Printf("list nodes error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to list nodes"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"nodes": items})
}
