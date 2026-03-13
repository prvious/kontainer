package setup

import (
	"fmt"
	nethttp "net/http"

	contractshttp "github.com/goravel/framework/contracts/http"

	setupactions "github.com/prvious/kontainer/server/app/actions/setup"
	"github.com/prvious/kontainer/server/app/facades"
)

type CallbackController struct {
	manifestState *ManifestState
	exchangeCode  *setupactions.ExchangeCodeAction
}

func NewCallbackController(state *ManifestState, exchangeCode *setupactions.ExchangeCodeAction) *CallbackController {
	return &CallbackController{manifestState: state, exchangeCode: exchangeCode}
}

func (c *CallbackController) Handle(ctx contractshttp.Context) contractshttp.Response {
	if ctx.Request().Query("setup_action") != "" {
		html := `<!DOCTYPE html>
<html>
<head><title>Kontainer Setup</title></head>
<body>
<h1>GitHub App Installed!</h1>
<p>Return to the Kontainer app to complete setup.</p>
</body>
</html>`
		return ctx.Response().Data(nethttp.StatusOK, "text/html; charset=utf-8", []byte(html))
	}

	code := ctx.Request().Query("code")
	if code == "" {
		return ctx.Response().Json(nethttp.StatusBadRequest, map[string]any{"error": "missing code parameter"})
	}

	creds, err := c.exchangeCode.Execute(code)
	if err != nil {
		facades.Log().Errorf("GitHub App code exchange failed: %v", err)
		return ctx.Response().Data(nethttp.StatusBadGateway, "text/html; charset=utf-8",
			[]byte(renderErrorHTML("Failed to exchange code with GitHub. Please try again.")))
	}

	c.manifestState.ClearState()

	installURL := fmt.Sprintf("https://github.com/apps/%s/installations/new", creds.Slug)
	return ctx.Response().Redirect(nethttp.StatusFound, installURL)
}
