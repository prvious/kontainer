package providers

import (
	"fmt"

	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/models"
)

// RegisterGitHubFromDB loads GitHub OAuth credentials from app_configs
// and registers the provider in the given registry. Returns an error
// if credentials are missing; returns nil silently if the registry
// already has a "github" provider.
func RegisterGitHubFromDB(registry *ProviderRegistry) error {
	configs := map[string]string{}
	for _, key := range []string{"github_client_id", "github_client_secret", "github_org"} {
		var ac models.AppConfig
		if err := facades.Orm().Query().Where("key", key).First(&ac); err != nil {
			continue
		}
		configs[key] = ac.Value
	}

	clientID := configs["github_client_id"]
	clientSecret := configs["github_client_secret"]
	if clientID == "" || clientSecret == "" {
		return fmt.Errorf("GitHub OAuth credentials not found in app_configs")
	}

	agentURL := facades.Config().GetString("cluster.agent_url", "http://localhost:3000")
	org := configs["github_org"]

	registry.Register(NewGitHubProvider(
		clientID,
		clientSecret,
		agentURL+"/auth/github/callback",
		org,
	))

	return nil
}
