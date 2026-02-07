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

type ServiceTaskItem struct {
	ID           string `json:"id"`
	Slot         int    `json:"slot"`
	NodeID       string `json:"nodeId"`
	NodeName     string `json:"nodeName"`
	State        string `json:"state"`
	DesiredState string `json:"desiredState"`
	ExitCode     *int   `json:"exitCode"`
	Error        string `json:"error"`
	Timestamp    string `json:"timestamp"`
	ContainerID  string `json:"containerId"`
}

type ListServiceTasksAction struct {
	client DockerClient
}

func NewListServiceTasksAction(client DockerClient) *ListServiceTasksAction {
	return &ListServiceTasksAction{client: client}
}

func (a *ListServiceTasksAction) Execute(ctx context.Context, serviceID string) ([]ServiceTaskItem, error) {
	// Verify the service exists
	_, _, err := a.client.ServiceInspectWithRaw(ctx, serviceID, types.ServiceInspectOptions{})
	if err != nil {
		return nil, err
	}

	// List tasks filtered by service
	taskFilter := filters.NewArgs()
	taskFilter.Add("service", serviceID)
	tasks, err := a.client.TaskList(ctx, types.TaskListOptions{Filters: taskFilter})
	if err != nil {
		return nil, err
	}

	// Resolve node names
	nodeNames := make(map[string]string)
	nodes, err := a.client.NodeList(ctx, types.NodeListOptions{})
	if err == nil {
		for _, n := range nodes {
			name := n.Description.Hostname
			if name == "" {
				name = n.ID[:12]
			}
			nodeNames[n.ID] = name
		}
	}

	// Sort tasks: most recent first
	sort.Slice(tasks, func(i, j int) bool {
		return tasks[i].Status.Timestamp.After(tasks[j].Status.Timestamp)
	})

	items := make([]ServiceTaskItem, 0, len(tasks))
	for _, t := range tasks {
		nodeName := nodeNames[t.NodeID]
		if nodeName == "" && t.NodeID != "" {
			nodeName = t.NodeID[:min(12, len(t.NodeID))]
		}

		var exitCode *int
		if t.Status.ContainerStatus != nil {
			ec := t.Status.ContainerStatus.ExitCode
			exitCode = &ec
		}

		containerID := ""
		if t.Status.ContainerStatus != nil && t.Status.ContainerStatus.ContainerID != "" {
			cid := t.Status.ContainerStatus.ContainerID
			if len(cid) > 12 {
				cid = cid[:12]
			}
			containerID = cid
		}

		items = append(items, ServiceTaskItem{
			ID:           t.ID,
			Slot:         t.Slot,
			NodeID:       t.NodeID,
			NodeName:     nodeName,
			State:        string(t.Status.State),
			DesiredState: string(t.DesiredState),
			ExitCode:     exitCode,
			Error:        t.Status.Err,
			Timestamp:    t.Status.Timestamp.UTC().Format("2006-01-02T15:04:05Z"),
			ContainerID:  containerID,
		})
	}

	return items, nil
}

type ListServiceTasksHandler struct {
	action *ListServiceTasksAction
}

func NewListServiceTasksHandler(action *ListServiceTasksAction) *ListServiceTasksHandler {
	return &ListServiceTasksHandler{action: action}
}

func (h *ListServiceTasksHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	serviceID := chi.URLParam(r, "serviceID")

	items, err := h.action.Execute(r.Context(), serviceID)
	if err != nil {
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "service not found"})
			return
		}
		log.Printf("list service tasks error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to list service tasks"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"tasks": items})
}
