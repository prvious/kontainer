# Migrations Reference

Complete reference for Goravel schema migrations. v1.17.

## Creating Migrations

```bash
./artisan make:migration create_users_table       # auto-generates Create table scaffold
./artisan make:migration add_avatar_to_users_table # auto-generates Add column scaffold
./artisan make:migration custom_name               # empty migration
./artisan make:migration create_users_table -m User # based on model struct
```

Migrations are stored in `database/migrations/` and auto-registered in `bootstrap/migrations.go`.

## Migration Structure

```go
package migrations

import (
    "github.com/goravel/framework/contracts/database/schema"
    "goravel/app/facades"
)

type M20241207095921CreateUsersTable struct{}

func (r *M20241207095921CreateUsersTable) Signature() string {
    return "20241207095921_create_users_table"
}

func (r *M20241207095921CreateUsersTable) Up() error {
    if !facades.Schema().HasTable("users") {
        return facades.Schema().Create("users", func(table schema.Blueprint) {
            table.ID()
            table.String("name").Nullable()
            table.String("email").Unique()
            table.Timestamps()
            table.SoftDeletes()
        })
    }
    return nil
}

func (r *M20241207095921CreateUsersTable) Down() error {
    return facades.Schema().DropIfExists("users")
}
```

Override connection with `Connection() string` method.

Register in `bootstrap/app.go`:

```go
foundation.Setup().WithMigrations(Migrations).WithConfig(config.Boot).Create()
```

## Running Migrations

```bash
./artisan migrate                     # run all pending
./artisan migrate:status              # show which have run
./artisan migrate:rollback            # rollback latest batch
./artisan migrate:rollback --step=5   # rollback last 5
./artisan migrate:rollback --batch=2  # rollback specific batch
./artisan migrate:reset               # rollback all
./artisan migrate:refresh             # rollback all + re-migrate
./artisan migrate:refresh --step=5
./artisan migrate:fresh               # drop all tables + migrate
./artisan migrate:fresh --seed        # fresh + seed
```

## Table Operations

```go
// Create
facades.Schema().Create("users", func(table schema.Blueprint) { ... })

// Modify existing
facades.Schema().Table("users", func(table schema.Blueprint) {
    table.String("avatar").Nullable()
})

// Rename
facades.Schema().Rename("users", "new_users")

// Drop
facades.Schema().Drop("users")
facades.Schema().DropIfExists("users")

// Check existence
facades.Schema().HasTable("users")
facades.Schema().HasColumn("users", "email")
facades.Schema().HasColumns("users", []string{"name", "email"})
facades.Schema().HasIndex("users", "email_unique")

// Table comment
table.Comment("user accounts")

// Specific connection
facades.Schema().Connection("sqlite").Create("users", func(table schema.Blueprint) { ... })
```

## Column Types

### Numeric

| Method | SQL Type |
|--------|----------|
| `table.ID()` / `table.ID("custom_id")` | BIGINT auto-increment PK |
| `table.BigIncrements("id")` | BIGINT auto-increment |
| `table.Increments("id")` | INT auto-increment |
| `table.IntegerIncrements("id")` | INT auto-increment |
| `table.MediumIncrements("id")` | MEDIUMINT auto-increment |
| `table.SmallIncrements("id")` | SMALLINT auto-increment |
| `table.TinyIncrements("id")` | TINYINT auto-increment |
| `table.BigInteger("votes")` | BIGINT |
| `table.Integer("votes")` | INT |
| `table.MediumInteger("votes")` | MEDIUMINT |
| `table.SmallInteger("votes")` | SMALLINT |
| `table.TinyInteger("votes")` | TINYINT |
| `table.UnsignedBigInteger("votes")` | BIGINT UNSIGNED |
| `table.UnsignedInteger("votes")` | INT UNSIGNED |
| `table.UnsignedMediumInteger("votes")` | MEDIUMINT UNSIGNED |
| `table.UnsignedSmallInteger("votes")` | SMALLINT UNSIGNED |
| `table.UnsignedTinyInteger("votes")` | TINYINT UNSIGNED |
| `table.Decimal("amount")` | DECIMAL |
| `table.Double("amount")` | DOUBLE |
| `table.Float("amount")` | FLOAT |

