package server

import (
	"fmt"
	"net/http"

	"github.com/clovismuneza/devoper/agent/auth"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
)

func New(authHandler *auth.AuthHandler, jwtMiddleware func(http.Handler) http.Handler) http.Handler {
	r := chi.NewRouter()
	r.Use(middleware.Logger)
	r.Use(middleware.Recoverer)

	// Public routes
	r.Get("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		fmt.Fprintf(w, `{"status":"ok"}`)
	})

	r.Get("/auth/{provider}", authHandler.RedirectHandler)
	r.Get("/auth/{provider}/callback", authHandler.CallbackHandler)

	// Protected routes
	r.Route("/api", func(r chi.Router) {
		r.Use(jwtMiddleware)
		// Future protected endpoints go here
	})

	return r
}
