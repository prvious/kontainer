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

type updateNodeRoleRequest struct {
	Role string `json:"role"`
}

type updateNodeRoleResponse struct {
	Role string `json:"role"`
}

type UpdateNodeRoleAction struct {
	client DockerClient
}

func NewUpdateNodeRoleAction(client DockerClient) *UpdateNodeRoleAction {
	return &UpdateNodeRoleAction{client: client}
}

func (a *UpdateNodeRoleAction) Execute(ctx context.Context, nodeID string, role swarm.NodeRole) error {
	node, _, err := a.client.NodeInspectWithRaw(ctx, nodeID)
	if err != nil {
		return err
	}

	node.Spec.Role = role

	return a.client.NodeUpdate(ctx, nodeID, node.Version, node.Spec)
}

type UpdateNodeRoleHandler struct {
	action *UpdateNodeRoleAction
}

func NewUpdateNodeRoleHandler(action *UpdateNodeRoleAction) *UpdateNodeRoleHandler {
	return &UpdateNodeRoleHandler{action: action}
}

func (h *UpdateNodeRoleHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	nodeID := chi.URLParam(r, "nodeID")

	var req updateNodeRoleRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "invalid request body"})
		return
	}

	role := swarm.NodeRole(req.Role)
	switch role {
	case swarm.NodeRoleManager, swarm.NodeRoleWorker:
		// valid
	default:
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "role must be manager or worker"})
		return
	}

	if err := h.action.Execute(r.Context(), nodeID, role); err != nil {
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "node not found"})
			return
		}
		log.Printf("update node role error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to update node role"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(updateNodeRoleResponse{Role: req.Role})
}
