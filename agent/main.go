package main

import (
	"log"
	"net/http"

	"github.com/clovismuneza/devoper/agent/auth"
	"github.com/clovismuneza/devoper/agent/auth/providers"
	"github.com/clovismuneza/devoper/agent/config"
	"github.com/clovismuneza/devoper/agent/server"
)

func main() {
	cfg := config.Load()

	registry := providers.NewProviderRegistry()
	registry.Register(providers.NewGitHubProvider(
		cfg.GithubClientID,
		cfg.GithubClientSecret,
		cfg.AgentURL+"/auth/github/callback",
	))

	sessions := auth.NewSessionStore()
	authHandler := auth.NewAuthHandler(registry, sessions, cfg.JWTSecret, cfg.AppScheme)
	jwtMiddleware := auth.JWTMiddleware(cfg.JWTSecret)

	handler := server.New(authHandler, jwtMiddleware)

	addr := ":" + cfg.Port
	log.Printf("devoper agent listening on %s", addr)
	log.Fatal(http.ListenAndServe(addr, handler))
}
