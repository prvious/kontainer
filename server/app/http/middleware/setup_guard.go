package middleware

import (
	nethttp "net/http"
	"strings"
	"sync/atomic"

	contractshttp "github.com/goravel/framework/contracts/http"
	"github.com/goravel/framework/facades"

	"github.com/prvious/kontainer/server/app/models"
)

var setupComplete atomic.Bool

func MarkSetupComplete() {
	setupComplete.Store(true)
}

func SetupGuard() contractshttp.Middleware {
	return func(ctx contractshttp.Context) {
		path := ctx.Request().Path()

		if path == "/health" || path == "/ping" || strings.HasPrefix(path, "/setup") {
			ctx.Request().Next()
			return
		}

		if setupComplete.Load() {
			ctx.Request().Next()
			return
		}

		if isSetupComplete() {
			setupComplete.Store(true)
			ctx.Request().Next()
			return
		}

		ctx.Response().Json(nethttp.StatusServiceUnavailable, map[string]any{
			"status": "setup_required",
		})
		ctx.Request().Abort()
	}
}

func isSetupComplete() bool {
	var config models.AppConfig
	err := facades.Orm().Query().Where("key", "setup_complete").First(&config)
	if err != nil {
		return false
	}
	return config.Value == "true"
}
