package websocket

import (
	"encoding/binary"
	"encoding/json"
	"io"
	"strings"
	"sync"

	gorillaWs "github.com/gorilla/websocket"
	"github.com/goravel/framework/support/carbon"
	"github.com/prvious/kontainer/server/app/facades"
)

// Docker multiplexed stream header byte values.
const (
	dockerStreamStdout byte = 1
	dockerStreamStderr byte = 2
)

// LogMessage is the JSON frame sent to the browser.
type LogMessage struct {
	Stream    string `json:"stream"`
	Timestamp string `json:"timestamp,omitempty"`
	Line      string `json:"line"`
}

// StreamDockerLogs reads from a Docker mux log stream and writes JSON frames to the WS connection.
// Blocks until the stream ends, the WS client disconnects, or done is closed.
func StreamDockerLogs(conn *gorillaWs.Conn, wsMu *sync.Mutex, logsReader io.ReadCloser, timestamps bool) {
	var closeOnce sync.Once
	closeReader := func() { closeOnce.Do(func() { logsReader.Close() }) }
	defer closeReader()

	// Goroutine to detect client disconnect.
	// Closing logsReader unblocks any blocking ReadFull in the main loop.
	done := make(chan struct{})
	go func() {
		defer close(done)
		for {
			if _, _, err := conn.ReadMessage(); err != nil {
				closeReader()
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
			if err != io.EOF && !isClosedError(err) {
				facades.Log().Errorf("log stream read header error: %v", err)
			}
			return
		}

		streamType := "stdout"
		if header[0] == dockerStreamStderr {
			streamType = "stderr"
		}

		payloadSize := binary.BigEndian.Uint32(header[4:8])
		payload := make([]byte, payloadSize)
		_, err = io.ReadFull(logsReader, payload)
		if err != nil {
			if err != io.EOF && !isClosedError(err) {
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

			if timestamps {
				if ts, rest, ok := parseDockerTimestamp(line); ok {
					msg.Timestamp = ts
					msg.Line = rest
				} else {
					msg.Line = line
				}
			} else {
				msg.Line = line
			}

			data, err := json.Marshal(msg)
			if err != nil {
				facades.Log().Errorf("log stream: failed to marshal log message: %v", err)
				continue
			}

			wsMu.Lock()
			writeErr := conn.WriteMessage(gorillaWs.TextMessage, data)
			wsMu.Unlock()
			if writeErr != nil {
				return
			}
		}
	}
}

// parseDockerTimestamp splits a Docker log line into timestamp and content.
// Docker timestamps are RFC3339Nano (e.g. "2025-01-01T00:00:00.000000000Z").
func parseDockerTimestamp(line string) (timestamp, content string, ok bool) {
	idx := strings.IndexByte(line, ' ')
	if idx <= 0 {
		return "", line, false
	}
	candidate := line[:idx]
	if c := carbon.Parse(candidate); c.IsValid() {
		return candidate, line[idx+1:], true
	}
	return "", line, false
}

// isClosedError checks if an error is from reading/writing a closed connection or pipe.
func isClosedError(err error) bool {
	if err == nil {
		return false
	}
	msg := err.Error()
	return strings.Contains(msg, "use of closed network connection") ||
		strings.Contains(msg, "closed pipe")
}
