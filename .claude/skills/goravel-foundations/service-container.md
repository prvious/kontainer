# Service Container Reference

The service container is Goravel's central registry for all bindings. All interactions happen through the `foundation.Application` interface (available as `app` in providers or via `facades.App()`).

## Binding Methods

### Bind (Transient)

Create a new instance on every `Make()` call:

```go
app.Bind(key, func(app foundation.Application) (any, error) {
    return NewService(app.MakeConfig()), nil
})
```

### Singleton

Resolve once, return the cached instance on subsequent calls:

```go
app.Singleton(key, func(app foundation.Application) (any, error) {
    return NewService(app.MakeConfig()), nil
})
```

### Instance

Bind a pre-constructed value directly:

```go
svc := NewService(cfg)
app.Instance(key, svc)
```

### BindWith (Parameterized)

Accept extra parameters at resolve time. Pair with `MakeWith()`:

```go
app.BindWith(key, func(app foundation.Application, params map[string]any) (any, error) {
    id := params["id"].(int)
    return NewService(id), nil
})
```

## Binding Keys

Use string constants for keys. Framework bindings use the `binding` package:

```go
import "github.com/goravel/framework/support/binding"

app.Singleton(binding.Route, func(app foundation.Application) (any, error) {
    return NewRoute(app.MakeConfig()), nil
})
```

For custom bindings, define a `Binding` constant in the package:

```go
package mypackage

const Binding = "mypackage"
```

## Resolving

### Make

Resolve a binding by key:

```go
instance, err := app.Make(key)
```

Cast the result to the expected type:

```go
raw, err := app.Make("mypackage")
svc := raw.(*MyService)
```

### MakeWith

Resolve a `BindWith` binding, passing parameters:

```go
instance, err := app.MakeWith(key, map[string]any{"id": 42})
```

### Outside a Provider

Use the `App` facade:

```go
instance, err := facades.App().Make(key)
```

## Convenience Resolvers

The `Application` interface provides typed resolver methods that return the correct interface directly (no type assertion needed):

| Method | Returns |
|--------|---------|
| `app.MakeArtisan()` | `console.Artisan` |
| `app.MakeAuth()` | `auth.Auth` |
| `app.MakeCache()` | `cache.Cache` |
| `app.MakeConfig()` | `config.Config` |
| `app.MakeCrypt()` | `crypt.Crypt` |
| `app.MakeEvent()` | `event.Instance` |
| `app.MakeGate()` | `auth.Gate` |
| `app.MakeGrpc()` | `grpc.Grpc` |
| `app.MakeHash()` | `hash.Hash` |
| `app.MakeLog()` | `log.Log` |
| `app.MakeMail()` | `mail.Mail` |
| `app.MakeOrm()` | `orm.Orm` |
| `app.MakeQueue()` | `queue.Queue` |
| `app.MakeRateLimiter()` | `http.RateLimiter` |
| `app.MakeRoute()` | `route.Route` |
| `app.MakeSchedule()` | `schedule.Schedule` |
| `app.MakeStorage()` | `filesystem.Storage` |
| `app.MakeValidation()` | `validation.Validation` |

These return `nil` if the corresponding facade is not installed (relevant for goravel-lite).

## Where to Bind

Always bind inside a service provider's `Register()` method:

```go
func (r *ServiceProvider) Register(app foundation.Application) {
    app.Singleton(Binding, func(app foundation.Application) (any, error) {
        return NewMyService(app.MakeConfig()), nil
    })
}
```

Never resolve other services inside `Register()` -- they may not be registered yet. Use `Boot()` for any code that needs to call `Make*` methods.

## Contextual Binding

Goravel v1.17 does NOT support contextual binding (binding different implementations based on which consumer receives the dependency). Unlike Laravel's `when()->needs()->give()` chain, there is no equivalent API on the Goravel container.

**Workaround:** Register distinct keys and resolve the correct one explicitly in each consumer's factory closure:

```go
app.Singleton("report.filesystem", func(app foundation.Application) (any, error) {
    return NewLocalFilesystem("/reports"), nil
})

app.Singleton("photo.filesystem", func(app foundation.Application) (any, error) {
    return NewS3Filesystem(app.MakeConfig()), nil
})

// In the consumer's binding, resolve the specific key:
app.Singleton("report.service", func(app foundation.Application) (any, error) {
    fs, _ := app.Make("report.filesystem")
    return NewReportService(fs.(Filesystem)), nil
})
```

## Tagging

Goravel v1.17 does NOT provide a `Tag()` / `Tagged()` API to group related bindings under a tag and resolve them as a set (as Laravel does).

**Workaround:** Maintain a slice of keys manually and resolve them in a loop:

```go
var reportKeys = []string{"report.csv", "report.pdf", "report.excel"}

// Register each
for _, key := range reportKeys {
    k := key // capture
    app.Singleton(k, func(app foundation.Application) (any, error) {
        return buildReport(k, app.MakeConfig()), nil
    })
}

// Resolve all tagged reports
func resolveReports(app foundation.Application) []Reporter {
    var reports []Reporter
    for _, key := range reportKeys {
        r, _ := app.Make(key)
        reports = append(reports, r.(Reporter))
    }
    return reports
}
```

## Method Injection

Goravel v1.17 does NOT support automatic method injection (auto-resolving method parameters from the container). Dependencies are resolved at the call site, not injected into method signatures.

In HTTP controllers, the `http.Context` is passed directly -- it is not resolved from the container. For other dependencies, resolve them in the provider's `Boot()` or at the point of use:

```go
func (r *ServiceProvider) Boot(app foundation.Application) {
    cfg := app.MakeConfig()
    cache := app.MakeCache()
    // Pass explicitly to wherever they're needed
}
```

## Binding Lifecycle Summary

| Method | New instance per resolve? | Accepts params? |
|--------|--------------------------|-----------------|
| `Bind` | Yes | No |
| `Singleton` | No (cached) | No |
| `Instance` | No (literal value) | No |
| `BindWith` | Yes | Yes (`map[string]any`) |
