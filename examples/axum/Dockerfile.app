FROM rust:1.92-slim AS builder
WORKDIR /usr/src/synapse
COPY . .
RUN cargo build --release -p axum@0.1.0

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /usr/src/synapse/target/release/axum /usr/local/bin/axum
EXPOSE 8080
CMD ["axum"]
