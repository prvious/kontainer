# Validation Reference

Full validation API for Goravel v1.17. Covers inline validation, form requests, all built-in rules, custom rules, filters, and error handling.

## Inline Validation

Validate directly on the request with `ctx.Request().Validate()`:

```go
validator, err := ctx.Request().Validate(map[string]string{
    "title": "required|max_len:255",
    "body":  "required",
    "email": "required|email",
})

if validator.Fails() {
    return ctx.Response().Json(422, http.Json{"errors": validator.Errors().All()})
}
```

Rules are pipe-delimited strings. Keys match incoming JSON/form field names.

### Nested Attributes

Use dot notation for nested fields:

```go
validator, err := ctx.Request().Validate(map[string]string{
    "author.name":        "required",
    "author.description": "required",
})
```

### Slice Validation

Use `*` to validate each element in a slice:

```go
validator, err := ctx.Request().Validate(map[string]string{
    "tags.*": "required|string",
})
```

## Manual Validator

Create a validator independently of the request using `facades.Validation().Make()`:

```go
validator, err := facades.Validation().Make(ctx,
    map[string]any{"name": "Goravel", "age": 25},  // data (map or struct)
    map[string]string{"name": "required", "age": "required|int"},  // rules
)
```

Pass optional modifiers as additional arguments:

```go
validator, err := facades.Validation().Make(ctx, data, rules,
    validation.Messages(map[string]string{
        "name.required": "Name is mandatory",
    }),
    validation.Attributes(map[string]string{
        "email": "email address",
    }),
    validation.PrepareForValidation(func(ctx http.Context, data validationcontract.Data) error {
        if name, exist := data.Get("name"); exist {
            return data.Set("name", strings.TrimSpace(name.(string)))
        }
        return nil
    }),
)
```

These modifiers (`Messages`, `Attributes`, `PrepareForValidation`) also work with `ctx.Request().Validate()`.

## Form Request Objects

For reusable, self-contained validation, define form request structs in `app/http/requests/`:

```go
package requests

import (
    "github.com/goravel/framework/contracts/http"
    "github.com/goravel/framework/contracts/validation"
)

type StorePostRequest struct {
    Name string `form:"name" json:"name"`
}

func (r *StorePostRequest) Authorize(ctx http.Context) error {
    return nil // return an error to reject the request
}

func (r *StorePostRequest) Rules(ctx http.Context) map[string]string {
    return map[string]string{
        "name": "required|max_len:255",
    }
}
```

Use in a controller:

```go
var req requests.StorePostRequest
errors, err := ctx.Request().ValidateRequest(&req)
// req.Name is auto-populated on success
```

Generate with: `./artisan make:request StorePostRequest`

### Optional Form Request Methods

| Method | Purpose |
|--------|---------|
| `Authorize(ctx) error` | Return non-nil error to reject (passed to `ValidateRequest` return) |
| `Rules(ctx) map[string]string` | Validation rules keyed by field name |
| `Filters(ctx) map[string]string` | Pre-validation data filters (e.g., `"name": "trim"`) |
| `Messages() map[string]string` | Custom error messages (e.g., `"title.required": "Title needed"`) |
| `Attributes() map[string]string` | Custom attribute names for `:attribute` placeholder |
| `PrepareForValidation(ctx, data) error` | Mutate data before validation runs |

### File Validation in Form Requests

```go
type UploadRequest struct {
    File  *multipart.FileHeader   `form:"file" json:"file"`
    Files []*multipart.FileHeader `form:"files" json:"files"`
}

func (r *UploadRequest) Rules(ctx http.Context) map[string]string {
    return map[string]string{
        "file":    "required|file",
        "files":   "required|array",
        "files.*": "required|file",
    }
}
```

## Working With Validated Input

Bind validated data to a struct:

```go
// After inline validation — binds ALL incoming data (including unvalidated fields)
validator, _ := ctx.Request().Validate(rules)
var user models.User
err := validator.Bind(&user)

// After form request validation — fields auto-bound to the request struct
var req requests.StorePostRequest
errors, err := ctx.Request().ValidateRequest(&req)
fmt.Println(req.Name)
```

## Error Messages API

```go
validator.Fails()                    // bool — did validation fail?
validator.Errors().One("email")      // string — one error for field (random if multiple)
validator.Errors().Get("email")      // []string — all errors for field
validator.Errors().All()             // map[string][]string — all errors, all fields
validator.Errors().Has("email")      // bool — does field have errors?
```

## Built-In Validation Rules

### Required Rules

| Rule | Description |
|------|-------------|
| `required` | Must be present and non-zero-value. `false` for bool fails. |
| `required_if:field,value,...` | Required if `field` equals any `value` |
| `required_unless:field,value,...` | Required unless `field` equals any `value` |
| `required_with:field1,field2,...` | Required if any listed field is present |
| `required_with_all:field1,field2,...` | Required if all listed fields are present |
| `required_without:field1,...` | Required if any listed field is absent |
| `required_without_all:field1,...` | Required if all listed fields are absent |

### Type Rules

| Rule | Description |
|------|-------------|
| `int` | Integer type. Supports size: `int`, `int:2`, `int:2,12` |
| `uint` | Unsigned integer, value >= 0 |
| `float` | Float type |
| `bool` | Bool string: true = `"1"/"on"/"yes"/"true"`, false = `"0"/"off"/"no"/"false"` |
| `string` | String type. Supports size: `string`, `string:2`, `string:2,12` |
| `slice` | Slice type (`[]intX`, `[]uintX`, `[]byte`, `[]string`) |
| `array` | Array or slice type |
| `map` | Map type |
| `number` | Numeric string >= 0 |
| `json` | Valid JSON string |

