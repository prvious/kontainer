package services

import (
	"bytes"
	"encoding/binary"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/prvious/kontainer/agent/auth"
	"github.com/prvious/kontainer/agent/auth/providers"
	"github.com/go-chi/chi/v5"
	"github.com/gorilla/websocket"
)

const testSecret = "test-secret-key"

func testToken(t *testing.T) string {
	t.Helper()
	token, err := auth.CreateToken(testSecret, &providers.ProviderUser{
		ID:       "1",
		Name:     "Test User",
		Email:    "test@test.com",
		Provider: "github",
	}, time.Hour)
	if err != nil {
		t.Fatalf("failed to create test token: %v", err)
	}
	return token
}

// buildMuxFrame constructs a Docker multiplexed log frame.
// stream: 1 = stdout, 2 = stderr
func buildMuxFrame(stream byte, data string) []byte {
	header := make([]byte, 8)
	header[0] = stream
	binary.BigEndian.PutUint32(header[4:8], uint32(len(data)))
	return append(header, []byte(data)...)
}

func buildMuxStream(frames ...[]byte) io.ReadCloser {
	var buf bytes.Buffer
	for _, f := range frames {
		buf.Write(f)
	}
	return io.NopCloser(&buf)
}

func setupTestServer(mock *mockDockerClient) *httptest.Server {
	handler := NewStreamServiceLogsHandler(mock, testSecret)

	r := chi.NewRouter()
	r.Get("/api/services/{serviceID}/logs", handler.ServeHTTP)

	return httptest.NewServer(r)
}

func dialWS(t *testing.T, serverURL, serviceID, token string, extraParams ...string) (*websocket.Conn, *http.Response, error) {
	t.Helper()
	wsURL := "ws" + strings.TrimPrefix(serverURL, "http") + "/api/services/" + serviceID + "/logs?token=" + token
	for _, p := range extraParams {
		wsURL += "&" + p
	}
	return websocket.DefaultDialer.Dial(wsURL, nil)
}

func TestStreamLogs_Unauthorized_MissingToken(t *testing.T) {
	mock := &mockDockerClient{}
	srv := setupTestServer(mock)
	defer srv.Close()

	wsURL := "ws" + strings.TrimPrefix(srv.URL, "http") + "/api/services/svc1/logs"
	_, resp, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err == nil {
		t.Fatal("expected dial error for missing token")
	}
	if resp != nil && resp.StatusCode != http.StatusUnauthorized {
		t.Errorf("expected 401, got %d", resp.StatusCode)
	}
}

func TestStreamLogs_Unauthorized_InvalidToken(t *testing.T) {
	mock := &mockDockerClient{}
	srv := setupTestServer(mock)
	defer srv.Close()

	_, resp, err := dialWS(t, srv.URL, "svc1", "invalid-token-here")
	if err == nil {
		t.Fatal("expected dial error for invalid token")
	}
	if resp != nil && resp.StatusCode != http.StatusUnauthorized {
		t.Errorf("expected 401, got %d", resp.StatusCode)
	}
}

func TestStreamLogs_StdoutAndStderr(t *testing.T) {
	mock := &mockDockerClient{
		logsReader: buildMuxStream(
			buildMuxFrame(1, "hello stdout\n"),
			buildMuxFrame(2, "hello stderr\n"),
		),
	}
	srv := setupTestServer(mock)
	defer srv.Close()

	conn, _, err := dialWS(t, srv.URL, "svc1", testToken(t), "timestamps=false")
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
	if msg1.Line != "hello stdout" {
		t.Errorf("expected 'hello stdout', got '%s'", msg1.Line)
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
	if msg2.Line != "hello stderr" {
		t.Errorf("expected 'hello stderr', got '%s'", msg2.Line)
	}
}

func TestStreamLogs_WithTimestamps(t *testing.T) {
	ts := "2025-01-15T10:30:00.123456789Z" // 30 chars
	line := ts + " application started\n"
	mock := &mockDockerClient{
		logsReader: buildMuxStream(
			buildMuxFrame(1, line),
		),
	}
	srv := setupTestServer(mock)
	defer srv.Close()

	conn, _, err := dialWS(t, srv.URL, "svc1", testToken(t))
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
	if msg.Line != "application started" {
		t.Errorf("expected 'application started', got '%s'", msg.Line)
	}
}

func TestStreamLogs_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		logsErr: errors.New("service not found"),
	}
	srv := setupTestServer(mock)
	defer srv.Close()

	conn, _, err := dialWS(t, srv.URL, "svc1", testToken(t))
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
	if !strings.Contains(errMsg["error"], "service not found") {
		t.Errorf("expected error containing 'service not found', got '%s'", errMsg["error"])
	}
}

func TestStreamLogs_StreamEnds(t *testing.T) {
	mock := &mockDockerClient{
		logsReader: buildMuxStream(
			buildMuxFrame(1, "last line\n"),
		),
	}
	srv := setupTestServer(mock)
	defer srv.Close()

	conn, _, err := dialWS(t, srv.URL, "svc1", testToken(t), "timestamps=false")
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
