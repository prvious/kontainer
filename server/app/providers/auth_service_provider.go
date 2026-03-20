package providers

import (
	"github.com/goravel/framework/contracts/foundation"
	"github.com/prvious/kontainer/server/app/facades"

	"github.com/prvious/kontainer/server/app/models"
	authservice "github.com/prvious/kontainer/server/app/services/auth"
	authproviders "github.com/prvious/kontainer/server/app/services/auth/providers"
)

type AuthServiceProvider struct{}

func (r *AuthServiceProvider) Register(app foundation.Application) {
	app.Singleton("auth.session_store", func(app foundation.Application) (any, error) {
		return authservice.NewSessionStore(), nil
	})

	app.Singleton("auth.bridge", func(app foundation.Application) (any, error) {
		return authservice.NewAuthBridge(), nil
	})

	app.Singleton("auth.provider_registry", func(app foundation.Application) (any, error) {
		return authproviders.NewProviderRegistry(), nil
	})
}

func (r *AuthServiceProvider) Boot(app foundation.Application) {
	r.loadJWTSecretFromDB()
	r.registerOAuthProviders(app)
}

func (r *AuthServiceProvider) loadJWTSecretFromDB() {
	var appConfig models.AppConfig
	err := facades.Orm().Query().Where("key", "jwt_secret").First(&appConfig)
	if err != nil || appConfig.Value == "" {
		facades.Log().Warning("JWT secret not found in app_configs — auth disabled until setup completes")
		return
	}

	facades.Config().Add("auth.guards.user.secret", appConfig.Value)
	facades.Log().Info("JWT secret loaded from app_configs into user guard")
}

func (r *AuthServiceProvider) registerOAuthProviders(app foundation.Application) {
	registry, err := app.Make("auth.provider_registry")
	if err != nil {
		facades.Log().Errorf("Failed to resolve provider registry: %v", err)
		return
	}
	providerRegistry, ok := registry.(*authproviders.ProviderRegistry)
	if !ok {
		facades.Log().Errorf("auth.provider_registry is not *ProviderRegistry")
		return
	}

	configs := map[string]string{}
	for _, key := range []string{"github_client_id", "github_client_secret", "github_org"} {
		var ac models.AppConfig
		if err := facades.Orm().Query().Where("key", key).First(&ac); err != nil {
			facades.Log().Warningf("Failed to load OAuth config key %q: %v", key, err)
			continue
		}
		configs[key] = ac.Value
	}

	clientID := configs["github_client_id"]
	clientSecret := configs["github_client_secret"]
	if clientID == "" || clientSecret == "" {
		facades.Log().Warning("GitHub OAuth credentials not found — OAuth disabled until setup completes")
		return
	}

	agentURL := facades.Config().GetString("cluster.agent_url", "http://localhost:3000")
	org := configs["github_org"]

	providerRegistry.Register(authproviders.NewGitHubProvider(
		clientID,
		clientSecret,
		agentURL+"/auth/github/callback",
		org,
	))

	facades.Log().Info("GitHub OAuth provider registered")
}
