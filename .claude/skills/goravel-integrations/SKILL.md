---
name: goravel-integrations
description: Use when working with Goravel gRPC, HTTP client, filesystem, mail, or localization
---

# Goravel Integrations

Goravel's integration layer covers outbound communication (gRPC, HTTP client), storage (filesystem), email delivery, localization, and package development. These are the building blocks for connecting to external systems and structuring reusable code.

**Version:** All patterns target Goravel v1.17.

## gRPC Server

Define controllers in `app/grpc/controllers/`, register via generated protobuf `Register*Server` functions in `routes/grpc.go`:

```go
// routes/grpc.go
proto.RegisterUserServiceServer(facades.Grpc().Server(), controllers.NewUserController())
```

Register gRPC routing in `bootstrap/app.go` via `WithRouting`:

```go
foundation.Setup().
    WithRouting(func() {
        routes.Grpc()
    }).Create()
```

### Interceptors

Unary server interceptor signature: `func(ctx context.Context, req any, info *grpc.UnaryServerInfo, handler grpc.UnaryHandler) (any, error)`.

Unary client interceptor signature: `func(ctx context.Context, method string, req, reply any, cc *grpc.ClientConn, invoker grpc.UnaryInvoker, opts ...grpc.CallOption) error`.

Register in `bootstrap/app.go`:

```go
foundation.Setup().
    WithGrpcServerInterceptors(func() []grpc.UnaryServerInterceptor {
        return []grpc.UnaryServerInterceptor{interceptors.SecretAuth}
    }).
    WithGrpcClientInterceptors(func() map[string][]grpc.UnaryClientInterceptor {
        return map[string][]grpc.UnaryClientInterceptor{
            "default": {interceptors.TestClient},
        }
    }).Create()
```

Client interceptor groups (e.g., `"default"`) map to `grpc.servers.*.interceptors` in `config/grpc.go`.

### Stats Handlers

Implement `google.golang.org/grpc/stats.Handler` (4 methods: `TagRPC`, `HandleRPC`, `TagConn`, `HandleConn`). Register via `WithGrpcServerStatsHandlers` / `WithGrpcClientStatsHandlers`.

### gRPC Client

Connect to a configured server and create a typed client:

```go
conn, err := facades.Grpc().Connect("user")
client := proto.NewUserServiceClient(conn)
resp, err := client.GetUser(ctx, &proto.UserRequest{Token: token})
```

Server names (e.g., `"user"`) match keys in `config/grpc.go` `servers` map.

See [grpc-reference.md](./grpc-reference.md) for protobuf setup, interceptor details, stats handlers, metadata, and error handling.

## HTTP Client

Make outbound HTTP requests via `facades.Http()`:

```go
resp, err := facades.Http().
    WithToken("bearer_token").
    WithHeader("Accept", "application/json").
    Get("https://api.example.com/users")

if resp.Successful() {
    var user User
    resp.Bind(&user)
}
```

All verbs: `Get`, `Post`, `Put`, `Patch`, `Delete`, `Head`, `Options`. POST/PUT/PATCH/DELETE accept `io.Reader` as second arg.

Build request bodies with `http.NewBody()`:

```go
import "github.com/goravel/framework/support/http"

body, _ := http.NewBody().SetField("name", "value").Build()
resp, _ := facades.Http().WithHeader("Content-Type", body.ContentType()).Post(url, body.Reader())
```

Response interface: `Status()`, `Successful()`, `Failed()`, `ClientError()`, `ServerError()`, `Bind(&v)`, `Body()`, `Json()`, `Header()`, `Cookies()`.

Testing: `facades.Http().Fake(map[string]any{...})` stubs responses. `AssertSent()` verifies requests. Call `Reset()` in test cleanup.

See [http-client.md](./http-client.md) for full request builder API, auth, cookies, URI templates, testing/faking.

## Filesystem

Operate via `facades.Storage()`. Default disk configured in `config/filesystems.go`.

