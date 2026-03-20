package setup

import "sync"

// ManifestState holds transient state for the GitHub App manifest setup flow.
type ManifestState struct {
	mu    sync.Mutex
	state string
	token string
	org   string
}

func NewManifestState() *ManifestState {
	return &ManifestState{}
}

func (s *ManifestState) Set(state, token, org string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.state = state
	s.token = token
	s.org = org
}

func (s *ManifestState) Get() (state, token, org string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.state, s.token, s.org
}

func (s *ManifestState) ClearToken() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.token = ""
}

func (s *ManifestState) ClearState() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.state = ""
}
