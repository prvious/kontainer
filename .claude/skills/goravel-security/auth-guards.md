# Auth Guards Reference

## Configuration

### config/auth.go

```go
"defaults": map[string]any{
    "guard": "user",
},
"guards": map[string]any{
    "user": map[string]any{
        "driver": "jwt",
        // Optional per-guard overrides (fall back to config/jwt.go):
        // "ttl":         60,
        // "refresh_ttl": 0,
        // "secret":      "guard-specific-secret",
    },
    "admin": map[string]any{
        "driver": "jwt",
        "ttl":    30,
        "secret": "admin-secret",
    },
},
"providers": map[string]any{
    "users": map[string]any{
        "driver": "orm",     // default ORM-based user provider
    },
},
```

### config/jwt.go

Default JWT parameters shared across guards unless overridden:
- `secret` -- signing key (generate with `./artisan jwt:secret`)
- `ttl` -- token lifetime in minutes
- `refresh_ttl` -- refresh window in minutes (0 = no refresh)

## JWT Methods

All methods operate on `facades.Auth(ctx)`. Chain `.Guard("name")` before any method to switch guards.

### Login (generate token from model)

```go
var user models.User
user.ID = 1
token, err := facades.Auth(ctx).Login(&user)
```

Model must have a primary key. Works automatically with `orm.Model`; otherwise tag the PK field with `gorm:"primaryKey"`.

### LoginUsingID (generate token from ID)

```go
token, err := facades.Auth(ctx).LoginUsingID(1)
```

### Parse (validate and decode token)

```go
payload, err := facades.Auth(ctx).Parse(token)
```

Returns `payload` with fields:
- `Guard` -- guard name
- `Key` -- user identifier
- `ExpireAt` -- expiration time
- `IssuedAt` -- issue time

Token accepts both raw and `Bearer `-prefixed formats.

If `err` is non-nil and is not `auth.ErrorTokenExpired`, `payload` is nil:

```go
import "github.com/goravel/framework/auth"

if errors.Is(err, auth.ErrorTokenExpired) {
    // Token expired but payload is still available
}
```

### User (retrieve authenticated user)

Call `Parse` first (typically in middleware), then:

```go
var user models.User
err := facades.Auth(ctx).User(&user)
```

### ID (get user identifier)

```go
id, err := facades.Auth(ctx).ID()
```

### Refresh (issue new token)

Call `Parse` first:

```go
newToken, err := facades.Auth(ctx).Refresh()
```

### Logout (invalidate token)

```go
err := facades.Auth(ctx).Logout()
```

## Multiple Guards

Always chain `.Guard("name")` when using a non-default guard:

```go
token, err := facades.Auth(ctx).Guard("admin").LoginUsingID(1)
facades.Auth(ctx).Guard("admin").Parse(token)
facades.Auth(ctx).Guard("admin").User(&user)
facades.Auth(ctx).Guard("admin").Refresh()
facades.Auth(ctx).Guard("admin").Logout()
```

## Middleware Integration

Typical auth middleware pattern -- parse the token and abort if invalid:

```go
func Auth() http.Middleware {
    return func(ctx http.Context) {
        token := ctx.Request().Header("Authorization", "")
        if token == "" {
            ctx.Response().String(http.StatusUnauthorized, "Unauthorized").Abort()
            return
        }
        _, err := facades.Auth(ctx).Parse(token)
        if err != nil {
            ctx.Response().String(http.StatusUnauthorized, "Invalid token").Abort()
            return
        }
        ctx.Request().Next()
    }
}
```

## Custom Guard Driver

Register in `AuthServiceProvider.Boot()`:

```go
import "github.com/goravel/framework/contracts/auth"

func (r *AuthServiceProvider) Boot(app foundation.Application) {
    facades.Auth().Extend("custom-driver", func(ctx http.Context, name string, userProvider auth.UserProvider) (auth.GuardDriver, error) {
        return &CustomGuard{}, nil
    })
}
```

Reference it in `config/auth.go`:

```go
"guards": map[string]any{
    "api": map[string]any{
        "driver":   "custom-driver",
        "provider": "users",
    },
},
```

## Custom User Provider

Register in `AuthServiceProvider.Boot()`:

```go
facades.Auth().Provider("custom-provider", func(ctx http.Context) (auth.UserProvider, error) {
    return &MyUserProvider{}, nil
})
```

Configure in `config/auth.go`:

```go
"providers": map[string]any{
    "users": map[string]any{
        "driver": "custom-provider",
    },
},
"guards": map[string]any{
    "api": map[string]any{
        "driver":   "jwt",
        "provider": "users",
    },
},
```
