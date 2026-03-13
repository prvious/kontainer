package auth_test

import (
	"testing"

	authservice "github.com/prvious/kontainer/server/app/services/auth"
)

func TestSessionStore_GenerateAndValidate(t *testing.T) {
	store := authservice.NewSessionStore()
	defer store.Close()

	state, err := store.Generate("session-123")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if state == "" {
		t.Fatal("expected non-empty state token")
	}

	sessionID, ok := store.Validate(state)
	if !ok {
		t.Fatal("expected valid state token")
	}
	if sessionID != "session-123" {
		t.Errorf("expected 'session-123', got '%s'", sessionID)
	}
}

func TestSessionStore_ValidateConsumesToken(t *testing.T) {
	store := authservice.NewSessionStore()
	defer store.Close()

	state, _ := store.Generate("session-456")

	_, ok := store.Validate(state)
	if !ok {
		t.Fatal("first validation should succeed")
	}

	_, ok = store.Validate(state)
	if ok {
		t.Fatal("second validation should fail — token already consumed")
	}
}

func TestSessionStore_ValidateUnknownToken(t *testing.T) {
	store := authservice.NewSessionStore()
	defer store.Close()

	_, ok := store.Validate("nonexistent-token")
	if ok {
		t.Fatal("expected false for unknown token")
	}
}
