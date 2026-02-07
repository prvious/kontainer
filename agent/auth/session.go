package auth

import (
	"crypto/rand"
	"encoding/hex"
	"sync"
	"time"
)

const sessionTTL = 10 * time.Minute

type sessionEntry struct {
	createdAt time.Time
	sessionID string
}

// SessionStore is an in-memory CSRF state store with TTL-based expiration.
type SessionStore struct {
	mu      sync.Mutex
	entries map[string]sessionEntry
}

func NewSessionStore() *SessionStore {
	s := &SessionStore{entries: make(map[string]sessionEntry)}
	go s.cleanup()
	return s
}

// Generate creates a new random state token, associates it with a bridge session ID, and stores it.
func (s *SessionStore) Generate(sessionID string) (string, error) {
	b := make([]byte, 16)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	token := hex.EncodeToString(b)

	s.mu.Lock()
	s.entries[token] = sessionEntry{createdAt: time.Now(), sessionID: sessionID}
	s.mu.Unlock()

	return token, nil
}

// Validate checks if a state token exists and has not expired, then removes it.
// Returns the associated bridge session ID and whether the token was valid.
func (s *SessionStore) Validate(token string) (string, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()

	entry, ok := s.entries[token]
	if !ok {
		return "", false
	}
	delete(s.entries, token)

	if time.Since(entry.createdAt) >= sessionTTL {
		return "", false
	}
	return entry.sessionID, true
}

func (s *SessionStore) cleanup() {
	ticker := time.NewTicker(time.Minute)
	defer ticker.Stop()

	for range ticker.C {
		s.mu.Lock()
		for token, entry := range s.entries {
			if time.Since(entry.createdAt) >= sessionTTL {
				delete(s.entries, token)
			}
		}
		s.mu.Unlock()
	}
}
