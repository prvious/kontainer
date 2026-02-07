package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"
	"sort"
	"strings"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/swarm"
)

type StackItem struct {
	Name            string `json:"name"`
	Services        int    `json:"services"`
	RunningReplicas int    `json:"runningReplicas"`
	DesiredReplicas int    `json:"desiredReplicas"`
	UpdatedAt       string `json:"updatedAt"`
}

type ListStacksAction struct {
	client DockerClient
}

func NewListStacksAction(client DockerClient) *ListStacksAction {
	return &ListStacksAction{client: client}
}

func (a *ListStacksAction) Execute(ctx context.Context) ([]StackItem, error) {
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

	// Group services by stack namespace label
	type stackAgg struct {
		services int
		running  int
		desired  int
		updated  string
	}

	stacks := make(map[string]*stackAgg)

	for _, svc := range svcs {
		stack := svc.Spec.Labels["com.docker.stack.namespace"]
		if stack == "" {
			continue // skip standalone services
		}

		agg, ok := stacks[stack]
		if !ok {
			agg = &stackAgg{}
			stacks[stack] = agg
		}

		agg.services++
		agg.running += runningCount[svc.ID]

		if svc.Spec.Mode.Replicated != nil && svc.Spec.Mode.Replicated.Replicas != nil {
			agg.desired += int(*svc.Spec.Mode.Replicated.Replicas)
		} else if svc.Spec.Mode.Global != nil {
			// For global services, count running tasks as desired
			agg.desired += runningCount[svc.ID]
		}

		updatedAt := svc.UpdatedAt.UTC().Format("2006-01-02T15:04:05Z")
		if updatedAt > agg.updated {
			agg.updated = updatedAt
		}
	}

	items := make([]StackItem, 0, len(stacks))
	for name, agg := range stacks {
		// Clean image sha from name if present (shouldn't be, but defensive)
		cleanName := name
		if idx := strings.Index(cleanName, "@sha256:"); idx != -1 {
			cleanName = cleanName[:idx]
		}

		items = append(items, StackItem{
			Name:            cleanName,
			Services:        agg.services,
			RunningReplicas: agg.running,
			DesiredReplicas: agg.desired,
			UpdatedAt:       agg.updated,
		})
	}

	sort.Slice(items, func(i, j int) bool {
		return items[i].Name < items[j].Name
	})

	return items, nil
}

type ListStacksHandler struct {
	action *ListStacksAction
}

func NewListStacksHandler(action *ListStacksAction) *ListStacksHandler {
	return &ListStacksHandler{action: action}
}

func (h *ListStacksHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	items, err := h.action.Execute(r.Context())
	if err != nil {
		log.Printf("list stacks error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to list stacks"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"stacks": items})
}
