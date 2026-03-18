# Logging Reference

## Configuration

Configure in `config/logging.go`. Default channel is `stack`.

## Channel Drivers

| Driver | Description | Notes |
|--------|-------------|-------|
| `stack` | Forwards to multiple channels | Default. Combine channels for redundancy. |
| `single` | Single log file | Set `print: true` to also output to console. |
| `daily` | One file per day | Auto-rotates. Set `print: true` for console output. |
| `custom` | Custom driver | Implement `log.Logger` interface, reference via `"via"` key. |

## Log Levels

All methods available on `facades.Log()`:

```go
facades.Log().Debug(message)
facades.Log().Debugf(message, args...)
facades.Log().Info(message)
facades.Log().Infof(message, args...)
facades.Log().Warning(message)
facades.Log().Warningf(message, args...)
facades.Log().Error(message)
facades.Log().Errorf(message, args...)
facades.Log().Fatal(message)
facades.Log().Fatalf(message, args...)
facades.Log().Panic(message)
facades.Log().Panicf(message, args...)
```

`Fatal` calls `os.Exit(1)` after logging. `Panic` calls `panic()` after logging.

## Writing to Specific Channels

```go
// Single channel
facades.Log().Channel("single").Info("written to single file only")

// Multiple channels simultaneously
facades.Log().Stack([]string{"single", "slack"}).Info("written to both")
```

## Contextual Logging

Chain methods add structured metadata to log entries:

```go
facades.Log().
    User("john").
    Code("AUTH_FAIL").
    Hint("check token expiry").
    In("authentication").
    Owner("security-team").
    Tags([]string{"auth", "critical"}).
    With(map[string]any{"ip": "192.168.1.1", "attempt": 3}).
    WithTrace().
    Error("login failed")
```

| Method | Purpose |
|--------|---------|
| `User(string)` | Associate a user with the entry |
| `Code(string)` | Set a code/slug describing the log |
| `Hint(string)` | Add a debugging hint |
| `In(string)` | Set the feature/domain category |
| `Owner(string)` | Set the responsible team (for alerting) |
| `Tags([]string)` | Add descriptive tags |
| `With(map[string]any)` | Add arbitrary key-value context |
| `WithTrace()` | Include stack trace information |
| `Request(http.Request)` | Attach HTTP request |
| `Response(http.Response)` | Attach HTTP response |

## Context Injection

Pass a Go context for correlation:

```go
facades.Log().WithContext(ctx).Info("request processed")
```

## Custom Channel Driver

Define a custom driver by specifying `"driver": "custom"` and `"via"` in config:

```go
// config/logging.go
"custom_channel": map[string]interface{}{
    "driver": "custom",
    "via":    &CustomLogger{},
},
```

Implement `framework/contracts/log.Logger`:

```go
type Logger interface {
    Handle(channel string) (Handler, error)
}
```

The `channel` argument receives the config path. Return a `Handler` that processes log entries.

Reference implementations: [Daily driver](https://github.com/goravel/framework/blob/master/log/logger/daily.go), [Single driver](https://github.com/goravel/framework/blob/master/log/logger/single.go).
