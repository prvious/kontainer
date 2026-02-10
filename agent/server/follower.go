package server

import (
	"encoding/json"
	"net/http"

	"github.com/prvious/kontainer/agent/cluster"
	"github.com/prvious/kontainer/agent/images"
	"github.com/prvious/kontainer/agent/services"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
)

type FollowerIdentity struct {
	NodeID   string `json:"nodeId"`
	Hostname string `json:"hostname"`
}

func NewFollower(
	identity FollowerIdentity,
	internalSecret string,
	metricsHandler http.Handler,
	execFollowerHandler *services.ExecContainerFollowerHandler,
	internalImagesHandler *images.InternalImagesHandler,
	internalRemoveImageHandler *images.InternalRemoveImageHandler,
	internalPruneImagesHandler *images.InternalPruneImagesHandler,
) http.Handler {
	r := chi.NewRouter()
	r.Use(middleware.Logger)
	r.Use(middleware.Recoverer)

	r.Route("/internal", func(r chi.Router) {
		r.Use(cluster.InternalAuth(internalSecret))

		r.Get("/health", func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(map[string]string{
				"status":   "ok",
				"nodeId":   identity.NodeID,
				"hostname": identity.Hostname,
			})
		})

		r.Method("GET", "/metrics", metricsHandler)
		r.Method("GET", "/images", internalImagesHandler)
		r.Delete("/images/{imageID}", internalRemoveImageHandler.ServeHTTP)
		r.Post("/images/prune", internalPruneImagesHandler.ServeHTTP)
		r.Get("/exec/{containerID}", execFollowerHandler.ServeHTTP)

		r.Get("/info", func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(map[string]string{
				"nodeId":   identity.NodeID,
				"hostname": identity.Hostname,
				"role":     cluster.RoleFollower,
			})
		})
	})

	return r
}
