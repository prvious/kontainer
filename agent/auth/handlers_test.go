package auth

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/clovismuneza/devoper/agent/auth/providers"
	"golang.org/x/oauth2"
)

func newTestHandler() (*AuthHandler, *providers.ProviderRegistry, *SessionStore, *AuthBridge) {
	registry := providers.NewProviderRegistry()
	registry.Register(&fakeProvider{name: "github"})
	sessions := &SessionStore{entries: make(map[string]sessionEntry)}
	bridge := NewAuthBridge()
	handler := NewAuthHandler(registry, sessions, testSecret, bridge)
	return handler, registry, sessions, bridge
}

func TestRedirectHandler_ValidProvider(t *testing.T) {
	handler, _, _, bridge := newTestHandler()

	sessionID, _ := bridge.CreateSession()
	req := httptest.NewRequest(http.MethodGet, "/auth/github?session="+sessionID, nil)
	req = withChiURLParam(req, "provider", "github")
	rec := httptest.NewRecorder()

	handler.RedirectHandler(rec, req)

	if rec.Code != http.StatusFound {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusFound)
	}

	location := rec.Header().Get("Location")
	if location == "" {
		t.Fatal("Location header should be set")
	}
}

func TestRedirectHandler_UnknownProvider(t *testing.T) {
	handler, _, _, _ := newTestHandler()

	req := httptest.NewRequest(http.MethodGet, "/auth/unknown?session=abc", nil)
	req = withChiURLParam(req, "provider", "unknown")
	rec := httptest.NewRecorder()

	handler.RedirectHandler(rec, req)

	if rec.Code != http.StatusNotFound {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusNotFound)
	}
}

func TestRedirectHandler_MissingSession(t *testing.T) {
	handler, _, _, _ := newTestHandler()

	req := httptest.NewRequest(http.MethodGet, "/auth/github", nil)
	req = withChiURLParam(req, "provider", "github")
	rec := httptest.NewRecorder()

	handler.RedirectHandler(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusBadRequest)
	}
}

func TestCallbackHandler_MissingCode(t *testing.T) {
	handler, _, sessions, _ := newTestHandler()

	state, _ := sessions.Generate("test-session")
	req := httptest.NewRequest(http.MethodGet, "/auth/github/callback?state="+state, nil)
	req = withChiURLParam(req, "provider", "github")
	rec := httptest.NewRecorder()

	handler.CallbackHandler(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusBadRequest)
	}
}

func TestCallbackHandler_MissingState(t *testing.T) {
	handler, _, _, _ := newTestHandler()

	req := httptest.NewRequest(http.MethodGet, "/auth/github/callback?code=abc123", nil)
	req = withChiURLParam(req, "provider", "github")
	rec := httptest.NewRecorder()

	handler.CallbackHandler(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusBadRequest)
	}
}

func TestCallbackHandler_InvalidState(t *testing.T) {
	handler, _, _, _ := newTestHandler()

	req := httptest.NewRequest(http.MethodGet, "/auth/github/callback?code=abc123&state=bogus", nil)
	req = withChiURLParam(req, "provider", "github")
	rec := httptest.NewRecorder()

	handler.CallbackHandler(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusBadRequest)
	}
}

func TestCallbackHandler_UnknownProvider(t *testing.T) {
	handler, _, _, _ := newTestHandler()

	req := httptest.NewRequest(http.MethodGet, "/auth/unknown/callback?code=abc123&state=xyz", nil)
	req = withChiURLParam(req, "provider", "unknown")
	rec := httptest.NewRecorder()

	handler.CallbackHandler(rec, req)

	if rec.Code != http.StatusNotFound {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusNotFound)
	}
}

func TestCallbackHandler_AccessDenied(t *testing.T) {
	// Create a fake token endpoint that returns a valid token
	tokenSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]interface{}{
			"access_token": "fake-token",
			"token_type":   "bearer",
		})
	}))
	defer tokenSrv.Close()

	registry := providers.NewProviderRegistry()
	registry.Register(&fakeAccessDeniedProvider{
		name: "github",
		config: &oauth2.Config{
			ClientID: "test",
			Endpoint: oauth2.Endpoint{
				TokenURL: tokenSrv.URL + "/token",
			},
		},
	})
	sessions := &SessionStore{entries: make(map[string]sessionEntry)}
	bridge := NewAuthBridge()
	handler := NewAuthHandler(registry, sessions, testSecret, bridge)

	state, _ := sessions.Generate("test-session")
	req := httptest.NewRequest(http.MethodGet, "/auth/github/callback?code=abc123&state="+state, nil)
	req = withChiURLParam(req, "provider", "github")
	rec := httptest.NewRecorder()

	handler.CallbackHandler(rec, req)

	if rec.Code != http.StatusForbidden {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusForbidden)
	}

	body := strings.TrimSpace(rec.Body.String())
	if !strings.Contains(body, "not a member") {
		t.Errorf("body = %q, want to contain 'not a member'", body)
	}
}

func TestCallbackHandler_OAuthExchangeFailure(t *testing.T) {
	registry := providers.NewProviderRegistry()
	registry.Register(&fakeOAuthProvider{
		name: "github",
		config: &oauth2.Config{
			ClientID: "test",
			Endpoint: oauth2.Endpoint{
				TokenURL: "http://127.0.0.1:0/token", // unreachable
			},
		},
	})
	sessions := &SessionStore{entries: make(map[string]sessionEntry)}
	bridge := NewAuthBridge()
	handler := NewAuthHandler(registry, sessions, testSecret, bridge)

	state, _ := sessions.Generate("test-session")
	req := httptest.NewRequest(http.MethodGet, "/auth/github/callback?code=abc123&state="+state, nil)
	req = withChiURLParam(req, "provider", "github")
	rec := httptest.NewRecorder()

	handler.CallbackHandler(rec, req)

	if rec.Code != http.StatusBadGateway {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusBadGateway)
	}
}
