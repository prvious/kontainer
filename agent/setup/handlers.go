package setup

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"sync"

	"github.com/prvious/kontainer/agent/storage"
)

// SetupHandlers holds the HTTP handlers for the setup flow.
type SetupHandlers struct {
	store      *storage.AppConfigStore
	agentURL   string
	onComplete func() // called after setup_complete is set

	mu    sync.Mutex
	state string // CSRF state for the GitHub App manifest flow
	token string // one-time token for the start page
	org   string // optional GitHub org for app creation
}

// NewSetupHandlers creates a new SetupHandlers instance.
func NewSetupHandlers(store *storage.AppConfigStore, agentURL string, onComplete func()) *SetupHandlers {
	return &SetupHandlers{
		store:      store,
		agentURL:   agentURL,
		onComplete: onComplete,
	}
}

// StatusHandler returns the current setup step based on what is in the AppConfigStore.
// Steps: "pending" -> "github_connected" -> "complete"
func (h *SetupHandlers) StatusHandler(w http.ResponseWriter, r *http.Request) {
	if h.store.IsSetupComplete() {
		writeJSON(w, http.StatusOK, map[string]string{"step": "complete"})
		return
	}

	_, hasAppID, err := h.store.Get("github_app_id")
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "failed to check setup status"})
		return
	}

	if hasAppID {
		writeJSON(w, http.StatusOK, map[string]string{"step": "github_connected"})
		return
	}

	writeJSON(w, http.StatusOK, map[string]string{"step": "pending"})
}

// ManifestHandler generates a CSRF state and one-time token, then returns a setupUrl
// pointing to /setup/github/start?token=...
func (h *SetupHandlers) ManifestHandler(w http.ResponseWriter, r *http.Request) {
	var body struct {
		Org string `json:"org"`
	}
	// Body is optional — no org means personal account
	_ = json.NewDecoder(r.Body).Decode(&body)

	state := generateRandomHex(16)
	token := generateRandomHex(16)

	h.mu.Lock()
	h.state = state
	h.token = token
	h.org = body.Org
	h.mu.Unlock()

	setupURL := fmt.Sprintf("%s/setup/github/start?token=%s", h.agentURL, token)
	writeJSON(w, http.StatusOK, map[string]string{"setupUrl": setupURL})
}

