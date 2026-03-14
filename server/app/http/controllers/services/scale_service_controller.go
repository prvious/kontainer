package services

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type ScaleServiceController struct {
	action *serviceActions.ScaleServiceAction
}

func NewScaleServiceController(action *serviceActions.ScaleServiceAction) *ScaleServiceController {
	return &ScaleServiceController{action: action}
}

type scaleRequest struct {
	Replicas *uint64 `form:"replicas" json:"replicas"`
}

func (c *ScaleServiceController) Handle(ctx http.Context) http.Response {
	serviceID := ctx.Request().Route("serviceID")

	var req scaleRequest
	if err := ctx.Request().Bind(&req); err != nil {
		return ctx.Response().Json(400, map[string]string{"error": "invalid request body"})
	}

	if req.Replicas == nil {
		return ctx.Response().Json(400, map[string]string{"error": "replicas field is required"})
	}

	err := c.action.Execute(ctx.Context(), serviceID, *req.Replicas)
	if err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "service not found"})
		}
		if errors.Is(err, docker.ErrGlobalMode) {
			return ctx.Response().Json(400, map[string]string{"error": "cannot scale a global service"})
		}
		facades.Log().Errorf("scale service error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to scale service"})
	}

	return ctx.Response().Json(200, map[string]any{"replicas": *req.Replicas})
}
