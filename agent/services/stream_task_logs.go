package services

import (
	"encoding/binary"
	"encoding/json"
	"io"
	"log"
	"net/http"
	"strings"

	"github.com/prvious/kontainer/agent/auth"
	"github.com/docker/docker/api/types/container"
	"github.com/go-chi/chi/v5"
	"github.com/gorilla/websocket"
)

type StreamTaskLogsHandler struct {
	client    DockerClient
	jwtSecret string
	upgrader  websocket.Upgrader
}

func NewStreamTaskLogsHandler(client DockerClient, jwtSecret string) *StreamTaskLogsHandler {
	return &StreamTaskLogsHandler{
		client:    client,
		jwtSecret: jwtSecret,
		upgrader: websocket.Upgrader{
			CheckOrigin: func(r *http.Request) bool { return true },
		},
	}
}

func (h *StreamTaskLogsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	// Validate JWT from query param before upgrade
	tokenStr := r.URL.Query().Get("token")
	if tokenStr == "" {
		http.Error(w, "missing token", http.StatusUnauthorized)
		return
	}

	_, err := auth.ParseToken(h.jwtSecret, tokenStr)
	if err != nil {
		http.Error(w, "invalid token", http.StatusUnauthorized)
		return
	}

	taskID := chi.URLParam(r, "taskID")
	if taskID == "" {
		http.Error(w, "missing taskID", http.StatusBadRequest)
		return
	}

	// Parse query params
	tail := r.URL.Query().Get("tail")
	if tail == "" {
		tail = "100"
	}
	timestamps := r.URL.Query().Get("timestamps") != "false"
	stdout := r.URL.Query().Get("stdout") != "false"
	stderr := r.URL.Query().Get("stderr") != "false"

	// Upgrade to WebSocket
	conn, err := h.upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("websocket upgrade error: %v", err)
		return
	}
	defer conn.Close()

	// Start Docker task log stream
	ctx := r.Context()
	logsReader, err := h.client.TaskLogs(ctx, taskID, container.LogsOptions{
		ShowStdout: stdout,
		ShowStderr: stderr,
		Follow:     true,
		Tail:       tail,
		Timestamps: timestamps,
	})
	if err != nil {
		log.Printf("docker task logs error: %v", err)
		writeJSONError(conn, "failed to start log stream: "+err.Error())
		return
	}
	defer logsReader.Close()

	// Goroutine to detect client disconnect
	done := make(chan struct{})
	go func() {
		defer close(done)
		for {
			if _, _, err := conn.ReadMessage(); err != nil {
				return
			}
		}
	}()

	// Read Docker mux stream and send JSON frames
	header := make([]byte, 8)
	for {
		select {
		case <-done:
			return
		default:
		}

		_, err := io.ReadFull(logsReader, header)
		if err != nil {
			if err != io.EOF {
				log.Printf("task log stream read header error: %v", err)
			}
			return
		}

		streamType := "stdout"
		if header[0] == 2 {
			streamType = "stderr"
		}

		payloadSize := binary.BigEndian.Uint32(header[4:8])
		payload := make([]byte, payloadSize)
		_, err = io.ReadFull(logsReader, payload)
		if err != nil {
			if err != io.EOF {
				log.Printf("task log stream read payload error: %v", err)
			}
			return
		}

		// Split payload into lines
		content := strings.TrimRight(string(payload), "\n")
		lines := strings.Split(content, "\n")

		for _, line := range lines {
			if line == "" {
				continue
			}

			msg := LogMessage{
				Stream: streamType,
			}

			if timestamps && len(line) > 30 && line[29] == 'Z' && line[30] == ' ' {
				msg.Timestamp = line[:30]
				msg.Line = line[31:]
			} else {
				msg.Line = line
			}

			data, err := json.Marshal(msg)
			if err != nil {
				continue
			}

			if err := conn.WriteMessage(websocket.TextMessage, data); err != nil {
				return
			}
		}
	}
}
