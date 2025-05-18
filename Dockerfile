# -------- Stage 1: Builder --------
FROM rust:slim as builder

# Add target for cross-compiling to ARM64
RUN rustup target add aarch64-unknown-linux-musl

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
