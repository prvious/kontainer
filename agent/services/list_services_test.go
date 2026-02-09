package services

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/image"
	"github.com/docker/docker/api/types/network"
	"github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/api/types/volume"
)

type mockDockerClient struct {
	services   []swarm.Service
	tasks      []swarm.Task
	svcErr     error
	taskErr    error
	inspectSvc swarm.Service
	inspectErr error
	nodes      []swarm.Node
	nodeErr    error
	logsReader io.ReadCloser
	logsErr    error
	updateErr  error
	removeErr      error
	removedIDs     []string
	inspectTask    swarm.Task
	inspectTaskErr error
	taskLogsReader io.ReadCloser
	taskLogsErr    error
	networks       []network.Summary
	networkErr     error
	volumes        volume.ListResponse
	volumeErr      error
	inspectNode    swarm.Node
	inspectNodeErr error
	nodeUpdateErr  error
	secrets          []swarm.Secret
	secretErr        error
	secretCreateResp swarm.SecretCreateResponse
	secretCreateErr  error
	secretRemoveErr  error
	configs          []swarm.Config
	configErr        error
	configCreateResp swarm.ConfigCreateResponse
	configCreateErr  error
	configRemoveErr    error
	configInspect      swarm.Config
	configInspectErr   error

	lastServiceListOpts types.ServiceListOptions

	// Exec function fields — configurable per-test
	execCreateFn  func(ctx context.Context, ctr string, opts container.ExecOptions) (types.IDResponse, error)
	execAttachFn  func(ctx context.Context, execID string, opts container.ExecAttachOptions) (types.HijackedResponse, error)
	execResizeFn  func(ctx context.Context, execID string, opts container.ResizeOptions) error
	execInspectFn func(ctx context.Context, execID string) (container.ExecInspect, error)
}

func (m *mockDockerClient) ServiceList(_ context.Context, opts types.ServiceListOptions) ([]swarm.Service, error) {
	m.lastServiceListOpts = opts
	return m.services, m.svcErr
}

func (m *mockDockerClient) TaskList(_ context.Context, _ types.TaskListOptions) ([]swarm.Task, error) {
	return m.tasks, m.taskErr
}

func (m *mockDockerClient) ServiceInspectWithRaw(_ context.Context, _ string, _ types.ServiceInspectOptions) (swarm.Service, []byte, error) {
	return m.inspectSvc, nil, m.inspectErr
}

func (m *mockDockerClient) NodeList(_ context.Context, _ types.NodeListOptions) ([]swarm.Node, error) {
	return m.nodes, m.nodeErr
}

func (m *mockDockerClient) ServiceLogs(_ context.Context, _ string, _ container.LogsOptions) (io.ReadCloser, error) {
	return m.logsReader, m.logsErr
}

func (m *mockDockerClient) ServiceUpdate(_ context.Context, _ string, _ swarm.Version, _ swarm.ServiceSpec, _ swarm.ServiceUpdateOptions) (swarm.ServiceUpdateResponse, error) {
	return swarm.ServiceUpdateResponse{}, m.updateErr
}

func (m *mockDockerClient) TaskInspectWithRaw(_ context.Context, _ string) (swarm.Task, []byte, error) {
	return m.inspectTask, nil, m.inspectTaskErr
}

func (m *mockDockerClient) TaskLogs(_ context.Context, _ string, _ container.LogsOptions) (io.ReadCloser, error) {
	return m.taskLogsReader, m.taskLogsErr
}

func (m *mockDockerClient) NetworkList(_ context.Context, _ network.ListOptions) ([]network.Summary, error) {
	return m.networks, m.networkErr
}

func (m *mockDockerClient) VolumeList(_ context.Context, _ volume.ListOptions) (volume.ListResponse, error) {
	return m.volumes, m.volumeErr
}

func (m *mockDockerClient) NodeInspectWithRaw(_ context.Context, _ string) (swarm.Node, []byte, error) {
	return m.inspectNode, nil, m.inspectNodeErr
}

func (m *mockDockerClient) NodeUpdate(_ context.Context, _ string, _ swarm.Version, _ swarm.NodeSpec) error {
	return m.nodeUpdateErr
}

func (m *mockDockerClient) ServiceRemove(_ context.Context, serviceID string) error {
	if m.removeErr != nil {
		return m.removeErr
	}
	m.removedIDs = append(m.removedIDs, serviceID)
	return nil
}

func (m *mockDockerClient) SecretList(_ context.Context, _ swarm.SecretListOptions) ([]swarm.Secret, error) {
	return m.secrets, m.secretErr
}

