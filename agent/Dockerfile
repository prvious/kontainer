# syntax=docker/dockerfile:1

# Build stage
FROM golang:1.25.1-alpine AS builder

# Install build dependencies
RUN apk add --no-cache git ca-certificates tzdata

WORKDIR /build

# Copy go mod files
COPY go.mod go.sum ./
RUN go mod download

# Copy source code
COPY . .

# Build the binary
# modernc.org/sqlite is pure Go — no CGO needed
RUN CGO_ENABLED=0 GOOS=linux go build -a -installsuffix cgo -ldflags="-s -w" -o agent .

# Runtime stage
FROM alpine:latest

# Install runtime dependencies
RUN apk --no-cache add ca-certificates tzdata

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/agent /app/agent

# Create directory for SQLite database (leader only)
RUN mkdir -p /data

# Expose default port
EXPOSE 8080

# Run as non-root user
RUN addgroup -g 1000 kontainer && \
    adduser -D -u 1000 -G kontainer kontainer && \
    chown -R kontainer:kontainer /app /data

USER kontainer

# Set default environment variables
ENV PORT=8080 \
    KONTAINER_ROLE=leader \
    KONTAINER_METRICS_DB=/data/kontainer-metrics.db

ENTRYPOINT ["/app/agent"]
