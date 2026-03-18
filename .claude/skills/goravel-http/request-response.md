# Request & Response Reference

Full request and response API for Goravel v1.17. Covers all methods on `ctx.Request()` and `ctx.Response()`, including file uploads, cookies, and sessions.

## Request: URL & Method

```go
ctx.Request().Path()       // "/users/1" (resolved path)
ctx.Request().OriginPath() // "/users/{id}" (route pattern)
ctx.Request().Url()        // "/users?name=Goravel" (path + query)
ctx.Request().FullUrl()    // "http://example.com/users?name=Goravel"
ctx.Request().Host()       // "example.com"
ctx.Request().Method()     // "GET", "POST", etc.
ctx.Request().Info()       // route information object
ctx.Request().Name()       // route name (if named)
```

## Request: Headers & IP

```go
header := ctx.Request().Header("X-Custom", "default")  // single header with default
headers := ctx.Request().Headers()                       // all headers
ip := ctx.Request().Ip()                                 // client IP
```

## Request: Route Parameters

For routes with `{param}` placeholders:

```go
// Route: /users/{id}
id := ctx.Request().Route("id")        // string
id := ctx.Request().RouteInt("id")     // int
id := ctx.Request().RouteInt64("id")   // int64
```

## Request: Query String

```go
name := ctx.Request().Query("name")              // string, empty if absent
name := ctx.Request().Query("name", "default")   // with default

id := ctx.Request().QueryInt("id")               // int
id := ctx.Request().QueryInt64("id")             // int64
ok := ctx.Request().QueryBool("active")          // bool

// /path?names=a&names=b
names := ctx.Request().QueryArray("names")       // []string

// /path?meta[key]=value
meta := ctx.Request().QueryMap("meta")           // map[string]string

queries := ctx.Request().Queries()               // map[string]string (all query params)
```

Note: Only one-dimensional JSON data is retrievable via Query methods; nested JSON returns empty.

## Request: Body Input (JSON/Form)

Retrieve order: JSON first, then form data.

```go
name := ctx.Request().Input("name")                // string
name := ctx.Request().Input("name", "default")     // with default
age := ctx.Request().InputInt("age")               // int
age := ctx.Request().InputInt64("age")             // int64
ok := ctx.Request().InputBool("active")            // bool
tags := ctx.Request().InputArray("tags")           // []string
meta := ctx.Request().InputMap("meta")             // map[string]string
items := ctx.Request().InputMapArray("items")      // map[string][]string
```

## Request: All Input

Merge of JSON + form + query (priority from front to back):

```go
data := ctx.Request().All()  // map[string]any
```

## Request: Bind Structs

### Bind JSON/Form Body

```go
type CreateUser struct {
    Name  string `form:"name" json:"name"`
    Email string `form:"email" json:"email"`
}

var user CreateUser
err := ctx.Request().Bind(&user)

// Also works with map
var data map[string]any
err := ctx.Request().Bind(&data)
```

### Bind Query Only

```go
type Filter struct {
    Page int    `form:"page"`
    Sort string `form:"sort"`
}

var filter Filter
err := ctx.Request().BindQuery(&filter)
```

Note: `BindQuery` only supports binding to structs.

## Request: Files

```go
file, err := ctx.Request().File("avatar")       // single file (*multipart.FileHeader)
files, err := ctx.Request().Files("documents")  // multiple files

// Save to disk
file, _ := ctx.Request().File("avatar")
file.Store("./public/avatars")
```

## Request: Cookies

```go
value := ctx.Request().Cookie("session_id")             // empty string if absent
value := ctx.Request().Cookie("session_id", "default")  // with default
```

## Request: Context Values

Attach and retrieve arbitrary data on the request context (useful in middleware):

```go
// Set (typically in middleware)
ctx.WithValue("user_id", 42)

// Get (in handler)
userID := ctx.Value("user_id")
```

## Request: Underlying Context

Access the standard `context.Context`:

```go
stdCtx := ctx.Context()
```

## Request: Origin

Access the underlying HTTP request object:

```go
origin := ctx.Request().Origin()
```

## Response: String

```go
ctx.Response().String(http.StatusOK, "Hello")
```

## Response: JSON

```go
ctx.Response().Json(http.StatusOK, http.Json{
    "id":   1,
    "name": "Goravel",
})

// Also accepts structs
ctx.Response().Json(http.StatusOK, myStruct)
```

## Response: Custom Data

Set content type and raw bytes:

```go
ctx.Response().Data(http.StatusOK, "text/html; charset=utf-8", []byte("<h1>Hello</h1>"))
```

## Response: Status Shortcuts

```go
ctx.Response().Success().String("OK")              // 200
ctx.Response().Success().Json(http.Json{"ok": true}) // 200
ctx.Response().Status(http.StatusCreated).Json(data) // custom status code
```

