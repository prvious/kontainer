package auth

import (
	"encoding/json"
	"errors"
	"log"
	"net/http"
	"time"

	"github.com/prvious/kontainer/agent/auth/providers"
	"github.com/go-chi/chi/v5"
	"github.com/gorilla/websocket"
)

const tokenTTL = 24 * time.Hour

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool { return true },
}

type AuthHandler struct {
	registry  *providers.ProviderRegistry
	sessions  *SessionStore
	jwtSecret string
	bridge    *AuthBridge
}

func NewAuthHandler(registry *providers.ProviderRegistry, sessions *SessionStore, jwtSecret string, bridge *AuthBridge) *AuthHandler {
	return &AuthHandler{
		registry:  registry,
		sessions:  sessions,
		jwtSecret: jwtSecret,
		bridge:    bridge,
	}
}

// ProvidersHandler returns the list of registered OAuth provider names.
func (h *AuthHandler) ProvidersHandler(w http.ResponseWriter, r *http.Request) {
	names := h.registry.Names()
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string][]string{"providers": names})
}

// AuthListenHandler upgrades to WebSocket, creates a bridge session, sends the session ID,
// then blocks until the OAuth callback delivers a token (or timeout).
func (h *AuthHandler) AuthListenHandler(w http.ResponseWriter, r *http.Request) {
	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("ws upgrade error: %v", err)
		return
	}
	defer conn.Close()

	sessionID, err := h.bridge.CreateSession()
	if err != nil {
		conn.WriteJSON(map[string]string{"error": "failed to create session"})
		return
	}
	defer h.bridge.Cleanup(sessionID)

	if err := conn.WriteJSON(map[string]string{"sessionId": sessionID}); err != nil {
		return
	}

	token, err := h.bridge.WaitForToken(sessionID, 10*time.Minute)
	if err != nil {
		conn.WriteJSON(map[string]string{"error": err.Error()})
		return
	}

	conn.WriteJSON(map[string]string{"token": token})
}

// RedirectHandler initiates OAuth by redirecting to the provider's authorize URL.
func (h *AuthHandler) RedirectHandler(w http.ResponseWriter, r *http.Request) {
	providerName := chi.URLParam(r, "provider")

	provider, err := h.registry.Get(providerName)
	if err != nil {
		http.Error(w, err.Error(), http.StatusNotFound)
		return
	}

	sessionID := r.URL.Query().Get("session")
	if sessionID == "" {
		http.Error(w, "missing session parameter", http.StatusBadRequest)
		return
	}

	state, err := h.sessions.Generate(sessionID)
	if err != nil {
		http.Error(w, "failed to generate state", http.StatusInternalServerError)
		return
	}

	authURL := provider.OAuthConfig().AuthCodeURL(state)
	http.Redirect(w, r, authURL, http.StatusFound)
}

// CallbackHandler handles the OAuth callback, exchanges the code, and pushes the JWT through the bridge.
func (h *AuthHandler) CallbackHandler(w http.ResponseWriter, r *http.Request) {
	providerName := chi.URLParam(r, "provider")

	provider, err := h.registry.Get(providerName)
	if err != nil {
		http.Error(w, err.Error(), http.StatusNotFound)
		return
	}

	code := r.URL.Query().Get("code")
	state := r.URL.Query().Get("state")

	if code == "" || state == "" {
		http.Error(w, "missing code or state", http.StatusBadRequest)
		return
	}

	sessionID, ok := h.sessions.Validate(state)
	if !ok {
		http.Error(w, "invalid or expired state", http.StatusBadRequest)
		return
	}

	oauthToken, err := provider.OAuthConfig().Exchange(r.Context(), code)
	if err != nil {
		log.Printf("oauth exchange error: %v", err)
		http.Error(w, "oauth exchange failed", http.StatusBadGateway)
		return
	}

	user, err := provider.FetchUser(r.Context(), oauthToken)
	if err != nil {
		var accessErr *providers.AccessDeniedError
		if errors.As(err, &accessErr) {
			http.Error(w, accessErr.Message, http.StatusForbidden)
			return
		}
		log.Printf("fetch user error: %v", err)
		http.Error(w, "failed to fetch user", http.StatusBadGateway)
		return
	}

	token, err := CreateToken(h.jwtSecret, user, tokenTTL)
	if err != nil {
		log.Printf("jwt creation error: %v", err)
		http.Error(w, "failed to create token", http.StatusInternalServerError)
		return
	}

	h.bridge.SendToken(sessionID, token)

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write([]byte(successHTML))
}

const successHTML = `<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>Authenticated</title></head>
<body style="display:flex;align-items:center;justify-content:center;height:100vh;font-family:system-ui,sans-serif">
<p>Authentication successful. You can close this tab.</p>
</body>
</html>`
