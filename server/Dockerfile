# syntax=docker/dockerfile:1
# Kontainer images: `--target controller` or `--target agent`, built from server/.
# See README "Run on a Swarm" for the stack that runs them.

FROM rust:1.98-slim-trixie AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p kontainer-controller -p kontainer-node-agent \
        --bin kontainer-controller --bin kontainer-node-agent \
    && cp target/release/kontainer-controller target/release/kontainer-node-agent /usr/local/bin/

# Both run as root: they need the Docker socket, which is root-owned on the host.
FROM gcr.io/distroless/cc-debian13 AS controller
WORKDIR /app
COPY --from=build /usr/local/bin/kontainer-controller /usr/local/bin/
COPY config/production.yaml config/
ENV LOCO_ENV=production
EXPOSE 5150 50051
ENTRYPOINT ["kontainer-controller"]
CMD ["start"]

FROM gcr.io/distroless/cc-debian13 AS agent
COPY --from=build /usr/local/bin/kontainer-node-agent /usr/local/bin/
EXPOSE 50052
ENTRYPOINT ["kontainer-node-agent"]
