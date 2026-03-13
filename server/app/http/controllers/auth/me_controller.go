package auth

import (
	nethttp "net/http"

	contractshttp "github.com/goravel/framework/contracts/http"
	"github.com/prvious/kontainer/server/app/facades"

	"github.com/prvious/kontainer/server/app/models"
)

type MeController struct{}

func NewMeController() *MeController {
	return &MeController{}
}

func (c *MeController) Handle(ctx contractshttp.Context) contractshttp.Response {
	var user models.User
	if err := facades.Auth(ctx).User(&user); err != nil {
		return ctx.Response().Json(nethttp.StatusUnauthorized, map[string]any{"error": "unauthorized"})
	}

	return ctx.Response().Json(nethttp.StatusOK, map[string]any{
		"user": map[string]any{
			"id":        user.ID,
			"name":      user.Name,
			"email":     user.Email,
			"avatarUrl": user.AvatarURL,
			"provider":  user.Provider,
		},
		"status": "ok",
	})
}
