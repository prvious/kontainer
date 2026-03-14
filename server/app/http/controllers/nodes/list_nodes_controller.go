package nodes

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/nodes"
	"github.com/prvious/kontainer/server/app/facades"
)

type ListNodesController struct {
	action *nodes.ListNodesAction
}

func NewListNodesController(action *nodes.ListNodesAction) *ListNodesController {
	return &ListNodesController{action: action}
}

func (c *ListNodesController) Handle(ctx http.Context) http.Response {
	nodeList, err := c.action.Execute(ctx.Context())
	if err != nil {
		facades.Log().Errorf("list nodes error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to list nodes"})
	}

	return ctx.Response().Json(200, map[string]any{"nodes": nodeList})
}
