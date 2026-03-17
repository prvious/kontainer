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
		labels := v.Labels
		if labels == nil {
			labels = make(map[string]string)
		}
		options := v.Options
		if options == nil {
			options = make(map[string]string)
		}

		var refCount int64 = -1
		var sizeBytes int64 = -1
		if v.UsageData != nil {
			refCount = v.UsageData.RefCount
			sizeBytes = v.UsageData.Size
		}

		stack := labels["com.docker.stack.namespace"]

		items = append(items, docker.Volume{
			Name:       v.Name,
			Driver:     v.Driver,
			Mountpoint: v.Mountpoint,
			Scope:      v.Scope,
			Labels:     labels,
			Options:    options,
			CreatedAt:  v.CreatedAt,
			RefCount:   refCount,
			SizeBytes:  sizeBytes,
			Stack:      stack,
		})
	}

	return items, nil
}
