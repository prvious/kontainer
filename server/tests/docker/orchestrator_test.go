package docker_test

import (
	"testing"

	"github.com/prvious/kontainer/server/app/services/docker"
	"github.com/prvious/kontainer/server/app/services/docker/swarm"
)

func TestSwarmOrchestrator_ImplementsInterface(t *testing.T) {
	// Compile-time check is already in swarm/client.go via:
	//   var _ docker.Orchestrator = (*SwarmOrchestrator)(nil)
	// This test exists as documentation and for future mock implementations.
	var _ docker.Orchestrator = (*swarm.SwarmOrchestrator)(nil)
	t.Log("SwarmOrchestrator satisfies Orchestrator interface")
}
