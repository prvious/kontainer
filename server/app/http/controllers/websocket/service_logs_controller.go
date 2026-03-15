package websocket

import (
	"context"
	goHttp "net/http"

	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/facades"
	ws "github.com/prvious/kontainer/server/app/services/websocket"
)

// ServiceLogsController bridges Goravel's http.Context to the raw WS handler.
// It validates JWT from the query param and extracts the route param before delegating.
type ServiceLogsController struct {
	handler *ws.ServiceLogsHandler
}

func NewServiceLogsController(handler *ws.ServiceLogsHandler) *ServiceLogsController {
	return &ServiceLogsController{handler: handler}
}

func (c *ServiceLogsController) Handle(ctx http.Context) http.Response {
	token := ctx.Request().Query("token", "")
	if token == "" {
		return ctx.Response().Json(goHttp.StatusUnauthorized, http.Json{"error": "missing token"})
	}
	if _, err := facades.Auth(ctx).Parse(token); err != nil {
		return ctx.Response().Json(goHttp.StatusUnauthorized, http.Json{"error": "invalid token"})
	}

	serviceID := ctx.Request().Route("serviceID")
	w := ctx.Response().Writer()
	r := ctx.Request().Origin().WithContext(
		context.WithValue(ctx.Request().Origin().Context(), ws.ServiceIDKey, serviceID),
	)
	c.handler.ServeHTTP(w, r)
	return nil
}
