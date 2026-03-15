package websocket

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"
	"time"

	gorillaWs "github.com/gorilla/websocket"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
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
	data, err := json.Marshal(msg)
	if err != nil {
		facades.Log().Errorf("exec: failed to marshal status message: %v", err)
		return
	}
	mu.Lock()
	if writeErr := conn.WriteMessage(gorillaWs.TextMessage, data); writeErr != nil {
		facades.Log().Errorf("exec: failed to write status message: %v", writeErr)
	}
	mu.Unlock()
}

var defaultShells = []string{"/bin/sh", "/bin/bash", "/bin/ash"}

// RunLocalExecSession runs a Docker exec session, relaying I/O between
// the WebSocket connection and the container.
func RunLocalExecSession(
	ctx context.Context,
	conn *gorillaWs.Conn,
	wsMu *sync.Mutex,
	orchestrator docker.Orchestrator,
	containerID, shell, user string,
	cols, rows uint,
) {
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
	var lastErr error

	for _, sh := range shells {
		s, err := orchestrator.ExecContainer(ctx, containerID, docker.ExecOptions{
			Shell: sh,
			User:  user,
			Cols:  cols,
			Rows:  rows,
		})
		if err != nil {
			facades.Log().Errorf("exec create error with shell %s: %v", sh, err)
			lastErr = err
			continue
		}

		session = s
		attached = true
		break
	}

	if !attached {
		msg := "no shell found in container"
		if lastErr != nil {
			msg = fmt.Sprintf("failed to create exec: %v", lastErr)
		}
		writeExecMsg(conn, wsMu, execStatusMsg{Type: "error", Message: msg})
		return
	}
	defer session.Close()

	done := make(chan struct{})
	var once sync.Once
	closeDone := func() { once.Do(func() { close(done) }) }

	// WS -> Container stdin
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
					if err := session.Resize(msg.Cols, msg.Rows); err != nil {
						facades.Log().Warningf("exec: resize failed: %v", err)
					}
				}
			}
		}
	}()

	// Container stdout -> WS
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
}

// sendCloseFrame writes a clean WebSocket close frame.
func sendCloseFrame(conn *gorillaWs.Conn) {
	_ = conn.WriteControl(
		gorillaWs.CloseMessage,
		gorillaWs.FormatCloseMessage(gorillaWs.CloseNormalClosure, ""),
		time.Now().Add(2*time.Second),
	)
}
