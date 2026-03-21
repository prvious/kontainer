package bootstrap

import (
	"github.com/goravel/framework/contracts/database/schema"

	"github.com/prvious/kontainer/server/database/migrations"
)

func Migrations() []schema.Migration {
	return []schema.Migration{
		&migrations.M20210101000001CreateJobsTable{},
		&migrations.M20260312062757CreateUsersTable{},
		&migrations.M20260312062834CreateAppConfigsTable{},
		&migrations.M20260312062903CreateNodeMetricsTable{},
		&migrations.M20260320000000CreateMetricsTable{},
		&migrations.M20260312062941CreateSettingsTable{},
	}
}
