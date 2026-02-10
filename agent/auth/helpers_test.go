package auth

import (
	"context"
	"net/http"

	"github.com/prvious/kontainer/agent/auth/providers"
	"github.com/go-chi/chi/v5"
	"golang.org/x/oauth2"
)

var testUser = &providers.ProviderUser{
	ID:        "12345",
	Name:      "Jane Doe",
	Email:     "jane@example.com",
	AvatarURL: "https://example.com/avatar.png",
	Provider:  "github",
}

const testSecret = "super-secret-key-for-testing-only"

func withChiURLParam(r *http.Request, key, value string) *http.Request {
	rctx := chi.NewRouteContext()
	rctx.URLParams.Add(key, value)
	return r.WithContext(context.WithValue(r.Context(), chi.RouteCtxKey, rctx))
}

// fakeProvider implements providers.Provider for testing.
type fakeProvider struct {
	name string
}

func (f *fakeProvider) Name() string { return f.name }
func (f *fakeProvider) OAuthConfig() *oauth2.Config {
	return &oauth2.Config{ClientID: "fake-id"}
}
func (f *fakeProvider) FetchUser(_ context.Context, _ *oauth2.Token) (*providers.ProviderUser, error) {
	return &providers.ProviderUser{ID: "1", Name: "Fake", Provider: f.name}, nil
}

// fakeOAuthProvider allows injecting a custom oauth2.Config for testing.
type fakeOAuthProvider struct {
	name   string
	config *oauth2.Config
}

func (f *fakeOAuthProvider) Name() string                { return f.name }
func (f *fakeOAuthProvider) OAuthConfig() *oauth2.Config { return f.config }
func (f *fakeOAuthProvider) FetchUser(_ context.Context, _ *oauth2.Token) (*providers.ProviderUser, error) {
	return &providers.ProviderUser{ID: "1", Name: "Fake", Provider: f.name}, nil
}

// fakeAccessDeniedProvider returns an AccessDeniedError from FetchUser.
type fakeAccessDeniedProvider struct {
	name   string
	config *oauth2.Config
}

func (f *fakeAccessDeniedProvider) Name() string                { return f.name }
func (f *fakeAccessDeniedProvider) OAuthConfig() *oauth2.Config { return f.config }
func (f *fakeAccessDeniedProvider) FetchUser(_ context.Context, _ *oauth2.Token) (*providers.ProviderUser, error) {
	return nil, &providers.AccessDeniedError{Message: "user is not a member of any allowed organization"}
}
