package setup

import (
	"fmt"
	"sync"

	contractshttp "github.com/goravel/framework/contracts/http"

	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/helpers"
)

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

type ManifestController struct {
	manifestState *ManifestState
}

func NewManifestController(state *ManifestState) *ManifestController {
	return &ManifestController{manifestState: state}
}

func (c *ManifestController) Handle(ctx contractshttp.Context) contractshttp.Response {
	org := ctx.Request().Input("org")

	state := helpers.RandomHex(16)
	token := helpers.RandomHex(16)

	c.manifestState.Set(state, token, org)

	agentURL := facades.Config().GetString("cluster.agent_url", "http://localhost:8080")
	setupURL := fmt.Sprintf("%s/setup/github/start?token=%s", agentURL, token)

	return ctx.Response().Json(200, map[string]any{"setupUrl": setupURL})
}

