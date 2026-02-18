package server

import (
	"net/http"
	"sync"
)

// SwappableHandler wraps an http.Handler and allows hot-swapping it at
// runtime. This is used to transition from setup-mode router to normal-mode
// router without restarting the HTTP server.
type SwappableHandler struct {
	mu      sync.RWMutex
	handler http.Handler
}

// NewSwappableHandler returns a SwappableHandler that initially delegates to h.
func NewSwappableHandler(h http.Handler) *SwappableHandler {
	return &SwappableHandler{handler: h}
}

// ServeHTTP delegates to the current handler under a read lock.
func (s *SwappableHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	s.mu.RLock()
	h := s.handler
	s.mu.RUnlock()
	h.ServeHTTP(w, r)
}

// Swap replaces the current handler with h under a write lock.
func (s *SwappableHandler) Swap(h http.Handler) {
	s.mu.Lock()
	s.handler = h
	s.mu.Unlock()
}
