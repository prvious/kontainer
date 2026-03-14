package websocket

import (
	"context"
	goHttp "net/http"

	"github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/facades"
	ws "github.com/prvious/kontainer/server/app/services/websocket"
)

// TaskLogsController bridges Goravel's http.Context to the raw WS handler.
// It validates JWT from the query param and extracts the route param before delegating.
type TaskLogsController struct {
	handler *ws.TaskLogsHandler
}

func NewTaskLogsController(handler *ws.TaskLogsHandler) *TaskLogsController {
	return &TaskLogsController{handler: handler}
}

func (c *TaskLogsController) Handle(ctx http.Context) http.Response {
	token := ctx.Request().Query("token", "")
	if token == "" {
		return ctx.Response().Json(goHttp.StatusUnauthorized, http.Json{"error": "missing token"})
	}
	if _, err := facades.Auth(ctx).Parse(token); err != nil {
		return ctx.Response().Json(goHttp.StatusUnauthorized, http.Json{"error": "invalid token"})
	}

	taskID := ctx.Request().Route("taskID")
	w := ctx.Response().Writer().(goHttp.ResponseWriter)
	r := ctx.Request().Origin().WithContext(
		context.WithValue(ctx.Request().Origin().Context(), ws.TaskIDKey, taskID),
	)
	c.handler.ServeHTTP(w, r)
	return nil
}
