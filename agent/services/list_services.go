package services

import (
	"context"
	"encoding/json"
	"io"
	"log"
	"net/http"
	"strings"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/swarm"
)

type DockerClient interface {
	ServiceList(ctx context.Context, opts types.ServiceListOptions) ([]swarm.Service, error)
	TaskList(ctx context.Context, opts types.TaskListOptions) ([]swarm.Task, error)
	ServiceInspectWithRaw(ctx context.Context, serviceID string, opts types.ServiceInspectOptions) (swarm.Service, []byte, error)
	NodeList(ctx context.Context, opts types.NodeListOptions) ([]swarm.Node, error)
	ServiceLogs(ctx context.Context, serviceID string, opts container.LogsOptions) (io.ReadCloser, error)
	ServiceUpdate(ctx context.Context, serviceID string, version swarm.Version, service swarm.ServiceSpec, options swarm.ServiceUpdateOptions) (swarm.ServiceUpdateResponse, error)
	ServiceRemove(ctx context.Context, serviceID string) error
	TaskInspectWithRaw(ctx context.Context, taskID string) (swarm.Task, []byte, error)
	TaskLogs(ctx context.Context, taskID string, opts container.LogsOptions) (io.ReadCloser, error)
}

type ServiceItem struct {
	ID              string `json:"id"`
	Name            string `json:"name"`
	Image           string `json:"image"`
	Stack           string `json:"stack"`
	ReplicasRunning int    `json:"replicasRunning"`
	ReplicasDesired int    `json:"replicasDesired"`
	UpdatedAt       string `json:"updatedAt"`
}

type ListServicesAction struct {
	client DockerClient
}

func NewListServicesAction(client DockerClient) *ListServicesAction {
	return &ListServicesAction{client: client}
}

func (a *ListServicesAction) Execute(ctx context.Context) ([]ServiceItem, error) {
	svcs, err := a.client.ServiceList(ctx, types.ServiceListOptions{})
	if err != nil {
		return nil, err
	}

	taskFilter := filters.NewArgs()
	taskFilter.Add("desired-state", "running")
	tasks, err := a.client.TaskList(ctx, types.TaskListOptions{Filters: taskFilter})
	if err != nil {
		return nil, err
	}

	// Count running tasks per service
	runningCount := make(map[string]int)
	for _, t := range tasks {
		if t.Status.State == swarm.TaskStateRunning {
			runningCount[t.ServiceID]++
		}
	}

	items := make([]ServiceItem, 0, len(svcs))
	for _, svc := range svcs {
		image := svc.Spec.TaskTemplate.ContainerSpec.Image
		if idx := strings.Index(image, "@sha256:"); idx != -1 {
			image = image[:idx]
		}

		stack := svc.Spec.Labels["com.docker.stack.namespace"]

		var desired int
		if svc.Spec.Mode.Replicated != nil && svc.Spec.Mode.Replicated.Replicas != nil {
			desired = int(*svc.Spec.Mode.Replicated.Replicas)
		} else if svc.Spec.Mode.Global != nil {
			desired = -1 // signals global mode
		}

		items = append(items, ServiceItem{
			ID:              svc.ID,
			Name:            svc.Spec.Name,
			Image:           image,
			Stack:           stack,
			ReplicasRunning: runningCount[svc.ID],
			ReplicasDesired: desired,
			UpdatedAt:       svc.UpdatedAt.UTC().Format("2006-01-02T15:04:05Z"),
		})
	}

	return items, nil
}

type ListServicesHandler struct {
	action *ListServicesAction
}

func NewListServicesHandler(action *ListServicesAction) *ListServicesHandler {
	return &ListServicesHandler{action: action}
}

func (h *ListServicesHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	items, err := h.action.Execute(r.Context())
	if err != nil {
		log.Printf("list services error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to list services"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"services": items})
}
