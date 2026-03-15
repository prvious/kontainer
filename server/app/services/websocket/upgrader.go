package websocket

import (
	"encoding/json"
	"net/http"

	gorillaWs "github.com/gorilla/websocket"
	"github.com/prvious/kontainer/server/app/facades"
)

// Context keys for passing route params from Goravel controllers to raw handlers.
type contextKey string

const (
	ServiceIDKey   contextKey = "serviceID"
	TaskIDKey      contextKey = "taskID"
	ContainerIDKey contextKey = "containerID"
)

// Upgrader is the shared WebSocket upgrader for all endpoints.
var Upgrader = gorillaWs.Upgrader{
	CheckOrigin: func(r *http.Request) bool { return true },
}

// writeJSONError sends a JSON error message over the WebSocket connection.
func writeJSONError(conn *gorillaWs.Conn, msg string) {
	data, err := json.Marshal(map[string]string{"error": msg})
	if err != nil {
		facades.Log().Errorf("ws: failed to marshal error message: %v", err)
		return
	}
	if writeErr := conn.WriteMessage(gorillaWs.TextMessage, data); writeErr != nil {
		facades.Log().Errorf("ws: failed to write error to client: %v", writeErr)
	}
}
