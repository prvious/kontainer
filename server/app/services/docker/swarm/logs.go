package swarm

import (
	"context"

	"github.com/docker/docker/api/types/container"

	"github.com/prvious/kontainer/server/app/services/docker"
)

func (s *SwarmOrchestrator) StreamServiceLogs(ctx context.Context, id string, opts docker.LogStreamOptions) (docker.LogStream, error) {
	return s.client.ServiceLogs(ctx, id, container.LogsOptions{
		ShowStdout: true,
		ShowStderr: true,
		Follow:     opts.Follow,
		Tail:       opts.Tail,
		Since:      opts.Since,
		Timestamps: opts.Timestamps,
	})
}

func (s *SwarmOrchestrator) StreamTaskLogs(ctx context.Context, id string, opts docker.LogStreamOptions) (docker.LogStream, error) {
	return s.client.TaskLogs(ctx, id, container.LogsOptions{
		ShowStdout: true,
		ShowStderr: true,
		Follow:     opts.Follow,
		Tail:       opts.Tail,
		Since:      opts.Since,
		Timestamps: opts.Timestamps,
	})
}
