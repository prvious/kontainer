package migrations

import (
	"github.com/goravel/framework/contracts/database/schema"

	"github.com/prvious/kontainer/server/app/facades"
)

type M20260312062903CreateNodeMetricsTable struct{}

// Signature The unique signature for the migration.
func (r *M20260312062903CreateNodeMetricsTable) Signature() string {
	return "20260312062903_create_node_metrics_table"
}

// Up Run the migrations.
func (r *M20260312062903CreateNodeMetricsTable) Up() error {
	if !facades.Schema().HasTable("node_metrics") {
		return facades.Schema().Create("node_metrics", func(table schema.Blueprint) {
			table.ID()
			table.Timestamp("timestamp")
			table.String("node_id")
			table.String("hostname")
			table.Integer("cpus")
			table.Float("cpu_pct", 2)
			table.UnsignedBigInteger("mem_used")
			table.UnsignedBigInteger("mem_total")
			table.Float("mem_pct", 2)
			table.UnsignedBigInteger("disk_used")
			table.UnsignedBigInteger("disk_total")
			table.Float("disk_pct", 2)

			table.Index("timestamp")
			table.Index("node_id")
			table.Timestamps()
		})
	}

	return nil
}

// Down Reverse the migrations.
func (r *M20260312062903CreateNodeMetricsTable) Down() error {
	return facades.Schema().DropIfExists("node_metrics")
}
