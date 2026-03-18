# Mail Reference

Complete reference for Goravel mail (`facades.Mail()`). Goravel v1.17.

## Configuration

Configure in `config/mail.go`. Set `MAIL_FROM_ADDRESS` and `MAIL_FROM_NAME` as global sender defaults. The sender address must match the configured SMTP.

## Sending Mail

Fluent API -- chain methods then call `Send()`:

```go
import "github.com/goravel/framework/mail"

err := facades.Mail().
    To([]string{"user@example.com"}).
    Cc([]string{"cc@example.com"}).
    Bcc([]string{"bcc@example.com"}).
    Subject("Order Shipped").
    Content(mail.Html("<h1>Hello Goravel</h1>")).
    Headers(map[string]string{"X-Mailer": "Goravel"}).
    Attach([]string{"invoice.pdf"}).
    Send()
```

### All fluent methods

| Method | Signature | Purpose |
|--------|-----------|---------|
| `To` | `[]string` | Recipients |
| `Cc` | `[]string` | Carbon copy |
| `Bcc` | `[]string` | Blind carbon copy |
| `Subject` | `string` | Email subject |
| `Content` | `*mail.Content` | Body (HTML or template) |
| `Headers` | `map[string]string` | Custom headers |
| `Attach` | `[]string` | File paths to attach |
| `From` | `mail.Address(addr, name)` | Override sender |
| `Send` | — | Send synchronously |
| `Queue` | `...mail.Queue` | Send via queue |

## Custom Sender

Override the global sender per email:

```go
facades.Mail().
    From(mail.Address("noreply@example.com", "Example")).
    To([]string{"user@example.com"}).
    Subject("Test").
    Content(mail.Html("<p>Hello</p>")).
    Send()
```

## Queued Mail

Replace `Send()` with `Queue()` for async delivery:

```go
// Default queue
err := facades.Mail().To([]string{"user@example.com"}).
    Subject("Test").
    Content(mail.Html("<p>Hello</p>")).
    Queue()

// Custom queue/connection
err := facades.Mail().To([]string{"user@example.com"}).
    Subject("Test").
    Content(mail.Html("<p>Hello</p>")).
    Queue(mail.Queue().Connection("redis").Queue("mail"))
```

## Mailable Structs

Encapsulate email configuration in a struct. Generate: `./artisan make:mail OrderShipped`. Stored in `app/mails/`.

```go
package mails

import "github.com/goravel/framework/contracts/mail"

type OrderShipped struct{}

func NewOrderShipped() *OrderShipped { return &OrderShipped{} }

func (m *OrderShipped) Envelope() *mail.Envelope {
    return &mail.Envelope{
        From:    mail.From{Address: "from@example.com", Name: "Shop"},
        To:      []string{"customer@example.com"},
        Cc:      []string{"support@example.com"},
        Bcc:     []string{"archive@example.com"},
        Subject: "Your Order Has Shipped",
    }
}

func (m *OrderShipped) Content() *mail.Content {
    return &mail.Content{Html: "<h1>Order shipped!</h1>"}
}

func (m *OrderShipped) Attachments() []string {
    return []string{"receipt.pdf"}
}

func (m *OrderShipped) Headers() map[string]string {
    return map[string]string{"X-Mailer": "Goravel"}
}

func (m *OrderShipped) Queue() *mail.Queue {
    return &mail.Queue{Connection: "redis", Queue: "mail"}
}
```

Send a mailable:

```go
err := facades.Mail().Send(mails.NewOrderShipped())
err := facades.Mail().Queue(mails.NewOrderShipped())
```

## Templates

### Configuration

Enable template support in `config/mail.go`:

```go
"template": map[string]any{
    "default": config.Env("MAIL_TEMPLATE_ENGINE", "html"),
    "engines": map[string]any{
        "html": map[string]any{
            "driver": "html",
            "path":   config.Env("MAIL_VIEWS_PATH", "resources/views/mail"),
        },
    },
},
```

### Creating Templates

Use Go's `html/template` syntax:

```html
<!-- resources/views/mail/welcome.html -->
<h1>Welcome {{.Name}}!</h1>
<p>Thank you for joining {{.AppName}}.</p>
```

### Sending with Templates

```go
facades.Mail().
    To([]string{"user@example.com"}).
    Subject("Welcome").
    Content(mail.Content{
        View: "welcome.tmpl",
        With: map[string]any{
            "Name":    "John",
            "AppName": "Goravel",
        },
    }).
    Send()
```

### Custom Template Engines

Register a custom engine in config:

```go
"template": map[string]any{
    "default": "blade",
    "engines": map[string]any{
        "blade": map[string]any{
            "driver": "custom",
            "via": func() (mail.Template, error) {
                return NewBladeEngine(), nil
            },
        },
    },
},
```