### String & Text

| Method | SQL Type |
|--------|----------|
| `table.String("name")` | VARCHAR(255) |
| `table.Char("code")` | CHAR |
| `table.Text("bio")` | TEXT |
| `table.TinyText("note")` | TINYTEXT |
| `table.MediumText("description")` | MEDIUMTEXT |
| `table.LongText("content")` | LONGTEXT |
| `table.Json("options")` | JSON |
| `table.Uuid("id")` | UUID / CHAR(36) |
| `table.Ulid("id")` | ULID / CHAR(26) |

### Boolean

| Method | SQL Type |
|--------|----------|
| `table.Boolean("active")` | BOOLEAN |

### Date & Time

| Method | SQL Type |
|--------|----------|
| `table.Date("birthday")` | DATE |
| `table.DateTime("published_at")` | DATETIME |
| `table.DateTimeTz("published_at")` | DATETIME with TZ |
| `table.Time("alarm")` | TIME |
| `table.TimeTz("alarm")` | TIME with TZ |
| `table.Timestamp("added_at")` | TIMESTAMP |
| `table.TimestampTz("added_at")` | TIMESTAMP with TZ |
| `table.Timestamps()` | created_at + updated_at |
| `table.TimestampsTz()` | TZ variants |
| `table.SoftDeletes()` | nullable deleted_at TIMESTAMP |
| `table.SoftDeletesTz()` | TZ variant |

### Special

| Method | SQL Type |
|--------|----------|
| `table.Enum("difficulty", []any{"easy", "hard"})` | ENUM (MySQL) or VARCHAR (others) |
| `table.Morphs("commentable")` | commentable_id (BIGINT) + commentable_type (VARCHAR) + index |
| `table.NullableMorphs("imageable")` | nullable morph columns + index |
| `table.NumericMorphs("taggable")` | numeric morph columns |
| `table.UuidMorphs("taggable")` | UUID morph columns |
| `table.UlidMorphs("taggable")` | ULID morph columns |
| `table.Column("geometry", "geometry")` | custom type |

## Column Modifiers

Chain modifiers on any column definition:

| Modifier | Effect |
|----------|--------|
| `.Nullable()` | Allow NULL |
| `.Default(value)` | Set default value |
| `.Unsigned()` | UNSIGNED (MySQL) |
| `.AutoIncrement()` | Auto-incrementing PK |
| `.Comment("text")` | Column comment (MySQL/Postgres) |
| `.After("column")` | Place after column (MySQL) |
| `.First()` | Place first (MySQL) |
| `.Change()` | Modify existing column (MySQL/Postgres/SQLServer) |
| `.UseCurrent()` | Default CURRENT_TIMESTAMP |
| `.UseCurrentOnUpdate()` | Update to CURRENT_TIMESTAMP (MySQL) |
| `.Always()` | Always generated (Postgres) |
| `.GeneratedAs()` | Generated column (Postgres) |

## Column Operations

```go
// Rename column
table.RenameColumn("old", "new")

// Drop column(s)
table.DropColumn("name")
table.DropColumn("name", "age")
```

## Indexes

```go
// Create
table.Primary("id")                // primary key
table.Primary("id", "name")       // composite PK
table.Unique("email")             // unique index
table.Unique("first", "last")     // composite unique
table.Index("name")               // standard index
table.Index("name", "age")        // composite index
table.FullText("body")            // fulltext index

// Rename
table.RenameIndex("old_index", "new_index")

// Drop
table.DropPrimary("id")
table.DropUnique("email")
table.DropUniqueByName("email_unique")
table.DropIndex("name")
table.DropIndexByName("name_index")
table.DropFullText("body")
table.DropFullTextByName("body_fulltext")
```

## Foreign Keys

```go
// Create
table.UnsignedBigInteger("user_id")
table.Foreign("user_id").References("id").On("users")

// Drop
table.DropForeign("user_id")
table.DropForeignByName("user_id_foreign")
```

## Database Inspection Commands

```bash
./artisan db:show                           # all tables
./artisan db:show --database=postgres
./artisan db:table users                    # single table structure
./artisan db:table users --database=postgres
```
