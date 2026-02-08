package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

type updateNodeAvailabilityRequest struct {
	Availability string `json:"availability"`
}

type updateNodeAvailabilityResponse struct {
	Availability string `json:"availability"`
}

type UpdateNodeAvailabilityAction struct {
	client DockerClient
}

func NewUpdateNodeAvailabilityAction(client DockerClient) *UpdateNodeAvailabilityAction {
	return &UpdateNodeAvailabilityAction{client: client}
}

func (a *UpdateNodeAvailabilityAction) Execute(ctx context.Context, nodeID string, availability swarm.NodeAvailability) error {
	node, _, err := a.client.NodeInspectWithRaw(ctx, nodeID)
	if err != nil {
		return err
	}

	node.Spec.Availability = availability

	return a.client.NodeUpdate(ctx, nodeID, node.Version, node.Spec)
}

type UpdateNodeAvailabilityHandler struct {
	action *UpdateNodeAvailabilityAction
}

func NewUpdateNodeAvailabilityHandler(action *UpdateNodeAvailabilityAction) *UpdateNodeAvailabilityHandler {
	return &UpdateNodeAvailabilityHandler{action: action}
}

func (h *UpdateNodeAvailabilityHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	nodeID := chi.URLParam(r, "nodeID")

	var req updateNodeAvailabilityRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "invalid request body"})
		return
	}

	avail := swarm.NodeAvailability(req.Availability)
	switch avail {
	case swarm.NodeAvailabilityActive, swarm.NodeAvailabilityDrain, swarm.NodeAvailabilityPause:
		// valid
	default:
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "availability must be active, drain, or pause"})
		return
	}

	if err := h.action.Execute(r.Context(), nodeID, avail); err != nil {
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "node not found"})
			return
		}
		log.Printf("update node availability error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to update node availability"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(updateNodeAvailabilityResponse{Availability: req.Availability})
}
