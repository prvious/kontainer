package services

import (
	"fmt"
	"log"
	"net/http"
	"sync"

	"github.com/gorilla/websocket"
)

func proxyWebSocket(clientConn *websocket.Conn, followerAddr, containerID, shell, secret string, cols, rows uint) {
	// Dial the follower's internal exec endpoint
	url := fmt.Sprintf("ws://%s/internal/exec/%s?shell=%s&cols=%d&rows=%d", followerAddr, containerID, shell, cols, rows)

	header := http.Header{}
	header.Set("X-Devoper-Secret", secret)

	followerConn, _, err := websocket.DefaultDialer.Dial(url, header)
	if err != nil {
		log.Printf("proxy dial follower error: %v", err)
		writeExecError(clientConn, fmt.Sprintf("failed to connect to follower: %v", err))
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
