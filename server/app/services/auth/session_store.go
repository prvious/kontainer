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
	mu        sync.Mutex
	entries   map[string]sessionEntry
	done      chan struct{}
	closeOnce sync.Once
}

func NewSessionStore() *SessionStore {
	s := &SessionStore{
		entries: make(map[string]sessionEntry),
		done:    make(chan struct{}),
	}
	go s.cleanup()
	return s
}

func (s *SessionStore) Close() {
	s.closeOnce.Do(func() {
		close(s.done)
	})
}

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

	for {
		select {
		case <-ticker.C:
			s.mu.Lock()
			for token, entry := range s.entries {
				if time.Since(entry.createdAt) >= sessionTTL {
					delete(s.entries, token)
				}
			}
			s.mu.Unlock()
		case <-s.done:
			return
		}
	}
}
