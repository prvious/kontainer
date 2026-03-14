.PHONY: proto proto-install

# Regenerate protobuf Go code (requires buf: go install github.com/bufbuild/buf/cmd/buf@latest)
proto:
	buf generate

# Install buf and Go protobuf plugins
proto-install:
	go install github.com/bufbuild/buf/cmd/buf@latest
	go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
	go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@latest