func (m *mockDockerClient) SecretCreate(_ context.Context, _ swarm.SecretSpec) (swarm.SecretCreateResponse, error) {
	return m.secretCreateResp, m.secretCreateErr
}

func (m *mockDockerClient) SecretRemove(_ context.Context, _ string) error {
	return m.secretRemoveErr
}

func (m *mockDockerClient) ConfigList(_ context.Context, _ swarm.ConfigListOptions) ([]swarm.Config, error) {
	return m.configs, m.configErr
}

func (m *mockDockerClient) ConfigCreate(_ context.Context, _ swarm.ConfigSpec) (swarm.ConfigCreateResponse, error) {
	return m.configCreateResp, m.configCreateErr
}

func (m *mockDockerClient) ConfigInspectWithRaw(_ context.Context, _ string) (swarm.Config, []byte, error) {
	return m.configInspect, nil, m.configInspectErr
}

func (m *mockDockerClient) ConfigRemove(_ context.Context, _ string) error {
	return m.configRemoveErr
}

func (m *mockDockerClient) ContainerExecCreate(ctx context.Context, ctr string, opts container.ExecOptions) (types.IDResponse, error) {
	if m.execCreateFn != nil {
		return m.execCreateFn(ctx, ctr, opts)
	}
	return types.IDResponse{}, nil
}

func (m *mockDockerClient) ContainerExecAttach(ctx context.Context, execID string, opts container.ExecAttachOptions) (types.HijackedResponse, error) {
	if m.execAttachFn != nil {
		return m.execAttachFn(ctx, execID, opts)
	}
	return types.HijackedResponse{}, nil
}

func (m *mockDockerClient) ContainerExecResize(ctx context.Context, execID string, opts container.ResizeOptions) error {
	if m.execResizeFn != nil {
		return m.execResizeFn(ctx, execID, opts)
	}
	return nil
}

func (m *mockDockerClient) ContainerExecInspect(ctx context.Context, execID string) (container.ExecInspect, error) {
	if m.execInspectFn != nil {
		return m.execInspectFn(ctx, execID)
	}
	return container.ExecInspect{}, nil
}

func (m *mockDockerClient) ImageList(_ context.Context, _ image.ListOptions) ([]image.Summary, error) {
	return nil, nil
}

func (m *mockDockerClient) ImageRemove(_ context.Context, _ string, _ image.RemoveOptions) ([]image.DeleteResponse, error) {
	return nil, nil
}

func (m *mockDockerClient) ImagesPrune(_ context.Context, _ filters.Args) (image.PruneReport, error) {
	return image.PruneReport{}, nil
}

func uint64Ptr(v uint64) *uint64 { return &v }

