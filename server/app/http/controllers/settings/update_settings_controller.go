package settings

import (
	"github.com/goravel/framework/contracts/http"
	settingsActions "github.com/prvious/kontainer/server/app/actions/settings"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/http/requests"
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
	var req requests.UpdateSettingsRequest
	errs, err := ctx.Request().ValidateRequest(&req)
	if err != nil {
		return ctx.Response().Json(400, map[string]string{"error": err.Error()})
	}
	if errs != nil {
		return ctx.Response().Json(422, map[string]any{"errors": errs.All()})
	}

	if err := c.updateAction.Execute(req.Settings); err != nil {
		facades.Log().Errorf("update settings error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to update settings"})
	}

	settings, err := c.getAction.Execute()
	if err != nil {
		return ctx.Response().Json(500, map[string]string{"error": "failed to read updated settings"})
	}
	return ctx.Response().Json(200, map[string]any{"settings": settings})
}
