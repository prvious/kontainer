package settings

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/clovismuneza/devoper/agent/storage"
)

type GetSettingsAction struct {
	store *storage.SettingsStore
}

func NewGetSettingsAction(store *storage.SettingsStore) *GetSettingsAction {
	return &GetSettingsAction{store: store}
}

func (a *GetSettingsAction) Execute(ctx context.Context) (map[string]string, error) {
	return a.store.GetAll()
}

type GetSettingsHandler struct {
	action *GetSettingsAction
}

func NewGetSettingsHandler(action *GetSettingsAction) *GetSettingsHandler {
	return &GetSettingsHandler{action: action}
}

func (h *GetSettingsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	settings, err := h.action.Execute(r.Context())
	if err != nil {
		log.Printf("get settings error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to get settings"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{"settings": settings})
}
