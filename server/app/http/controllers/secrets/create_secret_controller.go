package secrets

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/secrets"
	"github.com/prvious/kontainer/server/app/facades"
)

type CreateSecretController struct {
	action *secrets.CreateSecretAction
}

func NewCreateSecretController(action *secrets.CreateSecretAction) *CreateSecretController {
	return &CreateSecretController{action: action}
}

type createSecretRequest struct {
	Name string `json:"name"`
	Data string `json:"data"`
}

func (c *CreateSecretController) Handle(ctx http.Context) http.Response {
	var req createSecretRequest
	if err := ctx.Request().Bind(&req); err != nil {
		return ctx.Response().Json(400, map[string]string{"error": "invalid request body"})
	}

	if req.Name == "" {
		return ctx.Response().Json(400, map[string]string{"error": "name is required"})
	}

	if err := c.action.Execute(ctx.Context(), req.Name, []byte(req.Data)); err != nil {
		facades.Log().Errorf("create secret error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to create secret"})
	}

	return ctx.Response().Json(201, map[string]string{"name": req.Name})
}
