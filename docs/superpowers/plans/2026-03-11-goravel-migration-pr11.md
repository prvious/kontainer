# Goravel Agent Migration: PR 11 (WebSocket Endpoints)

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the three WebSocket endpoints (service logs, task logs, container exec) to Goravel. Goravel has no native WS support, so these use raw `net/http` handlers with `gorilla/websocket`. The exec flow changes: browser → WS → leader → **gRPC bidirectional stream** → follower → Docker exec (replaces the old WS-to-WS proxy).

**Architecture:** WebSocket handlers access `net/http` primitives from Goravel's context via `ctx.Response().Writer()` and `ctx.Request().Origin()`. JWT auth is validated from query param `?token=` before WS upgrade (WS doesn't support Authorization headers). Log streaming reads Docker's mux stream format and sends JSON frames. Exec on remote nodes uses the gRPC `Exec` bidirectional stream from PR 6 instead of proxying WebSocket to follower.

**Tech Stack:** Goravel framework, gorilla/websocket, gRPC streaming (PR 6), Docker SDK (logs, exec)

**Spec:** `docs/superpowers/specs/2026-03-11-goravel-agent-migration-design.md`

**Prerequisites:** PR 5 (Orchestrator), PR 6 (gRPC proto with `Exec` stream), PR 7 (cluster registry + gRPC pool)

---

## Chunk 1: Log Streaming

### File Structure

```
server/app/services/websocket/
├── upgrader.go              # Shared upgrader + auth helper
├── log_stream.go            # Docker mux stream → JSON frames logic
├── service_logs_handler.go  # /api/services/{serviceID}/logs
└── task_logs_handler.go     # /api/tasks/{taskID}/logs
```

### Task 1: WebSocket Upgrader + Auth Helper

Shared utilities for all WS endpoints.

**Files:**
- Create: `server/app/services/websocket/upgrader.go`

- [ ] **Step 1: Create upgrader with JWT auth**

```go
package websocket

import (
	"net/http"

	"github.com/goravel/framework/facades"
	gorillaWs "github.com/gorilla/websocket"
)

// Upgrader is the shared WebSocket upgrader for all endpoints.
var Upgrader = gorillaWs.Upgrader{
	CheckOrigin: func(r *http.Request) bool { return true },
}

// ValidateTokenFromQuery extracts and validates the JWT from the "token" query param.
// Returns nil error if valid.
func ValidateTokenFromQuery(r *http.Request) error {
	tokenStr := r.URL.Query().Get("token")
	if tokenStr == "" {
		return fmt.Errorf("missing token")
	}

	// Use Goravel's auth facade to parse the token
	_, err := facades.Auth().Parse(tokenStr)
	return err
}
```

**Note:** `facades.Auth().Parse(token)` validates the JWT using Goravel's auth system (configured JWT guard). This replaces the manual `auth.ParseToken(jwtSecret, token)` call.

- [ ] **Step 2: Commit**

```bash
cd server && git add app/services/websocket/upgrader.go
git commit -m "feat(websocket): add shared upgrader and JWT auth helper"
```

### Task 2: Docker Mux Stream Parser

Shared logic for parsing Docker's multiplexed log stream format (8-byte header: stream type byte + 3 padding + 4-byte big-endian payload length). Used by both service and task log handlers.

**Files:**
- Create: `server/app/services/websocket/log_stream.go`

- [ ] **Step 1: Create log stream parser**

