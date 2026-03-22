package metrics

import (
	"errors"

	"github.com/goravel/framework/contracts/http"

	metricsActions "github.com/prvious/kontainer/server/app/actions/metrics"
	"github.com/prvious/kontainer/server/app/facades"
)

type NodeMetricsController struct {
	action *metricsActions.GetNodeMetricsAction
}

func NewNodeMetricsController(action *metricsActions.GetNodeMetricsAction) *NodeMetricsController {
	return &NodeMetricsController{action: action}
}

func (c *NodeMetricsController) Handle(ctx http.Context) http.Response {
	nodeID := ctx.Request().Route("nodeID")
	if nodeID == "" {
		return ctx.Response().Json(400, map[string]string{"error": "missing nodeID"})
	}

	snapshot, err := c.action.Execute(ctx.Context(), nodeID)
	if err != nil {
		if errors.Is(err, metricsActions.ErrNoRecentMetrics) {
			return ctx.Response().Json(404, map[string]string{"error": "no recent metrics for this node"})
		}
		facades.Log().Errorf("node metrics controller: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to get node metrics"})
	}
	return ctx.Response().Json(200, snapshot)
}
