# HTTP Client Reference

Complete reference for Goravel's outbound HTTP client (`facades.Http()`). Goravel v1.17.

## Making Requests

All verbs return `(client.Response, error)`:

```go
resp, err := facades.Http().Get("https://example.com")
resp, err := facades.Http().Post("https://example.com", body)    // body is io.Reader
resp, err := facades.Http().Put("https://example.com", body)
resp, err := facades.Http().Patch("https://example.com", body)
resp, err := facades.Http().Delete("https://example.com", body)
resp, err := facades.Http().Head("https://example.com")
resp, err := facades.Http().Options("https://example.com")
```

Use a named client from `config/http.go`:

```go
resp, err := facades.Http().Client("github").Get("https://api.github.com/user")
```

## Headers

```go
// Single header
facades.Http().WithHeader("X-Custom", "value").Get(url)

// Multiple headers
facades.Http().WithHeaders(map[string]string{
    "Content-Type": "application/json",
    "Accept":       "application/xml",
}).Get(url)

// Accept shorthand
facades.Http().Accept("application/xml").Get(url)
facades.Http().AcceptJson().Get(url) // application/json

// Replace all headers
facades.Http().ReplaceHeaders(map[string]string{"Authorization": "Bearer token"}).Get(url)

// Remove header / flush all
facades.Http().WithoutHeader("X-Old").Get(url)
facades.Http().FlushHeaders().Get(url)
```

## Authentication

```go
// Bearer token (default type "Bearer")
facades.Http().WithToken("my_token").Get(url)

// Custom token type
facades.Http().WithToken("my_token", "Token").Get(url)

// Basic auth
facades.Http().WithBasicAuth("user", "pass").Get(url)

// Remove token
facades.Http().WithoutToken().Get(url)
```

## Query Parameters

```go
// Single
facades.Http().WithQueryParameter("sort", "name").Get(url)

// Multiple
facades.Http().WithQueryParameters(map[string]string{
    "page": "2", "pageSize": "10",
}).Get(url)

// Raw string
facades.Http().WithQueryString("filter=active&order=price").Get(url)
```

## URI Templates

Replace placeholders in URLs:

```go
facades.Http().WithUrlParameter("id", "123").Get("https://api.example.com/users/{id}")

facades.Http().WithUrlParameters(map[string]string{
    "bookId": "456", "chapter": "7",
}).Get("https://api.example.com/books/{bookId}/chapters/{chapter}")
```

## Request Body

POST, PUT, PATCH, DELETE accept `io.Reader` as second argument. Use `http.NewBody()` to build form/multipart payloads:

```go
import "github.com/goravel/framework/support/http"

body, err := http.NewBody().
    SetField("name", "krishan").
    SetField("email", "k@example.com").
    Build()

resp, err := facades.Http().
    WithHeader("Content-Type", body.ContentType()).
    Post(url, body.Reader())
```

For JSON, pass a `bytes.Reader` with serialized JSON and set `Content-Type: application/json` header.

## Cookies

```go
facades.Http().WithCookie(&http.Cookie{Name: "session", Value: "xyz"}).Get(url)

facades.Http().WithCookies([]*http.Cookie{
    {Name: "session", Value: "xyz"},
    {Name: "lang", Value: "en"},
}).Get(url)

facades.Http().WithoutCookie("lang").Get(url)
```

## Context

```go
ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
defer cancel()
resp, err := facades.Http().WithContext(ctx).Get(url)
```

## Response Interface

```go
resp.Status()        // int — HTTP status code
resp.Successful()    // bool — 2xx
resp.Failed()        // bool — not 2xx
resp.ClientError()   // bool — 4xx
resp.ServerError()   // bool — 5xx
resp.Redirect()      // bool — 3xx

resp.Body()          // (string, error)
resp.Json()          // (map[string]any, error)
resp.Bind(&struct)   // error — decode into struct

resp.Header("name")  // string
resp.Headers()       // http.Header
resp.Cookie("name")  // *http.Cookie
resp.Cookies()       // []*http.Cookie
```

Status helpers: `OK()`, `Created()`, `Accepted()`, `NoContent()`, `MovedPermanently()`, `Found()`, `BadRequest()`, `Unauthorized()`, `Forbidden()`, `NotFound()`, `RequestTimeout()`, `Conflict()`, `UnprocessableEntity()`, `TooManyRequests()`.

## Testing / Faking

### Stub responses

```go
facades.Http().Fake(map[string]any{
    "https://api.example.com/*": facades.Http().Response().Json(200, map[string]string{"ok": "true"}),
    "github":                    facades.Http().Response().OK(),  // by client name
    "*":                         facades.Http().Response().Status(404), // fallback
})
```

Implicit conversions: `int` becomes status code, `string` becomes 200 body, `map` becomes 200 JSON.

### Response sequences

```go
facades.Http().Fake(map[string]any{
    "github": facades.Http().Sequence().
        PushStatus(500).
        PushString(429, "Rate Limit").
        PushStatus(200),
})
```

Use `.WhenEmpty(response)` to define a default after all sequence entries are consumed.

### Response builder

```go
facades.Http().Response().Json(201, data)
facades.Http().Response().String(200, "body")
facades.Http().Response().File(200, "./fixtures/user.json")
facades.Http().Response().Make(200, "body", headers)
facades.Http().Response().OK()
facades.Http().Response().Status(403)
```

### Assertions

```go
facades.Http().AssertSent(func(req client.Request) bool {
    return req.Url() == "https://api.example.com/users" &&
           req.Method() == "POST" &&
           req.Input("role") == "admin"
})

facades.Http().AssertNotSent(func(req client.Request) bool {
    return req.Url() == "https://api.example.com/legacy"
})

facades.Http().AssertNothingSent()
facades.Http().AssertSentCount(3)
```

### Stray request prevention

```go
facades.Http().Fake(map[string]any{
    "github": facades.Http().Response().OK(),
}).PreventStrayRequests()

// Allow specific patterns
facades.Http().PreventStrayRequests().AllowStrayRequests([]string{
    "http://localhost:8080/*",
})
```

### Reset

Always reset in test cleanup. `Fake` and `Reset` mutate global state -- do not use with `t.Parallel()`.

```go
defer facades.Http().Reset()
```
