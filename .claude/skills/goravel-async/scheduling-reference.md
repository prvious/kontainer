# Scheduling Reference

## Defining Schedules

Register in `bootstrap/app.go` via `WithSchedule`. The scheduler starts automatically when calling `Start()` in `main.go`:

```go
foundation.Setup().
    WithSchedule(func() []schedule.Event {
        return []schedule.Event{
            facades.Schedule().Call(func() {
                // closure task
            }).Daily(),
            facades.Schedule().Command("report:generate").Daily(),
        }
    }).
    WithConfig(config.Boot).
    Start()
```

## Closure Scheduling

```go
facades.Schedule().Call(func() {
    // your logic
}).Daily()
```

## Command Scheduling

Schedule any Artisan command by name (with arguments):

```go
facades.Schedule().Command("send:emails name").Daily()
```

## Frequency Methods

| Method | Schedule |
|--------|----------|
| `.Cron("* * * * *")` | Custom cron (minutes) |
| `.Cron("* * * * * *")` | Custom cron (seconds) |
| `.EverySecond()` | Every second |
| `.EveryTwoSeconds()` | Every 2 seconds |
| `.EveryFiveSeconds()` | Every 5 seconds |
| `.EveryTenSeconds()` | Every 10 seconds |
| `.EveryFifteenSeconds()` | Every 15 seconds |
| `.EveryTwentySeconds()` | Every 20 seconds |
| `.EveryThirtySeconds()` | Every 30 seconds |
| `.EveryMinute()` | Every minute |
| `.EveryTwoMinutes()` | Every 2 minutes |
| `.EveryThreeMinutes()` | Every 3 minutes |
| `.EveryFourMinutes()` | Every 4 minutes |
| `.EveryFiveMinutes()` | Every 5 minutes |
| `.EveryTenMinutes()` | Every 10 minutes |
| `.EveryFifteenMinutes()` | Every 15 minutes |
| `.EveryThirtyMinutes()` | Every 30 minutes |
| `.Hourly()` | Every hour |
| `.HourlyAt(17)` | Every hour at :17 |
| `.EveryTwoHours()` | Every 2 hours |
| `.EveryThreeHours()` | Every 3 hours |
| `.EveryFourHours()` | Every 4 hours |
| `.EverySixHours()` | Every 6 hours |
| `.Daily()` | Midnight daily |
| `.DailyAt("13:00")` | Daily at 13:00 |
| `.Days(1, 3, 5)` | Mon, Wed, Fri |
| `.Weekdays()` | Mon-Fri |
| `.Weekends()` | Sat-Sun |
| `.Mondays()` | Every Monday |
| `.Tuesdays()` | Every Tuesday |
| `.Wednesdays()` | Every Wednesday |
| `.Thursdays()` | Every Thursday |
| `.Fridays()` | Every Friday |
| `.Saturdays()` | Every Saturday |
| `.Sundays()` | Every Sunday |
| `.Weekly()` | Every week |
| `.Monthly()` | Every month |
| `.Quarterly()` | Every quarter |
| `.Yearly()` | Every year |

## Preventing Overlaps

| Method | Behavior |
|--------|----------|
| `.SkipIfStillRunning()` | Skip this invocation if previous is still running |
| `.DelayIfStillRunning()` | Wait for previous to finish, then run |

```go
facades.Schedule().Command("send:emails").EveryMinute().SkipIfStillRunning()
facades.Schedule().Command("send:emails").EveryMinute().DelayIfStillRunning()
```

## Single-Server Execution

Prevent duplicate execution across multiple servers with `.OnOneServer()`. Requires a distributed cache driver (memcached, redis, or dynamodb) as the default cache driver:

```go
facades.Schedule().Command("report:generate").Daily().OnOneServer()
```

Closures must be named when using `OnOneServer()`:

```go
facades.Schedule().Call(func() {
    fmt.Println("goravel")
}).Daily().OnOneServer().Name("goravel")
```

## Logging

When `app.debug` is `true`, the console prints all scheduler logs. Otherwise, only `error` level logs appear.

## Manual Execution

```shell
./artisan schedule:run    # run pending tasks manually
./artisan schedule:list   # view all registered tasks
```
