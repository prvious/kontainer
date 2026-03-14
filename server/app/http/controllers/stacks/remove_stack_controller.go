package stacks

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/stacks"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type RemoveStackController struct {
	action *stacks.RemoveStackAction
}

func NewRemoveStackController(action *stacks.RemoveStackAction) *RemoveStackController {
	return &RemoveStackController{action: action}
}

func (c *RemoveStackController) Handle(ctx http.Context) http.Response {
	stackName := ctx.Request().Route("stackName")

	if err := c.action.Execute(ctx.Context(), stackName); err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "stack not found"})
		}
		facades.Log().Errorf("remove stack error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to remove stack"})
	}

	return ctx.Response().Json(200, map[string]string{"status": "removed"})
}
