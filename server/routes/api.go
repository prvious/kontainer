package routes

import (
	"os"

	"github.com/goravel/framework/contracts/http"
	"github.com/goravel/framework/contracts/route"

	agentActions "github.com/prvious/kontainer/server/app/actions/agents"
	configActions "github.com/prvious/kontainer/server/app/actions/configs"
	imageActions "github.com/prvious/kontainer/server/app/actions/images"
	metricsActions "github.com/prvious/kontainer/server/app/actions/metrics"
	networkActions "github.com/prvious/kontainer/server/app/actions/networks"
	nodeActions "github.com/prvious/kontainer/server/app/actions/nodes"
	secretActions "github.com/prvious/kontainer/server/app/actions/secrets"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
	settingsActions "github.com/prvious/kontainer/server/app/actions/settings"
	stackActions "github.com/prvious/kontainer/server/app/actions/stacks"
	taskActions "github.com/prvious/kontainer/server/app/actions/tasks"
	volumeActions "github.com/prvious/kontainer/server/app/actions/volumes"
	"github.com/prvious/kontainer/server/app/facades"
	agentControllers "github.com/prvious/kontainer/server/app/http/controllers/agents"
	authcontrollers "github.com/prvious/kontainer/server/app/http/controllers/auth"
	configControllers "github.com/prvious/kontainer/server/app/http/controllers/configs"
	imageControllers "github.com/prvious/kontainer/server/app/http/controllers/images"
	metricsControllers "github.com/prvious/kontainer/server/app/http/controllers/metrics"
	networkControllers "github.com/prvious/kontainer/server/app/http/controllers/networks"
	nodeControllers "github.com/prvious/kontainer/server/app/http/controllers/nodes"
	secretControllers "github.com/prvious/kontainer/server/app/http/controllers/secrets"
	serviceControllers "github.com/prvious/kontainer/server/app/http/controllers/services"
	settingsControllers "github.com/prvious/kontainer/server/app/http/controllers/settings"
	stackControllers "github.com/prvious/kontainer/server/app/http/controllers/stacks"
	taskControllers "github.com/prvious/kontainer/server/app/http/controllers/tasks"
	volumeControllers "github.com/prvious/kontainer/server/app/http/controllers/volumes"
	wsControllers "github.com/prvious/kontainer/server/app/http/controllers/websocket"
	"github.com/prvious/kontainer/server/app/http/middleware"
	"github.com/prvious/kontainer/server/app/services/cluster"
	"github.com/prvious/kontainer/server/app/services/docker"
	ws "github.com/prvious/kontainer/server/app/services/websocket"
)

