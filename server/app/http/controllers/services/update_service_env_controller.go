package services

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type UpdateServiceEnvController struct {
	action *serviceActions.UpdateServiceEnvAction
}

func NewUpdateServiceEnvController(action *serviceActions.UpdateServiceEnvAction) *UpdateServiceEnvController {
	return &UpdateServiceEnvController{action: action}
}

type updateEnvRequest struct {
	Env []string `json:"env"`
}

func (c *UpdateServiceEnvController) Handle(ctx http.Context) http.Response {
	serviceID := ctx.Request().Route("serviceID")

	var req updateEnvRequest
	if err := ctx.Request().Bind(&req); err != nil {
		return ctx.Response().Json(400, map[string]string{"error": "invalid request body"})
	}

	if err := c.action.Execute(ctx.Context(), serviceID, req.Env); err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "service not found"})
		}
		facades.Log().Errorf("update service env error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to update service env"})
	}

	return ctx.Response().Json(200, map[string]string{"status": "updated"})
}
