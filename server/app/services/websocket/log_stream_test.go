package websocket

import (
	"bytes"
	"encoding/binary"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"

	gorillaWs "github.com/gorilla/websocket"
)

// buildMuxFrame builds a Docker multiplexed log stream frame.
func buildMuxFrame(streamType byte, payload string) []byte {
	header := make([]byte, 8)
	header[0] = streamType
	binary.BigEndian.PutUint32(header[4:8], uint32(len(payload)))
	return append(header, []byte(payload)...)
}

// setupWSPair creates a connected WebSocket client+server pair for testing.
func setupWSPair(t *testing.T) (clientConn *gorillaWs.Conn, serverConn *gorillaWs.Conn) {
	t.Helper()
	upgrader := gorillaWs.Upgrader{CheckOrigin: func(r *http.Request) bool { return true }}
	serverDone := make(chan struct{})

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		c, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			t.Errorf("server upgrade: %v", err)
			return
		}
		serverConn = c
		close(serverDone)
		// Keep handler alive until test finishes
		select {}
	}))
	t.Cleanup(server.Close)

	url := "ws" + strings.TrimPrefix(server.URL, "http")
	client, _, err := gorillaWs.DefaultDialer.Dial(url, nil)
	if err != nil {
		t.Fatalf("client dial: %v", err)
	}
	t.Cleanup(func() { client.Close() })

	<-serverDone
	return client, serverConn
}

func TestStreamDockerLogs_StdoutFrame(t *testing.T) {
	client, server := setupWSPair(t)

	frame := buildMuxFrame(1, "hello world\n")
	reader := io.NopCloser(bytes.NewReader(frame))

	var wsMu sync.Mutex
	go StreamDockerLogs(server, &wsMu, reader, false)

	_, msg, err := client.ReadMessage()
	if err != nil {
		t.Fatalf("read: %v", err)
	}

	var logMsg LogMessage
	if err := json.Unmarshal(msg, &logMsg); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if logMsg.Stream != "stdout" {
		t.Errorf("stream = %q, want stdout", logMsg.Stream)
	}
	if logMsg.Line != "hello world" {
		t.Errorf("line = %q, want 'hello world'", logMsg.Line)
	}
	if logMsg.Timestamp != "" {
		t.Errorf("timestamp = %q, want empty", logMsg.Timestamp)
	}
}

func TestStreamDockerLogs_StderrFrame(t *testing.T) {
	client, server := setupWSPair(t)

	frame := buildMuxFrame(2, "error output\n")
	reader := io.NopCloser(bytes.NewReader(frame))

	var wsMu sync.Mutex
	go StreamDockerLogs(server, &wsMu, reader, false)

	_, msg, err := client.ReadMessage()
	if err != nil {
		t.Fatalf("read: %v", err)
	}

	var logMsg LogMessage
	json.Unmarshal(msg, &logMsg)
	if logMsg.Stream != "stderr" {
		t.Errorf("stream = %q, want stderr", logMsg.Stream)
	}
	if logMsg.Line != "error output" {
		t.Errorf("line = %q, want 'error output'", logMsg.Line)
	}
}

func TestStreamDockerLogs_MultiLinePayload(t *testing.T) {
	client, server := setupWSPair(t)

	frame := buildMuxFrame(1, "line1\nline2\nline3\n")
	reader := io.NopCloser(bytes.NewReader(frame))

	var wsMu sync.Mutex
	go StreamDockerLogs(server, &wsMu, reader, false)

	var lines []string
	for i := 0; i < 3; i++ {
		_, msg, err := client.ReadMessage()
		if err != nil {
			t.Fatalf("read[%d]: %v", i, err)
		}
		var logMsg LogMessage
		json.Unmarshal(msg, &logMsg)
		lines = append(lines, logMsg.Line)
	}

	expected := []string{"line1", "line2", "line3"}
	for i, want := range expected {
		if lines[i] != want {
			t.Errorf("lines[%d] = %q, want %q", i, lines[i], want)
		}
	}
}

func TestStreamDockerLogs_MultipleFrames(t *testing.T) {
	client, server := setupWSPair(t)

	var frames []byte
	frames = append(frames, buildMuxFrame(1, "stdout line\n")...)
	frames = append(frames, buildMuxFrame(2, "stderr line\n")...)
	reader := io.NopCloser(bytes.NewReader(frames))

	var wsMu sync.Mutex
	go StreamDockerLogs(server, &wsMu, reader, false)

	_, msg1, _ := client.ReadMessage()
	var log1 LogMessage
	json.Unmarshal(msg1, &log1)
	if log1.Stream != "stdout" || log1.Line != "stdout line" {
		t.Errorf("msg1 = %+v, want stdout/stdout line", log1)
	}

	_, msg2, _ := client.ReadMessage()
	var log2 LogMessage
	json.Unmarshal(msg2, &log2)
	if log2.Stream != "stderr" || log2.Line != "stderr line" {
		t.Errorf("msg2 = %+v, want stderr/stderr line", log2)
	}
}

