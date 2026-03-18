# Artisan Reference

## Writing Commands

Generate: `./artisan make:command SendEmails` or `./artisan make:command user/SendEmails` (subdirectory).

Implement four methods on a struct:

```go
package commands

import (
    "github.com/goravel/framework/contracts/console"
    "github.com/goravel/framework/contracts/console/command"
)

type SendEmails struct{}

func (r *SendEmails) Signature() string   { return "send:emails" }
func (r *SendEmails) Description() string { return "Send emails" }

func (r *SendEmails) Extend() command.Extend {
    return command.Extend{
        Category: "mail", // groups in `./artisan list`
    }
}

func (r *SendEmails) Handle(ctx console.Context) error {
    return nil
}
```

Register in `bootstrap/app.go` via `WithCommands`. Auto-generated commands auto-register in `bootstrap/commands.go`.

## Arguments

Define in `Extend().Arguments`. Single-value types: `ArgumentString`, `ArgumentInt`, `ArgumentFloat64`, `ArgumentBool`, etc. Slice types: `ArgumentStringSlice`, `ArgumentIntSlice`, etc.

```go
func (r *SendEmails) Extend() command.Extend {
    return command.Extend{
        Arguments: []command.Argument{
            &command.ArgumentString{
                Name: "subject", Usage: "email subject", Required: true,
            },
            &command.ArgumentStringSlice{
                Name: "emails", Usage: "recipients", Min: 1, Max: -1, // -1 = unlimited
            },
        },
    }
}
```

Single-value argument fields: `Name`, `Value` (default), `Usage`, `Required`.

Slice argument fields: `Name`, `Value` (default), `Usage`, `Min`, `Max`.

Timestamp arguments also support `Layouts []string` for custom time formats.

Retrieve:

```go
subject := ctx.ArgumentString("subject")
emails := ctx.ArgumentStringSlice("emails")
// Positional: ctx.Argument(0), ctx.Arguments()
```

## Flags (Options)

Define in `Extend().Flags`. Prefixed with `--` on CLI, aliased with single `-`.

```go
Flags: []command.Flag{
    &command.StringFlag{Name: "lang", Value: "en", Aliases: []string{"l"}, Usage: "language"},
    &command.BoolFlag{Name: "force", Aliases: []string{"f"}, Usage: "skip confirmation"},
    &command.IntFlag{Name: "retries", Value: 3},
}
```

Types: `StringFlag`, `StringSliceFlag`, `BoolFlag`, `Float64Flag`, `Float64SliceFlag`, `IntFlag`, `IntSliceFlag`, `Int64Flag`, `Int64SliceFlag`.

Retrieve: `ctx.Option("lang")` returns string. Use typed flag with matching retrieval.

Usage: `./artisan send:emails --lang Chinese` or `./artisan send:emails -l Chinese`.

## Prompting for Input

### Ask

```go
name, err := ctx.Ask("What is your name?", console.AskOption{
    Default:     "Alice",
    Placeholder: "Enter name...",
    Validate:    func(s string) error { ... },
    Multiple:    false, // true for multiline
    Lines:       5,     // for multiline
    Limit:       100,   // character limit
})
```

### Secret (hidden input)

```go
password, err := ctx.Secret("Password?", console.SecretOption{
    Validate: func(s string) error {
        if len(s) < 8 { return errors.New("too short") }
        return nil
    },
})
```

### Confirm

```go
if ctx.Confirm("Continue?", console.ConfirmOption{
    Default:     true,
    Affirmative: "Yes",
    Negative:    "No",
}) {
    // proceed
}
```

### Choice (single select)

```go
options := []console.Choice{
    {Key: "go", Value: "Go"},
    {Key: "py", Value: "Python", Selected: true},
}
selected, err := ctx.Choice("Pick language:", options, console.ChoiceOption{
    Default: "go",
})
```

### MultiSelect

```go
selected, err := ctx.MultiSelect("Pick languages:", options, console.MultiSelectOption{
    Default:    []string{"go"},
    Filterable: true, // type `/` to filter
    Limit:      3,    // max selections
})
```

## Output Formatting

Semantic output (colored automatically):

```go
ctx.Comment("comment")  // gray
ctx.Info("info")        // blue
ctx.Error("error")      // red
ctx.Warning("warning")  // yellow
ctx.Line("plain")       // no color
```

Direct color:

```go
ctx.Green("text")    ctx.Greenln("line")
ctx.Red("text")      ctx.Redln("line")
ctx.Yellow("text")   ctx.Yellowln("line")
ctx.Black("text")    ctx.Blackln("line")
```

Newlines: `ctx.NewLine()`, `ctx.NewLine(3)`.

Divider: `ctx.Divider()` (dashes), `ctx.Divider("=>")` (custom char).

### Progress Bars

```go
// Auto-advancing
items := []any{"a", "b", "c"}
_, err := ctx.WithProgressBar(items, func(item any) error {
    return nil
})

// Manual
bar := ctx.CreateProgressBar(total)
bar.Start()
for range items { bar.Advance() }
bar.Finish()
```

### Spinner

```go
err := ctx.Spinner("Processing...", console.SpinnerOption{
    Action: func() error {
        time.Sleep(2 * time.Second)
        return nil
    },
})
```

## Programmatic Execution

```go
facades.Artisan().Call("send:emails")
facades.Artisan().Call("send:emails --lang Chinese user@test.com")
```

