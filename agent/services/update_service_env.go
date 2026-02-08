package services

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net/http"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

type updateEnvRequest struct {
	Env []EnvVar `json:"env"`
}

type updateEnvResponse struct {
	Env []EnvVar `json:"env"`
}

type UpdateServiceEnvAction struct {
	client DockerClient
}

func NewUpdateServiceEnvAction(client DockerClient) *UpdateServiceEnvAction {
	return &UpdateServiceEnvAction{client: client}
}

func (a *UpdateServiceEnvAction) Execute(ctx context.Context, serviceID string, envVars []EnvVar) ([]EnvVar, error) {
	svc, _, err := a.client.ServiceInspectWithRaw(ctx, serviceID, types.ServiceInspectOptions{})
	if err != nil {
		return nil, err
	}

	// Build the new env slice in KEY=VALUE format
	newEnv := make([]string, 0, len(envVars))
	for _, ev := range envVars {
		newEnv = append(newEnv, fmt.Sprintf("%s=%s", ev.Key, ev.Value))
	}

	svc.Spec.TaskTemplate.ContainerSpec.Env = newEnv

	_, err = a.client.ServiceUpdate(ctx, serviceID, svc.Version, svc.Spec, swarm.ServiceUpdateOptions{})
	if err != nil {
		return nil, err
	}

	return envVars, nil
}

type UpdateServiceEnvHandler struct {
	action *UpdateServiceEnvAction
}

func NewUpdateServiceEnvHandler(action *UpdateServiceEnvAction) *UpdateServiceEnvHandler {
	return &UpdateServiceEnvHandler{action: action}
}

func (h *UpdateServiceEnvHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	serviceID := chi.URLParam(r, "serviceID")

	var req updateEnvRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "invalid request body"})
		return
	}

	if req.Env == nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "env field is required"})
		return
	}

	// Validate: no empty keys
	for _, ev := range req.Env {
		if ev.Key == "" {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusBadRequest)
			json.NewEncoder(w).Encode(map[string]string{"error": "env var key cannot be empty"})
			return
		}
	}

	result, err := h.action.Execute(r.Context(), serviceID, req.Env)
	if err != nil {
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "service not found"})
			return
		}
		log.Printf("update service env error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to update environment variables"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(updateEnvResponse{Env: result})
}
