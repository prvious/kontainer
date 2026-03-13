package auth_test

import (
	"testing"
	"time"

	authservice "github.com/prvious/kontainer/server/app/services/auth"
)

func TestAuthBridge_SendAndReceive(t *testing.T) {
	bridge := authservice.NewAuthBridge()

	sessionID, err := bridge.CreateSession()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	go func() {
		time.Sleep(10 * time.Millisecond)
		bridge.SendToken(sessionID, "jwt-token-123")
	}()

	token, err := bridge.WaitForToken(sessionID, 1*time.Second)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if token != "jwt-token-123" {
		t.Errorf("expected 'jwt-token-123', got '%s'", token)
	}
}

func TestAuthBridge_Timeout(t *testing.T) {
	bridge := authservice.NewAuthBridge()

	sessionID, _ := bridge.CreateSession()

	_, err := bridge.WaitForToken(sessionID, 50*time.Millisecond)
	if err == nil {
		t.Fatal("expected timeout error")
	}
}

func TestAuthBridge_UnknownSession(t *testing.T) {
	bridge := authservice.NewAuthBridge()

	_, err := bridge.WaitForToken("nonexistent", 50*time.Millisecond)
	if err == nil {
		t.Fatal("expected error for unknown session")
	}
}

func TestAuthBridge_Cleanup(t *testing.T) {
	bridge := authservice.NewAuthBridge()

	sessionID, _ := bridge.CreateSession()
	bridge.Cleanup(sessionID)

	ok := bridge.SendToken(sessionID, "token")
	if ok {
		t.Fatal("expected false — session was cleaned up")
	}
}
