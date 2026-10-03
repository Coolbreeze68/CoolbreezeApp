# syntax=docker/dockerfile:1
# Image de mini_crm : l'API sert aussi l'application web, sur le port 8080.
#
#   docker compose up --build          (voir docker-compose.yml)
#   docker build --build-context forge=<sources de forge> -t mini_crm .
#
# Le projet dépend des sources de forge (forge-runtime, @forge/web), passées
# comme contexte nommé `forge`.

ARG DEBIAN=trixie
ARG RUST_VERSION=1

# ------------------------------------------------------- application web
FROM node:22-slim AS web
COPY --from=forge packages/forge_web/package.json /forge/packages/forge_web/
COPY --from=forge packages/forge_web/src /forge/packages/forge_web/src
COPY scripts/use-forge.sh /src/scripts/
COPY web /src/web
RUN sh /src/scripts/use-forge.sh /forge
WORKDIR /src/web
RUN --mount=type=cache,target=/root/.npm \
    npm install --no-audit --no-fund && npm run build

# ------------------------------------------------------------------- API
FROM rust:${RUST_VERSION}-slim-${DEBIAN} AS api
COPY --from=forge Cargo.toml Cargo.lock /forge/
COPY --from=forge crates /forge/crates
COPY scripts/use-forge.sh /src/scripts/
COPY backend /src/backend
RUN sh /src/scripts/use-forge.sh /forge
WORKDIR /src/backend
# Features supplémentaires, par exemple `forge-runtime/redis`.
ARG CARGO_FEATURES=""
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/backend/target \
    cargo build --release --features "$CARGO_FEATURES" \
 && cp target/release/mini_crm /usr/local/bin/mini_crm

# --------------------------------------------------------- image finale
FROM debian:${DEBIAN}-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /data app \
 && mkdir -p /data/uploads && chown -R app /data
COPY --from=api /usr/local/bin/mini_crm /usr/local/bin/mini_crm
COPY --from=web /src/web/dist /srv/web
USER app
WORKDIR /data
VOLUME /data
ENV FORGE_ADDR=0.0.0.0:8080 \
    FORGE_STATIC_DIR=/srv/web \
    FORGE_UPLOAD_DIR=/data/uploads \
    FORGE_LOG_FORMAT=json \
    DATABASE_URL=sqlite:///data/data.db?mode=rwc
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=6s --start-period=20s CMD ["mini_crm", "health"]
ENTRYPOINT ["mini_crm"]
