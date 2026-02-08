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

func TestCreateSecretHandler_Success(t *testing.T) {
	mock := &mockDockerClient{
		secretCreateResp: swarm.SecretCreateResponse{ID: "new-secret-id"},
	}

	action := NewCreateSecretAction(mock)
	handler := NewCreateSecretHandler(action)

	body := `{"name":"db-password","data":"s3cret-value"}`
	req := httptest.NewRequest("POST", "/api/secrets", strings.NewReader(body))
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

	if resp["id"] != "new-secret-id" {
		t.Errorf("expected id new-secret-id, got %s", resp["id"])
	}
	if resp["name"] != "db-password" {
		t.Errorf("expected name db-password, got %s", resp["name"])
	}
}

func TestCreateSecretHandler_EmptyName(t *testing.T) {
	mock := &mockDockerClient{}

	action := NewCreateSecretAction(mock)
	handler := NewCreateSecretHandler(action)

	body := `{"name":"","data":"some-data"}`
	req := httptest.NewRequest("POST", "/api/secrets", strings.NewReader(body))
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

func TestCreateSecretHandler_InvalidBody(t *testing.T) {
	mock := &mockDockerClient{}

	action := NewCreateSecretAction(mock)
	handler := NewCreateSecretHandler(action)

	req := httptest.NewRequest("POST", "/api/secrets", strings.NewReader("not json"))
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

func TestCreateSecretHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		secretCreateErr: errors.New("already exists"),
	}

	action := NewCreateSecretAction(mock)
	handler := NewCreateSecretHandler(action)

	body := `{"name":"db-password","data":"s3cret"}`
	req := httptest.NewRequest("POST", "/api/secrets", strings.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to create secret" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}
