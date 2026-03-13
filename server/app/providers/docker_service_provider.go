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
		if err == nil && info.Swarm.NodeID != "" {
			return info.Swarm.NodeID, nil
		}
		return "", nil
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

		nodeIDInstance, _ := app.Make("docker.node_id")
		nodeID, _ := nodeIDInstance.(string)

		hostname, _ := os.Hostname()
		return swarm.NewSwarmOrchestrator(client, nodeID, hostname), nil
	})
}

func (r *DockerServiceProvider) Boot(app foundation.Application) {}
