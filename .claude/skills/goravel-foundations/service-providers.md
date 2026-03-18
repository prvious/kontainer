# Service Providers Reference

Service providers are the central place to configure and bootstrap all application and framework services. Every provider implements at minimum `Register()` and `Boot()`.

## Provider Interface

```go
type ServiceProvider interface {
    Register(app foundation.Application)
    Boot(app foundation.Application)
}
```

## Register vs Boot

### Register()

Bind services into the container. Do NOT resolve other services here -- they may not be registered yet.

```go
func (r *ServiceProvider) Register(app foundation.Application) {
    app.Singleton("email", func(app foundation.Application) (any, error) {
        return NewEmailService(), nil
    })
}
```

### Boot()

Called after ALL providers have been registered. Safe to resolve services, register routes, event listeners, middleware, etc.

```go
func (r *ServiceProvider) Boot(app foundation.Application) {
    emailSvc := app.MakeMail()
    // Configure, register listeners, etc.
}
```

## Creating a Provider

Generate with Artisan (auto-registers in `bootstrap/providers.go`):

```bash
./artisan make:provider NotificationServiceProvider
```

This creates a file in `app/providers/` and adds the provider to the `Providers` slice.

Manual creation:

```go
package providers

import "github.com/goravel/framework/contracts/foundation"

type NotificationServiceProvider struct{}

func (r *NotificationServiceProvider) Register(app foundation.Application) {
    app.Singleton("notification", func(app foundation.Application) (any, error) {
        return NewNotificationService(app.MakeConfig()), nil
    })
}

func (r *NotificationServiceProvider) Boot(app foundation.Application) {}
```

## Registering Providers

Providers are listed in `bootstrap/providers.go` and passed to `WithProviders` in `bootstrap/app.go`:

```go
// bootstrap/providers.go
func Providers() []foundation.ServiceProvider {
    return []foundation.ServiceProvider{
        &providers.AppServiceProvider{},
        &providers.NotificationServiceProvider{},
    }
}

// bootstrap/app.go
func Boot() contractsfoundation.Application {
    return foundation.Setup().
        WithProviders(Providers).
        WithConfig(config.Boot).
        Create()
}
```

## Dependency Relationship

Implement the optional `Relationship()` method to control provider ordering without relying on slice position:

```go
func (r *ServiceProvider) Relationship() binding.Relationship {
    return binding.Relationship{
        Bindings:     []string{"custom"},          // keys this provider registers
        Dependencies: []string{binding.Config},     // keys that must be registered first
        ProvideFor:   []string{binding.Cache},      // keys that depend on this provider
    }
}
```

- Providers with `Relationship()` are sorted by declared dependencies.
- Providers without `Relationship()` are registered last.

## Runners

Implement the optional `Runners()` method to start/stop long-running processes tied to the provider's service:

```go
func (r *ServiceProvider) Runners(app foundation.Application) []foundation.Runner {
    return []foundation.Runner{
        NewRouteRunner(app.MakeConfig(), app.MakeRoute()),
    }
}
```

Each Runner implements three methods:

```go
type Runner interface {
    ShouldRun() bool   // Gate: return false to skip this runner
    Run() error        // Start the service (e.g., HTTP listen)
    Shutdown() error   // Graceful stop
}
```

Example runner:

```go
type RouteRunner struct {
    config config.Config
    route  route.Route
}

func NewRouteRunner(config config.Config, route route.Route) *RouteRunner {
    return &RouteRunner{config: config, route: route}
}

func (r *RouteRunner) ShouldRun() bool {
    return r.route != nil && r.config.GetString("http.default") != ""
}

func (r *RouteRunner) Run() error {
    return r.route.Run()
}

func (r *RouteRunner) Shutdown() error {
    return r.route.Shutdown()
}
```

Register standalone runners (not tied to a provider) via `WithRunners` in `bootstrap/app.go`:

```go
foundation.Setup().
    WithRunners(func() []foundation.Runner {
        return []foundation.Runner{NewCustomRunner()}
    }).
    Create()
```

## Lifecycle Summary

1. `foundation.Setup()` initializes the app.
2. Configuration loads via `WithConfig`.
3. `Register()` called on every provider (order: providers with `Relationship()` sorted first, then the rest in slice order).
4. `Boot()` called on every provider.
5. `WithCallback` functions execute.
6. `app.Start()` triggers all Runners where `ShouldRun()` returns true.
7. On shutdown, `Shutdown()` called on all running Runners.
