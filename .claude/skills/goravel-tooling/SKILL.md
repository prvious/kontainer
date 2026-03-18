---
name: goravel-tooling
description: Use when writing Goravel artisan commands, tests, or configuring logging
---

# Goravel Tooling

Artisan CLI commands, testing (unit + HTTP), facade mocking, and structured logging. All patterns target Goravel v1.17.

## Artisan Commands

Generate with `./artisan make:command SendEmails`. Creates file in `app/console/commands/`.

Command structure -- implement four methods:

```go
package commands

import (
    "github.com/goravel/framework/contracts/console"
    "github.com/goravel/framework/contracts/console/command"
)

type SendEmails struct{}

func (r *SendEmails) Signature() string    { return "send:emails" }
func (r *SendEmails) Description() string  { return "Send emails" }

func (r *SendEmails) Extend() command.Extend {
    return command.Extend{
        Category: "mail",
        Arguments: []command.Argument{
            &command.ArgumentString{Name: "to", Usage: "recipient", Required: true},
        },
        Flags: []command.Flag{
            &command.StringFlag{Name: "lang", Value: "en", Aliases: []string{"l"}, Usage: "language"},
        },
    }
}

func (r *SendEmails) Handle(ctx console.Context) error {
    to := ctx.ArgumentString("to")
    lang := ctx.Option("lang")
    ctx.Info("Sending to " + to + " in " + lang)
    return nil
}
```

Register in `bootstrap/app.go` via `WithCommands`. Auto-generated commands register themselves in `bootstrap/commands.go`.

Retrieve input: `ctx.Argument(0)`, `ctx.Arguments()`, `ctx.ArgumentString("name")`, `ctx.Option("flag")`.

Prompt for input: `ctx.Ask(question)`, `ctx.Secret(question)`, `ctx.Confirm(question)`, `ctx.Choice(question, options)`, `ctx.MultiSelect(question, options)`.

Output methods: `ctx.Info()`, `ctx.Error()`, `ctx.Warning()`, `ctx.Comment()`, `ctx.Line()`, `ctx.NewLine()`.

Colors: `ctx.Green()`, `ctx.Red()`, `ctx.Yellow()`, `ctx.Greenln()`, `ctx.Redln()`, `ctx.Yellowln()`.

Progress bars:

```go
items := []any{"a", "b", "c"}
_, err := ctx.WithProgressBar(items, func(item any) error { return nil })

// Manual control
bar := ctx.CreateProgressBar(len(items))
bar.Start()
bar.Advance()
bar.Finish()
```

Spinner: `ctx.Spinner("Loading...", console.SpinnerOption{Action: func() error { ... }})`.

Call programmatically: `facades.Artisan().Call("send:emails --lang Chinese user@test.com")`.

See [artisan-reference.md](./artisan-reference.md) for argument types, all flag types, prompting options, and helper utilities (strings, processes, pluralization, color).

## Testing

Goravel uses `stretchr/testify` suite. Generate with `./artisan make:test feature/UserTest`.

```go
package feature

import (
    "testing"
    "github.com/stretchr/testify/suite"
    "goravel/tests"
)

type UserTestSuite struct {
    suite.Suite
    tests.TestCase
}

func TestUserTestSuite(t *testing.T) {
    suite.Run(t, new(UserTestSuite))
}

func (s *UserTestSuite) SetupTest()    {} // runs before each test
func (s *UserTestSuite) TearDownTest() {} // runs after each test

func (s *UserTestSuite) TestGetUser() {
    s.True(true)
}
```

### HTTP Tests

Simulate requests without real HTTP calls. Use `s.Http(s.T())` to get a `TestRequest`:

```go
func (s *UserTestSuite) TestGetUser() {
    resp, err := s.Http(s.T()).Get("/users/1")
    s.Nil(err)
    resp.AssertOk()
}

func (s *UserTestSuite) TestCreateUser() {
    body, _ := http.NewBody().SetField("name", "Alice").Build()
    resp, err := s.Http(s.T()).
        WithHeader("Content-Type", body.ContentType()).
        Post("/users", body)
    s.Nil(err)
    resp.AssertCreated().AssertJson(map[string]any{"name": "Alice"})
}
```

JSON assertions: `AssertJson()` (partial match), `AssertExactJson()` (exact match), `AssertJsonMissing()`, `AssertFluentJson(func(json) { json.Where("key", val).Missing("secret") })`.

Status shortcuts: `AssertOk()`, `AssertCreated()`, `AssertNotFound()`, `AssertUnauthorized()`, `AssertForbidden()`, `AssertBadRequest()`, `AssertStatus(code)`.

Request setup: `WithHeader()`, `WithHeaders()`, `WithCookie()`, `WithCookies()`, `WithSession()`.

### Mocking Facades

All facades are interface-backed. Mock with `mock.Factory()`:

