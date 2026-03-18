# ORM Methods Reference

Complete reference for `facades.Orm().Query()` methods. Goravel v1.17.

## Getting a Query Instance

```go
facades.Orm().Query()                         // default connection
facades.Orm().Connection("mysql").Query()     // named connection
facades.Orm().WithContext(ctx).Query()        // with context
```

## Retrieving Records

```go
// Single by ID
facades.Orm().Query().Find(&user, 1)
facades.Orm().Query().Find(&user, "uuid=?", "abc")     // custom PK

// Multiple by IDs
facades.Orm().Query().Find(&users, []int{1, 2, 3})

// First row
facades.Orm().Query().First(&user)
facades.Orm().Query().Where("name", "tom").First(&user)

// First or error
facades.Orm().Query().FindOrFail(&user, 1)              // returns error if not found
facades.Orm().Query().FirstOrFail(&user)                 // errors.OrmRecordNotFound

// First or callback
facades.Orm().Query().Where("name", "tom").FirstOr(&user, func() error {
    user.Name = "default"
    return nil
})

// All matching rows
facades.Orm().Query().Where("active", true).Get(&users)

// Single column
facades.Orm().Query().Model(&models.User{}).Pluck("email", &emails)

// Exists check
exists, err := facades.Orm().Query().Model(&models.User{}).Where("name", "tom").Exists()
```

## Retrieve or Create

```go
// Find by conditions, create if missing (persists)
facades.Orm().Query().Where("gender", 1).FirstOrCreate(&user, models.User{Name: "tom"})

// With additional creation attrs
facades.Orm().Query().FirstOrCreate(&user, models.User{Name: "tom"}, models.User{Avatar: "pic"})

// Find by conditions, instantiate if missing (does NOT persist)
facades.Orm().Query().FirstOrNew(&user, models.User{Name: "tom"})
```

## Create

```go
user := models.User{Name: "tom"}
err := facades.Orm().Query().Create(&user)          // auto-sets created_at, updated_at

// Batch create
users := []models.User{{Name: "a"}, {Name: "b"}}
err := facades.Orm().Query().Create(&users)

// Create via map (with Model: triggers events; with Table: no events)
facades.Orm().Query().Model(&models.User{}).Create(map[string]any{"name": "tom"})
facades.Orm().Query().Table("users").Create(map[string]any{"name": "tom"})
```

## Save (Update Existing)

```go
facades.Orm().Query().First(&user)
user.Name = "new name"
facades.Orm().Query().Save(&user)        // updates all fields
facades.Orm().Query().SaveQuietly(&user) // no events
```

## Update

```go
// Single column
facades.Orm().Query().Model(&models.User{}).Where("id", 1).Update("name", "new")

// Multiple columns via struct (only non-zero fields)
facades.Orm().Query().Model(&models.User{}).Where("id", 1).Update(models.User{Name: "new", Age: 25})

// Multiple columns via map (all specified fields)
facades.Orm().Query().Model(&models.User{}).Where("id", 1).Update(map[string]any{"name": "new", "age": 0})

// Update or create
facades.Orm().Query().UpdateOrCreate(&user, models.User{Name: "tom"}, models.User{Avatar: "pic"})

// JSON fields
facades.Orm().Query().Model(&models.User{}).Where("id", 1).Update("options->enabled", true)

// Raw expression
import "github.com/goravel/framework/database/db"
facades.Orm().Query().Model(&user).Update("age", db.Raw("age + ?", 1))
```

> When updating with a struct, zero-value fields are ignored. Use a `map` to set fields to zero.

## Delete

```go
// Soft delete (if SoftDeletes embedded)
res, err := facades.Orm().Query().Delete(&user)
res, err := facades.Orm().Query().Model(&models.User{}).Where("id", 1).Delete()
num := res.RowsAffected

// Force delete (bypass soft deletes)
facades.Orm().Query().Where("id", 1).ForceDelete(&models.User{})
facades.Orm().Query().Model(&models.User{}).Where("id", 1).ForceDelete()

// Delete with associations
facades.Orm().Query().Select("Account").Delete(&user)        // delete user's Account too
facades.Orm().Query().Select(orm.Associations).Delete(&user) // all associations
```

## Soft Deletes

Embed `orm.SoftDeletes` in the model struct. Queries automatically exclude soft-deleted rows.

```go
// Include soft-deleted
facades.Orm().Query().WithTrashed().First(&user)

// Restore
facades.Orm().Query().WithTrashed().Restore(&models.User{ID: 1})
// or
facades.Orm().Query().Model(&models.User{ID: 1}).WithTrashed().Restore()
```

## Where Clauses

