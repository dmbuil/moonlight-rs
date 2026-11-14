<!-- rumdl-disable MD033 -->
<!-- markdownlint-disable MD033 -->

# Moonlight

<div align="center">

<img src="./doc/moonlight-logo.png" alt="Moonlight logo" width="200" height="200">

[![CI](https://github.com/dmbuil/moonlight-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/dmbuil/moonlight-rs/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fdmbuil%2Fmoonlight-rs%2FHEAD%2FCargo.toml&query=%24.package.version&label=version)](Cargo.toml)
[![Rust](https://img.shields.io/badge/rust-1.91%2B-blue)](https://www.rust-lang.org/)
[![Leptos](https://img.shields.io/badge/leptos-0.8-blue)](https://leptos.dev)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS-lightgrey)
[![Last commit](https://img.shields.io/github/last-commit/dmbuil/moonlight-rs)](https://github.com/dmbuil/moonlight-rs/commits)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE.md)

*Leaving Horizon behind:* a fast, stateless OpenStack dashboard that keeps Horizon's layout
and modernises its controls.

| 🚀 [**Quick start**](#quick-start)
| ⚙️ [**Configuration**](CONFIGURATION.md)
| 🐳 [**Container**](#container-image-podman)
| 🏗️ [**Architecture**](ARCHITECTURE.md)
| 🗺️ [**Roadmap**](ROADMAP.md)
| 📄 [**License**](LICENSE.md) |

</div>

Moonlight is a *fast* OpenStack dashboard written in Rust:
[Leptos](https://leptos.dev) (server-side rendering + WASM hydration) on Axum, talking to
OpenStack through [`openstack_sdk`](https://github.com/gtema/openstack).

It is a **proof of concept** that replicates Horizon's *Project* dashboard: same structure,
same look and feel as the corporate Horizon theme but with modern controls, English/Spanish UI
and a server-enforced safety switch for every write.

- [Quick start](#quick-start)
- [Running the test server](#running-the-test-server)
- [Container image (Podman)](#container-image-podman)
- [Configuration](#configuration)
- [Features](#features)
- [Browser support](#browser-support)
- [Architecture](#architecture)
- [Tests](#tests)
- [Troubleshooting](#troubleshooting)
- [Status and roadmap](#status-and-roadmap)
- [License](#license)

## Quick start

Requirements: Rust ≥ 1.91 (tested with 1.98), the WASM target and `cargo-leptos`.

```bash
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos --locked

cp moonlight.conf.example moonlight.conf   # git-ignored
# edit it: [keystone] url and [session] cookie_key (see CONFIGURATION.md)
openssl rand -base64 64 | tr -d '\n'       # → paste as cookie_key

cargo leptos watch -- --config-file moonlight.conf
```

Open <http://127.0.0.1:3000> and sign in with your Keystone **domain / user / password**.

> [!WARNING]
> If the configuration points at a **production** cloud, keep `[writes] mode = "dry-run"`
> (the default).  
> See [Writes](#writes-off--dry-run--on).

## Running the test server

### 1. Development server (live reload)

```bash
cargo leptos watch -- --config-file moonlight.conf
```

- Builds the server binary (`ssr` feature) and the WASM front-end (`hydrate` feature),
  serves on **<http://127.0.0.1:3000>** and opens a live-reload socket on port **3001**.
- Any change under `src/`, `style/` or `public/` rebuilds and restarts automatically
  (a full rebuild takes ~1–2 min; incremental ones are faster).
- The repo-local `moonlight.conf` is re-read on every restart, so changing a setting only
  needs a save of any source file or a restart of the command. Arguments after `--` go to the
  server; without them it falls back to the default paths (`~/.moonlight/moonlight.conf`, …).
  `MOONLIGHT_*` environment variables still override the file.
- Logs go to stdout. `RUST_LOG=moonlight=debug` for more detail.

To build once without watching: `cargo leptos build` then `cargo leptos serve`.

### 2. Release build

> [!WARNING]
> **Stop `cargo leptos watch` first**  
> Both commands write the front-end bundle to the
> same `target/site/pkg/`. A release `.wasm` cannot hydrate the HTML of the debug server
> (it carries hot-reload markers), so the page renders but **stops responding to clicks**.
> If it happens, stop everything and start `cargo leptos watch` again.

```bash
cargo leptos build --release --precompress
./target/release/moonlight          # run from the repository root
```

`--precompress` writes brotli and gzip copies of the bundles (`target/site/pkg/*.br`, `*.gz`),
which the server sends as they are. Without it the server gzips the WASM, JS and CSS on each
request instead; pages and server-function responses are never compressed.

The binary reads its Leptos settings from `Cargo.toml` when started from the repository
root. To run it elsewhere (a container, another host), copy `target/release/moonlight` and
`target/site/` and set:

```bash
export LEPTOS_OUTPUT_NAME=moonlight
export LEPTOS_SITE_ROOT=site            # path to the copied target/site
export LEPTOS_SITE_PKG_DIR=pkg
export LEPTOS_SITE_ADDR=0.0.0.0:3000
export LEPTOS_ENV=PROD
./moonlight
```

...plus `--config-file /etc/moonlight/moonlight.conf` (or any default path, see
[CONFIGURATION.md](CONFIGURATION.md)). Session cookies are `Secure` by default; for plain-HTTP
development (`http://127.0.0.1:3000`) set `[session] secure_cookies = false`.

### 3. Against a test cloud (Devstack)

Real writes (`[writes] mode = "on"`) must only target a non-production cloud. Options:

- **A lab project/region** of your own cloud with a dedicated test user.
- **[DevStack](https://docs.openstack.org/devstack/latest/)** on a VM (≥ 8 GB RAM,
4 vCPU, Ubuntu 22.04/24.04):

  ```bash
  sudo useradd -s /bin/bash -d /opt/stack -m stack && echo "stack ALL=(ALL) NOPASSWD: ALL" | sudo tee /etc/sudoers.d/stack
  sudo -iu stack
  git clone https://opendev.org/openstack/devstack && cd devstack
  cat > local.conf <<'EOF'
  [[local|localrc]]
  ADMIN_PASSWORD=secret
  DATABASE_PASSWORD=$ADMIN_PASSWORD
  RABBIT_PASSWORD=$ADMIN_PASSWORD
  SERVICE_PASSWORD=$ADMIN_PASSWORD
  EOF
  ./stack.sh
  ```

  Then point Moonlight at it:

  ```toml
  [keystone]
  url = "http://<devstack-ip>/identity"
  default_region = "RegionOne"

  [writes]
  mode = "on"
  ```

  and sign in as `demo` / `secret` (project `demo`).

A good habit is one file per target, e.g. `prod.conf` (dry-run) and `devstack.conf` (on),
passed with `--config-file`.

## Container image (Podman)

The [`Dockerfile`](Dockerfile) builds in two stages: a Rust image compiles the server and the
WASM front end with `cargo leptos build --release --precompress`, then a slim Debian image keeps
only the binary, `target/site/` and the CA certificates, and runs as an unprivileged user.
Docker works the same way (`docker build`).

```bash
podman build -t localhost/moonlight .
podman run --rm -p 3000:3000 \
  -v ./moonlight.conf:/etc/moonlight/moonlight.conf:ro,Z \
  localhost/moonlight
```

Use the full name `localhost/moonlight`: given the short name `moonlight`, podman looks it up in
the registries when it is not in local storage (a failed build, or one run as another user or
in another `podman machine`), and asks which image to pull. `podman images localhost/moonlight`
shows whether the build produced it.

- **Configuration is mounted, never built in:** `moonlight.conf` holds the cookie key, and
  [`.containerignore`](.containerignore) keeps it, the logs and `target/` out of the build
  context. `/etc/moonlight/moonlight.conf` is a default path, so no `--config-file` is needed;
  `MOONLIGHT_*` variables (`-e`) override it as usual.
- **Logs:** the container writes only to `/var/log/moonlight` (`/app` is read-only for its
  user, so a relative `[log] file = "./moonlight.log"` fails with *Permission denied*). Either
  log to stdout (`[log] file = ""`, read with `podman logs`) or pass
  `-e MOONLIGHT_LOG_FILE=/var/log/moonlight/moonlight.log`, adding
  `-v moonlight-logs:/var/log/moonlight` to keep the files. An empty `MOONLIGHT_LOG_FILE` is
  ignored, so it can't switch a configured file off.
- **Logs in a host folder** (`-v ./logs:/var/log/moonlight:Z`): the container user is uid
  10001, which must be able to write there. With rootful podman (uids are the host's), run
  `chown 10001:10001 logs` once; with rootless podman, add
  `--userns=keep-id:uid=10001,gid=10001` so that uid is your own user.
- **TLS:** session cookies are `Secure` by default, so either put the container behind an
  HTTPS proxy, or mount a certificate and its key and set `MOONLIGHT_TLS_CERT_FILE` and
  `MOONLIGHT_TLS_KEY_FILE` (`[server]` in CONFIGURATION.md): Moonlight then serves HTTPS on
  port 3000 and picks up a renewed certificate by itself. For a local test over plain HTTP, set
  `[session] secure_cookies = false`.
- **Private CA:** mount the CA bundle and point `[tls] ca_file` at it.
- **Rebuilds** reuse the Cargo registry and `target/` through cache mounts. Build arguments
  `RUST_VERSION`, `DEBIAN_RELEASE` and `CARGO_LEPTOS_VERSION` change the toolchain
  (`--build-arg RUST_VERSION=1.91`).

## Configuration

Moonlight reads TOML files named `moonlight.conf`, layered the way oslo.config layers Nova's and
Neutron's; `MOONLIGHT_*` environment variables override them. Only `[keystone] url` is
required.

[CONFIGURATION.md](CONFIGURATION.md) lists every setting with its default and its
Horizon equivalent; [`moonlight.conf.example`](moonlight.conf.example) is a ready-to-copy file.

### Writes: off / dry-run / on

Every mutating action goes through one server function (`run_action`) and one executor
(`src/server/actions/`). The mode is **enforced on the server**; the UI only reflects it.

- `off`: every action is rejected; action buttons are disabled and a “Read-only” badge shows.
- `dry-run` *(default)*: each request is built with the SDK's typed endpoints (the same code
  path as `on`) and shown in the dialog. **Nothing is sent**; no connection is even opened.
- `on`: requests are sent with the user's own project-scoped token; Keystone policies apply.

Every write is logged with user, project, mode and action name (no IDs, names or keys). See
[ARCHITECTURE.md](ARCHITECTURE.md#writes) for the full path.

## Features

### Navigation and session

- Keystone login (domain / user / password), unscoped token kept in an encrypted cookie.
- Project switcher with search, region switcher, user menu (sign out).
- **English (US) / Español (España)**, switched instantly without reload; remembered in the
  `ml_lang` cookie, else taken from `Accept-Language`. Dates, sizes and decimals follow the language.
- Every table: instant filter (all terms must match), sortable columns, “Displaying N items”,
  refresh button and a chip showing backend cost (e.g. `3 API · 0.41 s`).

### Panels, details and actions

| Panel                | Detail page                                                                                                                         | Actions                                                                                                                                                                                                                                                                                                    |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **API Access**       | —                                                                                                                                   | —                                                                                                                                                                                                                                                                                                          |
| **Overview**         | quota donuts (compute, volume, network)                                                                                             | —                                                                                                                                                                                                                                                                                                          |
| **Instances**        | Overview · Interfaces · Log* · Console* · Action Log                                                                                | Launch (image / volume / volume snapshot, boot from new volume, flavor, networks, security groups, key pair, server group, count); create snapshot, rename, attach/detach volume, pause/unpause, suspend/resume, lock/unlock, soft/hard reboot, shut off/start, delete — menu adapts to the instance state |
| **Images**           | info, specs, custom properties                                                                                                      | create volume from image; edit (name, visibility, protected) and delete for project-owned images                                                                                                                                                                                                           |
| **Key Pairs**        | fingerprint, public key                                                                                                             | import public key, delete                                                                                                                                                                                                                                                                                  |
| **Server Groups**    | —                                                                                                                                   | create (policy), delete                                                                                                                                                                                                                                                                                    |
| **Volumes**          | info, specs, attachments, metadata, image metadata, snapshots                                                                       | create (empty / image / snapshot / volume), edit, extend, create snapshot, attach/detach, delete                                                                                                                                                                                                           |
| **Snapshots**        | info, source volume                                                                                                                 | create volume from snapshot, edit, delete                                                                                                                                                                                                                                                                  |
| **Network Topology** | Topology (networks as bars, routers and instances wired to them) · Graph (force layout) · List; hover popover, click opens the node | launch instance, create network, create router                                                                                                                                                                                                                                                             |
| **Networks**         | Overview · Subnets · Ports; plus subnet and port detail pages                                                                       | create (with optional subnet: CIDR, gateway or none, DHCP, pools, DNS), edit, delete; create/edit/delete subnets and ports                                                                                                                                                                                 |
| **Routers**          | Overview · Interfaces · Static Routes                                                                                               | create (with external network), edit, set/clear gateway, add/remove interface, delete                                                                                                                                                                                                                      |
| **Security Groups**  | rules table                                                                                                                         | create, edit, delete; add rule (direction, ethertype, protocol, ports, CIDR or remote group), delete rule                                                                                                                                                                                                  |
| **Floating IPs**     | —                                                                                                                                   | allocate (pool), associate to an instance port, disassociate, release                                                                                                                                                                                                                                      |

\* Log and Console need a POST to Nova: they are sent only with `[writes] mode = on` and
`[console] enabled = true` (off by default); in dry-run they show the request instead.

Every resource name is a link: instances ↔ volumes ↔ snapshots, ports ↔ instances,
networks ↔ subnets ↔ ports ↔ routers, floating IPs → instances, rules → remote groups.

Destructive actions ask for confirmation (“You have selected: X. This action cannot be undone.”),
like Horizon.

## Browser support

Moonlight is a server-rendered page that becomes interactive once its WebAssembly bundle loads.
Reading works in any browser that can render the HTML; sorting, filtering, dialogs and actions
need WebAssembly and JavaScript modules. The floor is set by one CSS function the theme is built
on, `light-dark()`, which gives every colour token its light and dark value:

| Browser                          | Minimum version     |
| -------------------------------- | ------------------- |
| Chrome, Edge, Chrome for Android | 123 (March 2024)    |
| Firefox, Firefox for Android     | 120 (November 2023) |
| Safari, iOS Safari               | 17.5 (May 2024)     |

Everything else the app uses has been supported for longer: WebAssembly with the features Rust
enables by default (bulk memory, reference types, sign extension, non-trapping float
conversions, multi-value), `dvh` viewport units, `prefers-color-scheme` and
`prefers-reduced-motion`, pointer and touch events, the Clipboard API (which needs HTTPS or
`localhost`), and `XMLHttpRequest` upload progress. In an older browser the colour tokens do
not resolve, so the page renders in the browser's default colours while the layout and the
WebAssembly still work. The VNC console is Nova's own noVNC page in a frame, so its
requirements are the cloud's.

## Architecture

Leptos server-side rendering plus WASM hydration on Axum; a stateless server that keeps the
user's unscoped Keystone token in an AES-GCM encrypted cookie and rescopes it per request;
parallel GET-only reads joined by pure, tested mappers; one server-enforced write path.

Modules, the session and its cookies, the password and WebSSO sign-in flows, caches and the
security measures are described, with diagrams, in [ARCHITECTURE.md](ARCHITECTURE.md).
[SECURITY.md](SECURITY.md) has the threat model, every measure with its limits, the accepted
risks and the deployment checklist.

## Tests

Tests live next to the code in `#[cfg(test)]` modules and need no cloud. Run them with
[cargo-nextest](https://nexte.st):

```bash
cargo install cargo-nextest --locked                 # once
cargo nextest run --features ssr                     # everything
cargo nextest run --features ssr actions             # every write, in dry-run, asserting the exact JSON
```

What they cover: [ARCHITECTURE.md](ARCHITECTURE.md#testing-strategy). The full pre-commit
check list (both clippy targets, Markdown lint) is in [AGENTS.md](AGENTS.md#2-commands).

## Troubleshooting

| Symptom                                                             | Cause / fix                                                                                                                             |
| ------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `configuration error: [keystone] url is not set`                    | Copy `moonlight.conf.example` to `~/.moonlight/moonlight.conf` and set it.                                                              |
| Login always fails                                                  | Wrong domain (check what Horizon uses) or Keystone unreachable; see the server log.                                                     |
| Logged out after every restart                                      | `[session] cookie_key` is empty.                                                                                                        |
| A panel shows `404` errors from Neutron/Glance                      | Version discovery failed; check `discovery of … failed` in the log.                                                                     |
| `ERROR openstack_sdk… No catalog information` at login              | Harmless: logged by the SDK for the unscoped token.                                                                                     |
| Cannot type in dialog fields                                        | A password-manager extension (e.g. 1Password) overlays inputs on `127.0.0.1`; disable it for the site.                                  |
| Page renders but clicks do nothing (links, menus, project switcher) | The served WASM does not match the server build (e.g. a `--release` build ran while `watch` was serving). Restart `cargo leptos watch`. |
| Sent back to the login page after ~1 hour                           | The Keystone token expired (default lifetime 1 h). Sign in again.                                                                       |
| Port 3000/3001 already in use                                       | Change `site-addr` / `reload-port` in `Cargo.toml`.                                                                                     |

## Status and roadmap

Verified against a dev/prod OpenStack cloud in read mode and in dry-run for writes;
real writes have only been validated by tests, not against a live production-grade
cloud.

## License

Apache License 2.0: see [LICENSE.md](LICENSE.md).
