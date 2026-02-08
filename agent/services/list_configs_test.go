package services

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/docker/docker/api/types/swarm"
)

func TestListConfigsHandler_Success(t *testing.T) {
	now := time.Now().UTC()
	mock := &mockDockerClient{
		configs: []swarm.Config{
			{
				ID: "cfg1",
				Spec: swarm.ConfigSpec{
					Annotations: swarm.Annotations{
						Name:   "nginx.conf",
						Labels: map[string]string{"app": "web"},
					},
					Data: []byte("worker_processes auto;"),
				},
				Meta: swarm.Meta{CreatedAt: now, UpdatedAt: now},
			},
			{
				ID: "cfg2",
				Spec: swarm.ConfigSpec{
					Annotations: swarm.Annotations{
						Name: "app.env",
					},
					Data: []byte("KEY=VALUE"),
				},
				Meta: swarm.Meta{CreatedAt: now, UpdatedAt: now},
			},
		},
	}

	action := NewListConfigsAction(mock)
	handler := NewListConfigsHandler(action)

	req := httptest.NewRequest("GET", "/api/configs", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Configs []ConfigItem `json:"configs"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Configs) != 2 {
		t.Fatalf("expected 2 configs, got %d", len(resp.Configs))
	}

	if resp.Configs[0].ID != "cfg1" {
		t.Errorf("expected id cfg1, got %s", resp.Configs[0].ID)
	}
	if resp.Configs[0].Name != "nginx.conf" {
		t.Errorf("expected name nginx.conf, got %s", resp.Configs[0].Name)
	}
	expectedSize := len("worker_processes auto;")
	if resp.Configs[0].DataSize != expectedSize {
		t.Errorf("expected dataSize %d, got %d", expectedSize, resp.Configs[0].DataSize)
	}
	if resp.Configs[0].Labels["app"] != "web" {
		t.Errorf("expected label app=web, got %v", resp.Configs[0].Labels)
	}
}

func TestListConfigsHandler_Empty(t *testing.T) {
	mock := &mockDockerClient{
		configs: []swarm.Config{},
	}

	action := NewListConfigsAction(mock)
	handler := NewListConfigsHandler(action)

	req := httptest.NewRequest("GET", "/api/configs", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Configs []ConfigItem `json:"configs"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Configs) != 0 {
		t.Errorf("expected 0 configs, got %d", len(resp.Configs))
	}
}

func TestListConfigsHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		configErr: errors.New("connection refused"),
	}

	action := NewListConfigsAction(mock)
	handler := NewListConfigsHandler(action)

	req := httptest.NewRequest("GET", "/api/configs", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to list configs" {
		t.Errorf("unexpected error message: %s", resp["error"])
	}
}
