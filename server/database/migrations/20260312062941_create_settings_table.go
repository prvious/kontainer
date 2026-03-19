package migrations

import (
	"github.com/goravel/framework/contracts/database/schema"

	"github.com/prvious/kontainer/server/app/facades"
)

type M20260312062941CreateSettingsTable struct{}

// Signature The unique signature for the migration.
func (r *M20260312062941CreateSettingsTable) Signature() string {
	return "20260312062941_create_settings_table"
}

// Up Run the migrations.
func (r *M20260312062941CreateSettingsTable) Up() error {
	if !facades.Schema().HasTable("settings") {
		return facades.Schema().Create("settings", func(table schema.Blueprint) {
			table.String("key")
			table.Text("value")
			table.Primary("key")
			table.Timestamps()
		})
	}

	return nil
}

// Down Reverse the migrations.
func (r *M20260312062941CreateSettingsTable) Down() error {
	return facades.Schema().DropIfExists("settings")
}