```go
facades.Orm().Query().Where("name", "tom")
facades.Orm().Query().Where("age > ?", 18)
facades.Orm().Query().Where("id", []int{1, 2, 3})           // IN
facades.Orm().Query().OrWhere("name", "tim")
facades.Orm().Query().WhereBetween("age", 18, 30)
facades.Orm().Query().WhereNotBetween("age", 18, 30)
facades.Orm().Query().WhereIn("id", []any{1, 2, 3})
facades.Orm().Query().WhereNotIn("id", []any{4, 5})
facades.Orm().Query().WhereNull("deleted_at")
facades.Orm().Query().OrWhereNull("name")

// Grouped where (parentheses)
facades.Orm().Query().Where("age", 25).Where(func(query orm.Query) orm.Query {
    return query.Where("votes", 100).OrWhere("votes", 200)
})

// JSON queries
facades.Orm().Query().Where("options->languages[0]", "en").First(&user)
facades.Orm().Query().WhereJsonContains("options->languages", "en").First(&user)

// WhereAll / WhereAny / WhereNone
facades.Orm().Query().WhereAll([]string{"weight", "height"}, "=", 200)
facades.Orm().Query().WhereAny([]string{"name", "email"}, "=", "John")
facades.Orm().Query().WhereNone([]string{"age", "score"}, ">", 18)
```

## Ordering, Grouping, Limit/Offset

```go
facades.Orm().Query().Order("name asc")
facades.Orm().Query().OrderBy("name")           // asc
facades.Orm().Query().OrderByDesc("name")        // desc
facades.Orm().Query().InRandomOrder()

facades.Orm().Query().Model(&models.User{}).Select("name", "sum(age) as total").Group("name").Having("name = ?", "tom")

facades.Orm().Query().Offset(10).Limit(5).Get(&users)
```

## Pagination

```go
var users []models.User
var total int64
facades.Orm().Query().Paginate(1, 10, &users, &total)
```

## Aggregates

```go
count, _ := facades.Orm().Query().Model(&models.User{}).Count()

var sum int
facades.Orm().Query().Model(models.User{}).Sum("age", &sum)

var avg float64
facades.Orm().Query().Model(models.User{}).Average("age", &avg)

var max int
facades.Orm().Query().Model(models.User{}).Max("age", &max)

var min int
facades.Orm().Query().Model(models.User{}).Min("age", &min)
```

## Cursor (Memory-Efficient Iteration)

```go
cursor, err := facades.Orm().Query().Model(models.User{}).Cursor()
for row := range cursor {
    var user models.User
    row.Scan(&user)
}
```

## Select / Distinct

```go
facades.Orm().Query().Select("name", "age").Get(&users)
facades.Orm().Query().Distinct("name").Find(&users)
```

## Scopes

Reusable query constraints:

```go
func Active(query orm.Query) orm.Query {
    return query.Where("active", true)
}

facades.Orm().Query().Scopes(Active).Get(&users)
```

Global scopes on a model via `GlobalScopes()` method; remove with `WithoutGlobalScopes()`.

## Transactions

```go
// Auto commit/rollback
facades.Orm().Transaction(func(tx orm.Query) error {
    return tx.Create(&user)
})

// Manual
tx, _ := facades.Orm().Query().BeginTransaction()
tx.Create(&user)
tx.Commit() // or tx.Rollback()
```

## Events / Observers

Model lifecycle events: `Retrieved`, `Creating`, `Created`, `Updating`, `Updated`, `Saving`, `Saved`, `Deleting`, `Deleted`, `ForceDeleting`, `ForceDeleted`, `Restoring`, `Restored`.

Inline via `DispatchesEvents()` on the model:

```go
func (u *User) DispatchesEvents() map[contractsorm.EventType]func(contractsorm.Event) error {
    return map[contractsorm.EventType]func(contractsorm.Event) error{
        contractsorm.EventCreating: func(event contractsorm.Event) error {
            // pre-create logic
            return nil
        },
    }
}
```

Or use observers (generate: `./artisan make:observer UserObserver`), register in `bootstrap/app.go`:

```go
facades.Orm().Observe(models.User{}, &observers.UserObserver{})
```

Event parameter methods: `Context()`, `GetAttribute(field)`, `GetOriginal(field)`, `SetAttribute(field, val)`, `IsDirty(fields...)`, `IsClean(fields...)`, `Query()`.

Mute events: `facades.Orm().Query().WithoutEvents().Find(&user, 1)`

## Debugging

```go
facades.Orm().Query().ToSql().Get(models.User{})    // SQL with placeholders
facades.Orm().Query().ToRawSql().Get(models.User{})  // SQL with bound values
```

## Raw SQL via ORM

```go
var result Result
facades.Orm().Query().Raw("SELECT id, name FROM users WHERE id = ?", 1).Scan(&result)

res, err := facades.Orm().Query().Exec("DROP TABLE temp")
res.RowsAffected
```

## Driver Detection

```go
driver := facades.Orm().Query().Driver()
if driver == orm.DriverSqlite { ... }
```
