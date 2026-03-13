package swarm

import (
	"context"
	"encoding/base64"

	dockerswarm "github.com/docker/docker/api/types/swarm"

	"github.com/prvious/kontainer/server/app/services/docker"
)

func (s *SwarmOrchestrator) ListConfigs(ctx context.Context) ([]docker.Config, error) {
	configs, err := s.client.ConfigList(ctx, dockerswarm.ConfigListOptions{})
	if err != nil {
		return nil, err
	}

	items := make([]docker.Config, 0, len(configs))
	for _, c := range configs {
		items = append(items, docker.Config{
			ID:        c.ID,
			Name:      c.Spec.Name,
			CreatedAt: docker.FormatTime(c.CreatedAt),
			UpdatedAt: docker.FormatTime(c.UpdatedAt),
		})
	}

	return items, nil
}

func (s *SwarmOrchestrator) CreateConfig(ctx context.Context, name string, data []byte) error {
	_, err := s.client.ConfigCreate(ctx, dockerswarm.ConfigSpec{
		Annotations: dockerswarm.Annotations{Name: name},
		Data:        data,
	})
	return err
}

func (s *SwarmOrchestrator) RemoveConfig(ctx context.Context, id string) error {
	return s.client.ConfigRemove(ctx, id)
}

func (s *SwarmOrchestrator) InspectConfig(ctx context.Context, id string) (*docker.ConfigDetail, error) {
	cfg, _, err := s.client.ConfigInspectWithRaw(ctx, id)
	if err != nil {
		return nil, err
	}

	return &docker.ConfigDetail{
		Config: docker.Config{
			ID:        cfg.ID,
			Name:      cfg.Spec.Name,
			CreatedAt: docker.FormatTime(cfg.CreatedAt),
			UpdatedAt: docker.FormatTime(cfg.UpdatedAt),
		},
		Data: base64.StdEncoding.EncodeToString(cfg.Spec.Data),
	}, nil
}
