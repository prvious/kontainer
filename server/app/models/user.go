package models

import "github.com/goravel/framework/database/orm"

type User struct {
	orm.Model
	Provider   string `gorm:"not null" json:"provider"`
	ProviderID string `gorm:"column:provider_id;not null" json:"provider_id"`
	Name       string `gorm:"not null" json:"name"`
	Email      string `json:"email"`
	AvatarURL  string `gorm:"column:avatar_url" json:"avatar_url"`
}
