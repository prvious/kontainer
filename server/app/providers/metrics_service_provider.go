package providers

import (
	"os"
	"time"

	"github.com/goravel/framework/contracts/foundation"

	"github.com/prvious/kontainer/server/app/actions/metrics"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/cluster"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type MetricsServiceProvider struct{}

func (p *MetricsServiceProvider) Register(app foundation.Application) {
	app.Singleton("metrics.collector", func(app foundation.Application) (any, error) {
		orchestratorInstance, err := app.Make("docker.orchestrator")
		if err != nil {
			return nil, err
		}
		orchestrator := orchestratorInstance.(docker.Orchestrator)

		registryInstance, err := app.Make("cluster.registry")
		if err != nil {
			return nil, err
		}
		registry := registryInstance.(*cluster.Registry)

		poolInstance, err := app.Make("cluster.pool")
		if err != nil {
			return nil, err
		}
		pool := poolInstance.(*cluster.GrpcPool)

		nodeID := facades.Config().GetString("cluster.node_id", "")
		hostname, _ := os.Hostname()

		nodeAction := metrics.NewCollectNodeMetricsAction()
		containerAction := metrics.NewCollectContainerMetricsAction(orchestrator)

		return metrics.NewCollectAndStoreAction(
			nodeAction, containerAction, registry, pool, nodeID, hostname,
		), nil
	})

	app.Singleton("metrics.pruner", func(app foundation.Application) (any, error) {
		retention := time.Duration(facades.Config().GetInt("metrics.retention_hours", 24)) * time.Hour
		return metrics.NewPruneMetricsAction(retention), nil
	})
}

func (p *MetricsServiceProvider) Boot(app foundation.Application) {}
