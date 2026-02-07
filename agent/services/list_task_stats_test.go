package services

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/docker/docker/api/types/swarm"
)

func TestListTaskStatsHandler_MixedStates(t *testing.T) {
	mock := &mockDockerClient{
		tasks: []swarm.Task{
			{Status: swarm.TaskStatus{State: swarm.TaskStateRunning}, DesiredState: swarm.TaskStateRunning},
			{Status: swarm.TaskStatus{State: swarm.TaskStateRunning}, DesiredState: swarm.TaskStateRunning},
			{Status: swarm.TaskStatus{State: swarm.TaskStateFailed}, DesiredState: swarm.TaskStateRunning},
			{Status: swarm.TaskStatus{State: swarm.TaskStateComplete}, DesiredState: swarm.TaskStateShutdown},
			{Status: swarm.TaskStatus{State: swarm.TaskStatePreparing}, DesiredState: swarm.TaskStateRunning},
			{Status: swarm.TaskStatus{State: swarm.TaskStateStarting}, DesiredState: swarm.TaskStateRunning},
		},
	}

	action := NewListTaskStatsAction(mock)
	handler := NewListTaskStatsHandler(action)

	req := httptest.NewRequest("GET", "/api/task-stats", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		TaskStats TaskStats `json:"taskStats"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	s := resp.TaskStats
	if s.Total != 6 {
		t.Errorf("expected total 6, got %d", s.Total)
	}
	if s.Running != 2 {
		t.Errorf("expected running 2, got %d", s.Running)
	}
	if s.Failed != 1 {
		t.Errorf("expected failed 1, got %d", s.Failed)
	}
	if s.Complete != 1 {
		t.Errorf("expected complete 1, got %d", s.Complete)
	}
	if s.Preparing != 2 {
		t.Errorf("expected preparing 2, got %d", s.Preparing)
	}
	if s.DesiredRunning != 5 {
		t.Errorf("expected desiredRunning 5, got %d", s.DesiredRunning)
	}
}

func TestListTaskStatsHandler_EmptyList(t *testing.T) {
	mock := &mockDockerClient{
		tasks: []swarm.Task{},
	}

	action := NewListTaskStatsAction(mock)
	handler := NewListTaskStatsHandler(action)

	req := httptest.NewRequest("GET", "/api/task-stats", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		TaskStats TaskStats `json:"taskStats"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	s := resp.TaskStats
	if s.Total != 0 || s.Running != 0 || s.Failed != 0 || s.Preparing != 0 || s.Complete != 0 || s.DesiredRunning != 0 {
		t.Errorf("expected all zeros, got %+v", s)
	}
}

func TestListTaskStatsHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		taskErr: errors.New("connection refused"),
	}

	action := NewListTaskStatsAction(mock)
	handler := NewListTaskStatsHandler(action)

	req := httptest.NewRequest("GET", "/api/task-stats", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to list task stats" {
		t.Errorf("unexpected error message: %s", resp["error"])
	}
}

func TestListTaskStatsHandler_AllRunning(t *testing.T) {
	mock := &mockDockerClient{
		tasks: []swarm.Task{
			{Status: swarm.TaskStatus{State: swarm.TaskStateRunning}, DesiredState: swarm.TaskStateRunning},
			{Status: swarm.TaskStatus{State: swarm.TaskStateRunning}, DesiredState: swarm.TaskStateRunning},
			{Status: swarm.TaskStatus{State: swarm.TaskStateRunning}, DesiredState: swarm.TaskStateRunning},
		},
	}

	action := NewListTaskStatsAction(mock)
	handler := NewListTaskStatsHandler(action)

	req := httptest.NewRequest("GET", "/api/task-stats", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		TaskStats TaskStats `json:"taskStats"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	s := resp.TaskStats
	if s.Running != s.Total {
		t.Errorf("expected running == total, got running=%d total=%d", s.Running, s.Total)
	}
	if s.Failed != 0 {
		t.Errorf("expected failed 0, got %d", s.Failed)
	}
	if s.DesiredRunning != 3 {
		t.Errorf("expected desiredRunning 3, got %d", s.DesiredRunning)
	}
}
