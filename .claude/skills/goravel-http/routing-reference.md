# Routing Reference

Full routing API for Goravel v1.17. For foundational concepts (service providers, facades), see the `goravel-foundations` skill.

## HTTP Drivers

Configure the HTTP driver in `config/http.go`. Goravel supports [gin](https://github.com/goravel/gin) (default) and [fiber](https://github.com/goravel/fiber).

## Route Registration

Define routes in files under `routes/`, register them in `bootstrap/app.go`:

```go
func Boot() contractsfoundation.Application {
    return foundation.Setup().
        WithRouting(func() {
            routes.Web()
            routes.Api()
        }).
        WithConfig(config.Boot).
        Create()
}
```

## HTTP Verb Methods

```go
facades.Route().Get("/path", handler)
facades.Route().Post("/path", handler)
facades.Route().Put("/path", handler)
facades.Route().Delete("/path", handler)
facades.Route().Patch("/path", handler)
facades.Route().Options("/path", handler)
facades.Route().Any("/path", handler)  // matches all HTTP methods
```

Handler signature: `func(ctx http.Context) http.Response`

## Route Parameters

Define parameters with `{name}` syntax. Read them via `ctx.Request().Input("name")` or typed variants:

```go
facades.Route().Get("/users/{id}", func(ctx http.Context) http.Response {
    id := ctx.Request().Input("id")
    return ctx.Response().Success().Json(http.Json{"id": id})
})
```

Typed route parameter accessors:

```go
id := ctx.Request().Route("id")        // string
id := ctx.Request().RouteInt("id")     // int
id := ctx.Request().RouteInt64("id")   // int64
```

## Named Routes

Assign a name to a route with `.Name()`, then retrieve route info by name:

```go
facades.Route().Get("/users", userController.Index).Name("users.index")

// Later, look up route info by name
info := facades.Route().Info("users.index")
```

## Route Groups

Group routes to share middleware, prefix, or both:

```go
facades.Route().Group(func(router route.Router) {
    router.Get("/users", userController.Index)
    router.Post("/users", userController.Store)
})
```

## Route Prefix

Apply a URL prefix to a route or group:

```go
facades.Route().Prefix("api/v1").Get("/users", userController.Index)
// Matches: /api/v1/users

facades.Route().Prefix("admin").Group(func(router route.Router) {
    router.Get("/dashboard", adminController.Dashboard)
    // Matches: /admin/dashboard
})
```

## Middleware on Routes

Attach middleware to individual routes or groups:

```go
facades.Route().Middleware(middleware.Auth()).Get("/profile", profileController.Show)

facades.Route().Prefix("api").Middleware(middleware.Auth(), middleware.Cors()).Group(func(router route.Router) {
    router.Get("/services", serviceController.Index)
})
```

## Resource Routes

Register all CRUD routes for a resource controller with a single call:

```go
facades.Route().Resource("/photos", controllers.NewPhotoController())
```

Generated routes:

| Verb      | URI               | Method  |
|-----------|-------------------|---------|
| GET       | `/photos`         | Index   |
| POST      | `/photos`         | Store   |
| GET       | `/photos/{photo}` | Show    |
| PUT/PATCH | `/photos/{photo}` | Update  |
| DELETE    | `/photos/{photo}` | Destroy |

The resource controller must implement these methods:

```go
type PhotoController struct{}

func NewPhotoController() *PhotoController { return &PhotoController{} }

func (c *PhotoController) Index(ctx http.Context) http.Response   { /* GET /photos */ }
func (c *PhotoController) Show(ctx http.Context) http.Response    { /* GET /photos/{photo} */ }
func (c *PhotoController) Store(ctx http.Context) http.Response   { /* POST /photos */ }
func (c *PhotoController) Update(ctx http.Context) http.Response  { /* PUT /photos/{photo} */ }
func (c *PhotoController) Destroy(ctx http.Context) http.Response { /* DELETE /photos/{photo} */ }
```

Generate a resource controller: `./artisan make:controller --resource PhotoController`

## Static File Routes

Serve static files and directories:

```go
facades.Route().Static("static", "./public")              // serve directory
facades.Route().StaticFile("logo", "./public/logo.png")   // serve single file
facades.Route().StaticFS("assets", http.Dir("./public"))  // serve via http.FileSystem
```

## Fallback Route

Handle unmatched requests:

```go
facades.Route().Fallback(func(ctx http.Context) http.Response {
    return ctx.Response().String(404, "not found")
})
```

## Rate Limiting

### Define Rate Limiters

Register limiters in `bootstrap/app.go` via `WithCallback`:

```go
func Boot() contractsfoundation.Application {
    return foundation.Setup().
        WithConfig(config.Boot).
        WithCallback(func() {
            facades.RateLimiter().For("global", func(ctx http.Context) http.Limit {
                return limit.PerMinute(1000)
            })
        }).
        Create()
}
```

Custom response on rate limit exceeded:

```go
facades.RateLimiter().For("api", func(ctx http.Context) http.Limit {
    return limit.PerMinute(100).Response(func(ctx http.Context) {
        ctx.Request().AbortWithStatus(http.StatusTooManyRequests)
    })
})
```

Segment limits by key (e.g., per-IP):

```go
facades.RateLimiter().For("api", func(ctx http.Context) http.Limit {
    return limit.PerMinute(100).By(ctx.Request().Ip())
})
```

Return `nil` to skip rate limiting for specific requests (e.g., VIP users).

### Multiple Rate Limits

Apply multiple limits with `ForWithLimits`:

```go
facades.RateLimiter().ForWithLimits("login", func(ctx http.Context) []http.Limit {
    return []http.Limit{
        limit.PerMinute(500),
        limit.PerMinute(100).By(ctx.Request().Ip()),
    }
})
```

### Attach to Routes

Use `middleware.Throttle("name")`:

```go
import "github.com/goravel/framework/http/middleware"

facades.Route().Middleware(middleware.Throttle("api")).Group(func(router route.Router) {
    router.Get("/data", dataController.Index)
})
```

## CORS

CORS is enabled by default. Configure in `config/cors.go`.

## Route Inspection

List all routes via CLI:

```bash
./artisan route:list
```

Programmatically retrieve all registered routes:

```go
routes := facades.Route().GetRoutes()
```

## Global Middleware

Register in `bootstrap/app.go` via `WithMiddleware`:

```go
func Boot() contractsfoundation.Application {
    return foundation.Setup().
        WithMiddleware(func(handler configuration.Middleware) {
            handler.Append(middleware.Cors(), middleware.Custom())
        }).
        WithConfig(config.Boot).
        Create()
}
```

`handler` methods:

| Method | Action |
|--------|--------|
| `Append(middlewares...)` | Add to end of middleware stack |
| `Prepend(middlewares...)` | Add to beginning of middleware stack |
| `Use(middlewares...)` | Replace entire middleware stack |
| `GetGlobalMiddleware()` | Get all registered global middleware |
| `Recover(fn)` | Set custom panic recovery handler |
| `GetRecover()` | Get current recovery handler |

## Custom Recovery

Set a panic recovery handler in `WithMiddleware`:

```go
handler.Append(middleware.Throttle("global")).Recover(func(ctx http.Context, err any) {
    facades.Log().Error(err)
    _ = ctx.Response().String(http.StatusInternalServerError, "internal error").Abort()
})
```

## Testing Routes

Use `ServeHTTP` to test routes without starting a server (available on the route facade).
