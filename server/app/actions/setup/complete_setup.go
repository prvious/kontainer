package setup

import (
	"fmt"

	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/http/middleware"
	"github.com/prvious/kontainer/server/app/models"
	authproviders "github.com/prvious/kontainer/server/app/services/auth/providers"
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
	if err := facades.Orm().Query().Where("key", "jwt_secret").First(&jwtConfig); err != nil {
		return fmt.Errorf("failed to load JWT secret: %w", err)
	}
	if jwtConfig.Value == "" {
		return fmt.Errorf("JWT secret is empty — cannot activate auth")
	}
	facades.Config().Add("auth.guards.user.secret", jwtConfig.Value)
	facades.Log().Info("JWT secret loaded into user guard — auth system now active")

	// Register OAuth providers now that credentials exist in the DB
	registry, err := facades.App().Make("auth.provider_registry")
	if err != nil {
		return fmt.Errorf("failed to resolve provider registry: %w", err)
	}
	providerRegistry, ok := registry.(*authproviders.ProviderRegistry)
	if !ok {
		return fmt.Errorf("auth.provider_registry is not *ProviderRegistry")
	}
	if err := authproviders.RegisterGitHubFromDB(providerRegistry); err != nil {
		return fmt.Errorf("failed to register OAuth providers: %w", err)
	}
	facades.Log().Info("GitHub OAuth provider registered after setup completion")

	return nil
}
