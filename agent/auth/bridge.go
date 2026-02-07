package auth

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"sync"
	"time"
)

// AuthBridge connects the WebSocket listener with the OAuth callback.
// Each pending session maps a session ID to a buffered token channel.
type AuthBridge struct {
	mu      sync.Mutex
	pending map[string]chan string
}

func NewAuthBridge() *AuthBridge {
	return &AuthBridge{
		pending: make(map[string]chan string),
	}
}

// CreateSession generates a random session ID and creates a buffered channel for it.
func (b *AuthBridge) CreateSession() (string, error) {
	buf := make([]byte, 16)
	if _, err := rand.Read(buf); err != nil {
		return "", err
	}
	id := hex.EncodeToString(buf)

	b.mu.Lock()
	b.pending[id] = make(chan string, 1)
	b.mu.Unlock()

	return id, nil
}

// SendToken delivers a token to the waiting WebSocket listener.
// Returns true if the session existed and the token was sent.
func (b *AuthBridge) SendToken(sessionID, token string) bool {
	b.mu.Lock()
	ch, ok := b.pending[sessionID]
	b.mu.Unlock()

	if !ok {
		return false
	}

	select {
	case ch <- token:
		return true
	default:
		return false
	}
}

// WaitForToken blocks until a token arrives or the timeout elapses.
func (b *AuthBridge) WaitForToken(sessionID string, timeout time.Duration) (string, error) {
	b.mu.Lock()
	ch, ok := b.pending[sessionID]
	b.mu.Unlock()

	if !ok {
		return "", errors.New("unknown session")
	}

	select {
	case token := <-ch:
		return token, nil
	case <-time.After(timeout):
		return "", errors.New("timeout waiting for token")
	}
}

// Cleanup removes a session and closes its channel.
func (b *AuthBridge) Cleanup(sessionID string) {
	b.mu.Lock()
	defer b.mu.Unlock()

	if ch, ok := b.pending[sessionID]; ok {
		close(ch)
		delete(b.pending, sessionID)
	}
}
