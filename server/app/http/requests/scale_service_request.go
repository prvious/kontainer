package requests

import "github.com/goravel/framework/contracts/http"

type ScaleServiceRequest struct {
	Replicas *uint64 `form:"replicas" json:"replicas"`
}

func (r *ScaleServiceRequest) Authorize(ctx http.Context) error {
	return nil
}

func (r *ScaleServiceRequest) Rules(ctx http.Context) map[string]string {
	return map[string]string{
		"replicas": "required",
	}
}
