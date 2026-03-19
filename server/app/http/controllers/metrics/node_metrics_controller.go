package metrics

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/facades"
	metricsActions "github.com/prvious/kontainer/server/app/actions/metrics"
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
		facades.Log().Errorf("node metrics controller: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to get node metrics"})
	}
	return ctx.Response().Json(200, snapshot)
}
