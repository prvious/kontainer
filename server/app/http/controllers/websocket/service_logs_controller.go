package websocket

import (
	"context"

	"github.com/goravel/framework/contracts/http"
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
	if resp := validateWSToken(ctx); resp != nil {
		return resp
	}

	serviceID := ctx.Request().Route("serviceID")
	w := ctx.Response().Writer()
	r := ctx.Request().Origin().WithContext(
		context.WithValue(ctx.Request().Origin().Context(), ws.ServiceIDKey, serviceID),
	)
	c.handler.ServeHTTP(w, r)
	return nil
}
