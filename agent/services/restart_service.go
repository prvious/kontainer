package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

type RestartServiceAction struct {
	client DockerClient
}

func NewRestartServiceAction(client DockerClient) *RestartServiceAction {
	return &RestartServiceAction{client: client}
}

func (a *RestartServiceAction) Execute(ctx context.Context, serviceID string) error {
	svc, _, err := a.client.ServiceInspectWithRaw(ctx, serviceID, types.ServiceInspectOptions{})
	if err != nil {
		return err
	}

	svc.Spec.TaskTemplate.ForceUpdate++

	_, err = a.client.ServiceUpdate(ctx, serviceID, svc.Version, svc.Spec, swarm.ServiceUpdateOptions{})
	return err
}

type RestartServiceHandler struct {
	action *RestartServiceAction
}

func NewRestartServiceHandler(action *RestartServiceAction) *RestartServiceHandler {
	return &RestartServiceHandler{action: action}
}

func (h *RestartServiceHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	serviceID := chi.URLParam(r, "serviceID")

	err := h.action.Execute(r.Context(), serviceID)
	if err != nil {
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "service not found"})
			return
		}
		log.Printf("restart service error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to restart service"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{"status": "restarting"})
}
