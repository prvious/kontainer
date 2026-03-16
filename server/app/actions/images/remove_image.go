package images

import (
	"context"
	"fmt"

	"github.com/prvious/kontainer/server/app/services/cluster"
	"github.com/prvious/kontainer/server/app/services/docker"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

// RemoveImageResponse holds the result of an image removal.
type RemoveImageResponse struct {
	Deleted  []string `json:"deleted"`
	Untagged []string `json:"untagged"`
}

// RemoveImageAction removes an image from the specified node.
type RemoveImageAction struct {
	orchestrator docker.Orchestrator
	registry     *cluster.Registry
	pool         *cluster.GrpcPool
	leaderNodeID string
}

func NewRemoveImageAction(
	orchestrator docker.Orchestrator,
	registry *cluster.Registry,
	pool *cluster.GrpcPool,
	leaderNodeID string,
) *RemoveImageAction {
	return &RemoveImageAction{
		orchestrator: orchestrator,
		registry:     registry,
		pool:         pool,
		leaderNodeID: leaderNodeID,
	}
}

func (a *RemoveImageAction) Execute(ctx context.Context, imageID string, nodeID string, force bool) (*RemoveImageResponse, error) {
	isLocal := nodeID == "" || nodeID == a.leaderNodeID
	if isLocal {
		result, err := a.orchestrator.RemoveImage(ctx, imageID, force)
		if err != nil {
			return nil, fmt.Errorf("remove local image: %w", err)
		}
		return &RemoveImageResponse{
			Deleted:  result.Deleted,
			Untagged: result.Untagged,
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

	resp, err := client.RemoveImage(ctx, &pb.RemoveImageRequest{
		ImageId: imageID,
		Force:   force,
	})
	if err != nil {
		return nil, fmt.Errorf("remove image on node %s: %w", nodeID, err)
	}

	return &RemoveImageResponse{
		Deleted:  resp.Deleted,
		Untagged: resp.Untagged,
	}, nil
}
