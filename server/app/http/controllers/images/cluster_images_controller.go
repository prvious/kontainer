package images

import (
	"github.com/goravel/framework/contracts/http"
	imageActions "github.com/prvious/kontainer/server/app/actions/images"
	"github.com/prvious/kontainer/server/app/facades"
)

type ClusterImagesController struct {
	action *imageActions.ListClusterImagesAction
}

func NewClusterImagesController(action *imageActions.ListClusterImagesAction) *ClusterImagesController {
	return &ClusterImagesController{action: action}
}

func (c *ClusterImagesController) Handle(ctx http.Context) http.Response {
	resp, err := c.action.Execute(ctx.Context())
	if err != nil {
		facades.Log().Errorf("list cluster images error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to list cluster images"})
	}

	return ctx.Response().Json(200, resp)
}
