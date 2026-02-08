package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/volume"
)

type VolumeItem struct {
	Name       string            `json:"name"`
	Driver     string            `json:"driver"`
	Scope      string            `json:"scope"`
	Mountpoint string            `json:"mountpoint"`
	Labels     map[string]string `json:"labels"`
	Options    map[string]string `json:"options"`
	CreatedAt  string            `json:"createdAt"`
	RefCount   int64             `json:"refCount"`
	SizeBytes  int64             `json:"sizeBytes"`
	Stack      string            `json:"stack"`
}

type ListVolumesAction struct {
	client DockerClient
}

func NewListVolumesAction(client DockerClient) *ListVolumesAction {
	return &ListVolumesAction{client: client}
}

func (a *ListVolumesAction) Execute(ctx context.Context) ([]VolumeItem, error) {
	resp, err := a.client.VolumeList(ctx, volume.ListOptions{})
	if err != nil {
		return nil, err
	}

	// Fetch services to cross-reference which services use each volume
	svcs, err := a.client.ServiceList(ctx, types.ServiceListOptions{})
	if err != nil {
		return nil, err
	}

	// Build set of volume names referenced by services (via mount specs)
	volumeServices := make(map[string]int)
	for _, svc := range svcs {
		if svc.Spec.TaskTemplate.ContainerSpec == nil {
			continue
		}
		for _, m := range svc.Spec.TaskTemplate.ContainerSpec.Mounts {
			if m.Type == "volume" {
				volumeServices[m.Source]++
			}
		}
	}

	items := make([]VolumeItem, 0, len(resp.Volumes))
	for _, vol := range resp.Volumes {
		if vol == nil {
			continue
		}

		var refCount int64 = -1
		var sizeBytes int64 = -1
		if vol.UsageData != nil {
			refCount = vol.UsageData.RefCount
			sizeBytes = vol.UsageData.Size
		}

		labels := vol.Labels
		if labels == nil {
			labels = map[string]string{}
		}

		options := vol.Options
		if options == nil {
			options = map[string]string{}
		}

		stack := labels["com.docker.stack.namespace"]

		items = append(items, VolumeItem{
			Name:       vol.Name,
			Driver:     vol.Driver,
			Scope:      vol.Scope,
			Mountpoint: vol.Mountpoint,
			Labels:     labels,
			Options:    options,
			CreatedAt:  vol.CreatedAt,
			RefCount:   refCount,
			SizeBytes:  sizeBytes,
			Stack:      stack,
		})
	}

	return items, nil
}

type ListVolumesHandler struct {
	action *ListVolumesAction
}

func NewListVolumesHandler(action *ListVolumesAction) *ListVolumesHandler {
	return &ListVolumesHandler{action: action}
}

func (h *ListVolumesHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	items, err := h.action.Execute(r.Context())
	if err != nil {
		log.Printf("list volumes error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to list volumes"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"volumes": items})
}
