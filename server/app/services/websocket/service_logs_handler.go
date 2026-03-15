package websocket

import (
	"context"
	"net/http"
	"strconv"
	"sync"

	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
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
	logsReader, err := h.orchestrator.StreamServiceLogs(context.Background(), serviceID, docker.LogStreamOptions{
		Follow:     true,
		Tail:       tail,
		Timestamps: timestamps,
	})
	if err != nil {
		facades.Log().Errorf("docker service logs error: %v", err)
		writeJSONError(conn, "failed to start log stream: "+err.Error())
		return
	}

	var wsMu sync.Mutex
	StreamDockerLogs(conn, &wsMu, logsReader, timestamps)
}

// clampTail validates and clamps the tail parameter to a reasonable range.
func clampTail(raw, fallback string) string {
	if raw == "" || raw == "all" {
		if raw == "all" {
			return raw
		}
		return fallback
	}
	n, err := strconv.Atoi(raw)
	if err != nil || n < 1 {
		return fallback
	}
	if n > 10000 {
		return "10000"
	}
	return raw
}
