package auth

import (
	"encoding/json"
	"net/http"
)

type MeHandler struct{}

func NewMeHandler() *MeHandler {
	return &MeHandler{}
}

type meResponse struct {
	User   meUser `json:"user"`
	Status string `json:"status"`
}

type meUser struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	Email     string `json:"email"`
	AvatarURL string `json:"avatarUrl"`
	Provider  string `json:"provider"`
}

func (h *MeHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	claims, ok := r.Context().Value(ClaimsKey).(*Claims)
	if !ok || claims == nil {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(meResponse{
		User: meUser{
			ID:        claims.UserID,
			Name:      claims.Name,
			Email:     claims.Email,
			AvatarURL: claims.AvatarURL,
			Provider:  claims.Provider,
		},
		Status: "ok",
	})
}
