# Testing Reference

## Test Setup

Generate tests: `./artisan make:test feature/UserTest`.

Tests use `stretchr/testify` suite. Embed both `suite.Suite` and `tests.TestCase`:

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

func (s *UserTestSuite) SetupTest()    {} // before each test
func (s *UserTestSuite) TearDownTest() {} // after each test

func (s *UserTestSuite) TestExample() {
    s.True(true)
}
```

## Environment

Default: reads `.env` from root. Override per-package by placing `.env` in the package directory. Use `--env` flag for alternate file:

```shell
go test ./... --env=.env.testing
```

## HTTP Tests

Use `s.Http(s.T())` to get a `TestRequest`. Simulates request cycle internally (no real HTTP).

### Making Requests

```go
response, err := s.Http(s.T()).Get("/users/1")
response, err := s.Http(s.T()).Post("/users", body)
response, err := s.Http(s.T()).Put("/users/1", body)
response, err := s.Http(s.T()).Delete("/users/1", nil)
```

### Building Request Bodies

```go
import "github.com/goravel/framework/support/http"

builder := http.NewBody().SetField("name", "Alice")
body, err := builder.Build()
response, err := s.Http(s.T()).
    WithHeader("Content-Type", body.ContentType()).
    Post("/users", body)
```

### Request Configuration

```go
// Headers
s.Http(s.T()).WithHeader("X-Custom", "value").Get("/")
s.Http(s.T()).WithHeaders(map[string]string{
    "Accept": "application/json",
    "X-Custom": "value",
}).Get("/")

// Cookies
import testhttp "github.com/goravel/framework/testing/http"

s.Http(s.T()).WithCookie(testhttp.Cookie("name", "val")).Get("/")
s.Http(s.T()).WithCookies(testhttp.Cookies(map[string]string{
    "name": "val", "lang": "en",
})).Get("/")

// Session
s.Http(s.T()).WithSession(map[string]any{"role": "admin"}).Get("/")
```

### Inspecting Responses

```go
content, err := response.Content()     // raw body string
json, err := response.Json()           // parsed as map[string]any
cookies := response.Cookies()
headers := response.Headers()
session, err := response.Session()     // session values
```

## Status Assertions

```go
response.AssertStatus(200)
response.AssertOk()                    // 200
response.AssertCreated()               // 201
response.AssertAccepted()              // 202
response.AssertNoContent()             // 204
response.AssertMovedPermanently()      // 301
response.AssertFound()                 // 302
response.AssertNotModified()           // 304
response.AssertTemporaryRedirect()     // 307
response.AssertBadRequest()            // 400
response.AssertUnauthorized()          // 401
response.AssertPaymentRequired()       // 402
response.AssertForbidden()             // 403
response.AssertNotFound()              // 404
response.AssertMethodNotAllowed()      // 405
response.AssertNotAcceptable()         // 406
response.AssertRequestTimeout()        // 408
response.AssertConflict()              // 409
response.AssertGone()                  // 410
response.AssertUnprocessableEntity()   // 422
response.AssertTooManyRequests()       // 429
response.AssertInternalServerError()   // 500
response.AssertServiceUnavailable()    // 503
response.AssertSuccessful()            // 2xx range
response.AssertServerError()           // 5xx range
response.AssertPartialContent()        // 206
```

## JSON Assertions

### Partial Match

```go
response.AssertJson(map[string]any{"name": "Alice"})
```

Passes if response contains at minimum these key-value pairs (extra fields allowed).

### Exact Match

```go
response.AssertExactJson(map[string]any{"name": "Alice", "id": float64(1)})
```

Fails if response has extra or missing fields.

### Missing Keys/Values

```go
response.AssertJsonMissing(map[string]any{"password": "secret"})
```

### Fluent JSON Testing

```go
import contractstesting "github.com/goravel/framework/contracts/testing"

response.AssertFluentJson(func(json contractstesting.AssertableJSON) {
    json.Where("id", float64(1)).
        Where("name", "Alice").
        WhereNot("role", "admin").
        Has("email").
        Missing("password").
        HasAll([]string{"id", "name", "email"}).
        MissingAll([]string{"password", "secret"}).
        HasAny([]string{"email", "phone"})
})
```

### Collection Assertions

```go
response.AssertFluentJson(func(json contractstesting.AssertableJSON) {
    // Count items
    json.Count("items", 2)

    // Assert first element
    json.First("items", func(j contractstesting.AssertableJSON) {
        j.Where("id", float64(1))
    })

    // Assert each element
    json.Each("items", func(j contractstesting.AssertableJSON) {
        j.Has("id")
    })

    // Combined: count + scope first
    json.HasWithScope("items", 2, func(j contractstesting.AssertableJSON) {
        j.Where("id", float64(1))
    })
})
```

## Content Assertions

```go
response.AssertSee([]string{"Welcome"})
response.AssertSee([]string{"<div>"}, false)       // false = don't escape HTML
response.AssertDontSee([]string{"Error"})
response.AssertSeeInOrder([]string{"First", "Second"})
```

## Header/Cookie Assertions

```go
response.AssertHeader("Content-Type", "application/json")
response.AssertHeaderMissing("X-Debug")
response.AssertCookie("name", "value")
response.AssertCookieMissing("session")
response.AssertCookieExpired("old_token")
response.AssertCookieNotExpired("token")
```

## Mocking Facades

All facades are interface-backed. Use `mock.Factory()` to create mocks:

```go
import "github.com/goravel/framework/testing/mock"

