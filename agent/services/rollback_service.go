package services

import (
	"context"
	"encoding/json"
	"errors"
	"log"
	"net/http"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

var errNoPreviousSpec = errors.New("service has no previous spec to rollback to")

type RollbackServiceAction struct {
	client DockerClient
}

func NewRollbackServiceAction(client DockerClient) *RollbackServiceAction {
	return &RollbackServiceAction{client: client}
}

func (a *RollbackServiceAction) Execute(ctx context.Context, serviceID string) error {
	svc, _, err := a.client.ServiceInspectWithRaw(ctx, serviceID, types.ServiceInspectOptions{})
	if err != nil {
		return err
	}

	if svc.PreviousSpec == nil {
		return errNoPreviousSpec
	}

	_, err = a.client.ServiceUpdate(ctx, serviceID, svc.Version, svc.Spec, swarm.ServiceUpdateOptions{
		Rollback: "previous",
	})
	return err
}

type RollbackServiceHandler struct {
	action *RollbackServiceAction
}

func NewRollbackServiceHandler(action *RollbackServiceAction) *RollbackServiceHandler {
	return &RollbackServiceHandler{action: action}
}

func (h *RollbackServiceHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	serviceID := chi.URLParam(r, "serviceID")

	err := h.action.Execute(r.Context(), serviceID)
	if err != nil {
		if err == errNoPreviousSpec {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusBadRequest)
			json.NewEncoder(w).Encode(map[string]string{"error": "service has no previous spec to rollback to"})
			return
		}
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "service not found"})
			return
		}
		log.Printf("rollback service error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to rollback service"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{"status": "rolling_back"})
}
