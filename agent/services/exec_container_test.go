package services

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/prvious/kontainer/agent/auth"
	"github.com/prvious/kontainer/agent/auth/providers"
	"github.com/prvious/kontainer/agent/cluster"
	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/container"
	"github.com/go-chi/chi/v5"
	"github.com/gorilla/websocket"
)

const testJWTSecret = "test-secret-key"

func createTestToken(t *testing.T) string {
	t.Helper()
	user := &providers.ProviderUser{
		ID:       "user-1",
		Name:     "Test User",
		Email:    "test@example.com",
		Provider: "github",
	}
	token, err := auth.CreateToken(testJWTSecret, user, 5*time.Minute)
	if err != nil {
		t.Fatalf("failed to create test token: %v", err)
	}
	return token
}

// newHijackedResponse creates a types.HijackedResponse backed by an in-memory
// pipe. The returned writer can be used to feed data to the reader side.
func newHijackedResponse() (types.HijackedResponse, *io.PipeWriter) {
	pr, pw := io.Pipe()
	// Create a net.Conn-like pair for the Conn field using net.Pipe.
	serverConn, clientConn := net.Pipe()
	// We use serverConn as the hijacked Conn (stdin writes go here)
	// and clientConn is the other end (we can discard or read from it).
	go func() {
		// Drain clientConn so writes to serverConn don't block.
		io.Copy(io.Discard, clientConn)
	}()
	return types.HijackedResponse{
		Conn:   serverConn,
		Reader: bufio.NewReader(pr),
	}, pw
}

// ── HTTP handler tests (no WebSocket upgrade) ──────────────────────────

func TestExecContainerHandler_MissingToken(t *testing.T) {
	registry := cluster.NewRegistry()
	handler := NewExecContainerHandler(&mockDockerClient{}, testJWTSecret, registry, "leader-node", "secret")

	r := chi.NewRouter()
	r.Get("/exec/{containerID}", handler.ServeHTTP)

	req := httptest.NewRequest("GET", "/exec/abc123", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", w.Code)
	}
	if !strings.Contains(w.Body.String(), "missing token") {
		t.Errorf("expected 'missing token' in body, got: %s", w.Body.String())
	}
}

func TestExecContainerHandler_InvalidToken(t *testing.T) {
	registry := cluster.NewRegistry()
	handler := NewExecContainerHandler(&mockDockerClient{}, testJWTSecret, registry, "leader-node", "secret")

	r := chi.NewRouter()
	r.Get("/exec/{containerID}", handler.ServeHTTP)

	req := httptest.NewRequest("GET", "/exec/abc123?token=bad-jwt-token", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", w.Code)
	}
	if !strings.Contains(w.Body.String(), "invalid token") {
		t.Errorf("expected 'invalid token' in body, got: %s", w.Body.String())
	}
}

func TestExecContainerFollowerHandler_MissingContainerID(t *testing.T) {
	handler := NewExecContainerFollowerHandler(&mockDockerClient{})

	// Use a route with an empty containerID parameter by registering a
	// catch-all and calling with an empty segment.
	r := chi.NewRouter()
	r.Get("/exec/{containerID}", handler.ServeHTTP)

	// Chi interprets "/exec/" as containerID="" only if we explicitly handle it.
	// Instead, call the handler directly with no chi URL param set.
	req := httptest.NewRequest("GET", "/exec/", nil)
	w := httptest.NewRecorder()
	// Invoke the handler directly (no chi context = empty containerID)
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}
	if !strings.Contains(w.Body.String(), "missing containerID") {
		t.Errorf("expected 'missing containerID' in body, got: %s", w.Body.String())
	}
}

// ── WebSocket-level tests ──────────────────────────────────────────────

// setupExecWSServer creates a test HTTP server with the ExecContainerHandler
// wired behind a Chi router, and returns the server plus a dialer function.
func setupExecWSServer(t *testing.T, mock *mockDockerClient) (*httptest.Server, func(containerID string, query string) (*websocket.Conn, *http.Response, error)) {
	t.Helper()
	registry := cluster.NewRegistry()
	handler := NewExecContainerHandler(mock, testJWTSecret, registry, "leader-node", "secret")

	r := chi.NewRouter()
	r.Get("/exec/{containerID}", handler.ServeHTTP)

	srv := httptest.NewServer(r)
	t.Cleanup(srv.Close)

	dialer := func(containerID string, query string) (*websocket.Conn, *http.Response, error) {
		wsURL := "ws" + strings.TrimPrefix(srv.URL, "http") + "/exec/" + containerID + "?" + query
		return websocket.DefaultDialer.Dial(wsURL, nil)
	}

	return srv, dialer
}

func TestRunExecSession_ExecCreateError(t *testing.T) {
	token := createTestToken(t)

	mock := &mockDockerClient{
		execCreateFn: func(_ context.Context, _ string, _ container.ExecOptions) (types.IDResponse, error) {
			return types.IDResponse{}, errors.New("docker daemon unreachable")
		},
	}

	_, dial := setupExecWSServer(t, mock)
	conn, _, err := dial("container-1", "token="+token)
	if err != nil {
		t.Fatalf("dial error: %v", err)
	}
	defer conn.Close()

	// Read the error message sent by the server
	_, msg, err := conn.ReadMessage()
	if err != nil {
		t.Fatalf("read error: %v", err)
	}

	var status execStatusMsg
	if err := json.Unmarshal(msg, &status); err != nil {
		t.Fatalf("unmarshal error: %v", err)
	}
	if status.Type != "error" {
		t.Errorf("expected type 'error', got %q", status.Type)
	}
	if !strings.Contains(status.Message, "failed to create exec") {
		t.Errorf("expected 'failed to create exec' in message, got: %s", status.Message)
	}
}

