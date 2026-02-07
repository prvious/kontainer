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

var errGlobalMode = errors.New("cannot scale a global service")

type scaleRequest struct {
	Replicas *uint64 `json:"replicas"`
}

type scaleResponse struct {
	Replicas uint64 `json:"replicas"`
}

type ScaleServiceAction struct {
	client DockerClient
}

func NewScaleServiceAction(client DockerClient) *ScaleServiceAction {
	return &ScaleServiceAction{client: client}
}

func (a *ScaleServiceAction) Execute(ctx context.Context, serviceID string, replicas uint64) error {
	svc, _, err := a.client.ServiceInspectWithRaw(ctx, serviceID, types.ServiceInspectOptions{})
	if err != nil {
		return err
	}

	if svc.Spec.Mode.Global != nil {
		return errGlobalMode
	}

	svc.Spec.Mode.Replicated.Replicas = &replicas

	_, err = a.client.ServiceUpdate(ctx, serviceID, svc.Version, svc.Spec, swarm.ServiceUpdateOptions{})
	return err
}

type ScaleServiceHandler struct {
	action *ScaleServiceAction
}

func NewScaleServiceHandler(action *ScaleServiceAction) *ScaleServiceHandler {
	return &ScaleServiceHandler{action: action}
}

func (h *ScaleServiceHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	serviceID := chi.URLParam(r, "serviceID")

	var req scaleRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "invalid request body"})
		return
	}

	if req.Replicas == nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "replicas field is required"})
		return
	}

	err := h.action.Execute(r.Context(), serviceID, *req.Replicas)
	if err != nil {
		if err == errGlobalMode {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusBadRequest)
			json.NewEncoder(w).Encode(map[string]string{"error": "cannot scale a global service"})
			return
		}
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "service not found"})
			return
		}
		log.Printf("scale service error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to scale service"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(scaleResponse{Replicas: *req.Replicas})
}