func TestListServicesHandler_ReplicatedWithStack(t *testing.T) {
	now := time.Now().UTC()
	mock := &mockDockerClient{
		services: []swarm.Service{
			{
				ID: "svc1",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{
						Name:   "web",
						Labels: map[string]string{"com.docker.stack.namespace": "myapp"},
					},
					Mode: swarm.ServiceMode{
						Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(3)},
					},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{
							Image: "nginx:latest@sha256:abc123",
						},
					},
				},
				Meta: swarm.Meta{UpdatedAt: now},
			},
		},
		tasks: []swarm.Task{
			{ServiceID: "svc1", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
			{ServiceID: "svc1", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
			{ServiceID: "svc1", Status: swarm.TaskStatus{State: swarm.TaskStateFailed}},
		},
	}

	action := NewListServicesAction(mock)
	handler := NewListServicesHandler(action)

	req := httptest.NewRequest("GET", "/api/services", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Services []ServiceItem `json:"services"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Services) != 1 {
		t.Fatalf("expected 1 service, got %d", len(resp.Services))
	}

	svc := resp.Services[0]
	if svc.ID != "svc1" {
		t.Errorf("expected id svc1, got %s", svc.ID)
	}
	if svc.Name != "web" {
		t.Errorf("expected name web, got %s", svc.Name)
	}
	if svc.Image != "nginx:latest" {
		t.Errorf("expected image nginx:latest, got %s", svc.Image)
	}
	if svc.Stack != "myapp" {
		t.Errorf("expected stack myapp, got %s", svc.Stack)
	}
	if svc.ReplicasRunning != 2 {
		t.Errorf("expected 2 running, got %d", svc.ReplicasRunning)
	}
	if svc.ReplicasDesired != 3 {
		t.Errorf("expected 3 desired, got %d", svc.ReplicasDesired)
	}
}

func TestListServicesHandler_GlobalMode(t *testing.T) {
	mock := &mockDockerClient{
		services: []swarm.Service{
			{
				ID: "svc-global",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{Name: "monitoring"},
					Mode: swarm.ServiceMode{
						Global: &swarm.GlobalService{},
					},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{
							Image: "prometheus:v2",
						},
					},
				},
				Meta: swarm.Meta{UpdatedAt: time.Now().UTC()},
			},
		},
		tasks: []swarm.Task{
			{ServiceID: "svc-global", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
			{ServiceID: "svc-global", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
		},
	}

	action := NewListServicesAction(mock)
	handler := NewListServicesHandler(action)

	req := httptest.NewRequest("GET", "/api/services", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	var resp struct {
		Services []ServiceItem `json:"services"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	svc := resp.Services[0]
	if svc.ReplicasDesired != -1 {
		t.Errorf("expected desired -1 for global mode, got %d", svc.ReplicasDesired)
	}
	if svc.ReplicasRunning != 2 {
		t.Errorf("expected 2 running, got %d", svc.ReplicasRunning)
	}
	if svc.Stack != "" {
		t.Errorf("expected empty stack, got %s", svc.Stack)
	}
}

func TestListServicesHandler_EmptyList(t *testing.T) {
	mock := &mockDockerClient{
		services: []swarm.Service{},
		tasks:    []swarm.Task{},
	}

	action := NewListServicesAction(mock)
	handler := NewListServicesHandler(action)

	req := httptest.NewRequest("GET", "/api/services", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Services []ServiceItem `json:"services"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Services) != 0 {
		t.Errorf("expected 0 services, got %d", len(resp.Services))
	}
}

func TestListServicesHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		svcErr: errors.New("connection refused"),
	}

	action := NewListServicesAction(mock)
	handler := NewListServicesHandler(action)

	req := httptest.NewRequest("GET", "/api/services", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to list services" {
		t.Errorf("unexpected error message: %s", resp["error"])
	}
}

func TestListServicesHandler_TaskListError(t *testing.T) {
	mock := &mockDockerClient{
		services: []swarm.Service{
			{
				ID: "svc1",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{Name: "web"},
					Mode:        swarm.ServiceMode{Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(1)}},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{Image: "nginx"},
					},
				},
				Meta: swarm.Meta{UpdatedAt: time.Now().UTC()},
			},
		},
		taskErr: errors.New("task list failed"),
	}

	action := NewListServicesAction(mock)
	handler := NewListServicesHandler(action)

	req := httptest.NewRequest("GET", "/api/services", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

func TestListServicesHandler_StackFilter(t *testing.T) {
	now := time.Now().UTC()
	mock := &mockDockerClient{
		services: []swarm.Service{
			{
				ID: "svc-web",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{
						Name:   "web_app",
						Labels: map[string]string{"com.docker.stack.namespace": "web"},
					},
					Mode: swarm.ServiceMode{
						Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(2)},
					},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{Image: "nginx:latest"},
					},
				},
				Meta: swarm.Meta{UpdatedAt: now},
			},
		},
		tasks: []swarm.Task{
			{ServiceID: "svc-web", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
			{ServiceID: "svc-web", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
		},
	}

	action := NewListServicesAction(mock)
	handler := NewListServicesHandler(action)

	req := httptest.NewRequest("GET", "/api/services?stack=web", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	// Verify the label filter was passed to Docker
	filterValues := mock.lastServiceListOpts.Filters.Get("label")
	found := false
	for _, v := range filterValues {
		if v == "com.docker.stack.namespace=web" {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("expected label filter com.docker.stack.namespace=web, got %v", filterValues)
	}

	var resp struct {
		Services []ServiceItem `json:"services"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Services) != 1 {
		t.Fatalf("expected 1 service, got %d", len(resp.Services))
	}

	if resp.Services[0].Name != "web_app" {
		t.Errorf("expected service name web_app, got %s", resp.Services[0].Name)
	}
}

func TestListServicesHandler_NoStackFilter(t *testing.T) {
	mock := &mockDockerClient{
		services: []swarm.Service{},
		tasks:    []swarm.Task{},
	}

	action := NewListServicesAction(mock)
	handler := NewListServicesHandler(action)

	req := httptest.NewRequest("GET", "/api/services", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	// Verify no label filter was set
	filterValues := mock.lastServiceListOpts.Filters.Get("label")
	if len(filterValues) != 0 {
		t.Errorf("expected no label filters, got %v", filterValues)
	}
}
