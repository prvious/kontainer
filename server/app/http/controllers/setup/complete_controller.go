package setup

import (
	nethttp "net/http"

	contractshttp "github.com/goravel/framework/contracts/http"

	setupactions "github.com/prvious/kontainer/server/app/actions/setup"
	"github.com/prvious/kontainer/server/app/facades"
)

type CompleteController struct {
	completeSetup *setupactions.CompleteSetupAction
}

func NewCompleteController(completeSetup *setupactions.CompleteSetupAction) *CompleteController {
	return &CompleteController{completeSetup: completeSetup}
}

func (c *CompleteController) Handle(ctx contractshttp.Context) contractshttp.Response {
	if err := c.completeSetup.Execute(); err != nil {
		facades.Log().Errorf("setup completion failed: %v", err)
		return ctx.Response().Json(nethttp.StatusInternalServerError, map[string]any{"error": "setup completion failed"})
	}

	return ctx.Response().Json(nethttp.StatusOK, map[string]any{"status": "complete"})
}
