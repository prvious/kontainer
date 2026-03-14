package networks

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/networks"
	"github.com/prvious/kontainer/server/app/facades"
)

type ListNetworksController struct {
	action *networks.ListNetworksAction
}

func NewListNetworksController(action *networks.ListNetworksAction) *ListNetworksController {
	return &ListNetworksController{action: action}
}

func (c *ListNetworksController) Handle(ctx http.Context) http.Response {
	networkList, err := c.action.Execute(ctx.Context())
	if err != nil {
		facades.Log().Errorf("list networks error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to list networks"})
	}

	return ctx.Response().Json(200, map[string]any{"networks": networkList})
}
