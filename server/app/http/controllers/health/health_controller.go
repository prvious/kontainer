package health

import (
	"github.com/goravel/framework/contracts/http"
)

type HealthController struct{}

func NewHealthController() *HealthController {
	return &HealthController{}
}

func (c *HealthController) Handle(ctx http.Context) http.Response {
	return ctx.Response().Json(200, map[string]string{"status": "ok"})
}
