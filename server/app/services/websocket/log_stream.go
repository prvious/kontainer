package websocket

import (
	"encoding/binary"
	"encoding/json"
	"io"
	"strings"

	gorillaWs "github.com/gorilla/websocket"
	"github.com/prvious/kontainer/server/app/facades"
)

// LogMessage is the JSON frame sent to the browser.
type LogMessage struct {
	Stream    string `json:"stream"`
	Timestamp string `json:"timestamp,omitempty"`
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
