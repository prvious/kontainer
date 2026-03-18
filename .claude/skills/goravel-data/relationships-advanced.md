# Relationships Advanced Reference

Complete reference for Goravel ORM relationships. v1.17.

## One To One (HasOne)

```go
type User struct {
    orm.Model
    Name  string
    Phone *Phone           // HasOne: Phone.UserID references User.ID
}

type Phone struct {
    orm.Model
    UserID uint
    Name   string
    User   *User           // BelongsTo (inverse)
}
```

### Custom Foreign Key / References

```go
type User struct {
    orm.Model
    Name  string
    Phone *Phone `gorm:"foreignKey:UserName"`              // Phone.UserName instead of Phone.UserID
}

type User struct {
    orm.Model
    Name  string
    Phone *Phone `gorm:"foreignKey:UserName;references:name"` // Phone.UserName references User.Name
}
```

## One To Many (HasMany)

```go
type Post struct {
    orm.Model
    Name     string
    Comments []*Comment     // HasMany: Comment.PostID references Post.ID
}

type Comment struct {
    orm.Model
    PostID uint
    Name   string
    Post   *Post           // BelongsTo (inverse)
}
```

Goravel auto-derives the FK as `{ParentModel}ID` (e.g., `PostID`).

## Many To Many

Requires a pivot table:

```go
type User struct {
    orm.Model
    Name  string
    Roles []*Role `gorm:"many2many:role_user"`
}

type Role struct {
    orm.Model
    Name  string
    Users []*User `gorm:"many2many:role_user"` // inverse
}
```

Pivot table: `role_user` with `user_id` and `role_id` columns.

### Custom Pivot Keys

```go
type User struct {
    orm.Model
    Roles []*Role `gorm:"many2many:role_user;joinForeignKey:UserName;joinReferences:RoleName"`
}
```

## Polymorphic Relationships

A child model belongs to multiple parent types via `{morph}_id` and `{morph}_type` columns.

### MorphOne / MorphMany

```go
type Post struct {
    orm.Model
    Name     string
    Image    *Image     `gorm:"polymorphic:Imageable"`     // MorphOne
    Comments []*Comment `gorm:"polymorphic:Commentable"`   // MorphMany
}

type Video struct {
    orm.Model
    Name     string
    Image    *Image     `gorm:"polymorphic:Imageable"`
    Comments []*Comment `gorm:"polymorphic:Commentable"`
}

type Image struct {
    orm.Model
    URL           string
    ImageableID   uint
    ImageableType string
}

type Comment struct {
    orm.Model
    Body            string
    CommentableID   uint
    CommentableType string
}
```

### Custom Polymorphic Value

By default `ImageableType` stores the model name. Override with:

```go
Image *Image `gorm:"polymorphic:Imageable;polymorphicValue:master"`
```

## Eager Loading

Solves N+1 query problem. Use `With` to preload relationships:

```go
// Single relationship
facades.Orm().Query().With("Author").Find(&books)

// Multiple relationships
facades.Orm().Query().With("Author").With("Publisher").Find(&book)

// Nested eager loading (dot syntax)
facades.Orm().Query().With("Author.Contacts").Find(&book)
```

### Constrained Eager Loading

```go
// Simple condition
facades.Orm().Query().With("Author", "name = ?", "author").Find(&book)

// Closure for complex conditions
facades.Orm().Query().With("Author", func(query orm.Query) orm.Query {
    return query.Where("name = ?", "author")
}).Find(&book)
```

### Lazy Eager Loading

Load relationships after the parent is already retrieved:

```go
facades.Orm().Query().Find(&books)

for _, book := range books {
    if someCondition {
        facades.Orm().Query().Load(&book, "Author")
    }
}

// With conditions
facades.Orm().Query().Load(&book, "Author", "name = ?", "specific")
facades.Orm().Query().Load(&book, "Author", func(query orm.Query) orm.Query {
    return query.Where("active", true)
})

// Only load if not already loaded
facades.Orm().Query().LoadMissing(&book, "Author")
```

## Association Operations

Use `.Association("RelationName")` to manipulate relationships directly.

### Find Associations

```go
var posts []models.Post
facades.Orm().Query().Model(&user).Association("Posts").Find(&posts)

// With conditions
facades.Orm().Query().Model(&user).Where("name = ?", "goravel").Order("id desc").Association("Posts").Find(&posts)
```

### Append Associations

For Many-To-Many / One-To-Many: adds to existing. For One-To-One: replaces.

```go
facades.Orm().Query().Model(&user).Association("Posts").Append([]*models.Post{post1, post2})
facades.Orm().Query().Model(&user).Association("Posts").Append(&models.Post{Name: "new"})
```

### Replace Associations

Replace all current associations:

```go
facades.Orm().Query().Model(&user).Association("Posts").Replace([]*models.Post{post1, post2})
```

### Delete Associations

Remove the relationship reference (does not delete the record from DB). FK must be nullable:

```go
facades.Orm().Query().Model(&user).Association("Posts").Delete([]*models.Post{post1, post2})
```

### Clear Associations

Remove all references:

```go
facades.Orm().Query().Model(&user).Association("Posts").Clear()
```

### Count Associations

```go
count := facades.Orm().Query().Model(&user).Association("Posts").Count()

// With conditions
count := facades.Orm().Query().Model(&user).Where("name = ?", "goravel").Association("Posts").Count()
```

### Batch Association Operations

Operate on a slice of parent models:

```go
// Find all posts for all users
facades.Orm().Query().Model(&users).Association("Posts").Find(&posts)

// Delete from all
facades.Orm().Query().Model(&users).Association("Posts").Delete(&postToRemove)

// Count across all
facades.Orm().Query().Model(&users).Association("Posts").Count()

// Append/Replace: argument length must match parent slice length
var users = []models.User{user1, user2, user3}
facades.Orm().Query().Model(&users).Association("Team").Append(&userA, &userB, &[]models.User{userA, userB, userC})
facades.Orm().Query().Model(&users).Association("Team").Replace(&userA, &userB, &[]models.User{userA, userB, userC})
```

## Creating/Updating with Associations

Control which associations are persisted with `Select` and `Omit`:

```go
user := models.User{Name: "user", Posts: []*models.Post{{Name: "post"}}}

// Create all child associations
facades.Orm().Query().Select(orm.Associations).Create(&user)

// Only create specific associations (must also list parent fields)
facades.Orm().Query().Select("Name", "Posts").Create(&user)

// Skip specific associations
facades.Orm().Query().Omit("Posts").Create(&user)

// Skip all associations
facades.Orm().Query().Omit(orm.Associations).Create(&user)
```

## Deleting with Associations

```go
// Delete user and their Account
facades.Orm().Query().Select("Account").Delete(&user)

// Delete user and specific associations
facades.Orm().Query().Select("Orders", "CreditCards").Delete(&user)

// Delete user and all associations
facades.Orm().Query().Select(orm.Associations).Delete(&user)
```

Note: associations only deleted when the parent model has a non-empty primary key.
