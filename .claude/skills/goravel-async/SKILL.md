---
name: goravel-async
description: Use when working with Goravel queues, events, listeners, task scheduling, or caching
---

# Goravel Async

Goravel's async layer covers background job processing, event-driven architecture, scheduled tasks, and caching. Operate through `facades.Queue()`, `facades.Event()`, `facades.Schedule()`, and `facades.Cache()`.

**Version:** All patterns target Goravel v1.17.

## Queues

Define a job by implementing `queue.Job` (two methods: `Signature` and `Handle`):

```go
package jobs

type ProcessPodcast struct{}

func (r *ProcessPodcast) Signature() string { return "process_podcast" }

func (r *ProcessPodcast) Handle(args ...any) error {
    // args received from dispatch call
    return nil
}
```

Register jobs in `bootstrap/app.go` via `WithJobs`. Generate with `./artisan make:job ProcessPodcast`.

Dispatch:

```go
// Async
err := facades.Queue().Job(&jobs.ProcessPodcast{}, []queue.Arg{
    {Type: "string", Value: "episode-42"},
}).Dispatch()

// Sync (runs in current process)
err := facades.Queue().Job(&jobs.ProcessPodcast{}, []queue.Arg{}).DispatchSync()

// Delayed
err := facades.Queue().Job(&jobs.ProcessPodcast{}, []queue.Arg{}).
    Delay(time.Now().Add(100 * time.Second)).Dispatch()

// Chain (sequential, stops on failure)
err := facades.Queue().Chain([]queue.Jobs{
    {Job: &jobs.Step1{}, Args: []queue.Arg{{Type: "int", Value: 1}}},
    {Job: &jobs.Step2{}, Args: []queue.Arg{{Type: "int", Value: 2}}},
}).Dispatch()

// Target specific queue/connection
err := facades.Queue().Job(&jobs.Test{}, []queue.Arg{}).
    OnConnection("redis").OnQueue("processing").Dispatch()
```

Drivers: `sync` (default, in-process), `database` (requires migration), custom (implement `queue.Driver`).

See [queues-reference.md](./queues-reference.md) for retries, failed jobs, queue workers, arg types.

## Events & Listeners

Define an event (data container with optional transform):

```go
type OrderShipped struct{}

func (r *OrderShipped) Handle(args []event.Arg) ([]event.Arg, error) {
    return args, nil
}
```

Define a listener:

```go
type SendShipmentNotification struct{}

func (r *SendShipmentNotification) Signature() string {
    return "send_shipment_notification"
}

func (r *SendShipmentNotification) Queue(args ...any) event.Queue {
    return event.Queue{Enable: false} // set Enable: true for async
}

func (r *SendShipmentNotification) Handle(args ...any) error {
    return nil // return error to stop propagation
}
```

Register in `bootstrap/app.go` via `WithEvents`:

```go
foundation.Setup().
    WithEvents(func() map[event.Event][]event.Listener {
        return map[event.Event][]event.Listener{
            events.NewOrderShipped(): {
                listeners.NewSendShipmentNotification(),
            },
        }
    })
```

Dispatch:

```go
err := facades.Event().Job(&events.OrderShipped{}, []event.Arg{
    {Type: "string", Value: "Goravel"},
    {Type: "int", Value: 1},
}).Dispatch()
```

Generate: `./artisan make:event OrderShipped`, `./artisan make:listener SendNotification`.

See [events-reference.md](./events-reference.md) for queued listeners, propagation control, arg types.

## Task Scheduling

Define schedules in `bootstrap/app.go` via `WithSchedule`:

```go
foundation.Setup().
    WithSchedule(func() []schedule.Event {
        return []schedule.Event{
            // Schedule a closure
            facades.Schedule().Call(func() {
                facades.Orm().Query().Where("1 = 1").Delete(&models.User{})
            }).Daily(),

            // Schedule an Artisan command
            facades.Schedule().Command("report:generate").Daily(),
        }
    })
```

Frequency methods chain onto schedule calls: `.EveryMinute()`, `.Hourly()`, `.Daily()`, `.DailyAt("13:00")`, `.Weekly()`, `.Monthly()`, `.Cron("* * * * *")`.

Overlap prevention: `.SkipIfStillRunning()`, `.DelayIfStillRunning()`.

Single-server execution: `.OnOneServer()` (requires memcached/redis/dynamodb cache driver). Named closures: `.Name("task-name")`.

See [scheduling-reference.md](./scheduling-reference.md) for all frequency methods and overlap options.

## Cache

Access via `facades.Cache()`. Built-in driver: `memory`. Redis available via `github.com/goravel/redis`.

```go
// Store
facades.Cache().Put("key", "value", 5*time.Second)
facades.Cache().Forever("key", "value")
facades.Cache().Add("key", "value", 5*time.Second) // only if absent

// Retrieve
value := facades.Cache().Get("key", "default")
value := facades.Cache().GetString("key", "default")
value := facades.Cache().GetInt("key", 0)
value := facades.Cache().GetBool("key", false)

// Retrieve & store (lazy populate)
value, err := facades.Cache().Remember("key", 5*time.Second, func() (any, error) {
    return "computed", nil
})

// Delete
facades.Cache().Forget("key")
facades.Cache().Pull("key", "default") // get + delete
facades.Cache().Flush()                // clear all

// Counters
facades.Cache().Increment("counter")
facades.Cache().Decrement("counter", 5)

// Multiple stores
facades.Cache().Store("redis").Get("key")

// Context
facades.Cache().WithContext(ctx).Get("key")
```

Atomic locks for distributed coordination:

```go
lock := facades.Cache().Lock("processing", 10*time.Second)
if lock.Get() {
    defer lock.Release()
    // exclusive work
}

// Block until lock available (max 5s wait)
lock.Block(5*time.Second, func() {
    // auto-released after closure
})
```

See [cache-reference.md](./cache-reference.md) for all methods, custom drivers, lock API.

## Related Skills

- See **goravel-foundations** skill for `bootstrap/app.go` lifecycle and `With*` registration methods (`WithJobs`, `WithEvents`, `WithSchedule`)
- See **goravel-data** skill for ORM operations used within jobs, listeners, and scheduled tasks
- See **goravel-tooling** skill for Artisan commands that can be scheduled, and for mocking Queue/Event/Cache in tests
- See **goravel-integrations** skill for queued mail delivery (`facades.Mail().Queue()`)

## Reference Files

| File | Contents |
|------|----------|
| [queues-reference.md](./queues-reference.md) | Job interface, dispatch methods, chains, delays, retries, failed jobs, drivers, workers, arg types |
| [events-reference.md](./events-reference.md) | Event/listener interfaces, registration, dispatching, queued listeners, propagation |
| [scheduling-reference.md](./scheduling-reference.md) | All frequency methods, command/closure scheduling, overlap prevention, single-server |
| [cache-reference.md](./cache-reference.md) | All cache methods, drivers, stores, atomic locks, custom drivers |
