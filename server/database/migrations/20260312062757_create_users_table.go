package migrations

import (
	"github.com/goravel/framework/contracts/database/schema"

	"github.com/prvious/kontainer/server/app/facades"
)

type M20260312062757CreateUsersTable struct{}

// Signature The unique signature for the migration.
func (r *M20260312062757CreateUsersTable) Signature() string {
	return "20260312062757_create_users_table"
}

// Up Run the migrations.
func (r *M20260312062757CreateUsersTable) Up() error {
	if !facades.Schema().HasTable("users") {
		return facades.Schema().Create("users", func(table schema.Blueprint) {
			table.ID()
			table.String("provider")
			table.String("provider_id")
			table.String("name")
			table.String("email").Nullable()
			table.String("avatar_url").Nullable()
			table.Timestamps()

			table.Unique("provider", "provider_id")
		})
	}

	return nil
}

// Down Reverse the migrations.
func (r *M20260312062757CreateUsersTable) Down() error {
	return facades.Schema().DropIfExists("users")
}
