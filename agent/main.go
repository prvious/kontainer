package main

import (
	"log"
	"net/http"

	"github.com/clovismuneza/devoper/agent/auth"
	"github.com/clovismuneza/devoper/agent/auth/providers"
	"github.com/clovismuneza/devoper/agent/config"
	"github.com/clovismuneza/devoper/agent/server"
	"github.com/joho/godotenv"
)

func mask(s string) string {
	if len(s) <= 4 {
		return "***"
	}
	return s[:4] + "***"
}

func main() {
	_ = godotenv.Load()

	cfg := config.Load()

	log.Println("--- devoper agent config ---")
	log.Printf("  agent url       : %s", cfg.AgentURL)
	log.Printf("  app scheme      : %s", cfg.AppScheme)
	log.Printf("  github client id: %s", mask(cfg.GithubClientID))
	log.Printf("  jwt secret      : %s", mask(cfg.JWTSecret))

	if cfg.GithubClientID == "" || cfg.GithubClientSecret == "" {
		log.Println("  ⚠ GITHUB_CLIENT_ID or GITHUB_CLIENT_SECRET is empty — OAuth will not work")
	}
	if cfg.JWTSecret == "" {
		log.Println("  ⚠ JWT_SECRET is empty — auth tokens will not be signed")
	}

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
	log.Printf("  listening on    : %s", addr)
	log.Println("----------------------------")
	log.Fatal(http.ListenAndServe(addr, handler))
}
