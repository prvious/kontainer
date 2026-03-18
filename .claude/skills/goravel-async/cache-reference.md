# Cache Reference

## Drivers

| Driver | Notes |
|--------|-------|
| `memory` | Built-in, default. In-process only, lost on restart. |
| `redis` | Via `github.com/goravel/redis` package |
| `custom` | Implement `cache.Driver` interface, register with `"via"` key |

Configure in `config/cache.go`. Switch stores at runtime with `facades.Cache().Store("redis")`.

## Retrieving

```go
value := facades.Cache().Get("key", "default")          // returns any
value := facades.Cache().GetBool("key", true)
value := facades.Cache().GetInt("key", 0)
value := facades.Cache().GetString("key", "")

// Default via closure (lazy evaluation)
value := facades.Cache().Get("key", func() any {
    return expensiveComputation()
})
```

## Existence Check

```go
exists := facades.Cache().Has("key") // bool
```

## Storing

```go
// With TTL
err := facades.Cache().Put("key", "value", 5*time.Second)

// Forever (duration 0 also means forever)
err := facades.Cache().Put("key", "value", 0)
ok := facades.Cache().Forever("key", "value")

// Only if key absent
ok := facades.Cache().Add("key", "value", 5*time.Second) // true if stored
```

## Retrieve & Store

```go
// Cache miss triggers closure, stores result with TTL
value, err := facades.Cache().Remember("key", 5*time.Second, func() (any, error) {
    return db.FetchExpensiveData()
})

// Same but stores forever
value, err := facades.Cache().RememberForever("key", func() (any, error) {
    return "computed", nil
})
```

## Retrieve & Delete

```go
value := facades.Cache().Pull("key", "default") // get then delete
```

## Removing

```go
ok := facades.Cache().Forget("key")  // single key
ok := facades.Cache().Flush()        // all keys
```

## Increment / Decrement

```go
newVal, err := facades.Cache().Increment("counter")       // +1
newVal, err := facades.Cache().Increment("counter", 5)    // +5
newVal, err := facades.Cache().Decrement("counter")       // -1
newVal, err := facades.Cache().Decrement("counter", 5)    // -5
```

## Multiple Stores

```go
value := facades.Cache().Store("redis").Get("key")
facades.Cache().Store("memory").Put("key", "value", time.Minute)
```

## Context Injection

```go
facades.Cache().WithContext(ctx).Get("key")
```

## Atomic Locks

Create distributed locks for coordination across processes/servers:

```go
lock := facades.Cache().Lock("resource", 10*time.Second)

// Try acquire
if lock.Get() {
    defer lock.Release()
    // exclusive work
}

// Acquire with closure (auto-release)
facades.Cache().Lock("resource").Get(func() {
    // auto-released when done
})

// Block until available (max wait time)
lock := facades.Cache().Lock("resource", 10*time.Second)
if lock.Block(5 * time.Second) {
    defer lock.Release()
}

// Block with closure (auto-release)
facades.Cache().Lock("resource", 10*time.Second).Block(5*time.Second, func() {
    // acquired and auto-released
})

// Force release (ignores ownership)
facades.Cache().Lock("resource").ForceRelease()
```

## Custom Driver

Implement the `cache.Driver` interface:

```go
type Driver interface {
    Add(key string, value any, t time.Duration) bool
    Decrement(key string, value ...int) (int, error)
    Forever(key string, value any) bool
    Forget(key string) bool
    Flush() bool
    Get(key string, def ...any) any
    GetBool(key string, def ...bool) bool
    GetInt(key string, def ...int) int
    GetInt64(key string, def ...int64) int64
    GetString(key string, def ...string) string
    Has(key string) bool
    Increment(key string, value ...int) (int, error)
    Lock(key string, t ...time.Duration) Lock
    Put(key string, value any, t time.Duration) error
    Pull(key string, def ...any) any
    Remember(key string, ttl time.Duration, callback func() (any, error)) (any, error)
    RememberForever(key string, callback func() (any, error)) (any, error)
    WithContext(ctx context.Context) Driver
}
```

Register in `config/cache.go`:

```go
"stores": map[string]any{
    "custom": map[string]any{
        "driver": "custom",
        "via":    &MyCustomDriver{},
    },
},
```
