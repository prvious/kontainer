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

	if err := helpers.ResolveFileEnvVars("TEST_SECRET"); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	got := os.Getenv("TEST_SECRET")
	if got != "my-secret-value" {
		t.Errorf("expected 'my-secret-value', got '%s'", got)
	}
}

func TestResolveFileEnvVars_NoFileVar(t *testing.T) {
	t.Setenv("TEST_VAR", "original")

	if err := helpers.ResolveFileEnvVars("TEST_VAR"); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	got := os.Getenv("TEST_VAR")
	if got != "original" {
		t.Errorf("expected 'original', got '%s'", got)
	}
}

func TestResolveFileEnvVars_FileNotFound(t *testing.T) {
	t.Setenv("TEST_MISSING_FILE", "/nonexistent/path")
	t.Setenv("TEST_MISSING", "fallback")

	err := helpers.ResolveFileEnvVars("TEST_MISSING")
	if err == nil {
		t.Error("expected error for missing file, got nil")
	}

	// Original value preserved since file read failed
	got := os.Getenv("TEST_MISSING")
	if got != "fallback" {
		t.Errorf("expected 'fallback', got '%s'", got)
	}
}

func TestResolveFileEnvVars_MultipleKeys(t *testing.T) {
	dir := t.TempDir()
	secretFile := filepath.Join(dir, "secret.txt")
	if err := os.WriteFile(secretFile, []byte("resolved-value"), 0644); err != nil {
		t.Fatal(err)
	}

	t.Setenv("KEY_A_FILE", secretFile)
	t.Setenv("KEY_A", "")
	t.Setenv("KEY_B", "untouched")
	// KEY_B_FILE is not set — KEY_B should be preserved

	if err := helpers.ResolveFileEnvVars("KEY_A", "KEY_B"); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if got := os.Getenv("KEY_A"); got != "resolved-value" {
		t.Errorf("KEY_A: expected 'resolved-value', got '%s'", got)
	}
	if got := os.Getenv("KEY_B"); got != "untouched" {
		t.Errorf("KEY_B: expected 'untouched', got '%s'", got)
	}
}
