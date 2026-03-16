package images

import (
	"github.com/goravel/framework/contracts/http"
	imageActions "github.com/prvious/kontainer/server/app/actions/images"
	"github.com/prvious/kontainer/server/app/facades"
)

type RemoveImageController struct {
	action *imageActions.RemoveImageAction
}

func NewRemoveImageController(action *imageActions.RemoveImageAction) *RemoveImageController {
	return &RemoveImageController{action: action}
}

func (c *RemoveImageController) Handle(ctx http.Context) http.Response {
	imageID := ctx.Request().Route("imageID")
	nodeID := ctx.Request().Query("nodeId", "")
	force := ctx.Request().QueryBool("force", false)

	resp, err := c.action.Execute(ctx.Context(), imageID, nodeID, force)
	if err != nil {
		facades.Log().Errorf("remove image error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to remove image"})
	}

	return ctx.Response().Json(200, resp)
}
