# Queues Reference

## Job Interface

A job implements two required methods:

```go
type ProcessPodcast struct{}

func (r *ProcessPodcast) Signature() string { return "process_podcast" }
func (r *ProcessPodcast) Handle(args ...any) error { return nil }
```

Optional retry control:

```go
func (r *ProcessPodcast) ShouldRetry(err error, attempt int) (retryable bool, delay time.Duration) {
    return attempt < 3, 10 * time.Second
}
```

Generate: `./artisan make:job ProcessPodcast` or `./artisan make:job user/ProcessPodcast`.

## Registration

Jobs auto-register when generated. Manual registration in `bootstrap/app.go`:

```go
foundation.Setup().
    WithJobs(Jobs).  // Jobs returns []queue.Job
    WithConfig(config.Boot).
    Create()
```

`bootstrap/jobs.go` defines the `Jobs()` function returning all job structs.

## Dispatching

```go
// Basic async dispatch
facades.Queue().Job(&jobs.Test{}, []queue.Arg{
    {Type: "int", Value: 1},
}).Dispatch()

// Synchronous (runs in-process, not queued)
facades.Queue().Job(&jobs.Test{}, []queue.Arg{}).DispatchSync()

// Delayed dispatch
facades.Queue().Job(&jobs.Test{}, []queue.Arg{}).
    Delay(time.Now().Add(100 * time.Second)).Dispatch()

// Target specific queue
facades.Queue().Job(&jobs.Test{}, []queue.Arg{}).OnQueue("emails").Dispatch()

// Target specific connection
facades.Queue().Job(&jobs.Test{}, []queue.Arg{}).OnConnection("redis").Dispatch()

// Both
facades.Queue().Job(&jobs.Test{}, []queue.Arg{}).
    OnConnection("redis").OnQueue("processing").Dispatch()
```

## Job Chains

Execute jobs sequentially. If one fails, remaining jobs are skipped:

```go
facades.Queue().Chain([]queue.Jobs{
    {Job: &jobs.Step1{}, Args: []queue.Arg{{Type: "int", Value: 1}}},
    {Job: &jobs.Step2{}, Args: []queue.Arg{{Type: "int", Value: 2}}},
}).Dispatch()
```

## Queue Drivers

Configure in `config/queue.go`:

| Driver | Behavior |
|--------|----------|
| `sync` | Default. Executes immediately in-process, not queued |
| `database` | Persists jobs to DB. Requires migration: `20210101000002_create_jobs_table.go` |
| `custom` | Implement `queue.Driver` interface. Use `"via"` key in config |

Custom driver config:

```go
"connections": map[string]any{
    "redis": map[string]any{
        "driver":     "custom",
        "connection": "default",
        "queue":      "default",
        "via": func() (queue.Driver, error) {
            return redisfacades.Queue("redis")
        },
    },
},
```

## Queue Workers

The default queue worker runs via the queue service provider's runner. Start additional workers by creating a custom runner and adding it to `WithRunners` in `bootstrap/app.go`:

```go
foundation.Setup().
    WithRunners(func() []contractsfoundation.Runner {
        return []contractsfoundation.Runner{YourRunner}
    }).Create()
```

Reference implementation: `github.com/goravel/framework/queue/runners.go`.

## Failed Jobs

View failed jobs: `./artisan queue:failed`

Retry failed jobs:

```shell
# Single
./artisan queue:retry <uuid>

# Multiple
./artisan queue:retry <uuid1> <uuid2>

# By connection/queue
./artisan queue:retry --connection=redis
./artisan queue:retry --queue=processing
./artisan queue:retry --connection=redis --queue=processing

# All
./artisan queue:retry all
```

## Connections vs Queues

A **connection** maps to a backend driver (sync, database, redis). Each connection has a default **queue** (configured in the connection's `queue` attribute). Multiple queues per connection allow job categorization and priority.

## Arg Types

`queue.Arg` supports these `Type` values:

`bool`, `int`, `int8`, `int16`, `int32`, `int64`, `uint`, `uint8`, `uint16`, `uint32`, `uint64`, `float32`, `float64`, `string`

Slice variants: `[]bool`, `[]int`, `[]int8`, `[]int16`, `[]int32`, `[]int64`, `[]uint`, `[]uint8`, `[]uint16`, `[]uint32`, `[]uint64`, `[]float32`, `[]float64`, `[]string`
