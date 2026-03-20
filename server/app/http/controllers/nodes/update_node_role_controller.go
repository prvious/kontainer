package nodes

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/nodes"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/http/requests"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type UpdateNodeRoleController struct {
	action *nodes.UpdateNodeRoleAction
}

func NewUpdateNodeRoleController(action *nodes.UpdateNodeRoleAction) *UpdateNodeRoleController {
	return &UpdateNodeRoleController{action: action}
}

func (c *UpdateNodeRoleController) Handle(ctx http.Context) http.Response {
	nodeID := ctx.Request().Route("nodeID")

	var req requests.UpdateNodeRoleRequest
	errs, err := ctx.Request().ValidateRequest(&req)
	if err != nil {
		return ctx.Response().Json(400, map[string]string{"error": err.Error()})
	}
	if errs != nil {
		return ctx.Response().Json(422, map[string]any{"errors": errs.All()})
	}

	if err := c.action.Execute(ctx.Context(), nodeID, req.Role); err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "node not found"})
		}
		facades.Log().Errorf("update node role error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to update node role"})
	}

	return ctx.Response().Json(200, map[string]string{"role": req.Role})
}
