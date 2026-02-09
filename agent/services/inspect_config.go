package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

type InspectConfigResponse struct {
	ID        string            `json:"id"`
	Name      string            `json:"name"`
	Labels    map[string]string `json:"labels"`
	Data      string            `json:"data"`
	CreatedAt string            `json:"createdAt"`
	UpdatedAt string            `json:"updatedAt"`
}

type InspectConfigAction struct {
	client DockerClient
}

func NewInspectConfigAction(client DockerClient) *InspectConfigAction {
	return &InspectConfigAction{client: client}
}

func (a *InspectConfigAction) Execute(ctx context.Context, configID string) (*InspectConfigResponse, error) {
	config, _, err := a.client.ConfigInspectWithRaw(ctx, configID)
	if err != nil {
		return nil, err
	}

	labels := config.Spec.Labels
	if labels == nil {
		labels = map[string]string{}
	}

	return &InspectConfigResponse{
		ID:        config.ID,
		Name:      config.Spec.Name,
		Labels:    labels,
		Data:      string(config.Spec.Data),
		CreatedAt: config.CreatedAt.UTC().Format("2006-01-02T15:04:05Z"),
		UpdatedAt: config.UpdatedAt.UTC().Format("2006-01-02T15:04:05Z"),
	}, nil
}

type InspectConfigHandler struct {
	action *InspectConfigAction
}

func NewInspectConfigHandler(action *InspectConfigAction) *InspectConfigHandler {
	return &InspectConfigHandler{action: action}
}

func (h *InspectConfigHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	configID := chi.URLParam(r, "configID")

	resp, err := h.action.Execute(r.Context(), configID)
	if err != nil {
		log.Printf("inspect config error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		if errdefs.IsNotFound(err) {
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "config not found"})
		} else {
			w.WriteHeader(http.StatusInternalServerError)
			json.NewEncoder(w).Encode(map[string]string{"error": "failed to inspect config"})
		}
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}
