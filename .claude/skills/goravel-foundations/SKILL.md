---
name: goravel-foundations
description: Use when working with Goravel service container, service providers, facades, configuration, or application bootstrap
---

# Goravel Foundations

Goravel's foundation layer manages the application lifecycle: bootstrapping, service registration, dependency resolution, and configuration. All framework and application services flow through the service container and are wired together via service providers.

**Version:** All patterns target Goravel v1.17.

## Request Lifecycle

1. `main.go` calls `bootstrap.Boot()` then `app.Wait()` to keep the process alive.
2. `bootstrap.Boot()` calls `foundation.Setup()` and chains `With*` methods:

```go
func Boot() contractsfoundation.Application {
    return foundation.Setup().
        WithConfig(config.Boot).
        WithProviders(Providers).
        WithRunners(func() []foundation.Runner { ... }).
        WithCallback(func() {
            // Post-boot code; all facades available here
        }).
        Create()
}
```

3. `Create()` loads configuration, calls `Register()` on all providers, then `Boot()` on all providers, then returns the app instance.
4. `app.Start()` launches HTTP/gRPC servers (via Runners). Custom code runs either before `app.Start()` in `main.go` or inside `WithCallback`.

## Service Container

Register bindings inside a service provider's `Register()` method:

```go
// Transient — new instance every resolve
app.Bind(key, func(app foundation.Application) (any, error) {
    return NewMyService(app.MakeConfig()), nil
})

// Singleton — resolved once, cached forever
app.Singleton(key, func(app foundation.Application) (any, error) {
    return NewMyService(app.MakeConfig()), nil
})

// Instance — bind an already-constructed value
app.Instance(key, myInstance)

// BindWith — receive extra parameters at resolve time
app.BindWith(key, func(app foundation.Application, params map[string]any) (any, error) {
    return NewMyService(params["id"]), nil
})
```

Resolve bindings:

```go
instance, err := app.Make(key)                           // basic resolve
instance, err := app.MakeWith(key, map[string]any{"id": 1}) // with params (matches BindWith)
```

Outside a provider, use the `App` facade: `facades.App().Make(key)`.

Convenience resolvers exist for all framework services: `app.MakeConfig()`, `app.MakeRoute()`, `app.MakeAuth()`, `app.MakeArtisan()`, etc.

See [service-container.md](./service-container.md) for full API reference.

## Service Providers

Every provider implements `Register(app)` and `Boot(app)`:

```go
type ServiceProvider struct{}

func (r *ServiceProvider) Register(app foundation.Application) {
    app.Singleton("myservice", func(app foundation.Application) (any, error) {
        return NewMyService(), nil
    })
}

func (r *ServiceProvider) Boot(app foundation.Application) {
    // Use resolved services here (e.g., register routes, event listeners)
}
```

- **Register** -- only bind into the container. Never resolve other services here.
- **Boot** -- all providers are registered; safe to resolve and use services.

Register providers in `bootstrap/app.go` via `WithProviders(Providers)`, where `Providers` is defined in `bootstrap/providers.go`.

Generate a provider: `./artisan make:provider MyProvider` (auto-registers it).

Providers can optionally implement `Runners()` to start/stop long-running processes (HTTP server, scheduler, queue worker) and `Relationship()` to declare dependency ordering.

See [service-providers.md](./service-providers.md) for full reference.

## Facades

Facades are thin functions in `app/facades/` that resolve services from the container:

```go
// app/facades/route.go
package facades

import "github.com/goravel/framework/contracts/route"

func Route() route.Route {
    return App().MakeRoute()
}
```

Use facades anywhere after boot:

```go
facades.Config().GetString("app.name", "default")
facades.Route().Get("/ping", handler)
```

Override or create custom facades by adding files to `app/facades/`.

Install/uninstall facades with `./artisan package:install <Name>` and `./artisan package:uninstall <Name>`.

See [facades.md](./facades.md) for the full facade list and custom facade patterns.

## Configuration

Config files live in the `config/` directory. Register them via `WithConfig(config.Boot)` in `bootstrap/app.go`.

```go
// Read from .env
facades.Config().Env("APP_NAME", "goravel")

// Read from config files (dot-notation)
facades.Config().GetString("app.name", "goravel")
facades.Config().GetInt("app.port", 8080)
facades.Config().GetBool("app.debug", false)
facades.Config().Get("app.name", "goravel") // returns any

// Set at runtime
facades.Config().Add("custom.key", "value")
```

Environment variables come from `.env` (copy `.env.example`). Override with real env vars or `--env=path` flag.

## Directory Structure

```
goravel/
  app/
    console/        # Artisan commands
    grpc/           # gRPC controllers + middleware
    http/           # HTTP controllers + middleware
    models/         # ORM models
    providers/      # Service providers
    facades/        # Facade accessors
  bootstrap/
    app.go          # Boot(), WithConfig, WithProviders, etc.
    providers.go    # Provider list
  config/           # Config files (app.go, database.go, etc.)
  database/
    migrations/
    seeders/
  routes/           # Route definitions
  storage/          # Logs, cache, compiled
  tests/
  main.go           # Entry point
```

Customize paths via `WithPaths()` in `bootstrap/app.go`:

```go
foundation.Setup().WithPaths(func(paths configuration.Paths) {
    paths.App("src")  // rename app/ to src/
})
```

## Related Skills

- **goravel-http** — Routing (`facades.Route()`), controllers, middleware, request/response, validation
- **goravel-data** — ORM models, migrations, query builder (`facades.Orm()`, `facades.DB()`)
- **goravel-security** — Authentication (`facades.Auth()`), authorization (`facades.Gate()`), encryption, hashing
- **goravel-async** — Queues, events, scheduling, caching (`facades.Queue()`, `facades.Cache()`)
- **goravel-tooling** — Artisan commands, testing, logging (`facades.Log()`)
- **goravel-integrations** — gRPC, HTTP client, filesystem, mail, localization, package development

## Reference Files

| File | Contents |
|------|----------|
| [service-container.md](./service-container.md) | All binding methods, resolving, convenience resolvers |
| [service-providers.md](./service-providers.md) | Register/Boot lifecycle, Runners, Relationship, deferred providers |
| [facades.md](./facades.md) | Full facade list, how facades work, creating custom facades |
