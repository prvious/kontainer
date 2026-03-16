package settings

import (
	"github.com/goravel/framework/contracts/http"
	settingsActions "github.com/prvious/kontainer/server/app/actions/settings"
)

type GetSettingsController struct {
	action *settingsActions.GetSettingsAction
}

func NewGetSettingsController(action *settingsActions.GetSettingsAction) *GetSettingsController {
	return &GetSettingsController{action: action}
}

func (c *GetSettingsController) Handle(ctx http.Context) http.Response {
	settings, err := c.action.Execute()
	if err != nil {
		return ctx.Response().Json(500, map[string]string{"error": "failed to get settings"})
	}
	return ctx.Response().Json(200, map[string]any{"settings": settings})
}
