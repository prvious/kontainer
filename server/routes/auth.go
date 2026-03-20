package routes

import (
	"github.com/goravel/framework/contracts/route"
	"github.com/prvious/kontainer/server/app/facades"

	authactions "github.com/prvious/kontainer/server/app/actions/auth"
	authcontrollers "github.com/prvious/kontainer/server/app/http/controllers/auth"
	authservice "github.com/prvious/kontainer/server/app/services/auth"
	authproviders "github.com/prvious/kontainer/server/app/services/auth/providers"
)

func Auth() {
	if facades.Config().GetString("cluster.role") != "leader" {
		return
	}

	registry, ok := resolve[*authproviders.ProviderRegistry]("auth.provider_registry")
	if !ok {
		return
	}
	sessions, ok := resolve[*authservice.SessionStore]("auth.session_store")
	if !ok {
		return
	}
	bridge, ok := resolve[*authservice.AuthBridge]("auth.bridge")
	if !ok {
		return
	}

	upsertUser := authactions.NewUpsertUserAction()

	providersCtrl := authcontrollers.NewProvidersController(registry)
	listenCtrl := authcontrollers.NewListenController(bridge)
	redirectCtrl := authcontrollers.NewRedirectController(registry, sessions)
	callbackCtrl := authcontrollers.NewCallbackController(registry, sessions, bridge, upsertUser)

	facades.Route().Prefix("auth").Group(func(router route.Router) {
		router.Get("providers", providersCtrl.Handle)
		router.Get("listen", listenCtrl.Handle)
		router.Get("{provider}", redirectCtrl.Handle)
		router.Get("{provider}/callback", callbackCtrl.Handle)
	})
}
