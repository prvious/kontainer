---
name: goravel-http
description: Use when building HTTP endpoints, routes, controllers, middleware, request handling, response building, or validation in Goravel
---

# Goravel HTTP

Goravel's HTTP layer handles routing, request parsing, response building, middleware, and validation. Operate it through `facades.Route()` for routing, `ctx.Request()` for input, `ctx.Response()` for output, and `ctx.Request().Validate()` for validation.

**Version:** All patterns target Goravel v1.17.

## Routing Basics

Define routes in `routes/` directory files, registered in `bootstrap/app.go` via `WithRouting`:

```go
func Boot() contractsfoundation.Application {
    return foundation.Setup().
        WithRouting(func() {
            routes.Api()
        }).
        WithConfig(config.Boot).
        Create()
}
```

Register routes with HTTP verb methods on `facades.Route()`:

```go
facades.Route().Get("/users", userController.Index)
facades.Route().Post("/users", userController.Store)
facades.Route().Put("/users/{id}", userController.Update)
facades.Route().Delete("/users/{id}", userController.Destroy)
facades.Route().Any("/wildcard", handler) // all methods
```

Group routes with shared prefix and middleware:

```go
facades.Route().Prefix("api").Middleware(middleware.Auth()).Group(func(router route.Router) {
    router.Get("/services", serviceController.Index)
    router.Post("/services", serviceController.Store)
})
```

Resource routes register all CRUD routes for a controller:

```go
facades.Route().Resource("/photos", controllers.NewPhotoController())
// Generates: Index (GET /), Show (GET /{id}), Store (POST /), Update (PUT /{id}), Destroy (DELETE /{id})
```

See [routing-reference.md](./routing-reference.md) for full routing API.

## Controllers

Store controllers in `app/http/controllers/`. Each controller is a struct with handler methods that receive `http.Context` and return `http.Response`:

```go
type ServiceController struct{}

func NewServiceController() *ServiceController {
    return &ServiceController{}
}

func (c *ServiceController) Index(ctx http.Context) http.Response {
    return ctx.Response().Success().Json(http.Json{"services": services})
}
```

Generate with `./artisan make:controller ServiceController` or `./artisan make:controller --resource PhotoController` for CRUD scaffolding.

## Middleware

Define middleware as functions returning `http.Middleware`:

```go
func Auth() http.Middleware {
    return func(ctx http.Context) {
        token := ctx.Request().Header("Authorization", "")
        if token == "" {
            ctx.Response().String(http.StatusUnauthorized, "Unauthorized").Abort()
            return
        }
        ctx.Request().Next()
    }
}
```

- **Route-level:** `facades.Route().Middleware(middleware.Auth()).Get("/users", handler)`
- **Global:** Register in `bootstrap/app.go` via `WithMiddleware` using `handler.Append(...)`, `handler.Prepend(...)`, or `handler.Use(...)`
- **Abort:** Call `ctx.Request().Abort()` or chain `.Abort()` on a response to stop the request pipeline

## Request Handling

Read input from `ctx.Request()`:

```go
id := ctx.Request().Input("id")              // JSON or form body
name := ctx.Request().Query("name", "default") // query string
routeID := ctx.Request().Route("id")          // route parameter {id}
header := ctx.Request().Header("X-Custom", "")
ip := ctx.Request().Ip()
all := ctx.Request().All()                    // merged json + form + query
```

Bind structured data:

```go
var req CreateServiceRequest
err := ctx.Request().Bind(&req)        // JSON or form body
err := ctx.Request().BindQuery(&req)   // query params only
```

See [request-response.md](./request-response.md) for the complete request and response API.

## Response Building

Build responses from `ctx.Response()`:

```go
ctx.Response().Json(http.StatusOK, http.Json{"key": "value"})
ctx.Response().String(http.StatusOK, "text")
ctx.Response().Success().Json(http.Json{"ok": true})       // shorthand for 200
ctx.Response().Status(http.StatusCreated).Json(data)       // custom status
ctx.Response().Data(200, "text/html", []byte("<h1>Hi</h1>"))
ctx.Response().Redirect(http.StatusFound, "/login")
ctx.Response().File("./public/logo.png")
ctx.Response().Download("./public/report.pdf", "report.pdf")
ctx.Response().NoContent()
```

Stream responses:

```go
ctx.Response().Stream(http.StatusOK, func(w http.StreamWriter) error {
    w.Write([]byte("chunk"))
    w.Flush()
    return nil
})
```

## Validation

Validate inline with rules strings:

```go
validator, err := ctx.Request().Validate(map[string]string{
    "name":  "required|max_len:255",
    "email": "required|email",
    "role":  "required|in:admin,user",
})
if validator.Fails() {
    return ctx.Response().Json(422, http.Json{"errors": validator.Errors().All()})
}
```

Use form request objects for reusable validation:

```go
var req requests.StorePostRequest
errors, err := ctx.Request().ValidateRequest(&req)
// req fields are auto-populated on success
```

Error inspection: `validator.Errors().One("field")`, `.Get("field")`, `.All()`, `.Has("field")`.

See [validation-rules.md](./validation-rules.md) for all built-in rules, custom rules, filters, and form request patterns.

## Session

Access via `ctx.Request().Session()` after registering `middleware.StartSession()`:

```go
ctx.Request().Session().Put("key", "value")
value := ctx.Request().Session().Get("key", "default")
ctx.Request().Session().Forget("key")
ctx.Request().Session().Flash("status", "Success!")
```

## Cookies

```go
// Read
value := ctx.Request().Cookie("name", "default")

// Write
ctx.Response().Cookie(http.Cookie{
    Name: "token", Value: "abc", Path: "/",
    Secure: true, HttpOnly: true,
    Expires: time.Now().Add(24 * time.Hour),
})

// Delete
ctx.Response().WithoutCookie("token")
```

## Rate Limiting

Define limiters in `bootstrap/app.go` via `WithCallback`, attach with `middleware.Throttle("name")`:

```go
facades.RateLimiter().For("api", func(ctx http.Context) http.Limit {
    return limit.PerMinute(100).By(ctx.Request().Ip())
})

facades.Route().Middleware(middleware.Throttle("api")).Group(func(router route.Router) {
    // rate-limited routes
})
```

## Related Skills

- See **goravel-foundations** skill for service providers, the service container, `bootstrap/app.go` lifecycle, and `With*` registration methods
- See **goravel-security** skill for authentication guards (`facades.Auth()`), authorization gates, and the auth middleware pattern
- See **goravel-data** skill for ORM models and database queries used in controllers

## Reference Files

| File | Contents |
|------|----------|
| [routing-reference.md](./routing-reference.md) | Route methods, parameters, named routes, resource routes, groups, rate limiting, fallback, CORS |
| [validation-rules.md](./validation-rules.md) | All built-in rules, custom rules, form requests, error messages, filters |
| [request-response.md](./request-response.md) | Full request API (input, query, file, header, cookie, IP), full response API (JSON, string, file, stream, redirect), sessions |
