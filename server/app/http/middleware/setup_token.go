package middleware

import (
	nethttp "net/http"

	contractshttp "github.com/goravel/framework/contracts/http"
)

const SetupTokenHeader = "X-Kontainer-Setup-Token"

func SetupToken(expectedToken string) contractshttp.Middleware {
	return func(ctx contractshttp.Context) {
		provided := ctx.Request().Header(SetupTokenHeader)

		if provided == "" {
			ctx.Response().Json(nethttp.StatusUnauthorized, map[string]any{
				"error": "setup token required",
			})
			ctx.Request().Abort()
			return
		}

		if provided != expectedToken {
			ctx.Response().Json(nethttp.StatusUnauthorized, map[string]any{
				"error": "invalid setup token",
			})
			ctx.Request().Abort()
			return
		}

		ctx.Request().Next()
	}
}
