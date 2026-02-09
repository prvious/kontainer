package images

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"time"

	"github.com/clovismuneza/devoper/agent/cluster"
	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/image"
	"github.com/go-chi/chi/v5"
)

// MutatingDockerClient extends DockerClient with write operations.
type MutatingDockerClient interface {
	DockerClient
	ImageRemove(ctx context.Context, imageID string, opts image.RemoveOptions) ([]image.DeleteResponse, error)
	ImagesPrune(ctx context.Context, pruneFilters filters.Args) (image.PruneReport, error)
}

// --- Internal handler (per-node) ---

type InternalImagesHandler struct {
	action   *ListImagesAction
	nodeID   string
	hostname string
}

func NewInternalImagesHandler(action *ListImagesAction, nodeID, hostname string) *InternalImagesHandler {
	return &InternalImagesHandler{action: action, nodeID: nodeID, hostname: hostname}
}

func (h *InternalImagesHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	items, err := h.action.Execute(r.Context(), h.nodeID, h.hostname)
	if err != nil {
		log.Printf("internal images error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to list images"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(NodeImages{
		NodeID:   h.nodeID,
		Hostname: h.hostname,
		Images:   items,
	})
}

// --- Cluster handler (leader fan-out) ---

type ClusterImagesHandler struct {
	local          *InternalImagesHandler
	registry       *cluster.Registry
	internalSecret string
}

func NewClusterImagesHandler(local *InternalImagesHandler, registry *cluster.Registry, internalSecret string) *ClusterImagesHandler {
	return &ClusterImagesHandler{local: local, registry: registry, internalSecret: internalSecret}
}

func (h *ClusterImagesHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	localImages, err := h.local.action.Execute(r.Context(), h.local.nodeID, h.local.hostname)
	if err != nil {
		log.Printf("cluster images (local) error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to collect local images"})
		return
	}

	nodes := []NodeImages{{
		NodeID:   h.local.nodeID,
		Hostname: h.local.hostname,
		Images:   localImages,
	}}

	for _, follower := range h.registry.All() {
		nodeImages, err := fetchFollowerImages(r.Context(), follower.Address, h.internalSecret)
		if err != nil {
			log.Printf("cluster images: failed to fetch from %s (%s): %v", follower.NodeID, follower.Address, err)
			continue
		}
		nodes = append(nodes, nodeImages)
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(ClusterImagesResponse{Nodes: nodes})
}

// --- Remove image handler ---

type RemoveImageHandler struct {
	client         MutatingDockerClient
	registry       *cluster.Registry
	leaderNodeID   string
	internalSecret string
}

func NewRemoveImageHandler(client MutatingDockerClient, registry *cluster.Registry, leaderNodeID, internalSecret string) *RemoveImageHandler {
	return &RemoveImageHandler{client: client, registry: registry, leaderNodeID: leaderNodeID, internalSecret: internalSecret}
}

func (h *RemoveImageHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	imageID := chi.URLParam(r, "imageID")
	nodeID := r.URL.Query().Get("node")
	force := r.URL.Query().Get("force") == "true"

	if nodeID == "" || nodeID == h.leaderNodeID {
		h.removeLocal(w, r, imageID, force)
		return
	}

	addr, ok := h.registry.ByNodeID(nodeID)
	if !ok {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusNotFound)
		json.NewEncoder(w).Encode(map[string]string{"error": "node not found"})
		return
	}

	h.proxyRemove(w, r, addr, imageID, force)
}

func (h *RemoveImageHandler) removeLocal(w http.ResponseWriter, r *http.Request, imageID string, force bool) {
	results, err := h.client.ImageRemove(r.Context(), imageID, image.RemoveOptions{Force: force, PruneChildren: true})
	if err != nil {
		log.Printf("remove image error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": fmt.Sprintf("failed to remove image: %v", err)})
		return
	}

	resp := RemoveImageResponse{
		Deleted:  make([]string, 0),
		Untagged: make([]string, 0),
	}
	for _, r := range results {
		if r.Deleted != "" {
			resp.Deleted = append(resp.Deleted, r.Deleted)
		}
		if r.Untagged != "" {
			resp.Untagged = append(resp.Untagged, r.Untagged)
		}
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

func (h *RemoveImageHandler) proxyRemove(w http.ResponseWriter, r *http.Request, addr, imageID string, force bool) {
	forceParam := "false"
	if force {
		forceParam = "true"
	}
	url := fmt.Sprintf("http://%s/internal/images/%s?force=%s", addr, imageID, forceParam)

	req, err := http.NewRequestWithContext(r.Context(), http.MethodDelete, url, nil)
	if err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to create proxy request"})
		return
	}
	req.Header.Set(cluster.InternalSecretHeader, h.internalSecret)

	resp, err := imagesHTTPClient.Do(req)
	if err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadGateway)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to reach follower"})
		return
	}
	defer resp.Body.Close()

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(resp.StatusCode)
	io.Copy(w, resp.Body)
}

// --- Internal remove handler (follower) ---

type InternalRemoveImageHandler struct {
	client MutatingDockerClient
}

func NewInternalRemoveImageHandler(client MutatingDockerClient) *InternalRemoveImageHandler {
	return &InternalRemoveImageHandler{client: client}
}