func Api() {
	if facades.Config().GetString("cluster.role") != "leader" {
		return
	}

	meCtrl := authcontrollers.NewMeController()

	// Resolve orchestrator for all Docker API controllers
	orchestrator := resolveOrchestrator()

	// Agents controller from cluster registry
	agentsHandler := buildAgentsHandler()

	// WebSocket routes — inside /api prefix but WITHOUT JWT middleware.
	// WS controllers handle JWT auth from the ?token= query param themselves.
	registerWebSocketRoutes(orchestrator)

	facades.Route().Prefix("api").Middleware(middleware.JWT()).Group(func(router route.Router) {
		router.Get("me", meCtrl.Handle)
		router.Get("agents", agentsHandler)

		if orchestrator == nil {
			facades.Log().Warningf("Docker orchestrator unavailable — all Docker API routes will return 503")
			unavailable := func(ctx http.Context) http.Response {
				return ctx.Response().Json(503, map[string]string{
					"error": "docker orchestrator is not available on this agent",
				})
			}
			router.Get("services", unavailable)
			router.Get("services/{serviceID}", unavailable)
			router.Post("services/{serviceID}/scale", unavailable)
			router.Post("services/{serviceID}/restart", unavailable)
			router.Post("services/{serviceID}/rollback", unavailable)
			router.Put("services/{serviceID}/env", unavailable)
			router.Put("services/{serviceID}/image", unavailable)
			router.Get("tasks", unavailable)
			router.Get("tasks/stats", unavailable)
			router.Get("tasks/{taskID}", unavailable)
			router.Get("task-stats", unavailable)
			router.Get("services/{serviceID}/tasks", unavailable)
			router.Get("nodes", unavailable)
			router.Get("nodes/{nodeID}/tasks", unavailable)
			router.Patch("nodes/{nodeID}/availability", unavailable)
			router.Patch("nodes/{nodeID}/role", unavailable)
			router.Get("stacks", unavailable)
			router.Delete("stacks/{stackName}", unavailable)
			router.Get("networks", unavailable)
			router.Get("volumes", unavailable)
			router.Get("secrets", unavailable)
			router.Post("secrets", unavailable)
			router.Delete("secrets/{secretID}", unavailable)
			router.Get("configs", unavailable)
			router.Post("configs", unavailable)
			router.Get("configs/{configID}", unavailable)
			router.Delete("configs/{configID}", unavailable)
			return
		}

		// Services
		listSvc := serviceControllers.NewListServicesController(serviceActions.NewListServicesAction(orchestrator))
		inspectSvc := serviceControllers.NewInspectServiceController(serviceActions.NewInspectServiceAction(orchestrator))
		scaleSvc := serviceControllers.NewScaleServiceController(serviceActions.NewScaleServiceAction(orchestrator))
		restartSvc := serviceControllers.NewRestartServiceController(serviceActions.NewRestartServiceAction(orchestrator))
		rollbackSvc := serviceControllers.NewRollbackServiceController(serviceActions.NewRollbackServiceAction(orchestrator))
		updateEnv := serviceControllers.NewUpdateServiceEnvController(serviceActions.NewUpdateServiceEnvAction(orchestrator))
		updateImg := serviceControllers.NewUpdateServiceImageController(serviceActions.NewUpdateServiceImageAction(orchestrator))

		router.Get("services", listSvc.Handle)
		router.Get("services/{serviceID}", inspectSvc.Handle)
		router.Post("services/{serviceID}/scale", scaleSvc.Handle)
		router.Post("services/{serviceID}/restart", restartSvc.Handle)
		router.Post("services/{serviceID}/rollback", rollbackSvc.Handle)
		router.Put("services/{serviceID}/env", updateEnv.Handle)
		router.Put("services/{serviceID}/image", updateImg.Handle)

		// Tasks
		listTasks := taskControllers.NewListTasksController(taskActions.NewListTasksAction(orchestrator))
		inspectTask := taskControllers.NewInspectTaskController(taskActions.NewInspectTaskAction(orchestrator))
		taskStats := taskControllers.NewListTaskStatsController(taskActions.NewListTaskStatsAction(orchestrator))
		taskStatsAgg := taskControllers.NewListTaskStatsAggController(taskActions.NewListTaskStatsAggAction(orchestrator))

		router.Get("tasks", listTasks.Handle)
		router.Get("tasks/stats", taskStats.Handle)
		router.Get("tasks/{taskID}", inspectTask.Handle)
		router.Get("task-stats", taskStatsAgg.Handle)
		router.Get("services/{serviceID}/tasks", listTasks.Handle)

		// Nodes
		listNodes := nodeControllers.NewListNodesController(nodeActions.NewListNodesAction(orchestrator))
		updateAvail := nodeControllers.NewUpdateNodeAvailabilityController(nodeActions.NewUpdateNodeAvailabilityAction(orchestrator))
		updateRole := nodeControllers.NewUpdateNodeRoleController(nodeActions.NewUpdateNodeRoleAction(orchestrator))

		router.Get("nodes", listNodes.Handle)
		router.Get("nodes/{nodeID}/tasks", listTasks.Handle)
		router.Patch("nodes/{nodeID}/availability", updateAvail.Handle)
		router.Patch("nodes/{nodeID}/role", updateRole.Handle)

		// Stacks
		listStacks := stackControllers.NewListStacksController(stackActions.NewListStacksAction(orchestrator))
		removeStack := stackControllers.NewRemoveStackController(stackActions.NewRemoveStackAction(orchestrator))

		router.Get("stacks", listStacks.Handle)
		router.Delete("stacks/{stackName}", removeStack.Handle)

		// Networks & Volumes
		listNets := networkControllers.NewListNetworksController(networkActions.NewListNetworksAction(orchestrator))
		listVols := volumeControllers.NewListVolumesController(volumeActions.NewListVolumesAction(orchestrator))

		router.Get("networks", listNets.Handle)
		router.Get("volumes", listVols.Handle)

		// Secrets
		listSecrets := secretControllers.NewListSecretsController(secretActions.NewListSecretsAction(orchestrator))
		createSecret := secretControllers.NewCreateSecretController(secretActions.NewCreateSecretAction(orchestrator))
		removeSecret := secretControllers.NewRemoveSecretController(secretActions.NewRemoveSecretAction(orchestrator))

		router.Get("secrets", listSecrets.Handle)
		router.Post("secrets", createSecret.Handle)
		router.Delete("secrets/{secretID}", removeSecret.Handle)

		// Configs
		listConfigs := configControllers.NewListConfigsController(configActions.NewListConfigsAction(orchestrator))
		createConfig := configControllers.NewCreateConfigController(configActions.NewCreateConfigAction(orchestrator))
		inspectConfig := configControllers.NewInspectConfigController(configActions.NewInspectConfigAction(orchestrator))
		removeConfig := configControllers.NewRemoveConfigController(configActions.NewRemoveConfigAction(orchestrator))

		router.Get("configs", listConfigs.Handle)
		router.Post("configs", createConfig.Handle)
		router.Get("configs/{configID}", inspectConfig.Handle)
		router.Delete("configs/{configID}", removeConfig.Handle)

		// Metrics
		clusterMetricsCtrl, nodeMetricsCtrl := buildMetricsControllers()
		if clusterMetricsCtrl != nil {
			router.Get("metrics", clusterMetricsCtrl.Handle)
			router.Get("metrics/{nodeID}", nodeMetricsCtrl.Handle)
		} else {
			unavailable := func(ctx http.Context) http.Response {
				return ctx.Response().Json(503, map[string]string{"error": "metrics unavailable"})
			}
			router.Get("metrics", unavailable)
			router.Get("metrics/{nodeID}", unavailable)
		}

		// Images
		clusterImagesCtrl, removeImageCtrl, pruneImagesCtrl := buildImageControllers(orchestrator)
		if clusterImagesCtrl != nil {
			router.Get("images", clusterImagesCtrl.Handle)
			router.Delete("images/{imageID}", removeImageCtrl.Handle)
			router.Post("images/prune", pruneImagesCtrl.Handle)
		} else {
			unavailable := func(ctx http.Context) http.Response {
				return ctx.Response().Json(503, map[string]string{"error": "image management unavailable"})
			}
			router.Get("images", unavailable)
			router.Delete("images/{imageID}", unavailable)
			router.Post("images/prune", unavailable)
		}
	})

	// Settings routes — no orchestrator dependency
	facades.Route().Prefix("api").Middleware(middleware.JWT()).Group(func(router route.Router) {
		getSettingsAction := settingsActions.NewGetSettingsAction()
		updateSettingsAction := settingsActions.NewUpdateSettingsAction()

		getSettingsCtrl := settingsControllers.NewGetSettingsController(getSettingsAction)
		updateSettingsCtrl := settingsControllers.NewUpdateSettingsController(getSettingsAction, updateSettingsAction)

		router.Get("settings", getSettingsCtrl.Handle)
		router.Put("settings", updateSettingsCtrl.Handle)
	})
}

