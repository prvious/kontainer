package setup

import (
	"fmt"

	"github.com/prvious/kontainer/server/app/facades"

	"github.com/prvious/kontainer/server/app/http/middleware"
	"github.com/prvious/kontainer/server/app/models"
)

type CompleteSetupAction struct{}

func NewCompleteSetupAction() *CompleteSetupAction {
	return &CompleteSetupAction{}
}

func (a *CompleteSetupAction) Execute() error {
	var appID models.AppConfig
	if err := facades.Orm().Query().Where("key", "github_app_id").First(&appID); err != nil || appID.Value == "" {
		return fmt.Errorf("GitHub App not connected — complete the GitHub setup first")
	}

	var complete models.AppConfig
	if err := facades.Orm().Query().UpdateOrCreate(&complete,
		models.AppConfig{Key: "setup_complete"},
		models.AppConfig{Value: "true"},
	); err != nil {
		return fmt.Errorf("failed to complete setup: %w", err)
	}

	middleware.MarkSetupComplete()

	var jwtConfig models.AppConfig
	if err := facades.Orm().Query().Where("key", "jwt_secret").First(&jwtConfig); err == nil && jwtConfig.Value != "" {
		facades.Config().Add("auth.guards.user.secret", jwtConfig.Value)
		facades.Log().Info("JWT secret loaded into user guard — auth system now active")
	}

	return nil
}
