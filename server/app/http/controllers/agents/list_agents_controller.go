package agents

import (
	"github.com/goravel/framework/contracts/http"
	agentActions "github.com/prvious/kontainer/server/app/actions/agents"
)

type ListAgentsController struct {
	action *agentActions.ListAgentsAction
}

func NewListAgentsController(action *agentActions.ListAgentsAction) *ListAgentsController {
	return &ListAgentsController{action: action}
}

func (c *ListAgentsController) Handle(ctx http.Context) http.Response {
	agents := c.action.Execute()
	return ctx.Response().Json(200, map[string]any{
		"agents": agents,
	})
}
