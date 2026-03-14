package bootstrap

import (
	contractsfoundation "github.com/goravel/framework/contracts/foundation"
	contractsconfiguration "github.com/goravel/framework/contracts/foundation/configuration"
	"github.com/goravel/framework/foundation"
	googlegrpc "google.golang.org/grpc"

	grpcinterceptors "github.com/prvious/kontainer/server/app/grpc/interceptors"
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
		WithGrpcServerInterceptors(func() []googlegrpc.UnaryServerInterceptor {
			return []googlegrpc.UnaryServerInterceptor{
				grpcinterceptors.SecretAuth,
			}
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
