package images

import (
	"context"
	"fmt"

	"github.com/prvious/kontainer/server/app/services/cluster"
	"github.com/prvious/kontainer/server/app/services/docker"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

// ImageItem represents a single image with its node context.
type ImageItem struct {
	ID       string   `json:"id"`
	Tags     []string `json:"tags"`
	Size     int64    `json:"size"`
	Created  int64    `json:"created"`
	NodeID   string   `json:"nodeId"`
	Hostname string   `json:"hostname"`
}

// NodeImages groups images by node.
type NodeImages struct {
	NodeID   string      `json:"nodeId"`
	Hostname string      `json:"hostname"`
	Images   []ImageItem `json:"images"`
}

// ClusterImagesResponse is the top-level response for listing cluster images.
type ClusterImagesResponse struct {
	Nodes []NodeImages `json:"nodes"`
}

// ListClusterImagesAction collects images from the local node and all followers.
type ListClusterImagesAction struct {
	orchestrator docker.Orchestrator
	registry     *cluster.Registry
	pool         *cluster.GrpcPool
	nodeID       string
	hostname     string
}

func NewListClusterImagesAction(
	orchestrator docker.Orchestrator,
	registry *cluster.Registry,
	pool *cluster.GrpcPool,
	nodeID string,
	hostname string,
) *ListClusterImagesAction {
	return &ListClusterImagesAction{
		orchestrator: orchestrator,
		registry:     registry,
		pool:         pool,
		nodeID:       nodeID,
		hostname:     hostname,
	}
}

func (a *ListClusterImagesAction) Execute(ctx context.Context) (*ClusterImagesResponse, error) {
	var nodes []NodeImages

	// Collect local images
	localImages, err := a.orchestrator.ListImages(ctx)
	if err != nil {
		return nil, fmt.Errorf("list local images: %w", err)
	}

	localItems := make([]ImageItem, 0, len(localImages))
	for _, img := range localImages {
		localItems = append(localItems, ImageItem{
			ID:       img.ID,
			Tags:     img.Tags,
			Size:     img.Size,
			Created:  img.Created,
			NodeID:   a.nodeID,
			Hostname: a.hostname,
		})
	}
	nodes = append(nodes, NodeImages{
		NodeID:   a.nodeID,
		Hostname: a.hostname,
		Images:   localItems,
	})

	// Fan out to followers via gRPC
	for _, agent := range a.registry.All() {
		if agent.NodeID == a.nodeID {
			continue
		}

		client, err := a.pool.GetClient(agent.Address)
		if err != nil {
			continue
		}

		resp, err := client.ListImages(ctx, &pb.ListImagesRequest{})
		if err != nil {
			continue
		}

		items := make([]ImageItem, 0, len(resp.Images))
		for _, img := range resp.Images {
			items = append(items, ImageItem{
				ID:       img.Id,
				Tags:     img.Tags,
				Size:     img.Size,
				Created:  img.Created,
				NodeID:   resp.NodeId,
				Hostname: resp.Hostname,
			})
		}
		nodes = append(nodes, NodeImages{
			NodeID:   resp.NodeId,
			Hostname: resp.Hostname,
			Images:   items,
		})
	}

	return &ClusterImagesResponse{Nodes: nodes}, nil
}
