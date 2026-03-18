---
name: goravel-data
description: Use when working with Goravel ORM models, database queries, migrations, seeding, or relationships
---

# Goravel Data

Goravel's data layer provides ORM models, a query builder, schema migrations, seeders, and factories. Access everything through `facades.Orm()` for ORM operations and `facades.DB()` for raw/query-builder operations.

**Version:** All patterns target Goravel v1.17.

## Defining Models

Models live in `app/models/`. Generate with `./artisan make:model User`.

```go
package models

import "github.com/goravel/framework/database/orm"

type User struct {
    orm.Model        // id, created_at, updated_at
    Name   string
    Email  string
    orm.SoftDeletes  // deleted_at (optional)
}
```

Conventions:
- Struct name = PascalCase singular (`UserOrder`)
- Table name = snake_case plural (`user_orders`), override with `TableName() string`
- Override default connection with `Connection() string`
- Use gorm tags for column config: `gorm:"type:text"`, `gorm:"column:custom_name"`

## Basic CRUD

All operations start from `facades.Orm().Query()`:

```go
// Create
user := models.User{Name: "tom", Email: "tom@example.com"}
err := facades.Orm().Query().Create(&user)

// Read
var user models.User
facades.Orm().Query().Find(&user, 1)                        // by ID
facades.Orm().Query().Where("email", "tom@example.com").First(&user)

// Update
user.Name = "Tim"
facades.Orm().Query().Save(&user)                            // full save
facades.Orm().Query().Model(&models.User{}).Where("id", 1).Update("name", "Tim") // column update

// Delete
facades.Orm().Query().Delete(&user)                          // soft delete if SoftDeletes embedded
facades.Orm().Query().Where("id", 1).ForceDelete(&models.User{}) // hard delete
```

## Query Methods

```go
facades.Orm().Query().Find(&user, 1)              // single by ID
facades.Orm().Query().Find(&users, []int{1,2,3})  // multiple by IDs
facades.Orm().Query().First(&user)                 // first row
facades.Orm().Query().Get(&users)                  // all matching rows
facades.Orm().Query().Pluck("email", &emails)      // single column
facades.Orm().Query().FindOrFail(&user, 1)         // error if not found
facades.Orm().Query().FirstOrCreate(&user, models.User{Name: "tom"})
facades.Orm().Query().UpdateOrCreate(&user, models.User{Name: "tom"}, models.User{Email: "new"})

// Aggregates
count, _ := facades.Orm().Query().Model(&models.User{}).Count()
facades.Orm().Query().Model(&models.User{}).Sum("age", &sum)
facades.Orm().Query().Model(&models.User{}).Max("age", &max)
facades.Orm().Query().Model(&models.User{}).Min("age", &min)
facades.Orm().Query().Model(&models.User{}).Average("age", &avg)

// Pagination
facades.Orm().Query().Paginate(1, 10, &users, &total)
```

See [orm-methods.md](./orm-methods.md) for the full ORM method reference.

## Relationships

Define relationships by embedding pointer fields with gorm tags:

```go
type User struct {
    orm.Model
    Name    string
    Phone   *Phone            // HasOne (Phone.UserID -> User.ID)
    Posts   []*Post           // HasMany (Post.UserID -> User.ID)
    Roles   []*Role `gorm:"many2many:role_user"` // ManyToMany
}

type Phone struct {
    orm.Model
    UserID uint
    User   *User             // BelongsTo (inverse)
}
```

Eager load with `With`:

```go
facades.Orm().Query().With("Posts").Find(&user, 1)
facades.Orm().Query().With("Author.Contacts").Find(&book) // nested
```

Polymorphic:

```go
type Post struct {
    orm.Model
    Image *Image `gorm:"polymorphic:Imageable"`
}
```

See [relationships-advanced.md](./relationships-advanced.md) for all relationship types, association operations, and eager loading patterns.

## Migrations

Generate: `./artisan make:migration create_users_table`

```go
func (r *M20241207095921CreateUsersTable) Up() error {
    return facades.Schema().Create("users", func(table schema.Blueprint) {
        table.ID()
        table.String("name").Nullable()
        table.String("email").Unique()
        table.Timestamps()
        table.SoftDeletes()
    })
}

func (r *M20241207095921CreateUsersTable) Down() error {
    return facades.Schema().DropIfExists("users")
}
```

Run: `./artisan migrate`, rollback: `./artisan migrate:rollback`, reset: `./artisan migrate:fresh`

See [migrations-reference.md](./migrations-reference.md) for all column types, modifiers, indexes, and foreign keys.

## Seeding

Generate: `./artisan make:seeder UserSeeder`

```go
func (s *UserSeeder) Run() error {
    user := models.User{Name: "admin"}
    return facades.Orm().Query().Create(&user)
}
```

Run: `./artisan db:seed`, specific: `./artisan db:seed --seeder=UserSeeder`

Call sub-seeders: `facades.Seeder().Call([]seeder.Seeder{&UserSeeder{}})`.

## Factories

Bind a factory to a model via `Factory() factory.Factory`, then use it:

```go
var user models.User
facades.Orm().Factory().Create(&user)                           // persist
facades.Orm().Factory().Count(5).Make(&users)                   // in-memory only
facades.Orm().Factory().Create(&user, map[string]any{"Name": "custom"}) // override attrs
```

## Transactions

```go
// Automatic
facades.Orm().Transaction(func(tx orm.Query) error {
    return tx.Create(&user)
})

// Manual
tx, _ := facades.Orm().Query().BeginTransaction()
if err := tx.Create(&user); err != nil {
    tx.Rollback()
} else {
    tx.Commit()
}
```

## Raw SQL (Query Builder)

Use `facades.DB()` for query-builder and raw SQL operations:

```go
facades.DB().Table("users").Where("age > ?", 18).Get(&users)
facades.DB().Select(&users, "SELECT * FROM users WHERE age > ?", 18)
facades.DB().Table("users").Where("id", 1).Update("name", "new")
```

See [query-builder.md](./query-builder.md) for the full query builder API.

## Related Skills

- See **goravel-foundations** skill for service providers and the service container (where ORM bindings are registered)
- See **goravel-security** skill for authentication guards and user model requirements (`facades.Auth()`)
- See **goravel-async** skill for caching query results (`facades.Cache()`) and queued database operations
- See **goravel-tooling** skill for database testing, factories in tests, and `RefreshDatabase()`

## Reference Files

| File | Contents |
|------|----------|
| [orm-methods.md](./orm-methods.md) | All ORM query methods, CRUD, soft deletes, scopes, events/observers, serialization |
| [query-builder.md](./query-builder.md) | Select, where clauses, joins, ordering, grouping, limit/offset, raw queries, aggregates, chunking, pagination |
| [migrations-reference.md](./migrations-reference.md) | Column types, modifiers, indexes, foreign keys, migration commands |
| [relationships-advanced.md](./relationships-advanced.md) | All relationship types, polymorphic, eager loading, association operations |
