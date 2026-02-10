package providers

import (
	"context"
	"fmt"
	"sort"

	"golang.org/x/oauth2"
)

// AccessDeniedError is returned when a user authenticates successfully but is not
// authorized (e.g. not a member of an allowed organization).
type AccessDeniedError struct {
	Message string
}

func (e *AccessDeniedError) Error() string { return e.Message }

// ProviderUser is a normalized user representation returned by any OAuth provider.
type ProviderUser struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	Email     string `json:"email"`
	AvatarURL string `json:"avatarUrl"`
	Provider  string `json:"provider"`
}

// Provider is the interface that each OAuth provider must implement.
type Provider interface {
	Name() string
	OAuthConfig() *oauth2.Config
	FetchUser(ctx context.Context, token *oauth2.Token) (*ProviderUser, error)
}

// ProviderRegistry holds registered OAuth providers keyed by name.
type ProviderRegistry struct {
	providers map[string]Provider
}

func NewProviderRegistry() *ProviderRegistry {
	return &ProviderRegistry{providers: make(map[string]Provider)}
}

func (r *ProviderRegistry) Register(p Provider) {
	r.providers[p.Name()] = p
}

func (r *ProviderRegistry) Names() []string {
	names := make([]string, 0, len(r.providers))
	for name := range r.providers {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

func (r *ProviderRegistry) Get(name string) (Provider, error) {
	p, ok := r.providers[name]
	if !ok {
		return nil, fmt.Errorf("unknown provider: %s", name)
	}
	return p, nil
}
