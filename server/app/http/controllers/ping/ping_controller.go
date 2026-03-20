package ping

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/models"
)

type PingController struct{}

func NewPingController() *PingController {
	return &PingController{}
}

func (c *PingController) Handle(ctx http.Context) http.Response {
	status := "ok"
	if !isSetupDone() {
		status = "setup_required"
	}

	return ctx.Response().Json(200, map[string]string{
		"app":    "dev.kontainer.app",
		"status": status,
	})
}

func isSetupDone() bool {
	var config models.AppConfig
	if err := facades.Orm().Query().Where("key", "setup_complete").First(&config); err != nil {
		return false
	}
	return config.Value == "true"
}
