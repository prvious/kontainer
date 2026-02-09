package images

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/image"
)

type mockDockerClient struct {
	images    []image.Summary
	imageErr  error
	removeRes []image.DeleteResponse
	removeErr error
	pruneRes  image.PruneReport
	pruneErr  error
}

func (m *mockDockerClient) ImageList(_ context.Context, _ image.ListOptions) ([]image.Summary, error) {
	return m.images, m.imageErr
}

func (m *mockDockerClient) ImageRemove(_ context.Context, _ string, _ image.RemoveOptions) ([]image.DeleteResponse, error) {
	return m.removeRes, m.removeErr
}

func (m *mockDockerClient) ImagesPrune(_ context.Context, _ filters.Args) (image.PruneReport, error) {
	return m.pruneRes, m.pruneErr
}

func TestListImagesAction_HappyPath(t *testing.T) {
	mock := &mockDockerClient{
		images: []image.Summary{
			{
				ID:       "sha256:abc123def456789",
				RepoTags: []string{"nginx:latest", "nginx:1.25"},
				Size:     142000000,
				Created:  1700000000,
			},
			{
				ID:       "sha256:def456abc789012",
				RepoTags: []string{"redis:7"},
				Size:     45000000,
				Created:  1699000000,
			},
		},
	}

	action := NewListImagesAction(mock)
	items, err := action.Execute(context.Background(), "node-1", "host-1")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(items) != 2 {
		t.Fatalf("expected 2 images, got %d", len(items))
	}

	if items[0].ID != "abc123def456" {
		t.Errorf("expected truncated ID abc123def456, got %s", items[0].ID)
	}
	if len(items[0].Tags) != 2 {
		t.Errorf("expected 2 tags, got %d", len(items[0].Tags))
	}
	if items[0].Size != 142000000 {
		t.Errorf("expected size 142000000, got %d", items[0].Size)
	}
	if items[0].NodeID != "node-1" {
		t.Errorf("expected nodeID node-1, got %s", items[0].NodeID)
	}
	if items[0].Hostname != "host-1" {
		t.Errorf("expected hostname host-1, got %s", items[0].Hostname)
	}

	if items[1].ID != "def456abc789" {
		t.Errorf("expected truncated ID def456abc789, got %s", items[1].ID)
	}
}

func TestListImagesAction_EmptyList(t *testing.T) {
	mock := &mockDockerClient{images: []image.Summary{}}

	action := NewListImagesAction(mock)
	items, err := action.Execute(context.Background(), "node-1", "host-1")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(items) != 0 {
		t.Errorf("expected 0 images, got %d", len(items))
	}
}

func TestListImagesAction_DockerError(t *testing.T) {
	mock := &mockDockerClient{imageErr: errors.New("connection refused")}

	action := NewListImagesAction(mock)
	_, err := action.Execute(context.Background(), "node-1", "host-1")
	if err == nil {
		t.Fatal("expected error, got nil")
	}
}

func TestInternalImagesHandler_HappyPath(t *testing.T) {
	mock := &mockDockerClient{
		images: []image.Summary{
			{
				ID:       "sha256:abc123def456789",
				RepoTags: []string{"nginx:latest"},
				Size:     142000000,
				Created:  1700000000,
			},
		},
	}

	action := NewListImagesAction(mock)
	handler := NewInternalImagesHandler(action, "node-1", "host-1")

	req := httptest.NewRequest("GET", "/internal/images", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp NodeImages
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.NodeID != "node-1" {
		t.Errorf("expected nodeID node-1, got %s", resp.NodeID)
	}
	if len(resp.Images) != 1 {
		t.Fatalf("expected 1 image, got %d", len(resp.Images))
	}
	if resp.Images[0].Tags[0] != "nginx:latest" {
		t.Errorf("expected tag nginx:latest, got %s", resp.Images[0].Tags[0])
	}
}

func TestInternalImagesHandler_Error(t *testing.T) {
	mock := &mockDockerClient{imageErr: errors.New("docker error")}

	action := NewListImagesAction(mock)
	handler := NewInternalImagesHandler(action, "node-1", "host-1")

	req := httptest.NewRequest("GET", "/internal/images", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to list images" {
		t.Errorf("unexpected error message: %s", resp["error"])
	}
}
