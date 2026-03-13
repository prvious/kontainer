package swarm

import (
	"context"
	"io"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/image"
	"github.com/docker/docker/api/types/network"
	dockerswarm "github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/api/types/system"
	"github.com/docker/docker/api/types/volume"

	"github.com/prvious/kontainer/server/app/services/docker"
)

// DockerAPI is a narrow interface for the Docker SDK methods we use.
// This makes the Swarm implementation testable with mocks.
type DockerAPI interface {
	ServiceList(ctx context.Context, opts types.ServiceListOptions) ([]dockerswarm.Service, error)
	ServiceInspectWithRaw(ctx context.Context, serviceID string, opts types.ServiceInspectOptions) (dockerswarm.Service, []byte, error)
	ServiceUpdate(ctx context.Context, serviceID string, version dockerswarm.Version, service dockerswarm.ServiceSpec, options dockerswarm.ServiceUpdateOptions) (dockerswarm.ServiceUpdateResponse, error)
	ServiceRemove(ctx context.Context, serviceID string) error
	ServiceLogs(ctx context.Context, serviceID string, opts container.LogsOptions) (io.ReadCloser, error)
	TaskList(ctx context.Context, opts types.TaskListOptions) ([]dockerswarm.Task, error)
	TaskInspectWithRaw(ctx context.Context, taskID string) (dockerswarm.Task, []byte, error)
	TaskLogs(ctx context.Context, taskID string, opts container.LogsOptions) (io.ReadCloser, error)
	NodeList(ctx context.Context, opts types.NodeListOptions) ([]dockerswarm.Node, error)
	NodeInspectWithRaw(ctx context.Context, nodeID string) (dockerswarm.Node, []byte, error)
	NodeUpdate(ctx context.Context, nodeID string, version dockerswarm.Version, node dockerswarm.NodeSpec) error
	NetworkList(ctx context.Context, opts network.ListOptions) ([]network.Summary, error)
	VolumeList(ctx context.Context, opts volume.ListOptions) (volume.ListResponse, error)
	SecretList(ctx context.Context, opts dockerswarm.SecretListOptions) ([]dockerswarm.Secret, error)
	SecretCreate(ctx context.Context, secret dockerswarm.SecretSpec) (dockerswarm.SecretCreateResponse, error)
	SecretRemove(ctx context.Context, id string) error
	ConfigList(ctx context.Context, opts dockerswarm.ConfigListOptions) ([]dockerswarm.Config, error)
	ConfigCreate(ctx context.Context, config dockerswarm.ConfigSpec) (dockerswarm.ConfigCreateResponse, error)
	ConfigInspectWithRaw(ctx context.Context, id string) (dockerswarm.Config, []byte, error)
	ConfigRemove(ctx context.Context, id string) error
	ImageList(ctx context.Context, opts image.ListOptions) ([]image.Summary, error)
	ImageRemove(ctx context.Context, imageID string, opts image.RemoveOptions) ([]image.DeleteResponse, error)
	ImagesPrune(ctx context.Context, pruneFilters filters.Args) (image.PruneReport, error)
	ContainerExecCreate(ctx context.Context, container string, config container.ExecOptions) (types.IDResponse, error)
	ContainerExecAttach(ctx context.Context, execID string, config container.ExecAttachOptions) (types.HijackedResponse, error)
	ContainerExecResize(ctx context.Context, execID string, options container.ResizeOptions) error
	ContainerExecInspect(ctx context.Context, execID string) (container.ExecInspect, error)
	ContainerStats(ctx context.Context, containerID string, stream bool) (container.StatsResponseReader, error)
	Info(ctx context.Context) (system.Info, error)
}

// SwarmOrchestrator implements the Orchestrator interface for Docker Swarm.
type SwarmOrchestrator struct {
	client   DockerAPI
	nodeID   string
	hostname string
}

func NewSwarmOrchestrator(client DockerAPI, nodeID, hostname string) *SwarmOrchestrator {
	return &SwarmOrchestrator{
		client:   client,
		nodeID:   nodeID,
		hostname: hostname,
	}
}

// Compile-time check that SwarmOrchestrator implements Orchestrator
var _ docker.Orchestrator = (*SwarmOrchestrator)(nil)
