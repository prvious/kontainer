package requests

import "github.com/goravel/framework/contracts/http"

type UpdateSettingsRequest struct {
	Settings map[string]string `form:"settings" json:"settings"`
}

func (r *UpdateSettingsRequest) Authorize(ctx http.Context) error {
	return nil
}

func (r *UpdateSettingsRequest) Rules(ctx http.Context) map[string]string {
	return map[string]string{
		"settings": "required",
	}
}