```go
package websocket

import (
	"encoding/binary"
	"encoding/json"
	"io"
	"strings"

	"github.com/goravel/framework/facades"
	gorillaWs "github.com/gorilla/websocket"
)

// LogMessage is the JSON frame sent to the browser.
type LogMessage struct {
	Stream    string `json:"stream"`
	Timestamp string `json:"timestamp"`
	Line      string `json:"line"`
}

// StreamDockerLogs reads from a Docker mux log stream and writes JSON frames to the WS connection.
// Blocks until the stream ends, the WS client disconnects, or done is closed.
func StreamDockerLogs(conn *gorillaWs.Conn, logsReader io.ReadCloser, timestamps bool) {
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
				facades.Log().Errorf("log stream read header error: %v", err)
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
				facades.Log().Errorf("log stream read payload error: %v", err)
			}
			return
		}

		content := strings.TrimRight(string(payload), "\n")
		lines := strings.Split(content, "\n")

		for _, line := range lines {
			if line == "" {
				continue
			}

			msg := LogMessage{Stream: streamType}

			// Parse timestamp if present (Docker format: "2025-01-01T00:00:00.000000000Z content")
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

			if err := conn.WriteMessage(gorillaWs.TextMessage, data); err != nil {
				return
			}
		}
	}
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/services/websocket/log_stream.go
git commit -m "feat(websocket): add Docker mux stream log parser"
```

### Task 3: Service Logs Handler

Ports `agent/services/stream_service_logs.go`. Validates JWT, upgrades to WS, streams Docker service logs.

**Files:**
- Create: `server/app/services/websocket/service_logs_handler.go`

- [ ] **Step 1: Create service logs handler**

```go
package websocket

import (
	"net/http"

	"github.com/goravel/framework/facades"
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
func (h *ServiceLogsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if err := ValidateTokenFromQuery(r); err != nil {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	// Extract serviceID from Goravel route context.
	// Since this is registered as a raw handler, we need the route param
	// from the URL path. The path format is /api/services/{serviceID}/logs.
	serviceID := extractRouteParam(r, "serviceID")
	if serviceID == "" {
		http.Error(w, "missing serviceID", http.StatusBadRequest)
		return
	}

	tail := r.URL.Query().Get("tail")
	if tail == "" {
		tail = "100"
	}
	timestamps := r.URL.Query().Get("timestamps") != "false"
	stdout := r.URL.Query().Get("stdout") != "false"
	stderr := r.URL.Query().Get("stderr") != "false"

	conn, err := Upgrader.Upgrade(w, r, nil)
	if err != nil {
		facades.Log().Errorf("websocket upgrade error: %v", err)
		return
	}
	defer conn.Close()

	logsReader, err := h.orchestrator.StreamServiceLogs(r.Context(), serviceID, docker.LogOptions{
		Stdout:     stdout,
		Stderr:     stderr,
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
```

**Note:** `extractRouteParam` is a helper that reads the route parameter. When using Goravel's raw handler registration, route params may come from the Goravel context or need to be parsed from the URL path. Implementation depends on how we register the handler — either via Goravel's route `.Any()` with a wrapper, or via direct Chi-compatible registration.

**Preferred approach:** Register the handler via a Goravel controller wrapper:

```go
// In the controller
func (c *ServiceLogsWsController) Handle(ctx http.Context) http.Response {
	serviceID := ctx.Request().Route("serviceID")

	w := ctx.Response().Writer()
	r := ctx.Request().Origin()

	// Pass serviceID via request context for the handler
	r = r.WithContext(context.WithValue(r.Context(), "serviceID", serviceID))

	c.handler.ServeHTTP(w.(http.ResponseWriter), r)
	return nil
}
```

This approach accesses Goravel route params cleanly and passes them to the raw handler.

- [ ] **Step 2: Commit**

```bash
cd server && git add app/services/websocket/service_logs_handler.go
git commit -m "feat(websocket): add service logs streaming handler"
```

### Task 4: Task Logs Handler

Same pattern as service logs, but for tasks. Ports `agent/services/stream_task_logs.go`.

**Files:**
- Create: `server/app/services/websocket/task_logs_handler.go`

- [ ] **Step 1: Create task logs handler**

Nearly identical to service logs. Calls `orchestrator.StreamTaskLogs()` instead.

