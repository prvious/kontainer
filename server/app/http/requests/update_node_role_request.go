package requests

import "github.com/goravel/framework/contracts/http"

type UpdateNodeRoleRequest struct {
	Role string `form:"role" json:"role"`
}

func (r *UpdateNodeRoleRequest) Authorize(ctx http.Context) error {
	return nil
}

func (r *UpdateNodeRoleRequest) Rules(ctx http.Context) map[string]string {
	return map[string]string{
		"role": "required|in:manager,worker",
	}
}
