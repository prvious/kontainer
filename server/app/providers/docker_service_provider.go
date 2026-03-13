package providers

import (
	"context"
	"os"

	dockerclient "github.com/docker/docker/client"
	"github.com/goravel/framework/contracts/foundation"

	"github.com/prvious/kontainer/server/app/facades"
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

	app.Singleton("docker.orchestrator", func(app foundation.Application) (any, error) {
		clientInstance, err := app.Make("docker.client")
		if err != nil {
			return nil, err
		}
		client, ok := clientInstance.(*dockerclient.Client)
		if !ok {
			facades.Log().Errorf("docker.client is not *dockerclient.Client")
			return nil, err
		}

		hostname, _ := os.Hostname()
		nodeID := ""
		info, err := client.Info(context.Background())
		if err == nil && info.Swarm.NodeID != "" {
			nodeID = info.Swarm.NodeID
		}

		return swarm.NewSwarmOrchestrator(client, nodeID, hostname), nil
	})
}

func (r *DockerServiceProvider) Boot(app foundation.Application) {}
