package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"
	"strings"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

type updateImageRequest struct {
	Image string `json:"image"`
}

type updateImageResponse struct {
	Image string `json:"image"`
}

type UpdateServiceImageAction struct {
	client DockerClient
}

func NewUpdateServiceImageAction(client DockerClient) *UpdateServiceImageAction {
	return &UpdateServiceImageAction{client: client}
}

func (a *UpdateServiceImageAction) Execute(ctx context.Context, serviceID string, image string) (string, error) {
	svc, _, err := a.client.ServiceInspectWithRaw(ctx, serviceID, types.ServiceInspectOptions{})
	if err != nil {
		return "", err
	}

	svc.Spec.TaskTemplate.ContainerSpec.Image = image

	_, err = a.client.ServiceUpdate(ctx, serviceID, svc.Version, svc.Spec, swarm.ServiceUpdateOptions{})
	if err != nil {
		return "", err
	}

	return image, nil
}

type UpdateServiceImageHandler struct {
	action *UpdateServiceImageAction
}

func NewUpdateServiceImageHandler(action *UpdateServiceImageAction) *UpdateServiceImageHandler {
	return &UpdateServiceImageHandler{action: action}
}

func (h *UpdateServiceImageHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	serviceID := chi.URLParam(r, "serviceID")

	var req updateImageRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "invalid request body"})
		return
	}

	if strings.TrimSpace(req.Image) == "" {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "image field is required"})
		return
	}

	result, err := h.action.Execute(r.Context(), serviceID, req.Image)
	if err != nil {
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "service not found"})
			return
		}
		log.Printf("update service image error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to update service image"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(updateImageResponse{Image: result})
}
