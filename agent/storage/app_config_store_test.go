package storage

import (
	"os"
	"path/filepath"
	"testing"
)

func TestAppConfigStore_SetAndGet(t *testing.T) {
	store, err := NewAppConfigStore(tempDBPath(t))
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	// Set a value
	if err := store.Set("app_id", "12345"); err != nil {
		t.Fatal(err)
	}

	// Get it back
	val, ok, err := store.Get("app_id")
	if err != nil {
		t.Fatal(err)
	}
	if !ok {
		t.Fatal("expected key to exist")
	}
	if val != "12345" {
		t.Errorf("expected 12345, got %s", val)
	}

	// Get missing key
	_, ok, err = store.Get("nonexistent")
	if err != nil {
		t.Fatal(err)
	}
	if ok {
		t.Error("expected key not to exist")
	}

	// Upsert (overwrite)
	if err := store.Set("app_id", "67890"); err != nil {
		t.Fatal(err)
	}
	val, ok, err = store.Get("app_id")
	if err != nil {
		t.Fatal(err)
	}
	if !ok || val != "67890" {
		t.Errorf("expected 67890 after upsert, got %s (ok=%v)", val, ok)
	}
}

func TestAppConfigStore_SetAllAndGetAll(t *testing.T) {
	store, err := NewAppConfigStore(tempDBPath(t))
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	// GetAll on empty table
	all, err := store.GetAll()
	if err != nil {
		t.Fatal(err)
	}
	if len(all) != 0 {
		t.Errorf("expected empty map, got %v", all)
	}

	// SetAll with multiple keys
	err = store.SetAll(map[string]string{
		"app_id":         "111",
		"client_id":      "222",
		"client_secret":  "333",
		"private_key":    "pem-data",
		"webhook_secret": "whsec",
	})
	if err != nil {
		t.Fatal(err)
	}

	// GetAll should return all keys
	all, err = store.GetAll()
	if err != nil {
		t.Fatal(err)
	}
	if len(all) != 5 {
		t.Fatalf("expected 5 entries, got %d", len(all))
	}
	if all["app_id"] != "111" {
		t.Errorf("expected 111, got %s", all["app_id"])
	}
	if all["client_secret"] != "333" {
		t.Errorf("expected 333, got %s", all["client_secret"])
	}
	if all["private_key"] != "pem-data" {
		t.Errorf("expected pem-data, got %s", all["private_key"])
	}

	// SetAll upserts existing keys
	err = store.SetAll(map[string]string{
		"app_id":  "999",
		"org":     "my-org",
	})
	if err != nil {
		t.Fatal(err)
	}

	all, err = store.GetAll()
	if err != nil {
		t.Fatal(err)
	}
	if len(all) != 6 {
		t.Fatalf("expected 6 entries, got %d", len(all))
	}
	if all["app_id"] != "999" {
		t.Errorf("expected 999 after upsert, got %s", all["app_id"])
	}
	if all["org"] != "my-org" {
		t.Errorf("expected my-org, got %s", all["org"])
	}
	// Existing keys should be untouched
	if all["client_id"] != "222" {
		t.Errorf("expected 222, got %s", all["client_id"])
	}

	// SetAll with empty map is a no-op
	err = store.SetAll(map[string]string{})
	if err != nil {
		t.Fatal(err)
	}
}

func TestAppConfigStore_IsSetupComplete(t *testing.T) {
	store, err := NewAppConfigStore(tempDBPath(t))
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	// Before setting the flag, should be false
	if store.IsSetupComplete() {
		t.Error("expected IsSetupComplete to be false initially")
	}

	// Set setup_complete to something other than "true"
	if err := store.Set("setup_complete", "false"); err != nil {
		t.Fatal(err)
	}
	if store.IsSetupComplete() {
		t.Error("expected IsSetupComplete to be false when value is 'false'")
	}

	// Set setup_complete to "true"
	if err := store.Set("setup_complete", "true"); err != nil {
		t.Fatal(err)
	}
	if !store.IsSetupComplete() {
		t.Error("expected IsSetupComplete to be true")
	}

	// Overwrite back to something else
	if err := store.Set("setup_complete", "0"); err != nil {
		t.Fatal(err)
	}
	if store.IsSetupComplete() {
		t.Error("expected IsSetupComplete to be false after overwrite to '0'")
	}
}

func TestAppConfigStore_SharedDB(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "shared.db")

	// Create a settings store on the same DB file
	settingsStore, err := NewSettingsStore(dbPath)
	if err != nil {
		t.Fatal(err)
	}
	defer settingsStore.Close()

	// Create an app config store on the same DB file
	appConfigStore, err := NewAppConfigStore(dbPath)
	if err != nil {
		t.Fatal(err)
	}
	defer appConfigStore.Close()

	// Both should work independently
	if err := settingsStore.Set("setting_key", "setting_val"); err != nil {
		t.Fatal(err)
	}
	if err := appConfigStore.Set("config_key", "config_val"); err != nil {
		t.Fatal(err)
	}

	// Settings store should not see app_config keys
	settingsAll, err := settingsStore.GetAll()
	if err != nil {
		t.Fatal(err)
	}
	if _, exists := settingsAll["config_key"]; exists {
		t.Error("settings store should not contain app_config keys")
	}

	// App config store should not see settings keys
	configAll, err := appConfigStore.GetAll()
	if err != nil {
		t.Fatal(err)
	}
	if _, exists := configAll["setting_key"]; exists {
		t.Error("app config store should not contain settings keys")
	}

	// Each should have their own data
	val, ok, err := appConfigStore.Get("config_key")
	if err != nil || !ok || val != "config_val" {
		t.Errorf("unexpected result: val=%s ok=%v err=%v", val, ok, err)
	}
}

func TestNewAppConfigStore_InvalidPath(t *testing.T) {
	_, err := NewAppConfigStore(filepath.Join(os.DevNull, "nope", "db"))
	if err == nil {
		t.Error("expected error for invalid path")
	}
}
