package services

import (
	"context"
	"encoding/json"
	"errors"
	"log"
	"net/http"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/filters"
	"github.com/go-chi/chi/v5"
)

var errNoServicesInStack = errors.New("no services found for stack")

type RemoveStackAction struct {
	client DockerClient
}

func NewRemoveStackAction(client DockerClient) *RemoveStackAction {
	return &RemoveStackAction{client: client}
}

func (a *RemoveStackAction) Execute(ctx context.Context, stackName string) (int, error) {
	filter := filters.NewArgs()
	filter.Add("label", "com.docker.stack.namespace="+stackName)

	svcs, err := a.client.ServiceList(ctx, types.ServiceListOptions{Filters: filter})
	if err != nil {
		return 0, err
	}

	if len(svcs) == 0 {
		return 0, errNoServicesInStack
	}

	removed := 0
	for _, svc := range svcs {
		if err := a.client.ServiceRemove(ctx, svc.ID); err != nil {
			return removed, err
		}
		removed++
	}

	return removed, nil
}

type RemoveStackHandler struct {
	action *RemoveStackAction
}

func NewRemoveStackHandler(action *RemoveStackAction) *RemoveStackHandler {
	return &RemoveStackHandler{action: action}
}

func (h *RemoveStackHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	stackName := chi.URLParam(r, "stackName")

	removed, err := h.action.Execute(r.Context(), stackName)
	if err != nil {
		if errors.Is(err, errNoServicesInStack) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"error": "no services found for stack"})
			return
		}
		log.Printf("remove stack error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to remove stack"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"removed": removed})
}
