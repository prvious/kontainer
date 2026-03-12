package helpers

import (
	"os"
	"strings"
)

// ResolveFileEnvVars checks if a _FILE suffixed env var exists for each
// given key. If found, reads the file and sets the base env var to its
// trimmed contents. Used for Docker Swarm secrets mounted as files.
func ResolveFileEnvVars(keys ...string) {
	for _, key := range keys {
		fileKey := key + "_FILE"
		filePath := os.Getenv(fileKey)
		if filePath == "" {
			continue
		}

		data, err := os.ReadFile(filePath)
		if err != nil {
			continue
		}

		os.Setenv(key, strings.TrimSpace(string(data)))
	}
}
