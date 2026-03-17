package docker

import (
	"context"
	"io"
	"time"
)

// --- Domain Types ---

type EnvVar struct {
	Key   string `json:"key"`
	Value string `json:"value"`
}

type Service struct {
	ID              string `json:"id"`
	Name            string `json:"name"`
	Image           string `json:"image"`
	Stack           string `json:"stack"`
	ReplicasRunning int    `json:"replicasRunning"`
	ReplicasDesired int    `json:"replicasDesired"`
	UpdatedAt       string `json:"updatedAt"`
}

type UpdateStatusInfo struct {
	State       string `json:"state"`
	StartedAt   string `json:"startedAt"`
	CompletedAt string `json:"completedAt"`
	Message     string `json:"message"`
}

type ServiceDetail struct {
	ID            string            `json:"id"`
	Name          string            `json:"name"`
	Image         string            `json:"image"`
	Stack         string            `json:"stack"`
	Mode          string            `json:"mode"`
	Replicas      *int              `json:"replicas"`
	CreatedAt     string            `json:"createdAt"`
	UpdatedAt     string            `json:"updatedAt"`
	Env           []EnvVar          `json:"env"`
	Ports         []PortConfig      `json:"ports"`
	Mounts        []Mount           `json:"mounts"`
	Constraints   []string          `json:"constraints"`
	Labels        map[string]string `json:"labels"`
	UpdateStatus  *UpdateStatusInfo `json:"updateStatus"`
	PreviousImage string            `json:"previousImage"`
	RawSpec       any               `json:"rawSpec,omitempty"`
}

type PortConfig struct {
	Protocol      string `json:"protocol"`
	TargetPort    uint32 `json:"targetPort"`
	PublishedPort uint32 `json:"publishedPort"`
	PublishMode   string `json:"publishMode"`
}

type Mount struct {
	Type     string `json:"type"`
	Source   string `json:"source"`
	Target   string `json:"target"`
	ReadOnly bool   `json:"readOnly"`
}

type Task struct {
	ID           string `json:"id"`
	ServiceID    string `json:"serviceId"`
	ServiceName  string `json:"serviceName"`
	NodeID       string `json:"nodeId"`
	NodeName     string `json:"nodeName"`
	State        string `json:"state"`
	DesiredState string `json:"desiredState"`
	Timestamp    string `json:"timestamp"`
	ContainerID  string `json:"containerId"`
	Image        string `json:"image"`
	Slot         int    `json:"slot"`
	ExitCode     *int   `json:"exitCode"`
	Error        string `json:"error"`
}

type TaskNetworkInfo struct {
	Name      string   `json:"name"`
	Addresses []string `json:"addresses"`
}

type TaskDetail struct {
	Task
	Pid       int               `json:"pid"`
	CreatedAt string            `json:"createdAt"`
	UpdatedAt string            `json:"updatedAt"`
	Env       []EnvVar          `json:"env"`
	Mounts    []Mount           `json:"mounts"`
	Networks  []TaskNetworkInfo `json:"networks"`
}

type ContainerStats struct {
	ContainerID string  `json:"containerId"`
	Name        string  `json:"name"`
	ServiceID   string  `json:"serviceId"`
	NodeID      string  `json:"nodeId"`
	CPUPercent  float64 `json:"cpuPercent"`
	MemUsage    int64   `json:"memUsage"`
	MemLimit    int64   `json:"memLimit"`
	MemPercent  float64 `json:"memPercent"`
}

type TaskStatsAggregate struct {
	Total          int `json:"total"`
	Running        int `json:"running"`
	Failed         int `json:"failed"`
	Preparing      int `json:"preparing"`
	Complete       int `json:"complete"`
	DesiredRunning int `json:"desiredRunning"`
}

type NetworkServiceRef struct {
	ID   string `json:"id"`
	Name string `json:"name"`
}

type Node struct {
	ID            string `json:"id"`
	Hostname      string `json:"hostname"`
	Status        string `json:"status"`
	Availability  string `json:"availability"`
	Role          string `json:"role"`
	EngineVersion string `json:"engineVersion"`
	OS            string `json:"os"`
	Architecture  string `json:"architecture"`
	IP            string `json:"ip"`
	Leader        bool   `json:"leader"`
	UpdatedAt     string `json:"updatedAt"`
}

type Network struct {
	ID         string              `json:"id"`
	Name       string              `json:"name"`
	Driver     string              `json:"driver"`
	Scope      string              `json:"scope"`
	Internal   bool                `json:"internal"`
	Attachable bool                `json:"attachable"`
	Ingress    bool                `json:"ingress"`
	Services   []NetworkServiceRef `json:"services"`
	CreatedAt  string              `json:"createdAt"`
}

type Volume struct {
	Name       string            `json:"name"`
	Driver     string            `json:"driver"`
	Mountpoint string            `json:"mountpoint"`
	Scope      string            `json:"scope"`
	Labels     map[string]string `json:"labels"`
	Options    map[string]string `json:"options"`
	CreatedAt  string            `json:"createdAt"`
	RefCount   int64             `json:"refCount"`
	SizeBytes  int64             `json:"sizeBytes"`
	Stack      string            `json:"stack"`
}

type Stack struct {
	Name            string `json:"name"`
	Services        int    `json:"services"`
	RunningReplicas int    `json:"runningReplicas"`
	DesiredReplicas int    `json:"desiredReplicas"`
	UpdatedAt       string `json:"updatedAt"`
}

type Secret struct {
	ID        string            `json:"id"`
	Name      string            `json:"name"`
	Labels    map[string]string `json:"labels"`
	CreatedAt string            `json:"createdAt"`
	UpdatedAt string            `json:"updatedAt"`
}

type Config struct {
	ID        string            `json:"id"`
	Name      string            `json:"name"`
	Labels    map[string]string `json:"labels"`
	DataSize  int64             `json:"dataSize"`
	CreatedAt string            `json:"createdAt"`
	UpdatedAt string            `json:"updatedAt"`
}

type ConfigDetail struct {
	Config
	Data string `json:"data"`
}

type Image struct {
	ID       string   `json:"id"`
	Tags     []string `json:"tags"`
	Size     int64    `json:"size"`
	Created  int64    `json:"created"`
	NodeID   string   `json:"nodeId"`
	Hostname string   `json:"hostname"`
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
	Deleted  []string `json:"deleted"`
	Untagged []string `json:"untagged"`
}

type ImagePruneResult struct {
	SpaceReclaimed uint64 `json:"spaceReclaimed"`
	ImagesDeleted  int    `json:"imagesDeleted"`
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
	TaskStatsAgg(ctx context.Context) (*TaskStatsAggregate, error)

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
