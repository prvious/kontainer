package services

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"strconv"
	"sync"
	"time"

	"github.com/clovismuneza/devoper/agent/auth"
	"github.com/clovismuneza/devoper/agent/cluster"
	"github.com/docker/docker/api/types"
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

func writeExecMsg(conn *websocket.Conn, mu *sync.Mutex, msg execStatusMsg) {
	data, _ := json.Marshal(msg)
	mu.Lock()
	conn.WriteMessage(websocket.TextMessage, data)
	mu.Unlock()
}

// defaultShells is the order in which shells are tried when the requested
// shell is not found (exit code 127).
var defaultShells = []string{"/bin/sh", "/bin/bash", "/bin/ash"}

func runExecSession(conn *websocket.Conn, wsMu *sync.Mutex, client DockerClient, containerID, shell, user string, cols, rows uint) {
	ctx := context.Background()

	// Build list of shells to try.
	// When no shell is specified, try all defaults and let the container decide.
	// When a specific shell is requested, try it first then fall back to defaults.
	var shells []string
	if shell == "" {
		shells = append(shells, defaultShells...)
	} else {
		shells = []string{shell}
		for _, s := range defaultShells {
			if s != shell {
				shells = append(shells, s)
			}
		}
	}

	var execID string
	var hijacked types.HijackedResponse
	var attached bool

	for _, sh := range shells {
		resp, err := client.ContainerExecCreate(ctx, containerID, container.ExecOptions{
			Tty:          true,
			AttachStdin:  true,
			AttachStdout: true,
			AttachStderr: true,
			Cmd:          []string{sh},
			User:         user,
		})
		if err != nil {
			log.Printf("exec create error: %v", err)
			writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: fmt.Sprintf("failed to create exec: %v", err)})
			return
		}

		hj, err := client.ContainerExecAttach(ctx, resp.ID, container.ExecAttachOptions{
			Tty: true,
		})
		if err != nil {
			log.Printf("exec attach error: %v", err)
			writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: fmt.Sprintf("failed to attach exec: %v", err)})
			return
		}

		// Set initial terminal size
		if cols > 0 && rows > 0 {
			_ = client.ContainerExecResize(ctx, resp.ID, container.ResizeOptions{
				Width:  cols,
				Height: rows,
			})
		}

		// Wait briefly and check if the exec exited immediately (command not found).
		time.Sleep(50 * time.Millisecond)
		inspect, err := client.ContainerExecInspect(ctx, resp.ID)
		if err == nil && !inspect.Running && inspect.ExitCode == 127 {
			hj.Close()
			continue
		}

		execID = resp.ID
		hijacked = hj
		attached = true
		break
	}

	if !attached {
		writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: "no shell found in container (tried: " + fmt.Sprintf("%v", shells) + ")"})
		return
	}
	defer hijacked.Close()

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
				if _, err := hijacked.Conn.Write(msgBytes); err != nil {
					return
				}
			} else {
				var msg execControlMsg
				if err := json.Unmarshal(msgBytes, &msg); err != nil {
					continue
				}
				if msg.Type == "resize" && msg.Cols > 0 && msg.Rows > 0 {
					_ = client.ContainerExecResize(ctx, execID, container.ResizeOptions{
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
				wsMu.Lock()
				writeErr := conn.WriteMessage(websocket.BinaryMessage, buf[:n])
				wsMu.Unlock()
				if writeErr != nil {
					return
				}
			}
			if err != nil {
				return
			}
		}
	}()

	<-done

	// Try to get exit code
	inspect, err := client.ContainerExecInspect(ctx, execID)
	if err == nil && !inspect.Running {
		writeExecMsg(conn, wsMu, execStatusMsg{Type: "exit", Code: inspect.ExitCode})
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
	user := r.URL.Query().Get("user")
	cols := parseUint(r.URL.Query().Get("cols"), 80)
	rows := parseUint(r.URL.Query().Get("rows"), 24)

	isLocal := nodeID == "" || nodeID == h.leaderNodeID

	// Pre-validate routing BEFORE upgrading WebSocket so we can return
	// proper HTTP errors instead of upgrading and immediately closing.
	var followerAddr string
	if !isLocal {
		addr, ok := h.registry.ByNodeID(nodeID)
		if !ok {
			// Node ID not in registry — likely a Swarm node ID that doesn't
			// match agent-reported IDs. Fall back to local exec; Docker will
			// return "No such container" if the container isn't here.
			log.Printf("exec: node %s not in registry, falling back to local exec", nodeID)
			isLocal = true
		} else {
			followerAddr = addr
		}
	}

	conn, err := h.upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("exec websocket upgrade error: %v", err)
		return
	}
	defer conn.Close()

	var wsMu sync.Mutex

	if isLocal {
		runExecSession(conn, &wsMu, h.client, containerID, shell, user, cols, rows)
		sendCloseFrame(conn)
		return
	}

	proxyWebSocket(conn, followerAddr, containerID, shell, user, h.internalSecret, cols, rows)
	sendCloseFrame(conn)
}

// sendCloseFrame writes a clean WebSocket close frame so the peer
// (proxy or browser) receives any pending messages before the TCP drop.
func sendCloseFrame(conn *websocket.Conn) {
	_ = conn.WriteControl(
		websocket.CloseMessage,
		websocket.FormatCloseMessage(websocket.CloseNormalClosure, ""),
		time.Now().Add(2*time.Second),
	)
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
	user := r.URL.Query().Get("user")
	cols := parseUint(r.URL.Query().Get("cols"), 80)
	rows := parseUint(r.URL.Query().Get("rows"), 24)

	conn, err := h.upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("follower exec websocket upgrade error: %v", err)
		return
	}
	defer conn.Close()

	var wsMu sync.Mutex
	runExecSession(conn, &wsMu, h.client, containerID, shell, user, cols, rows)
	sendCloseFrame(conn)
}

func parseUint(s string, fallback uint) uint {
	v, err := strconv.ParseUint(s, 10, 32)
	if err != nil {
		return fallback
	}
	return uint(v)
}
