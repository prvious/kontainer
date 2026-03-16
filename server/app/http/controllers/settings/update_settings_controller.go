package settings

import (
	"github.com/goravel/framework/contracts/http"
	settingsActions "github.com/prvious/kontainer/server/app/actions/settings"
)

type UpdateSettingsController struct {
	getAction    *settingsActions.GetSettingsAction
	updateAction *settingsActions.UpdateSettingsAction
}

func NewUpdateSettingsController(
	getAction *settingsActions.GetSettingsAction,
	updateAction *settingsActions.UpdateSettingsAction,
) *UpdateSettingsController {
	return &UpdateSettingsController{getAction: getAction, updateAction: updateAction}
}

func (c *UpdateSettingsController) Handle(ctx http.Context) http.Response {
	var req struct {
		Settings map[string]string `json:"settings"`
	}
	if err := ctx.Request().Bind(&req); err != nil {
		return ctx.Response().Json(400, map[string]string{"error": "invalid request body"})
	}
	if len(req.Settings) == 0 {
		return ctx.Response().Json(400, map[string]string{"error": "settings map is required and must not be empty"})
	}

	if err := c.updateAction.Execute(req.Settings); err != nil {
		return ctx.Response().Json(500, map[string]string{"error": "failed to update settings"})
	}

	settings, err := c.getAction.Execute()
	if err != nil {
		return ctx.Response().Json(500, map[string]string{"error": "failed to read updated settings"})
	}
	return ctx.Response().Json(200, map[string]any{"settings": settings})
}