func TestStreamDockerLogs_WithTimestamps(t *testing.T) {
	client, server := setupWSPair(t)

	logLine := "2025-01-15T10:30:00.123456789Z hello from docker\n"
	frame := buildMuxFrame(1, logLine)
	reader := io.NopCloser(bytes.NewReader(frame))

	var wsMu sync.Mutex
	go StreamDockerLogs(server, &wsMu, reader, true)

	_, msg, _ := client.ReadMessage()
	var logMsg LogMessage
	json.Unmarshal(msg, &logMsg)
	if logMsg.Timestamp != "2025-01-15T10:30:00.123456789Z" {
		t.Errorf("timestamp = %q, want '2025-01-15T10:30:00.123456789Z'", logMsg.Timestamp)
	}
	if logMsg.Line != "hello from docker" {
		t.Errorf("line = %q, want 'hello from docker'", logMsg.Line)
	}
}

func TestStreamDockerLogs_WithShortTimestamp(t *testing.T) {
	client, server := setupWSPair(t)

	// Fewer fractional digits — the old hardcoded parser would fail here
	logLine := "2025-01-15T10:30:00.123Z short timestamp content\n"
	frame := buildMuxFrame(1, logLine)
	reader := io.NopCloser(bytes.NewReader(frame))

	var wsMu sync.Mutex
	go StreamDockerLogs(server, &wsMu, reader, true)

	_, msg, _ := client.ReadMessage()
	var logMsg LogMessage
	json.Unmarshal(msg, &logMsg)
	if logMsg.Timestamp != "2025-01-15T10:30:00.123Z" {
		t.Errorf("timestamp = %q, want '2025-01-15T10:30:00.123Z'", logMsg.Timestamp)
	}
	if logMsg.Line != "short timestamp content" {
		t.Errorf("line = %q, want 'short timestamp content'", logMsg.Line)
	}
}

func TestStreamDockerLogs_TimestampsDisabledDoesNotParse(t *testing.T) {
	client, server := setupWSPair(t)

	logLine := "2025-01-15T10:30:00.123456789Z hello\n"
	frame := buildMuxFrame(1, logLine)
	reader := io.NopCloser(bytes.NewReader(frame))

	var wsMu sync.Mutex
	go StreamDockerLogs(server, &wsMu, reader, false)

	_, msg, _ := client.ReadMessage()
	var logMsg LogMessage
	json.Unmarshal(msg, &logMsg)
	if logMsg.Timestamp != "" {
		t.Errorf("timestamp = %q, want empty (timestamps disabled)", logMsg.Timestamp)
	}
	if logMsg.Line != "2025-01-15T10:30:00.123456789Z hello" {
		t.Errorf("line = %q, want full line with timestamp", logMsg.Line)
	}
}

func TestStreamDockerLogs_EmptyLinesSkipped(t *testing.T) {
	client, server := setupWSPair(t)

	frame := buildMuxFrame(1, "line1\n\n\nline2\n")
	reader := io.NopCloser(bytes.NewReader(frame))

	var wsMu sync.Mutex
	go StreamDockerLogs(server, &wsMu, reader, false)

	_, msg1, _ := client.ReadMessage()
	_, msg2, _ := client.ReadMessage()
	var log1, log2 LogMessage
	json.Unmarshal(msg1, &log1)
	json.Unmarshal(msg2, &log2)

	if log1.Line != "line1" {
		t.Errorf("msg1.Line = %q, want 'line1'", log1.Line)
	}
	if log2.Line != "line2" {
		t.Errorf("msg2.Line = %q, want 'line2'", log2.Line)
	}
}

func TestParseDockerTimestamp(t *testing.T) {
	tests := []struct {
		name      string
		line      string
		wantTS    string
		wantLine  string
		wantFound bool
	}{
		{
			"RFC3339Nano full",
			"2025-01-15T10:30:00.123456789Z content here",
			"2025-01-15T10:30:00.123456789Z", "content here", true,
		},
		{
			"RFC3339Nano short fractional",
			"2025-01-15T10:30:00.12Z short",
			"2025-01-15T10:30:00.12Z", "short", true,
		},
		{
			"RFC3339 no fractional",
			"2025-01-15T10:30:00Z no fraction",
			"2025-01-15T10:30:00Z", "no fraction", true,
		},
		{
			"not a timestamp",
			"just a regular log line",
			"", "just a regular log line", false,
		},
		{
			"no space separator",
			"2025-01-15T10:30:00Z",
			"", "2025-01-15T10:30:00Z", false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ts, content, ok := parseDockerTimestamp(tt.line)
			if ok != tt.wantFound {
				t.Errorf("ok = %v, want %v", ok, tt.wantFound)
			}
			if ts != tt.wantTS {
				t.Errorf("timestamp = %q, want %q", ts, tt.wantTS)
			}
			if content != tt.wantLine {
				t.Errorf("content = %q, want %q", content, tt.wantLine)
			}
		})
	}
}
