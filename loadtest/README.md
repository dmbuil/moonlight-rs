# Moonlight load tests

How much load one Moonlight replica takes, measured with scripted users
([Goose](https://book.goose.rs)) in front of a fake OpenStack that answers at once, so the
numbers are Moonlight's own (SSR, server functions, mapping, caches, cookie decryption) and not
Keystone's or Nova's.

This is a crate of its own (`loadtest/`, its own `Cargo.toml` and `target/`): none of it is built
into Moonlight's server or WASM.

- `fake-openstack`: Keystone, Nova, Neutron, Cinder and Glance on one port, GET only (writes
  answer `405`), any password accepted, a distinct token per sign-in. Project size and latency are
  options: `--servers`, `--volumes`, `--images`, `--flavors`, `--networks`, `--delay-ms`,
  `--jitter-ms`. An API path it doesn't know answers `404` and is printed, and Ctrl-C prints the
  requests it answered per endpoint (how many OpenStack calls the run cost).
- `scenarios`: Goose users that sign in once and then loop their journey with 1–3 s of think time.
  Two scenarios, half the users each unless `--scenarios` picks one:
  - `instancesuser`: Project › Compute › Instances only, as a full page load (weight 1) and as an
    in-app navigation, which only calls `list_instances` (weight 3);
  - `projectuser`: a working session: Overview (`overview`, `get_usage`), Instances (twice), an
    instance's detail (twice, `get_instance`, the IDs taken from the user's own list), Volumes and
    Networks as in-app navigations, plus one full load per round of one of those pages, in turn.

## Run it

Moonlight must be a release build: the dev server's debug build and its 44 MB WASM say nothing
about production. Never build release while `cargo leptos watch` is serving (AGENTS.md §6); run
`scripts/dev.sh stop` first, and `scripts/dev.sh start` afterwards rebuilds the dev bundle.

```bash
# 1. Moonlight, release (from the repository root)
cargo leptos build --release --precompress

# 2. The fake OpenStack (from loadtest/)
cargo build --release
target/release/fake-openstack --servers 50 --volumes 50

# 3. Moonlight against it, on another port than the dev server (from the repository root)
LEPTOS_SITE_ADDR=127.0.0.1:3100 LEPTOS_SITE_ROOT=target/site \
  ./target/release/moonlight --config-file loadtest/moonlight.loadtest.conf

# 4. The users (from loadtest/)
target/release/scenarios --host http://127.0.0.1:3100 --users 50 --increase-rate 10 \
  --run-time 2m --report-file report.html
```

`moonlight.loadtest.conf` points `[keystone] url` at the stub, turns off the sign-in throttle
(every Goose user signs in once, and with `[log] client_ip_header` unset they would all be the
same address) and uses plain-HTTP cookies. Use it only for this, never for the dev server.

Scenario settings come from the environment: `LOADTEST_WAIT_MS` (`min,max` think time, default
`1000,3000`; `0,0` for a throughput run), `LOADTEST_USER_PREFIX` (`loaduser`),
`LOADTEST_PASSWORD` and `LOADTEST_DOMAIN` (empty by default: Moonlight then uses its
`[keystone] default_domain`, as the login form does). Goose's own options are on the command line
(`target/release/scenarios --help`).

Cold cache: set `[cache] lookup_seconds = 0` in a copy of the config and run again. A slow cloud:
`fake-openstack --delay-ms 100 --jitter-ms 100`. Moonlight's memory and CPU during a run:
`ps -o rss=,%cpu= -p $(pgrep -x moonlight)`.

## Against a real cloud

The same `scenarios` binary runs against a Moonlight that talks to a real (test) cloud: point
`--host` at it, keep the user count low (5–20) and the run short. After signing in, the journey
only reads. Accounts and password come from the environment, typed in your own shell and never
written to a file:

```bash
export LOADTEST_USERS="user1,user2"
read -rs LOADTEST_PASSWORD && export LOADTEST_PASSWORD
target/release/scenarios --host http://127.0.0.1:3000 --scenarios projectuser --users 5 \
  --increase-rate 1 --run-time 2m --report-file real.html
```

Each Goose user takes the accounts in turn. With `RUST_LOG=moonlight=debug`, Moonlight's log
counts the OpenStack calls of every server function (`calls=`, `connect_ms`, `body_ms`), next to
Goose's latencies.

## Micro-benchmarks

`benches/mapping.rs`, in Moonlight's own crate, times the pure mappers (`server::mapping`) on
100, 1,000 and 5,000 rows, plus the JSON a list answer becomes:

```bash
export CARGO_TARGET_DIR=target/bench
cargo bench --features ssr --bench mapping -- --save-baseline before   # on the old code
cargo bench --features ssr --bench mapping -- --baseline before        # on the new code
```

With its own target directory: a bench builds the package's binaries in the `release` profile
too, so in the default `target/` it overwrites the `target/release/moonlight` that
`cargo leptos build --release` made, and that binary then starts without cargo-leptos's settings
(it warns that `LEPTOS_OUTPUT_NAME` is not set).

For numbers comparable between two versions, run everything on one machine with nothing else
busy. For absolute numbers, put the load generator on another host.

## Baselines

A summary of the results is in the main README, *Performance*; every run (machine, users,
requests per second, percentiles, memory, OpenStack calls) is in `ROADMAP.md`, item 120.
