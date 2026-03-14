package websocket

import (
	"encoding/json"
	"net/http"

	gorillaWs "github.com/gorilla/websocket"
)

// Upgrader is the shared WebSocket upgrader for all endpoints.
var Upgrader = gorillaWs.Upgrader{
	CheckOrigin: func(r *http.Request) bool { return true },
}

// writeJSONError sends a JSON error message over the WebSocket connection.
func writeJSONError(conn *gorillaWs.Conn, msg string) {
	data, _ := json.Marshal(map[string]string{"error": msg})
	_ = conn.WriteMessage(gorillaWs.TextMessage, data)
}
