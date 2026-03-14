package interceptors

import (
	"context"
	"crypto/subtle"

	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"

	"github.com/prvious/kontainer/server/app/facades"
)

const SecretMetadataKey = "x-kontainer-secret"

// ValidateSecret checks the shared secret from gRPC metadata.
// Called by the unary interceptor and directly from streaming handlers
// (Goravel does not support framework-level stream interceptors).
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
	if len(values) == 0 || subtle.ConstantTimeCompare([]byte(values[0]), []byte(secret)) != 1 {
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
