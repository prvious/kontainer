package setup

import (
	"fmt"
	nethttp "net/http"
	"strconv"

	"github.com/goravel/framework/contracts/database/orm"
	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/helpers"
	"github.com/prvious/kontainer/server/app/models"
)

type ExchangeCodeAction struct{}

func NewExchangeCodeAction() *ExchangeCodeAction {
	return &ExchangeCodeAction{}
}

type GitHubAppCredentials struct {
	AppID         int
	Slug          string
	ClientID      string
	ClientSecret  string
	PEM           string
	WebhookSecret string
	OwnerLogin    string
}

func (a *ExchangeCodeAction) Execute(code string) (*GitHubAppCredentials, error) {
	conversionURL := fmt.Sprintf("https://api.github.com/app-manifests/%s/conversions", code)
	resp, err := facades.Http().
		WithHeader("Accept", "application/json").
		Post(conversionURL, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to contact GitHub API: %w", err)
	}

	if resp.Status() != nethttp.StatusCreated {
		body, _ := resp.Body()
		return nil, fmt.Errorf("GitHub API returned status %d: %s", resp.Status(), body)
	}

	var ghResp struct {
		ID            int    `json:"id"`
		Slug          string `json:"slug"`
		ClientID      string `json:"client_id"`
		ClientSecret  string `json:"client_secret"`
		PEM           string `json:"pem"`
		WebhookSecret string `json:"webhook_secret"`
		Owner         struct {
			Login string `json:"login"`
		} `json:"owner"`
	}
	if err := resp.Bind(&ghResp); err != nil {
		return nil, fmt.Errorf("failed to parse GitHub API response: %w", err)
	}

	jwtSecret := helpers.RandomHex(32)

	configs := map[string]string{
		"github_app_id":         strconv.Itoa(ghResp.ID),
		"github_client_id":      ghResp.ClientID,
		"github_client_secret":  ghResp.ClientSecret,
		"github_private_key":    ghResp.PEM,
		"github_webhook_secret": ghResp.WebhookSecret,
		"github_org":            ghResp.Owner.Login,
		"jwt_secret":            jwtSecret,
	}

	if err := facades.Orm().Transaction(func(tx orm.Query) error {
		for key, value := range configs {
			var appConfig models.AppConfig
			if err := tx.UpdateOrCreate(&appConfig,
				models.AppConfig{Key: key},
				models.AppConfig{Value: value},
			); err != nil {
				return fmt.Errorf("failed to store %s: %w", key, err)
			}
		}
		return nil
	}); err != nil {
		return nil, err
	}

	return &GitHubAppCredentials{
		AppID:         ghResp.ID,
		Slug:          ghResp.Slug,
		ClientID:      ghResp.ClientID,
		ClientSecret:  ghResp.ClientSecret,
		PEM:           ghResp.PEM,
		WebhookSecret: ghResp.WebhookSecret,
		OwnerLogin:    ghResp.Owner.Login,
	}, nil
}

