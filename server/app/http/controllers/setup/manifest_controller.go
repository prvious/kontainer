package setup

import (
	"fmt"

	contractshttp "github.com/goravel/framework/contracts/http"

	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/helpers"
	setupservice "github.com/prvious/kontainer/server/app/services/setup"
)

type ManifestController struct {
	manifestState *setupservice.ManifestState
}

func NewManifestController(state *setupservice.ManifestState) *ManifestController {
	return &ManifestController{manifestState: state}
}

func (c *ManifestController) Handle(ctx contractshttp.Context) contractshttp.Response {
	org := ctx.Request().Input("org")

	state := helpers.RandomHex(16)
	token := helpers.RandomHex(16)

	c.manifestState.Set(state, token, org)

	agentURL := facades.Config().GetString("cluster.agent_url", "http://localhost:3000")
	setupURL := fmt.Sprintf("%s/setup/github/start?token=%s", agentURL, token)

	return ctx.Response().Json(200, map[string]any{"setupUrl": setupURL})
}
