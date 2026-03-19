package controllers

import (
	"io"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/grpc/interceptors"
	"github.com/prvious/kontainer/server/app/services/docker"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

func (c *AgentController) Exec(stream pb.AgentService_ExecServer) error {
	// Validate shared secret (Goravel does not support framework-level stream interceptors)
	if err := interceptors.ValidateSecret(stream.Context()); err != nil {
		return err
	}

	// Wait for INIT message with container ID and shell
	initMsg, err := stream.Recv()
	if err != nil {
		return err
	}
	if initMsg.Type != pb.ExecMessage_INIT {
		return stream.Send(&pb.ExecMessage{
			Type:  pb.ExecMessage_ERROR,
			Error: "expected INIT message",
		})
	}

	containerID := string(initMsg.Data)
	shell := initMsg.Shell
	if shell == "" {
		shell = "/bin/sh"
	}

	session, err := c.orchestrator.ExecContainer(stream.Context(), containerID, docker.ExecOptions{
		Shell: shell,
		Cols:  uint(initMsg.Cols),
		Rows:  uint(initMsg.Rows),
	})
	if err != nil {
		return stream.Send(&pb.ExecMessage{
			Type:  pb.ExecMessage_ERROR,
			Error: err.Error(),
		})
	}
	defer session.Close()

	// Read from Docker exec → send to gRPC stream
	done := make(chan error, 2)
	go func() {
		buf := make([]byte, 4096)
		for {
			n, err := session.Read(buf)
			if n > 0 {
				if sendErr := stream.Send(&pb.ExecMessage{
					Type: pb.ExecMessage_DATA,
					Data: buf[:n],
				}); sendErr != nil {
					done <- sendErr
					return
				}
			}
			if err != nil {
				if err == io.EOF {
					if sendErr := stream.Send(&pb.ExecMessage{
						Type:     pb.ExecMessage_EXIT,
						ExitCode: 0,
					}); sendErr != nil {
						facades.Log().Errorf("exec: failed to send EXIT message: %v", sendErr)
					}
				}
				done <- err
				return
			}
		}
	}()

	// Read from gRPC stream → write to Docker exec
	go func() {
		for {
			msg, err := stream.Recv()
			if err != nil {
				done <- err
				return
			}

			switch msg.Type {
			case pb.ExecMessage_DATA:
				if _, err := session.Write(msg.Data); err != nil {
					done <- err
					return
				}
			case pb.ExecMessage_RESIZE:
				if err := session.Resize(uint(msg.Cols), uint(msg.Rows)); err != nil {
					facades.Log().Errorf("exec: resize failed: %v", err)
				}
			}
		}
	}()

	return <-done
}
