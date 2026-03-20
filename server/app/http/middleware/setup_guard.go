package middleware

import (
	"strings"
	"sync/atomic"

	contractshttp "github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/facades"

	"github.com/prvious/kontainer/server/app/models"
)

var setupComplete atomic.Bool

func MarkSetupComplete() {
	setupComplete.Store(true)
}

func SetupGuard() contractshttp.Middleware {
	if facades.Config().GetString("cluster.role") != "leader" {
		return func(ctx contractshttp.Context) {
			ctx.Request().Next()
		}
	}

	return func(ctx contractshttp.Context) {
		path := ctx.Request().Path()

		if path == "/health" || path == "/ping" || strings.HasPrefix(path, "/setup") || strings.HasPrefix(path, "/auth") {
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

		ctx.Response().Json(contractshttp.StatusServiceUnavailable, map[string]any{
			"status": "setup_required",
		})
		ctx.Request().Abort(contractshttp.StatusServiceUnavailable)
	}
}

func isSetupComplete() bool {
	var config models.AppConfig
	err := facades.Orm().Query().Where("key", "setup_complete").First(&config)
	if err != nil {
		facades.Log().Errorf("Failed to check setup status from DB: %v", err)
		return false
	}
	return config.Value == "true"
}
