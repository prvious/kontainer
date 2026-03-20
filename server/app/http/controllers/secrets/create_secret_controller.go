package secrets

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/secrets"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/http/requests"
)

type CreateSecretController struct {
	action *secrets.CreateSecretAction
}

func NewCreateSecretController(action *secrets.CreateSecretAction) *CreateSecretController {
	return &CreateSecretController{action: action}
}

func (c *CreateSecretController) Handle(ctx http.Context) http.Response {
	var req requests.CreateSecretRequest
	errors, err := ctx.Request().ValidateRequest(&req)
	if err != nil {
		return ctx.Response().Json(400, map[string]string{"error": err.Error()})
	}
	if errors != nil {
		return ctx.Response().Json(422, map[string]any{"errors": errors.All()})
	}

	if err := c.action.Execute(ctx.Context(), req.Name, []byte(req.Data)); err != nil {
		facades.Log().Errorf("create secret error [name=%s]: %v", req.Name, err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to create secret"})
	}

	return ctx.Response().Json(201, map[string]string{"name": req.Name})
}
