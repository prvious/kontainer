package setup

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"sync"

	contractshttp "github.com/goravel/framework/contracts/http"
	"github.com/goravel/framework/facades"
)

type ManifestState struct {
	mu    sync.Mutex
	State string
	Token string
	Org   string
}

func NewManifestState() *ManifestState {
	return &ManifestState{}
}

func (s *ManifestState) Set(state, token, org string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.State = state
	s.Token = token
	s.Org = org
}

func (s *ManifestState) Get() (state, token, org string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.State, s.Token, s.Org
}

func (s *ManifestState) ClearToken() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.Token = ""
}

func (s *ManifestState) ClearState() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.State = ""
}

type ManifestController struct {
	manifestState *ManifestState
}

func NewManifestController(state *ManifestState) *ManifestController {
	return &ManifestController{manifestState: state}
}

func (c *ManifestController) Handle(ctx contractshttp.Context) contractshttp.Response {
	org := ctx.Request().Input("org")

	state := randomHex(16)
	token := randomHex(16)

	c.manifestState.Set(state, token, org)

	agentURL := facades.Config().GetString("cluster.agent_url", "http://localhost:8080")
	setupURL := fmt.Sprintf("%s/setup/github/start?token=%s", agentURL, token)

	return ctx.Response().Json(200, map[string]any{"setupUrl": setupURL})
}

func randomHex(nBytes int) string {
	b := make([]byte, nBytes)
	if _, err := rand.Read(b); err != nil {
		panic("crypto/rand failed: " + err.Error())
	}
	return hex.EncodeToString(b)
}
