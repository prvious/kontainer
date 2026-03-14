package websocket

import (
	"net/http"

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

	tail := r.URL.Query().Get("tail")
	if tail == "" {
		tail = "100"
	}
	timestamps := r.URL.Query().Get("timestamps") != "false"

	conn, err := Upgrader.Upgrade(w, r, nil)
	if err != nil {
		facades.Log().Errorf("websocket upgrade error: %v", err)
		return
	}
	defer conn.Close()

	logsReader, err := h.orchestrator.StreamTaskLogs(r.Context(), taskID, docker.LogStreamOptions{
		Follow:     true,
		Tail:       tail,
		Timestamps: timestamps,
	})
	if err != nil {
		facades.Log().Errorf("docker task logs error: %v", err)
		writeJSONError(conn, "failed to start log stream: "+err.Error())
		return
	}

	StreamDockerLogs(conn, logsReader, timestamps)
}
