package swarm

import (
	"context"

	dockerswarm "github.com/docker/docker/api/types/swarm"

	"github.com/prvious/kontainer/server/app/services/docker"
)

func (s *SwarmOrchestrator) ListSecrets(ctx context.Context) ([]docker.Secret, error) {
	secrets, err := s.client.SecretList(ctx, dockerswarm.SecretListOptions{})
	if err != nil {
		return nil, err
	}

	items := make([]docker.Secret, 0, len(secrets))
	for _, sec := range secrets {
		items = append(items, docker.Secret{
			ID:        sec.ID,
			Name:      sec.Spec.Name,
			CreatedAt: docker.FormatTime(sec.CreatedAt),
			UpdatedAt: docker.FormatTime(sec.UpdatedAt),
		})
	}

	return items, nil
}

func (s *SwarmOrchestrator) CreateSecret(ctx context.Context, name string, data []byte) error {
	_, err := s.client.SecretCreate(ctx, dockerswarm.SecretSpec{
		Annotations: dockerswarm.Annotations{Name: name},
		Data:        data,
	})
	return err
}

func (s *SwarmOrchestrator) RemoveSecret(ctx context.Context, id string) error {
	return s.client.SecretRemove(ctx, id)
}
