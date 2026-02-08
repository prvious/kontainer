package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

type RemoveConfigAction struct {
	client DockerClient
}

func NewRemoveConfigAction(client DockerClient) *RemoveConfigAction {
	return &RemoveConfigAction{client: client}
}

func (a *RemoveConfigAction) Execute(ctx context.Context, configID string) error {
	return a.client.ConfigRemove(ctx, configID)
}

type RemoveConfigHandler struct {
	action *RemoveConfigAction
}

func NewRemoveConfigHandler(action *RemoveConfigAction) *RemoveConfigHandler {
	return &RemoveConfigHandler{action: action}
}

func (h *RemoveConfigHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	configID := chi.URLParam(r, "configID")

	err := h.action.Execute(r.Context(), configID)
	if err != nil {
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "config not found"})
			return
		}
		log.Printf("remove config error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to remove config"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]bool{"removed": true})
}
