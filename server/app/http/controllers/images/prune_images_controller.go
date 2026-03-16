package images

import (
	"github.com/goravel/framework/contracts/http"
	imageActions "github.com/prvious/kontainer/server/app/actions/images"
	"github.com/prvious/kontainer/server/app/facades"
)

type PruneImagesController struct {
	action *imageActions.PruneImagesAction
}

func NewPruneImagesController(action *imageActions.PruneImagesAction) *PruneImagesController {
	return &PruneImagesController{action: action}
}

func (c *PruneImagesController) Handle(ctx http.Context) http.Response {
	nodeID := ctx.Request().Query("nodeId", "")

	resp, err := c.action.Execute(ctx.Context(), nodeID)
	if err != nil {
		facades.Log().Errorf("prune images error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to prune images"})
	}

	return ctx.Response().Json(200, resp)
}
