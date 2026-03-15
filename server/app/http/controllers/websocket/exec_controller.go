package websocket

import (
	"context"

	"github.com/goravel/framework/contracts/http"
	ws "github.com/prvious/kontainer/server/app/services/websocket"
)

// ExecController bridges Goravel's http.Context to the raw WS handler.
// It validates JWT from the query param and extracts the route param before delegating.
type ExecController struct {
	handler *ws.ExecHandler
}

func NewExecController(handler *ws.ExecHandler) *ExecController {
	return &ExecController{handler: handler}
}

func (c *ExecController) Handle(ctx http.Context) http.Response {
	if resp := validateWSToken(ctx); resp != nil {
		return resp
	}

	containerID := ctx.Request().Route("containerID")
	w := ctx.Response().Writer()
	r := ctx.Request().Origin().WithContext(
		context.WithValue(ctx.Request().Origin().Context(), ws.ContainerIDKey, containerID),
	)
	c.handler.ServeHTTP(w, r)
	return nil
}
