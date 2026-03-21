package console

import (
	"context"

	"github.com/goravel/framework/contracts/console"
	"github.com/goravel/framework/contracts/schedule"

	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/cluster"
)

type metricsCollector interface {
	Execute(ctx context.Context)
}

type metricsPruner interface {
	Execute() error
}

type Kernel struct{}

func (kernel *Kernel) Commands() []console.Command {
	return []console.Command{}
}

func (kernel *Kernel) Schedule() []schedule.Event {
	var events []schedule.Event

	// All scheduled tasks (discovery, metrics) only run on the leader
	if facades.Config().GetString("cluster.role") != "leader" {
		return events
	}

	discoveryInstance, err := facades.App().Make("cluster.discovery")
	if err != nil {
		facades.Log().Errorf("console kernel: failed to resolve cluster.discovery: %v", err)
		return events
	}
	discovery, ok := discoveryInstance.(cluster.Discovery)
	if !ok {
		facades.Log().Errorf("console kernel: cluster.discovery does not implement Discovery")
		return events
	}

	devMode := facades.Config().GetString("cluster.dev_followers") != ""

	if devMode {
		events = append(events, facades.Schedule().Call(func() {
			discovery.Discover(context.Background())
		}).EveryFiveSeconds().SkipIfStillRunning())
	} else {
		events = append(events, facades.Schedule().Call(func() {
			discovery.Discover(context.Background())
		}).EveryTenSeconds().SkipIfStillRunning())
	}

	// Metrics collection (every 60s) and pruning (every 5min)
	collectorInstance, err := facades.App().Make("metrics.collector")
	if err != nil {
		facades.Log().Errorf("console kernel: failed to resolve metrics.collector: %v", err)
		return events
	}
	collector, ok := collectorInstance.(metricsCollector)
	if !ok {
		facades.Log().Errorf("console kernel: metrics.collector does not implement metricsCollector (got %T)", collectorInstance)
		return events
	}

	events = append(events, facades.Schedule().Call(func() {
		collector.Execute(context.Background())
	}).EveryMinute().SkipIfStillRunning())

	prunerInstance, err := facades.App().Make("metrics.pruner")
	if err != nil {
		facades.Log().Errorf("console kernel: failed to resolve metrics.pruner: %v", err)
		return events
	}
	pruner, ok := prunerInstance.(metricsPruner)
	if !ok {
		facades.Log().Errorf("console kernel: metrics.pruner does not implement metricsPruner (got %T)", prunerInstance)
		return events
	}

	events = append(events, facades.Schedule().Call(func() {
		if err := pruner.Execute(); err != nil {
			facades.Log().Errorf("metrics prune error: %v", err)
		}
	}).EveryFiveMinutes().SkipIfStillRunning())

	return events
}