func registerWebSocketRoutes(orchestrator docker.Orchestrator) {
	if orchestrator == nil {
		facades.Log().Warningf("Docker orchestrator unavailable — WebSocket log/exec routes will return 503")
		unavailable := func(ctx http.Context) http.Response {
			return ctx.Response().Json(503, http.Json{
				"error": "docker orchestrator is not available on this agent",
			})
		}
		facades.Route().Prefix("api").Group(func(router route.Router) {
			router.Get("services/{serviceID}/logs", unavailable)
			router.Get("tasks/{taskID}/logs", unavailable)
			router.Get("exec/{containerID}", unavailable)
		})
		return
	}

	serviceLogsHandler := ws.NewServiceLogsHandler(orchestrator)
	taskLogsHandler := ws.NewTaskLogsHandler(orchestrator)

	serviceLogsCtrl := wsControllers.NewServiceLogsController(serviceLogsHandler)
	taskLogsCtrl := wsControllers.NewTaskLogsController(taskLogsHandler)

	// Build exec handler with cluster dependencies
	execCtrl := buildExecController(orchestrator)

	facades.Route().Prefix("api").Group(func(router route.Router) {
		router.Get("services/{serviceID}/logs", serviceLogsCtrl.Handle)
		router.Get("tasks/{taskID}/logs", taskLogsCtrl.Handle)

		if execCtrl != nil {
			router.Get("exec/{containerID}", execCtrl.Handle)
		}
	})
}

func buildExecController(orchestrator docker.Orchestrator) *wsControllers.ExecController {
	registryInstance, err := facades.App().Make("cluster.registry")
	if err != nil {
		facades.Log().Errorf("Failed to resolve cluster.registry for exec handler: %v", err)
		return nil
	}
	registry, ok := registryInstance.(*cluster.Registry)
	if !ok {
		facades.Log().Errorf("cluster.registry is not *cluster.Registry for exec handler")
		return nil
	}

	poolInstance, err := facades.App().Make("cluster.pool")
	if err != nil {
		facades.Log().Errorf("Failed to resolve cluster.pool for exec handler: %v", err)
		return nil
	}
	pool, ok := poolInstance.(*cluster.GrpcPool)
	if !ok {
		facades.Log().Errorf("cluster.pool is not *cluster.GrpcPool for exec handler")
		return nil
	}

	leaderNodeID := facades.Config().GetString("cluster.node_id", "")

	execHandler := ws.NewExecHandler(orchestrator, registry, pool, leaderNodeID)
	return wsControllers.NewExecController(execHandler)
}

