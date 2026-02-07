package server

import (
	"fmt"
	"net/http"

	"github.com/clovismuneza/devoper/agent/auth"
	"github.com/clovismuneza/devoper/agent/services"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
)

func New(
	authHandler *auth.AuthHandler,
	jwtMiddleware func(http.Handler) http.Handler,
	listServicesHandler *services.ListServicesHandler,
	inspectServiceHandler *services.InspectServiceHandler,
	listNodesHandler *services.ListNodesHandler,
) http.Handler {
	r := chi.NewRouter()
	r.Use(middleware.Logger)
	r.Use(middleware.Recoverer)

	// Public routes
	r.Get("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		fmt.Fprintf(w, `{"status":"ok"}`)
	})

	r.Get("/ping", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		fmt.Fprintf(w, `{"app":"dev.devoper.app"}`)
	})

	r.Get("/auth/listen", authHandler.AuthListenHandler)
	r.Get("/auth/{provider}", authHandler.RedirectHandler)
	r.Get("/auth/{provider}/callback", authHandler.CallbackHandler)

	// Protected routes
	r.Route("/api", func(r chi.Router) {
		r.Use(jwtMiddleware)
		r.Method("GET", "/services", listServicesHandler)
		r.Method("GET", "/services/{serviceID}", inspectServiceHandler)
		r.Method("GET", "/nodes", listNodesHandler)
	})

	return r
}
