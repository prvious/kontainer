package services

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/gorilla/websocket"
)

func setupTaskLogsTestServer(mock *mockDockerClient) *httptest.Server {
	handler := NewStreamTaskLogsHandler(mock, testSecret)

	r := chi.NewRouter()
	r.Get("/api/tasks/{taskID}/logs", handler.ServeHTTP)

	return httptest.NewServer(r)
}

func dialTaskWS(t *testing.T, serverURL, taskID, token string, extraParams ...string) (*websocket.Conn, *http.Response, error) {
	t.Helper()
	wsURL := "ws" + strings.TrimPrefix(serverURL, "http") + "/api/tasks/" + taskID + "/logs?token=" + token
	for _, p := range extraParams {
		wsURL += "&" + p
	}
	return websocket.DefaultDialer.Dial(wsURL, nil)
}

func TestStreamTaskLogs_Unauthorized_MissingToken(t *testing.T) {
	mock := &mockDockerClient{}
	srv := setupTaskLogsTestServer(mock)
	defer srv.Close()

	wsURL := "ws" + strings.TrimPrefix(srv.URL, "http") + "/api/tasks/task1/logs"
	_, resp, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err == nil {
		t.Fatal("expected dial error for missing token")
	}
	if resp != nil && resp.StatusCode != http.StatusUnauthorized {
		t.Errorf("expected 401, got %d", resp.StatusCode)
	}
}

func TestStreamTaskLogs_Unauthorized_InvalidToken(t *testing.T) {
	mock := &mockDockerClient{}
	srv := setupTaskLogsTestServer(mock)
	defer srv.Close()

	_, resp, err := dialTaskWS(t, srv.URL, "task1", "invalid-token-here")
	if err == nil {
		t.Fatal("expected dial error for invalid token")
	}
	if resp != nil && resp.StatusCode != http.StatusUnauthorized {
		t.Errorf("expected 401, got %d", resp.StatusCode)
	}
}

func TestStreamTaskLogs_StdoutAndStderr(t *testing.T) {
	mock := &mockDockerClient{
		taskLogsReader: buildMuxStream(
			buildMuxFrame(1, "task stdout\n"),
			buildMuxFrame(2, "task stderr\n"),
		),
	}
	srv := setupTaskLogsTestServer(mock)
	defer srv.Close()

	conn, _, err := dialTaskWS(t, srv.URL, "task1", testToken(t), "timestamps=false")
	if err != nil {
		t.Fatalf("dial error: %v", err)
	}
	defer conn.Close()

	// Read stdout message
	var msg1 LogMessage
	_, data1, err := conn.ReadMessage()
	if err != nil {
		t.Fatalf("read error: %v", err)
	}
	if err := json.Unmarshal(data1, &msg1); err != nil {
		t.Fatalf("unmarshal error: %v", err)
	}
	if msg1.Stream != "stdout" {
		t.Errorf("expected stdout, got %s", msg1.Stream)
	}
	if msg1.Line != "task stdout" {
		t.Errorf("expected 'task stdout', got '%s'", msg1.Line)
	}

	// Read stderr message
	var msg2 LogMessage
	_, data2, err := conn.ReadMessage()
	if err != nil {
		t.Fatalf("read error: %v", err)
	}
	if err := json.Unmarshal(data2, &msg2); err != nil {
		t.Fatalf("unmarshal error: %v", err)
	}
	if msg2.Stream != "stderr" {
		t.Errorf("expected stderr, got %s", msg2.Stream)
	}
	if msg2.Line != "task stderr" {
		t.Errorf("expected 'task stderr', got '%s'", msg2.Line)
	}
}

func TestStreamTaskLogs_WithTimestamps(t *testing.T) {
	ts := "2025-01-15T10:30:00.123456789Z"
	line := ts + " container started\n"
	mock := &mockDockerClient{
		taskLogsReader: buildMuxStream(
			buildMuxFrame(1, line),
		),
	}
	srv := setupTaskLogsTestServer(mock)
	defer srv.Close()

	conn, _, err := dialTaskWS(t, srv.URL, "task1", testToken(t))
	if err != nil {
		t.Fatalf("dial error: %v", err)
	}
	defer conn.Close()

	var msg LogMessage
	_, data, err := conn.ReadMessage()
	if err != nil {
		t.Fatalf("read error: %v", err)
	}
	if err := json.Unmarshal(data, &msg); err != nil {
		t.Fatalf("unmarshal error: %v", err)
	}
	if msg.Timestamp != ts {
		t.Errorf("expected timestamp '%s', got '%s'", ts, msg.Timestamp)
	}
	if msg.Line != "container started" {
		t.Errorf("expected 'container started', got '%s'", msg.Line)
	}
}

func TestStreamTaskLogs_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		taskLogsErr: errors.New("task not found"),
	}
	srv := setupTaskLogsTestServer(mock)
	defer srv.Close()

	conn, _, err := dialTaskWS(t, srv.URL, "task1", testToken(t))
	if err != nil {
		t.Fatalf("dial error: %v", err)
	}
	defer conn.Close()

	_, data, err := conn.ReadMessage()
	if err != nil {
		t.Fatalf("read error: %v", err)
	}

	var errMsg map[string]string
	if err := json.Unmarshal(data, &errMsg); err != nil {
		t.Fatalf("unmarshal error: %v", err)
	}
	if !strings.Contains(errMsg["error"], "task not found") {
		t.Errorf("expected error containing 'task not found', got '%s'", errMsg["error"])
	}
}

func TestStreamTaskLogs_StreamEnds(t *testing.T) {
	mock := &mockDockerClient{
		taskLogsReader: buildMuxStream(
			buildMuxFrame(1, "last task line\n"),
		),
	}
	srv := setupTaskLogsTestServer(mock)
	defer srv.Close()

	conn, _, err := dialTaskWS(t, srv.URL, "task1", testToken(t), "timestamps=false")
	if err != nil {
		t.Fatalf("dial error: %v", err)
	}
	defer conn.Close()

	// Read the one message
	_, _, err = conn.ReadMessage()
	if err != nil {
		t.Fatalf("read error: %v", err)
	}

	// Next read should indicate close
	conn.SetReadDeadline(time.Now().Add(2 * time.Second))
	_, _, err = conn.ReadMessage()
	if err == nil {
		t.Fatal("expected error on read after stream ends")
	}
}
