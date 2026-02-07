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
	scaleServiceHandler *services.ScaleServiceHandler,
	restartServiceHandler *services.RestartServiceHandler,
	rollbackServiceHandler *services.RollbackServiceHandler,
	listStacksHandler *services.ListStacksHandler,
	removeStackHandler *services.RemoveStackHandler,
	listServiceTasksHandler *services.ListServiceTasksHandler,
	inspectTaskHandler *services.InspectTaskHandler,
	listNodesHandler *services.ListNodesHandler,
	listTaskStatsHandler *services.ListTaskStatsHandler,
	streamLogsHandler *services.StreamServiceLogsHandler,
	streamTaskLogsHandler *services.StreamTaskLogsHandler,
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
		// WebSocket routes — self-managed auth via query param token
		r.Get("/services/{serviceID}/logs", streamLogsHandler.ServeHTTP)
		r.Get("/tasks/{taskID}/logs", streamTaskLogsHandler.ServeHTTP)

		// REST routes — JWT middleware via Authorization header
		r.Group(func(r chi.Router) {
			r.Use(jwtMiddleware)
			r.Method("GET", "/services", listServicesHandler)
			r.Method("GET", "/services/{serviceID}", inspectServiceHandler)
			r.Method("PATCH", "/services/{serviceID}", scaleServiceHandler)
			r.Method("POST", "/services/{serviceID}/restart", restartServiceHandler)
			r.Method("POST", "/services/{serviceID}/rollback", rollbackServiceHandler)
			r.Method("GET", "/services/{serviceID}/tasks", listServiceTasksHandler)
			r.Method("GET", "/tasks/{taskID}", inspectTaskHandler)
			r.Method("GET", "/stacks", listStacksHandler)
			r.Method("DELETE", "/stacks/{stackName}", removeStackHandler)
			r.Method("GET", "/nodes", listNodesHandler)
			r.Method("GET", "/task-stats", listTaskStatsHandler)
		})
	})

	return r
}