## Response: File Serving

```go
ctx.Response().File("./public/logo.png")                   // serve file inline
ctx.Response().Download("./public/report.pdf", "report.pdf") // force download with filename
```

## Response: Stream

Write chunked responses:

```go
ctx.Response().Stream(http.StatusOK, func(w http.StreamWriter) error {
    for _, chunk := range chunks {
        if _, err := w.Write([]byte(chunk)); err != nil {
            return err
        }
        if err := w.Flush(); err != nil {
            return err
        }
    }
    return nil
})
```

## Response: Redirect

```go
ctx.Response().Redirect(http.StatusMovedPermanently, "https://goravel.dev")
ctx.Response().Redirect(http.StatusFound, "/login")
```

## Response: No Content

```go
ctx.Response().NoContent()                    // 204 by default
ctx.Response().NoContent(http.StatusAccepted) // custom status
```

## Response: Headers

Chain `.Header()` before the response body method:

```go
ctx.Response().Header("X-Request-Id", "abc123").Json(http.StatusOK, data)
```

## Response: Cookies

### Set Cookie

```go
import "time"

ctx.Response().Cookie(http.Cookie{
    Name:     "token",
    Value:    "jwt-value",
    Path:     "/",
    Domain:   "example.com",
    Expires:  time.Now().Add(24 * time.Hour),
    Secure:   true,
    HttpOnly: true,
})
```

### Remove Cookie

```go
ctx.Response().WithoutCookie("token")
```

## Response: Views

Render HTML templates from `resources/views/`:

```go
ctx.Response().View().Make("welcome.tmpl", map[string]any{"name": "Goravel"})

// First available view from a list
ctx.Response().View().First([]string{"custom/admin.tmpl", "admin.tmpl"}, data)
```

Check if a view exists: `facades.View().Exist("welcome.tmpl")`

Share data with all views: `facades.View().Share("key", "value")` (call in `WithCallback`).

## Response: Origin (Middleware Inspection)

Inspect the outgoing response, typically in middleware:

```go
origin := ctx.Response().Origin()
origin.Body()    // response body bytes
origin.Header()  // response headers
origin.Size()    // response size
origin.Status()  // HTTP status code
```

## Response: Abort

Terminate the middleware chain (stop further processing):

```go
ctx.Request().Abort()                           // abort with no status
ctx.Request().Abort(http.StatusNotFound)         // abort with status
ctx.Response().String(404, "Not Found").Abort()  // send response then abort
```

## Session API

Requires `middleware.StartSession()` registered globally or on the route. Access via `ctx.Request().Session()`.

### Read

```go
value := ctx.Request().Session().Get("key")             // nil if absent
value := ctx.Request().Session().Get("key", "default")  // with default
all := ctx.Request().Session().All()                     // map[string]any
subset := ctx.Request().Session().Only([]string{"username", "email"})
```

### Check Existence

```go
ctx.Request().Session().Has("key")     // true if present and non-nil
ctx.Request().Session().Exists("key")  // true if present (even if nil)
ctx.Request().Session().Missing("key") // true if absent
```

### Write

```go
ctx.Request().Session().Put("key", "value")
```

### Delete

```go
value := ctx.Request().Session().Pull("key")  // get and delete
ctx.Request().Session().Forget("key1", "key2") // delete specific keys
ctx.Request().Session().Flush()                 // delete all
```

### Flash Data

One-request-only session data:

```go
ctx.Request().Session().Flash("status", "Saved!")  // available next request only
ctx.Request().Session().Reflash()                   // keep all flash data one more request
ctx.Request().Session().Keep("status")              // keep specific flash data one more request
ctx.Request().Session().Now("status", "Immediate")  // available current request only
```

### Session Management

```go
ctx.Request().Session().Regenerate()   // new session ID, keep data
ctx.Request().Session().Invalidate()   // new session ID, flush data

ctx.Request().Session().GetID()        // current session ID
ctx.Request().Session().GetName()      // session cookie name

ctx.Request().HasSession()             // bool
ctx.Request().SetSession(session)      // attach a custom session instance
```

After `Invalidate()` or `Regenerate()`, save the new session ID to a cookie:

```go
ctx.Response().Cookie(http.Cookie{
    Name:   ctx.Request().Session().GetName(),
    Value:  ctx.Request().Session().GetID(),
    MaxAge: facades.Config().GetInt("session.lifetime") * 60,
    Path:   facades.Config().GetString("session.path"),
})
```

## CSRF Protection

For server-rendered forms, register `middleware.VerifyCsrfToken(exceptPaths)` and include a hidden field:

```html
<input type="hidden" name="_token" value="{{ .csrf_token }}" />
```

Or pass `X-CSRF-TOKEN={{ .csrf_token }}` as a request header.
