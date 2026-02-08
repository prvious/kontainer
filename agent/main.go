package main

import (
	"context"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/clovismuneza/devoper/agent/auth"
	"github.com/clovismuneza/devoper/agent/auth/providers"
	"github.com/clovismuneza/devoper/agent/cluster"
	"github.com/clovismuneza/devoper/agent/config"
	"github.com/clovismuneza/devoper/agent/metrics"
	"github.com/clovismuneza/devoper/agent/server"
	"github.com/clovismuneza/devoper/agent/services"
	"github.com/clovismuneza/devoper/agent/storage"
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
	log.Printf("  role            : %s", cfg.Role)
	log.Printf("  agent url       : %s", cfg.AgentURL)
	log.Printf("  github client id: %s", mask(cfg.GithubClientID))
	log.Printf("  jwt secret      : %s", mask(cfg.JWTSecret))

	if cfg.Role == cluster.RoleFollower {
		startFollower(cfg)
	} else {
		startLeader(cfg)
	}
}

func startFollower(cfg *config.Config) {
	if cfg.InternalSecret == "" {
		log.Fatal("DEVOPER_INTERNAL_SECRET is required for follower mode")
	}

	hostname, _ := os.Hostname()
	nodeID := os.Getenv("DOCKER_NODE_ID")

	dockerClient, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
	if err != nil {
		log.Fatalf("Failed to create Docker client: %v", err)
	}

	if nodeID == "" {
		info, err := dockerClient.Info(context.Background())
		if err == nil && info.Swarm.NodeID != "" {
			nodeID = info.Swarm.NodeID
		}
	}

	nodeAction := metrics.NewCollectNodeMetricsAction(dockerClient)
	containerAction := metrics.NewCollectContainerMetricsAction(dockerClient)
	metricsHandler := metrics.NewInternalMetricsHandler(nodeAction, containerAction, nodeID, hostname)

	identity := server.FollowerIdentity{
		NodeID:   nodeID,
		Hostname: hostname,
	}

	handler := server.NewFollower(identity, cfg.InternalSecret, metricsHandler)

	addr := ":" + cfg.Port
	log.Printf("  listening on    : %s (follower)", addr)
	log.Println("----------------------------")
	log.Fatal(http.ListenAndServe(addr, handler))
}

