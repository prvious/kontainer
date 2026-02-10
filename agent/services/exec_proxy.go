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

func proxyWebSocket(clientConn *websocket.Conn, followerAddr, containerID, shell, user, secret string, cols, rows uint) {
	// Build URL with properly encoded query params
	u := fmt.Sprintf("ws://%s/internal/exec/%s", followerAddr, containerID)
	params := url.Values{}
	params.Set("shell", shell)
	if user != "" {
		params.Set("user", user)
	}
	params.Set("cols", fmt.Sprintf("%d", cols))
	params.Set("rows", fmt.Sprintf("%d", rows))
	dialURL := u + "?" + params.Encode()

	header := http.Header{}
	header.Set("X-Kontainer-Secret", secret)

	followerConn, _, err := proxyDialer.Dial(dialURL, header)
	if err != nil {
		log.Printf("proxy dial follower error: %v", err)
		data, _ := (&execStatusMsg{Type: "error", Message: fmt.Sprintf("failed to connect to follower: %v", err)}).marshal()
		clientConn.WriteMessage(websocket.TextMessage, data)
		return
	}
	done := make(chan struct{})
	var once sync.Once
	closeDone := func() { once.Do(func() { close(done) }) }

	var wg sync.WaitGroup
	wg.Add(2)

	// Client → Follower
	go func() {
		defer wg.Done()
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
		defer wg.Done()
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
	// Close follower connection to unblock any goroutine still blocked on
	// follower reads/writes. The remaining Client→Follower goroutine (if
	// still running) will exit when the caller closes clientConn.
	followerConn.Close()
	wg.Wait()
}

func (m *execStatusMsg) marshal() ([]byte, error) {
	return json.Marshal(m)
}
