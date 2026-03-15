package images

import (
	"context"
	"fmt"

	"github.com/prvious/kontainer/server/app/services/cluster"
	"github.com/prvious/kontainer/server/app/services/docker"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

// PruneImagesResponse holds aggregated results of image pruning.
type PruneImagesResponse struct {
	SpaceReclaimed uint64 `json:"spaceReclaimed"`
	ImagesDeleted  int    `json:"imagesDeleted"`
}

// PruneImagesAction prunes unused images on one or all nodes.
type PruneImagesAction struct {
	orchestrator docker.Orchestrator
	registry     *cluster.Registry
	pool         *cluster.GrpcPool
	leaderNodeID string
}

func NewPruneImagesAction(
	orchestrator docker.Orchestrator,
	registry *cluster.Registry,
	pool *cluster.GrpcPool,
	leaderNodeID string,
) *PruneImagesAction {
	return &PruneImagesAction{
		orchestrator: orchestrator,
		registry:     registry,
		pool:         pool,
		leaderNodeID: leaderNodeID,
	}
}

func (a *PruneImagesAction) Execute(ctx context.Context, nodeID string) (*PruneImagesResponse, error) {
	// If nodeID is specified, prune only that node.
	if nodeID != "" {
		return a.pruneNode(ctx, nodeID)
	}

	// No nodeID — prune all nodes (local + followers).
	var totalSpace uint64
	var totalDeleted int

	localResult, err := a.orchestrator.PruneImages(ctx)
	if err != nil {
		return nil, fmt.Errorf("prune local images: %w", err)
	}
	totalSpace += localResult.SpaceReclaimed
	totalDeleted += localResult.ImagesDeleted

	for _, agent := range a.registry.All() {
		if agent.NodeID == a.leaderNodeID {
			continue
		}

		client, err := a.pool.GetClient(agent.Address)
		if err != nil {
			continue
		}

		resp, err := client.PruneImages(ctx, &pb.PruneImagesRequest{})
		if err != nil {
			continue
		}

		totalSpace += resp.SpaceReclaimed
		totalDeleted += int(resp.ImagesDeleted)
	}

	return &PruneImagesResponse{
		SpaceReclaimed: totalSpace,
		ImagesDeleted:  totalDeleted,
	}, nil
}

func (a *PruneImagesAction) pruneNode(ctx context.Context, nodeID string) (*PruneImagesResponse, error) {
	isLocal := nodeID == a.leaderNodeID
	if isLocal {
		result, err := a.orchestrator.PruneImages(ctx)
		if err != nil {
			return nil, fmt.Errorf("prune local images: %w", err)
		}
		return &PruneImagesResponse{
			SpaceReclaimed: result.SpaceReclaimed,
			ImagesDeleted:  result.ImagesDeleted,
		}, nil
	}

	addr, ok := a.registry.ByNodeID(nodeID)
	if !ok {
		return nil, fmt.Errorf("node %s not found in registry", nodeID)
	}

	client, err := a.pool.GetClient(addr)
	if err != nil {
		return nil, fmt.Errorf("get grpc client for node %s: %w", nodeID, err)
	}

	resp, err := client.PruneImages(ctx, &pb.PruneImagesRequest{})
	if err != nil {
		return nil, fmt.Errorf("prune images on node %s: %w", nodeID, err)
	}

	return &PruneImagesResponse{
		SpaceReclaimed: resp.SpaceReclaimed,
		ImagesDeleted:  int(resp.ImagesDeleted),
	}, nil
}
