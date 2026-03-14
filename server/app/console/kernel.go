package console

import (
	"context"

	"github.com/goravel/framework/contracts/schedule"

	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/cluster"
)

type Kernel struct{}

func (kernel *Kernel) Schedule() []schedule.Event {
	var events []schedule.Event

	// Only run discovery on leader nodes
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

	return events
}
