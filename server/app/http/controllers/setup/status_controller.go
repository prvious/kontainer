package setup

import (
	nethttp "net/http"

	contractshttp "github.com/goravel/framework/contracts/http"

	setupactions "github.com/prvious/kontainer/server/app/actions/setup"
)

type StatusController struct {
	checkStatus *setupactions.CheckStatusAction
}

func NewStatusController(checkStatus *setupactions.CheckStatusAction) *StatusController {
	return &StatusController{checkStatus: checkStatus}
}

func (c *StatusController) Handle(ctx contractshttp.Context) contractshttp.Response {
	step, err := c.checkStatus.Execute()
	if err != nil {
		return ctx.Response().Json(nethttp.StatusInternalServerError, map[string]any{
			"error": "failed to check setup status",
		})
	}

	return ctx.Response().Json(nethttp.StatusOK, map[string]any{"step": step})
}
