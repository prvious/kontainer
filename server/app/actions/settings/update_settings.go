package settings

import (
	"fmt"
	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/models"
)

type UpdateSettingsAction struct{}

func NewUpdateSettingsAction() *UpdateSettingsAction {
	return &UpdateSettingsAction{}
}

func (a *UpdateSettingsAction) Execute(settings map[string]string) error {
	var errs []error
	for key, value := range settings {
		var setting models.Setting
		if err := facades.Orm().Query().UpdateOrCreate(&setting,
			models.Setting{Key: key},
			models.Setting{Value: value},
		); err != nil {
			errs = append(errs, fmt.Errorf("failed to save %s: %w", key, err))
		}
	}
	if len(errs) > 0 {
		return fmt.Errorf("settings update errors: %v", errs)
	}
	return nil
}