// StartHandler verifies the token from query param, then serves an HTML page that
// auto-submits the GitHub App manifest form.
func (h *SetupHandlers) StartHandler(w http.ResponseWriter, r *http.Request) {
	queryToken := r.URL.Query().Get("token")

	h.mu.Lock()
	storedState := h.state
	storedToken := h.token
	storedOrg := h.org
	h.mu.Unlock()

	if storedState == "" || storedToken == "" {
		renderError(w, http.StatusForbidden, "No setup session found. Start the setup flow first.")
		return
	}

	if queryToken != storedToken {
		renderError(w, http.StatusUnauthorized, "Invalid or expired setup token.")
		return
	}

	appName := "Kontainer"
	if storedOrg != "" {
		appName = fmt.Sprintf("Kontainer (%s)", storedOrg)
	}

	manifest := map[string]interface{}{
		"name":          appName,
		"url":           h.agentURL,
		"callback_urls": []string{h.agentURL + "/auth/github/callback"},
		"redirect_url":  h.agentURL + "/setup/github/callback",
		"setup_url":     h.agentURL + "/setup/github/callback",
		"public":        false,
		"default_permissions": map[string]string{
			"members":         "read",
			"emails": "read",
		},
		"default_events": []string{},
	}

	manifestJSON, err := json.Marshal(manifest)
	if err != nil {
		renderError(w, http.StatusInternalServerError, "Failed to generate manifest.")
		return
	}

	// Invalidate the token after use
	h.mu.Lock()
	h.token = ""
	h.mu.Unlock()

	var githubURL string
	if storedOrg != "" {
		githubURL = fmt.Sprintf("https://github.com/organizations/%s/settings/apps/new?state=%s", storedOrg, storedState)
	} else {
		githubURL = fmt.Sprintf("https://github.com/settings/apps/new?state=%s", storedState)
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(http.StatusOK)
	fmt.Fprintf(w, `<!DOCTYPE html>
<html>
<head><title>Kontainer Setup</title></head>
<body>
<p>Redirecting to GitHub...</p>
<form id="manifest-form" action="%s" method="post">
<input type="hidden" name="manifest" value='%s'>
</form>
<script>document.getElementById('manifest-form').submit();</script>
</body>
</html>`, githubURL, htmlEscapeJSON(string(manifestJSON)))
}

// CallbackHandler handles two GitHub redirects:
// 1. After manifest creation: exchanges code for credentials, then redirects to install page
// 2. After app installation: shows success page (setup_action param present)
func (h *SetupHandlers) CallbackHandler(w http.ResponseWriter, r *http.Request) {
	// Post-install redirect — GitHub sends setup_action after the user installs the app
	if r.URL.Query().Get("setup_action") != "" {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		w.WriteHeader(http.StatusOK)
		fmt.Fprint(w, `<!DOCTYPE html>
<html>
<head><title>Kontainer Setup</title></head>
<body>
<h1>GitHub App Installed!</h1>
<p>Return to the Kontainer app to complete setup.</p>
</body>
</html>`)
		return
	}

	code := r.URL.Query().Get("code")
	if code == "" {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "missing code parameter"})
		return
	}

	// Exchange the code for app credentials
	conversionURL := fmt.Sprintf("https://api.github.com/app-manifests/%s/conversions", code)
	resp, err := http.Post(conversionURL, "application/json", nil)
	if err != nil {
		renderError(w, http.StatusBadGateway, "Failed to contact GitHub API.")
		return
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		renderError(w, http.StatusBadGateway, "Failed to read GitHub API response.")
		return
	}

	if resp.StatusCode != http.StatusCreated {
		renderError(w, http.StatusBadGateway, fmt.Sprintf("GitHub API returned status %d.", resp.StatusCode))
		return
	}

	var ghResp struct {
		ID            int    `json:"id"`
		Slug          string `json:"slug"`
		ClientID      string `json:"client_id"`
		ClientSecret  string `json:"client_secret"`
		PEM           string `json:"pem"`
		WebhookSecret string `json:"webhook_secret"`
		Owner         struct {
			Login string `json:"login"`
		} `json:"owner"`
	}
	if err := json.Unmarshal(body, &ghResp); err != nil {
		renderError(w, http.StatusBadGateway, "Failed to parse GitHub API response.")
		return
	}

	// Generate a JWT secret for the agent
	jwtSecret := generateRandomHex(32)

	// Store all credentials
	err = h.store.SetAll(map[string]string{
		"github_app_id":         strconv.Itoa(ghResp.ID),
		"github_client_id":      ghResp.ClientID,
		"github_client_secret":  ghResp.ClientSecret,
		"github_private_key":    ghResp.PEM,
		"github_webhook_secret": ghResp.WebhookSecret,
		"github_org":            ghResp.Owner.Login,
		"jwt_secret":            jwtSecret,
	})
	if err != nil {
		renderError(w, http.StatusInternalServerError, "Failed to store GitHub App credentials.")
		return
	}

	// Clear the CSRF state
	h.mu.Lock()
	h.state = ""
	h.mu.Unlock()

	// Redirect to the GitHub App installation page
	installURL := fmt.Sprintf("https://github.com/apps/%s/installations/new", ghResp.Slug)
	http.Redirect(w, r, installURL, http.StatusFound)
}

// CompleteHandler verifies GitHub credentials exist, sets setup_complete = true,
// and returns a success JSON response.
func (h *SetupHandlers) CompleteHandler(w http.ResponseWriter, r *http.Request) {
	_, hasAppID, err := h.store.Get("github_app_id")
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "failed to check setup status"})
		return
	}

	if !hasAppID {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "GitHub App not connected. Complete the GitHub setup first."})
		return
	}

	if err := h.store.Set("setup_complete", "true"); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "failed to complete setup"})
		return
	}

	writeJSON(w, http.StatusOK, map[string]string{"status": "complete"})

	// Trigger mode switch after responding
	if h.onComplete != nil {
		go h.onComplete()
	}
}

// writeJSON writes a JSON response with the given status code and payload.
func writeJSON(w http.ResponseWriter, status int, data interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(data)
}

// renderError writes an HTML error page.
func renderError(w http.ResponseWriter, status int, message string) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(status)
	fmt.Fprintf(w, `<!DOCTYPE html>
<html>
<head><title>Kontainer Setup - Error</title></head>
<body>
<h1>Setup Error</h1>
<p>%s</p>
</body>
</html>`, message)
}

// generateRandomHex generates a random hex string of the given byte length.
func generateRandomHex(nBytes int) string {
	b := make([]byte, nBytes)
	if _, err := rand.Read(b); err != nil {
		panic("crypto/rand failed: " + err.Error())
	}
	return hex.EncodeToString(b)
}

// htmlEscapeJSON escapes a JSON string for safe embedding in an HTML attribute value
// enclosed in single quotes. It replaces single quotes and ampersands.
func htmlEscapeJSON(s string) string {
	// The JSON is embedded in value='...' so we need to escape single quotes and ampersands
	result := make([]byte, 0, len(s))
	for i := 0; i < len(s); i++ {
		switch s[i] {
		case '\'':
			result = append(result, []byte("&#39;")...)
		case '&':
			result = append(result, []byte("&amp;")...)
		case '<':
			result = append(result, []byte("&lt;")...)
		case '>':
			result = append(result, []byte("&gt;")...)
		default:
			result = append(result, s[i])
		}
	}
	return string(result)
}
