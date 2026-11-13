FROM rust:1.82-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.toml
COPY apps/api apps/api
RUN cargo build --release -p metrics-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/metrics-api /usr/local/bin/metrics-api
EXPOSE 8080
CMD ["/usr/local/bin/metrics-api"]

