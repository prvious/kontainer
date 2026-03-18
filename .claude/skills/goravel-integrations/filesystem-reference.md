# Filesystem Reference

Complete reference for Goravel file storage (`facades.Storage()`). Goravel v1.17.

## Drivers

| Driver | Package |
|--------|---------|
| Local  | Built-in |
| S3     | `github.com/goravel/s3` |
| OSS    | `github.com/goravel/oss` |
| COS    | `github.com/goravel/cos` |
| Minio  | `github.com/goravel/minio` |

Configure in `config/filesystems.go`. The local driver stores files relative to its `root` setting (default: `storage/app`).

## Disk Selection

```go
facades.Storage().Put("file.txt", "data")              // default disk
facades.Storage().Disk("s3").Put("file.txt", "data")   // specific disk
facades.Storage().WithContext(ctx).Put("file.txt", "data") // with context
```

## Writing Files

```go
// Put string content
err := facades.Storage().Put("file.txt", "contents")

// Put file with auto-generated name (returns path)
file, _ := filesystem.NewFile("./logo.png")
path := facades.Storage().PutFile("photos", file)

// Put file with specific name
path := facades.Storage().PutFileAs("photos", file, "photo.jpg")
```

`PutFile` generates a unique filename from the file's MIME type. `PutFileAs` uses the given name (appends extension from MIME if name has no suffix).

## Reading Files

```go
content, err := facades.Storage().Get("file.txt")          // string contents
exists := facades.Storage().Exists("file.txt")              // bool
missing := facades.Storage().Missing("file.txt")            // bool
```

## URLs

```go
url := facades.Storage().Url("file.jpg")
// Local: /storage/file.jpg (relative)
// S3: fully qualified URL

// Temporary URL (non-local drivers only)
url, err := facades.Storage().TemporaryUrl("file.jpg", time.Now().Add(5*time.Minute))
```

## File Metadata

```go
size, err := facades.Storage().Size("file.jpg")              // int64
modTime, err := facades.Storage().LastModified("file.jpg")   // time.Time
mime, err := facades.Storage().MimeType("file.jpg")          // string
path := facades.Storage().Path("file.jpg")                   // relative path
```

Also available via `filesystem.NewFile()`:

```go
file, _ := filesystem.NewFile("./logo.png")
size, _ := file.Size()
modTime, _ := file.LastModified()
mime, _ := file.MimeType()
```

## Copy and Move

```go
err := facades.Storage().Copy("old/file.jpg", "new/file.jpg")
err := facades.Storage().Move("old/file.jpg", "new/file.jpg")
```

## Deleting Files

```go
err := facades.Storage().Delete("file.jpg")
err := facades.Storage().Delete("file1.jpg", "file2.jpg") // variadic
err := facades.Storage().Disk("s3").Delete("file.jpg")
```

## Directories

```go
// List files
files, err := facades.Storage().Files("directory")       // top-level only
files, err := facades.Storage().AllFiles("directory")     // recursive

// List directories
dirs, err := facades.Storage().Directories("directory")
dirs, err := facades.Storage().AllDirectories("directory")

// Create
err := facades.Storage().MakeDirectory("new-dir")

// Delete (with all contents)
err := facades.Storage().DeleteDirectory("old-dir")
```

## File Uploads

From HTTP request context:

```go
func (r *Controller) Upload(ctx http.Context) http.Response {
    file, err := ctx.Request().File("avatar")

    // Auto-generated name
    path, err := file.Store("avatars")

    // Specific name
    path, err := file.StoreAs("avatars", "profile")

    // Specific disk
    path, err := file.Disk("s3").Store("avatars")

    // File info
    name := file.GetClientOriginalName()
    ext := file.GetClientOriginalExtension()
    hash := file.HashName()        // safe random name
    safeExt, _ := file.Extension() // from MIME type
}
```

Prefer `HashName()` and `Extension()` over `GetClientOriginalName()`/`GetClientOriginalExtension()` -- client-provided names may be tampered.

## Public Disk

Serve files from `storage/app/public` via a static route:

```go
facades.Route().Static("storage", "./storage/app/public")
```

## Custom Drivers

Set `"driver": "custom"` in config, provide a `"via"` that implements `filesystem.Driver`:

```go
"custom": map[string]any{
    "driver": "custom",
    "via":    NewMyDriver(),
},
```

The `filesystem.Driver` interface:

```go
type Driver interface {
    AllDirectories(path string) ([]string, error)
    AllFiles(path string) ([]string, error)
    Copy(oldFile, newFile string) error
    Delete(file ...string) error
    DeleteDirectory(directory string) error
    Directories(path string) ([]string, error)
    Exists(file string) bool
    Files(path string) ([]string, error)
    Get(file string) (string, error)
    GetBytes(file string) ([]byte, error)
    LastModified(file string) (time.Time, error)
    MakeDirectory(directory string) error
    MimeType(file string) (string, error)
    Missing(file string) bool
    Move(oldFile, newFile string) error
    Path(file string) string
    Put(file, content string) error
    PutFile(path string, source File) (string, error)
    PutFileAs(path string, source File, name string) (string, error)
    Size(file string) (int64, error)
    TemporaryUrl(file string, time time.Time) (string, error)
    WithContext(ctx context.Context) Driver
    Url(file string) string
}
```

Use `facades.Config().Env()` inside custom drivers (config is not yet loaded at registration time).
