package volumes

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/volumes"
	"github.com/prvious/kontainer/server/app/facades"
)

type ListVolumesController struct {
	action *volumes.ListVolumesAction
}

func NewListVolumesController(action *volumes.ListVolumesAction) *ListVolumesController {
	return &ListVolumesController{action: action}
}

func (c *ListVolumesController) Handle(ctx http.Context) http.Response {
	volumeList, err := c.action.Execute(ctx.Context())
	if err != nil {
		facades.Log().Errorf("list volumes error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to list volumes"})
	}

	return ctx.Response().Json(200, map[string]any{"volumes": volumeList})
}
