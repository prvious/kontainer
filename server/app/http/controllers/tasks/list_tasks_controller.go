package tasks

import (
	"github.com/goravel/framework/contracts/http"
	taskActions "github.com/prvious/kontainer/server/app/actions/tasks"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListTasksController struct {
	action *taskActions.ListTasksAction
}

func NewListTasksController(action *taskActions.ListTasksAction) *ListTasksController {
	return &ListTasksController{action: action}
}

func (c *ListTasksController) Handle(ctx http.Context) http.Response {
	// Support both query params (?serviceID=) and route params ({serviceID}, {nodeID})
	serviceID := ctx.Request().Route("serviceID")
	if serviceID == "" {
		serviceID = ctx.Request().Query("serviceID", "")
	}
	nodeID := ctx.Request().Route("nodeID")
	if nodeID == "" {
		nodeID = ctx.Request().Query("nodeID", "")
	}

	filters := docker.TaskFilters{
		ServiceID: serviceID,
		NodeID:    nodeID,
	}

	tasks, err := c.action.Execute(ctx.Context(), filters)
	if err != nil {
		facades.Log().Errorf("list tasks error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to list tasks"})
	}

	return ctx.Response().Json(200, map[string]any{"tasks": tasks})
}
