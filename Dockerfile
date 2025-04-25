FROM rust:latest AS builder

WORKDIR /app
COPY . .

# add targets and musl-tools
RUN apt-get update -y && apt-get install -y musl-tools
RUN rustup target add aarch64-unknown-linux-musl
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc

# build the binaries for server and client
RUN cargo build -p kv --release --target=aarch64-unknown-linux-musl
RUN cargo build -p kv --bin client --release --target=aarch64-unknown-linux-musl

FROM alpine:latest
WORKDIR /app

COPY --from=builder /app/target/aarch64-unknown-linux-musl/release/kv /usr/bin/quaso-server
COPY --from=builder /app/target/aarch64-unknown-linux-musl/release/client /usr/bin/quaso-cli

RUN apk update
RUN apk add vim tmux net-tools netcat-openbsd bash just

CMD ["bash"]
