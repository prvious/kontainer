package auth

import (
	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/models"
	"github.com/prvious/kontainer/server/app/services/auth/providers"
)

// UpsertUserAction finds or creates a User by provider + provider_id.
type UpsertUserAction struct{}

func NewUpsertUserAction() *UpsertUserAction {
	return &UpsertUserAction{}
}

func (a *UpsertUserAction) Execute(providerUser *providers.ProviderUser) (*models.User, error) {
	var user models.User

	err := facades.Orm().Query().UpdateOrCreate(&user,
		models.User{Provider: providerUser.Provider, ProviderID: providerUser.ID},
		models.User{Name: providerUser.Name, Email: providerUser.Email, AvatarURL: providerUser.AvatarURL},
	)
	if err != nil {
		return nil, err
	}

	return &user, nil
}
