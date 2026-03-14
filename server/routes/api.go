package routes

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/goravel/framework/contracts/route"

	agentActions "github.com/prvious/kontainer/server/app/actions/agents"
	"github.com/prvious/kontainer/server/app/facades"
	agentControllers "github.com/prvious/kontainer/server/app/http/controllers/agents"
	authcontrollers "github.com/prvious/kontainer/server/app/http/controllers/auth"
	"github.com/prvious/kontainer/server/app/http/middleware"
	"github.com/prvious/kontainer/server/app/services/cluster"
)

func Api() {
	meCtrl := authcontrollers.NewMeController()

	// Build agents controller from registry
	var agentsHandler func(ctx http.Context) http.Response
	registryInstance, err := facades.App().Make("cluster.registry")
	if err == nil {
		if registry, ok := registryInstance.(*cluster.Registry); ok {
			action := agentActions.NewListAgentsAction(registry)
			ctrl := agentControllers.NewListAgentsController(action)
			agentsHandler = ctrl.Handle
		}
	}
	if agentsHandler == nil {
		facades.Log().Errorf("Failed to resolve cluster.registry for /api/agents — endpoint will return 503")
		agentsHandler = func(ctx http.Context) http.Response {
			return ctx.Response().Json(503, map[string]any{
				"error": "cluster management unavailable: registry failed to initialize",
			})
		}
	}

	facades.Route().Prefix("api").Middleware(middleware.JWT()).Group(func(router route.Router) {
		router.Get("me", meCtrl.Handle)
		router.Get("agents", agentsHandler)
	})
}
