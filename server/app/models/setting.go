package models

type Setting struct {
	Key   string `gorm:"primaryKey"`
	Value string `gorm:"type:text;not null"`
}

func (r *Setting) TableName() string {
	return "settings"
}
