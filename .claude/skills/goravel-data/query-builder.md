# Query Builder Reference

Complete reference for `facades.DB()` query builder. Goravel v1.17.

The query builder operates on raw tables (not models). Use `db` struct tags for column mapping. It does not auto-apply soft delete filters.

```go
type User struct {
    ID   string `db:"id"`
    Name string `db:"name"`
}
```

## Retrieving Records

```go
// All rows
var users []User
facades.DB().Table("users").Get(&users)

// First row
facades.DB().Table("users").Where("id", 1).First(&user)

// By ID
facades.DB().Table("users").Find(&user, 1)
facades.DB().Table("users").Find(&users, []int{1, 2, 3})

// FindOrFail / FirstOrFail (returns sql.ErrNoRows)
facades.DB().Table("users").FindOrFail(&user, 1)

// FirstOr (closure on not found)
facades.DB().Table("users").Where("name", "John").FirstOr(&user, func() error {
    return errors.New("not found")
})

// Single column value
var name string
facades.DB().Table("users").Where("id", 1).Value("name", &name)

// Pluck single column
var emails []string
facades.DB().Table("users").Pluck("email", &emails)
```

## Aggregates

```go
count, err := facades.DB().Table("users").Count()

var sum int
facades.DB().Table("users").Sum("age", &sum)

var avg float64
facades.DB().Table("users").Avg("age", &avg)

var min int
facades.DB().Table("users").Min("age", &min)

var max int
facades.DB().Table("users").Max("age", &max)
```

## Exists / DoesntExist

```go
exists, err := facades.DB().Table("users").Where("votes > ?", 100).Exists()
notExists, err := facades.DB().Table("users").Where("votes > ?", 100).DoesntExist()
```

## Select / Distinct

```go
facades.DB().Table("users").Select("name", "email as user_email").Get(&users)
facades.DB().Table("users").Distinct().Select("name").Get(&users)

// Subquery in select
facades.DB().Table("users").Select("name", db.Raw("(SELECT COUNT(*) FROM posts WHERE users.id = posts.user_id) as post_count")).Get(&users)
```

## Where Clauses

```go
// Basic
facades.DB().Table("users").Where("votes", 100)
facades.DB().Table("users").Where("votes >= ?", 100)
facades.DB().Table("users").Where("votes LIKE ?", "%goravel%")
facades.DB().Table("users").Where("votes", []int{1, 2, 3})    // IN

// OrWhere
facades.DB().Table("users").Where("name", "tom").OrWhere("name", "tim")

// WhereNot / OrWhereNot
facades.DB().Table("users").WhereNot("votes", 100)
facades.DB().Table("users").WhereNot(func(query db.Query) db.Query {
    return query.Where("age", 25)
})

// WhereBetween / WhereNotBetween
facades.DB().Table("users").WhereBetween("votes", 1, 100)
facades.DB().Table("users").WhereNotBetween("votes", 1, 100)

// WhereIn / WhereNotIn
facades.DB().Table("users").WhereIn("id", []any{1, 2, 3})

// WhereNull / WhereNotNull
facades.DB().Table("users").WhereNull("updated_at")

// WhereLike / WhereNotLike
facades.DB().Table("users").WhereLike("name", "%goravel%")

// WhereColumn (compare two columns)
facades.DB().Table("users").WhereColumn("first_name", "last_name")

// WhereAll / WhereAny / WhereNone
facades.DB().Table("products").WhereAll([]string{"weight", "height"}, "=", 200)
facades.DB().Table("users").WhereAny([]string{"name", "email"}, "=", "John")
facades.DB().Table("products").WhereNone([]string{"age", "score"}, ">", 18)

// WhereExists
facades.DB().Table("users").WhereExists(func() db.Query {
    return facades.DB().Table("posts").WhereColumn("posts.user_id", "users.id")
})
```

### Grouped Where (Logical Grouping)

```go
import "github.com/goravel/framework/contracts/database/db"

facades.DB().Table("users").Where("age", 25).Where(func(query db.Query) db.Query {
    return query.Where("votes", 100).OrWhere("votes", 200)
})
// WHERE age = 25 AND (votes = 100 OR votes = 200)
```

### Raw Where

```go
facades.DB().Table("users").WhereRaw("age = ? or age = ?", []any{25, 30}).Get(&users)
facades.DB().Table("users").OrWhereRaw("age = ?", []any{25}).Get(&users)
```

## Joins

```go
facades.DB().Table("users").Join("posts as p ON users.id = p.user_id AND p.id = ?", 1).Get(&users)
facades.DB().Table("users").LeftJoin("posts as p ON users.id = p.user_id").Get(&users)
facades.DB().Table("users").RightJoin("posts as p ON users.id = p.user_id").Get(&users)
facades.DB().Table("users").CrossJoin("posts as p ON users.id = p.user_id").Get(&users)
```

## Ordering