func TestRunExecSession_NoShellFound(t *testing.T) {
	token := createTestToken(t)

	// Every exec create succeeds, every inspect returns exit code 127 (command not found)
	mock := &mockDockerClient{
		execCreateFn: func(_ context.Context, _ string, _ container.ExecOptions) (types.IDResponse, error) {
			return types.IDResponse{ID: "exec-id"}, nil
		},
		execAttachFn: func(_ context.Context, _ string, _ container.ExecAttachOptions) (types.HijackedResponse, error) {
			hj, pw := newHijackedResponse()
			// Close the writer immediately — shell exited
			pw.Close()
			return hj, nil
		},
		execInspectFn: func(_ context.Context, _ string) (container.ExecInspect, error) {
			return container.ExecInspect{Running: false, ExitCode: 127}, nil
		},
	}

	_, dial := setupExecWSServer(t, mock)
	conn, _, err := dial("container-1", "token="+token+"&shell=/bin/sh")
	if err != nil {
		t.Fatalf("dial error: %v", err)
	}
	defer conn.Close()

	// Read the error message
	_, msg, err := conn.ReadMessage()
	if err != nil {
		t.Fatalf("read error: %v", err)
	}

	var status execStatusMsg
	if err := json.Unmarshal(msg, &status); err != nil {
		t.Fatalf("unmarshal error: %v", err)
	}
	if status.Type != "error" {
		t.Errorf("expected type 'error', got %q", status.Type)
	}
	if !strings.Contains(status.Message, "no shell found") {
		t.Errorf("expected 'no shell found' in message, got: %s", status.Message)
	}
}

func TestRunExecSession_ShellFallback(t *testing.T) {
	token := createTestToken(t)

	var mu sync.Mutex
	shellsAttempted := []string{}
	callCount := 0

	mock := &mockDockerClient{
		execCreateFn: func(_ context.Context, _ string, opts container.ExecOptions) (types.IDResponse, error) {
			mu.Lock()
			callCount++
			currentCall := callCount
			if len(opts.Cmd) > 0 {
				shellsAttempted = append(shellsAttempted, opts.Cmd[0])
			}
			mu.Unlock()
			return types.IDResponse{ID: fmt.Sprintf("exec-%d", currentCall)}, nil
		},
		execAttachFn: func(_ context.Context, _ string, _ container.ExecAttachOptions) (types.HijackedResponse, error) {
			hj, pw := newHijackedResponse()
			// Write some data so the reader has something, then keep it open
			// for the "successful" shell, close immediately for failing ones.
			go func() {
				// Give runExecSession time to inspect
				time.Sleep(100 * time.Millisecond)
				pw.Write([]byte("shell ready\n"))
				// Keep pipe open briefly then close to let the session end
				time.Sleep(100 * time.Millisecond)
				pw.Close()
			}()
			return hj, nil
		},
		execInspectFn: func(_ context.Context, execID string) (container.ExecInspect, error) {
			// First call (exec-1) → exit code 127 (shell not found)
			// Second call (exec-2) → still running (shell found!)
			if execID == "exec-1" {
				return container.ExecInspect{Running: false, ExitCode: 127}, nil
			}
			// For exec-2 during the initial check: running
			// For exec-2 at the end: exited normally
			return container.ExecInspect{Running: true, ExitCode: 0}, nil
		},
	}

	_, dial := setupExecWSServer(t, mock)
	conn, _, err := dial("container-1", "token="+token+"&shell=/bin/zsh")
	if err != nil {
		t.Fatalf("dial error: %v", err)
	}
	defer conn.Close()

	// Read messages until close or timeout
	conn.SetReadDeadline(time.Now().Add(3 * time.Second))
	var receivedData []byte
	for {
		msgType, msg, err := conn.ReadMessage()
		if err != nil {
			break
		}
		if msgType == websocket.BinaryMessage {
			receivedData = append(receivedData, msg...)
		}
	}

	mu.Lock()
	defer mu.Unlock()

	// Should have tried at least 2 shells: /bin/zsh (failed) then fallback
	if len(shellsAttempted) < 2 {
		t.Fatalf("expected at least 2 shell attempts, got %d: %v", len(shellsAttempted), shellsAttempted)
	}
	if shellsAttempted[0] != "/bin/zsh" {
		t.Errorf("expected first shell /bin/zsh, got %s", shellsAttempted[0])
	}
	// Verify we received binary data from the successful shell
	if !bytes.Contains(receivedData, []byte("shell ready")) {
		t.Errorf("expected to receive 'shell ready' data, got: %q", string(receivedData))
	}
}

// ── parseUint helper tests ─────────────────────────────────────────────

func TestParseUint(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		fallback uint
		expected uint
	}{
		{name: "valid number", input: "120", fallback: 80, expected: 120},
		{name: "zero", input: "0", fallback: 80, expected: 0},
		{name: "empty string returns fallback", input: "", fallback: 80, expected: 80},
		{name: "non-numeric returns fallback", input: "abc", fallback: 24, expected: 24},
		{name: "negative returns fallback", input: "-5", fallback: 80, expected: 80},
		{name: "float returns fallback", input: "3.14", fallback: 80, expected: 80},
		{name: "large valid number", input: "4294967295", fallback: 80, expected: 4294967295},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := parseUint(tt.input, tt.fallback)
			if got != tt.expected {
				t.Errorf("parseUint(%q, %d) = %d, want %d", tt.input, tt.fallback, got, tt.expected)
			}
		})
	}
}
