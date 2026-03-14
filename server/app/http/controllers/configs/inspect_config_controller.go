package configs

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/configs"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type InspectConfigController struct {
	action *configs.InspectConfigAction
}

func NewInspectConfigController(action *configs.InspectConfigAction) *InspectConfigController {
	return &InspectConfigController{action: action}
}

func (c *InspectConfigController) Handle(ctx http.Context) http.Response {
	configID := ctx.Request().Route("configID")

	detail, err := c.action.Execute(ctx.Context(), configID)
	if err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "config not found"})
		}
		facades.Log().Errorf("inspect config error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to inspect config"})
	}

	return ctx.Response().Json(200, detail)
}