func startLeader(cfg *config.Config) {
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

	hostname, _ := os.Hostname()

	nodeAction := metrics.NewCollectNodeMetricsAction(dockerClient)
	containerAction := metrics.NewCollectContainerMetricsAction(dockerClient)
	leaderNodeID := os.Getenv("DOCKER_NODE_ID")
	if leaderNodeID == "" {
		info, err := dockerClient.Info(context.Background())
		if err == nil && info.Swarm.NodeID != "" {
			leaderNodeID = info.Swarm.NodeID
		}
	}
	internalMetricsHandler := metrics.NewInternalMetricsHandler(nodeAction, containerAction, leaderNodeID, hostname)

	listServicesAction := services.NewListServicesAction(dockerClient)
	listServicesHandler := services.NewListServicesHandler(listServicesAction)

	inspectServiceAction := services.NewInspectServiceAction(dockerClient)
	inspectServiceHandler := services.NewInspectServiceHandler(inspectServiceAction)

	scaleServiceAction := services.NewScaleServiceAction(dockerClient)
	scaleServiceHandler := services.NewScaleServiceHandler(scaleServiceAction)

	restartServiceAction := services.NewRestartServiceAction(dockerClient)
	restartServiceHandler := services.NewRestartServiceHandler(restartServiceAction)

	rollbackServiceAction := services.NewRollbackServiceAction(dockerClient)
	rollbackServiceHandler := services.NewRollbackServiceHandler(rollbackServiceAction)

	listServiceTasksAction := services.NewListServiceTasksAction(dockerClient)
	listServiceTasksHandler := services.NewListServiceTasksHandler(listServiceTasksAction)

	inspectTaskAction := services.NewInspectTaskAction(dockerClient)
	inspectTaskHandler := services.NewInspectTaskHandler(inspectTaskAction)

	listStacksAction := services.NewListStacksAction(dockerClient)
	listStacksHandler := services.NewListStacksHandler(listStacksAction)

	removeStackAction := services.NewRemoveStackAction(dockerClient)
	removeStackHandler := services.NewRemoveStackHandler(removeStackAction)

	listNodesAction := services.NewListNodesAction(dockerClient)
	listNodesHandler := services.NewListNodesHandler(listNodesAction)

	listTaskStatsAction := services.NewListTaskStatsAction(dockerClient)
	listTaskStatsHandler := services.NewListTaskStatsHandler(listTaskStatsAction)

	listNetworksAction := services.NewListNetworksAction(dockerClient)
	listNetworksHandler := services.NewListNetworksHandler(listNetworksAction)

	listVolumesAction := services.NewListVolumesAction(dockerClient)
	listVolumesHandler := services.NewListVolumesHandler(listVolumesAction)

	updateServiceEnvAction := services.NewUpdateServiceEnvAction(dockerClient)
	updateServiceEnvHandler := services.NewUpdateServiceEnvHandler(updateServiceEnvAction)

	updateServiceImageAction := services.NewUpdateServiceImageAction(dockerClient)
	updateServiceImageHandler := services.NewUpdateServiceImageHandler(updateServiceImageAction)

	updateNodeAvailabilityAction := services.NewUpdateNodeAvailabilityAction(dockerClient)
	updateNodeAvailabilityHandler := services.NewUpdateNodeAvailabilityHandler(updateNodeAvailabilityAction)

	updateNodeRoleAction := services.NewUpdateNodeRoleAction(dockerClient)
	updateNodeRoleHandler := services.NewUpdateNodeRoleHandler(updateNodeRoleAction)

	listSecretsAction := services.NewListSecretsAction(dockerClient)
	listSecretsHandler := services.NewListSecretsHandler(listSecretsAction)

	createSecretAction := services.NewCreateSecretAction(dockerClient)
	createSecretHandler := services.NewCreateSecretHandler(createSecretAction)

	removeSecretAction := services.NewRemoveSecretAction(dockerClient)
	removeSecretHandler := services.NewRemoveSecretHandler(removeSecretAction)

	listConfigsAction := services.NewListConfigsAction(dockerClient)
	listConfigsHandler := services.NewListConfigsHandler(listConfigsAction)

	createConfigAction := services.NewCreateConfigAction(dockerClient)
	createConfigHandler := services.NewCreateConfigHandler(createConfigAction)

	removeConfigAction := services.NewRemoveConfigAction(dockerClient)
	removeConfigHandler := services.NewRemoveConfigHandler(removeConfigAction)

	streamLogsHandler := services.NewStreamServiceLogsHandler(dockerClient, cfg.JWTSecret)
	streamTaskLogsHandler := services.NewStreamTaskLogsHandler(dockerClient, cfg.JWTSecret)

	sessions := auth.NewSessionStore()
	bridge := auth.NewAuthBridge()
	authHandler := auth.NewAuthHandler(registry, sessions, cfg.JWTSecret, bridge)
	jwtMiddleware := auth.JWTMiddleware(cfg.JWTSecret)

	// Cluster setup
	clusterRegistry := cluster.NewRegistry()
	selfInfo := cluster.NewAgentInfo("", hostname, cfg.AgentURL, cluster.RoleLeader, "healthy")

	listAgentsAction := services.NewListAgentsAction(clusterRegistry, selfInfo)
	listAgentsHandler := services.NewListAgentsHandler(listAgentsAction)

	// Metrics storage and background collector
	interval, err := time.ParseDuration(cfg.MetricsInterval)
	if err != nil {
		interval = 30 * time.Second
	}
	retention, err := time.ParseDuration(cfg.MetricsRetention)
	if err != nil {
		retention = 24 * time.Hour
	}

	metricsStore, err := storage.NewMetricsStore(cfg.MetricsDBPath, retention)
	if err != nil {
		log.Fatalf("Failed to create metrics store: %v", err)
	}

	collector := metrics.NewMetricsCollector(internalMetricsHandler, clusterRegistry, cfg.InternalSecret, metricsStore, interval)
	collector.Start(context.Background())

	clusterMetricsHandler := metrics.NewClusterMetricsHandler(internalMetricsHandler, clusterRegistry, cfg.InternalSecret, metricsStore)
	nodeMetricsHandler := metrics.NewNodeMetricsHandler(internalMetricsHandler, clusterRegistry, cfg.InternalSecret)

	// Start follower discovery
	if addrs := cluster.ParseDevFollowers(cfg.DevFollowers); len(addrs) > 0 {
		// Dev mode: ping static localhost addresses
		log.Printf("  dev followers   : %s", cfg.DevFollowers)
		devDiscovery := cluster.NewDevDiscovery(addrs, cfg.InternalSecret, clusterRegistry)
		devDiscovery.Start(context.Background())
		defer devDiscovery.Stop()
	} else if cfg.InternalSecret != "" {
		// Production: discover followers via Swarm API
		discovery := cluster.NewDiscovery(dockerClient, clusterRegistry, cfg.FollowerService, cfg.InternalSecret, cfg.Port)
		discovery.Start(context.Background())
		defer discovery.Stop()
	}

	handler := server.New(authHandler, jwtMiddleware, listServicesHandler, inspectServiceHandler, scaleServiceHandler, restartServiceHandler, rollbackServiceHandler, listStacksHandler, removeStackHandler, listServiceTasksHandler, inspectTaskHandler, listNodesHandler, listTaskStatsHandler, streamLogsHandler, streamTaskLogsHandler, listNetworksHandler, listVolumesHandler, updateServiceEnvHandler, updateServiceImageHandler, updateNodeAvailabilityHandler, updateNodeRoleHandler, listSecretsHandler, createSecretHandler, removeSecretHandler, listConfigsHandler, createConfigHandler, removeConfigHandler, listAgentsHandler, clusterMetricsHandler, nodeMetricsHandler)

	srv := &http.Server{Addr: ":" + cfg.Port, Handler: handler}

	// Graceful shutdown on SIGINT/SIGTERM
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-sigCh
		log.Println("shutting down...")

		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		srv.Shutdown(ctx)
	}()

	log.Printf("  listening on    : %s (leader)", srv.Addr)
	log.Println("----------------------------")

	if err := srv.ListenAndServe(); err != http.ErrServerClosed {
		log.Fatalf("server error: %v", err)
	}

	// Cleanup in correct order: stop collector before closing store
	collector.Stop()
	metricsStore.Close()
}
