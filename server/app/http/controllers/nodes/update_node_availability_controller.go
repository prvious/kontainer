package nodes

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/actions/nodes"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/http/requests"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type UpdateNodeAvailabilityController struct {
	action *nodes.UpdateNodeAvailabilityAction
}

func NewUpdateNodeAvailabilityController(action *nodes.UpdateNodeAvailabilityAction) *UpdateNodeAvailabilityController {
	return &UpdateNodeAvailabilityController{action: action}
}

func (c *UpdateNodeAvailabilityController) Handle(ctx http.Context) http.Response {
	nodeID := ctx.Request().Route("nodeID")

	var req requests.UpdateNodeAvailabilityRequest
	errs, err := ctx.Request().ValidateRequest(&req)
	if err != nil {
		return ctx.Response().Json(400, map[string]string{"error": err.Error()})
	}
	if errs != nil {
		return ctx.Response().Json(422, map[string]any{"errors": errs.All()})
	}

	if err := c.action.Execute(ctx.Context(), nodeID, req.Availability); err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "node not found"})
		}
		facades.Log().Errorf("update node availability error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to update node availability"})
	}

	return ctx.Response().Json(200, map[string]string{"availability": req.Availability})
}
