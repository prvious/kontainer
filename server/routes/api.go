package routes

import (
	"github.com/goravel/framework/contracts/route"

	agentActions "github.com/prvious/kontainer/server/app/actions/agents"
	"github.com/prvious/kontainer/server/app/facades"
	authcontrollers "github.com/prvious/kontainer/server/app/http/controllers/auth"
	agentControllers "github.com/prvious/kontainer/server/app/http/controllers/agents"
	"github.com/prvious/kontainer/server/app/http/middleware"
	"github.com/prvious/kontainer/server/app/services/cluster"
)

func Api() {
	meCtrl := authcontrollers.NewMeController()

	// Build agents controller from registry
	var listAgentsCtrl *agentControllers.ListAgentsController
	registryInstance, err := facades.App().Make("cluster.registry")
	if err == nil {
		if registry, ok := registryInstance.(*cluster.Registry); ok {
			action := agentActions.NewListAgentsAction(registry)
			listAgentsCtrl = agentControllers.NewListAgentsController(action)
		}
	}
	if listAgentsCtrl == nil {
		facades.Log().Errorf("Failed to resolve cluster.registry for /api/agents")
	}

	facades.Route().Prefix("api").Middleware(middleware.JWT()).Group(func(router route.Router) {
		router.Get("me", meCtrl.Handle)

		if listAgentsCtrl != nil {
			router.Get("agents", listAgentsCtrl.Handle)
		}
	})
}
