package routes

import (
	"github.com/prvious/kontainer/server/app/facades"
	healthControllers "github.com/prvious/kontainer/server/app/http/controllers/health"
	pingControllers "github.com/prvious/kontainer/server/app/http/controllers/ping"
)

func Web() {
	healthCtrl := healthControllers.NewHealthController()
	pingCtrl := pingControllers.NewPingController()

	facades.Route().Get("health", healthCtrl.Handle)
	facades.Route().Get("ping", pingCtrl.Handle)
}
