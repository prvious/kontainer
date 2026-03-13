package routes

import (
	"github.com/goravel/framework/contracts/route"
	"github.com/goravel/framework/facades"

	setupactions "github.com/prvious/kontainer/server/app/actions/setup"
	"github.com/prvious/kontainer/server/app/helpers"
	setupcontrollers "github.com/prvious/kontainer/server/app/http/controllers/setup"
	"github.com/prvious/kontainer/server/app/http/middleware"
)

func Setup() {
	setupToken := helpers.GetSetupToken()

	facades.Log().Infof("Setup token: %s", setupToken)

	manifestState := setupcontrollers.NewManifestState()

	checkStatus := setupactions.NewCheckStatusAction()
	exchangeCode := setupactions.NewExchangeCodeAction()
	completeSetup := setupactions.NewCompleteSetupAction()

	statusCtrl := setupcontrollers.NewStatusController(checkStatus)
	manifestCtrl := setupcontrollers.NewManifestController(manifestState)
	startCtrl := setupcontrollers.NewStartController(manifestState)
	callbackCtrl := setupcontrollers.NewCallbackController(manifestState, exchangeCode)
	completeCtrl := setupcontrollers.NewCompleteController(completeSetup)

	facades.Route().Prefix("setup").Group(func(router route.Router) {
		router.Get("status", statusCtrl.Handle)
		router.Get("github/start", startCtrl.Handle)
		router.Get("github/callback", callbackCtrl.Handle)

		router.Middleware(middleware.SetupToken(setupToken)).Group(func(r route.Router) {
			r.Post("manifest", manifestCtrl.Handle)
			r.Post("complete", completeCtrl.Handle)
		})
	})
}