func (h *InternalRemoveImageHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	imageID := chi.URLParam(r, "imageID")
	force := r.URL.Query().Get("force") == "true"

	results, err := h.client.ImageRemove(r.Context(), imageID, image.RemoveOptions{Force: force, PruneChildren: true})
	if err != nil {
		log.Printf("internal remove image error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": fmt.Sprintf("failed to remove image: %v", err)})
		return
	}

	resp := RemoveImageResponse{
		Deleted:  make([]string, 0),
		Untagged: make([]string, 0),
	}
	for _, r := range results {
		if r.Deleted != "" {
			resp.Deleted = append(resp.Deleted, r.Deleted)
		}
		if r.Untagged != "" {
			resp.Untagged = append(resp.Untagged, r.Untagged)
		}
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

// --- Prune images handler ---

type PruneImagesHandler struct {
	client         MutatingDockerClient
	registry       *cluster.Registry
	leaderNodeID   string
	internalSecret string
}

func NewPruneImagesHandler(client MutatingDockerClient, registry *cluster.Registry, leaderNodeID, internalSecret string) *PruneImagesHandler {
	return &PruneImagesHandler{client: client, registry: registry, leaderNodeID: leaderNodeID, internalSecret: internalSecret}
}

func (h *PruneImagesHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	nodeID := r.URL.Query().Get("node")

	if nodeID != "" {
		if nodeID == h.leaderNodeID {
			h.pruneLocal(w, r)
			return
		}
		addr, ok := h.registry.ByNodeID(nodeID)
		if !ok {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "node not found"})
			return
		}
		h.proxyPrune(w, r, addr)
		return
	}

	// Prune all nodes
	total := PruneImagesResponse{}

	// Local prune
	report, err := h.client.ImagesPrune(r.Context(), filters.NewArgs(filters.Arg("dangling", "false")))
	if err != nil {
		log.Printf("prune images (local) error: %v", err)
	} else {
		total.SpaceReclaimed += report.SpaceReclaimed
		total.ImagesDeleted += len(report.ImagesDeleted)
	}

	// Fan out to followers
	for _, follower := range h.registry.All() {
		result, err := proxyPruneRequest(r.Context(), follower.Address, h.internalSecret)
		if err != nil {
			log.Printf("prune images: failed on %s (%s): %v", follower.NodeID, follower.Address, err)
			continue
		}
		total.SpaceReclaimed += result.SpaceReclaimed
		total.ImagesDeleted += result.ImagesDeleted
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(total)
}

func (h *PruneImagesHandler) pruneLocal(w http.ResponseWriter, r *http.Request) {
	report, err := h.client.ImagesPrune(r.Context(), filters.NewArgs(filters.Arg("dangling", "false")))
	if err != nil {
		log.Printf("prune images error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to prune images"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(PruneImagesResponse{
		SpaceReclaimed: report.SpaceReclaimed,
		ImagesDeleted:  len(report.ImagesDeleted),
	})
}

func (h *PruneImagesHandler) proxyPrune(w http.ResponseWriter, r *http.Request, addr string) {
	result, err := proxyPruneRequest(r.Context(), addr, h.internalSecret)
	if err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadGateway)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to reach follower"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(result)
}

// --- Internal prune handler (follower) ---

type InternalPruneImagesHandler struct {
	client MutatingDockerClient
}

func NewInternalPruneImagesHandler(client MutatingDockerClient) *InternalPruneImagesHandler {
	return &InternalPruneImagesHandler{client: client}
}

func (h *InternalPruneImagesHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	report, err := h.client.ImagesPrune(r.Context(), filters.NewArgs(filters.Arg("dangling", "false")))
	if err != nil {
		log.Printf("internal prune images error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to prune images"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(PruneImagesResponse{
		SpaceReclaimed: report.SpaceReclaimed,
		ImagesDeleted:  len(report.ImagesDeleted),
	})
}

// --- HTTP helpers ---

var imagesHTTPClient = &http.Client{Timeout: 10 * time.Second}

func fetchFollowerImages(ctx context.Context, addr, secret string) (NodeImages, error) {
	url := fmt.Sprintf("http://%s/internal/images", addr)

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return NodeImages{}, fmt.Errorf("create request: %w", err)
	}
	req.Header.Set(cluster.InternalSecretHeader, secret)

	resp, err := imagesHTTPClient.Do(req)
	if err != nil {
		return NodeImages{}, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, 4096))
		return NodeImages{}, fmt.Errorf("unexpected status %d: %s", resp.StatusCode, string(body))
	}

	var nodeImages NodeImages
	if err := json.NewDecoder(resp.Body).Decode(&nodeImages); err != nil {
		return NodeImages{}, fmt.Errorf("decode response: %w", err)
	}

	return nodeImages, nil
}

func proxyPruneRequest(ctx context.Context, addr, secret string) (PruneImagesResponse, error) {
	url := fmt.Sprintf("http://%s/internal/images/prune", addr)

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, nil)
	if err != nil {
		return PruneImagesResponse{}, fmt.Errorf("create request: %w", err)
	}
	req.Header.Set(cluster.InternalSecretHeader, secret)

	resp, err := imagesHTTPClient.Do(req)
	if err != nil {
		return PruneImagesResponse{}, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, 4096))
		return PruneImagesResponse{}, fmt.Errorf("unexpected status %d: %s", resp.StatusCode, string(body))
	}

	var result PruneImagesResponse
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return PruneImagesResponse{}, fmt.Errorf("decode response: %w", err)
	}

	return result, nil
}
