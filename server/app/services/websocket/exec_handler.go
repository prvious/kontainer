package websocket

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"regexp"
	"strconv"
	"sync"
	"time"

	gorillaWs "github.com/gorilla/websocket"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/cluster"
	"github.com/prvious/kontainer/server/app/services/docker"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

var validContainerID = regexp.MustCompile(`^[a-zA-Z0-9][a-zA-Z0-9_.-]*$`)

// ExecHandler handles WebSocket connections for container exec sessions.
// It supports both local exec (on the leader node) and remote exec via gRPC proxy.
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

// ServeHTTP is registered as a raw net/http handler.
// JWT auth is validated by the Goravel controller wrapper before this is called.
func (h *ExecHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	containerID, _ := r.Context().Value(ContainerIDKey).(string)
	if containerID == "" {
		http.Error(w, "missing containerID", http.StatusBadRequest)
		return
	}
	if !validContainerID.MatchString(containerID) || len(containerID) > 128 {
		http.Error(w, "invalid containerID", http.StatusBadRequest)
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
			// Node not in registry — report error instead of silently falling back
			conn, err := Upgrader.Upgrade(w, r, nil)
			if err != nil {
				facades.Log().Errorf("exec websocket upgrade error: %v", err)
				return
			}
			defer conn.Close()
			var wsMu sync.Mutex
			writeExecMsg(conn, &wsMu, execStatusMsg{
				Type:    "error",
				Message: fmt.Sprintf("node %s is not available in the cluster", nodeID),
			})
			sendCloseFrame(conn)
			return
		}
		followerAddr = addr
	}

	conn, err := Upgrader.Upgrade(w, r, nil)
	if err != nil {
		facades.Log().Errorf("exec websocket upgrade error: %v", err)
		return
	}
	defer conn.Close()

	var wsMu sync.Mutex

	if isLocal {
		RunLocalExecSession(r.Context(), conn, &wsMu, h.orchestrator, containerID, shell, user, cols, rows)
	} else {
		h.proxyViaGrpc(r.Context(), conn, &wsMu, followerAddr, containerID, shell, user, cols, rows)
	}

	sendCloseFrame(conn)
}

// proxyViaGrpc relays exec I/O between the browser WebSocket and a follower's
// gRPC Exec bidirectional stream.
func (h *ExecHandler) proxyViaGrpc(
	parentCtx context.Context,
	conn *gorillaWs.Conn,
	wsMu *sync.Mutex,
	followerAddr, containerID, shell, user string,
	cols, rows uint,
) {
	client, err := h.pool.GetClient(followerAddr)
	if err != nil {
		facades.Log().Errorf("exec: failed to connect to follower %s for container %s: %v", followerAddr, containerID, err)
		writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: "failed to connect to follower"})
		return
	}

	ctx, cancel := context.WithCancel(parentCtx)
	defer cancel()

	stream, err := client.Exec(ctx)
	if err != nil {
		facades.Log().Errorf("exec: failed to start exec stream on %s for container %s: %v", followerAddr, containerID, err)
		writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: "failed to start exec stream"})
		return
	}

	// Send initial setup message with container ID, shell, user, and terminal dimensions.
	err = stream.Send(&pb.ExecMessage{
		Type:  pb.ExecMessage_INIT,
		Data:  []byte(containerID),
		Shell: shell,
		User:  user,
		Cols:  uint32(cols),
		Rows:  uint32(rows),
	})
	if err != nil {
		facades.Log().Errorf("exec: failed to send INIT to %s for container %s: %v", followerAddr, containerID, err)
		writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: "failed to init exec"})
		return
	}

	done := make(chan struct{})
	var once sync.Once
	closeDone := func() { once.Do(func() { close(done) }) }

	var wg sync.WaitGroup
	wg.Add(2)

	// WS -> gRPC stream
	go func() {
		defer wg.Done()
		defer closeDone()
		for {
			msgType, msgBytes, err := conn.ReadMessage()
			if err != nil {
				if gorillaWs.IsUnexpectedCloseError(err, gorillaWs.CloseNormalClosure, gorillaWs.CloseGoingAway) {
					facades.Log().Warningf("exec: unexpected WS close in gRPC proxy: %v", err)
				}
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
					facades.Log().Warningf("exec: failed to parse control message: %v", err)
					continue
				}
				if msg.Type == "resize" {
					if err := stream.Send(&pb.ExecMessage{
						Type: pb.ExecMessage_RESIZE,
						Cols: uint32(msg.Cols),
						Rows: uint32(msg.Rows),
					}); err != nil {
						facades.Log().Warningf("exec: gRPC resize send failed: %v", err)
					}
				}
			}
		}
	}()

	// gRPC stream -> WS
	go func() {
		defer wg.Done()
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
				writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: resp.Error})
				return
			}
		}
	}()

	<-done

	// Cancel the gRPC context to unblock stream.Recv(), and set a read deadline
	// to unblock conn.ReadMessage(). Then wait for both goroutines to finish.
	cancel()
	conn.SetReadDeadline(time.Now())
	wg.Wait()
	conn.SetReadDeadline(time.Time{})

	if err := stream.CloseSend(); err != nil {
		facades.Log().Warningf("exec: gRPC CloseSend error: %v", err)
	}
}

func parseUint(s string, fallback uint) uint {
	v, err := strconv.ParseUint(s, 10, 32)
	if err != nil {
		return fallback
	}
	return uint(v)
}
