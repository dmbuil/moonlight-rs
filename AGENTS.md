# AGENTS.md — working on Moonlight

Guidance for AI agents (and humans) changing this repository. Read `README.md` for what
Moonlight is and `ARCHITECTURE.md` for how it is built (session, sign-in flows, caches); this
file is about **how to work on it safely and efficiently**. Everything here was learned the
hard way while building the PoC.

---

## 1. Safety rules (non-negotiable)

1. **The configured cloud may be production.** The repo-local `moonlight.conf` points at a
   production RHOSO cloud. Never change its `[writes] mode` away from `dry-run` (nor set
   `MOONLIGHT_WRITES`, which overrides it), and never send a mutating request
   (POST/PUT/PATCH/DELETE, or Nova `…/action`) to it. Real writes are only allowed against a
   cloud the user explicitly designates as a test cloud.
2. **Writes are enforced server-side.** All mutations must go through `OsAction` →
   `server::actions::apply` → `Exec`. Never call an SDK write endpoint from anywhere else,
   and never gate a write only in the UI.
3. **`fetch.rs` is GET-only.** Read paths must not use endpoints with side effects. The only
   read-only POSTs allowed are the instance Log and Console (`os-getConsoleOutput`,
   `remote-consoles`, `os-getVNCConsole`), in `server/console.rs`: they go through `Exec`, so
   they are sent only with `[writes] mode = on` and `[console] enabled = true`; and Heat's
   `POST validate` and `POST stacks/preview` (roadmap 62), in `server/heat.rs`, also through
   `Exec`. Don't add others without asking. The other write outside `OsAction` is the logout's token
   revocation (`actions::revoke_token`, `[session] revoke_on_logout`), also through `Exec`.
   The one write outside `Exec` is *Create Image* › *File* (roadmap 45, `server/upload.rs`): the
   data is streamed, which the SDK can't do. The record is still an `OsAction`
   (`ImageCreate`, whose `upload_bytes` makes dry-run preview the `PUT`). With `[images] upload =
   proxy` the `PUT /upload/image/{id}` route checks the session, `[writes] mode = on` and
   `max_upload_gb`, then forwards the body to Glance; with `direct` the server function
   `get_image_upload_ticket` makes the same checks and returns Glance's URL and the scoped
   token for that one request (§1.5: it travels only in that response and is never logged,
   stored or put in a `Debug` output). Both are off by default (`upload = "off"`).
