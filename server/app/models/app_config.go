package models

type AppConfig struct {
	Key   string `gorm:"primaryKey"`
	Value string `gorm:"type:text;not null"`
}

func (r *AppConfig) TableName() string {
	return "app_configs"
}
