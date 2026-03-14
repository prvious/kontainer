package tasks

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	taskActions "github.com/prvious/kontainer/server/app/actions/tasks"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type InspectTaskController struct {
	action *taskActions.InspectTaskAction
}

func NewInspectTaskController(action *taskActions.InspectTaskAction) *InspectTaskController {
	return &InspectTaskController{action: action}
}

func (c *InspectTaskController) Handle(ctx http.Context) http.Response {
	taskID := ctx.Request().Route("taskID")

	detail, err := c.action.Execute(ctx.Context(), taskID)
	if err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "task not found"})
		}
		facades.Log().Errorf("inspect task error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to inspect task"})
	}

	return ctx.Response().Json(200, detail)
}
