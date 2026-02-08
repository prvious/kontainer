package server

import (
	"fmt"
	"net/http"

	"github.com/clovismuneza/devoper/agent/auth"
	"github.com/clovismuneza/devoper/agent/metrics"
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
	listNetworksHandler *services.ListNetworksHandler,
	listVolumesHandler *services.ListVolumesHandler,
	updateServiceEnvHandler *services.UpdateServiceEnvHandler,
	updateServiceImageHandler *services.UpdateServiceImageHandler,
	updateNodeAvailabilityHandler *services.UpdateNodeAvailabilityHandler,
	updateNodeRoleHandler *services.UpdateNodeRoleHandler,
	listSecretsHandler *services.ListSecretsHandler,
	createSecretHandler *services.CreateSecretHandler,
	removeSecretHandler *services.RemoveSecretHandler,
	listConfigsHandler *services.ListConfigsHandler,
	createConfigHandler *services.CreateConfigHandler,
	removeConfigHandler *services.RemoveConfigHandler,
	listAgentsHandler *services.ListAgentsHandler,
	clusterMetricsHandler *metrics.ClusterMetricsHandler,
	nodeMetricsHandler *metrics.NodeMetricsHandler,
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
			r.Method("GET", "/networks", listNetworksHandler)
			r.Method("GET", "/volumes", listVolumesHandler)
			r.Method("PUT", "/services/{serviceID}/env", updateServiceEnvHandler)
			r.Method("PUT", "/services/{serviceID}/image", updateServiceImageHandler)
			r.Method("PUT", "/nodes/{nodeID}/availability", updateNodeAvailabilityHandler)
			r.Method("PUT", "/nodes/{nodeID}/role", updateNodeRoleHandler)
			r.Method("GET", "/secrets", listSecretsHandler)
			r.Method("POST", "/secrets", createSecretHandler)
			r.Method("DELETE", "/secrets/{secretID}", removeSecretHandler)
			r.Method("GET", "/configs", listConfigsHandler)
			r.Method("POST", "/configs", createConfigHandler)
			r.Method("DELETE", "/configs/{configID}", removeConfigHandler)
			r.Method("GET", "/agents", listAgentsHandler)
			r.Method("GET", "/metrics", clusterMetricsHandler)
			r.Method("GET", "/metrics/{nodeID}", nodeMetricsHandler)
		})
	})

	return r
}
