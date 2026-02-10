package providers

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"
	"strings"

	"golang.org/x/oauth2"
	oauthGithub "golang.org/x/oauth2/endpoints"
)

type GitHubProvider struct {
	config       *oauth2.Config
	allowedOrgs  []string
	allowedUsers []string
}

func NewGitHubProvider(clientID, clientSecret, redirectURL string, allowedOrgs, allowedUsers []string) *GitHubProvider {
	scopes := []string{"read:user", "user:email"}
	if len(allowedOrgs) > 0 {
		scopes = append(scopes, "read:org")
	}

	return &GitHubProvider{
		config: &oauth2.Config{
			ClientID:     clientID,
			ClientSecret: clientSecret,
			RedirectURL:  redirectURL,
			Scopes:       scopes,
			Endpoint:     oauthGithub.GitHub,
		},
		allowedOrgs:  allowedOrgs,
		allowedUsers: allowedUsers,
	}
}

func (g *GitHubProvider) Name() string {
	return "github"
}

func (g *GitHubProvider) OAuthConfig() *oauth2.Config {
	return g.config
}

func (g *GitHubProvider) FetchUser(ctx context.Context, token *oauth2.Token) (*ProviderUser, error) {
	client := g.config.Client(ctx, token)

	user, login, err := g.fetchProfile(client)
	if err != nil {
		return nil, err
	}

	if user.Email == "" {
		email, err := g.fetchPrimaryEmail(client)
		if err != nil {
			return nil, err
		}
		user.Email = email
	}

	if err := g.checkAccess(client, login); err != nil {
		return nil, err
	}

	return user, nil
}

type githubProfile struct {
	ID        int    `json:"id"`
	Login     string `json:"login"`
	Name      string `json:"name"`
	Email     string `json:"email"`
	AvatarURL string `json:"avatar_url"`
}

func (g *GitHubProvider) fetchProfile(client *http.Client) (*ProviderUser, string, error) {
	resp, err := client.Get("https://api.github.com/user")
	if err != nil {
		return nil, "", fmt.Errorf("github: fetch profile: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, "", fmt.Errorf("github: fetch profile: status %d", resp.StatusCode)
	}

	var profile githubProfile
	if err := json.NewDecoder(resp.Body).Decode(&profile); err != nil {
		return nil, "", fmt.Errorf("github: decode profile: %w", err)
	}

	name := profile.Name
	if name == "" {
		name = profile.Login
	}

	return &ProviderUser{
		ID:        strconv.Itoa(profile.ID),
		Name:      name,
		Email:     profile.Email,
		AvatarURL: profile.AvatarURL,
		Provider:  "github",
	}, profile.Login, nil
}

type githubEmail struct {
	Email    string `json:"email"`
	Primary  bool   `json:"primary"`
	Verified bool   `json:"verified"`
}

func (g *GitHubProvider) fetchPrimaryEmail(client *http.Client) (string, error) {
	resp, err := client.Get("https://api.github.com/user/emails")
	if err != nil {
		return "", fmt.Errorf("github: fetch emails: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("github: fetch emails: status %d", resp.StatusCode)
	}

	var emails []githubEmail
	if err := json.NewDecoder(resp.Body).Decode(&emails); err != nil {
		return "", fmt.Errorf("github: decode emails: %w", err)
	}

	for _, e := range emails {
		if e.Primary && e.Verified {
			return e.Email, nil
		}
	}

	return "", fmt.Errorf("github: no primary verified email found")
}

type githubOrg struct {
	Login string `json:"login"`
}

func (g *GitHubProvider) checkOrgMembership(client *http.Client) error {
	page := 1
	for {
		url := fmt.Sprintf("https://api.github.com/user/orgs?per_page=100&page=%d", page)
		resp, err := client.Get(url)
		if err != nil {
			return fmt.Errorf("github: fetch orgs: %w", err)
		}
		defer resp.Body.Close()

		if resp.StatusCode != http.StatusOK {
			return fmt.Errorf("github: fetch orgs: status %d", resp.StatusCode)
		}

		var orgs []githubOrg
		if err := json.NewDecoder(resp.Body).Decode(&orgs); err != nil {
			return fmt.Errorf("github: decode orgs: %w", err)
		}

		for _, org := range orgs {
			for _, allowed := range g.allowedOrgs {
				if strings.EqualFold(org.Login, allowed) {
					return nil
				}
			}
		}

		if len(orgs) < 100 {
			break
		}
		page++
	}

	return &AccessDeniedError{
		Message: "user is not a member of any allowed organization",
	}
}

func (g *GitHubProvider) checkAccess(client *http.Client, login string) error {
	hasUsers := len(g.allowedUsers) > 0
	hasOrgs := len(g.allowedOrgs) > 0

	if !hasUsers && !hasOrgs {
		return nil
	}

	if hasUsers && !g.matchesUser(login) {
		return &AccessDeniedError{
			Message: fmt.Sprintf("user %q is not in the allowed users list", login),
		}
	}

	if hasOrgs {
		if err := g.checkOrgMembership(client); err != nil {
			return err
		}
	}

	return nil
}

func (g *GitHubProvider) matchesUser(login string) bool {
	for _, allowed := range g.allowedUsers {
		if strings.EqualFold(login, allowed) {
			return true
		}
	}
	return false
}
