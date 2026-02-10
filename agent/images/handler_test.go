package images

import (
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/prvious/kontainer/agent/cluster"
	"github.com/docker/docker/api/types/image"
	"github.com/go-chi/chi/v5"
)

func TestClusterImagesHandler_LocalOnly(t *testing.T) {
	mock := &mockDockerClient{
		images: []image.Summary{
			{
				ID:       "sha256:abc123def456789",
				RepoTags: []string{"nginx:latest"},
				Size:     100000000,
				Created:  1700000000,
			},
		},
	}

	action := NewListImagesAction(mock)
	local := NewInternalImagesHandler(action, "leader-node", "leader-host")
	registry := cluster.NewRegistry()
	handler := NewClusterImagesHandler(local, registry, "test-secret")

	req := httptest.NewRequest("GET", "/api/images", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp ClusterImagesResponse
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Nodes) != 1 {
		t.Fatalf("expected 1 node, got %d", len(resp.Nodes))
	}
	if resp.Nodes[0].NodeID != "leader-node" {
		t.Errorf("expected nodeID leader-node, got %s", resp.Nodes[0].NodeID)
	}
	if len(resp.Nodes[0].Images) != 1 {
		t.Errorf("expected 1 image, got %d", len(resp.Nodes[0].Images))
	}
}

func TestClusterImagesHandler_WithFollower(t *testing.T) {
	mock := &mockDockerClient{
		images: []image.Summary{
			{
				ID:       "sha256:abc123def456789",
				RepoTags: []string{"nginx:latest"},
				Size:     100000000,
				Created:  1700000000,
			},
		},
	}

	// Set up a fake follower server
	followerImages := NodeImages{
		NodeID:   "follower-node",
		Hostname: "follower-host",
		Images: []ImageItem{
			{ID: "def456abc789", Tags: []string{"redis:7"}, Size: 50000000, Created: 1699000000, NodeID: "follower-node", Hostname: "follower-host"},
		},
	}
	followerServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("X-Kontainer-Secret") != "test-secret" {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(followerImages)
	}))
	defer followerServer.Close()

	action := NewListImagesAction(mock)
	local := NewInternalImagesHandler(action, "leader-node", "leader-host")
	registry := cluster.NewRegistry()
	// Register the follower using its test server address (strip http://)
	addr := followerServer.Listener.Addr().String()
	registry.Update([]cluster.AgentInfo{
		{NodeID: "follower-node", Hostname: "follower-host", Address: addr},
	})
	handler := NewClusterImagesHandler(local, registry, "test-secret")

	req := httptest.NewRequest("GET", "/api/images", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp ClusterImagesResponse
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Nodes) != 2 {
		t.Fatalf("expected 2 nodes, got %d", len(resp.Nodes))
	}
}

func TestClusterImagesHandler_FollowerFailureNonFatal(t *testing.T) {
	mock := &mockDockerClient{
		images: []image.Summary{},
	}

	action := NewListImagesAction(mock)
	local := NewInternalImagesHandler(action, "leader-node", "leader-host")
	registry := cluster.NewRegistry()
	// Register a follower at a non-existent address
	registry.Update([]cluster.AgentInfo{
		{NodeID: "bad-follower", Hostname: "bad-host", Address: "127.0.0.1:1"},
	})
	handler := NewClusterImagesHandler(local, registry, "test-secret")

	req := httptest.NewRequest("GET", "/api/images", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	// Should still succeed with just the leader node
	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp ClusterImagesResponse
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Nodes) != 1 {
		t.Errorf("expected 1 node (leader only), got %d", len(resp.Nodes))
	}
}

func TestRemoveImageHandler_LocalDelete(t *testing.T) {
	mock := &mockDockerClient{
		removeRes: []image.DeleteResponse{
			{Deleted: "sha256:abc123"},
			{Untagged: "nginx:latest"},
		},
	}

	registry := cluster.NewRegistry()
	handler := NewRemoveImageHandler(mock, registry, "leader-node", "test-secret")

	r := chi.NewRouter()
	r.Delete("/api/images/{imageID}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/images/abc123?node=leader-node&force=false", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}

	var resp RemoveImageResponse
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Deleted) != 1 || resp.Deleted[0] != "sha256:abc123" {
		t.Errorf("expected 1 deleted layer, got %v", resp.Deleted)
	}
	if len(resp.Untagged) != 1 || resp.Untagged[0] != "nginx:latest" {
		t.Errorf("expected 1 untagged, got %v", resp.Untagged)
	}
}

