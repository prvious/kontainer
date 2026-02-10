package settings

import (
	"context"
	"encoding/json"
	"log"
	"net/http"

	"github.com/prvious/kontainer/agent/storage"
)

type UpdateSettingsAction struct {
	store *storage.SettingsStore
}

func NewUpdateSettingsAction(store *storage.SettingsStore) *UpdateSettingsAction {
	return &UpdateSettingsAction{store: store}
}

func (a *UpdateSettingsAction) Execute(ctx context.Context, settings map[string]string) error {
	return a.store.SetAll(settings)
}

type UpdateSettingsHandler struct {
	action    *UpdateSettingsAction
	getAction *GetSettingsAction
}

func NewUpdateSettingsHandler(action *UpdateSettingsAction, getAction *GetSettingsAction) *UpdateSettingsHandler {
	return &UpdateSettingsHandler{action: action, getAction: getAction}
}

func (h *UpdateSettingsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	var body struct {
		Settings map[string]string `json:"settings"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "invalid request body"})
		return
	}

	if err := h.action.Execute(r.Context(), body.Settings); err != nil {
		log.Printf("update settings error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to update settings"})
		return
	}

	// Return updated settings
	settings, err := h.getAction.Execute(r.Context())
	if err != nil {
		log.Printf("get settings after update error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to read updated settings"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{"settings": settings})
}
