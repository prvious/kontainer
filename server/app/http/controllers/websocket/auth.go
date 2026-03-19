package websocket

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/facades"
)

// validateWSToken validates the JWT token from the ?token= query parameter.
// WebSocket endpoints cannot use the standard JWT middleware because the browser
// WebSocket API does not support custom headers. Returns an error response if
// validation fails, or nil if the token is valid.
func validateWSToken(ctx http.Context) http.Response {
	token := ctx.Request().Query("token", "")
	if token == "" {
		return ctx.Response().Json(401, map[string]any{"error": "missing token"})
	}
	if _, err := facades.Auth(ctx).Parse(token); err != nil {
		return ctx.Response().Json(401, map[string]any{"error": "invalid token"})
	}
	return nil
}
