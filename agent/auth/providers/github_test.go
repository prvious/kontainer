package providers

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"golang.org/x/oauth2"
)

func TestCheckOrgMembership_Match(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode([]githubOrg{
			{Login: "acme-corp"},
			{Login: "other-org"},
		})
	}))
	defer srv.Close()

	p := &GitHubProvider{
		allowedOrgs: []string{"acme-corp"},
	}

	client := srv.Client()
	// Override the URL by wrapping the transport
	originalTransport := client.Transport
	client.Transport = roundTripFunc(func(req *http.Request) (*http.Response, error) {
		req.URL.Scheme = "http"
		req.URL.Host = srv.Listener.Addr().String()
		if originalTransport != nil {
			return originalTransport.RoundTrip(req)
		}
		return http.DefaultTransport.RoundTrip(req)
	})

	err := p.checkOrgMembership(client)
	if err != nil {
		t.Fatalf("expected nil, got %v", err)
	}
}

func TestCheckOrgMembership_CaseInsensitive(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode([]githubOrg{
			{Login: "Acme-Corp"},
		})
	}))
	defer srv.Close()

	p := &GitHubProvider{
		allowedOrgs: []string{"acme-corp"},
	}

	client := testClientWithServer(srv)

	err := p.checkOrgMembership(client)
	if err != nil {
		t.Fatalf("expected nil (case-insensitive match), got %v", err)
	}
}

func TestCheckOrgMembership_NoMatch(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode([]githubOrg{
			{Login: "unrelated-org"},
		})
	}))
	defer srv.Close()

	p := &GitHubProvider{
		allowedOrgs: []string{"acme-corp"},
	}

	client := testClientWithServer(srv)

	err := p.checkOrgMembership(client)
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	var accessErr *AccessDeniedError
	if !isAccessDenied(err, &accessErr) {
		t.Fatalf("expected AccessDeniedError, got %T: %v", err, err)
	}
}

func TestCheckOrgMembership_APIError(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusInternalServerError)
	}))
	defer srv.Close()

	p := &GitHubProvider{
		allowedOrgs: []string{"acme-corp"},
	}

	client := testClientWithServer(srv)

	err := p.checkOrgMembership(client)
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	// Should be a regular error, not AccessDeniedError
	var accessErr *AccessDeniedError
	if isAccessDenied(err, &accessErr) {
		t.Fatal("expected regular error, not AccessDeniedError")
	}
}

func TestFetchUser_SkipsOrgCheckWhenNoAllowedOrgs(t *testing.T) {
	profileCalled := false
	emailsCalled := false
	orgsCalled := false

	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/user":
			profileCalled = true
			json.NewEncoder(w).Encode(githubProfile{
				ID:    42,
				Login: "testuser",
				Name:  "Test User",
				Email: "test@example.com",
			})
		case "/user/emails":
			emailsCalled = true
			json.NewEncoder(w).Encode([]githubEmail{})
		case "/user/orgs":
			orgsCalled = true
			json.NewEncoder(w).Encode([]githubOrg{})
		}
	}))
	defer srv.Close()

	p := &GitHubProvider{
		config: &oauth2.Config{
			Endpoint: oauth2.Endpoint{
				TokenURL: srv.URL + "/token",
			},
		},
		allowedOrgs: nil, // no org restriction
	}

	// Create a token with a transport that redirects to the test server
	token := &oauth2.Token{AccessToken: "test-token"}
	ctx := context.WithValue(context.Background(), oauth2.HTTPClient, testClientWithServer(srv))

	user, err := p.FetchUser(ctx, token)
	if err != nil {
		t.Fatalf("FetchUser failed: %v", err)
	}

	if !profileCalled {
		t.Error("expected profile endpoint to be called")
	}
	if emailsCalled {
		// Email should not be called since profile has an email
		t.Error("did not expect emails endpoint to be called")
	}
	if orgsCalled {
		t.Error("did not expect orgs endpoint to be called when allowedOrgs is empty")
	}
	if user.Name != "Test User" {
		t.Errorf("expected name 'Test User', got %q", user.Name)
	}
}

func TestNewGitHubProvider_ScopesWithOrgs(t *testing.T) {
	p := NewGitHubProvider("id", "secret", "http://localhost/callback", []string{"my-org"})
	scopes := p.config.Scopes

	found := false
	for _, s := range scopes {
		if s == "read:org" {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("expected read:org scope when allowedOrgs is set, got %v", scopes)
	}
}

func TestNewGitHubProvider_ScopesWithoutOrgs(t *testing.T) {
	p := NewGitHubProvider("id", "secret", "http://localhost/callback", nil)
	scopes := p.config.Scopes

	for _, s := range scopes {
		if s == "read:org" {
			t.Errorf("did not expect read:org scope when allowedOrgs is empty, got %v", scopes)
			break
		}
	}
}

// -- helpers --

type roundTripFunc func(*http.Request) (*http.Response, error)

func (f roundTripFunc) RoundTrip(req *http.Request) (*http.Response, error) {
	return f(req)
}

func testClientWithServer(srv *httptest.Server) *http.Client {
	client := srv.Client()
	client.Transport = roundTripFunc(func(req *http.Request) (*http.Response, error) {
		req.URL.Scheme = "http"
		req.URL.Host = srv.Listener.Addr().String()
		return http.DefaultTransport.RoundTrip(req)
	})
	return client
}

func isAccessDenied(err error, target **AccessDeniedError) bool {
	for err != nil {
		if e, ok := err.(*AccessDeniedError); ok {
			*target = e
			return true
		}
		// unwrap
		if u, ok := err.(interface{ Unwrap() error }); ok {
			err = u.Unwrap()
		} else {
			return false
		}
	}
	return false
}
