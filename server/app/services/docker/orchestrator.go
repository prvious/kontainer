package docker

import (
	"context"
	"io"
	"time"
)

// --- Domain Types ---

type Service struct {
	ID              string
	Name            string
	Image           string
	Stack           string
	ReplicasRunning int
	ReplicasDesired int
	UpdatedAt       string
}

type ServiceDetail struct {
	Service
	Env         []string
	Ports       []PortConfig
	Mounts      []Mount
	Constraints []string
	Labels      map[string]string
	RawSpec     any
}

type PortConfig struct {
	Protocol      string
	TargetPort    uint32
	PublishedPort uint32
	PublishMode   string
}

type Mount struct {
	Type   string
	Source string
	Target string
}

type Task struct {
	ID           string
	ServiceID    string
	NodeID       string
	Status       string
	DesiredState string
	Timestamp    string
	ContainerID  string
	Image        string
	Slot         int
	Error        string
}

type TaskDetail struct {
	Task
	ServiceName string
	NodeName    string
}

type ContainerStats struct {
	ContainerID string
	Name        string
	ServiceID   string
	NodeID      string
	CPUPercent  float64
	MemUsage    int64
	MemLimit    int64
	MemPercent  float64
}

type Node struct {
	ID            string
	Hostname      string
	Status        string
	Availability  string
	Role          string
	EngineVersion string
	IP            string
}

type Network struct {
	ID         string
	Name       string
	Driver     string
	Scope      string
	Internal   bool
	Attachable bool
}

type Volume struct {
	Name       string
	Driver     string
	Mountpoint string
	Scope      string
	CreatedAt  string
}

type Stack struct {
	Name     string
	Services int
}

type Secret struct {
	ID        string
	Name      string
	CreatedAt string
	UpdatedAt string
}

type Config struct {
	ID        string
	Name      string
	CreatedAt string
	UpdatedAt string
}

type ConfigDetail struct {
	Config
	Data string
}

type Image struct {
	ID       string
	Tags     []string
	Size     int64
	Created  int64
	NodeID   string
	Hostname string
}

type LogStreamOptions struct {
	Follow     bool
	Tail       string
	Since      string
	Timestamps bool
}

type LogStream interface {
	io.ReadCloser
}

type ExecOptions struct {
	Shell string
	User  string
	Cols  uint
	Rows  uint
}

type ExecSession interface {
	io.ReadWriteCloser
	Resize(cols, rows uint) error
}

type ImageRemoveResult struct {
	Deleted  []string
	Untagged []string
}

type ImagePruneResult struct {
	SpaceReclaimed uint64
	ImagesDeleted  int
}

// --- Filter Types ---

type ServiceFilters struct {
	Stack string
}

type TaskFilters struct {
	ServiceID string
	NodeID    string
}

// --- Orchestrator Interface ---

type Orchestrator interface {
	// Services
	ListServices(ctx context.Context, filters ServiceFilters) ([]Service, error)
	InspectService(ctx context.Context, id string) (*ServiceDetail, error)
	ScaleService(ctx context.Context, id string, replicas uint64) error
	RestartService(ctx context.Context, id string) error
	RollbackService(ctx context.Context, id string) error
	UpdateServiceEnv(ctx context.Context, id string, env []string) error
	UpdateServiceImage(ctx context.Context, id string, image string) error

	// Tasks
	ListTasks(ctx context.Context, filters TaskFilters) ([]Task, error)
	InspectTask(ctx context.Context, id string) (*TaskDetail, error)
	TaskStats(ctx context.Context) ([]ContainerStats, error)

	// Nodes
	ListNodes(ctx context.Context) ([]Node, error)
	UpdateNodeAvailability(ctx context.Context, id string, availability string) error
	UpdateNodeRole(ctx context.Context, id string, role string) error

	// Networks & Volumes
	ListNetworks(ctx context.Context) ([]Network, error)
	ListVolumes(ctx context.Context) ([]Volume, error)

	// Stacks
	ListStacks(ctx context.Context) ([]Stack, error)
	RemoveStack(ctx context.Context, name string) error

	// Secrets
	ListSecrets(ctx context.Context) ([]Secret, error)
	CreateSecret(ctx context.Context, name string, data []byte) error
	RemoveSecret(ctx context.Context, id string) error

	// Configs
	ListConfigs(ctx context.Context) ([]Config, error)
	CreateConfig(ctx context.Context, name string, data []byte) error
	RemoveConfig(ctx context.Context, id string) error
	InspectConfig(ctx context.Context, id string) (*ConfigDetail, error)

	// Images (local node only)
	ListImages(ctx context.Context) ([]Image, error)
	RemoveImage(ctx context.Context, id string, force bool) (*ImageRemoveResult, error)
	PruneImages(ctx context.Context) (*ImagePruneResult, error)

	// Logs
	StreamServiceLogs(ctx context.Context, id string, opts LogStreamOptions) (LogStream, error)
	StreamTaskLogs(ctx context.Context, id string, opts LogStreamOptions) (LogStream, error)

	// Exec
	ExecContainer(ctx context.Context, containerID string, opts ExecOptions) (ExecSession, error)
}

// --- Unexported helpers ---

func FormatTime(t time.Time) string {
	return t.UTC().Format("2006-01-02T15:04:05Z")
}