```go
package websocket

import (
	"net/http"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type TaskLogsHandler struct {
	orchestrator docker.Orchestrator
}

func NewTaskLogsHandler(orchestrator docker.Orchestrator) *TaskLogsHandler {
	return &TaskLogsHandler{orchestrator: orchestrator}
}

func (h *TaskLogsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if err := ValidateTokenFromQuery(r); err != nil {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	taskID := r.Context().Value("taskID").(string)
	if taskID == "" {
		http.Error(w, "missing taskID", http.StatusBadRequest)
		return
	}

	tail := r.URL.Query().Get("tail")
	if tail == "" {
		tail = "100"
	}
	timestamps := r.URL.Query().Get("timestamps") != "false"
	stdout := r.URL.Query().Get("stdout") != "false"
	stderr := r.URL.Query().Get("stderr") != "false"

	conn, err := Upgrader.Upgrade(w, r, nil)
	if err != nil {
		facades.Log().Errorf("websocket upgrade error: %v", err)
		return
	}
	defer conn.Close()

	logsReader, err := h.orchestrator.StreamTaskLogs(r.Context(), taskID, docker.LogOptions{
		Stdout:     stdout,
		Stderr:     stderr,
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
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/services/websocket/task_logs_handler.go
git commit -m "feat(websocket): add task logs streaming handler"
```

---

## Chunk 2: Container Exec

### File Structure

```
server/app/services/websocket/
├── exec_handler.go          # Leader exec handler (local + gRPC proxy)
└── exec_session.go          # Local exec session logic (Docker exec)
```

### Task 5: Local Exec Session

Ports the `runExecSession` function from `agent/services/exec_container.go`. Handles local container exec with shell fallback.

**Files:**
- Create: `server/app/services/websocket/exec_session.go`

- [ ] **Step 1: Create local exec session**

```go
package websocket

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"
	"time"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
	gorillaWs "github.com/gorilla/websocket"
)

type execControlMsg struct {
	Type string `json:"type"`
	Cols uint   `json:"cols,omitempty"`
	Rows uint   `json:"rows,omitempty"`
}

type execStatusMsg struct {
	Type    string `json:"type"`
	Message string `json:"message,omitempty"`
	Code    int    `json:"code,omitempty"`
}

func writeExecMsg(conn *gorillaWs.Conn, mu *sync.Mutex, msg execStatusMsg) {
	data, _ := json.Marshal(msg)
	mu.Lock()
	conn.WriteMessage(gorillaWs.TextMessage, data)
	mu.Unlock()
}

var defaultShells = []string{"/bin/sh", "/bin/bash", "/bin/ash"}

// RunLocalExecSession runs a Docker exec session, relaying I/O between
// the WebSocket connection and the container.
func RunLocalExecSession(
	conn *gorillaWs.Conn,
	wsMu *sync.Mutex,
	orchestrator docker.Orchestrator,
	containerID, shell, user string,
	cols, rows uint,
) {
	ctx := context.Background()

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

	var session docker.ExecSession
	var attached bool

	for _, sh := range shells {
		s, err := orchestrator.ExecContainer(ctx, containerID, docker.ExecOptions{
			Cmd:  []string{sh},
			User: user,
			Cols: cols,
			Rows: rows,
		})
		if err != nil {
			facades.Log().Errorf("exec create error: %v", err)
			writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: fmt.Sprintf("failed to create exec: %v", err)})
			return
		}

		// Wait briefly and check if the shell exists (exit code 127 = not found)
		time.Sleep(50 * time.Millisecond)
		if code, running := s.Inspect(); !running && code == 127 {
			s.Close()
			continue
		}

		session = s
		attached = true
		break
	}

	if !attached {
		writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: "no shell found in container"})
		return
	}
	defer session.Close()

	done := make(chan struct{})
	var once sync.Once
	closeDone := func() { once.Do(func() { close(done) }) }

	// WS → Container stdin
	go func() {
		defer closeDone()
		for {
			msgType, msgBytes, err := conn.ReadMessage()
			if err != nil {
				return
			}
			if msgType == gorillaWs.BinaryMessage {
				if _, err := session.Write(msgBytes); err != nil {
					return
				}
			} else {
				var msg execControlMsg
				if err := json.Unmarshal(msgBytes, &msg); err != nil {
					continue
				}
				if msg.Type == "resize" && msg.Cols > 0 && msg.Rows > 0 {
					session.Resize(msg.Cols, msg.Rows)
				}
			}
		}
	}()

	// Container stdout → WS
	go func() {
		defer closeDone()
		buf := make([]byte, 4096)
		for {
			n, err := session.Read(buf)
			if n > 0 {
				wsMu.Lock()
				writeErr := conn.WriteMessage(gorillaWs.BinaryMessage, buf[:n])
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

	if code, running := session.Inspect(); !running {
		writeExecMsg(conn, wsMu, execStatusMsg{Type: "exit", Code: code})
	}
}

// sendCloseFrame writes a clean WebSocket close frame.
func sendCloseFrame(conn *gorillaWs.Conn) {
	_ = conn.WriteControl(
		gorillaWs.CloseMessage,
		gorillaWs.FormatCloseMessage(gorillaWs.CloseNormalClosure, ""),
		time.Now().Add(2*time.Second),
	)
}

func writeJSONError(conn *gorillaWs.Conn, msg string) {
	data, _ := json.Marshal(map[string]string{"error": msg})
	conn.WriteMessage(gorillaWs.TextMessage, data)
}
```

