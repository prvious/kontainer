package storage

import (
	"os"
	"path/filepath"
	"testing"
)

func tempDBPath(t *testing.T) string {
	t.Helper()
	return filepath.Join(t.TempDir(), "test.db")
}

func TestSettingsStore_SetAndGet(t *testing.T) {
	store, err := NewSettingsStore(tempDBPath(t))
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	if err := store.Set("foo", "bar"); err != nil {
		t.Fatal(err)
	}

	val, ok, err := store.Get("foo")
	if err != nil {
		t.Fatal(err)
	}
	if !ok {
		t.Fatal("expected key to exist")
	}
	if val != "bar" {
		t.Errorf("expected bar, got %s", val)
	}
}

func TestSettingsStore_GetMissing(t *testing.T) {
	store, err := NewSettingsStore(tempDBPath(t))
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	_, ok, err := store.Get("nonexistent")
	if err != nil {
		t.Fatal(err)
	}
	if ok {
		t.Error("expected key not to exist")
	}
}

func TestSettingsStore_SetOverwrite(t *testing.T) {
	store, err := NewSettingsStore(tempDBPath(t))
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	store.Set("key", "first")
	store.Set("key", "second")

	val, ok, err := store.Get("key")
	if err != nil {
		t.Fatal(err)
	}
	if !ok || val != "second" {
		t.Errorf("expected second, got %s (ok=%v)", val, ok)
	}
}

func TestSettingsStore_GetAll(t *testing.T) {
	store, err := NewSettingsStore(tempDBPath(t))
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	store.Set("a", "1")
	store.Set("b", "2")

	all, err := store.GetAll()
	if err != nil {
		t.Fatal(err)
	}
	if len(all) != 2 {
		t.Fatalf("expected 2 settings, got %d", len(all))
	}
	if all["a"] != "1" || all["b"] != "2" {
		t.Errorf("unexpected values: %v", all)
	}
}

func TestSettingsStore_GetAllEmpty(t *testing.T) {
	store, err := NewSettingsStore(tempDBPath(t))
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	all, err := store.GetAll()
	if err != nil {
		t.Fatal(err)
	}
	if len(all) != 0 {
		t.Errorf("expected empty map, got %v", all)
	}
}

func TestSettingsStore_SetAll(t *testing.T) {
	store, err := NewSettingsStore(tempDBPath(t))
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	store.Set("existing", "old")

	err = store.SetAll(map[string]string{
		"existing": "new",
		"added":    "value",
	})
	if err != nil {
		t.Fatal(err)
	}

	all, _ := store.GetAll()
	if all["existing"] != "new" {
		t.Errorf("expected new, got %s", all["existing"])
	}
	if all["added"] != "value" {
		t.Errorf("expected value, got %s", all["added"])
	}
}

func TestSettingsStore_SetAllEmpty(t *testing.T) {
	store, err := NewSettingsStore(tempDBPath(t))
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	err = store.SetAll(map[string]string{})
	if err != nil {
		t.Fatal(err)
	}
}

func TestSettingsStore_SharedDB(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "shared.db")

	// Create a metrics store on the same DB file
	metricsStore, err := NewMetricsStore(dbPath, 0)
	if err != nil {
		t.Fatal(err)
	}
	defer metricsStore.Close()

	// Create a settings store on the same DB file
	settingsStore, err := NewSettingsStore(dbPath)
	if err != nil {
		t.Fatal(err)
	}
	defer settingsStore.Close()

	// Both should work concurrently
	if err := settingsStore.Set("test", "value"); err != nil {
		t.Fatal(err)
	}
	val, ok, err := settingsStore.Get("test")
	if err != nil || !ok || val != "value" {
		t.Errorf("unexpected result: val=%s ok=%v err=%v", val, ok, err)
	}
}

func TestNewSettingsStore_InvalidPath(t *testing.T) {
	_, err := NewSettingsStore(filepath.Join(os.DevNull, "nope", "db"))
	if err == nil {
		t.Error("expected error for invalid path")
	}
}
