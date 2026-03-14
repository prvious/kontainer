package configs

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/configs"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type RemoveConfigController struct {
	action *configs.RemoveConfigAction
}

func NewRemoveConfigController(action *configs.RemoveConfigAction) *RemoveConfigController {
	return &RemoveConfigController{action: action}
}

func (c *RemoveConfigController) Handle(ctx http.Context) http.Response {
	configID := ctx.Request().Route("configID")

	if err := c.action.Execute(ctx.Context(), configID); err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "config not found"})
		}
		facades.Log().Errorf("remove config error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to remove config"})
	}

	return ctx.Response().Json(200, map[string]string{"status": "removed"})
}