**Note:** The `docker.ExecSession` interface (from PR 5) abstracts the Docker exec hijacked connection. It provides `Read`, `Write`, `Resize`, `Inspect`, and `Close` methods.

- [ ] **Step 2: Commit**

```bash
cd server && git add app/services/websocket/exec_session.go
git commit -m "feat(websocket): add local exec session with shell fallback"
```

### Task 6: Leader Exec Handler (WS → gRPC Proxy)

Ports `agent/services/exec_container.go` and `agent/services/exec_proxy.go`. On the leader, validates JWT, determines if exec is local or remote. For remote nodes, uses the gRPC `Exec` bidirectional stream instead of WS-to-WS proxy.

**Files:**
- Create: `server/app/services/websocket/exec_handler.go`

- [ ] **Step 1: Create exec handler**

```go
package websocket

import (
	"encoding/json"
	"io"
	"net/http"
	"strconv"
	"sync"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/services/cluster"
	"github.com/prvious/kontainer/server/app/services/docker"
	pb "github.com/prvious/kontainer/server/proto/agent"
	gorillaWs "github.com/gorilla/websocket"
)

type ExecHandler struct {
	orchestrator docker.Orchestrator
	registry     *cluster.Registry
	pool         *cluster.GrpcPool
	leaderNodeID string
}

func NewExecHandler(
	orchestrator docker.Orchestrator,
	registry *cluster.Registry,
	pool *cluster.GrpcPool,
	leaderNodeID string,
) *ExecHandler {
	return &ExecHandler{
		orchestrator: orchestrator,
		registry:     registry,
		pool:         pool,
		leaderNodeID: leaderNodeID,
	}
}

func (h *ExecHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if err := ValidateTokenFromQuery(r); err != nil {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	containerID := r.Context().Value("containerID").(string)
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
	var followerAddr string
	if !isLocal {
		addr, ok := h.registry.ByNodeID(nodeID)
		if !ok {
			facades.Log().Warningf("exec: node %s not in registry, falling back to local", nodeID)
			isLocal = true
		} else {
			followerAddr = addr
		}
	}

	conn, err := Upgrader.Upgrade(w, r, nil)
	if err != nil {
		facades.Log().Errorf("exec websocket upgrade error: %v", err)
		return
	}
	defer conn.Close()

	var wsMu sync.Mutex

	if isLocal {
		RunLocalExecSession(conn, &wsMu, h.orchestrator, containerID, shell, user, cols, rows)
	} else {
		h.proxyViaGrpc(conn, &wsMu, followerAddr, containerID, shell, user, cols, rows)
	}

	sendCloseFrame(conn)
}

// proxyViaGrpc relays exec I/O between the browser WebSocket and a follower's
// gRPC Exec bidirectional stream.
func (h *ExecHandler) proxyViaGrpc(
	conn *gorillaWs.Conn,
	wsMu *sync.Mutex,
	followerAddr, containerID, shell, user string,
	cols, rows uint,
) {
	client, err := h.pool.GetClient(followerAddr)
	if err != nil {
		writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: "failed to connect to follower"})
		return
	}

	stream, err := client.Exec(context.Background())
	if err != nil {
		writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: "failed to start exec stream"})
		return
	}

	// Send initial setup message
	err = stream.Send(&pb.ExecMessage{
		Type:        pb.ExecMessage_INIT,
		ContainerId: containerID,
		Shell:       shell,
		User:        user,
		Cols:        uint32(cols),
		Rows:        uint32(rows),
	})
	if err != nil {
		writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: "failed to init exec"})
		return
	}

	done := make(chan struct{})
	var once sync.Once
	closeDone := func() { once.Do(func() { close(done) }) }

	// WS → gRPC stream
	go func() {
		defer closeDone()
		for {
			msgType, msgBytes, err := conn.ReadMessage()
			if err != nil {
				return
			}
			if msgType == gorillaWs.BinaryMessage {
				if err := stream.Send(&pb.ExecMessage{
					Type: pb.ExecMessage_DATA,
					Data: msgBytes,
				}); err != nil {
					return
				}
			} else {
				var msg execControlMsg
				if err := json.Unmarshal(msgBytes, &msg); err != nil {
					continue
				}
				if msg.Type == "resize" {
					stream.Send(&pb.ExecMessage{
						Type: pb.ExecMessage_RESIZE,
						Cols: uint32(msg.Cols),
						Rows: uint32(msg.Rows),
					})
				}
			}
		}
	}()

	// gRPC stream → WS
	go func() {
		defer closeDone()
		for {
			resp, err := stream.Recv()
			if err != nil {
				if err != io.EOF {
					facades.Log().Errorf("exec grpc recv error: %v", err)
				}
				return
			}
			switch resp.Type {
			case pb.ExecMessage_DATA:
				wsMu.Lock()
				writeErr := conn.WriteMessage(gorillaWs.BinaryMessage, resp.Data)
				wsMu.Unlock()
				if writeErr != nil {
					return
				}
			case pb.ExecMessage_EXIT:
				writeExecMsg(conn, wsMu, execStatusMsg{Type: "exit", Code: int(resp.ExitCode)})
				return
			case pb.ExecMessage_ERROR:
				writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: string(resp.Data)})
				return
			}
		}
	}()

	<-done
	stream.CloseSend()
}

func parseUint(s string, fallback uint) uint {
	v, err := strconv.ParseUint(s, 10, 32)
	if err != nil {
		return fallback
	}
	return uint(v)
}
```

