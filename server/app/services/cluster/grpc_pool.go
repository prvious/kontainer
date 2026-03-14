package cluster

import (
	"context"
	"fmt"
	"sync"

	pb "github.com/prvious/kontainer/server/proto/agent"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"
)

// GrpcPool manages gRPC client connections to follower agents.
// Connections are created on demand and cached by address.
type GrpcPool struct {
	mu     sync.RWMutex
	conns  map[string]*grpc.ClientConn
	secret string
}

func NewGrpcPool(secret string) *GrpcPool {
	return &GrpcPool{
		conns:  make(map[string]*grpc.ClientConn),
		secret: secret,
	}
}

// GetClient returns a gRPC AgentServiceClient for the given address.
// Creates and caches the connection if not already established.
func (p *GrpcPool) GetClient(address string) (pb.AgentServiceClient, error) {
	p.mu.RLock()
	conn, ok := p.conns[address]
	p.mu.RUnlock()
	if ok {
		return pb.NewAgentServiceClient(conn), nil
	}

	p.mu.Lock()
	defer p.mu.Unlock()

	// Double-check after acquiring write lock
	if conn, ok = p.conns[address]; ok {
		return pb.NewAgentServiceClient(conn), nil
	}

	conn, err := grpc.NewClient(address,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithUnaryInterceptor(p.unaryInterceptor()),
		grpc.WithStreamInterceptor(p.streamInterceptor()),
	)
	if err != nil {
		return nil, fmt.Errorf("grpc dial %s: %w", address, err)
	}

	p.conns[address] = conn
	return pb.NewAgentServiceClient(conn), nil
}

// Remove closes and removes the connection for the given address.
func (p *GrpcPool) Remove(address string) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if conn, ok := p.conns[address]; ok {
		conn.Close()
		delete(p.conns, address)
	}
}

// CloseAll closes all connections in the pool.
func (p *GrpcPool) CloseAll() {
	p.mu.Lock()
	defer p.mu.Unlock()
	for addr, conn := range p.conns {
		conn.Close()
		delete(p.conns, addr)
	}
}

func (p *GrpcPool) unaryInterceptor() grpc.UnaryClientInterceptor {
	return func(ctx context.Context, method string, req, reply any, cc *grpc.ClientConn, invoker grpc.UnaryInvoker, opts ...grpc.CallOption) error {
		ctx = metadata.AppendToOutgoingContext(ctx, "x-kontainer-secret", p.secret)
		return invoker(ctx, method, req, reply, cc, opts...)
	}
}

func (p *GrpcPool) streamInterceptor() grpc.StreamClientInterceptor {
	return func(ctx context.Context, desc *grpc.StreamDesc, cc *grpc.ClientConn, method string, streamer grpc.Streamer, opts ...grpc.CallOption) (grpc.ClientStream, error) {
		ctx = metadata.AppendToOutgoingContext(ctx, "x-kontainer-secret", p.secret)
		return streamer(ctx, desc, cc, method, opts...)
	}
}
