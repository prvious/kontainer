package settings

import (
	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/models"
)

type GetSettingsAction struct{}

func NewGetSettingsAction() *GetSettingsAction {
	return &GetSettingsAction{}
}

func (a *GetSettingsAction) Execute() (map[string]string, error) {
	var settings []models.Setting
	if err := facades.Orm().Query().Get(&settings); err != nil {
		return nil, err
	}

	result := make(map[string]string, len(settings))
	for _, s := range settings {
		result[s.Key] = s.Value
	}
	return result, nil
}
