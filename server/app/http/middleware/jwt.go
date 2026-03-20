package middleware

import (
	contractshttp "github.com/goravel/framework/contracts/http"

	"github.com/prvious/kontainer/server/app/facades"
)

// JWT validates the Authorization Bearer token using Goravel's auth facade.
func JWT() contractshttp.Middleware {
	return func(ctx contractshttp.Context) {
		token := ctx.Request().Header("Authorization")
		if token == "" {
			ctx.Response().Json(contractshttp.StatusUnauthorized, map[string]any{
				"error": "authorization required",
			})
			ctx.Request().Abort(contractshttp.StatusUnauthorized)
			return
		}

		_, err := facades.Auth(ctx).Parse(token)
		if err != nil {
			ctx.Response().Json(contractshttp.StatusUnauthorized, map[string]any{
				"error": "invalid or expired token",
			})
			ctx.Request().Abort(contractshttp.StatusUnauthorized)
			return
		}

		ctx.Request().Next()
	}
}
