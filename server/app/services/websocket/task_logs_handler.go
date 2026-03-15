package websocket

import (
	"context"
	"net/http"
	"sync"

	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

// TaskLogsHandler handles WebSocket connections for task log streaming.
type TaskLogsHandler struct {
	orchestrator docker.Orchestrator
}

func NewTaskLogsHandler(orchestrator docker.Orchestrator) *TaskLogsHandler {
	return &TaskLogsHandler{orchestrator: orchestrator}
}

// ServeHTTP is registered as a raw net/http handler.
// JWT auth is validated by the Goravel controller wrapper before this is called.
func (h *TaskLogsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	taskID, _ := r.Context().Value(TaskIDKey).(string)
	if taskID == "" {
		http.Error(w, "missing taskID", http.StatusBadRequest)
		return
	}

	tail := clampTail(r.URL.Query().Get("tail"), "100")
	timestamps := r.URL.Query().Get("timestamps") != "false"

	conn, err := Upgrader.Upgrade(w, r, nil)
	if err != nil {
		facades.Log().Errorf("websocket upgrade error: %v", err)
		return
	}
	defer conn.Close()

	// Use a background context — the HTTP request context may be cancelled
	// after the WebSocket upgrade completes, but the log stream should continue.
	logsReader, err := h.orchestrator.StreamTaskLogs(context.Background(), taskID, docker.LogStreamOptions{
		Follow:     true,
		Tail:       tail,
		Timestamps: timestamps,
	})
	if err != nil {
		facades.Log().Errorf("docker task logs error: %v", err)
		writeJSONError(conn, "failed to start log stream: "+err.Error())
		return
	}

	var wsMu sync.Mutex
	StreamDockerLogs(conn, &wsMu, logsReader, timestamps)
}
