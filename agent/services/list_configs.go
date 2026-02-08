package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/docker/docker/api/types/swarm"
)

type ConfigItem struct {
	ID        string            `json:"id"`
	Name      string            `json:"name"`
	Labels    map[string]string `json:"labels"`
	DataSize  int               `json:"dataSize"`
	CreatedAt string            `json:"createdAt"`
	UpdatedAt string            `json:"updatedAt"`
}

type ListConfigsAction struct {
	client DockerClient
}

func NewListConfigsAction(client DockerClient) *ListConfigsAction {
	return &ListConfigsAction{client: client}
}

func (a *ListConfigsAction) Execute(ctx context.Context) ([]ConfigItem, error) {
	configs, err := a.client.ConfigList(ctx, swarm.ConfigListOptions{})
	if err != nil {
		return nil, err
	}

	items := make([]ConfigItem, 0, len(configs))
	for _, c := range configs {
		labels := c.Spec.Labels
		if labels == nil {
			labels = map[string]string{}
		}
		items = append(items, ConfigItem{
			ID:        c.ID,
			Name:      c.Spec.Name,
			Labels:    labels,
			DataSize:  len(c.Spec.Data),
			CreatedAt: c.CreatedAt.UTC().Format("2006-01-02T15:04:05Z"),
			UpdatedAt: c.UpdatedAt.UTC().Format("2006-01-02T15:04:05Z"),
		})
	}

	return items, nil
}

type ListConfigsHandler struct {
	action *ListConfigsAction
}

func NewListConfigsHandler(action *ListConfigsAction) *ListConfigsHandler {
	return &ListConfigsHandler{action: action}
}

func (h *ListConfigsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	items, err := h.action.Execute(r.Context())
	if err != nil {
		log.Printf("list configs error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to list configs"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"configs": items})
}
