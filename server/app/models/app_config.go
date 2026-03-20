package models

import "github.com/goravel/framework/database/orm"

type AppConfig struct {
	orm.Timestamps
	Key   string `gorm:"primaryKey" json:"key"`
	Value string `gorm:"type:text;not null" json:"value"`
}

func (r *AppConfig) TableName() string {
	return "app_configs"
}
