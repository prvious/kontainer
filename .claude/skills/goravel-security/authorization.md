# Authorization Reference

## Gates

Gates are closures registered via `facades.Gate().Define()`. Register them in `bootstrap/app.go::WithCallback` so all facades are available.

### Defining Gates

```go
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
```

### Checking Authorization

```go
// Boolean checks
facades.Gate().WithContext(ctx).Allows("update-post", map[string]any{"post": post})
facades.Gate().WithContext(ctx).Denies("update-post", map[string]any{"post": post})

// Multiple abilities
facades.Gate().Any([]string{"update-post", "delete-post"}, map[string]any{"post": post})
facades.Gate().None([]string{"update-post", "delete-post"}, map[string]any{"post": post})
```

### Full Response (Inspect)

`Inspect` returns the authorization response with a message instead of a boolean:

```go
response := facades.Gate().Inspect("edit-settings", nil)
if response.Allowed() {
    // authorized
} else {
    fmt.Println(response.Message())
}
```

### Context Injection

Pass context to gate checks with `WithContext`. The `context.Context` is forwarded to `Before`, `After`, and `Define` callbacks:

```go
facades.Gate().WithContext(ctx).Allows("update-post", map[string]any{"post": post})
```

### Gate Interceptors

**Before** -- runs before all other checks. Non-nil return short-circuits:

```go
facades.Gate().Before(func(ctx context.Context, ability string, arguments map[string]any) contractsaccess.Response {
    user := ctx.Value("user").(models.User)
    if isAdministrator(user) {
        return access.NewAllowResponse()
    }
    return nil // continue to gate definition
})
```

**After** -- runs after all checks. Non-nil return overrides only when `Define` returned nil:

```go
facades.Gate().After(func(ctx context.Context, ability string, arguments map[string]any, result contractsaccess.Response) contractsaccess.Response {
    user := ctx.Value("user").(models.User)
    if isAdministrator(user) {
        return access.NewAllowResponse()
    }
    return nil
})
```

## Policies

Policies group authorization logic for a resource into a struct. Each method is a gate callback.

### Generate

```shell
./artisan make:policy PostPolicy
./artisan make:policy user/PostPolicy   # nested directory
```

### Writing a Policy

```go
package policies

import (
    "context"
    "goravel/app/models"
    "github.com/goravel/framework/auth/access"
    contractsaccess "github.com/goravel/framework/contracts/auth/access"
)

type PostPolicy struct{}

func NewPostPolicy() *PostPolicy {
    return &PostPolicy{}
}

func (r *PostPolicy) Update(ctx context.Context, arguments map[string]any) contractsaccess.Response {
    user := ctx.Value("user").(models.User)
    post := arguments["post"].(models.Post)
    if user.ID == post.UserID {
        return access.NewAllowResponse()
    }
    return access.NewDenyResponse("You do not own this post.")
}

func (r *PostPolicy) Delete(ctx context.Context, arguments map[string]any) contractsaccess.Response {
    // similar logic
}
```

### Registering Policies

Register each policy method as a gate in `bootstrap/app.go::WithCallback`:

```go
facades.Gate().Define("update-post", policies.NewPostPolicy().Update)
facades.Gate().Define("delete-post", policies.NewPostPolicy().Delete)
```

### Using Policies

Once registered, policies are invoked via the same gate API:

```go
facades.Gate().WithContext(ctx).Allows("update-post", map[string]any{"post": post})
```
