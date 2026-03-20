package routes

import (
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

func unavailableHandler(message string) func(ctx http.Context) http.Response {
	return func(ctx http.Context) http.Response {
		return ctx.Response().Json(http.StatusServiceUnavailable, map[string]string{"error": message})
	}
}

func Api() {
	if facades.Config().GetString("cluster.role") != "leader" {
		return
	}

	// Resolve shared dependencies once
	orchestrator, _ := resolve[docker.Orchestrator]("docker.orchestrator")
	registry, _ := resolve[*cluster.Registry]("cluster.registry")
	pool, _ := resolve[*cluster.GrpcPool]("cluster.pool")
	hostname, _ := resolve[string]("docker.hostname")
	leaderNodeID := facades.Config().GetString("cluster.node_id", "")

	// WebSocket routes — inside /api prefix but WITHOUT JWT middleware.
	// WS controllers handle JWT auth from the ?token= query param themselves.
	registerWebSocketRoutes(orchestrator, registry, pool, leaderNodeID)

	facades.Route().Prefix("api").Middleware(middleware.JWT()).Group(func(router route.Router) {
		router.Get("me", authcontrollers.NewMeController().Handle)

		// Agents
		if registry != nil {
			agentsCtrl := agentControllers.NewListAgentsController(agentActions.NewListAgentsAction(registry))
			router.Get("agents", agentsCtrl.Handle)
		} else {
			router.Get("agents", unavailableHandler("cluster management unavailable"))
		}

		if orchestrator == nil {
			facades.Log().Warningf("Docker orchestrator unavailable — all Docker API routes will return 503")
			dockerUnavailable := unavailableHandler("docker orchestrator is not available on this agent")
			router.Get("services", dockerUnavailable)
			router.Get("services/{serviceID}", dockerUnavailable)
			router.Post("services/{serviceID}/scale", dockerUnavailable)
			router.Post("services/{serviceID}/restart", dockerUnavailable)
			router.Post("services/{serviceID}/rollback", dockerUnavailable)
			router.Put("services/{serviceID}/env", dockerUnavailable)
			router.Put("services/{serviceID}/image", dockerUnavailable)
			router.Get("tasks", dockerUnavailable)
			router.Get("tasks/stats", dockerUnavailable)
			router.Get("tasks/{taskID}", dockerUnavailable)
			router.Get("task-stats", dockerUnavailable)
			router.Get("services/{serviceID}/tasks", dockerUnavailable)
			router.Get("nodes", dockerUnavailable)
			router.Get("nodes/{nodeID}/tasks", dockerUnavailable)
			router.Patch("nodes/{nodeID}/availability", dockerUnavailable)
			router.Patch("nodes/{nodeID}/role", dockerUnavailable)
			router.Get("stacks", dockerUnavailable)
			router.Delete("stacks/{stackName}", dockerUnavailable)
			router.Get("networks", dockerUnavailable)
			router.Get("volumes", dockerUnavailable)
			router.Get("secrets", dockerUnavailable)
			router.Post("secrets", dockerUnavailable)
			router.Delete("secrets/{secretID}", dockerUnavailable)
			router.Get("configs", dockerUnavailable)
			router.Post("configs", dockerUnavailable)
			router.Get("configs/{configID}", dockerUnavailable)
			router.Delete("configs/{configID}", dockerUnavailable)
		} else {
			// Services
			router.Get("services", serviceControllers.NewListServicesController(serviceActions.NewListServicesAction(orchestrator)).Handle)
			router.Get("services/{serviceID}", serviceControllers.NewInspectServiceController(serviceActions.NewInspectServiceAction(orchestrator)).Handle)
			router.Post("services/{serviceID}/scale", serviceControllers.NewScaleServiceController(serviceActions.NewScaleServiceAction(orchestrator)).Handle)
			router.Post("services/{serviceID}/restart", serviceControllers.NewRestartServiceController(serviceActions.NewRestartServiceAction(orchestrator)).Handle)
			router.Post("services/{serviceID}/rollback", serviceControllers.NewRollbackServiceController(serviceActions.NewRollbackServiceAction(orchestrator)).Handle)
			router.Put("services/{serviceID}/env", serviceControllers.NewUpdateServiceEnvController(serviceActions.NewUpdateServiceEnvAction(orchestrator)).Handle)
			router.Put("services/{serviceID}/image", serviceControllers.NewUpdateServiceImageController(serviceActions.NewUpdateServiceImageAction(orchestrator)).Handle)

			// Tasks
			listTasks := taskControllers.NewListTasksController(taskActions.NewListTasksAction(orchestrator))
			router.Get("tasks", listTasks.Handle)
			router.Get("tasks/stats", taskControllers.NewListTaskStatsController(taskActions.NewListTaskStatsAction(orchestrator)).Handle)
			router.Get("tasks/{taskID}", taskControllers.NewInspectTaskController(taskActions.NewInspectTaskAction(orchestrator)).Handle)
			router.Get("task-stats", taskControllers.NewListTaskStatsAggController(taskActions.NewListTaskStatsAggAction(orchestrator)).Handle)
			router.Get("services/{serviceID}/tasks", listTasks.Handle)

			// Nodes
			router.Get("nodes", nodeControllers.NewListNodesController(nodeActions.NewListNodesAction(orchestrator)).Handle)
			router.Get("nodes/{nodeID}/tasks", listTasks.Handle)
			router.Patch("nodes/{nodeID}/availability", nodeControllers.NewUpdateNodeAvailabilityController(nodeActions.NewUpdateNodeAvailabilityAction(orchestrator)).Handle)
			router.Patch("nodes/{nodeID}/role", nodeControllers.NewUpdateNodeRoleController(nodeActions.NewUpdateNodeRoleAction(orchestrator)).Handle)

			// Stacks
			router.Get("stacks", stackControllers.NewListStacksController(stackActions.NewListStacksAction(orchestrator)).Handle)
			router.Delete("stacks/{stackName}", stackControllers.NewRemoveStackController(stackActions.NewRemoveStackAction(orchestrator)).Handle)

			// Networks & Volumes
			router.Get("networks", networkControllers.NewListNetworksController(networkActions.NewListNetworksAction(orchestrator)).Handle)
			router.Get("volumes", volumeControllers.NewListVolumesController(volumeActions.NewListVolumesAction(orchestrator)).Handle)

			// Secrets
			router.Get("secrets", secretControllers.NewListSecretsController(secretActions.NewListSecretsAction(orchestrator)).Handle)
			router.Post("secrets", secretControllers.NewCreateSecretController(secretActions.NewCreateSecretAction(orchestrator)).Handle)
			router.Delete("secrets/{secretID}", secretControllers.NewRemoveSecretController(secretActions.NewRemoveSecretAction(orchestrator)).Handle)

			// Configs
			router.Get("configs", configControllers.NewListConfigsController(configActions.NewListConfigsAction(orchestrator)).Handle)
			router.Post("configs", configControllers.NewCreateConfigController(configActions.NewCreateConfigAction(orchestrator)).Handle)
			router.Get("configs/{configID}", configControllers.NewInspectConfigController(configActions.NewInspectConfigAction(orchestrator)).Handle)
			router.Delete("configs/{configID}", configControllers.NewRemoveConfigController(configActions.NewRemoveConfigAction(orchestrator)).Handle)
		}

		// Metrics
		collector, collectorOk := resolve[*metricsActions.CollectAndStoreAction]("metrics.collector")
		if collectorOk && registry != nil {
			router.Get("metrics", metricsControllers.NewClusterMetricsController(metricsActions.NewGetClusterMetricsAction(collector, registry)).Handle)
			router.Get("metrics/{nodeID}", metricsControllers.NewNodeMetricsController(metricsActions.NewGetNodeMetricsAction(collector, registry, leaderNodeID)).Handle)
		} else {
			metricsUnavailable := unavailableHandler("metrics unavailable")
			router.Get("metrics", metricsUnavailable)
			router.Get("metrics/{nodeID}", metricsUnavailable)
		}

		// Images
		if orchestrator != nil && registry != nil && pool != nil {
			router.Get("images", imageControllers.NewClusterImagesController(imageActions.NewListClusterImagesAction(orchestrator, registry, pool, leaderNodeID, hostname)).Handle)
			router.Delete("images/{imageID}", imageControllers.NewRemoveImageController(imageActions.NewRemoveImageAction(orchestrator, registry, pool, leaderNodeID)).Handle)
			router.Post("images/prune", imageControllers.NewPruneImagesController(imageActions.NewPruneImagesAction(orchestrator, registry, pool, leaderNodeID)).Handle)
		} else {
			imagesUnavailable := unavailableHandler("image management unavailable")
			router.Get("images", imagesUnavailable)
			router.Delete("images/{imageID}", imagesUnavailable)
			router.Post("images/prune", imagesUnavailable)
		}

		// Settings — no orchestrator dependency
		getSettingsAction := settingsActions.NewGetSettingsAction()
		updateSettingsAction := settingsActions.NewUpdateSettingsAction()
		router.Get("settings", settingsControllers.NewGetSettingsController(getSettingsAction).Handle)
		router.Put("settings", settingsControllers.NewUpdateSettingsController(getSettingsAction, updateSettingsAction).Handle)
	})
}

func registerWebSocketRoutes(orchestrator docker.Orchestrator, registry *cluster.Registry, pool *cluster.GrpcPool, leaderNodeID string) {
	if orchestrator == nil {
		facades.Log().Warningf("Docker orchestrator unavailable — WebSocket log/exec routes will return 503")
		wsUnavailable := unavailableHandler("docker orchestrator is not available on this agent")
		facades.Route().Prefix("api").Group(func(router route.Router) {
			router.Get("services/{serviceID}/logs", wsUnavailable)
			router.Get("tasks/{taskID}/logs", wsUnavailable)
			router.Get("exec/{containerID}", wsUnavailable)
		})
		return
	}

	serviceLogsCtrl := wsControllers.NewServiceLogsController(ws.NewServiceLogsHandler(orchestrator))
	taskLogsCtrl := wsControllers.NewTaskLogsController(ws.NewTaskLogsHandler(orchestrator))

	facades.Route().Prefix("api").Group(func(router route.Router) {
		router.Get("services/{serviceID}/logs", serviceLogsCtrl.Handle)
		router.Get("tasks/{taskID}/logs", taskLogsCtrl.Handle)

		if registry != nil && pool != nil {
			execHandler := ws.NewExecHandler(orchestrator, registry, pool, leaderNodeID)
			execCtrl := wsControllers.NewExecController(execHandler)
			router.Get("exec/{containerID}", execCtrl.Handle)
		}
	})
}
