package setup

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"

	"github.com/goravel/framework/facades"

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
	resp, err := http.Post(conversionURL, "application/json", nil)
	if err != nil {
		return nil, fmt.Errorf("failed to contact GitHub API: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read GitHub API response: %w", err)
	}

	if resp.StatusCode != http.StatusCreated {
		return nil, fmt.Errorf("GitHub API returned status %d", resp.StatusCode)
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
	if err := json.Unmarshal(body, &ghResp); err != nil {
		return nil, fmt.Errorf("failed to parse GitHub API response: %w", err)
	}

	jwtSecret := generateRandomHex(32)

	configs := map[string]string{
		"github_app_id":         strconv.Itoa(ghResp.ID),
		"github_client_id":      ghResp.ClientID,
		"github_client_secret":  ghResp.ClientSecret,
		"github_private_key":    ghResp.PEM,
		"github_webhook_secret": ghResp.WebhookSecret,
		"github_org":            ghResp.Owner.Login,
		"jwt_secret":            jwtSecret,
	}

	for key, value := range configs {
		var appConfig models.AppConfig
		if err := facades.Orm().Query().UpdateOrCreate(&appConfig,
			models.AppConfig{Key: key},
			models.AppConfig{Value: value},
		); err != nil {
			return nil, fmt.Errorf("failed to store %s: %w", key, err)
		}
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

func generateRandomHex(nBytes int) string {
	b := make([]byte, nBytes)
	if _, err := rand.Read(b); err != nil {
		panic("crypto/rand failed: " + err.Error())
	}
	return hex.EncodeToString(b)
}
