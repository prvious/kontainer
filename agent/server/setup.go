package server

import (
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/prvious/kontainer/agent/setup"
)

func NewSetupRouter(setupHandlers *setup.SetupHandlers, setupToken string) http.Handler {
	r := chi.NewRouter()
	r.Use(middleware.Logger)
	r.Use(middleware.Recoverer)

	// Public: health + ping (with setup_required status)
	r.Get("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		fmt.Fprintf(w, `{"status":"ok"}`)
	})

	r.Get("/ping", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		fmt.Fprintf(w, `{"app":"dev.kontainer.app","status":"setup_required"}`)
	})

	// Setup routes (token-protected)
	r.Route("/setup", func(r chi.Router) {
		r.Use(setup.SetupTokenMiddleware(setupToken))
		r.Get("/status", setupHandlers.StatusHandler)
		r.Post("/github/manifest", setupHandlers.ManifestHandler)
		r.Post("/complete", setupHandlers.CompleteHandler)
	})

	// GitHub start page — token verified via query param inside the handler
	r.Get("/setup/github/start", setupHandlers.StartHandler)

	// GitHub callback — state-protected inside the handler
	r.Get("/setup/github/callback", setupHandlers.CallbackHandler)

	// Everything else returns 403
	r.NotFound(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusForbidden)
		json.NewEncoder(w).Encode(map[string]string{"error": "agent is in setup mode"})
	})

	return r
}