func TestWithMocks(t *testing.T) {
    mockFactory := mock.Factory()

    // Config
    mockConfig := mockFactory.Config()
    mockConfig.On("GetString", "app.name", "test").Return("Kontainer").Once()

    // Cache
    mockCache := mockFactory.Cache()
    mockCache.On("Put", "key", "val", mock.Anything).Return(nil).Once()
    mockCache.On("Get", "key", "default").Return("val").Once()

    // Auth
    mockAuth := mockFactory.Auth()
    mockAuth.On("Logout", mock.Anything).Return(nil).Once()

    // ORM (chained)
    mockOrm := mockFactory.Orm()
    mockQuery := mockFactory.OrmQuery()
    mockOrm.On("Query").Return(mockQuery)
    mockQuery.On("Create", mock.Anything).Return(nil).Once()
    mockQuery.On("Where", "id = ?", 1).Return(mockQuery).Once()
    mockQuery.On("Find", mock.Anything).Return(nil).Once()

    // Log (redirects to fmt)
    mockFactory.Log()

    // Queue (composite)
    mockQueue := mockFactory.Queue()
    mockTask := mockFactory.QueueTask()
    mockQueue.On("Job", mock.Anything, mock.Anything).Return(mockTask).Once()
    mockTask.On("Dispatch").Return(nil).Once()

    // Event (composite)
    mockEvent := mockFactory.Event()
    mockEventTask := mockFactory.EventTask()
    mockEvent.On("Job", mock.Anything, mock.Anything).Return(mockEventTask).Once()
    mockEventTask.On("Dispatch").Return(nil).Once()

    // Verification
    mockConfig.AssertExpectations(t)
    mockCache.AssertExpectations(t)
}
```

### Available Mock Factories

| Method | Facade |
|--------|--------|
| `App()` | `facades.App()` |
| `Artisan()` | `facades.Artisan()` |
| `Auth()` | `facades.Auth()` |
| `Cache()` | `facades.Cache()` |
| `Config()` | `facades.Config()` |
| `Crypt()` | `facades.Crypt()` |
| `Event()` | `facades.Event()` |
| `Gate()` | `facades.Gate()` |
| `Grpc()` | `facades.Grpc()` |
| `Hash()` | `facades.Hash()` |
| `Lang()` | `facades.Lang()` |
| `Log()` | `facades.Log()` (uses fmt) |
| `Mail()` | `facades.Mail()` |
| `Orm()` | `facades.Orm()` |
| `Queue()` | `facades.Queue()` |
| `Storage()` | `facades.Storage()` |
| `Validation()` | `facades.Validation()` |
| `View()` | `facades.View()` |

Composite mocks: `OrmQuery()`, `OrmTransaction()`, `EventTask()`, `QueueTask()`, `StorageDriver()`, `ValidationValidator()`, `ValidationErrors()`.

### ORM Transaction Mocking

```go
mockOrm := mockFactory.Orm()
mockTx := mockFactory.OrmTransaction()
mockOrm.On("Transaction", mock.Anything).Return(func(txFunc func(tx orm.Transaction) error) error {
    return txFunc(mockTx)
})
mockTx.On("Create", mock.Anything).Return(nil).Once()
```

## Database Testing

### Docker-Based Isolation

Use `TestMain` for package-level setup. Each package gets its own database container:

```go
func TestMain(m *testing.M) {
    database, err := facades.Testing().Docker().Database()
    // or: facades.Testing().Docker().Database("postgres")
    if err != nil { panic(err) }

    database.Build()
    database.Ready()
    database.Migrate()

    facades.App().Restart() // reload config with new DB

    exit := m.Run()
    database.Shutdown()
    os.Exit(exit)
}
```

Cache image: `facades.Testing().Docker().Cache()` or `Cache("redis")`.

Custom image:

```go
import contractstesting "github.com/goravel/framework/contracts/testing"

image, _ := facades.Testing().Docker().Image(contractstesting.Image{
    Repository: "mysql", Tag: "5.7",
    Env: []string{"MYSQL_ROOT_PASSWORD=123123", "MYSQL_DATABASE=goravel"},
    ExposedPorts: []string{"3306"},
})
```

### RefreshDatabase

Reset DB between tests within a suite:

```go
func (s *UserTestSuite) SetupTest() {
    s.RefreshDatabase()
}
```

### Seeders

```go
s.Seed()                                           // DatabaseSeeder
s.Seed(&seeders.UserSeeder{}, &seeders.PhotoSeeder{}) // specific seeders

// Docker-level seeding
database.Seed()
database.Seed(&seeders.UserSeeder{})
```

### Factories

```go
var user models.User
err := facades.Orm().Factory().Create(&user)
```

### Fresh

Reset database or cache image:

```go
database.Fresh()
cache.Fresh()
```

Images auto-uninstall after 1 hour. Manual: `database.Shutdown()`.
