package auth

import (
	"errors"
	nethttp "net/http"

	contractshttp "github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/facades"

	authactions "github.com/prvious/kontainer/server/app/actions/auth"
	authservice "github.com/prvious/kontainer/server/app/services/auth"
	authproviders "github.com/prvious/kontainer/server/app/services/auth/providers"
)

type CallbackController struct {
	registry   *authproviders.ProviderRegistry
	sessions   *authservice.SessionStore
	bridge     *authservice.AuthBridge
	upsertUser *authactions.UpsertUserAction
}

func NewCallbackController(
	registry *authproviders.ProviderRegistry,
	sessions *authservice.SessionStore,
	bridge *authservice.AuthBridge,
	upsertUser *authactions.UpsertUserAction,
) *CallbackController {
	return &CallbackController{
		registry:   registry,
		sessions:   sessions,
		bridge:     bridge,
		upsertUser: upsertUser,
	}
}

const successHTML = `<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>Authenticated</title></head>
<body style="display:flex;align-items:center;justify-content:center;height:100vh;font-family:system-ui,sans-serif">
<p>Authentication successful. You can close this tab.</p>
</body>
</html>`

func (c *CallbackController) Handle(ctx contractshttp.Context) contractshttp.Response {
	providerName := ctx.Request().Route("provider")

	provider, err := c.registry.Get(providerName)
	if err != nil {
		return ctx.Response().Json(nethttp.StatusNotFound, map[string]any{"error": err.Error()})
	}

	code := ctx.Request().Query("code")
	state := ctx.Request().Query("state")

	if code == "" || state == "" {
		return ctx.Response().Json(nethttp.StatusBadRequest, map[string]any{"error": "missing code or state"})
	}

	sessionID, ok := c.sessions.Validate(state)
	if !ok {
		return ctx.Response().Json(nethttp.StatusBadRequest, map[string]any{"error": "invalid or expired state"})
	}

	oauthToken, err := provider.OAuthConfig().Exchange(ctx.Request().Origin().Context(), code)
	if err != nil {
		facades.Log().Errorf("oauth exchange error: %v", err)
		return ctx.Response().Json(nethttp.StatusBadGateway, map[string]any{"error": "oauth exchange failed"})
	}

	providerUser, err := provider.FetchUser(ctx.Request().Origin().Context(), oauthToken)
	if err != nil {
		var accessErr *authproviders.AccessDeniedError
		if errors.As(err, &accessErr) {
			return ctx.Response().Json(nethttp.StatusForbidden, map[string]any{"error": accessErr.Message})
		}
		facades.Log().Errorf("fetch user error: %v", err)
		return ctx.Response().Json(nethttp.StatusBadGateway, map[string]any{"error": "failed to fetch user"})
	}

	user, err := c.upsertUser.Execute(providerUser)
	if err != nil {
		facades.Log().Errorf("upsert user error: %v", err)
		return ctx.Response().Json(nethttp.StatusInternalServerError, map[string]any{"error": "failed to save user"})
	}

	token, err := facades.Auth(ctx).LoginUsingID(user.ID)
	if err != nil {
		facades.Log().Errorf("jwt creation error: %v", err)
		return ctx.Response().Json(nethttp.StatusInternalServerError, map[string]any{"error": "failed to create token"})
	}

	c.bridge.SendToken(sessionID, token)

	return ctx.Response().Data(nethttp.StatusOK, "text/html; charset=utf-8", []byte(successHTML))
}
