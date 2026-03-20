package requests

import "github.com/goravel/framework/contracts/http"

type CreateConfigRequest struct {
	Name string `form:"name" json:"name"`
	Data string `form:"data" json:"data"`
}

func (r *CreateConfigRequest) Authorize(ctx http.Context) error {
	return nil
}

func (r *CreateConfigRequest) Rules(ctx http.Context) map[string]string {
	return map[string]string{
		"name": "required",
		"data": "required",
	}
}
