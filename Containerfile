# syntax=docker/dockerfile:1
# ---- builder ----
# Not: etiket sabit; rustc 1.99.x. Guncellemek icin bu iki satiri degistirin.
FROM docker.io/library/rust:1.99-slim AS builder
WORKDIR /build

# Dependency cache layer: build with a dummy main.rs first.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src target/release/deps/rust_learning_app* target/release/rust_learning_app*

# Real sources (askama templates are needed at compile time).
COPY templates ./templates
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ---- runtime (rustc is required to compile student code) ----
FROM docker.io/library/rust:1.99-slim AS runtime
RUN useradd --uid 1000 --create-home --shell /usr/sbin/nologin app \
    && mkdir -p /app/data && chown app:app /app/data
WORKDIR /app
COPY --from=builder /build/target/release/rust_learning_app /app/rust_learning_app
COPY content ./content
COPY templates ./templates
COPY static ./static

ENV CONTENT_DIR=/app/content \
    DATA_DIR=/app/data \
    PORT=8080 \
    RUST_LOG=info
USER app
VOLUME /app/data
EXPOSE 8080
CMD ["/app/rust_learning_app"]
