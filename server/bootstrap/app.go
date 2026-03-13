package bootstrap

import (
	contractsfoundation "github.com/goravel/framework/contracts/foundation"
	contractsconfiguration "github.com/goravel/framework/contracts/foundation/configuration"
	"github.com/goravel/framework/foundation"

	"github.com/prvious/kontainer/server/app/http/middleware"
	"github.com/prvious/kontainer/server/config"
	"github.com/prvious/kontainer/server/routes"
)

func Boot() contractsfoundation.Application {
	return foundation.Setup().
		WithMigrations(Migrations).
		WithMiddleware(func(handler contractsconfiguration.Middleware) {
			handler.Append(middleware.SetupGuard())
		}).
		WithRouting(func() {
			routes.Web()
			routes.Auth()
			routes.Api()
			routes.Setup()
			routes.Grpc()
		}).
		WithProviders(Providers).
		WithConfig(config.Boot).
		Create()
}
