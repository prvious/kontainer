package metrics

import (
	"context"
	"log"
	"sync"
	"time"

	"github.com/clovismuneza/devoper/agent/cluster"
	"github.com/clovismuneza/devoper/agent/storage"
)

type MetricsCollector struct {
	local          *InternalMetricsHandler
	registry       *cluster.Registry
	internalSecret string
	store          *storage.MetricsStore
	interval       time.Duration
	stopCh         chan struct{}
	stopOnce       sync.Once
	wg             sync.WaitGroup
}

func NewMetricsCollector(
	local *InternalMetricsHandler,
	registry *cluster.Registry,
	internalSecret string,
	store *storage.MetricsStore,
	interval time.Duration,
) *MetricsCollector {
	return &MetricsCollector{
		local:          local,
		registry:       registry,
		internalSecret: internalSecret,
		store:          store,
		interval:       interval,
		stopCh:         make(chan struct{}),
	}
}

func (c *MetricsCollector) Start(ctx context.Context) {
	c.wg.Add(1)
	go func() {
		defer c.wg.Done()

		ticker := time.NewTicker(c.interval)
		defer ticker.Stop()

		var tickCount int

		for {
			select {
			case <-ticker.C:
				c.collectAndStore(ctx)

				tickCount++
				if tickCount%10 == 0 {
					if err := c.store.Prune(); err != nil {
						log.Printf("metrics collector: prune error: %v", err)
					}
				}

			case <-c.stopCh:
				return
			case <-ctx.Done():
				return
			}
		}
	}()
}

func (c *MetricsCollector) Stop() {
	c.stopOnce.Do(func() {
		close(c.stopCh)
	})
	c.wg.Wait()
}

func snapshotToRow(s AgentMetricsSnapshot) storage.MetricsRow {
	return storage.MetricsRow{
		Timestamp:   s.Timestamp,
		NodeID:      s.NodeID,
		Hostname:    s.Hostname,
		CPUs:        s.Node.CPUs,
		CPUPercent:  s.Node.CPUPercent,
		MemUsed:     s.Node.MemoryUsed,
		MemTotal:    s.Node.MemoryTotal,
		MemPercent:  s.Node.MemoryPercent,
		DiskUsed:    s.Node.DiskUsed,
		DiskTotal:   s.Node.DiskTotal,
		DiskPercent: s.Node.DiskPercent,
	}
}

func (c *MetricsCollector) collectAndStore(ctx context.Context) {
	// Collect and store local metrics
	localSnapshot, err := c.local.Collect(ctx)
	if err != nil {
		log.Printf("metrics collector: local collect error: %v", err)
	} else {
		if err := c.store.Insert(snapshotToRow(localSnapshot)); err != nil {
			log.Printf("metrics collector: local insert error: %v", err)
		}
	}

	// Collect and store follower metrics
	for _, follower := range c.registry.All() {
		snapshot, err := fetchFollowerMetrics(ctx, follower.Address, c.internalSecret)
		if err != nil {
			log.Printf("metrics collector: follower %s (%s) error: %v", follower.NodeID, follower.Address, err)
			continue
		}
		if err := c.store.Insert(snapshotToRow(snapshot)); err != nil {
			log.Printf("metrics collector: follower insert error: %v", err)
		}
	}
}
