package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"
	"sort"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

type NodeTaskItem struct {
	ID           string `json:"id"`
	ServiceID    string `json:"serviceId"`
	ServiceName  string `json:"serviceName"`
	Slot         int    `json:"slot"`
	State        string `json:"state"`
	DesiredState string `json:"desiredState"`
	Error        string `json:"error"`
	Timestamp    string `json:"timestamp"`
	ContainerID  string `json:"containerId"`
}

type ListNodeTasksAction struct {
	client DockerClient
}

func NewListNodeTasksAction(client DockerClient) *ListNodeTasksAction {
	return &ListNodeTasksAction{client: client}
}

func (a *ListNodeTasksAction) Execute(ctx context.Context, nodeID string) ([]NodeTaskItem, error) {
	// Verify node exists
	_, _, err := a.client.NodeInspectWithRaw(ctx, nodeID)
	if err != nil {
		return nil, err
	}

	// List tasks filtered by node
	taskFilter := filters.NewArgs()
	taskFilter.Add("node", nodeID)
	tasks, err := a.client.TaskList(ctx, types.TaskListOptions{Filters: taskFilter})
	if err != nil {
		return nil, err
	}

	// Resolve service names
	serviceNames := make(map[string]string)
	servicesList, err := a.client.ServiceList(ctx, types.ServiceListOptions{})
	if err == nil {
		for _, s := range servicesList {
			serviceNames[s.ID] = s.Spec.Name
		}
	}

	// Sort tasks: most recent first
	sort.Slice(tasks, func(i, j int) bool {
		return tasks[i].Status.Timestamp.After(tasks[j].Status.Timestamp)
	})

	items := make([]NodeTaskItem, 0, len(tasks))
	for _, t := range tasks {
		serviceName := serviceNames[t.ServiceID]
		if serviceName == "" && t.ServiceID != "" {
			serviceName = t.ServiceID[:min(12, len(t.ServiceID))]
		}

		containerID := ""
		if t.Status.ContainerStatus != nil && t.Status.ContainerStatus.ContainerID != "" {
			cid := t.Status.ContainerStatus.ContainerID
			if len(cid) > 12 {
				cid = cid[:12]
			}
			containerID = cid
		}

		items = append(items, NodeTaskItem{
			ID:           t.ID,
			ServiceID:    t.ServiceID,
			ServiceName:  serviceName,
			Slot:         t.Slot,
			State:        string(t.Status.State),
			DesiredState: string(t.DesiredState),
			Error:        t.Status.Err,
			Timestamp:    t.Status.Timestamp.UTC().Format("2006-01-02T15:04:05Z"),
			ContainerID:  containerID,
		})
	}

	return items, nil
}

type ListNodeTasksHandler struct {
	action *ListNodeTasksAction
}

func NewListNodeTasksHandler(action *ListNodeTasksAction) *ListNodeTasksHandler {
	return &ListNodeTasksHandler{action: action}
}

func (h *ListNodeTasksHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	nodeID := chi.URLParam(r, "nodeID")

	items, err := h.action.Execute(r.Context(), nodeID)
	if err != nil {
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "node not found"})
			return
		}
		log.Printf("list node tasks error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to list node tasks"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"tasks": items})
}
