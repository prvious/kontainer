package requests

import "github.com/goravel/framework/contracts/http"

type UpdateNodeAvailabilityRequest struct {
	Availability string `form:"availability" json:"availability"`
}

func (r *UpdateNodeAvailabilityRequest) Authorize(ctx http.Context) error {
	return nil
}

func (r *UpdateNodeAvailabilityRequest) Rules(ctx http.Context) map[string]string {
	return map[string]string{
		"availability": "required|in:active,drain,pause",
	}
}