**Key change from old architecture:** The old code used `proxyWebSocket()` which opened a second WS connection to the follower's `/internal/exec/{containerID}`. The new code uses a gRPC bidirectional stream (`AgentService.Exec`), which is more efficient and type-safe.

- [ ] **Step 2: Add missing `context` import and any proto message types**

The `ExecMessage` proto type should have been defined in PR 6's proto file:

```protobuf
message ExecMessage {
  enum Type {
    INIT = 0;
    DATA = 1;
    RESIZE = 2;
    EXIT = 3;
    ERROR = 4;
  }
  Type type = 1;
  bytes data = 2;
  string container_id = 3;
  string shell = 4;
  string user = 5;
  uint32 cols = 6;
  uint32 rows = 7;
  int32 exit_code = 8;
}
```

Verify this is in the PR 6 proto and generated code.

- [ ] **Step 3: Commit**

```bash
cd server && git add app/services/websocket/exec_handler.go
git commit -m "feat(websocket): add exec handler with gRPC proxy for remote nodes"
```

### Task 7: Register WebSocket Routes

Register the three WS endpoints in Goravel routes. Since these are raw `net/http` handlers, we use Goravel controller wrappers that extract route params and delegate.

**Files:**
- Modify: `server/routes/api.go`

- [ ] **Step 1: Create WS controller wrappers**