func TestRemoveImageHandler_ForceDelete(t *testing.T) {
	mock := &mockDockerClient{
		removeRes: []image.DeleteResponse{
			{Deleted: "sha256:abc123"},
		},
	}

	registry := cluster.NewRegistry()
	handler := NewRemoveImageHandler(mock, registry, "leader-node", "test-secret")

	r := chi.NewRouter()
	r.Delete("/api/images/{imageID}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/images/abc123?node=leader-node&force=true", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

func TestRemoveImageHandler_UnknownNode(t *testing.T) {
	mock := &mockDockerClient{}
	registry := cluster.NewRegistry()
	handler := NewRemoveImageHandler(mock, registry, "leader-node", "test-secret")

	r := chi.NewRouter()
	r.Delete("/api/images/{imageID}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/images/abc123?node=unknown-node", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", w.Code)
	}
}

func TestRemoveImageHandler_ProxiedDelete(t *testing.T) {
	// Set up a fake follower
	followerServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(RemoveImageResponse{
			Deleted:  []string{"sha256:proxied"},
			Untagged: []string{"redis:7"},
		})
	}))
	defer followerServer.Close()

	mock := &mockDockerClient{}
	registry := cluster.NewRegistry()
	addr := followerServer.Listener.Addr().String()
	registry.Update([]cluster.AgentInfo{
		{NodeID: "follower-node", Address: addr},
	})
	handler := NewRemoveImageHandler(mock, registry, "leader-node", "test-secret")

	r := chi.NewRouter()
	r.Delete("/api/images/{imageID}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", fmt.Sprintf("/api/images/abc123?node=follower-node&force=true"), nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

func TestRemoveImageHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		removeErr: errors.New("image in use"),
	}

	registry := cluster.NewRegistry()
	handler := NewRemoveImageHandler(mock, registry, "leader-node", "test-secret")

	r := chi.NewRouter()
	r.Delete("/api/images/{imageID}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/images/abc123?node=leader-node", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

func TestPruneImagesHandler_AllNodes(t *testing.T) {
	mock := &mockDockerClient{
		pruneRes: image.PruneReport{
			SpaceReclaimed: 500000000,
			ImagesDeleted:  []image.DeleteResponse{{Deleted: "a"}, {Deleted: "b"}},
		},
	}

	// Fake follower that returns prune results
	followerServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(PruneImagesResponse{
			SpaceReclaimed: 300000000,
			ImagesDeleted:  3,
		})
	}))
	defer followerServer.Close()

	registry := cluster.NewRegistry()
	addr := followerServer.Listener.Addr().String()
	registry.Update([]cluster.AgentInfo{
		{NodeID: "follower-node", Address: addr},
	})
	handler := NewPruneImagesHandler(mock, registry, "leader-node", "test-secret")

	req := httptest.NewRequest("POST", "/api/images/prune", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp PruneImagesResponse
	json.NewDecoder(w.Body).Decode(&resp)

	// 500MB from leader + 300MB from follower
	if resp.SpaceReclaimed != 800000000 {
		t.Errorf("expected 800000000 bytes reclaimed, got %d", resp.SpaceReclaimed)
	}
	// 2 from leader + 3 from follower
	if resp.ImagesDeleted != 5 {
		t.Errorf("expected 5 images deleted, got %d", resp.ImagesDeleted)
	}
}

func TestPruneImagesHandler_SpecificNode(t *testing.T) {
	mock := &mockDockerClient{
		pruneRes: image.PruneReport{
			SpaceReclaimed: 500000000,
			ImagesDeleted:  []image.DeleteResponse{{Deleted: "a"}},
		},
	}

	registry := cluster.NewRegistry()
	handler := NewPruneImagesHandler(mock, registry, "leader-node", "test-secret")

	req := httptest.NewRequest("POST", "/api/images/prune?node=leader-node", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp PruneImagesResponse
	json.NewDecoder(w.Body).Decode(&resp)

	if resp.SpaceReclaimed != 500000000 {
		t.Errorf("expected 500000000 bytes reclaimed, got %d", resp.SpaceReclaimed)
	}
}

func TestPruneImagesHandler_UnknownNode(t *testing.T) {
	mock := &mockDockerClient{}
	registry := cluster.NewRegistry()
	handler := NewPruneImagesHandler(mock, registry, "leader-node", "test-secret")

	req := httptest.NewRequest("POST", "/api/images/prune?node=unknown-node", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", w.Code)
	}
}
