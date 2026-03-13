package auth

import (
	nethttp "net/http"
	"time"

	contractshttp "github.com/goravel/framework/contracts/http"
	"github.com/gorilla/websocket"

	"github.com/prvious/kontainer/server/app/facades"
	authservice "github.com/prvious/kontainer/server/app/services/auth"
)

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *nethttp.Request) bool { return true },
}

type ListenController struct {
	bridge *authservice.AuthBridge
}

func NewListenController(bridge *authservice.AuthBridge) *ListenController {
	return &ListenController{bridge: bridge}
}

func (c *ListenController) Handle(ctx contractshttp.Context) contractshttp.Response {
	w := ctx.Response().Writer()
	r := ctx.Request().Origin()

	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		facades.Log().Errorf("ws upgrade error: %v", err)
		return nil
	}
	defer conn.Close()

	sessionID, err := c.bridge.CreateSession()
	if err != nil {
		conn.WriteJSON(map[string]string{"error": "failed to create session"}) //nolint:errcheck
		return nil
	}
	defer c.bridge.Cleanup(sessionID)

	if err := conn.WriteJSON(map[string]string{"sessionId": sessionID}); err != nil {
		return nil
	}

	done := make(chan struct{})
	go func() {
		for {
			if _, _, err := conn.ReadMessage(); err != nil {
				close(done)
				return
			}
		}
	}()

	token, err := c.bridge.WaitForToken(sessionID, 10*time.Minute, done)
	if err != nil {
		conn.WriteJSON(map[string]string{"error": err.Error()}) //nolint:errcheck
		return nil
	}

	conn.WriteJSON(map[string]string{"token": token}) //nolint:errcheck
	return nil
}
