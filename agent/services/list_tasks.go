package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"
	"sort"

	"github.com/docker/docker/api/types"
)

type TaskItem struct {
	ID           string `json:"id"`
	ServiceID    string `json:"serviceId"`
	ServiceName  string `json:"serviceName"`
	Slot         int    `json:"slot"`
	NodeID       string `json:"nodeId"`
	NodeName     string `json:"nodeName"`
	State        string `json:"state"`
	DesiredState string `json:"desiredState"`
	ExitCode     *int   `json:"exitCode"`
	Error        string `json:"error"`
	Timestamp    string `json:"timestamp"`
	ContainerID  string `json:"containerId"`
	Image        string `json:"image"`
}

type ListTasksAction struct {
	client DockerClient
}

func NewListTasksAction(client DockerClient) *ListTasksAction {
	return &ListTasksAction{client: client}
}

func (a *ListTasksAction) Execute(ctx context.Context) ([]TaskItem, error) {
	// List all tasks
	tasks, err := a.client.TaskList(ctx, types.TaskListOptions{})
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

	// Resolve service names
	serviceNames := make(map[string]string)
	services, err := a.client.ServiceList(ctx, types.ServiceListOptions{})
	if err == nil {
		for _, s := range services {
			serviceNames[s.ID] = s.Spec.Name
		}
	}

	// Sort tasks: most recent first
	sort.Slice(tasks, func(i, j int) bool {
		return tasks[i].Status.Timestamp.After(tasks[j].Status.Timestamp)
	})

	items := make([]TaskItem, 0, len(tasks))
	for _, t := range tasks {
		nodeName := nodeNames[t.NodeID]
		if nodeName == "" && t.NodeID != "" {
			nodeName = t.NodeID[:min(12, len(t.NodeID))]
		}

		serviceName := serviceNames[t.ServiceID]
		if serviceName == "" && t.ServiceID != "" {
			serviceName = t.ServiceID[:min(12, len(t.ServiceID))]
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

		image := ""
		if t.Spec.ContainerSpec != nil {
			image = t.Spec.ContainerSpec.Image
			// Extract image name without digest for readability
			if idx := len(image); idx > 0 {
				// Remove @sha256:... suffix if present
				for i := 0; i < len(image); i++ {
					if image[i] == '@' {
						image = image[:i]
						break
					}
				}
			}
		}

		items = append(items, TaskItem{
			ID:           t.ID,
			ServiceID:    t.ServiceID,
			ServiceName:  serviceName,
			Slot:         t.Slot,
			NodeID:       t.NodeID,
			NodeName:     nodeName,
			State:        string(t.Status.State),
			DesiredState: string(t.DesiredState),
			ExitCode:     exitCode,
			Error:        t.Status.Err,
			Timestamp:    t.Status.Timestamp.UTC().Format("2006-01-02T15:04:05Z"),
			ContainerID:  containerID,
			Image:        image,
		})
	}

	return items, nil
}

type ListTasksHandler struct {
	action *ListTasksAction
}

func NewListTasksHandler(action *ListTasksAction) *ListTasksHandler {
	return &ListTasksHandler{action: action}
}

func (h *ListTasksHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	items, err := h.action.Execute(r.Context())
	if err != nil {
		log.Printf("list tasks error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to list tasks"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"tasks": items})
}
