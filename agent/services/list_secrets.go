package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/docker/docker/api/types/swarm"
)

type SecretItem struct {
	ID        string            `json:"id"`
	Name      string            `json:"name"`
	Labels    map[string]string `json:"labels"`
	CreatedAt string            `json:"createdAt"`
	UpdatedAt string            `json:"updatedAt"`
}

type ListSecretsAction struct {
	client DockerClient
}

func NewListSecretsAction(client DockerClient) *ListSecretsAction {
	return &ListSecretsAction{client: client}
}

func (a *ListSecretsAction) Execute(ctx context.Context) ([]SecretItem, error) {
	secrets, err := a.client.SecretList(ctx, swarm.SecretListOptions{})
	if err != nil {
		return nil, err
	}

	items := make([]SecretItem, 0, len(secrets))
	for _, s := range secrets {
		labels := s.Spec.Labels
		if labels == nil {
			labels = map[string]string{}
		}
		items = append(items, SecretItem{
			ID:        s.ID,
			Name:      s.Spec.Name,
			Labels:    labels,
			CreatedAt: s.CreatedAt.UTC().Format("2006-01-02T15:04:05Z"),
			UpdatedAt: s.UpdatedAt.UTC().Format("2006-01-02T15:04:05Z"),
		})
	}

	return items, nil
}

type ListSecretsHandler struct {
	action *ListSecretsAction
}

func NewListSecretsHandler(action *ListSecretsAction) *ListSecretsHandler {
	return &ListSecretsHandler{action: action}
}

func (h *ListSecretsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	items, err := h.action.Execute(r.Context())
	if err != nil {
		log.Printf("list secrets error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to list secrets"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"secrets": items})
}