```go
facades.DB().Table("users").OrderBy("name")           // ASC
facades.DB().Table("users").OrderByDesc("name")        // DESC
facades.DB().Table("users").OrderByRaw("age DESC, id ASC")
facades.DB().Table("users").Latest()                    // ORDER BY created_at DESC
facades.DB().Table("users").Latest("updated_at")
facades.DB().Table("users").InRandomOrder()
```

## Grouping / Having

```go
facades.DB().Table("users").Where("age", 25).GroupBy("name").Having("name = ?", "John").OrderBy("name").Get(&users)
```

## Limit / Offset

```go
facades.DB().Table("users").Offset(10).Limit(5).Get(&users)
```

## Pagination

```go
var users []User
var total int64
facades.DB().Table("users").Where("name", "John").Paginate(1, 10, &users, &total)
```

## Conditional Clauses (When)

```go
facades.DB().Table("users").When(shouldFilter, func(query db.Query) db.Query {
    return query.Where("age", 25)
}).Get(&users)

// With else clause
facades.DB().Table("users").When(false, func(query db.Query) db.Query {
    return query.OrderBy("name")
}, func(query db.Query) db.Query {
    return query.OrderBy("id")
}).Get(&users)
```

## Insert

```go
// By struct
result, err := facades.DB().Table("products").Insert(Product{Name: "goravel"})

// Batch
result, err := facades.DB().Table("products").Insert([]Product{{Name: "a"}, {Name: "b"}})

// By map
result, err := facades.DB().Table("products").Insert(map[string]any{"name": "goravel"})

// Get auto-increment ID (mysql/sqlite only)
id, err := facades.DB().Table("products").InsertGetID(Product{Name: "goravel"})
```

## Update

```go
// Single column
result, err := facades.DB().Table("products").Where("id", 1).Update("name", "new")

// By struct
result, err := facades.DB().Table("products").Where("id", 1).Update(Product{Name: "new"})

// By map
result, err := facades.DB().Table("products").Where("id", 1).Update(map[string]any{"name": "new"})

// JSON fields
facades.DB().Table("users").Where("id", 1).Update("options->enabled", true)

// UpdateOrInsert
facades.DB().Table("users").Where("id", 1).UpdateOrInsert(
    map[string]any{"email": "john@example.com"},
    map[string]any{"phone": "123"},
)

// Increment / Decrement
facades.DB().Table("users").Where("id", 1).Increment("votes")
facades.DB().Table("users").Where("id", 1).Increment("votes", 5)
facades.DB().Table("users").Where("id", 1).Decrement("votes")
```

## Delete

```go
result, err := facades.DB().Table("users").Where("id", 1).Delete()
```

## Raw Expressions

```go
import "github.com/goravel/framework/database/db"

facades.DB().Model(&user).Update("age", db.Raw("age - ?", 1))
```

## Native SQL

```go
// Select
var users []User
facades.DB().Select(&users, "SELECT * FROM users WHERE id = ?", 1)

// Insert
facades.DB().Insert("INSERT INTO users (name) VALUES (?)", "Goravel")

// Update
facades.DB().Update("UPDATE users SET name = ? WHERE id = ?", "new", 1)

// Delete
facades.DB().Delete("DELETE FROM users WHERE id = ?", 1)

// Statement (DDL etc.)
facades.DB().Statement("DROP TABLE temp")
```

## Transactions

```go
import "github.com/goravel/framework/contracts/database/db"

// Automatic
facades.DB().Transaction(func(tx db.Tx) error {
    _, err := tx.Table("products").Insert(Product{Name: "item"})
    return err
})

// Manual
tx, _ := facades.DB().BeginTransaction()
_, err := tx.Insert("INSERT INTO users (name) VALUES (?)", "Goravel")
if err != nil { tx.Rollback() } else { tx.Commit() }
```

## Chunking / Cursor

```go
// Chunk -- process N rows at a time
facades.DB().Table("products").Chunk(100, func(rows []db.Row) error {
    for _, row := range rows {
        var p Product
        row.Scan(&p)
    }
    return nil
})

// Each -- traverse all
facades.DB().Table("products").Each(func(rows []db.Row) error { ... })

// Cursor -- one row at a time, minimal memory
rows, _ := facades.DB().Table("products").Cursor()
for row := range rows {
    var p Product
    row.Scan(&p)
}
```

## Pessimistic Locking

```go
facades.DB().Table("users").Where("votes > ?", 100).SharedLock().Get(&users)
facades.DB().Table("users").Where("votes > ?", 100).LockForUpdate().Get(&users)
```

## Multiple Connections

```go
facades.DB().Connection("postgres").Select(&user, "SELECT * FROM users WHERE id = ?", 1)
```

## Debugging

```go
facades.DB().Table("users").Where("id", 1).ToSql().Get(models.User{})    // placeholder SQL
facades.DB().Table("users").Where("id", 1).ToRawSql().Get(models.User{}) // bound SQL
```
