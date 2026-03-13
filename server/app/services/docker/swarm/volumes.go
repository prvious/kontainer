package swarm

import (
	"context"

	"github.com/docker/docker/api/types/volume"

	"github.com/prvious/kontainer/server/app/services/docker"
)

func (s *SwarmOrchestrator) ListVolumes(ctx context.Context) ([]docker.Volume, error) {
	resp, err := s.client.VolumeList(ctx, volume.ListOptions{})
	if err != nil {
		return nil, err
	}

	items := make([]docker.Volume, 0, len(resp.Volumes))
	for _, v := range resp.Volumes {
		items = append(items, docker.Volume{
			Name:       v.Name,
			Driver:     v.Driver,
			Mountpoint: v.Mountpoint,
			Scope:      v.Scope,
			CreatedAt:  v.CreatedAt,
		})
	}

	return items, nil
}
