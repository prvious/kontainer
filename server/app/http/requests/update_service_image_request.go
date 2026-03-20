package requests

import "github.com/goravel/framework/contracts/http"

type UpdateServiceImageRequest struct {
	Image string `form:"image" json:"image"`
}

func (r *UpdateServiceImageRequest) Authorize(ctx http.Context) error {
	return nil
}

func (r *UpdateServiceImageRequest) Rules(ctx http.Context) map[string]string {
	return map[string]string{
		"image": "required",
	}
}
