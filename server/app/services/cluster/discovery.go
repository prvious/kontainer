package cluster

import "context"

// Discovery defines the contract for cluster follower discovery.
// Implementations discover followers and update the registry.
type Discovery interface {
	Discover(ctx context.Context)
}
