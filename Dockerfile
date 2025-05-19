FROM --platform=linux/arm64 rust:1.87-alpine3.20 AS builder

# Install required packages, including the musl cross toolchain
RUN apk add --no-cache openssl-dev musl-dev pkgconf && rustup target add aarch64-unknown-linux-musl

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
