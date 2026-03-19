package metrics

import (
	"time"

	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/facades"
	metricsActions "github.com/prvious/kontainer/server/app/actions/metrics"
)

type ClusterMetricsController struct {
	action *metricsActions.GetClusterMetricsAction
}

func NewClusterMetricsController(action *metricsActions.GetClusterMetricsAction) *ClusterMetricsController {
	return &ClusterMetricsController{action: action}
}

func (c *ClusterMetricsController) Handle(ctx http.Context) http.Response {
	rangeDuration := 1 * time.Hour
	switch ctx.Request().Query("range", "1h") {
	case "6h":
		rangeDuration = 6 * time.Hour
	case "24h":
		rangeDuration = 24 * time.Hour
	}

	resp, err := c.action.Execute(ctx.Context(), rangeDuration)
	if err != nil {
		facades.Log().Errorf("cluster metrics controller: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to collect metrics"})
	}
	return ctx.Response().Json(200, resp)
}
