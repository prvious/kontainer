package helpers

import (
	"fmt"
	"os"
	"strings"
)

// ResolveFileEnvVars checks if a _FILE suffixed env var exists for each
// given key. If found, reads the file and sets the base env var to its
// trimmed contents. Used for Docker Swarm secrets mounted as files.
// Returns an error if a _FILE var is set but the file cannot be read.
func ResolveFileEnvVars(keys ...string) error {
	for _, key := range keys {
		fileKey := key + "_FILE"
		filePath := os.Getenv(fileKey)
		if filePath == "" {
			continue
		}

		data, err := os.ReadFile(filePath)
		if err != nil {
			return fmt.Errorf("failed to read %s=%q for env var %s: %w", fileKey, filePath, key, err)
		}

		if err := os.Setenv(key, strings.TrimSpace(string(data))); err != nil {
			return fmt.Errorf("failed to set env var %s: %w", key, err)
		}
	}
	return nil
}
