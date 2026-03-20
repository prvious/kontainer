package requests

import "github.com/goravel/framework/contracts/http"

type CreateSecretRequest struct {
	Name string `form:"name" json:"name"`
	Data string `form:"data" json:"data"`
}

func (r *CreateSecretRequest) Authorize(ctx http.Context) error {
	return nil
}

func (r *CreateSecretRequest) Rules(ctx http.Context) map[string]string {
	return map[string]string{
		"name": "required",
		"data": "required",
	}
}
