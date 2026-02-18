package server

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestSwappableHandler(t *testing.T) {
	handlerA := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("A"))
	})
	handlerB := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("B"))
	})

	sw := NewSwappableHandler(handlerA)

	// Verify it serves "A"
	rec := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	sw.ServeHTTP(rec, req)

	if rec.Body.String() != "A" {
		t.Fatalf("expected body %q, got %q", "A", rec.Body.String())
	}

	// Swap to handler B
	sw.Swap(handlerB)

	// Verify it now serves "B"
	rec = httptest.NewRecorder()
	req = httptest.NewRequest(http.MethodGet, "/", nil)
	sw.ServeHTTP(rec, req)

	if rec.Body.String() != "B" {
		t.Fatalf("expected body %q, got %q", "B", rec.Body.String())
	}
}
