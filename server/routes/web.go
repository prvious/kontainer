package routes

import (
	controllers "github.com/prvious/kontainer/server/app/http/controllers"
	"github.com/prvious/kontainer/server/app/facades"
)

func Web() {
	healthCtrl := controllers.NewHealthController()
	pingCtrl := controllers.NewPingController()

	facades.Route().Get("health", healthCtrl.Handle)
	facades.Route().Get("ping", pingCtrl.Handle)
}
