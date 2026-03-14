package tasks

import (
	"github.com/goravel/framework/contracts/http"
	taskActions "github.com/prvious/kontainer/server/app/actions/tasks"
	"github.com/prvious/kontainer/server/app/facades"
)

type ListTaskStatsController struct {
	action *taskActions.ListTaskStatsAction
}

func NewListTaskStatsController(action *taskActions.ListTaskStatsAction) *ListTaskStatsController {
	return &ListTaskStatsController{action: action}
}

func (c *ListTaskStatsController) Handle(ctx http.Context) http.Response {
	stats, err := c.action.Execute(ctx.Context())
	if err != nil {
		facades.Log().Errorf("list task stats error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to list task stats"})
	}

	return ctx.Response().Json(200, map[string]any{"stats": stats})
}
