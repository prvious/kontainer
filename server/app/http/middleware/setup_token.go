package middleware

import (
	"crypto/subtle"

	contractshttp "github.com/goravel/framework/contracts/http"
)

const SetupTokenHeader = "X-Kontainer-Setup-Token"

func SetupToken(expectedToken string) contractshttp.Middleware {
	return func(ctx contractshttp.Context) {
		provided := ctx.Request().Header(SetupTokenHeader)

		if provided == "" {
			ctx.Response().Json(contractshttp.StatusUnauthorized, map[string]any{
				"error": "setup token required",
			})
			ctx.Request().Abort(contractshttp.StatusUnauthorized)
			return
		}

		if subtle.ConstantTimeCompare([]byte(provided), []byte(expectedToken)) != 1 {
			ctx.Response().Json(contractshttp.StatusUnauthorized, map[string]any{
				"error": "invalid setup token",
			})
			ctx.Request().Abort(contractshttp.StatusUnauthorized)
			return
		}

		ctx.Request().Next()
	}
}