Disable color output: `./artisan list --no-ansi`.

## Color Package

For standalone terminal coloring outside commands:

```go
import "github.com/goravel/framework/support/color"

color.Blue().Println("info")
color.Green().Printf("Hello, %s!", name)
color.New(color.FgRed).Println("custom")
```

Printer methods: `Print`, `Println`, `Printf`, `Sprint`, `Sprintln`, `Sprintf`.

Colors: `Red`, `Green`, `Yellow`, `Blue`, `Magenta`, `Cyan`, `White`, `Black`, `Gray`, `Default`.

## String Utilities

Fluent string manipulation. Import `github.com/goravel/framework/support/str`.

```go
str.Of("hello_world").Camel().String()    // "helloWorld"
str.Of("HelloWorld").Snake().String()     // "hello_world"
str.Of("HelloWorld").Kebab().String()     // "hello-world"
str.Of("hello_world").Studly().String()   // "HelloWorld"
str.Of("hello world").Title().String()    // "Hello World"
str.Of("  text  ").Trim().String()        // "text"
str.Of("UPPER").Lower().String()          // "upper"
str.Of("lower").Upper().String()          // "LOWER"
```

Inspection: `.Contains("sub")`, `.StartsWith("pre")`, `.EndsWith("suf")`, `.IsEmpty()`, `.IsUuid()`, `.IsUlid()`, `.Length()`.

Manipulation: `.After("X")`, `.Before("X")`, `.Between("[", "]")`, `.Replace("old", "new")`, `.Remove("text")`, `.Limit(10)`, `.Words(5)`, `.Substr(start, len)`, `.Mask("*", 3)`, `.Plural()`, `.Singular()`.

Conditional: `.When(cond, fn)`, `.WhenEmpty(fn)`, `.WhenContains("sub", fn)`.

Split/join: `.Explode(" ")`, `.Split(" ")`.

Pattern matching: `.Match(regex)`, `.MatchAll(regex)`, `.IsMatch(patterns...)`.

## Pluralization

```go
import "github.com/goravel/framework/support/pluralizer"

pluralizer.Plural("goose")   // "geese"
pluralizer.Singular("cars")  // "car"
```

Register custom rules at app boot:

```go
import "github.com/goravel/framework/support/pluralizer/rules"

pluralizer.RegisterIrregular("english", rules.NewSubstitution("mouse", "mice"))
pluralizer.RegisterUninflected("english", "sheep")
```

Language support: `pluralizer.UseLanguage("spanish")`, `pluralizer.RegisterLanguage(&MyLang{})`.

Also available as fluent string methods: `str.Of("goose").Plural().String()`.

## Process Execution

Run external commands via `facades.Process()`:

```go
// Blocking
result := facades.Process().Run("ls", "-la")
result.Output()      // stdout
result.ErrorOutput() // stderr
result.ExitCode()    // int
result.Failed()      // bool

// String command (auto-wraps with shell)
result := facades.Process().Run("echo Hello | tr a-z A-Z")

// With options
result := facades.Process().
    Path("/var/www").
    Timeout(10 * time.Minute).
    Env(map[string]string{"KEY": "val"}).
    Input(strings.NewReader("stdin data")).
    Run("cmd", "args")

// Stream output
facades.Process().OnOutput(func(typ process.OutputType, b []byte) {
    fmt.Print(string(b))
}).Run("tail", "-f", "log.txt")

// Async
running, _ := facades.Process().Start("sleep", "60")
running.PID()
running.Stop(5 * time.Second)
result := running.Wait()

// Pipelines
result := facades.Process().Pipe(func(pipe process.Pipe) {
    pipe.Command("echo", "hello").As("source")
    pipe.Command("tr", "a-z", "A-Z").As("upper")
}).Run()

// Concurrent pool
results, _ := facades.Process().Pool(func(pool process.Pool) {
    pool.Command("task1").As("first")
    pool.Command("task2").As("second")
}).Concurrency(2).Timeout(1 * time.Minute).Run()
```

Result methods: `.Output()`, `.ErrorOutput()`, `.ExitCode()`, `.Failed()`, `.Error()`, `.Command()`, `.SeeInOutput("str")`.

## Helper Packages Quick Reference

| Package | Import | Purpose |
|---------|--------|---------|
| `path` | `support/path` | `path.App()`, `path.Base()`, `path.Config()`, `path.Storage()`, `path.Database()`, `path.Public()` |
| `carbon` | `support/carbon` | Date/time: `carbon.Now()`, `carbon.Parse("2020-01-01")`, `carbon.SetTimezone(carbon.UTC)` |
| `debug` | `support/debug` | `debug.Dump(vars...)`, `debug.SDump(vars...)` for string, `debug.FDump(writer, vars...)` |
| `collect` | `support/collect` | Slice ops: `Map`, `Filter`, `Each`, `GroupBy`, `Unique`, `Count`, `Sum`, `Min`, `Max`, `Merge`, `Split` |
| `maps` | `support/maps` | Map ops: `Get`, `Set`, `Add`, `Has`, `HasAny`, `Exists`, `Forget`, `Pull`, `Only`, `Where` |
| `convert` | `support/convert` | `Tap(val, fn)`, `With(val, fn)`, `Transform(val, fn)`, `Default("", "fallback")`, `Pointer(val)` |
