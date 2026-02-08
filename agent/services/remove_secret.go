package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

type RemoveSecretAction struct {
	client DockerClient
}

func NewRemoveSecretAction(client DockerClient) *RemoveSecretAction {
	return &RemoveSecretAction{client: client}
}

func (a *RemoveSecretAction) Execute(ctx context.Context, secretID string) error {
	return a.client.SecretRemove(ctx, secretID)
}

type RemoveSecretHandler struct {
	action *RemoveSecretAction
}

func NewRemoveSecretHandler(action *RemoveSecretAction) *RemoveSecretHandler {
	return &RemoveSecretHandler{action: action}
}

func (h *RemoveSecretHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	secretID := chi.URLParam(r, "secretID")

	err := h.action.Execute(r.Context(), secretID)
	if err != nil {
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "secret not found"})
			return
		}
		log.Printf("remove secret error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to remove secret"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]bool{"removed": true})
}
