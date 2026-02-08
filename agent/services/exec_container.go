package services

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"strconv"
	"sync"

	"github.com/clovismuneza/devoper/agent/auth"
	"github.com/clovismuneza/devoper/agent/cluster"
	"github.com/docker/docker/api/types/container"
	"github.com/go-chi/chi/v5"
	"github.com/gorilla/websocket"
)

// Client → Server control messages (text frames only)
type execControlMsg struct {
	Type string `json:"type"`
	Cols uint   `json:"cols,omitempty"`
	Rows uint   `json:"rows,omitempty"`
}

// Server → Client control messages (text frames only; terminal data uses binary frames)
type execStatusMsg struct {
	Type    string `json:"type"`
	Message string `json:"message,omitempty"`
	Code    int    `json:"code,omitempty"`
}

func writeExecError(conn *websocket.Conn, msg string) {
	data, _ := json.Marshal(execStatusMsg{Type: "error", Message: msg})
	conn.WriteMessage(websocket.TextMessage, data)
}

func runExecSession(conn *websocket.Conn, client DockerClient, containerID, shell string, cols, rows uint) {
	ctx := context.Background()

	execResp, err := client.ContainerExecCreate(ctx, containerID, container.ExecOptions{
		Tty:          true,
		AttachStdin:  true,
		AttachStdout: true,
		AttachStderr: true,
		Cmd:          []string{shell},
	})
	if err != nil {
		log.Printf("exec create error: %v", err)
		writeExecError(conn, fmt.Sprintf("failed to create exec: %v", err))
		return
	}

	hijacked, err := client.ContainerExecAttach(ctx, execResp.ID, container.ExecAttachOptions{
		Tty: true,
	})
	if err != nil {
		log.Printf("exec attach error: %v", err)
		writeExecError(conn, fmt.Sprintf("failed to attach exec: %v", err))
		return
	}
	defer hijacked.Close()

	// Set initial terminal size
	if cols > 0 && rows > 0 {
		_ = client.ContainerExecResize(ctx, execResp.ID, container.ResizeOptions{
			Width:  cols,
			Height: rows,
		})
	}

	done := make(chan struct{})
	var once sync.Once
	closeDone := func() { once.Do(func() { close(done) }) }

	// Goroutine 1: Read from WebSocket → write to container stdin
	// Binary frames = raw terminal input, text frames = JSON control messages
	go func() {
		defer closeDone()
		for {
			msgType, msgBytes, err := conn.ReadMessage()
			if err != nil {
				return
			}
			if msgType == websocket.BinaryMessage {
				// Raw terminal input → write directly to container stdin
				if _, err := hijacked.Conn.Write(msgBytes); err != nil {
					return
				}
			} else {
				// Text frame → JSON control message
				var msg execControlMsg
				if err := json.Unmarshal(msgBytes, &msg); err != nil {
					continue
				}
				if msg.Type == "resize" && msg.Cols > 0 && msg.Rows > 0 {
					_ = client.ContainerExecResize(ctx, execResp.ID, container.ResizeOptions{
						Width:  msg.Cols,
						Height: msg.Rows,
					})
				}
			}
		}
	}()

	// Goroutine 2: Read from container stdout → write to WebSocket as binary frames
	go func() {
		defer closeDone()
		buf := make([]byte, 4096)
		for {
			n, err := hijacked.Reader.Read(buf)
			if n > 0 {
				if writeErr := conn.WriteMessage(websocket.BinaryMessage, buf[:n]); writeErr != nil {
					return
				}
			}
			if err != nil {
				if err != io.EOF {
					log.Printf("exec read error: %v", err)
				}
				return
			}
		}
	}()

	<-done

	// Try to get exit code
	inspect, err := client.ContainerExecInspect(ctx, execResp.ID)
	if err == nil && !inspect.Running {
		exitMsg, _ := json.Marshal(execStatusMsg{Type: "exit", Code: inspect.ExitCode})
		conn.WriteMessage(websocket.TextMessage, exitMsg)
	}
}

// ExecContainerHandler handles exec WebSocket connections on the leader.
// It validates JWT from query param and routes to local or follower exec.
type ExecContainerHandler struct {
	client         DockerClient
	jwtSecret      string
	registry       *cluster.Registry
	leaderNodeID   string
	internalSecret string
	upgrader       websocket.Upgrader
}

func NewExecContainerHandler(client DockerClient, jwtSecret string, registry *cluster.Registry, leaderNodeID, internalSecret string) *ExecContainerHandler {
	return &ExecContainerHandler{
		client:         client,
		jwtSecret:      jwtSecret,
		registry:       registry,
		leaderNodeID:   leaderNodeID,
		internalSecret: internalSecret,
		upgrader: websocket.Upgrader{
			CheckOrigin: func(r *http.Request) bool { return true },
		},
	}
}

func (h *ExecContainerHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	// Validate JWT from query param
	tokenStr := r.URL.Query().Get("token")
	if tokenStr == "" {
		http.Error(w, "missing token", http.StatusUnauthorized)
		return
	}
	if _, err := auth.ParseToken(h.jwtSecret, tokenStr); err != nil {
		http.Error(w, "invalid token", http.StatusUnauthorized)
		return
	}

	containerID := chi.URLParam(r, "containerID")
	if containerID == "" {
		http.Error(w, "missing containerID", http.StatusBadRequest)
		return
	}

	nodeID := r.URL.Query().Get("nodeID")
	shell := r.URL.Query().Get("shell")
	if shell == "" {
		shell = "/bin/sh"
	}
	cols := parseUint(r.URL.Query().Get("cols"), 80)
	rows := parseUint(r.URL.Query().Get("rows"), 24)

	conn, err := h.upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("exec websocket upgrade error: %v", err)
		return
	}
	defer conn.Close()

	// Route: local if nodeID matches leader or is empty
	if nodeID == "" || nodeID == h.leaderNodeID {
		runExecSession(conn, h.client, containerID, shell, cols, rows)
		return
	}

	// Route: proxy to follower
	followerAddr, ok := h.registry.ByNodeID(nodeID)
	if !ok {
		writeExecError(conn, fmt.Sprintf("no follower found for node %s", nodeID))
		return
	}

	proxyWebSocket(conn, followerAddr, containerID, shell, h.internalSecret, cols, rows)
}

// ExecContainerFollowerHandler handles exec WebSocket connections on followers.
// Protected by internal auth middleware (X-Devoper-Secret).
type ExecContainerFollowerHandler struct {
	client   DockerClient
	upgrader websocket.Upgrader
}

func NewExecContainerFollowerHandler(client DockerClient) *ExecContainerFollowerHandler {
	return &ExecContainerFollowerHandler{
		client: client,
		upgrader: websocket.Upgrader{
			CheckOrigin: func(r *http.Request) bool { return true },
		},
	}
}

func (h *ExecContainerFollowerHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	containerID := chi.URLParam(r, "containerID")
	if containerID == "" {
		http.Error(w, "missing containerID", http.StatusBadRequest)
		return
	}

	shell := r.URL.Query().Get("shell")
	if shell == "" {
		shell = "/bin/sh"
	}
	cols := parseUint(r.URL.Query().Get("cols"), 80)
	rows := parseUint(r.URL.Query().Get("rows"), 24)

	conn, err := h.upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("follower exec websocket upgrade error: %v", err)
		return
	}
	defer conn.Close()

	runExecSession(conn, h.client, containerID, shell, cols, rows)
}

func parseUint(s string, fallback uint) uint {
	v, err := strconv.ParseUint(s, 10, 32)
	if err != nil {
		return fallback
	}
	return uint(v)
}
