package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/docker/docker/api/types/swarm"
)

type CreateConfigRequest struct {
	Name   string            `json:"name"`
	Data   string            `json:"data"`
	Labels map[string]string `json:"labels,omitempty"`
}

type CreateConfigAction struct {
	client DockerClient
}

func NewCreateConfigAction(client DockerClient) *CreateConfigAction {
	return &CreateConfigAction{client: client}
}

func (a *CreateConfigAction) Execute(ctx context.Context, req CreateConfigRequest) (string, error) {
	spec := swarm.ConfigSpec{
		Annotations: swarm.Annotations{
			Name:   req.Name,
			Labels: req.Labels,
		},
		Data: []byte(req.Data),
	}

	resp, err := a.client.ConfigCreate(ctx, spec)
	if err != nil {
		return "", err
	}

	return resp.ID, nil
}

type CreateConfigHandler struct {
	action *CreateConfigAction
}

func NewCreateConfigHandler(action *CreateConfigAction) *CreateConfigHandler {
	return &CreateConfigHandler{action: action}
}

func (h *CreateConfigHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	var req CreateConfigRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "invalid request body"})
		return
	}

	if req.Name == "" {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "name is required"})
		return
	}

	id, err := h.action.Execute(r.Context(), req)
	if err != nil {
		log.Printf("create config error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to create config"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusCreated)
	json.NewEncoder(w).Encode(map[string]string{"id": id, "name": req.Name})
}
