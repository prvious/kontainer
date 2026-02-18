package auth_test

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/golang-jwt/jwt/v5"
	"github.com/prvious/kontainer/agent/auth"
)

func TestMeHandler_ReturnsClaims(t *testing.T) {
	handler := auth.NewMeHandler()

	claims := &auth.Claims{
		RegisteredClaims: jwt.RegisteredClaims{Issuer: "kontainer-agent"},
		UserID:           "u1",
		Name:             "Jane Doe",
		Email:            "jane@example.com",
		AvatarURL:        "https://example.com/avatar.png",
		Provider:         "github",
	}

	req := httptest.NewRequest("GET", "/api/me", nil)
	ctx := context.WithValue(req.Context(), auth.ClaimsKey, claims)
	req = req.WithContext(ctx)

	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		User struct {
			ID        string `json:"id"`
			Name      string `json:"name"`
			Email     string `json:"email"`
			AvatarURL string `json:"avatarUrl"`
			Provider  string `json:"provider"`
		} `json:"user"`
		Status string `json:"status"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("failed to decode response: %v", err)
	}
	if resp.User.ID != "u1" {
		t.Errorf("expected user ID u1, got %s", resp.User.ID)
	}
	if resp.User.Name != "Jane Doe" {
		t.Errorf("expected name Jane Doe, got %s", resp.User.Name)
	}
	if resp.Status != "ok" {
		t.Errorf("expected status ok, got %s", resp.Status)
	}
}

func TestMeHandler_NoClaims(t *testing.T) {
	handler := auth.NewMeHandler()

	req := httptest.NewRequest("GET", "/api/me", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", w.Code)
	}
}
