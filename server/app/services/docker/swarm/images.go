package swarm

import (
	"context"

	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/image"

	"github.com/prvious/kontainer/server/app/services/docker"
)

func (s *SwarmOrchestrator) ListImages(ctx context.Context) ([]docker.Image, error) {
	imgs, err := s.client.ImageList(ctx, image.ListOptions{})
	if err != nil {
		return nil, err
	}

	items := make([]docker.Image, 0, len(imgs))
	for _, img := range imgs {
		items = append(items, docker.Image{
			ID:       img.ID,
			Tags:     img.RepoTags,
			Size:     img.Size,
			Created:  img.Created,
			NodeID:   s.nodeID,
			Hostname: s.hostname,
		})
	}

	return items, nil
}

func (s *SwarmOrchestrator) RemoveImage(ctx context.Context, id string, force bool) (*docker.ImageRemoveResult, error) {
	responses, err := s.client.ImageRemove(ctx, id, image.RemoveOptions{Force: force})
	if err != nil {
		return nil, err
	}

	result := &docker.ImageRemoveResult{}
	for _, r := range responses {
		if r.Deleted != "" {
			result.Deleted = append(result.Deleted, r.Deleted)
		}
		if r.Untagged != "" {
			result.Untagged = append(result.Untagged, r.Untagged)
		}
	}

	return result, nil
}

func (s *SwarmOrchestrator) PruneImages(ctx context.Context) (*docker.ImagePruneResult, error) {
	report, err := s.client.ImagesPrune(ctx, filters.NewArgs())
	if err != nil {
		return nil, err
	}

	return &docker.ImagePruneResult{
		SpaceReclaimed: report.SpaceReclaimed,
		ImagesDeleted:  len(report.ImagesDeleted),
	}, nil
}