4. **Horizon is reference material, read-only.** If a browser tab with the production
   Horizon is shared, you may navigate and read (DOM text, computed styles, menu hrefs). Do
   not click buttons, submit forms, or switch project/region there
   (`/auth/switch/…` changes the user's session). Prefer reading `href`s with JavaScript
   and navigating by URL.
5. **Never handle the user's credentials.** Do not type passwords into the login form; ask
   the user to sign in. Never log tokens; cache keys are `sha256` hashes on purpose.
6. **Never write tokens to disk.** Every SDK connection is built with `auth_cache: Some(false)`
   and `.disable_auth_cache(true)` (`server/os.rs`). Keep it that way.
7. Don't commit or push unless asked. `moonlight.conf` is git-ignored and holds a cookie key.

---

## 2. Commands

```bash
cargo leptos watch -- --config-file moonlight.conf   # dev server :3000, live reload :3001
cargo fmt                                            # format Rust (config: rustfmt.toml, width 120)
cargo fmt --check                                    # what CI runs: fails on unformatted code
cargo nextest run --features ssr                     # all tests (embedded #[cfg(test)])
cargo nextest run --features ssr actions             # write payloads, dry-run
cargo check --features ssr                           # server type-check (fast)
cargo check --lib --features hydrate --target wasm32-unknown-unknown   # client type-check
cargo clippy --features ssr --all-targets -- -D warnings              # server lints (incl. tests)
cargo clippy --lib --features hydrate --target wasm32-unknown-unknown -- -D warnings  # client lints
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --features ssr   # rustdoc (also hydrate)
cargo leptos build --release                         # ONLY with `watch` stopped (see §6)
rumdl check .                                        # Markdown lint (config: .rumdl.toml)
rumdl fmt .                                          # Markdown auto-fix
cargo deny check                                     # advisories, licences, duplicates, sources (deny.toml)
```

Always run **both** `cargo check` variants: code that compiles for `ssr` can fail for
`hydrate` (and vice versa) because of `#[cfg(feature = …)]` blocks and `web-sys` features.

**Whenever you add or change code**, before reporting it done:

1. Run `cargo fmt` (CI fails on `cargo fmt --check`).
2. Run **both** `cargo clippy` variants (they also type-check, so they replace `cargo check`)
   and keep them at **zero warnings** (the tree is clean; `-D warnings` fails on any). Don't
   silence a lint with `#[allow(…)]` unless it is a false positive, and then say why in a comment.
3. Run `cargo doc` (both variants, with `RUSTDOCFLAGS="-D warnings"`, as CI does) when you
   touched doc comments: it catches broken intra-doc links (`` [`gone_fn`] ``), bare URLs and
   bad code blocks. Doc comments may link to private items (`--document-private-items`).
4. Run the tests with `cargo nextest run --features ssr` (faster, parallel per test, clearer
   failure output). New logic gets a test next to it (§7). Fall back to `cargo test` only if
   doctests are added: nextest does not run them (`cargo test --doc --features ssr`).
5. Report the actual result (passed/failed counts, remaining warnings), not "should pass".
6. When a dependency is added or bumped, run `cargo deny check`: a RustSec advisory, a yanked
   version, a licence outside `deny.toml`'s allow list or a source other than crates.io fails
   it. Duplicate versions only warn; don't widen the policy to make it pass, fix the
   dependency or record an `ignore` with its reason.

**CI** (`.github/workflows/ci.yml`) runs these on every push to `main` and every pull request:
`cargo fmt --check`, both clippy variants, both `cargo doc` variants,
`cargo nextest run --features ssr`, `rumdl check .` and `cargo deny check`, each as its own job.
A red job is a real failure; fix the cause, never skip the job or loosen its flags.

**Whenever you add or change Markdown** (`AGENTS.md`, `README.md`, `ROADMAP.md`, …), run
`rumdl fmt .` and then `rumdl check .`, which must report no issues. `.rumdl.toml` sets
100-column lines (tables and code blocks exempt); wrap prose by hand at that width. After
`fmt`, read its diff: a wrapped line that starts with `+`, `-` or `*` becomes a list item,
so reword the sentence instead (e.g. "own, plus `shared=true`" rather than "own + …").
Fenced code blocks need a language (`text` for plain listings).

**Dev server: use `scripts/dev.sh`** rather than running `watch` by hand. `start` (no-op if it is
already running) and `restart` launch it in the background with `moonlight.conf` and block until
it serves. After an edit, `wait` blocks until watch's rebuild settles and prints only `ready …` or
the rustc errors (exit 0 ready, 1 failed, 2 timeout). `status`, `stop` and `logs [N]` (ANSI
stripped) cover the rest. The log is `$TMPDIR/moonlight-dev-server.log`, outside `target/` so a
`cargo clean` doesn't delete it.

---

## 3. Architecture in one screen

```text
src/api.rs           #[server] fns — the only browser↔server boundary
src/model/           DTOs (serde) per service; OsAction (action.rs), ChoiceSet, WriteMode
src/server/os.rs     login (v3password, unscoped) → per-request v3token rescope, cached (moka)
src/server/fetch.rs  parallel GETs (tokio::join!) → mapping
src/server/console.rs instance Log/Console: the only read-only POSTs, via Exec (§1.3)
src/server/mapping/  pure serde_json::Value → model, one file per service (unit-tested with
                     API-shaped JSON; shared payloads in mapping/fixtures.rs)
src/server/actions/  OsAction → typed SDK endpoints, one file per service; mod.rs: Exec
                     (Off | DryRun preview | On send) and `apply`, which dispatches each variant
src/components/      layout, nav model, DataTable, dialog engine, ops (per-resource actions)
src/pages/           login, overview, instances(+detail), lists, details
```

Session = AES-GCM private cookie (`cookie` crate) with the **unscoped** token, user, project,
region. The server is stateless; in-process caches are disposable.

---

## 4. Recipes

### Add a read-only panel or detail page

1. Model in `model/<service>.rs` (`Clone, Debug, PartialEq, Serialize, Deserialize`).
2. Pure mapper in `server/mapping/<service>.rs` + a test with JSON shaped like the real API
   (include odd cases: missing name → fall back to ID, `""` vs `null`, string booleans).
3. Fetcher in `server/fetch.rs`: lookups in parallel; use `soft()` for lists that only
   provide names so a failing lookup doesn't break the page; return `Timed<T>`.
4. `#[server(input = GetUrl)]` fn in `api.rs` whose body is `scoped(name, |s, c| …)`, or
   `scoped_id(name, id, |s, c, id| …)` for an ID from the URL. Both give the session and
   scoped connection, a timeout and the call count that `timed` reads. Don't type call counts:
   send requests through the `fetch.rs` macros or `counted(…)`, and spawn with `spawn_counted`.
5. Page: `Resource::new(move || (id(), data_version()), …)` inside a `<Transition>`;
   use `render()`, `DataTable`, `DetailHeader`, `Card`/`Kv`.
6. Route in `app.rs`; menu entry in `components/nav.rs` if it is a panel.
7. i18n keys (§5). Link the resource from every place that mentions it.

### Add a write action

1. Variant in `OsAction` (`model/action.rs`).
2. An `async fn` in `server/actions/<service>.rs` building the **typed SDK endpoint**, called
   from its arm in `actions::apply` (`mod.rs`); use `x.send(ep)` or `x.create(ep, "what")`
   (returns the new ID, placeholder in dry-run).
3. Test in that file's `mod tests` asserting method, path and the exact JSON body via `dry(...)`. Every
   variant needs one (`every_action_has_a_dry_run_test`), and `dry` also checks that no string
   field reaches the request path without `check_ids` (roadmap 67): name path parameters `id`
   or `*_id`, or add them to `OsAction::check_ids`.
4. UI in `components/ops/<service>.rs`: a `MenuItem` (+ `Dialog::form` /
   `Dialog::confirm`), wired into the list's `actions_col` and the detail page's `DetailHeader items`.
5. Verify in the browser **in dry-run only**; the dialog shows the request (with
   `[ui] show_requests = true`, as in the repo's `moonlight.conf`; off by default, roadmap 78).

### Gate a panel or action by what the cloud offers

Panels declare `requires` in `components/nav.rs`; actions and tabs check a constant from
`model::needs` against `dialog::caps()` (e.g. `needs::REBUILD.met(&caps())`). Add a
`Requires` to `needs` for every write that depends on a microversion or a Neutron extension,
and prefer a fallback over hiding when one exists (plain `extraroute` → `RouterSetRoutes`).
Unknown capabilities pass every check, so tests and a failed detection show everything. What
was detected is shown by `lists::ModulesCard`, which belongs to the (future) Admin dashboard,
not to Project › API Access.

Panels that Horizon gets from plugins (Octavia, Heat, later DNS, Swift…) are **opt-in**: add a
`model::Plugin` variant, require `Requires::All(&[Requires::Plugin(p), Requires::Service(…)])`,
call `plugin_enabled(p)?` in each of its server functions and list its actions in
`OsAction::plugins`. `Requires::Plugin` does not fail open.

### Add a dialog field

`Field::{text, area, number, check, select, multi}` with `.required()`, `.value()`,
`.help()`, `.when(field, value)`. Dynamic options come from `Source::Set(ChoiceSet::…)`
(add the set in `model/choice.rs` and `fetch::choices`); options that depend on another field use
`Source::Switch`, and the choices of one resource (e.g. one instance's ports) use
`Source::Of(set, id)`, which keeps those whose first cell is `id`. A value that is preselected
or preset by the caller runs `Dialog::on_change` once its set loads, as a user pick would.
Visibility is transitive: hiding a field hides fields that depend on it.

---

## 5. i18n

- Every string lives in the `translations!` table in `src/i18n.rs` with **both** English
  and Spanish; the macro makes a missing translation a compile error. Never hard-code UI text.
- Read the language reactively inside closures: `{move || i18n.t(K::X)}`. Code that builds
  dialogs/menus takes a `Lang` snapshot (fine: they are rebuilt on open).
- Language comes from the `ml_lang` cookie, else `Accept-Language`, computed identically on
  server and client so hydration matches. `Lang::EsEs` uses decimal commas.
- Horizon's Spanish UI is partially translated ("Visión general", "Orquestación"); we
  translate fully.

---

## 6. Gotchas we hit (and the fix)

### gtema `openstack_sdk` (0.22.x)

- **Version discovery is not automatic.** Catalog endpoints are often unversioned
  (`https://neutron…/`); without `discover_service_endpoint` calls are made to `/networks`
  instead of `/v2.0/networks` → 404. `os::discover` runs it once per cached connection.
- **Module names carry microversions and stale files exist in the crate dir.** Trust the
  `pub mod` lines of the parent `.rs`, not `ls`: e.g. `server::get_20`, `server::list_21`,
  `server::list_detailed_21`, `server_group::list_20`, `keypair::create_210`,
  `server_group::create_264`, `block_storage::v3::r#type::list`.
- Inspect a builder quickly: read the `pub struct Request` / nested structs in the module
  source under `~/.cargo/registry/src/*/openstack-sdk-<service>-<ver>/src/…`.
- **Not every list is `Pageable`** (e.g. volume types, AZs): use `ep.query_async()` directly,
  not `paged(...)`.
- **`Pagination::All` sends no `limit`**: the SDK's `page_limit()` is `usize::MAX` for both
  variants, so each service used its own default page (Glance: 25) and a list took one GET per
  page. Set it on the builder; `list_where!` does (`LIST_PAGE`, 1000). A list builder without
  `.limit()` then fails to compile in the macro: call `ep.query_async()` for it instead.
- **`response_key` varies**: `os-interface` returns the raw object (`interfaceAttachments`
  inside); keypairs come wrapped (`{"keypair": {...}}`) — use `mapping::unwrap`.
- Builders use `strip_option` + `into`: `Option<Cow<str>>` setters take `&str`/`String`;
  **`Option<Option<…>>` fields** (subnet `cidr`/`gateway_ip`, floating IP `port_id`, router
  `external_gateway_info`, most Cinder volume/snapshot fields) take `Some(Cow::from(x))`, or
  `None::<Cow<str>>` to serialise an explicit `null` (used to clear gateways/associations).
- Nova actions with no payload (`os-start`, `pause`, `lock`…) take `serde_json::Value::Null`.
- Image updates are JSON-Patch (`json_patch::Patch`, crate `json-patch = "4"`).
- Unscoped login logs `ERROR … No catalog information`: harmless.
- `AsyncOpenStack` is `Clone` and cheap to store in `moka`.

### OpenStack semantics

- **Admins see every project's Neutron resources.** Filter explicitly with `tenant_id`
  (routers, security groups, floating IPs, subnets, ports). Networks are the project's own,
  plus `shared=true`, plus `router:external=true`, merged with `merge_by_id`. Nova/Cinder
  lists are already project-scoped without `all_tenants`.
- Nova launch takes security groups **by name**; rules reference remote groups **by ID**
  (two choice sets: `SecurityGroups` vs `SecurityGroupIds`).
- Cinder `bootable` is a string `"true"`/`"false"`.
- Quotas use `-1` for unlimited → `None`.
- The production cloud boots VMs from volume: launch defaults to *Create New Volume*.
- Keystone tokens last ~1 h: after that every call redirects to `/login`.
- **Never fan out unbounded.** One call per item (provider, flavor, agent, network) must go
  through `fetch::Bounded` (`FAN_OUT` in flight), not a bare `JoinSet`: Admin › Hypervisors'
  Placement tab fired hundreds of GETs at once and ttcp-dev refused this machine, every endpoint,
  for minutes (roadmap 85). Load per-item details only when they are shown (a tab on open).

### Leptos 0.8

- **`#[server]` generates a struct named after the fn in CamelCase** — it clashes with
  models (`session_info` → `SessionInfo`). Name server fns `get_*`, `list_*`, `run_*`.
- **`Action` clashes with `leptos::prelude::Action`** → our enum is `OsAction`.
- Reading a `Resource` outside `<Suspense>/<Transition>` logs a hydration warning — wrap
  every read, including breadcrumbs and header chips.
- `leptos_axum::redirect` during an SSR render only sets `Location`; also call
  `ResponseOptions::set_status(FOUND)` (see `session::require`), but only for page loads
  (`Accept: text/html`): a server function call given a 302 parses the login page and fails
  with "error deserializing server function results" (roadmap 76). Routes use
  `SsrMode::Async` so redirects and full HTML work.
- Ownership in `view!`: closures inside reactive blocks must be `Fn`. Compute owned values
  *before* the `view!` (`let x = f(&data);`), `.clone()` captured closures per render branch,
  put shared data in `StoredValue`, and never return a view borrowing a function argument
  (clone `String`s out first). Edition 2024 RPIT captures lifetimes: helpers returning
  `impl IntoView` from `&str` args should return `AnyView`.
- Generic `#[component]`s work (`DataTable<T>`), cells are `Arc<dyn Fn(&T, Lang) -> AnyView>`.
- Build a table's `vec![col(…), …]` **outside** `view!` (a `StoredValue`, as `list_page!` does).
  Written inline in nested `view!` closures, type-checking never finished (rustc ran 40+ min and
  filled the disk; `pages/identity.rs`, roadmap 57). If a build hangs, look at the last change.
- `web-sys` is a regular dependency (not optional) because some features
  (`HtmlDetailsElement`) are referenced in code compiled for both targets.
- Menus use native `<details>`; close them via `NodeRef<Details>::set_open(false)`.
- Reading a disposed signal panics, and in WASM a panic kills the whole app (the next page
  stays on "Loading…"). Navigating away disposes a page's signals while its elements still
  get `blur`/`mouseleave`: hover/focus state uses `try_get`/`try_set`
  (`pages/topology.rs`). A JS `.click()` doesn't reproduce it; test with a real hover.
- Signals created in an event handler belong to whatever owner is current there, e.g. a
  `Combo`'s option list, which re-renders and disposes them. Create long-lived ones (rows added
  by a click) under the component's own `Owner` (`member_rows.rs`, roadmap 54).

### Tooling and environment

- **Never run `cargo leptos build --release` while `cargo leptos watch` is serving.** Both
  write `target/site/pkg`; the release WASM can't hydrate debug HTML (hot-reload markers) and
  the page silently stops reacting to clicks. Recovery: stop everything, restart `watch`
  (a `touch` is not enough; the bundle is only rewritten when it changes or on start-up).
- Settings are read at process start (`server/settings.rs`): `moonlight.conf` files, then the
  `MOONLIGHT_*` environment variables (there is no `.env` support); `watch` restarts the
  server on each rebuild. Always start it with the repo-local `moonlight.conf`
  (`-- --config-file moonlight.conf`): the user edits that file.
- **Disk space:** `target/` grows to 20+ GB (two targets, incremental caches), and
  `target/debug/deps` keeps every old build of the crate (65 GB seen). On
  `No space left on device`, delete `target/release` and the `incremental` dirs; if that is not
  enough, `cargo clean -p moonlight` (only this crate's outputs, rebuildable). Never user files;
  then restart `watch` (a failed build keeps the old binary).
- **`tokio-rt-worker has overflowed its stack`** (debug server, roadmap 62): the first request
  after a start builds the connection and renders the page in one poll, and unoptimised futures
  are huge. `main.rs` gives the workers 16 MiB (`WORKER_STACK`); at 1 MiB it crashes on every
  start. If it comes back, raise that, rather than boxing futures one by one.
- **Two `Set-Cookie` headers in a plain axum handler** need `AppendHeaders([...])`: a bare
  `[(SET_COOKIE, a), (SET_COOKIE, b)]` *inserts*, so only the last survives (WebSSO lost its
  session cookie this way, roadmap 80). Server functions use `session::set_cookie` (appends).
- Floating UI inside tables (menus, popovers) must use `position: fixed` computed from the
  trigger's rect: `.table-wrap` has `overflow-x: auto`, which clips absolute children.
- Tests with time: build the fixture **once** (`let original = sample();`) — two
  `Utc::now()` calls differ.

### Browser automation (Claude in Chrome / Brave)

- Work in the session's own tab group; ask the user to move existing tabs into it.
- **Wait for hydration** after `navigate` before clicking (≈2–3 s or check that a click
  changed something); early clicks/JS `.click()` are lost.
- A password manager (1Password) overlays inputs on `127.0.0.1`: typing or focusing a field
  can make the tab inaccessible to automation. Reload, then fill forms **without focusing**:
  set `input.value` and dispatch `new Event('input', {bubbles: true})`
  (`'change'` for `<select>`), then click submit via JS.
- To check pages quickly, read `.count`, `.timing`, `.alert-error` with JavaScript rather
  than screenshots.
- The browser may keep a stale `/pkg/moonlight.css` after a CSS change (a reload doesn't
  always refetch it). Check with `getComputedStyle`; swap the `<link>` `href` to
  `/pkg/moonlight.css?v=<timestamp>` to load the new one.
- Clicking by element *reference* scrolls it into view first; that scroll closes open row
  menus (by design). Inside menus, click items via JavaScript (`button.click()`) instead.
- Verify polling/refresh with `read_network_requests` (`urlPattern: "/api/"`): it must show
  only `GET`s.

### Remote development (Claude on a remote host, Brave on the Mac)

The Claude in Chrome extension only reaches a Claude Code running on the same machine. When
Claude and the dev server run on a remote host (`porygon`, the Rocky VM, over VPN; repo in
`/home/rocky/moonlight-rs`), Claude drives the Mac's Brave with `chrome-devtools-mcp` through an
SSH tunnel instead.

- **Once, on the host:** `sudo dnf install -y nodejs tmux`, then
  `claude mcp add chrome-devtools --scope user -- npx -y chrome-devtools-mcp@latest
  --browser-url=http://127.0.0.1:9222`. The host needs only a `Host` entry in the Mac's
  `~/.ssh/config`; the script passes the tunnels itself.
- **Each session, on the Mac:** `scripts/moonlight-remote.sh` (local and git-ignored: it names
  this setup's host and paths; it may not exist in another checkout) starts Brave with a separate
  profile and its DevTools port (9222), opens the tunnels (host's 9222 → Brave, Mac's
  3000/3001 → dev server and live reload) and attaches to the `moonlight` tmux session. There:
  `scripts/dev.sh start`, then `claude`; `/mcp` must show `chrome-devtools` connected.
- The user signs in in that Brave window (§1.5). Keep only Moonlight in its profile: whoever
  reaches port 9222 controls the browser, so never expose it beyond the tunnel.
- The tools are `mcp__chrome-devtools__*`, not `mcp__claude-in-chrome__*`; the tips above
  (hydration, filling forms without focusing) still apply.
- Over the tunnel the dev WASM (44 MB, `[profile.wasm-dev]`) arrives gzipped, about 4.5 MB
  (`server::compression`).

---

## 7. Conventions

- Format Rust with `cargo fmt`, never by hand. `rustfmt.toml` sets `max_width = 120`; rustfmt
  leaves macro bodies (`view!`, `translations!`) alone, so keep those readable yourself. The
  tree-wide reflow is in `.git-blame-ignore-revs`; run
  `git config blame.ignoreRevsFile .git-blame-ignore-revs` once so `git blame` skips it, and
  add any later formatting-only commit there.
- Tests sit next to the code (`#[cfg(test)] mod tests`); prefer pure functions so logic is
  testable without network. `tokio::test` for async action tests.
- A source file over ~1500 lines gets split along its section banners into a directory module
  whose `mod.rs` re-exports the parts (`pub use x::*`), so callers' paths do not change.
- Display `-` for empty values, fall back to IDs for unnamed resources (like Horizon).
- Every table row that has actions gets an `actions` column (split button: first item as
  button, rest in a menu, delete last, destructive items red with confirmation).
- Every destructive dialog (`Dialog.danger`: deletes, rebuild, detach, release…) lists the
  affected resources as bullets (`Dialog.selection`) and submits only after the user ticks
  "Yes, I understand"; the dialog engine enforces it. Use `Dialog::confirm` or `ops::bulk`
  rather than building that by hand; non-destructive actions use `Dialog::simple` (one click).
- Keep the Horizon structure (groups/panels, breadcrumbs, tab names) so users feel at home;
  improve the controls, not the information architecture.
- Colors: utility `#2b3447`, primary `#0066ff`, secondary `#f7f7ff`; no company logo.
- No hex colours in CSS rules: add a token on `:root` as `light-dark(light, dark)` (dark mode).
  Use `--accent` for blue text/indicators, `--primary` only for fills under white text.
- Server fns return `Result<T, AppError>`. Map SDK errors with `sfe("context")` (classified by
  `server::errors::classify`); use `AppError::invalid` for validation. Never match on strings.
- Write user-facing summaries with numbers measured in the session (calls, seconds), not guesses.

### Documentation: new code is born documented

Every change ships with its documentation, in the same edit; a fix or a feature is not done
until it is. In English, short, and kept true when the code changes.

- **Modules** start with a `//!` header: what lives there and the rule it keeps (e.g.
  "GET only", "pure, no I/O"). A new file gets one before its first item.
- **Public items** (`pub fn`, `pub struct`/`enum`, their fields and variants, `#[server]` fns,
  `#[component]`s, `pub const`) get a `///` saying what it is for and anything the signature
  doesn't: units, `None`/empty meaning, which microversion or extension it needs, what it
  returns on failure. Private helpers get one when the name alone doesn't say it.
  No lint enforces this (`missing_docs` would also demand ~1,850 self-explanatory fields and
  variants), so review it: every function, type and constant has a `///` today.
- **Comments say why, not what.** Write them where the code follows an OpenStack quirk, a
  microversion, a Horizon behaviour, a safety rule (§1) or a gotcha (§6); name the API, version
  or extension so the next reader can check it. Don't cite roadmap items or issue numbers in
  code, comments, rustdoc or test messages: say the reason itself (the commit message links the
  item).
- **Fixes** leave a comment at the fix naming the cause when it isn't obvious from the code
  (e.g. "Nova only returns `type` from 2.2"), and the test that guards it says the same in its
  name or assertion message.
- Don't narrate the obvious (`// increment i`), don't leave commented-out code, and update or
  delete a comment that the change makes false.
- Markdown docs (`README.md`, `CONFIGURATION.md`, `SECURITY.md`) change with the behaviour: a new
  setting is documented in `CONFIGURATION.md` and `moonlight.conf.example` in the same change;
  a change to a guard, a gate or a secret's handling updates `SECURITY.md`.
