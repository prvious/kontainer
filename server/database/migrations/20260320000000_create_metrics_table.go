package migrations

import (
	"github.com/goravel/framework/contracts/database/schema"

	"github.com/prvious/kontainer/server/app/facades"
)

type M20260320000000CreateMetricsTable struct{}

func (r *M20260320000000CreateMetricsTable) Signature() string {
	return "20260320000000_create_metrics_table"
}

func (r *M20260320000000CreateMetricsTable) Up() error {
	if err := facades.Schema().DropIfExists("node_metrics"); err != nil {
		return err
	}

	if !facades.Schema().HasTable("metrics") {
		return facades.Schema().Create("metrics", func(table schema.Blueprint) {
			table.ID()
			table.Timestamp("timestamp")
			table.String("kind")
			table.String("node_id")
			table.String("subject_id")
			table.String("subject_name")
			table.Float("cpu_pct", 2)
			table.BigInteger("mem_used")
			table.BigInteger("mem_total")
			table.Float("mem_pct", 2)
			table.Integer("cpus")
			table.BigInteger("disk_used")
			table.BigInteger("disk_total")
			table.Float("disk_pct", 2)
			table.String("service_id")

			table.Index("kind", "node_id", "timestamp")
			table.Index("kind", "timestamp")
			table.Timestamps()
		})
	}

	return nil
}

func (r *M20260320000000CreateMetricsTable) Down() error {
	return facades.Schema().DropIfExists("metrics")
}
