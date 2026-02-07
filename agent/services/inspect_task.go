package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"
	"strings"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

type TaskNetworkInfo struct {
	Name      string   `json:"name"`
	Addresses []string `json:"addresses"`
}

type TaskDetail struct {
	ID           string            `json:"id"`
	ServiceID    string            `json:"serviceId"`
	Slot         int               `json:"slot"`
	NodeID       string            `json:"nodeId"`
	NodeName     string            `json:"nodeName"`
	State        string            `json:"state"`
	DesiredState string            `json:"desiredState"`
	Error        string            `json:"error"`
	Image        string            `json:"image"`
	ContainerID  string            `json:"containerId"`
	PID          int               `json:"pid"`
	ExitCode     *int              `json:"exitCode"`
	CreatedAt    string            `json:"createdAt"`
	UpdatedAt    string            `json:"updatedAt"`
	Env          []EnvVar          `json:"env"`
	Mounts       []MountInfo       `json:"mounts"`
	Networks     []TaskNetworkInfo `json:"networks"`
}

type InspectTaskAction struct {
	client DockerClient
}

func NewInspectTaskAction(client DockerClient) *InspectTaskAction {
	return &InspectTaskAction{client: client}
}

func (a *InspectTaskAction) Execute(ctx context.Context, taskID string) (*TaskDetail, error) {
	task, _, err := a.client.TaskInspectWithRaw(ctx, taskID)
	if err != nil {
		return nil, err
	}

	// Resolve node name
	nodeName := ""
	if task.NodeID != "" {
		nodes, err := a.client.NodeList(ctx, types.NodeListOptions{})
		if err == nil {
			for _, n := range nodes {
				if n.ID == task.NodeID {
					nodeName = n.Description.Hostname
					break
				}
			}
		}
		if nodeName == "" {
			nodeName = task.NodeID[:min(12, len(task.NodeID))]
		}
	}

	// Image
	image := ""
	if task.Spec.ContainerSpec != nil {
		image = task.Spec.ContainerSpec.Image
		if idx := strings.Index(image, "@sha256:"); idx != -1 {
			image = image[:idx]
		}
	}

	// Container status
	containerID := ""
	pid := 0
	var exitCode *int
	if task.Status.ContainerStatus != nil {
		containerID = task.Status.ContainerStatus.ContainerID
		pid = int(task.Status.ContainerStatus.PID)
		ec := task.Status.ContainerStatus.ExitCode
		exitCode = &ec
	}

	// Env vars
	env := make([]EnvVar, 0)
	if task.Spec.ContainerSpec != nil {
		for _, e := range task.Spec.ContainerSpec.Env {
			parts := strings.SplitN(e, "=", 2)
			ev := EnvVar{Key: parts[0]}
			if len(parts) == 2 {
				ev.Value = parts[1]
			}
			env = append(env, ev)
		}
	}

	// Mounts
	mounts := make([]MountInfo, 0)
	if task.Spec.ContainerSpec != nil {
		for _, m := range task.Spec.ContainerSpec.Mounts {
			mounts = append(mounts, MountInfo{
				Type:     string(m.Type),
				Source:   m.Source,
				Target:   m.Target,
				ReadOnly: m.ReadOnly,
			})
		}
	}

	// Networks
	networks := make([]TaskNetworkInfo, 0)
	for _, na := range task.NetworksAttachments {
		name := na.Network.Spec.Name
		if name == "" && len(na.Network.ID) > 0 {
			name = na.Network.ID[:min(12, len(na.Network.ID))]
		}
		networks = append(networks, TaskNetworkInfo{
			Name:      name,
			Addresses: na.Addresses,
		})
	}

	return &TaskDetail{
		ID:           task.ID,
		ServiceID:    task.ServiceID,
		Slot:         task.Slot,
		NodeID:       task.NodeID,
		NodeName:     nodeName,
		State:        string(task.Status.State),
		DesiredState: string(task.DesiredState),
		Error:        task.Status.Err,
		Image:        image,
		ContainerID:  containerID,
		PID:          pid,
		ExitCode:     exitCode,
		CreatedAt:    task.CreatedAt.UTC().Format("2006-01-02T15:04:05Z"),
		UpdatedAt:    task.Status.Timestamp.UTC().Format("2006-01-02T15:04:05Z"),
		Env:          env,
		Mounts:       mounts,
		Networks:     networks,
	}, nil
}

type InspectTaskHandler struct {
	action *InspectTaskAction
}

func NewInspectTaskHandler(action *InspectTaskAction) *InspectTaskHandler {
	return &InspectTaskHandler{action: action}
}

func (h *InspectTaskHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	taskID := chi.URLParam(r, "taskID")

	detail, err := h.action.Execute(r.Context(), taskID)
	if err != nil {
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "task not found"})
			return
		}
		log.Printf("inspect task error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to inspect task"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(detail)
}
