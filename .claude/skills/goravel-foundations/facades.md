# Facades Reference

Facades are thin accessor functions in `app/facades/` that resolve services from the container via convenience `Make*` methods. They provide a clean, static-like API without sacrificing testability.

## How Facades Work

1. A service provider registers a binding in the container (e.g., `binding.Route`).
2. A facade function calls the corresponding `Make*` method on the app instance.
3. Application code calls the facade to get the resolved instance.

```go
// 1. Provider registers
func (r *ServiceProvider) Register(app foundation.Application) {
    app.Singleton(binding.Route, func(app foundation.Application) (any, error) {
        return NewRoute(app.MakeConfig()), nil
    })
}

// 2. Facade resolves
// app/facades/route.go
func Route() route.Route {
    return App().MakeRoute()
}

// 3. Application uses
facades.Route().Get("/ping", handler)
```

## Available Facades

| Facade | Type | Description |
|--------|------|-------------|
| `facades.App()` | `foundation.Application` | The application / service container itself |
| `facades.Artisan()` | `console.Artisan` | CLI command registration and execution |
| `facades.Auth()` | `auth.Auth` | Authentication and guards |
| `facades.Cache()` | `cache.Cache` | Cache store (Redis, memory, etc.) |
| `facades.Config()` | `config.Config` | Configuration access (dot-notation) |
| `facades.Crypt()` | `crypt.Crypt` | Encryption / decryption |
| `facades.Event()` | `event.Instance` | Event dispatching |
| `facades.Gate()` | `auth.Gate` | Authorization policies |
| `facades.Grpc()` | `grpc.Grpc` | gRPC server management |
| `facades.Hash()` | `hash.Hash` | Hashing (bcrypt, argon2) |
| `facades.Log()` | `log.Log` | Structured logging |
| `facades.Mail()` | `mail.Mail` | Email sending |
| `facades.Orm()` | `orm.Orm` | Database ORM |
| `facades.Queue()` | `queue.Queue` | Job queuing |
| `facades.RateLimiter()` | `http.RateLimiter` | Rate limiting definitions |
| `facades.Route()` | `route.Route` | HTTP router |
| `facades.Schedule()` | `schedule.Schedule` | Task scheduling |
| `facades.Storage()` | `filesystem.Storage` | File storage (local, S3, etc.) |
| `facades.Validation()` | `validation.Validation` | Request validation |

**goravel-lite note:** Only `Artisan` and `Config` are installed by default. Install others with `./artisan package:install <Name>`.

## Config Facade Quick Reference

The most commonly used facade. Full method set:

```go
// Read .env values
facades.Config().Env("APP_NAME", "goravel")

// Read config values (dot-notation into config/ files)
facades.Config().Get("app.name", "goravel")        // any
facades.Config().GetString("app.name", "goravel")   // string
facades.Config().GetInt("app.port", 8080)            // int
facades.Config().GetBool("app.debug", false)         // bool

// Set config values at runtime
facades.Config().Add("custom.key", "value")
facades.Config().Add("custom.nested", map[string]any{"foo": "bar"})
```

## Creating a Custom Facade

1. Register a binding in a service provider:

```go
const Binding = "analytics"

func (r *AnalyticsServiceProvider) Register(app foundation.Application) {
    app.Singleton(Binding, func(app foundation.Application) (any, error) {
        return NewAnalyticsService(app.MakeConfig()), nil
    })
}
```

2. Create the facade file in `app/facades/`:

```go
// app/facades/analytics.go
package facades

import "your/module/app/contracts"

func Analytics() contracts.Analytics {
    instance, _ := App().Make("analytics")
    return instance.(contracts.Analytics)
}
```

3. Use it anywhere after boot:

```go
facades.Analytics().Track("event_name", data)
```

## Overriding Built-in Facades

Add a file to `app/facades/` with the same function name. Your local facade takes precedence over the framework's default. This is useful for wrapping a framework service with custom logic.

## Installing / Uninstalling Facades

```bash
# Install a specific facade
./artisan package:install Route

# Install all available facades
./artisan package:install --all

# Install all facades with default drivers
./artisan package:install --all --default

# Uninstall a facade
./artisan package:uninstall Route
```

When selecting facades interactively, press `x` to select, then `Enter` to confirm (pressing Enter alone selects nothing).
