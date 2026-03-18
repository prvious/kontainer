# Events Reference

## Event Interface

An event is a data container. Its `Handle` receives and returns `[]event.Arg`, optionally transforming data before listeners receive it:

```go
package events

import "github.com/goravel/framework/contracts/event"

type OrderShipped struct{}

func (r *OrderShipped) Handle(args []event.Arg) ([]event.Arg, error) {
    return args, nil // pass through or transform
}
```

Generate: `./artisan make:event OrderShipped` or `./artisan make:event user/OrderShipped`.

## Listener Interface

Listeners implement three methods:

```go
package listeners

import "github.com/goravel/framework/contracts/event"

type SendShipmentNotification struct{}

// Unique identifier for this listener
func (r *SendShipmentNotification) Signature() string {
    return "send_shipment_notification"
}

// Queue configuration. Set Enable: true to process asynchronously.
func (r *SendShipmentNotification) Queue(args ...any) event.Queue {
    return event.Queue{
        Enable:     false,  // true = async via queue system
        Connection: "",     // queue connection name (empty = default)
        Queue:      "",     // queue name (empty = default)
    }
}

// Handle receives the args returned by the event's Handle method.
func (r *SendShipmentNotification) Handle(args ...any) error {
    name := args[0] // access dispatched args positionally
    return nil
}
```

Generate: `./artisan make:listener SendNotification` or `./artisan make:listener user/SendNotification`.

## Registration

Register event-to-listener mappings in `bootstrap/app.go` via `WithEvents`:

```go
foundation.Setup().
    WithEvents(func() map[event.Event][]event.Listener {
        return map[event.Event][]event.Listener{
            events.NewOrderShipped(): {
                listeners.NewSendShipmentNotification(),
                listeners.NewLogOrderActivity(),
            },
        }
    }).
    WithConfig(config.Boot).
    Create()
```

One event can have multiple listeners. Listeners execute in registration order.

## Dispatching Events

```go
err := facades.Event().Job(&events.OrderShipped{}, []event.Arg{
    {Type: "string", Value: "Goravel"},
    {Type: "int", Value: 1},
}).Dispatch()
```

The dispatch flow:
1. Event's `Handle` receives and optionally transforms args
2. Each registered listener's `Handle` receives the transformed args
3. If a listener returns an error, propagation stops (remaining listeners skipped)

## Queued Listeners

Set `Enable: true` in the listener's `Queue()` method to process asynchronously via the queue system. Requires a configured queue driver (not `sync`).

```go
func (r *SendShipmentNotification) Queue(args ...any) event.Queue {
    return event.Queue{
        Enable:     true,
        Connection: "redis",
        Queue:      "notifications",
    }
}
```

**Warning:** Queued listeners dispatched inside database transactions may execute before the transaction commits. Move the event dispatch outside the transaction to avoid referencing uncommitted data.

## Propagation Control

Return an error from a listener's `Handle` to stop propagation to subsequent listeners for that event. This is the only mechanism for stopping propagation.

## Arg Types

Same as queue args. `event.Arg` supports these `Type` values:

`bool`, `int`, `int8`, `int16`, `int32`, `int64`, `uint`, `uint8`, `uint16`, `uint32`, `uint64`, `float32`, `float64`, `string`

Slice variants: `[]bool`, `[]int`, `[]int8`, `[]int16`, `[]int32`, `[]int64`, `[]uint`, `[]uint8`, `[]uint16`, `[]uint32`, `[]uint64`, `[]float32`, `[]float64`, `[]string`
