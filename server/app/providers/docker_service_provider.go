package providers

import (
	"context"
	"fmt"
	"os"

	dockerclient "github.com/docker/docker/client"
	"github.com/goravel/framework/contracts/foundation"

	"github.com/prvious/kontainer/server/app/services/docker/swarm"
)

type DockerServiceProvider struct{}

func (r *DockerServiceProvider) Register(app foundation.Application) {
	app.Singleton("docker.client", func(app foundation.Application) (any, error) {
		return dockerclient.NewClientWithOpts(
			dockerclient.FromEnv,
			dockerclient.WithAPIVersionNegotiation(),
		)
	})

	app.Singleton("docker.node_id", func(app foundation.Application) (any, error) {
		clientInstance, err := app.Make("docker.client")
		if err != nil {
			return "", err
		}
		client, ok := clientInstance.(*dockerclient.Client)
		if !ok {
			return "", fmt.Errorf("docker.client is not *dockerclient.Client")
		}
		info, err := client.Info(context.Background())
		if err != nil {
			return "", fmt.Errorf("failed to get Docker info for node ID: %w", err)
		}
		return info.Swarm.NodeID, nil
	})

	app.Singleton("docker.orchestrator", func(app foundation.Application) (any, error) {
		clientInstance, err := app.Make("docker.client")
		if err != nil {
			return nil, err
		}
		client, ok := clientInstance.(*dockerclient.Client)
		if !ok {
			return nil, fmt.Errorf("docker.client is not *dockerclient.Client")
		}

		nodeIDInstance, err := app.Make("docker.node_id")
		if err != nil {
			return nil, fmt.Errorf("failed to resolve docker.node_id: %w", err)
		}
		nodeID, ok := nodeIDInstance.(string)
		if !ok {
			return nil, fmt.Errorf("docker.node_id is not a string")
		}

		hostname, _ := os.Hostname()
		return swarm.NewSwarmOrchestrator(client, nodeID, hostname), nil
	})
}

func (r *DockerServiceProvider) Boot(app foundation.Application) {}
