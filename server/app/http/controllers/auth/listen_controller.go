package auth

import (
	"log"
	nethttp "net/http"
	"time"

	contractshttp "github.com/goravel/framework/contracts/http"
	"github.com/gorilla/websocket"

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
		log.Printf("ws upgrade error: %v", err)
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

	token, err := c.bridge.WaitForToken(sessionID, 10*time.Minute)
	if err != nil {
		conn.WriteJSON(map[string]string{"error": err.Error()}) //nolint:errcheck
		return nil
	}

	conn.WriteJSON(map[string]string{"token": token}) //nolint:errcheck
	return nil
}
