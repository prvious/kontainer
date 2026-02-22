package setup

import (
	"encoding/json"
	"net/http"
)

const SetupTokenHeader = "X-Kontainer-Setup-Token"

// SetupTokenMiddleware returns a Chi-compatible middleware that validates
// the X-Kontainer-Setup-Token header against the provided token.
func SetupTokenMiddleware(token string) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			provided := r.Header.Get(SetupTokenHeader)

			if provided == "" {
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusUnauthorized)
				json.NewEncoder(w).Encode(map[string]string{"error": "setup token required"})
				return
			}

			if provided != token {
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusUnauthorized)
				json.NewEncoder(w).Encode(map[string]string{"error": "invalid setup token"})
				return
			}

			next.ServeHTTP(w, r)
		})
	}
}
