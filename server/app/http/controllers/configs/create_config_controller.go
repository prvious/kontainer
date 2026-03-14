package configs

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/configs"
	"github.com/prvious/kontainer/server/app/facades"
)

type CreateConfigController struct {
	action *configs.CreateConfigAction
}

func NewCreateConfigController(action *configs.CreateConfigAction) *CreateConfigController {
	return &CreateConfigController{action: action}
}

type createConfigRequest struct {
	Name string `json:"name"`
	Data string `json:"data"`
}

func (c *CreateConfigController) Handle(ctx http.Context) http.Response {
	var req createConfigRequest
	if err := ctx.Request().Bind(&req); err != nil {
		return ctx.Response().Json(400, map[string]string{"error": "invalid request body"})
	}

	if req.Name == "" {
		return ctx.Response().Json(400, map[string]string{"error": "name is required"})
	}

	if err := c.action.Execute(ctx.Context(), req.Name, []byte(req.Data)); err != nil {
		facades.Log().Errorf("create config error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to create config"})
	}

	return ctx.Response().Json(201, map[string]string{"name": req.Name})
}
