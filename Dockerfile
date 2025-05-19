FROM rust:1.82-slim as builder

# Install required packages, including the musl cross toolchain
RUN apt-get update && apt-get install -y \
    musl-tools \
    gcc-aarch64-linux-gnu \
    libc6-dev-arm64-cross \
    pkg-config \
    cmake \
    clang \
    curl \
    build-essential \
    && rustup target add aarch64-unknown-linux-musl

# Set linker for musl cross-compilation
ENV CC_aarch64_unknown_linux_musl=aarch64-linux-gnu-gcc

WORKDIR /app

# Copy full source and build
COPY . .
RUN cargo build --release --target aarch64-unknown-linux-musl
RUN strip target/aarch64-unknown-linux-musl/release/weather-provider

# -------- Stage 2: Minimal Runtime --------
FROM scratch

# Copy only the statically linked binary and CA certs
COPY --from=builder /app/target/aarch64-unknown-linux-musl/release/weather-provider /weather-provider
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

ENV RUST_LOG=info
ENTRYPOINT ["/weather-provider"]
