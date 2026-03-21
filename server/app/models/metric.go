package models

import (
	"time"

	"github.com/goravel/framework/database/orm"
)

type Metric struct {
	orm.Model
	Timestamp   time.Time `gorm:"not null" json:"timestamp"`
	Kind        string    `gorm:"not null" json:"kind"`
	NodeID      string    `gorm:"column:node_id;not null" json:"node_id"`
	SubjectID   string    `gorm:"column:subject_id;not null" json:"subject_id"`
	SubjectName string    `gorm:"column:subject_name;not null" json:"subject_name"`
	CPUPercent  float64   `gorm:"column:cpu_pct" json:"cpu_pct"`
	MemUsed     int64     `gorm:"column:mem_used" json:"mem_used"`
	MemTotal    int64     `gorm:"column:mem_total" json:"mem_total"`
	MemPercent  float64   `gorm:"column:mem_pct" json:"mem_pct"`
	CPUs        int       `gorm:"column:cpus" json:"cpus"`
	DiskUsed    int64     `gorm:"column:disk_used" json:"disk_used"`
	DiskTotal   int64     `gorm:"column:disk_total" json:"disk_total"`
	DiskPercent float64   `gorm:"column:disk_pct" json:"disk_pct"`
	ServiceID   string    `gorm:"column:service_id" json:"service_id"`
}

func (r *Metric) TableName() string {
	return "metrics"
}
