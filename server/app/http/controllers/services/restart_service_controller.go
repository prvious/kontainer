package services

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type RestartServiceController struct {
	action *serviceActions.RestartServiceAction
}

func NewRestartServiceController(action *serviceActions.RestartServiceAction) *RestartServiceController {
	return &RestartServiceController{action: action}
}

func (c *RestartServiceController) Handle(ctx http.Context) http.Response {
	serviceID := ctx.Request().Route("serviceID")

	if err := c.action.Execute(ctx.Context(), serviceID); err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "service not found"})
		}
		facades.Log().Errorf("restart service error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to restart service"})
	}

	return ctx.Response().Json(200, map[string]string{"status": "restarting"})
}
