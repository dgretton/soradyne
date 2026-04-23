# Multi-stage build for soradyne-cli integration tests.
# Build context must be the repo root (or at least include packages/).
#
#   docker build -f packages/soradyne_core/tests/docker/Dockerfile -t soradyne-test .

FROM rust:1-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Copy both packages (soradyne_cli depends on soradyne_core via relative path)
COPY packages/soradyne_core/ packages/soradyne_core/
COPY packages/soradyne_cli/ packages/soradyne_cli/

RUN cd packages/soradyne_cli && cargo build --release --no-default-features 2>&1

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends procps && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/packages/soradyne_cli/target/release/soradyne-cli /usr/local/bin/

ENV RUST_LOG=info

ENTRYPOINT ["soradyne-cli"]
