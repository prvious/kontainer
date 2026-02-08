package services

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/docker/docker/api/types/swarm"
)

func TestCreateConfigHandler_Success(t *testing.T) {
	mock := &mockDockerClient{
		configCreateResp: swarm.ConfigCreateResponse{ID: "new-config-id"},
	}

	action := NewCreateConfigAction(mock)
	handler := NewCreateConfigHandler(action)

	body := `{"name":"nginx.conf","data":"worker_processes auto;"}`
	req := httptest.NewRequest("POST", "/api/configs", strings.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusCreated {
		t.Fatalf("expected 201, got %d", w.Code)
	}

	var resp map[string]string
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp["id"] != "new-config-id" {
		t.Errorf("expected id new-config-id, got %s", resp["id"])
	}
	if resp["name"] != "nginx.conf" {
		t.Errorf("expected name nginx.conf, got %s", resp["name"])
	}
}

func TestCreateConfigHandler_EmptyName(t *testing.T) {
	mock := &mockDockerClient{}

	action := NewCreateConfigAction(mock)
	handler := NewCreateConfigHandler(action)

	body := `{"name":"","data":"some-data"}`
	req := httptest.NewRequest("POST", "/api/configs", strings.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "name is required" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestCreateConfigHandler_InvalidBody(t *testing.T) {
	mock := &mockDockerClient{}

	action := NewCreateConfigAction(mock)
	handler := NewCreateConfigHandler(action)

	req := httptest.NewRequest("POST", "/api/configs", strings.NewReader("not json"))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "invalid request body" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestCreateConfigHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		configCreateErr: errors.New("already exists"),
	}

	action := NewCreateConfigAction(mock)
	handler := NewCreateConfigHandler(action)

	body := `{"name":"nginx.conf","data":"content"}`
	req := httptest.NewRequest("POST", "/api/configs", strings.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to create config" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}
