package services

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type RollbackServiceController struct {
	action *serviceActions.RollbackServiceAction
}

func NewRollbackServiceController(action *serviceActions.RollbackServiceAction) *RollbackServiceController {
	return &RollbackServiceController{action: action}
}

func (c *RollbackServiceController) Handle(ctx http.Context) http.Response {
	serviceID := ctx.Request().Route("serviceID")

	if err := c.action.Execute(ctx.Context(), serviceID); err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "service not found"})
		}
		facades.Log().Errorf("rollback service error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to rollback service"})
	}

	return ctx.Response().Json(200, map[string]string{"status": "rolling_back"})
}
