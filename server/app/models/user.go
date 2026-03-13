package models

import "github.com/goravel/framework/database/orm"

type User struct {
	orm.Model
	Provider   string `gorm:"not null"`
	ProviderID string `gorm:"column:provider_id;not null"`
	Name       string `gorm:"not null"`
	Email      string
	AvatarURL  string `gorm:"column:avatar_url"`
}
