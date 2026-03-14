package services

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type UpdateServiceImageController struct {
	action *serviceActions.UpdateServiceImageAction
}

func NewUpdateServiceImageController(action *serviceActions.UpdateServiceImageAction) *UpdateServiceImageController {
	return &UpdateServiceImageController{action: action}
}

type updateImageRequest struct {
	Image string `json:"image"`
}

func (c *UpdateServiceImageController) Handle(ctx http.Context) http.Response {
	serviceID := ctx.Request().Route("serviceID")

	var req updateImageRequest
	if err := ctx.Request().Bind(&req); err != nil {
		return ctx.Response().Json(400, map[string]string{"error": "invalid request body"})
	}

	if req.Image == "" {
		return ctx.Response().Json(400, map[string]string{"error": "image field is required"})
	}

	if err := c.action.Execute(ctx.Context(), serviceID, req.Image); err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "service not found"})
		}
		facades.Log().Errorf("update service image error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to update service image"})
	}

	return ctx.Response().Json(200, map[string]string{"status": "updated"})
}
