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
		hostname, err := os.Hostname()
		if err != nil {
			facades.Log().Warningf("metrics: failed to get hostname, using empty string: %v", err)
		}

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

func (p *MetricsServiceProvider) Boot(app foundation.Application) {
	// When running inside a container, /host/proc is typically bind-mounted from the host's /proc.
	// Setting HOST_PROC tells gopsutil to read host-level metrics instead of container-level metrics.
	if _, err := os.Stat("/host/proc"); err == nil {
		if err := os.Setenv("HOST_PROC", "/host/proc"); err != nil {
			facades.Log().Warningf("metrics: failed to set HOST_PROC: %v", err)
		} else {
			facades.Log().Infof("metrics: detected container environment, set HOST_PROC=/host/proc")
		}
	}
}
