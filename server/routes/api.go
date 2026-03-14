package routes

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/goravel/framework/contracts/route"

	agentActions "github.com/prvious/kontainer/server/app/actions/agents"
	configActions "github.com/prvious/kontainer/server/app/actions/configs"
	networkActions "github.com/prvious/kontainer/server/app/actions/networks"
	nodeActions "github.com/prvious/kontainer/server/app/actions/nodes"
	secretActions "github.com/prvious/kontainer/server/app/actions/secrets"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
	stackActions "github.com/prvious/kontainer/server/app/actions/stacks"
	taskActions "github.com/prvious/kontainer/server/app/actions/tasks"
	volumeActions "github.com/prvious/kontainer/server/app/actions/volumes"
	"github.com/prvious/kontainer/server/app/facades"
	agentControllers "github.com/prvious/kontainer/server/app/http/controllers/agents"
	authcontrollers "github.com/prvious/kontainer/server/app/http/controllers/auth"
	configControllers "github.com/prvious/kontainer/server/app/http/controllers/configs"
	networkControllers "github.com/prvious/kontainer/server/app/http/controllers/networks"
	nodeControllers "github.com/prvious/kontainer/server/app/http/controllers/nodes"
	secretControllers "github.com/prvious/kontainer/server/app/http/controllers/secrets"
	serviceControllers "github.com/prvious/kontainer/server/app/http/controllers/services"
	stackControllers "github.com/prvious/kontainer/server/app/http/controllers/stacks"
	taskControllers "github.com/prvious/kontainer/server/app/http/controllers/tasks"
	volumeControllers "github.com/prvious/kontainer/server/app/http/controllers/volumes"
	"github.com/prvious/kontainer/server/app/http/middleware"
	"github.com/prvious/kontainer/server/app/services/cluster"
	"github.com/prvious/kontainer/server/app/services/docker"
)

func Api() {
	meCtrl := authcontrollers.NewMeController()

	// Resolve orchestrator for all Docker API controllers
	orchestrator := resolveOrchestrator()

	// Agents controller from cluster registry
	agentsHandler := buildAgentsHandler()

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
			router.Get("nodes", unavailable)
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

		router.Get("tasks", listTasks.Handle)
		router.Get("tasks/stats", taskStats.Handle)
		router.Get("tasks/{taskID}", inspectTask.Handle)

		// Nodes
		listNodes := nodeControllers.NewListNodesController(nodeActions.NewListNodesAction(orchestrator))
		updateAvail := nodeControllers.NewUpdateNodeAvailabilityController(nodeActions.NewUpdateNodeAvailabilityAction(orchestrator))
		updateRole := nodeControllers.NewUpdateNodeRoleController(nodeActions.NewUpdateNodeRoleAction(orchestrator))

		router.Get("nodes", listNodes.Handle)
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
	})
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