```go
import "github.com/goravel/framework/testing/mock"

func TestSomething(t *testing.T) {
    mockFactory := mock.Factory()

    // Mock any facade
    mockConfig := mockFactory.Config()
    mockConfig.On("GetString", "app.name", "test").Return("Kontainer").Once()

    mockCache := mockFactory.Cache()
    mockCache.On("Get", "key", "default").Return("value").Once()

    // facades.Log() uses fmt instead of real log output
    mockFactory.Log()

    // Run code under test, then verify
    mockConfig.AssertExpectations(t)
}
```

Available mock factories: `App`, `Artisan`, `Auth`, `Cache`, `Config`, `Crypt`, `Event`, `Gate`, `Grpc`, `Hash`, `Lang`, `Log`, `Mail`, `Orm`, `Queue`, `Storage`, `Validation`, `View`. Composite mocks: `OrmQuery`, `OrmTransaction`, `EventTask`, `QueueTask`, `StorageDriver`, `ValidationValidator`, `ValidationErrors`.

### Database Testing

Use Docker-based isolated databases per test package:

```go
func TestMain(m *testing.M) {
    database, _ := facades.Testing().Docker().Database()
    database.Build()
    database.Ready()
    database.Migrate()
    exit := m.Run()
    database.Shutdown()
    os.Exit(exit)
}
```

Within suites: `s.RefreshDatabase()` in `SetupTest()` to reset between tests. `s.Seed()` to run seeders. Factories: `facades.Orm().Factory().Create(&user)`.

See [testing-reference.md](./testing-reference.md) for all HTTP assertions, fluent JSON testing, Docker setup, and full mock examples.

## Logging

Access via `facades.Log()`. Configure channels in `config/logging.go`. Default channel is `stack` (forwards to multiple channels).

```go
facades.Log().Debug(message)
facades.Log().Info(message)
facades.Log().Warning(message)
facades.Log().Error(message)
facades.Log().Fatal(message)
facades.Log().Panic(message)

// Formatted
facades.Log().Infof("user %s logged in", username)
facades.Log().Errorf("failed: %v", err)
```

Write to specific channels:

```go
facades.Log().Channel("single").Info(message)
facades.Log().Stack([]string{"single", "slack"}).Info(message)
```

Chain context methods: `.User("John")`, `.Code("E001")`, `.Hint("check config")`, `.In("auth")`, `.Owner("team-infra")`, `.Tags([]string{"critical"})`, `.With(map[string]any{"key": "val"})`, `.WithTrace()`, `.Request(req)`, `.Response(resp)`.

Context injection: `facades.Log().WithContext(ctx)`.

See [logging-reference.md](./logging-reference.md) for channel drivers, custom drivers, and configuration.

## Key Helpers

Goravel ships utility packages -- use when appropriate:

| Package | Import | Key Methods |
|---------|--------|-------------|
| `str` | `support/str` | `str.Of("text").Camel()`, `.Snake()`, `.Kebab()`, `.Studly()`, `.Plural()`, `.Singular()`, `.Contains()`, `.Replace()` |
| `carbon` | `support/carbon` | `carbon.Now()`, `.Parse()`, `.SetTimezone()`, `.FromTimestamp()` |
| `color` | `support/color` | `color.Blue().Println()`, `color.New(color.FgRed).Printf()` |
| `path` | `support/path` | `path.App()`, `path.Base()`, `path.Config()`, `path.Storage()`, `path.Database()` |
| `debug` | `support/debug` | `debug.Dump(vars...)`, `debug.SDump(vars...)` |
| `collect` | `support/collect` | `collect.Map()`, `.Filter()`, `.Each()`, `.GroupBy()`, `.Unique()` |
| `maps` | `support/maps` | `maps.Get()`, `.Set()`, `.Has()`, `.Forget()`, `.Where()` |
| `convert` | `support/convert` | `convert.Tap()`, `.With()`, `.Transform()`, `.Default()`, `.Pointer()` |
| `pluralizer` | `support/pluralizer` | `pluralizer.Plural("car")`, `pluralizer.Singular("cars")` |
| `process` | `facades.Process()` | `.Run("cmd", "args")`, `.Start()`, `.Pipe()`, `.Pool()` |

## Related Skills

- See **goravel-foundations** skill for `bootstrap/app.go` lifecycle and `WithCommands` registration
- See **goravel-http** skill for HTTP test assertions context (routes, controllers, middleware being tested)
- See **goravel-data** skill for ORM models, factories, seeders, and `RefreshDatabase()` used in database tests
- See **goravel-security** skill for `facades.Auth()` details when mocking auth in tests
- See **goravel-async** skill for `facades.Queue()`, `facades.Event()`, `facades.Cache()` details when mocking async facades

## Reference Files

| File | Contents |
|------|----------|
| [artisan-reference.md](./artisan-reference.md) | Command structure, argument/flag types, prompting, output formatting, colors, progress bars, processes, strings, pluralization |
| [testing-reference.md](./testing-reference.md) | Test setup, HTTP tests, all assertions, fluent JSON, mocking facades, database testing, Docker |
| [logging-reference.md](./logging-reference.md) | Channel drivers, log levels, contextual logging, custom drivers, configuration |
