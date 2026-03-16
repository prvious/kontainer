package bootstrap

import (
	"os"

	contractsfoundation "github.com/goravel/framework/contracts/foundation"
	contractsconfiguration "github.com/goravel/framework/contracts/foundation/configuration"
	"github.com/goravel/framework/contracts/schedule"
	"github.com/goravel/framework/foundation"
	googlegrpc "google.golang.org/grpc"

	"github.com/prvious/kontainer/server/app/console"
	grpcinterceptors "github.com/prvious/kontainer/server/app/grpc/interceptors"
	"github.com/prvious/kontainer/server/app/http/middleware"
	"github.com/prvious/kontainer/server/config"
	"github.com/prvious/kontainer/server/routes"
)

func Boot() contractsfoundation.Application {
	// Use os.Getenv because facades are not bootstrapped yet during Setup().
	role := os.Getenv("AGENT_ROLE")
	if role == "" {
		role = "leader"
	}

	builder := foundation.Setup().
		WithMigrations(Migrations).
		WithGrpcServerInterceptors(func() []googlegrpc.UnaryServerInterceptor {
			return []googlegrpc.UnaryServerInterceptor{
				grpcinterceptors.SecretAuth,
			}
		}).
		WithProviders(Providers).
		WithConfig(config.Boot)

	if role == "leader" {
		builder = builder.
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
			WithSchedule(func() []schedule.Event {
				kernel := &console.Kernel{}
				return kernel.Schedule()
			})
	} else {
		// Follower: only gRPC server, no HTTP routes or scheduled tasks
		builder = builder.
			WithRouting(func() {
				routes.Grpc()
			})
	}

	return builder.Create()
}
