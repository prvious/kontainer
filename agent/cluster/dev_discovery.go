package cluster

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"strings"
	"time"
)

// DevDiscovery pings a static list of follower addresses (for local development).
// In dev, agents run on localhost with different ports instead of on an overlay network.
type DevDiscovery struct {
	addresses      []string
	internalSecret string
	registry       *Registry
	interval       time.Duration
	cancel         context.CancelFunc
}

func NewDevDiscovery(addresses []string, internalSecret string, registry *Registry) *DevDiscovery {
	return &DevDiscovery{
		addresses:      addresses,
		internalSecret: internalSecret,
		registry:       registry,
		interval:       5 * time.Second,
	}
}

// ParseDevFollowers splits a comma-separated list of "host:port" addresses.
func ParseDevFollowers(s string) []string {
	if s == "" {
		return nil
	}
	parts := strings.Split(s, ",")
	addrs := make([]string, 0, len(parts))
	for _, p := range parts {
		p = strings.TrimSpace(p)
		if p != "" {
			addrs = append(addrs, p)
		}
	}
	return addrs
}

func (d *DevDiscovery) Start(ctx context.Context) {
	ctx, d.cancel = context.WithCancel(ctx)
	go d.loop(ctx)
}

func (d *DevDiscovery) Stop() {
	if d.cancel != nil {
		d.cancel()
	}
}

func (d *DevDiscovery) loop(ctx context.Context) {
	d.discover(ctx)
	ticker := time.NewTicker(d.interval)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			d.discover(ctx)
		}
	}
}

func (d *DevDiscovery) discover(ctx context.Context) {
	var healthy []AgentInfo
	for _, addr := range d.addresses {
		info, err := d.pingFollower(ctx, addr)
		if err != nil {
			log.Printf("dev discovery: follower %s unreachable: %v", addr, err)
			continue
		}

		info.Address = addr
		info.Role = RoleFollower
		info.Status = "healthy"
		info.LastSeen = time.Now().UTC().Format(time.RFC3339)

		healthy = append(healthy, info)
	}
	d.registry.Update(healthy)
}

func (d *DevDiscovery) pingFollower(ctx context.Context, addr string) (AgentInfo, error) {
	url := fmt.Sprintf("http://%s/internal/health", addr)
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return AgentInfo{}, err
	}
	req.Header.Set(InternalSecretHeader, d.internalSecret)

	httpClient := &http.Client{Timeout: 3 * time.Second}
	resp, err := httpClient.Do(req)
	if err != nil {
		return AgentInfo{}, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return AgentInfo{}, fmt.Errorf("unexpected status %d", resp.StatusCode)
	}

	var info AgentInfo
	if err := json.NewDecoder(resp.Body).Decode(&info); err != nil {
		return AgentInfo{}, err
	}
	return info, nil
}
