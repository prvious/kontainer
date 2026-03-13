package setup

import (
	"github.com/goravel/framework/facades"

	"github.com/prvious/kontainer/server/app/models"
)

type CheckStatusAction struct{}

func NewCheckStatusAction() *CheckStatusAction {
	return &CheckStatusAction{}
}

func (a *CheckStatusAction) Execute() (string, error) {
	if a.hasKey("setup_complete") {
		return "complete", nil
	}
	if a.hasKey("github_app_id") {
		return "github_connected", nil
	}
	return "pending", nil
}

func (a *CheckStatusAction) hasKey(key string) bool {
	var config models.AppConfig
	err := facades.Orm().Query().Where("key", key).First(&config)
	return err == nil && config.Value != ""
}
