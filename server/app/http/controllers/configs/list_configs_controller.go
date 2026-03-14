package configs

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/configs"
	"github.com/prvious/kontainer/server/app/facades"
)

type ListConfigsController struct {
	action *configs.ListConfigsAction
}

func NewListConfigsController(action *configs.ListConfigsAction) *ListConfigsController {
	return &ListConfigsController{action: action}
}

func (c *ListConfigsController) Handle(ctx http.Context) http.Response {
	configList, err := c.action.Execute(ctx.Context())
	if err != nil {
		facades.Log().Errorf("list configs error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to list configs"})
	}

	return ctx.Response().Json(200, map[string]any{"configs": configList})
}