These sit in the controllers directory and bridge Goravel's context to the raw handlers:

```go
// server/app/http/controllers/websocket/logs_controller.go
package websocket

import (
	"context"
	goHttp "net/http"

	"github.com/goravel/framework/contracts/http"
	ws "github.com/prvious/kontainer/server/app/services/websocket"
)

type ServiceLogsController struct {
	handler *ws.ServiceLogsHandler
}

func NewServiceLogsController(handler *ws.ServiceLogsHandler) *ServiceLogsController {
	return &ServiceLogsController{handler: handler}
}

func (c *ServiceLogsController) Handle(ctx http.Context) http.Response {
	serviceID := ctx.Request().Route("serviceID")
	w := ctx.Response().Writer().(goHttp.ResponseWriter)
	r := ctx.Request().Origin().WithContext(
		context.WithValue(ctx.Request().Origin().Context(), "serviceID", serviceID),
	)
	c.handler.ServeHTTP(w, r)
	return nil
}
```

Same pattern for TaskLogsController and ExecController (passing `taskID` / `containerID` respectively).

- [ ] **Step 2: Add WS routes outside JWT middleware group**

WebSocket routes validate auth via query param token, not the JWT middleware:

```go
// WebSocket routes (self-managed auth via query param)
router.Get("/services/{serviceID}/logs", serviceLogsCtrl.Handle)
router.Get("/tasks/{taskID}/logs", taskLogsCtrl.Handle)
router.Get("/exec/{containerID}", execCtrl.Handle)
```

These go inside the `/api` prefix but **outside** the JWT middleware group (matching the old architecture).

- [ ] **Step 3: Commit**

```bash
cd server && git add app/http/controllers/websocket/ routes/api.go
git commit -m "feat(routes): register WebSocket log and exec routes"
```

### Task 8: Build Verification

- [ ] **Step 1: Verify build**

```bash
cd server && go build ./...
```

- [ ] **Step 2: Final commit**

```bash
git add -A && git commit -m "feat: complete PR 11 — WebSocket endpoints"
```

---

## Summary

| Endpoint | Old Handler | New Handler | Change |
|---|---|---|---|
| `/api/services/{id}/logs` | `StreamServiceLogsHandler` (WS) | `ServiceLogsHandler` (WS) | Uses Orchestrator interface |
| `/api/tasks/{id}/logs` | `StreamTaskLogsHandler` (WS) | `TaskLogsHandler` (WS) | Uses Orchestrator interface |
| `/api/exec/{id}` | `ExecContainerHandler` (WS→WS proxy) | `ExecHandler` (WS→gRPC proxy) | **gRPC replaces WS proxy** |
| (follower) `/internal/exec/{id}` | `ExecContainerFollowerHandler` (WS) | gRPC `Exec` stream handler (PR 6) | **gRPC replaces WS** |

### Key Architectural Change

**Old exec flow (remote node):**
```
Browser → WS → Leader (proxyWebSocket) → WS → Follower → Docker exec
```

**New exec flow (remote node):**
```
Browser → WS → Leader (proxyViaGrpc) → gRPC stream → Follower → Docker exec
```

Benefits:
- No second WebSocket connection to manage
- Type-safe protobuf messages instead of raw byte forwarding
- Shared secret auth via gRPC interceptor (no manual header injection)
- Connection reuse via gRPC pool
