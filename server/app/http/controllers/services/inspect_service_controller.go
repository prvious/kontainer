package services

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type InspectServiceController struct {
	action *serviceActions.InspectServiceAction
}

func NewInspectServiceController(action *serviceActions.InspectServiceAction) *InspectServiceController {
	return &InspectServiceController{action: action}
}

func (c *InspectServiceController) Handle(ctx http.Context) http.Response {
	serviceID := ctx.Request().Route("serviceID")

	detail, err := c.action.Execute(ctx.Context(), serviceID)
	if err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "service not found"})
		}
		facades.Log().Errorf("inspect service error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to inspect service"})
	}

	return ctx.Response().Json(200, detail)
}
