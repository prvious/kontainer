package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/docker/docker/api/types/swarm"
)

type CreateSecretRequest struct {
	Name   string            `json:"name"`
	Data   string            `json:"data"`
	Labels map[string]string `json:"labels,omitempty"`
}

type CreateSecretAction struct {
	client DockerClient
}

func NewCreateSecretAction(client DockerClient) *CreateSecretAction {
	return &CreateSecretAction{client: client}
}

func (a *CreateSecretAction) Execute(ctx context.Context, req CreateSecretRequest) (string, error) {
	spec := swarm.SecretSpec{
		Annotations: swarm.Annotations{
			Name:   req.Name,
			Labels: req.Labels,
		},
		Data: []byte(req.Data),
	}

	resp, err := a.client.SecretCreate(ctx, spec)
	if err != nil {
		return "", err
	}

	return resp.ID, nil
}

type CreateSecretHandler struct {
	action *CreateSecretAction
}

func NewCreateSecretHandler(action *CreateSecretAction) *CreateSecretHandler {
	return &CreateSecretHandler{action: action}
}

func (h *CreateSecretHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	var req CreateSecretRequest
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
		log.Printf("create secret error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to create secret"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusCreated)
	json.NewEncoder(w).Encode(map[string]string{"id": id, "name": req.Name})
}
