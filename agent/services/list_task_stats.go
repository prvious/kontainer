package services

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/swarm"
)

type TaskStats struct {
	Total          int `json:"total"`
	Running        int `json:"running"`
	Failed         int `json:"failed"`
	Preparing      int `json:"preparing"`
	Complete       int `json:"complete"`
	DesiredRunning int `json:"desiredRunning"`
}

type ListTaskStatsAction struct {
	client DockerClient
}

func NewListTaskStatsAction(client DockerClient) *ListTaskStatsAction {
	return &ListTaskStatsAction{client: client}
}

func (a *ListTaskStatsAction) Execute(ctx context.Context) (TaskStats, error) {
	tasks, err := a.client.TaskList(ctx, types.TaskListOptions{})
	if err != nil {
		return TaskStats{}, err
	}

	var stats TaskStats
	stats.Total = len(tasks)

	for _, t := range tasks {
		switch t.Status.State {
		case swarm.TaskStateRunning:
			stats.Running++
		case swarm.TaskStateFailed:
			stats.Failed++
		case swarm.TaskStateComplete:
			stats.Complete++
		case swarm.TaskStateNew,
			swarm.TaskStatePending,
			swarm.TaskStateAssigned,
			swarm.TaskStateAccepted,
			swarm.TaskStateReady,
			swarm.TaskStatePreparing,
			swarm.TaskStateStarting,
			swarm.TaskStateAllocated:
			stats.Preparing++
		}

		if t.DesiredState == swarm.TaskStateRunning {
			stats.DesiredRunning++
		}
	}

	return stats, nil
}

type ListTaskStatsHandler struct {
	action *ListTaskStatsAction
}

func NewListTaskStatsHandler(action *ListTaskStatsAction) *ListTaskStatsHandler {
	return &ListTaskStatsHandler{action: action}
}

func (h *ListTaskStatsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	stats, err := h.action.Execute(r.Context())
	if err != nil {
		log.Printf("list task stats error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to list task stats"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"taskStats": stats})
}
