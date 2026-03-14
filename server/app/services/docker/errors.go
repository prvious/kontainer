package docker

import "errors"

var (
	ErrNotFound   = errors.New("resource not found")
	ErrGlobalMode = errors.New("operation not supported in global mode")
)
