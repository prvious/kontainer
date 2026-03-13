package setup

import (
	nethttp "net/http"

	contractshttp "github.com/goravel/framework/contracts/http"

	setupactions "github.com/prvious/kontainer/server/app/actions/setup"
)

type CompleteController struct {
	completeSetup *setupactions.CompleteSetupAction
}

func NewCompleteController(completeSetup *setupactions.CompleteSetupAction) *CompleteController {
	return &CompleteController{completeSetup: completeSetup}
}

func (c *CompleteController) Handle(ctx contractshttp.Context) contractshttp.Response {
	if err := c.completeSetup.Execute(); err != nil {
		return ctx.Response().Json(nethttp.StatusBadRequest, map[string]any{"error": err.Error()})
	}

	return ctx.Response().Json(nethttp.StatusOK, map[string]any{"status": "complete"})
}
