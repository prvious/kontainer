package helpers_test

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/prvious/kontainer/server/app/helpers"
)

func TestResolveFileEnvVars(t *testing.T) {
	dir := t.TempDir()
	secretFile := filepath.Join(dir, "secret.txt")
	if err := os.WriteFile(secretFile, []byte("  my-secret-value  \n"), 0644); err != nil {
		t.Fatal(err)
	}

	t.Setenv("TEST_SECRET_FILE", secretFile)
	t.Setenv("TEST_SECRET", "")

	helpers.ResolveFileEnvVars("TEST_SECRET")

	got := os.Getenv("TEST_SECRET")
	if got != "my-secret-value" {
		t.Errorf("expected 'my-secret-value', got '%s'", got)
	}
}

func TestResolveFileEnvVars_NoFileVar(t *testing.T) {
	t.Setenv("TEST_VAR", "original")

	helpers.ResolveFileEnvVars("TEST_VAR")

	got := os.Getenv("TEST_VAR")
	if got != "original" {
		t.Errorf("expected 'original', got '%s'", got)
	}
}

func TestResolveFileEnvVars_FileNotFound(t *testing.T) {
	t.Setenv("TEST_MISSING_FILE", "/nonexistent/path")
	t.Setenv("TEST_MISSING", "fallback")

	helpers.ResolveFileEnvVars("TEST_MISSING")

	got := os.Getenv("TEST_MISSING")
	if got != "fallback" {
		t.Errorf("expected 'fallback', got '%s'", got)
	}
}
