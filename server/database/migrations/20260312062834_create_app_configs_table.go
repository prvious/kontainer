package migrations

import (
	"github.com/goravel/framework/contracts/database/schema"

	"github.com/prvious/kontainer/server/app/facades"
)

type M20260312062834CreateAppConfigsTable struct{}

// Signature The unique signature for the migration.
func (r *M20260312062834CreateAppConfigsTable) Signature() string {
	return "20260312062834_create_app_configs_table"
}

// Up Run the migrations.
func (r *M20260312062834CreateAppConfigsTable) Up() error {
	if !facades.Schema().HasTable("app_configs") {
		return facades.Schema().Create("app_configs", func(table schema.Blueprint) {
			table.String("key")
			table.Text("value")
			table.Primary("key")
			table.Timestamps()
		})
	}

	return nil
}

// Down Reverse the migrations.
func (r *M20260312062834CreateAppConfigsTable) Down() error {
	return facades.Schema().DropIfExists("app_configs")
}
