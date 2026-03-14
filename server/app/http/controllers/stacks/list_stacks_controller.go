package stacks

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/stacks"
	"github.com/prvious/kontainer/server/app/facades"
)

type ListStacksController struct {
	action *stacks.ListStacksAction
}

func NewListStacksController(action *stacks.ListStacksAction) *ListStacksController {
	return &ListStacksController{action: action}
}

func (c *ListStacksController) Handle(ctx http.Context) http.Response {
	stackList, err := c.action.Execute(ctx.Context())
	if err != nil {
		facades.Log().Errorf("list stacks error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to list stacks"})
	}

	return ctx.Response().Json(200, map[string]any{"stacks": stackList})
}
