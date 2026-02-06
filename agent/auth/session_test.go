package auth

import (
	"testing"
	"time"
)

func newTestStore() *SessionStore {
	return &SessionStore{entries: make(map[string]sessionEntry)}
}

func TestSessionStore_GenerateAndValidate(t *testing.T) {
	s := newTestStore()

	token, err := s.Generate()
	if err != nil {
		t.Fatalf("Generate failed: %v", err)
	}
	if token == "" {
		t.Fatal("Generate returned empty token")
	}

	if !s.Validate(token) {
		t.Fatal("Validate returned false for valid token")
	}
}

func TestSessionStore_GenerateUniqueness(t *testing.T) {
	s := newTestStore()

	tokens := make(map[string]bool)
	for i := 0; i < 100; i++ {
		token, err := s.Generate()
		if err != nil {
			t.Fatalf("Generate failed: %v", err)
		}
		if tokens[token] {
			t.Fatalf("duplicate token generated: %s", token)
		}
		tokens[token] = true
	}
}

func TestSessionStore_ValidateConsumesToken(t *testing.T) {
	s := newTestStore()

	token, err := s.Generate()
	if err != nil {
		t.Fatalf("Generate failed: %v", err)
	}

	if !s.Validate(token) {
		t.Fatal("first Validate should return true")
	}

	if s.Validate(token) {
		t.Fatal("second Validate should return false (token consumed)")
	}
}

func TestSessionStore_ValidateUnknownToken(t *testing.T) {
	s := newTestStore()

	if s.Validate("nonexistent-token") {
		t.Fatal("Validate should return false for unknown token")
	}
}

func TestSessionStore_ValidateExpiredToken(t *testing.T) {
	s := newTestStore()

	token, err := s.Generate()
	if err != nil {
		t.Fatalf("Generate failed: %v", err)
	}

	// Manually expire the entry
	s.mu.Lock()
	s.entries[token] = sessionEntry{createdAt: time.Now().Add(-sessionTTL - time.Second)}
	s.mu.Unlock()

	if s.Validate(token) {
		t.Fatal("Validate should return false for expired token")
	}
}

func TestSessionStore_ValidateEmptyToken(t *testing.T) {
	s := newTestStore()

	if s.Validate("") {
		t.Fatal("Validate should return false for empty token")
	}
}
