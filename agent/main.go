package main

import (
	"context"
	"flag"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/docker/docker/client"
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
)

func mask(s string) string {
	if len(s) <= 4 {
		return "***"
	}
	return s[:4] + "***"
}

func main() {
	configPath := flag.String("config", "kontainer.yml", "path to config file")
	flag.Parse()

	cfg, err := config.Load(*configPath)
	if err != nil {
		log.Fatalf("failed to load config: %v", err)
	}

	log.Println("--- kontainer agent config ---")
	log.Printf("  config file     : %s", *configPath)
	log.Printf("  role            : %s", cfg.Role)
	log.Printf("  agent url       : %s", cfg.AgentURL)
	log.Printf("  github client id: %s", mask(cfg.GitHub.ClientID))
	log.Printf("  jwt secret      : %s", mask(cfg.JWTSecret))

	if cfg.Role == cluster.RoleFollower {
		startFollower(cfg)
	} else {
		startLeader(cfg)
	}
}

func startFollower(cfg *config.Config) {
	if cfg.Cluster.InternalSecret == "" {
		log.Fatal("cluster.internal_secret is required for follower mode")
	}

	hostname, _ := os.Hostname()

	dockerClient, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
	if err != nil {
		log.Fatalf("Failed to create Docker client: %v", err)
	}

	nodeID := ""
	info, err := dockerClient.Info(context.Background())
	if err == nil && info.Swarm.NodeID != "" {
		nodeID = info.Swarm.NodeID
	}

	nodeAction := metrics.NewCollectNodeMetricsAction()
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
	handler := server.NewFollower(identity, cfg.Cluster.InternalSecret, metricsHandler, execFollowerHandler, internalImagesHandler, internalRemoveImageHandler, internalPruneImagesHandler)

	srv := &http.Server{Addr: ":" + cfg.Port, Handler: handler}

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
	if cfg.GitHub.ClientID == "" || cfg.GitHub.ClientSecret == "" {
		log.Println("  ⚠ github.client_id or github.client_secret is empty — OAuth will not work")
	}
	if cfg.JWTSecret == "" {
		log.Println("  ⚠ jwt_secret is empty — auth tokens will not be signed")
	}

	if len(cfg.GitHub.AllowedOrgs) > 0 {
		log.Printf("  GitHub org allowlist: %v", cfg.GitHub.AllowedOrgs)
	}
	if len(cfg.GitHub.AllowedUsers) > 0 {
		log.Printf("  GitHub user allowlist: %v", cfg.GitHub.AllowedUsers)
	}

	registry := providers.NewProviderRegistry()
	registry.Register(providers.NewGitHubProvider(
		cfg.GitHub.ClientID,
		cfg.GitHub.ClientSecret,
		cfg.AgentURL+"/auth/github/callback",
		cfg.GitHub.AllowedOrgs,
		cfg.GitHub.AllowedUsers,
	))

	dockerClient, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
	if err != nil {
		log.Fatalf("Failed to create Docker client: %v", err)
	}

	hostname, _ := os.Hostname()

	nodeAction := metrics.NewCollectNodeMetricsAction()
	containerAction := metrics.NewCollectContainerMetricsAction(dockerClient)
	leaderNodeID := ""
	info, err := dockerClient.Info(context.Background())
	if err == nil && info.Swarm.NodeID != "" {
		leaderNodeID = info.Swarm.NodeID
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

	listTasksAction := services.NewListTasksAction(dockerClient)
	listTasksHandler := services.NewListTasksHandler(listTasksAction)

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

	clusterRegistry := cluster.NewRegistry()
	selfInfo := cluster.NewAgentInfo("", hostname, cfg.AgentURL, cluster.RoleLeader, "healthy")

	listAgentsAction := services.NewListAgentsAction(clusterRegistry, selfInfo)
	listAgentsHandler := services.NewListAgentsHandler(listAgentsAction)

	execHandler := services.NewExecContainerHandler(dockerClient, cfg.JWTSecret, clusterRegistry, leaderNodeID, cfg.Cluster.InternalSecret)

	interval, err := time.ParseDuration(cfg.Metrics.Interval)
	if err != nil {
		interval = 30 * time.Second
	}
	retention, err := time.ParseDuration(cfg.Metrics.Retention)
	if err != nil {
		retention = 24 * time.Hour
	}

	metricsStore, err := storage.NewMetricsStore(cfg.Metrics.DBPath, retention)
	if err != nil {
		log.Fatalf("Failed to create metrics store: %v", err)
	}

	settingsStore, err := storage.NewSettingsStore(cfg.Metrics.DBPath)
	if err != nil {
		log.Fatalf("Failed to create settings store: %v", err)
	}

	collector := metrics.NewMetricsCollector(internalMetricsHandler, clusterRegistry, cfg.Cluster.InternalSecret, metricsStore, interval)
	collector.Start(context.Background())

	clusterMetricsHandler := metrics.NewClusterMetricsHandler(internalMetricsHandler, clusterRegistry, cfg.Cluster.InternalSecret, metricsStore)
	nodeMetricsHandler := metrics.NewNodeMetricsHandler(internalMetricsHandler, clusterRegistry, cfg.Cluster.InternalSecret)

	listImagesAction := images.NewListImagesAction(dockerClient)
	leaderImagesHandler := images.NewInternalImagesHandler(listImagesAction, leaderNodeID, hostname)
	clusterImagesHandler := images.NewClusterImagesHandler(leaderImagesHandler, clusterRegistry, cfg.Cluster.InternalSecret)
	removeImageHandler := images.NewRemoveImageHandler(dockerClient, clusterRegistry, leaderNodeID, cfg.Cluster.InternalSecret)
	pruneImagesHandler := images.NewPruneImagesHandler(dockerClient, clusterRegistry, leaderNodeID, cfg.Cluster.InternalSecret)

	getSettingsAction := settings.NewGetSettingsAction(settingsStore)
	getSettingsHandler := settings.NewGetSettingsHandler(getSettingsAction)
	updateSettingsAction := settings.NewUpdateSettingsAction(settingsStore)
	updateSettingsHandler := settings.NewUpdateSettingsHandler(updateSettingsAction, getSettingsAction)

	if len(cfg.Cluster.DevFollowers) > 0 {
		log.Printf("  dev followers   : %v", cfg.Cluster.DevFollowers)
		devDiscovery := cluster.NewDevDiscovery(cfg.Cluster.DevFollowers, cfg.Cluster.InternalSecret, clusterRegistry)
		devDiscovery.Start(context.Background())
		defer devDiscovery.Stop()
	} else if cfg.Cluster.InternalSecret != "" {
		discovery := cluster.NewDiscovery(dockerClient, clusterRegistry, cfg.Cluster.FollowerService, cfg.Cluster.InternalSecret, cfg.Port)
		discovery.Start(context.Background())
		defer discovery.Stop()
	}

	handler := server.New(authHandler, jwtMiddleware, listServicesHandler, inspectServiceHandler, scaleServiceHandler, restartServiceHandler, rollbackServiceHandler, listStacksHandler, removeStackHandler, listServiceTasksHandler, inspectTaskHandler, listNodesHandler, listNodeTasksHandler, listTaskStatsHandler, listTasksHandler, streamLogsHandler, streamTaskLogsHandler, listNetworksHandler, listVolumesHandler, updateServiceEnvHandler, updateServiceImageHandler, updateNodeAvailabilityHandler, updateNodeRoleHandler, listSecretsHandler, createSecretHandler, removeSecretHandler, listConfigsHandler, createConfigHandler, removeConfigHandler, inspectConfigHandler, listAgentsHandler, execHandler, clusterMetricsHandler, nodeMetricsHandler, clusterImagesHandler, removeImageHandler, pruneImagesHandler, getSettingsHandler, updateSettingsHandler)

	srv := &http.Server{Addr: ":" + cfg.Port, Handler: handler}

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

	collector.Stop()
	settingsStore.Close()
	metricsStore.Close()
}
