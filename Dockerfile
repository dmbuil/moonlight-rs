# Moonlight container image, two stages (see README.md › Container image):
#   1. build: compiles the server binary and the WASM front end with cargo-leptos.
#   2. runtime: a slim Debian with only the binary, the site bundle and the CA store.
#
#   podman build -t localhost/moonlight .
#   podman run --rm -p 3000:3000 -v ./moonlight.conf:/etc/moonlight/moonlight.conf:ro,Z \
#     localhost/moonlight
#
# The full name keeps podman from looking `moonlight` up in a registry (and prompting).
#
# No configuration is baked in: moonlight.conf holds the cookie key, so it is mounted at run time
# (and .containerignore keeps it out of the build context).

ARG RUST_VERSION=1.98
ARG DEBIAN_RELEASE=trixie

# --- Stage 1: build ---------------------------------------------------------------------------
FROM docker.io/library/rust:${RUST_VERSION}-slim-${DEBIAN_RELEASE} AS build

# The version the repo is built and tested with; cargo-leptos downloads the matching
# wasm-bindgen and wasm-opt itself.
ARG CARGO_LEPTOS_VERSION=0.3.11

RUN rustup target add wasm32-unknown-unknown \
 && cargo install cargo-leptos --locked --version "${CARGO_LEPTOS_VERSION}"

WORKDIR /src
COPY . .

# The cache mounts keep the registry and target/ between builds (podman/buildah support them).
# Because target/ is a cache, the artifacts are copied out to /out in the same step.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo leptos build --release \
 && mkdir -p /out \
 && cp target/release/moonlight /out/moonlight \
 && cp -r target/site /out/site

# --- Stage 2: runtime -------------------------------------------------------------------------
FROM docker.io/library/debian:${DEBIAN_RELEASE}-slim AS runtime

# rustls verifies Keystone's certificate against the system store (rustls-native-certs).
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /app --shell /usr/sbin/nologin moonlight \
 && mkdir -p /etc/moonlight \
 && install -d -o moonlight -g moonlight -m 0750 /var/log/moonlight

WORKDIR /app
COPY --from=build /out/moonlight /app/moonlight
COPY --from=build /out/site /app/site

# Without Cargo.toml next to the binary, Leptos reads its settings from these variables.
ENV LEPTOS_OUTPUT_NAME=moonlight \
    LEPTOS_SITE_ROOT=/app/site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=0.0.0.0:3000 \
    LEPTOS_ENV=PROD

USER moonlight
EXPOSE 3000
# The only directory the moonlight user can write: /app is root's, so a relative
# `[log] file` such as "./moonlight.log" fails with "Permission denied".
VOLUME /var/log/moonlight

# /etc/moonlight/moonlight.conf is one of the default paths, so no --config-file is needed.
ENTRYPOINT ["/app/moonlight"]
