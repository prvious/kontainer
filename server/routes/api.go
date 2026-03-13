package routes

import (
	"github.com/goravel/framework/contracts/route"
	"github.com/goravel/framework/facades"

	authcontrollers "github.com/prvious/kontainer/server/app/http/controllers/auth"
	"github.com/prvious/kontainer/server/app/http/middleware"
)

func Api() {
	meCtrl := authcontrollers.NewMeController()

	facades.Route().Prefix("api").Middleware(middleware.JWT()).Group(func(router route.Router) {
		router.Get("me", meCtrl.Handle)
	})
}
