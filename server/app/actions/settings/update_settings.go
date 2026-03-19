package settings

import (
	"fmt"

	"github.com/goravel/framework/contracts/database/orm"
	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/models"
)

type UpdateSettingsAction struct{}

func NewUpdateSettingsAction() *UpdateSettingsAction {
	return &UpdateSettingsAction{}
}

func (a *UpdateSettingsAction) Execute(settings map[string]string) error {
	return facades.Orm().Transaction(func(tx orm.Query) error {
		for key, value := range settings {
			var setting models.Setting
			if err := tx.UpdateOrCreate(&setting,
				models.Setting{Key: key},
				models.Setting{Value: value},
			); err != nil {
				return fmt.Errorf("failed to save %s: %w", key, err)
			}
		}
		return nil
	})
}
