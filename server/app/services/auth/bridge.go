package auth

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"sync"
	"time"
)

// AuthBridge connects the WebSocket listener with the OAuth callback.
type AuthBridge struct {
	mu      sync.Mutex
	pending map[string]chan string
}

func NewAuthBridge() *AuthBridge {
	return &AuthBridge{
		pending: make(map[string]chan string),
	}
}

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

func (b *AuthBridge) WaitForToken(sessionID string, timeout time.Duration, done <-chan struct{}) (string, error) {
	b.mu.Lock()
	ch, ok := b.pending[sessionID]
	b.mu.Unlock()

	if !ok {
		return "", errors.New("unknown session")
	}

	select {
	case token := <-ch:
		if token == "" {
			return "", errors.New("session closed")
		}
		return token, nil
	case <-time.After(timeout):
		return "", errors.New("timeout waiting for token")
	case <-done:
		return "", errors.New("client disconnected")
	}
}

func (b *AuthBridge) Cleanup(sessionID string) {
	b.mu.Lock()
	defer b.mu.Unlock()
	delete(b.pending, sessionID)
}
