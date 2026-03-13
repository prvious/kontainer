package auth

import (
	nethttp "net/http"

	contractshttp "github.com/goravel/framework/contracts/http"

	authservice "github.com/prvious/kontainer/server/app/services/auth"
	authproviders "github.com/prvious/kontainer/server/app/services/auth/providers"
)

type RedirectController struct {
	registry *authproviders.ProviderRegistry
	sessions *authservice.SessionStore
}

func NewRedirectController(
	registry *authproviders.ProviderRegistry,
	sessions *authservice.SessionStore,
) *RedirectController {
	return &RedirectController{registry: registry, sessions: sessions}
}

func (c *RedirectController) Handle(ctx contractshttp.Context) contractshttp.Response {
	providerName := ctx.Request().Route("provider")

	provider, err := c.registry.Get(providerName)
	if err != nil {
		return ctx.Response().Json(nethttp.StatusNotFound, map[string]any{
			"error": err.Error(),
		})
	}

	sessionID := ctx.Request().Query("session")
	if sessionID == "" {
		return ctx.Response().Json(nethttp.StatusBadRequest, map[string]any{
			"error": "missing session parameter",
		})
	}

	state, err := c.sessions.Generate(sessionID)
	if err != nil {
		return ctx.Response().Json(nethttp.StatusInternalServerError, map[string]any{
			"error": "failed to generate state",
		})
	}

	authURL := provider.OAuthConfig().AuthCodeURL(state)
	return ctx.Response().Redirect(nethttp.StatusFound, authURL)
}
