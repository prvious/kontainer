package ping

import (
	"github.com/goravel/framework/contracts/http"
)

type PingController struct{}

func NewPingController() *PingController {
	return &PingController{}
}

func (c *PingController) Handle(ctx http.Context) http.Response {
	return ctx.Response().Json(200, map[string]string{
		"app":    "dev.kontainer.app",
		"status": "ok",
	})
}
