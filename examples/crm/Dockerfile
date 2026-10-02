# syntax=docker/dockerfile:1
# check=skip=FromPlatformFlagConstDisallowed
# Image de mini_crm : l'API sert aussi l'application web, sur le port 8080.
#
#   docker compose up --build          (voir docker-compose.yml)
#   docker build --build-context forge=<sources de forge> -t mini_crm .
#
# Le projet dépend des sources de forge (forge-runtime, forge_flutter), passées
# comme contexte nommé `forge`.

ARG DEBIAN=trixie
ARG RUST_VERSION=1
ARG FLUTTER_VERSION=3.47.6

# ------------------------------------------------------- application web
# Le SDK Flutter pour Linux n'existe qu'en x86-64 ; le résultat (JavaScript)
# ne dépend pas de la plateforme.
FROM --platform=linux/amd64 debian:${DEBIAN}-slim AS web
ARG FLUTTER_VERSION
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl git unzip xz-utils \
 && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL "https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${FLUTTER_VERSION}-stable.tar.xz" \
  | tar -xJ -C /opt \
 && git config --global --add safe.directory /opt/flutter
ENV PATH=/opt/flutter/bin:$PATH
COPY --from=forge packages/forge_flutter /forge/packages/forge_flutter
COPY scripts/use-forge.sh /src/scripts/
COPY app /src/app
COPY backend/Cargo.toml /src/backend/
RUN sh /src/scripts/use-forge.sh /forge
WORKDIR /src/app
RUN flutter pub get && flutter build web --release --no-web-resources-cdn

# ------------------------------------------------------------------- API
FROM rust:${RUST_VERSION}-slim-${DEBIAN} AS api
COPY --from=forge Cargo.toml Cargo.lock /forge/
COPY --from=forge crates /forge/crates
COPY scripts/use-forge.sh /src/scripts/
COPY backend /src/backend
COPY app/pubspec.yaml /src/app/
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
 && mkdir /data && chown app /data
COPY --from=api /usr/local/bin/mini_crm /usr/local/bin/mini_crm
COPY --from=web /src/app/build/web /srv/web
USER app
WORKDIR /data
VOLUME /data
ENV FORGE_ADDR=0.0.0.0:8080 \
    FORGE_STATIC_DIR=/srv/web \
    FORGE_LOG_FORMAT=json \
    DATABASE_URL=sqlite:///data/data.db?mode=rwc
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=6s --start-period=20s CMD ["mini_crm", "health"]
ENTRYPOINT ["mini_crm"]
