package services

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/docker/docker/api/types/network"
	"github.com/docker/docker/api/types/swarm"
)

func TestListNetworksHandler_OverlayWithServices(t *testing.T) {
	now := time.Now().UTC()
	mock := &mockDockerClient{
		networks: []network.Summary{
			{
				ID:         "net1",
				Name:       "my-overlay",
				Driver:     "overlay",
				Scope:      "swarm",
				Internal:   false,
				Attachable: true,
				Ingress:    false,
				Created:    now,
			},
			{
				ID:         "net-ingress",
				Name:       "ingress",
				Driver:     "overlay",
				Scope:      "swarm",
				Internal:   false,
				Attachable: false,
				Ingress:    true,
				Created:    now,
			},
		},
		services: []swarm.Service{
			{
				ID: "svc1",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{Name: "web"},
					Mode:        swarm.ServiceMode{Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(1)}},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{Image: "nginx"},
						Networks: []swarm.NetworkAttachmentConfig{
							{Target: "net1"},
						},
					},
				},
				Meta: swarm.Meta{UpdatedAt: now},
			},
			{
				ID: "svc2",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{Name: "api"},
					Mode:        swarm.ServiceMode{Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(2)}},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{Image: "myapi"},
						Networks: []swarm.NetworkAttachmentConfig{
							{Target: "net1"},
						},
					},
				},
				Meta: swarm.Meta{UpdatedAt: now},
			},
		},
	}

	action := NewListNetworksAction(mock)
	handler := NewListNetworksHandler(action)

	req := httptest.NewRequest("GET", "/api/networks", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Networks []NetworkItem `json:"networks"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Networks) != 2 {
		t.Fatalf("expected 2 networks, got %d", len(resp.Networks))
	}

	overlay := resp.Networks[0]
	if overlay.ID != "net1" {
		t.Errorf("expected id net1, got %s", overlay.ID)
	}
	if overlay.Name != "my-overlay" {
		t.Errorf("expected name my-overlay, got %s", overlay.Name)
	}
	if overlay.Driver != "overlay" {
		t.Errorf("expected driver overlay, got %s", overlay.Driver)
	}
	if overlay.Scope != "swarm" {
		t.Errorf("expected scope swarm, got %s", overlay.Scope)
	}
	if !overlay.Attachable {
		t.Error("expected attachable to be true")
	}
	if overlay.Ingress {
		t.Error("expected ingress to be false")
	}
	if len(overlay.Services) != 2 {
		t.Fatalf("expected 2 attached services, got %d", len(overlay.Services))
	}
	if overlay.Services[0].Name != "web" {
		t.Errorf("expected first service name web, got %s", overlay.Services[0].Name)
	}
	if overlay.Services[1].Name != "api" {
		t.Errorf("expected second service name api, got %s", overlay.Services[1].Name)
	}

	ingress := resp.Networks[1]
	if !ingress.Ingress {
		t.Error("expected ingress to be true")
	}
	if len(ingress.Services) != 0 {
		t.Errorf("expected 0 services on ingress, got %d", len(ingress.Services))
	}
}

func TestListNetworksHandler_NoServicesAttached(t *testing.T) {
	mock := &mockDockerClient{
		networks: []network.Summary{
			{
				ID:      "net-empty",
				Name:    "isolated",
				Driver:  "overlay",
				Scope:   "swarm",
				Created: time.Now().UTC(),
			},
		},
		services: []swarm.Service{},
	}

	action := NewListNetworksAction(mock)
	handler := NewListNetworksHandler(action)

	req := httptest.NewRequest("GET", "/api/networks", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	var resp struct {
		Networks []NetworkItem `json:"networks"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Networks) != 1 {
		t.Fatalf("expected 1 network, got %d", len(resp.Networks))
	}
	if len(resp.Networks[0].Services) != 0 {
		t.Errorf("expected 0 services, got %d", len(resp.Networks[0].Services))
	}
}

func TestListNetworksHandler_EmptyList(t *testing.T) {
	mock := &mockDockerClient{
		networks: []network.Summary{},
		services: []swarm.Service{},
	}

	action := NewListNetworksAction(mock)
	handler := NewListNetworksHandler(action)

	req := httptest.NewRequest("GET", "/api/networks", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Networks []NetworkItem `json:"networks"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Networks) != 0 {
		t.Errorf("expected 0 networks, got %d", len(resp.Networks))
	}
}

func TestListNetworksHandler_NetworkListError(t *testing.T) {
	mock := &mockDockerClient{
		networkErr: errors.New("connection refused"),
	}

	action := NewListNetworksAction(mock)
	handler := NewListNetworksHandler(action)

	req := httptest.NewRequest("GET", "/api/networks", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to list networks" {
		t.Errorf("unexpected error message: %s", resp["error"])
	}
}

func TestListNetworksHandler_ServiceListError(t *testing.T) {
	mock := &mockDockerClient{
		networks: []network.Summary{
			{
				ID:      "net1",
				Name:    "overlay1",
				Driver:  "overlay",
				Scope:   "swarm",
				Created: time.Now().UTC(),
			},
		},
		svcErr: errors.New("service list failed"),
	}

	action := NewListNetworksAction(mock)
	handler := NewListNetworksHandler(action)

	req := httptest.NewRequest("GET", "/api/networks", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

func TestListNetworksHandler_ServiceRefByName(t *testing.T) {
	now := time.Now().UTC()
	mock := &mockDockerClient{
		networks: []network.Summary{
			{
				ID:      "net-abc",
				Name:    "my-net",
				Driver:  "overlay",
				Scope:   "swarm",
				Created: now,
			},
		},
		services: []swarm.Service{
			{
				ID: "svc1",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{Name: "web"},
					Mode:        swarm.ServiceMode{Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(1)}},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{Image: "nginx"},
						Networks: []swarm.NetworkAttachmentConfig{
							{Target: "my-net"}, // references by name, not ID
						},
					},
				},
				Meta: swarm.Meta{UpdatedAt: now},
			},
		},
	}

	action := NewListNetworksAction(mock)
	handler := NewListNetworksHandler(action)

	req := httptest.NewRequest("GET", "/api/networks", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	var resp struct {
		Networks []NetworkItem `json:"networks"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Networks) != 1 {
		t.Fatalf("expected 1 network, got %d", len(resp.Networks))
	}
	if len(resp.Networks[0].Services) != 1 {
		t.Fatalf("expected 1 service attached by name, got %d", len(resp.Networks[0].Services))
	}
	if resp.Networks[0].Services[0].Name != "web" {
		t.Errorf("expected service name web, got %s", resp.Networks[0].Services[0].Name)
	}
}
