package websocket

import (
	"context"

	"github.com/goravel/framework/contracts/http"
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
	if resp := validateWSToken(ctx); resp != nil {
		return resp
	}

	taskID := ctx.Request().Route("taskID")
	w := ctx.Response().Writer()
	r := ctx.Request().Origin().WithContext(
		context.WithValue(ctx.Request().Origin().Context(), ws.TaskIDKey, taskID),
	)
	c.handler.ServeHTTP(w, r)
	return nil
}
