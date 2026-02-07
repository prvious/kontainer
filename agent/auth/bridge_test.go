package auth

import (
	"testing"
	"time"
)

func TestAuthBridge_CreateAndSend(t *testing.T) {
	b := NewAuthBridge()

	sessionID, err := b.CreateSession()
	if err != nil {
		t.Fatalf("CreateSession failed: %v", err)
	}
	if sessionID == "" {
		t.Fatal("CreateSession returned empty ID")
	}

	// Send token in background, then wait for it
	go func() {
		time.Sleep(10 * time.Millisecond)
		b.SendToken(sessionID, "my-jwt")
	}()

	token, err := b.WaitForToken(sessionID, time.Second)
	if err != nil {
		t.Fatalf("WaitForToken failed: %v", err)
	}
	if token != "my-jwt" {
		t.Fatalf("token = %q, want %q", token, "my-jwt")
	}
}

func TestAuthBridge_SendToUnknownSession(t *testing.T) {
	b := NewAuthBridge()

	if b.SendToken("nonexistent", "token") {
		t.Fatal("SendToken should return false for unknown session")
	}
}

func TestAuthBridge_WaitTimeout(t *testing.T) {
	b := NewAuthBridge()

	sessionID, err := b.CreateSession()
	if err != nil {
		t.Fatalf("CreateSession failed: %v", err)
	}

	_, err = b.WaitForToken(sessionID, 50*time.Millisecond)
	if err == nil {
		t.Fatal("WaitForToken should return error on timeout")
	}
}

func TestAuthBridge_WaitUnknownSession(t *testing.T) {
	b := NewAuthBridge()

	_, err := b.WaitForToken("nonexistent", time.Second)
	if err == nil {
		t.Fatal("WaitForToken should return error for unknown session")
	}
}

func TestAuthBridge_Cleanup(t *testing.T) {
	b := NewAuthBridge()

	sessionID, _ := b.CreateSession()
	b.Cleanup(sessionID)

	if b.SendToken(sessionID, "token") {
		t.Fatal("SendToken should return false after Cleanup")
	}
}

func TestAuthBridge_UniqueSessionIDs(t *testing.T) {
	b := NewAuthBridge()

	ids := make(map[string]bool)
	for i := 0; i < 100; i++ {
		id, err := b.CreateSession()
		if err != nil {
			t.Fatalf("CreateSession failed: %v", err)
		}
		if ids[id] {
			t.Fatalf("duplicate session ID: %s", id)
		}
		ids[id] = true
	}
}
