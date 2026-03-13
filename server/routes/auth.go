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
	registryInstance, err := facades.App().Make("auth.provider_registry")
	if err != nil {
		facades.Log().Errorf("Failed to resolve auth.provider_registry: %v", err)
		return
	}
	registry, ok := registryInstance.(*authproviders.ProviderRegistry)
	if !ok {
		facades.Log().Errorf("auth.provider_registry is not *ProviderRegistry")
		return
	}

	sessionInstance, err := facades.App().Make("auth.session_store")
	if err != nil {
		facades.Log().Errorf("Failed to resolve auth.session_store: %v", err)
		return
	}
	sessions, ok := sessionInstance.(*authservice.SessionStore)
	if !ok {
		facades.Log().Errorf("auth.session_store is not *SessionStore")
		return
	}

	bridgeInstance, err := facades.App().Make("auth.bridge")
	if err != nil {
		facades.Log().Errorf("Failed to resolve auth.bridge: %v", err)
		return
	}
	bridge, ok := bridgeInstance.(*authservice.AuthBridge)
	if !ok {
		facades.Log().Errorf("auth.bridge is not *AuthBridge")
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
