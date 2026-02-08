package services

import (
	"encoding/json"
	"fmt"
	"log"
	"net"
	"net/http"
	"net/url"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

var proxyDialer = websocket.Dialer{
	HandshakeTimeout: 5 * time.Second,
	NetDial: (&net.Dialer{
		Timeout: 5 * time.Second,
	}).Dial,
}

func proxyWebSocket(clientConn *websocket.Conn, followerAddr, containerID, shell, secret string, cols, rows uint) {
	// Build URL with properly encoded query params
	u := fmt.Sprintf("ws://%s/internal/exec/%s", followerAddr, containerID)
	params := url.Values{}
	params.Set("shell", shell)
	params.Set("cols", fmt.Sprintf("%d", cols))
	params.Set("rows", fmt.Sprintf("%d", rows))
	dialURL := u + "?" + params.Encode()

	header := http.Header{}
	header.Set("X-Devoper-Secret", secret)

	followerConn, _, err := proxyDialer.Dial(dialURL, header)
	if err != nil {
		log.Printf("proxy dial follower error: %v", err)
		data, _ := (&execStatusMsg{Type: "error", Message: fmt.Sprintf("failed to connect to follower: %v", err)}).marshal()
		clientConn.WriteMessage(websocket.TextMessage, data)
		return
	}
	defer followerConn.Close()

	done := make(chan struct{})
	var once sync.Once
	closeDone := func() { once.Do(func() { close(done) }) }

	// Client → Follower
	go func() {
		defer closeDone()
		for {
			msgType, msg, err := clientConn.ReadMessage()
			if err != nil {
				return
			}
			if err := followerConn.WriteMessage(msgType, msg); err != nil {
				return
			}
		}
	}()

	// Follower → Client
	go func() {
		defer closeDone()
		for {
			msgType, msg, err := followerConn.ReadMessage()
			if err != nil {
				return
			}
			if err := clientConn.WriteMessage(msgType, msg); err != nil {
				return
			}
		}
	}()

	<-done
}

func (m *execStatusMsg) marshal() ([]byte, error) {
	return json.Marshal(m)
}
