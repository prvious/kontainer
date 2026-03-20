package models

import (
	"time"

	"github.com/goravel/framework/database/orm"
)

type NodeMetric struct {
	orm.Model
	Timestamp   time.Time `gorm:"not null" json:"timestamp"`
	NodeID      string    `gorm:"column:node_id;not null" json:"node_id"`
	Hostname    string    `gorm:"not null" json:"hostname"`
	CPUs        int       `gorm:"column:cpus" json:"cpus"`
	CPUPercent  float64   `gorm:"column:cpu_pct" json:"cpu_percent"`
	MemUsed     uint64    `gorm:"column:mem_used" json:"mem_used"`
	MemTotal    uint64    `gorm:"column:mem_total" json:"mem_total"`
	MemPercent  float64   `gorm:"column:mem_pct" json:"mem_percent"`
	DiskUsed    uint64    `gorm:"column:disk_used" json:"disk_used"`
	DiskTotal   uint64    `gorm:"column:disk_total" json:"disk_total"`
	DiskPercent float64   `gorm:"column:disk_pct" json:"disk_percent"`
}

func (r *NodeMetric) TableName() string {
	return "node_metrics"
}
