package swarm

import (
	"context"
	"io"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/container"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type execSession struct {
	hijacked types.HijackedResponse
	execID   string
	client   DockerAPI
}

func (e *execSession) Read(p []byte) (int, error) {
	return e.hijacked.Reader.Read(p)
}

func (e *execSession) Write(p []byte) (int, error) {
	return e.hijacked.Conn.Write(p)
}

func (e *execSession) Close() error {
	e.hijacked.Close()
	return nil
}

func (e *execSession) Resize(cols, rows uint) error {
	return e.client.ContainerExecResize(context.Background(), e.execID, container.ResizeOptions{
		Width:  cols,
		Height: rows,
	})
}

// Compile-time check that execSession implements ExecSession
var _ docker.ExecSession = (*execSession)(nil)

func (s *SwarmOrchestrator) ExecContainer(ctx context.Context, containerID string, opts docker.ExecOptions) (docker.ExecSession, error) {
	shell := opts.Shell
	if shell == "" {
		shell = "/bin/sh"
	}

	execConfig := container.ExecOptions{
		Cmd:          []string{shell},
		AttachStdin:  true,
		AttachStdout: true,
		AttachStderr: true,
		Tty:          true,
		User:         opts.User,
	}

	resp, err := s.client.ContainerExecCreate(ctx, containerID, execConfig)
	if err != nil {
		return nil, err
	}

	hijacked, err := s.client.ContainerExecAttach(ctx, resp.ID, container.ExecAttachOptions{
		Tty: true,
	})
	if err != nil {
		return nil, err
	}

	if opts.Cols > 0 && opts.Rows > 0 {
		_ = s.client.ContainerExecResize(ctx, resp.ID, container.ResizeOptions{
			Width:  opts.Cols,
			Height: opts.Rows,
		})
	}

	return &execSession{
		hijacked: hijacked,
		execID:   resp.ID,
		client:   s.client,
	}, nil
}

// Ensure DockerAPI satisfies the exec-related methods
var _ interface {
	ContainerExecCreate(ctx context.Context, container string, config container.ExecOptions) (types.IDResponse, error)
	ContainerExecAttach(ctx context.Context, execID string, config container.ExecAttachOptions) (types.HijackedResponse, error)
	ContainerExecResize(ctx context.Context, execID string, options container.ResizeOptions) error
} = (DockerAPI)(nil)

// ExecInspectResult provides exec inspection data.
type ExecInspectResult struct {
	ExitCode int
	Running  bool
}

func (s *SwarmOrchestrator) InspectExec(ctx context.Context, execID string) (*ExecInspectResult, error) {
	resp, err := s.client.ContainerExecInspect(ctx, execID)
	if err != nil {
		return nil, err
	}

	return &ExecInspectResult{
		ExitCode: resp.ExitCode,
		Running:  resp.Running,
	}, nil
}

// Ensure io.ReadWriteCloser is satisfied
var _ io.ReadWriteCloser = (*execSession)(nil)