```go
facades.Storage().Put("file.txt", "contents")
content, _ := facades.Storage().Get("file.txt")
exists := facades.Storage().Exists("file.txt")
facades.Storage().Delete("file.txt")
facades.Storage().Copy("old.txt", "new.txt")
facades.Storage().Move("old.txt", "new.txt")
```

Switch disks: `facades.Storage().Disk("s3").Put(...)`.

Drivers: `local` (built-in), `s3` (`github.com/goravel/s3`), `oss`, `cos`, `minio`. Custom drivers implement the `filesystem.Driver` interface.

File uploads: `ctx.Request().File("avatar").Store("avatars")`.

See [filesystem-reference.md](./filesystem-reference.md) for directories, metadata, temporary URLs, uploads, custom drivers.

## Mail

Send email via `facades.Mail()`:

```go
import "github.com/goravel/framework/mail"

err := facades.Mail().
    To([]string{"user@example.com"}).
    Subject("Welcome").
    Content(mail.Html("<h1>Hello</h1>")).
    Send()
```

Queue mail: replace `.Send()` with `.Queue()`. Customize queue: `.Queue(mail.Queue().Connection("redis").Queue("mail"))`.

Mailable structs encapsulate email config. Generate: `./artisan make:mail OrderShipped`. Send: `facades.Mail().Send(mails.NewOrderShipped())`.

Template support via `html/template` engine. Configure in `config/mail.go` template section.

See [mail-reference.md](./mail-reference.md) for Cc/Bcc, attachments, custom sender, mailable struct, templates, custom drivers.

## Localization

Translation files in `lang/` directory (flat: `en.json`, or nested: `en/user.json`).

```go
// Set locale per request
facades.App().SetLocale(ctx, "en")

// Retrieve translation
facades.Lang(ctx).Get("welcome")                     // simple key
facades.Lang(ctx).Get("role/user.required.user_id")   // nested file + key

// With replacements — placeholders use :prefix
facades.Lang(ctx).Get("welcome", translation.Option{
    Replace: map[string]string{"name": "Goravel"},
})

// Pluralization
facades.Lang(ctx).Choice("messages.apples", 10)
```

Plural rules: `"one|many"` or range syntax `"{0} none|[1,19] some|[20,*] many"`.

Embed loading: create `lang/fs.go` with `//go:embed *`, set `lang_fs` in `config/app.go`.

## Package Development

Create a package skeleton: `./artisan make:package sms`.

Packages are structured around a `ServiceProvider` with `Register()` and `Boot()` methods. Publish resources (config, migrations, models, assets) via `app.Publishes()` in `Boot()`:

```go
func (r *ServiceProvider) Boot(app foundation.Application) {
    app.Publishes("github.com/goravel/example-package", map[string]string{
        "config/sms.go": app.ConfigPath("sms.go"),
    })
}
```

Install packages: `./artisan package:install github.com/goravel/example-package`. Publish resources: `./artisan vendor:publish --package=<name>`.

Package `setup/setup.go` defines install/uninstall logic using `packages.Setup()`, `modify.RegisterProvider()`, `modify.File()`.

## Related Skills

- See **goravel-foundations** skill for service providers, `bootstrap/app.go` lifecycle, and `WithRouting`/`WithGrpc*` registration
- See **goravel-http** skill for inbound HTTP routing and middleware (distinct from the outbound HTTP client here)
- See **goravel-async** skill for queued mail delivery (`facades.Mail().Queue()`) and cache-based features
- See **goravel-tooling** skill for mocking `facades.Storage()`, `facades.Mail()`, and `facades.Http()` in tests

## Reference Files

| File | Contents |
|------|----------|
| [grpc-reference.md](./grpc-reference.md) | Protobuf setup, server/client, interceptors (unary/stream), stats handlers, metadata, error handling |
| [http-client.md](./http-client.md) | All HTTP verbs, headers, auth, query params, request body, cookies, URI templates, response API, testing/faking |
| [filesystem-reference.md](./filesystem-reference.md) | Drivers, CRUD, directories, metadata, temporary URLs, uploads, custom drivers |
| [mail-reference.md](./mail-reference.md) | Configuration, sending, Cc/Bcc, attachments, mailable structs, templates, queued mail, custom drivers |
