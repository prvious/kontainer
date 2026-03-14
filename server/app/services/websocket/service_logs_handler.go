package websocket

import (
	"net/http"

	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

// contextKey is an unexported type for context keys in this package.
type contextKey string

const (
	// ServiceIDKey is the context key for serviceID.
	ServiceIDKey contextKey = "serviceID"
	// TaskIDKey is the context key for taskID.
	TaskIDKey contextKey = "taskID"
	// ContainerIDKey is the context key for containerID.
	ContainerIDKey contextKey = "containerID"
)

// ServiceLogsHandler handles WebSocket connections for service log streaming.
type ServiceLogsHandler struct {
	orchestrator docker.Orchestrator
}

func NewServiceLogsHandler(orchestrator docker.Orchestrator) *ServiceLogsHandler {
	return &ServiceLogsHandler{orchestrator: orchestrator}
}

// ServeHTTP is registered as a raw net/http handler.
// JWT auth is validated by the Goravel controller wrapper before this is called.
func (h *ServiceLogsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	serviceID, _ := r.Context().Value(ServiceIDKey).(string)
	if serviceID == "" {
		http.Error(w, "missing serviceID", http.StatusBadRequest)
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

	logsReader, err := h.orchestrator.StreamServiceLogs(r.Context(), serviceID, docker.LogStreamOptions{
		Follow:     true,
		Tail:       tail,
		Timestamps: timestamps,
	})
	if err != nil {
		facades.Log().Errorf("docker service logs error: %v", err)
		writeJSONError(conn, "failed to start log stream: "+err.Error())
		return
	}

	StreamDockerLogs(conn, logsReader, timestamps)
}
