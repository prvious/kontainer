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

func TestListSecretsHandler_Success(t *testing.T) {
	now := time.Now().UTC()
	mock := &mockDockerClient{
		secrets: []swarm.Secret{
			{
				ID: "sec1",
				Spec: swarm.SecretSpec{
					Annotations: swarm.Annotations{
						Name:   "db-password",
						Labels: map[string]string{"env": "prod"},
					},
				},
				Meta: swarm.Meta{CreatedAt: now, UpdatedAt: now},
			},
			{
				ID: "sec2",
				Spec: swarm.SecretSpec{
					Annotations: swarm.Annotations{
						Name: "api-key",
					},
				},
				Meta: swarm.Meta{CreatedAt: now, UpdatedAt: now},
			},
		},
	}

	action := NewListSecretsAction(mock)
	handler := NewListSecretsHandler(action)

	req := httptest.NewRequest("GET", "/api/secrets", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Secrets []SecretItem `json:"secrets"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Secrets) != 2 {
		t.Fatalf("expected 2 secrets, got %d", len(resp.Secrets))
	}

	if resp.Secrets[0].ID != "sec1" {
		t.Errorf("expected id sec1, got %s", resp.Secrets[0].ID)
	}
	if resp.Secrets[0].Name != "db-password" {
		t.Errorf("expected name db-password, got %s", resp.Secrets[0].Name)
	}
	if resp.Secrets[0].Labels["env"] != "prod" {
		t.Errorf("expected label env=prod, got %v", resp.Secrets[0].Labels)
	}
}

func TestListSecretsHandler_Empty(t *testing.T) {
	mock := &mockDockerClient{
		secrets: []swarm.Secret{},
	}

	action := NewListSecretsAction(mock)
	handler := NewListSecretsHandler(action)

	req := httptest.NewRequest("GET", "/api/secrets", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Secrets []SecretItem `json:"secrets"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Secrets) != 0 {
		t.Errorf("expected 0 secrets, got %d", len(resp.Secrets))
	}
}

func TestListSecretsHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		secretErr: errors.New("connection refused"),
	}

	action := NewListSecretsAction(mock)
	handler := NewListSecretsHandler(action)

	req := httptest.NewRequest("GET", "/api/secrets", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to list secrets" {
		t.Errorf("unexpected error message: %s", resp["error"])
	}
}
