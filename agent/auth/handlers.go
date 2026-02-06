package auth

import (
	"fmt"
	"log"
	"net/http"
	"net/url"
	"time"

	"github.com/clovismuneza/devoper/agent/auth/providers"
	"github.com/go-chi/chi/v5"
)

const tokenTTL = 24 * time.Hour

type AuthHandler struct {
	registry  *providers.ProviderRegistry
	sessions  *SessionStore
	jwtSecret string
	appScheme string
}

func NewAuthHandler(registry *providers.ProviderRegistry, sessions *SessionStore, jwtSecret, appScheme string) *AuthHandler {
	return &AuthHandler{
		registry:  registry,
		sessions:  sessions,
		jwtSecret: jwtSecret,
		appScheme: appScheme,
	}
}

// RedirectHandler initiates OAuth by redirecting to the provider's authorize URL.
func (h *AuthHandler) RedirectHandler(w http.ResponseWriter, r *http.Request) {
	providerName := chi.URLParam(r, "provider")

	provider, err := h.registry.Get(providerName)
	if err != nil {
		http.Error(w, err.Error(), http.StatusNotFound)
		return
	}

	state, err := h.sessions.Generate()
	if err != nil {
		http.Error(w, "failed to generate state", http.StatusInternalServerError)
		return
	}

	authURL := provider.OAuthConfig().AuthCodeURL(state)
	http.Redirect(w, r, authURL, http.StatusFound)
}

// CallbackHandler handles the OAuth callback, exchanges the code, and redirects to the desktop app.
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

	if !h.sessions.Validate(state) {
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

	deepLink := fmt.Sprintf("%s://auth/callback?token=%s", h.appScheme, url.QueryEscape(token))
	http.Redirect(w, r, deepLink, http.StatusFound)
}
