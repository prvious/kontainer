package secrets

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/secrets"
	"github.com/prvious/kontainer/server/app/facades"
)

type ListSecretsController struct {
	action *secrets.ListSecretsAction
}

func NewListSecretsController(action *secrets.ListSecretsAction) *ListSecretsController {
	return &ListSecretsController{action: action}
}

func (c *ListSecretsController) Handle(ctx http.Context) http.Response {
	secretList, err := c.action.Execute(ctx.Context())
	if err != nil {
		facades.Log().Errorf("list secrets error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to list secrets"})
	}

	return ctx.Response().Json(200, map[string]any{"secrets": secretList})
}
