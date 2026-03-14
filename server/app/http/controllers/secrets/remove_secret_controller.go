package secrets

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/secrets"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type RemoveSecretController struct {
	action *secrets.RemoveSecretAction
}

func NewRemoveSecretController(action *secrets.RemoveSecretAction) *RemoveSecretController {
	return &RemoveSecretController{action: action}
}

func (c *RemoveSecretController) Handle(ctx http.Context) http.Response {
	secretID := ctx.Request().Route("secretID")

	if err := c.action.Execute(ctx.Context(), secretID); err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "secret not found"})
		}
		facades.Log().Errorf("remove secret error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to remove secret"})
	}

	return ctx.Response().Json(200, map[string]string{"status": "removed"})
}
