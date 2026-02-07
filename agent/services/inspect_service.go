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

type EnvVar struct {
	Key   string `json:"key"`
	Value string `json:"value"`
}

type MountInfo struct {
	Type     string `json:"type"`
	Source   string `json:"source"`
	Target   string `json:"target"`
	ReadOnly bool   `json:"readOnly"`
}

type PortInfo struct {
	PublishedPort uint32 `json:"publishedPort"`
	TargetPort    uint32 `json:"targetPort"`
	Protocol      string `json:"protocol"`
	PublishMode   string `json:"publishMode"`
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
	Mounts        []MountInfo       `json:"mounts"`
	Constraints   []string          `json:"constraints"`
	Ports         []PortInfo        `json:"ports"`
	UpdateStatus  *UpdateStatusInfo `json:"updateStatus"`
	PreviousImage *string           `json:"previousImage"`
}

type InspectServiceAction struct {
	client DockerClient
}

func NewInspectServiceAction(client DockerClient) *InspectServiceAction {
	return &InspectServiceAction{client: client}
}

func (a *InspectServiceAction) Execute(ctx context.Context, serviceID string) (*ServiceDetail, error) {
	svc, _, err := a.client.ServiceInspectWithRaw(ctx, serviceID, types.ServiceInspectOptions{})
	if err != nil {
		return nil, err
	}

	image := svc.Spec.TaskTemplate.ContainerSpec.Image
	if idx := strings.Index(image, "@sha256:"); idx != -1 {
		image = image[:idx]
	}

	stack := svc.Spec.Labels["com.docker.stack.namespace"]

	mode := "replicated"
	var replicas *int
	if svc.Spec.Mode.Replicated != nil && svc.Spec.Mode.Replicated.Replicas != nil {
		r := int(*svc.Spec.Mode.Replicated.Replicas)
		replicas = &r
	} else if svc.Spec.Mode.Global != nil {
		mode = "global"
	}

	// Parse env vars
	env := make([]EnvVar, 0)
	for _, e := range svc.Spec.TaskTemplate.ContainerSpec.Env {
		parts := strings.SplitN(e, "=", 2)
		ev := EnvVar{Key: parts[0]}
		if len(parts) == 2 {
			ev.Value = parts[1]
		}
		env = append(env, ev)
	}

	// Parse mounts
	mounts := make([]MountInfo, 0)
	for _, m := range svc.Spec.TaskTemplate.ContainerSpec.Mounts {
		mounts = append(mounts, MountInfo{
			Type:     string(m.Type),
			Source:   m.Source,
			Target:   m.Target,
			ReadOnly: m.ReadOnly,
		})
	}

	// Parse constraints
	constraints := make([]string, 0)
	if svc.Spec.TaskTemplate.Placement != nil {
		constraints = append(constraints, svc.Spec.TaskTemplate.Placement.Constraints...)
	}

	// Parse ports
	ports := make([]PortInfo, 0)
	if svc.Endpoint.Ports != nil {
		for _, p := range svc.Endpoint.Ports {
			ports = append(ports, PortInfo{
				PublishedPort: p.PublishedPort,
				TargetPort:    p.TargetPort,
				Protocol:      string(p.Protocol),
				PublishMode:   string(p.PublishMode),
			})
		}
	}

	// Parse update status
	var updateStatus *UpdateStatusInfo
	if svc.UpdateStatus != nil && svc.UpdateStatus.State != "" {
		us := UpdateStatusInfo{
			State:   string(svc.UpdateStatus.State),
			Message: svc.UpdateStatus.Message,
		}
		if svc.UpdateStatus.StartedAt != nil {
			us.StartedAt = svc.UpdateStatus.StartedAt.UTC().Format("2006-01-02T15:04:05Z")
		}
		if svc.UpdateStatus.CompletedAt != nil {
			us.CompletedAt = svc.UpdateStatus.CompletedAt.UTC().Format("2006-01-02T15:04:05Z")
		}
		updateStatus = &us
	}

	// Extract previous image if a previous spec exists
	var previousImage *string
	if svc.PreviousSpec != nil && svc.PreviousSpec.TaskTemplate.ContainerSpec != nil {
		prev := svc.PreviousSpec.TaskTemplate.ContainerSpec.Image
		if idx := strings.Index(prev, "@sha256:"); idx != -1 {
			prev = prev[:idx]
		}
		if prev != image {
			previousImage = &prev
		}
	}

	return &ServiceDetail{
		ID:            svc.ID,
		Name:          svc.Spec.Name,
		Image:         image,
		Stack:         stack,
		Mode:          mode,
		Replicas:      replicas,
		CreatedAt:     svc.CreatedAt.UTC().Format("2006-01-02T15:04:05Z"),
		UpdatedAt:     svc.UpdatedAt.UTC().Format("2006-01-02T15:04:05Z"),
		Env:           env,
		Mounts:        mounts,
		Constraints:   constraints,
		Ports:         ports,
		UpdateStatus:  updateStatus,
		PreviousImage: previousImage,
	}, nil
}

type InspectServiceHandler struct {
	action *InspectServiceAction
}

func NewInspectServiceHandler(action *InspectServiceAction) *InspectServiceHandler {
	return &InspectServiceHandler{action: action}
}

func (h *InspectServiceHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	serviceID := chi.URLParam(r, "serviceID")

	detail, err := h.action.Execute(r.Context(), serviceID)
	if err != nil {
		if errdefs.IsNotFound(err) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "service not found"})
			return
		}
		log.Printf("inspect service error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to inspect service"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(detail)
}