func resolveOrchestrator() docker.Orchestrator {
	instance, err := facades.App().Make("docker.orchestrator")
	if err != nil {
		facades.Log().Errorf("Failed to resolve docker.orchestrator for API routes: %v", err)
		return nil
	}
	orch, ok := instance.(docker.Orchestrator)
	if !ok {
		facades.Log().Errorf("docker.orchestrator does not implement docker.Orchestrator")
		return nil
	}
	return orch
}

func buildMetricsControllers() (*metricsControllers.ClusterMetricsController, *metricsControllers.NodeMetricsController) {
	collectorInstance, err := facades.App().Make("metrics.collector")
	if err != nil {
		facades.Log().Warningf("metrics routes unavailable: failed to resolve metrics.collector: %v", err)
		return nil, nil
	}
	collector, ok := collectorInstance.(*metricsActions.CollectAndStoreAction)
	if !ok {
		facades.Log().Warningf("metrics routes unavailable: metrics.collector is not *metricsActions.CollectAndStoreAction")
		return nil, nil
	}

	registryInstance, err := facades.App().Make("cluster.registry")
	if err != nil {
		facades.Log().Warningf("metrics routes unavailable: failed to resolve cluster.registry: %v", err)
		return nil, nil
	}
	registry, ok := registryInstance.(*cluster.Registry)
	if !ok {
		facades.Log().Warningf("metrics routes unavailable: cluster.registry is not *cluster.Registry")
		return nil, nil
	}

	leaderNodeID := facades.Config().GetString("cluster.node_id", "")

	clusterMetricsCtrl := metricsControllers.NewClusterMetricsController(
		metricsActions.NewGetClusterMetricsAction(collector, registry),
	)
	nodeMetricsCtrl := metricsControllers.NewNodeMetricsController(
		metricsActions.NewGetNodeMetricsAction(collector, registry, leaderNodeID),
	)

	return clusterMetricsCtrl, nodeMetricsCtrl
}

func buildImageControllers(orchestrator docker.Orchestrator) (
	*imageControllers.ClusterImagesController,
	*imageControllers.RemoveImageController,
	*imageControllers.PruneImagesController,
) {
	registryInstance, err := facades.App().Make("cluster.registry")
	if err != nil {
		facades.Log().Warningf("image routes unavailable: failed to resolve cluster.registry: %v", err)
		return nil, nil, nil
	}
	registry, ok := registryInstance.(*cluster.Registry)
	if !ok {
		facades.Log().Warningf("image routes unavailable: cluster.registry is not *cluster.Registry")
		return nil, nil, nil
	}

	poolInstance, err := facades.App().Make("cluster.pool")
	if err != nil {
		facades.Log().Warningf("image routes unavailable: failed to resolve cluster.pool: %v", err)
		return nil, nil, nil
	}
	pool, ok := poolInstance.(*cluster.GrpcPool)
	if !ok {
		facades.Log().Warningf("image routes unavailable: cluster.pool is not *cluster.GrpcPool")
		return nil, nil, nil
	}

	leaderNodeID := facades.Config().GetString("cluster.node_id", "")
	hostname, err := os.Hostname()
	if err != nil {
		facades.Log().Warningf("image routes: failed to get hostname, using empty string: %v", err)
	}

	clusterImagesCtrl := imageControllers.NewClusterImagesController(
		imageActions.NewListClusterImagesAction(orchestrator, registry, pool, leaderNodeID, hostname),
	)
	removeImageCtrl := imageControllers.NewRemoveImageController(
		imageActions.NewRemoveImageAction(orchestrator, registry, pool, leaderNodeID),
	)
	pruneImagesCtrl := imageControllers.NewPruneImagesController(
		imageActions.NewPruneImagesAction(orchestrator, registry, pool, leaderNodeID),
	)

	return clusterImagesCtrl, removeImageCtrl, pruneImagesCtrl
}

func buildAgentsHandler() func(ctx http.Context) http.Response {
	unavailable := func(ctx http.Context) http.Response {
		return ctx.Response().Json(503, map[string]any{
			"error": "cluster management unavailable",
		})
	}

	registryInstance, err := facades.App().Make("cluster.registry")
	if err != nil {
		facades.Log().Errorf("Failed to resolve cluster.registry for /api/agents: %v", err)
		return unavailable
	}

	registry, ok := registryInstance.(*cluster.Registry)
	if !ok {
		facades.Log().Errorf("cluster.registry resolved but is not *cluster.Registry (got %T)", registryInstance)
		return unavailable
	}

	action := agentActions.NewListAgentsAction(registry)
	ctrl := agentControllers.NewListAgentsController(action)
	return ctrl.Handle
}
