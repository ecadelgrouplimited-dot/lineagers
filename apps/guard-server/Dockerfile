# Build from the repository root (the server depends on the lineage crate by path):
#   docker build -f apps/guard-server/Dockerfile -t lineage-guard .
FROM rust:1.89-slim AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY examples ./examples
COPY apps/guard-server ./apps/guard-server
RUN cargo build --release --locked --manifest-path apps/guard-server/Cargo.toml

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --home-dir /data guard && mkdir -p /data && chown guard /data
COPY --from=build /src/apps/guard-server/target/release/guard-server /usr/local/bin/guard-server
USER guard
ENV GUARD_DATA_DIR=/data GUARD_BIND=0.0.0.0:9200
VOLUME /data
EXPOSE 9200
ENTRYPOINT ["guard-server"]
