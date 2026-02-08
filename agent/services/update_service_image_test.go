package services

import (
	"bytes"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/docker/docker/api/types/swarm"
	dockererrdefs "github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

func setupImageRouter(handler *UpdateServiceImageHandler) *chi.Mux {
	r := chi.NewRouter()
	r.Put("/api/services/{serviceID}/image", handler.ServeHTTP)
	return r
}

func TestUpdateServiceImageHandler_Success(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc1",
			Meta: swarm.Meta{
				Version: swarm.Version{Index: 10},
			},
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{Name: "web"},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{
						Image: "nginx:1.24",
					},
				},
			},
		},
	}

	action := NewUpdateServiceImageAction(mock)
	handler := NewUpdateServiceImageHandler(action)
	router := setupImageRouter(handler)

	body := `{"image":"nginx:1.25"}`
	req := httptest.NewRequest("PUT", "/api/services/svc1/image", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}

	var resp updateImageResponse
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Image != "nginx:1.25" {
		t.Errorf("expected image nginx:1.25, got %s", resp.Image)
	}
}

func TestUpdateServiceImageHandler_EmptyImage(t *testing.T) {
	mock := &mockDockerClient{}
	action := NewUpdateServiceImageAction(mock)
	handler := NewUpdateServiceImageHandler(action)
	router := setupImageRouter(handler)

	body := `{"image":""}`
	req := httptest.NewRequest("PUT", "/api/services/svc1/image", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "image field is required" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestUpdateServiceImageHandler_WhitespaceOnlyImage(t *testing.T) {
	mock := &mockDockerClient{}
	action := NewUpdateServiceImageAction(mock)
	handler := NewUpdateServiceImageHandler(action)
	router := setupImageRouter(handler)

	body := `{"image":"   "}`
	req := httptest.NewRequest("PUT", "/api/services/svc1/image", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}
}

func TestUpdateServiceImageHandler_InvalidBody(t *testing.T) {
	mock := &mockDockerClient{}
	action := NewUpdateServiceImageAction(mock)
	handler := NewUpdateServiceImageHandler(action)
	router := setupImageRouter(handler)

	req := httptest.NewRequest("PUT", "/api/services/svc1/image", bytes.NewBufferString("not json"))
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}
}

func TestUpdateServiceImageHandler_ServiceNotFound(t *testing.T) {
	mock := &mockDockerClient{
		inspectErr: dockererrdefs.NotFound(errors.New("not found")),
	}

	action := NewUpdateServiceImageAction(mock)
	handler := NewUpdateServiceImageHandler(action)
	router := setupImageRouter(handler)

	body := `{"image":"nginx:1.25"}`
	req := httptest.NewRequest("PUT", "/api/services/nonexistent/image", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", w.Code)
	}
}

func TestUpdateServiceImageHandler_DockerUpdateError(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID:   "svc1",
			Meta: swarm.Meta{Version: swarm.Version{Index: 1}},
			Spec: swarm.ServiceSpec{
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{
						Image: "nginx:1.24",
					},
				},
			},
		},
		updateErr: errors.New("update failed"),
	}

	action := NewUpdateServiceImageAction(mock)
	handler := NewUpdateServiceImageHandler(action)
	router := setupImageRouter(handler)

	body := `{"image":"nginx:1.25"}`
	req := httptest.NewRequest("PUT", "/api/services/svc1/image", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}