**Note on `int` rule:** When using `ctx.Request().Validate()`, JSON integers are parsed as `float64` by `json.Unmarshal`, causing `int` validation to fail. Fix: use `validation.PrepareForValidation` to cast, or use `facades.Validation().Make()` instead.

### Size & Comparison Rules

| Rule | Description |
|------|-------------|
| `max:value` | <= value (numeric types) |
| `min:value` | >= value (numeric types) |
| `between:min,max` | Number within range |
| `len:value` | Exact length (string/array/slice/map) |
| `min_len:value` | Minimum length |
| `max_len:value` | Maximum length |
| `eq:value` | Equal to value |
| `ne:value` | Not equal to value |
| `lt:value` | Less than value |
| `gt:value` | Greater than value |

### Field Comparison Rules

| Rule | Description |
|------|-------------|
| `eq_field:field` | Equal to another field's value |
| `ne_field:field` | Not equal to another field |
| `gt_field:field` | Greater than another field |
| `gte_field:field` | Greater than or equal to another field |
| `lt_field:field` | Less than another field |
| `lte_field:field` | Less than or equal to another field |

### String Rules

| Rule | Description |
|------|-------------|
| `email` | Valid email address |
| `alpha` | Alphabetic characters only |
| `alpha_num` | Letters and numbers only |
| `alpha_dash` | Letters, numbers, dashes, underscores |
| `starts_with:value` | Starts with substring |
| `ends_with:value` | Ends with substring |
| `regex:pattern` | Matches regex (e.g., `regex:^\d{4,6}$`) |

### Enumeration Rules

| Rule | Description |
|------|-------------|
| `in:foo,bar,...` | Value is in the list |
| `not_in:foo,bar,...` | Value is not in the list |

### Date Rules

| Rule | Description |
|------|-------------|
| `date` | Valid date string |
| `gt_date:value` | After the given date |
| `lt_date:value` | Before the given date |
| `gte_date:value` | On or after the given date |
| `lte_date:value` | On or before the given date |

### Network & ID Rules

| Rule | Description |
|------|-------------|
| `full_url` | Full URL (must start with http/https) |
| `ip` | Valid IPv4 or IPv6 |
| `ipv4` | Valid IPv4 |
| `ipv6` | Valid IPv6 |
| `uuid` | Valid UUID |
| `uuid3` | Valid UUID v3 |
| `uuid4` | Valid UUID v4 |
| `uuid5` | Valid UUID v5 |

### File Rules

| Rule | Description |
|------|-------------|
| `file` | Uploaded file |
| `image` | Uploaded image file (supports suffix check) |

## Custom Validation Rules

Generate: `./artisan make:rule Uppercase`

Implement `Signature()`, `Passes()`, and `Message()`:

```go
type Uppercase struct{}

func (r *Uppercase) Signature() string { return "uppercase" }

func (r *Uppercase) Passes(ctx context.Context, data validation.Data, val any, options ...any) bool {
    return strings.ToUpper(val.(string)) == val.(string)
}

func (r *Uppercase) Message(ctx context.Context) string {
    return "The :attribute must be uppercase."
}
```

Rules created via `make:rule` auto-register in `bootstrap/rules.go`. Manual rules require registration in `bootstrap/app.go` via `WithRules(Rules)`.

## Built-In Filters

Apply filters in form request `Filters()` method or via `validation.PrepareForValidation`. Filters transform data before validation.

| Filter | Description |
|--------|-------------|
| `int`/`toInt` | Convert to `int` |
| `uint`/`toUint` | Convert to `uint` |
| `int64`/`toInt64` | Convert to `int64` |
| `float`/`toFloat` | Convert to `float` |
| `bool`/`toBool` | Convert to `bool` |
| `trim`/`trimSpace` | Trim whitespace both sides |
| `ltrim`/`trimLeft` | Trim whitespace left |
| `rtrim`/`trimRight` | Trim whitespace right |
| `lower`/`lowercase` | Lowercase string |
| `upper`/`uppercase` | Uppercase string |
| `lcFirst`/`lowerFirst` | Lowercase first character |
| `ucFirst`/`upperFirst` | Uppercase first character |
| `ucWord`/`upperWord` | Uppercase first character of each word |
| `camel`/`camelCase` | camelCase conversion |
| `snake`/`snakeCase` | snake_case conversion |
| `escapeJs`/`escapeJS` | Escape JS string |
| `escapeHtml`/`escapeHTML` | Escape HTML string |
| `str2ints`/`strToInts` | String to `[]int` |
| `str2time`/`strToTime` | String to `time.Time` |
| `str2arr`/`strToArray` | String to `[]string` |

## Custom Filters

Generate: `./artisan make:filter ToSlug`

Implement `Signature()` and `Handle()`:

```go
type ToSlug struct{}

func (r *ToSlug) Signature() string { return "toSlug" }

func (r *ToSlug) Handle(ctx context.Context) any {
    return func(val any) string {
        return strings.ReplaceAll(strings.ToLower(val.(string)), " ", "-")
    }
}
```

Filters created via `make:filter` auto-register in `bootstrap/filters.go`. Manual filters require `WithFilters(Filters)` in `bootstrap/app.go`.
