package services

import (
	"github.com/goravel/framework/contracts/http"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListServicesController struct {
	action *serviceActions.ListServicesAction
}

func NewListServicesController(action *serviceActions.ListServicesAction) *ListServicesController {
	return &ListServicesController{action: action}
}

func (c *ListServicesController) Handle(ctx http.Context) http.Response {
	stack := ctx.Request().Query("stack", "")

	services, err := c.action.Execute(ctx.Context(), docker.ServiceFilters{Stack: stack})
	if err != nil {
		facades.Log().Errorf("list services error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to list services"})
	}

	return ctx.Response().Json(200, map[string]any{"services": services})
}
