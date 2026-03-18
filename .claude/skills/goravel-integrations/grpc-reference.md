# gRPC Reference

Complete reference for Goravel gRPC server, client, interceptors, stats handlers, and metadata. Goravel v1.17.

## Configuration

`config/grpc.go` defines the server host/port and client connection targets:

```go
config.Add("grpc", map[string]any{
    "host": config.Env("GRPC_HOST", ""),
    "port": config.Env("GRPC_PORT", ""),
    "servers": map[string]any{
        "user": map[string]any{
            "host":           config.Env("GRPC_USER_HOST", ""),
            "port":           config.Env("GRPC_USER_PORT", ""),
            "interceptors":   []string{"default"},
            "stats_handlers": []string{},
        },
    },
})
```

- `host`/`port`: this agent's gRPC listener
- `servers`: named remote gRPC services the client connects to
- `interceptors`: list of interceptor group names to apply
- `stats_handlers`: list of stats handler group names to apply

## Protobuf Compilation

Define `.proto` files, compile with `protoc`:

```bash
protoc --go_out=. --go-grpc_out=. proto/agent.proto
```

Generated code provides `Register*Server` (server-side) and `New*Client` (client-side) functions.

## Server Setup

### Controllers

Place in `app/grpc/controllers/`. Each controller implements generated service interfaces:

```go
package controllers

import (
    "context"
    "net/http"
    proto "github.com/goravel/example-proto"
)

type UserController struct{}

func NewUserController() *UserController { return &UserController{} }

func (r *UserController) GetUser(ctx context.Context, req *proto.UserRequest) (*proto.UserResponse, error) {
    return &proto.UserResponse{
        Code: http.StatusOK,
        Data: &proto.User{Id: 1, Name: "Goravel", Token: req.GetToken()},
    }, nil
}
```

### Route Registration

Register in `routes/grpc.go`, then wire in `bootstrap/app.go`:

```go
// routes/grpc.go
func Grpc() {
    proto.RegisterUserServiceServer(facades.Grpc().Server(), controllers.NewUserController())
}

// bootstrap/app.go
foundation.Setup().
    WithRouting(func() {
        routes.Grpc()
    }).Create()
```

## Interceptors

### Unary Server Interceptor

```go
// app/grpc/interceptors/auth.go
func Auth(ctx context.Context, req any, info *grpc.UnaryServerInfo, handler grpc.UnaryHandler) (any, error) {
    // Pre-processing: validate auth, log, etc.
    if err := validateToken(ctx); err != nil {
        return nil, status.Error(codes.Unauthenticated, "invalid token")
    }
    resp, err := handler(ctx, req)
    // Post-processing: metrics, logging, etc.
    return resp, err
}
```

### Unary Client Interceptor

```go
func AddMetadata(ctx context.Context, method string, req, reply any, cc *grpc.ClientConn, invoker grpc.UnaryInvoker, opts ...grpc.CallOption) error {
    ctx = metadata.AppendToOutgoingContext(ctx, "x-request-id", uuid.New().String())
    return invoker(ctx, method, req, reply, cc, opts...)
}
```

### Registration

Register in `bootstrap/app.go`:

```go
foundation.Setup().
    WithGrpcServerInterceptors(func() []grpc.UnaryServerInterceptor {
        return []grpc.UnaryServerInterceptor{
            interceptors.Auth,
            interceptors.Logger,
        }
    }).
    WithGrpcClientInterceptors(func() map[string][]grpc.UnaryClientInterceptor {
        return map[string][]grpc.UnaryClientInterceptor{
            "default": {interceptors.AddMetadata},
            "secure":  {interceptors.AddSecret},
        }
    }).Create()
```

Client interceptor map keys are group names. Reference them in `config/grpc.go` under `servers.*.interceptors`:

```go
"user": map[string]any{
    "host":         "localhost",
    "port":         "9001",
    "interceptors": []string{"default", "secure"}, // applies both groups
},
```

### Stream Interceptors

Goravel does not provide framework-level stream interceptor registration. For streaming RPCs, validate manually inside the handler:

```go
func (r *Controller) StreamLogs(req *pb.LogRequest, stream pb.Service_StreamLogsServer) error {
    if err := interceptors.ValidateSecret(stream.Context()); err != nil {
        return err
    }
    // proceed with streaming
}
```

## Stats Handlers

Implement `google.golang.org/grpc/stats.Handler`:

```go
type MetricsHandler struct{}

func (h *MetricsHandler) TagRPC(ctx context.Context, info *stats.RPCTagInfo) context.Context { return ctx }
func (h *MetricsHandler) HandleRPC(ctx context.Context, s stats.RPCStats) {
    switch s.(type) {
    case *stats.End:
        // record latency, status
    }
}
func (h *MetricsHandler) TagConn(ctx context.Context, info *stats.ConnTagInfo) context.Context { return ctx }
func (h *MetricsHandler) HandleConn(ctx context.Context, s stats.ConnStats) {}
```

Register:

```go
foundation.Setup().
    WithGrpcServerStatsHandlers(func() []stats.Handler {
        return []stats.Handler{&MetricsHandler{}}
    }).
    WithGrpcClientStatsHandlers(func() map[string][]stats.Handler {
        return map[string][]stats.Handler{
            "user": {&MetricsHandler{}},
        }
    }).Create()
```

Client stats handler map keys match `servers.*.stats_handlers` in config.

## Client Connections

Connect to a named server from `config/grpc.go`:

```go
conn, err := facades.Grpc().Connect("user")
if err != nil {
    facades.Log().Errorf("gRPC connect failed: %v", err)
}
client := proto.NewUserServiceClient(conn)

resp, err := client.GetUser(ctx, &proto.UserRequest{Token: "abc"})
```

The connection name (`"user"`) must match a key under `grpc.servers` in config. Interceptors and stats handlers configured for that server are applied automatically.

## Metadata

Read incoming metadata (server-side):

```go
md, ok := metadata.FromIncomingContext(ctx)
values := md.Get("x-custom-header")
```

Attach outgoing metadata (client-side):

```go
ctx = metadata.AppendToOutgoingContext(ctx, "x-request-id", "123")
// or via client interceptor
```

## Error Handling

Use `google.golang.org/grpc/status` and `google.golang.org/grpc/codes`:

```go
import (
    "google.golang.org/grpc/codes"
    "google.golang.org/grpc/status"
)

// Return typed errors
return nil, status.Error(codes.NotFound, "user not found")
return nil, status.Errorf(codes.Internal, "db error: %v", err)

// Check error codes on client
st, ok := status.FromError(err)
if ok && st.Code() == codes.NotFound {
    // handle not found
}
```

Common codes: `OK`, `Canceled`, `InvalidArgument`, `NotFound`, `AlreadyExists`, `PermissionDenied`, `Unauthenticated`, `ResourceExhausted`, `Internal`, `Unavailable`, `DeadlineExceeded`.
