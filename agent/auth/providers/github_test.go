package providers

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"golang.org/x/oauth2"
)

func TestNewGitHubProvider_Scopes(t *testing.T) {
	p := NewGitHubProvider("id", "secret", "http://localhost/callback", "my-org")
	scopes := p.config.Scopes

	found := false
	for _, s := range scopes {
		if s == "read:org" {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("expected read:org scope when org is non-empty, got %v", scopes)
	}
}

func TestNewGitHubProvider_ScopesNoOrg(t *testing.T) {
	p := NewGitHubProvider("id", "secret", "http://localhost/callback", "")
	scopes := p.config.Scopes

	for _, s := range scopes {
		if s == "read:org" {
			t.Errorf("did not expect read:org scope when org is empty, got %v", scopes)
			break
		}
	}
}

func TestCheckAccess_NoOrg(t *testing.T) {
	p := &GitHubProvider{}
	err := p.checkAccess(http.DefaultClient, "anyone")
	if err != nil {
		t.Fatalf("expected nil when org is empty, got %v", err)
	}
}

func TestCheckAccess_Member(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNoContent) // 204 = member
	}))
	defer srv.Close()

	p := &GitHubProvider{org: "acme-corp"}

	err := p.checkAccess(testClientWithServer(srv), "alice")
	if err != nil {
		t.Fatalf("expected nil for 204 response, got %v", err)
	}
}

func TestCheckAccess_NotMember(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNotFound) // 404 = not a member
	}))
	defer srv.Close()

	p := &GitHubProvider{org: "acme-corp"}

	err := p.checkAccess(testClientWithServer(srv), "charlie")
	if err == nil {
		t.Fatal("expected error for 404 response, got nil")
	}

	var accessErr *AccessDeniedError
	if !isAccessDenied(err, &accessErr) {
		t.Fatalf("expected AccessDeniedError, got %T: %v", err, err)
	}
}

func TestCheckAccess_APIError(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusInternalServerError) // 500
	}))
	defer srv.Close()

	p := &GitHubProvider{org: "acme-corp"}

	err := p.checkAccess(testClientWithServer(srv), "alice")
	if err == nil {
		t.Fatal("expected error for 500 response, got nil")
	}

	// Should be an AccessDeniedError (any non-204 is "not a member")
	var accessErr *AccessDeniedError
	if !isAccessDenied(err, &accessErr) {
		t.Fatalf("expected AccessDeniedError, got %T: %v", err, err)
	}
}

func TestCheckAccess_CorrectURL(t *testing.T) {
	var requestedPath string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requestedPath = r.URL.Path
		w.WriteHeader(http.StatusNoContent)
	}))
	defer srv.Close()

	p := &GitHubProvider{org: "acme-corp"}

	err := p.checkAccess(testClientWithServer(srv), "alice")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expected := "/orgs/acme-corp/members/alice"
	if requestedPath != expected {
		t.Errorf("expected request path %q, got %q", expected, requestedPath)
	}
}

func TestFetchUser_SkipsOrgCheckWhenNoOrg(t *testing.T) {
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
		default:
			orgsCalled = true
		}
	}))
	defer srv.Close()

	p := &GitHubProvider{
		config: &oauth2.Config{
			Endpoint: oauth2.Endpoint{
				TokenURL: srv.URL + "/token",
			},
		},
		org: "", // no org restriction
	}

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
		t.Error("did not expect emails endpoint to be called when profile has email")
	}
	if orgsCalled {
		t.Error("did not expect org check when org is empty")
	}
	if user.Name != "Test User" {
		t.Errorf("expected name 'Test User', got %q", user.Name)
	}
}

func TestFetchUser_WithOrg(t *testing.T) {
	profileCalled := false
	orgCheckCalled := false

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
		case "/orgs/acme-corp/members/testuser":
			orgCheckCalled = true
			w.WriteHeader(http.StatusNoContent) // 204 = member
		}
	}))
	defer srv.Close()

	p := &GitHubProvider{
		config: &oauth2.Config{
			Endpoint: oauth2.Endpoint{
				TokenURL: srv.URL + "/token",
			},
		},
		org: "acme-corp",
	}

	token := &oauth2.Token{AccessToken: "test-token"}
	ctx := context.WithValue(context.Background(), oauth2.HTTPClient, testClientWithServer(srv))

	user, err := p.FetchUser(ctx, token)
	if err != nil {
		t.Fatalf("FetchUser failed: %v", err)
	}

	if !profileCalled {
		t.Error("expected profile endpoint to be called")
	}
	if !orgCheckCalled {
		t.Error("expected org membership check to be called")
	}
	if user.ID != "42" {
		t.Errorf("expected user ID '42', got %q", user.ID)
	}
	if user.Name != "Test User" {
		t.Errorf("expected name 'Test User', got %q", user.Name)
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
