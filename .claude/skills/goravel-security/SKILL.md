---
name: goravel-security
description: Use when implementing authentication, authorization, encryption, or hashing in Goravel
---

# Goravel Security

Goravel provides authentication via JWT guards, authorization via gates and policies, AES-256-GCM encryption, and Bcrypt/Argon2id password hashing. Access these through `facades.Auth()`, `facades.Gate()`, `facades.Crypt()`, and `facades.Hash()` respectively.

**Version:** All patterns target Goravel v1.17.

## Authentication (JWT Guards)

Configure guards in `config/auth.go` and JWT parameters in `config/jwt.go`. Generate the JWT secret with `./artisan jwt:secret`.

```go
// config/auth.go
"defaults": map[string]any{
    "guard": "user",
},
"guards": map[string]any{
    "user": map[string]any{
        "driver": "jwt",
    },
    "admin": map[string]any{
        "driver": "jwt",
        "ttl":    30,           // override per-guard
        "secret": "admin-secret",
    },
},
```

### Token Lifecycle

```go
// Generate token from model (model must have primary key)
token, err := facades.Auth(ctx).Login(&user)

// Generate token from ID directly
token, err := facades.Auth(ctx).LoginUsingID(1)

// Parse token (works with or without "Bearer " prefix)
payload, err := facades.Auth(ctx).Parse(token)
// payload.Guard, payload.Key, payload.ExpireAt, payload.IssuedAt

// Check expiration
errors.Is(err, auth.ErrorTokenExpired)

// Get authenticated user (call Parse first, typically in middleware)
var user models.User
err := facades.Auth(ctx).User(&user)
id, err := facades.Auth(ctx).ID()

// Refresh token (call Parse first)
newToken, err := facades.Auth(ctx).Refresh()

// Invalidate token
err := facades.Auth(ctx).Logout()
```

### Multiple Guards

Chain `.Guard("name")` before any auth method when not using the default guard:

```go
token, err := facades.Auth(ctx).Guard("admin").LoginUsingID(1)
err := facades.Auth(ctx).Guard("admin").Parse(token)
err := facades.Auth(ctx).Guard("admin").User(&user)
```

See [auth-guards.md](./auth-guards.md) for custom guards, custom user providers, and per-guard configuration.

## Authorization (Gates & Policies)

Define gates in `bootstrap/app.go::WithCallback`. Gates are closures; policies are structs with methods registered as gates.

### Gates

```go
// Define a gate
facades.Gate().Define("update-post",
    func(ctx context.Context, arguments map[string]any) contractsaccess.Response {
        user := ctx.Value("user").(models.User)
        post := arguments["post"].(models.Post)
        if user.ID == post.UserID {
            return access.NewAllowResponse()
        }
        return access.NewDenyResponse("forbidden")
    },
)

// Check authorization
facades.Gate().WithContext(ctx).Allows("update-post", map[string]any{"post": post})
facades.Gate().WithContext(ctx).Denies("update-post", map[string]any{"post": post})

// Multiple abilities
facades.Gate().Any([]string{"update-post", "delete-post"}, args)
facades.Gate().None([]string{"update-post", "delete-post"}, args)

// Get full response with message
response := facades.Gate().Inspect("update-post", args)
response.Allowed()
response.Message()
```

### Policies

```go
// app/policies/post_policy.go
type PostPolicy struct{}

func (r *PostPolicy) Update(ctx context.Context, arguments map[string]any) contractsaccess.Response {
    user := ctx.Value("user").(models.User)
    post := arguments["post"].(models.Post)
    if user.ID == post.UserID {
        return access.NewAllowResponse()
    }
    return access.NewDenyResponse("You do not own this post.")
}

// Register in bootstrap/app.go WithCallback:
facades.Gate().Define("update-post", policies.NewPostPolicy().Update)
```

See [authorization.md](./authorization.md) for gate interceptors (Before/After), context injection, and policy generation.

## Encryption

Configure `APP_KEY` in `.env`. Generate with `./artisan key:generate`. Uses AES-256-GCM with GMAC.

```go
secret, err := facades.Crypt().EncryptString("sensitive data")
plaintext, err := facades.Crypt().DecryptString(secret)
```

## Hashing

Configure the default driver in `config/hashing.go`. Supports Argon2id (default) and Bcrypt.

```go
hashed, err := facades.Hash().Make(password)
match := facades.Hash().Check("plain-text", hashed)
if facades.Hash().NeedsRehash(hashed) {
    hashed, _ = facades.Hash().Make("plain-text")
}
```

See [encryption-hashing.md](./encryption-hashing.md) for configuration details and driver options.

## Related Skills

- See **goravel-http** skill for middleware patterns, route-level middleware, and request/response handling
- See **goravel-foundations** skill for `bootstrap/app.go` lifecycle and `WithCallback` where gates are registered
- See **goravel-data** skill for ORM models used as auth user models

## Reference Files

| File | Contents |
|------|----------|
| [auth-guards.md](./auth-guards.md) | Guard configuration, JWT methods, custom guards, custom user providers, multi-auth, token refresh flows |
| [authorization.md](./authorization.md) | Gates (Define, Allows, Denies, Any, None, Inspect), policies, gate interceptors (Before/After), context injection |
| [encryption-hashing.md](./encryption-hashing.md) | Encrypter facade (EncryptString, DecryptString), Hash facade (Make, Check, NeedsRehash), hashing drivers |
