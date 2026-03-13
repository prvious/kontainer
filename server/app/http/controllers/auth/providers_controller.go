package auth

import (
	contractshttp "github.com/goravel/framework/contracts/http"

	authproviders "github.com/prvious/kontainer/server/app/services/auth/providers"
)

type ProvidersController struct {
	registry *authproviders.ProviderRegistry
}

func NewProvidersController(registry *authproviders.ProviderRegistry) *ProvidersController {
	return &ProvidersController{registry: registry}
}

func (c *ProvidersController) Handle(ctx contractshttp.Context) contractshttp.Response {
	return ctx.Response().Json(200, map[string]any{
		"providers": c.registry.Names(),
	})
}
