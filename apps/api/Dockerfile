FROM rust:1.82-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.toml
COPY apps/api apps/api
RUN cargo build --release -p metrics-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
RUN groupadd --system metrics && useradd --system --gid metrics --uid 10001 metrics \
    && mkdir -p /var/lib/metrics/artifacts \
    && chown -R metrics:metrics /var/lib/metrics
COPY --from=build --chown=metrics:metrics /src/target/release/metrics-api /usr/local/bin/metrics-api
USER metrics
EXPOSE 8080
CMD ["/usr/local/bin/metrics-api"]

