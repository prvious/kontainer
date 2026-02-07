package main

import (
	"log"
	"net/http"

	"github.com/clovismuneza/devoper/agent/auth"
	"github.com/clovismuneza/devoper/agent/auth/providers"
	"github.com/clovismuneza/devoper/agent/config"
	"github.com/clovismuneza/devoper/agent/server"
	"github.com/clovismuneza/devoper/agent/services"
	"github.com/docker/docker/client"
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

	dockerClient, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
	if err != nil {
		log.Fatalf("Failed to create Docker client: %v", err)
	}

	listServicesAction := services.NewListServicesAction(dockerClient)
	listServicesHandler := services.NewListServicesHandler(listServicesAction)

	inspectServiceAction := services.NewInspectServiceAction(dockerClient)
	inspectServiceHandler := services.NewInspectServiceHandler(inspectServiceAction)

	listNodesAction := services.NewListNodesAction(dockerClient)
	listNodesHandler := services.NewListNodesHandler(listNodesAction)

	listTaskStatsAction := services.NewListTaskStatsAction(dockerClient)
	listTaskStatsHandler := services.NewListTaskStatsHandler(listTaskStatsAction)

	streamLogsHandler := services.NewStreamServiceLogsHandler(dockerClient, cfg.JWTSecret)

	sessions := auth.NewSessionStore()
	bridge := auth.NewAuthBridge()
	authHandler := auth.NewAuthHandler(registry, sessions, cfg.JWTSecret, bridge)
	jwtMiddleware := auth.JWTMiddleware(cfg.JWTSecret)

	handler := server.New(authHandler, jwtMiddleware, listServicesHandler, inspectServiceHandler, listNodesHandler, listTaskStatsHandler, streamLogsHandler)

	addr := ":" + cfg.Port
	log.Printf("  listening on    : %s", addr)
	log.Println("----------------------------")
	log.Fatal(http.ListenAndServe(addr, handler))
}
