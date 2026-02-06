package auth

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/clovismuneza/devoper/agent/auth/providers"
	"golang.org/x/oauth2"
)

func newTestHandler() (*AuthHandler, *providers.ProviderRegistry, *SessionStore) {
	registry := providers.NewProviderRegistry()
	registry.Register(&fakeProvider{name: "github"})
	sessions := &SessionStore{entries: make(map[string]sessionEntry)}
	handler := NewAuthHandler(registry, sessions, testSecret, "devoper")
	return handler, registry, sessions
}

func TestRedirectHandler_ValidProvider(t *testing.T) {
	handler, _, _ := newTestHandler()

	req := httptest.NewRequest(http.MethodGet, "/auth/github", nil)
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
	handler, _, _ := newTestHandler()

	req := httptest.NewRequest(http.MethodGet, "/auth/unknown", nil)
	req = withChiURLParam(req, "provider", "unknown")
	rec := httptest.NewRecorder()

	handler.RedirectHandler(rec, req)

	if rec.Code != http.StatusNotFound {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusNotFound)
	}
}

func TestCallbackHandler_MissingCode(t *testing.T) {
	handler, _, sessions := newTestHandler()

	state, _ := sessions.Generate()
	req := httptest.NewRequest(http.MethodGet, "/auth/github/callback?state="+state, nil)
	req = withChiURLParam(req, "provider", "github")
	rec := httptest.NewRecorder()

	handler.CallbackHandler(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusBadRequest)
	}
}

func TestCallbackHandler_MissingState(t *testing.T) {
	handler, _, _ := newTestHandler()

	req := httptest.NewRequest(http.MethodGet, "/auth/github/callback?code=abc123", nil)
	req = withChiURLParam(req, "provider", "github")
	rec := httptest.NewRecorder()

	handler.CallbackHandler(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusBadRequest)
	}
}

func TestCallbackHandler_InvalidState(t *testing.T) {
	handler, _, _ := newTestHandler()

	req := httptest.NewRequest(http.MethodGet, "/auth/github/callback?code=abc123&state=bogus", nil)
	req = withChiURLParam(req, "provider", "github")
	rec := httptest.NewRecorder()

	handler.CallbackHandler(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusBadRequest)
	}
}

func TestCallbackHandler_UnknownProvider(t *testing.T) {
	handler, _, _ := newTestHandler()

	req := httptest.NewRequest(http.MethodGet, "/auth/unknown/callback?code=abc123&state=xyz", nil)
	req = withChiURLParam(req, "provider", "unknown")
	rec := httptest.NewRecorder()

	handler.CallbackHandler(rec, req)

	if rec.Code != http.StatusNotFound {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusNotFound)
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
	handler := NewAuthHandler(registry, sessions, testSecret, "devoper")

	state, _ := sessions.Generate()
	req := httptest.NewRequest(http.MethodGet, "/auth/github/callback?code=abc123&state="+state, nil)
	req = withChiURLParam(req, "provider", "github")
	rec := httptest.NewRecorder()

	handler.CallbackHandler(rec, req)

	if rec.Code != http.StatusBadGateway {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusBadGateway)
	}
}
