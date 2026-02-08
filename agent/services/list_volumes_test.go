package services

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/docker/docker/api/types/mount"
	"github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/api/types/volume"
)

func TestListVolumesHandler_WithUsageData(t *testing.T) {
	mock := &mockDockerClient{
		volumes: volume.ListResponse{
			Volumes: []*volume.Volume{
				{
					Name:       "db-data",
					Driver:     "local",
					Scope:      "local",
					Mountpoint: "/var/lib/docker/volumes/db-data/_data",
					Labels: map[string]string{
						"com.docker.stack.namespace": "myapp",
					},
					Options:   map[string]string{},
					CreatedAt: "2025-01-15T10:30:00Z",
					UsageData: &volume.UsageData{
						RefCount: 2,
						Size:     1048576,
					},
				},
				{
					Name:       "cache-vol",
					Driver:     "local",
					Scope:      "local",
					Mountpoint: "/var/lib/docker/volumes/cache-vol/_data",
					Labels:     map[string]string{},
					Options:    map[string]string{},
					CreatedAt:  "2025-02-01T08:00:00Z",
					UsageData:  nil,
				},
			},
		},
		services: []swarm.Service{},
	}

	action := NewListVolumesAction(mock)
	handler := NewListVolumesHandler(action)

	req := httptest.NewRequest("GET", "/api/volumes", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Volumes []VolumeItem `json:"volumes"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Volumes) != 2 {
		t.Fatalf("expected 2 volumes, got %d", len(resp.Volumes))
	}

	db := resp.Volumes[0]
	if db.Name != "db-data" {
		t.Errorf("expected name db-data, got %s", db.Name)
	}
	if db.Driver != "local" {
		t.Errorf("expected driver local, got %s", db.Driver)
	}
	if db.Stack != "myapp" {
		t.Errorf("expected stack myapp, got %s", db.Stack)
	}
	if db.RefCount != 2 {
		t.Errorf("expected ref count 2, got %d", db.RefCount)
	}
	if db.SizeBytes != 1048576 {
		t.Errorf("expected size 1048576, got %d", db.SizeBytes)
	}
	if db.Mountpoint != "/var/lib/docker/volumes/db-data/_data" {
		t.Errorf("expected mountpoint, got %s", db.Mountpoint)
	}

	cache := resp.Volumes[1]
	if cache.RefCount != -1 {
		t.Errorf("expected ref count -1 (no usage data), got %d", cache.RefCount)
	}
	if cache.SizeBytes != -1 {
		t.Errorf("expected size -1 (no usage data), got %d", cache.SizeBytes)
	}
	if cache.Stack != "" {
		t.Errorf("expected empty stack, got %s", cache.Stack)
	}
}

func TestListVolumesHandler_EmptyList(t *testing.T) {
	mock := &mockDockerClient{
		volumes: volume.ListResponse{
			Volumes: []*volume.Volume{},
		},
		services: []swarm.Service{},
	}

	action := NewListVolumesAction(mock)
	handler := NewListVolumesHandler(action)

	req := httptest.NewRequest("GET", "/api/volumes", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Volumes []VolumeItem `json:"volumes"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Volumes) != 0 {
		t.Errorf("expected 0 volumes, got %d", len(resp.Volumes))
	}
}

func TestListVolumesHandler_VolumeListError(t *testing.T) {
	mock := &mockDockerClient{
		volumeErr: errors.New("connection refused"),
	}

	action := NewListVolumesAction(mock)
	handler := NewListVolumesHandler(action)

	req := httptest.NewRequest("GET", "/api/volumes", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to list volumes" {
		t.Errorf("unexpected error message: %s", resp["error"])
	}
}

func TestListVolumesHandler_ServiceListError(t *testing.T) {
	mock := &mockDockerClient{
		volumes: volume.ListResponse{
			Volumes: []*volume.Volume{
				{
					Name:      "test-vol",
					Driver:    "local",
					Scope:     "local",
					CreatedAt: "2025-01-01T00:00:00Z",
				},
			},
		},
		svcErr: errors.New("service list failed"),
	}

	action := NewListVolumesAction(mock)
	handler := NewListVolumesHandler(action)

	req := httptest.NewRequest("GET", "/api/volumes", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

func TestListVolumesHandler_NilLabelsAndOptions(t *testing.T) {
	mock := &mockDockerClient{
		volumes: volume.ListResponse{
			Volumes: []*volume.Volume{
				{
					Name:      "bare-vol",
					Driver:    "local",
					Scope:     "local",
					Labels:    nil,
					Options:   nil,
					CreatedAt: "2025-01-01T00:00:00Z",
				},
			},
		},
		services: []swarm.Service{},
	}

	action := NewListVolumesAction(mock)
	handler := NewListVolumesHandler(action)

	req := httptest.NewRequest("GET", "/api/volumes", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	var resp struct {
		Volumes []VolumeItem `json:"volumes"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Volumes) != 1 {
		t.Fatalf("expected 1 volume, got %d", len(resp.Volumes))
	}
	if resp.Volumes[0].Labels == nil {
		t.Error("expected labels to be non-nil empty map")
	}
	if resp.Volumes[0].Options == nil {
		t.Error("expected options to be non-nil empty map")
	}
}

func TestListVolumesHandler_StackFromLabels(t *testing.T) {
	mock := &mockDockerClient{
		volumes: volume.ListResponse{
			Volumes: []*volume.Volume{
				{
					Name:   "stack-vol",
					Driver: "local",
					Scope:  "local",
					Labels: map[string]string{
						"com.docker.stack.namespace": "production",
					},
					CreatedAt: "2025-06-01T00:00:00Z",
				},
			},
		},
		services: []swarm.Service{
			{
				ID: "svc1",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{Name: "web"},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{
							Image: "nginx",
							Mounts: []mount.Mount{
								{
									Type:   mount.TypeVolume,
									Source: "stack-vol",
									Target: "/data",
								},
							},
						},
					},
				},
			},
		},
	}

	action := NewListVolumesAction(mock)
	handler := NewListVolumesHandler(action)

	req := httptest.NewRequest("GET", "/api/volumes", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	var resp struct {
		Volumes []VolumeItem `json:"volumes"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Volumes) != 1 {
		t.Fatalf("expected 1 volume, got %d", len(resp.Volumes))
	}
	if resp.Volumes[0].Stack != "production" {
		t.Errorf("expected stack production, got %s", resp.Volumes[0].Stack)
	}
}
