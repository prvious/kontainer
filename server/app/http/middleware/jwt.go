package middleware

import (
	nethttp "net/http"

	contractshttp "github.com/goravel/framework/contracts/http"
	"github.com/goravel/framework/facades"
)

// JWT validates the Authorization Bearer token using Goravel's auth facade.
func JWT() contractshttp.Middleware {
	return func(ctx contractshttp.Context) {
		token := ctx.Request().Header("Authorization")
		if token == "" {
			ctx.Response().Json(nethttp.StatusUnauthorized, map[string]any{
				"error": "authorization required",
			})
			ctx.Request().Abort()
			return
		}

		_, err := facades.Auth(ctx).Parse(token)
		if err != nil {
			ctx.Response().Json(nethttp.StatusUnauthorized, map[string]any{
				"error": "invalid or expired token",
			})
			ctx.Request().Abort()
			return
		}

		ctx.Request().Next()
	}
}
