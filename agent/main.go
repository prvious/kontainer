package main

import (
	"context"
	"log"
	"net/http"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/prvious/kontainer/agent/auth"
	"github.com/prvious/kontainer/agent/auth/providers"
	"github.com/prvious/kontainer/agent/cluster"
	"github.com/prvious/kontainer/agent/config"
	"github.com/prvious/kontainer/agent/images"
	"github.com/prvious/kontainer/agent/metrics"
	"github.com/prvious/kontainer/agent/server"
	"github.com/prvious/kontainer/agent/services"
	"github.com/prvious/kontainer/agent/settings"
	"github.com/prvious/kontainer/agent/storage"
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

	log.Println("--- kontainer agent config ---")
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
		log.Fatal("KONTAINER_INTERNAL_SECRET is required for follower mode")
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

	listImagesAction := images.NewListImagesAction(dockerClient)
	internalImagesHandler := images.NewInternalImagesHandler(listImagesAction, nodeID, hostname)
	internalRemoveImageHandler := images.NewInternalRemoveImageHandler(dockerClient)
	internalPruneImagesHandler := images.NewInternalPruneImagesHandler(dockerClient)

	execFollowerHandler := services.NewExecContainerFollowerHandler(dockerClient)
	handler := server.NewFollower(identity, cfg.InternalSecret, metricsHandler, execFollowerHandler, internalImagesHandler, internalRemoveImageHandler, internalPruneImagesHandler)

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

	log.Printf("  listening on    : %s (follower)", srv.Addr)
	log.Println("----------------------------")

	if err := srv.ListenAndServe(); err != http.ErrServerClosed {
		log.Fatalf("server error: %v", err)
	}
}

func startLeader(cfg *config.Config) {
	if cfg.GithubClientID == "" || cfg.GithubClientSecret == "" {
		log.Println("  ⚠ GITHUB_CLIENT_ID or GITHUB_CLIENT_SECRET is empty — OAuth will not work")
	}
	if cfg.JWTSecret == "" {
		log.Println("  ⚠ JWT_SECRET is empty — auth tokens will not be signed")
	}

	var allowedOrgs []string
	if cfg.GithubAllowedOrgs != "" {
		for _, org := range strings.Split(cfg.GithubAllowedOrgs, ",") {
			if o := strings.TrimSpace(org); o != "" {
				allowedOrgs = append(allowedOrgs, o)
			}
		}
	}
	if len(allowedOrgs) > 0 {
		log.Printf("  GitHub org allowlist: %v", allowedOrgs)
	}

	var allowedUsers []string
	if cfg.GithubAllowedUsers != "" {
		for _, user := range strings.Split(cfg.GithubAllowedUsers, ",") {
			if u := strings.TrimSpace(user); u != "" {
				allowedUsers = append(allowedUsers, u)
			}
		}
	}
	if len(allowedUsers) > 0 {
		log.Printf("  GitHub user allowlist: %v", allowedUsers)
	}

	registry := providers.NewProviderRegistry()
	registry.Register(providers.NewGitHubProvider(
		cfg.GithubClientID,
		cfg.GithubClientSecret,
		cfg.AgentURL+"/auth/github/callback",
		allowedOrgs,
		allowedUsers,
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

	listNodeTasksAction := services.NewListNodeTasksAction(dockerClient)
	listNodeTasksHandler := services.NewListNodeTasksHandler(listNodeTasksAction)

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

	inspectConfigAction := services.NewInspectConfigAction(dockerClient)
	inspectConfigHandler := services.NewInspectConfigHandler(inspectConfigAction)

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

	execHandler := services.NewExecContainerHandler(dockerClient, cfg.JWTSecret, clusterRegistry, leaderNodeID, cfg.InternalSecret)

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

	settingsStore, err := storage.NewSettingsStore(cfg.MetricsDBPath)
	if err != nil {
		log.Fatalf("Failed to create settings store: %v", err)
	}

	collector := metrics.NewMetricsCollector(internalMetricsHandler, clusterRegistry, cfg.InternalSecret, metricsStore, interval)
	collector.Start(context.Background())

	clusterMetricsHandler := metrics.NewClusterMetricsHandler(internalMetricsHandler, clusterRegistry, cfg.InternalSecret, metricsStore)
	nodeMetricsHandler := metrics.NewNodeMetricsHandler(internalMetricsHandler, clusterRegistry, cfg.InternalSecret)

	listImagesAction := images.NewListImagesAction(dockerClient)
	leaderImagesHandler := images.NewInternalImagesHandler(listImagesAction, leaderNodeID, hostname)
	clusterImagesHandler := images.NewClusterImagesHandler(leaderImagesHandler, clusterRegistry, cfg.InternalSecret)
	removeImageHandler := images.NewRemoveImageHandler(dockerClient, clusterRegistry, leaderNodeID, cfg.InternalSecret)
	pruneImagesHandler := images.NewPruneImagesHandler(dockerClient, clusterRegistry, leaderNodeID, cfg.InternalSecret)

	getSettingsAction := settings.NewGetSettingsAction(settingsStore)
	getSettingsHandler := settings.NewGetSettingsHandler(getSettingsAction)
	updateSettingsAction := settings.NewUpdateSettingsAction(settingsStore)
	updateSettingsHandler := settings.NewUpdateSettingsHandler(updateSettingsAction, getSettingsAction)

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

	handler := server.New(authHandler, jwtMiddleware, listServicesHandler, inspectServiceHandler, scaleServiceHandler, restartServiceHandler, rollbackServiceHandler, listStacksHandler, removeStackHandler, listServiceTasksHandler, inspectTaskHandler, listNodesHandler, listNodeTasksHandler, listTaskStatsHandler, streamLogsHandler, streamTaskLogsHandler, listNetworksHandler, listVolumesHandler, updateServiceEnvHandler, updateServiceImageHandler, updateNodeAvailabilityHandler, updateNodeRoleHandler, listSecretsHandler, createSecretHandler, removeSecretHandler, listConfigsHandler, createConfigHandler, removeConfigHandler, inspectConfigHandler, listAgentsHandler, execHandler, clusterMetricsHandler, nodeMetricsHandler, clusterImagesHandler, removeImageHandler, pruneImagesHandler, getSettingsHandler, updateSettingsHandler)

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

	// Cleanup in correct order: stop collector before closing stores
	collector.Stop()
	settingsStore.Close()
	metricsStore.Close()
}
