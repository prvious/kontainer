package setup

import (
	"crypto/subtle"
	"encoding/json"
	"fmt"
	"html"
	nethttp "net/http"
	"net/url"

	contractshttp "github.com/goravel/framework/contracts/http"

	"github.com/prvious/kontainer/server/app/facades"
	setupservice "github.com/prvious/kontainer/server/app/services/setup"
)

type StartController struct {
	manifestState *setupservice.ManifestState
}

func NewStartController(state *setupservice.ManifestState) *StartController {
	return &StartController{manifestState: state}
}

func (c *StartController) Handle(ctx contractshttp.Context) contractshttp.Response {
	queryToken := ctx.Request().Query("token")

	state, storedToken, org := c.manifestState.Get()

	if state == "" || storedToken == "" {
		return ctx.Response().Data(nethttp.StatusForbidden, "text/html; charset=utf-8",
			[]byte(renderErrorHTML("No setup session found. Start the setup flow first.")))
	}

	if subtle.ConstantTimeCompare([]byte(queryToken), []byte(storedToken)) != 1 {
		return ctx.Response().Data(nethttp.StatusUnauthorized, "text/html; charset=utf-8",
			[]byte(renderErrorHTML("Invalid or expired setup token.")))
	}

	agentURL := facades.Config().GetString("cluster.agent_url", "http://localhost:3000")

	appName := "Kontainer"
	if org != "" {
		appName = fmt.Sprintf("Kontainer (%s)", org)
	}

	manifest := map[string]interface{}{
		"name":          appName,
		"url":           agentURL,
		"callback_urls": []string{agentURL + "/auth/github/callback"},
		"redirect_url":  agentURL + "/setup/github/callback",
		"setup_url":     agentURL + "/setup/github/callback",
		"public":        false,
		"default_permissions": map[string]string{
			"members": "read",
			"emails":  "read",
		},
		"default_events": []string{},
	}

	manifestJSON, err := json.Marshal(manifest)
	if err != nil {
		return ctx.Response().Data(nethttp.StatusInternalServerError, "text/html; charset=utf-8",
			[]byte(renderErrorHTML("Failed to generate manifest.")))
	}

	c.manifestState.ClearToken()

	var githubURL string
	if org != "" {
		githubURL = fmt.Sprintf("https://github.com/organizations/%s/settings/apps/new?state=%s", url.PathEscape(org), state)
	} else {
		githubURL = fmt.Sprintf("https://github.com/settings/apps/new?state=%s", state)
	}

	html := fmt.Sprintf(`<!DOCTYPE html>
<html>
<head><title>Kontainer Setup</title></head>
<body>
<p>Redirecting to GitHub...</p>
<form id="manifest-form" action="%s" method="post">
<input type="hidden" name="manifest" value='%s'>
</form>
<script>document.getElementById('manifest-form').submit();</script>
</body>
</html>`, githubURL, htmlEscapeJSON(string(manifestJSON)))

	return ctx.Response().Data(nethttp.StatusOK, "text/html; charset=utf-8", []byte(html))
}

func renderErrorHTML(message string) string {
	return fmt.Sprintf(`<!DOCTYPE html>
<html>
<head><title>Kontainer Setup - Error</title></head>
<body>
<h1>Setup Error</h1>
<p>%s</p>
</body>
</html>`, html.EscapeString(message))
}

func htmlEscapeJSON(s string) string {
	result := make([]byte, 0, len(s))
	for i := 0; i < len(s); i++ {
		switch s[i] {
		case '\'':
			result = append(result, []byte("&#39;")...)
		case '&':
			result = append(result, []byte("&amp;")...)
		case '<':
			result = append(result, []byte("&lt;")...)
		case '>':
			result = append(result, []byte("&gt;")...)
		default:
			result = append(result, s[i])
		}
	}
	return string(result)
}
