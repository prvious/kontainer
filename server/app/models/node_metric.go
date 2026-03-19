package models

import (
	"time"

	"github.com/goravel/framework/database/orm"
)

type NodeMetric struct {
	orm.Model
	Timestamp   time.Time `gorm:"not null" json:"timestamp"`
	NodeID      string    `gorm:"column:node_id;not null"`
	Hostname    string    `gorm:"not null"`
	CPUs        int       `gorm:"column:cpus"`
	CPUPercent  float64   `gorm:"column:cpu_pct"`
	MemUsed     uint64    `gorm:"column:mem_used"`
	MemTotal    uint64    `gorm:"column:mem_total"`
	MemPercent  float64   `gorm:"column:mem_pct"`
	DiskUsed    uint64    `gorm:"column:disk_used"`
	DiskTotal   uint64    `gorm:"column:disk_total"`
	DiskPercent float64   `gorm:"column:disk_pct"`
}

func (r *NodeMetric) TableName() string {
	return "node_metrics"
}
