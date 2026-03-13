package interceptors

import (
	"context"

	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"

	"github.com/prvious/kontainer/server/app/facades"
)

const SecretMetadataKey = "x-kontainer-secret"

// ValidateSecret checks the shared secret from gRPC metadata.
// Used by both unary and stream interceptors, and can be called
// directly from streaming handlers where framework-level stream
// interceptors are not available.
func ValidateSecret(ctx context.Context) error {
	secret := facades.Config().GetString("cluster.secret")
	if secret == "" {
		return status.Error(codes.Internal, "cluster secret not configured")
	}

	md, ok := metadata.FromIncomingContext(ctx)
	if !ok {
		return status.Error(codes.Unauthenticated, "missing metadata")
	}

	values := md.Get(SecretMetadataKey)
	if len(values) == 0 || values[0] != secret {
		return status.Error(codes.Unauthenticated, "invalid secret")
	}

	return nil
}

// SecretAuth validates the shared secret for unary RPCs.
func SecretAuth(ctx context.Context, req any, info *grpc.UnaryServerInfo, handler grpc.UnaryHandler) (any, error) {
	if err := ValidateSecret(ctx); err != nil {
		return nil, err
	}
	return handler(ctx, req)
}

// SecretAuthStream validates the shared secret for streaming RPCs.
func SecretAuthStream(srv any, ss grpc.ServerStream, info *grpc.StreamServerInfo, handler grpc.StreamHandler) error {
	if err := ValidateSecret(ss.Context()); err != nil {
		return err
	}
	return handler(srv, ss)
}
