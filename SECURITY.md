# Security

How Moonlight protects the people who use it and the clouds it talks to: the threat model,
every measure in the code and where it lives, the limits of those measures, and what an
operator must do at deployment time. [ARCHITECTURE.md](ARCHITECTURE.md) explains the same
mechanisms as part of the whole design, with diagrams; [CONFIGURATION.md](CONFIGURATION.md)
lists every setting named here. Roadmap numbers (`roadmap 48`) refer to
[ROADMAP.md](ROADMAP.md).

- [Reporting a vulnerability](#reporting-a-vulnerability)
- [Threat model](#threat-model)
- [Design principles](#design-principles)
- [Session and cookies](#session-and-cookies)
- [Authentication](#authentication)
- [Browser-facing protections](#browser-facing-protections)
- [Requests to OpenStack](#requests-to-openstack)
- [Authorization](#authorization)
- [Secrets](#secrets)
- [Logging and caches](#logging-and-caches)
- [Deployment checklist](#deployment-checklist)
- [Known limitations and accepted risks](#known-limitations-and-accepted-risks)
- [Review history](#review-history)

## Reporting a vulnerability

Please do not open a public issue for a security problem. Use GitHub's private vulnerability
reporting on the repository (*Security › Report a vulnerability*). Include the version or
commit, the configuration keys involved (never their secret values) and steps to reproduce.
You will get an acknowledgement, and a fix or a documented decision, in this file.

## Threat model

Moonlight is a web dashboard that holds, for the duration of a sign-in, a Keystone token that
can do everything its user can do on the cloud. That token is the asset. Everything below is
about keeping it, and what it can reach, out of other hands.

**What Moonlight trusts.**

- **Keystone and the service policies.** Every OpenStack call is made with the signed-in
  user's own token. Moonlight has no service account and no admin credentials, so Nova,
  Neutron, Cinder, Glance and the rest decide what is allowed, exactly as with the CLI.
  Moonlight's own authorization checks are defence in depth, never the only gate.
- **The host and its configuration.** `moonlight.conf` holds the cookie key. Whoever can read
  it can forge any session; whoever can change it can point Moonlight at another Keystone.
- **The reverse proxy, when there is one.** TLS termination, the client address header and
  connection limits are the proxy's job (see the [deployment checklist](#deployment-checklist)).
- **The identity provider**, for WebSSO deployments: Keystone's Apache does the OIDC or SAML
  dance; Moonlight only receives the resulting token.

**What Moonlight defends against.**

- Attacks on a signed-in user's browser: cross-site request forgery, clickjacking, script
  injection from cloud data, session theft and fixation, secrets leaking through compression
  or caches.
- Attacks through user input on the OpenStack APIs: request paths and query strings shaped
  by IDs, names, filters or markers; oversized or unbounded requests.
- Password guessing against Keystone through the login form.
- Information leaks: internal endpoint URLs in error messages, tokens in logs or on disk.
- Accidental writes to a production cloud, from the UI or from a bug in it.

**Out of scope.** Compromise of the host, the proxy, Keystone or the identity provider; a
browser with a hostile extension; denial of service by a party with legitimate credentials
beyond the bounds noted below.

## Design principles

1. **Stateless server.** Everything needed to serve a request travels in the encrypted
   session cookie. In-process caches only save Keystone round-trips and can be dropped at
   any time, so any replica can serve any request and a restart loses no session.
2. **The user's own token, always.** No service account; every call is rescoped from the
   user's unscoped token to the project they are working in.
3. **Writes are enforced on the server.** One write path, `OsAction` → `actions::apply` →
   `Exec`, obeys `[writes] mode = off | dry-run | on`. The default is `dry-run`: every request
   is built and shown, nothing is sent. The UI reflects the mode; it never is the gate.
4. **Reads are GET only.** `server/fetch.rs` never calls an endpoint with side effects. The
   read-only POSTs OpenStack forces on a client (instance log and console, Heat validate and
   preview) go through `Exec` like writes, so they too obey the mode.
5. **Secrets never persist.** Tokens are never logged or written to disk, the SDK's on-disk
   auth cache is disabled on every connection, cache keys are SHA-256 hashes, `Debug`
   implementations mask tokens and keys, and secret payloads are wiped from memory on drop.
6. **Pure, tested mapping.** JSON from the cloud becomes typed models in functions without
   I/O, unit-tested with API-shaped payloads, so odd or hostile values are handled once.
7. **Capabilities over assumptions, but never for authorization.** What a cloud offers is
   detected to shape the UI; the `admin` role is read from the token itself for every gate.

## Session and cookies

Code: `src/server/session.rs`, `src/server/config.rs`.

| Measure | Detail |
| --- | --- |
| Encrypted, authenticated cookie | The `Session` (unscoped token, user, project, region, timestamps) is AES-256-GCM encrypted by the `cookie` crate's private jar under `[session] cookie_key`. A tampered or foreign cookie decrypts to nothing and is simply "not signed in". |
| Cookie name as associated data | The cookie name is part of the AEAD associated data, so a value issued under one name cannot be replayed under the other. |
| `__Host-` prefix | With `[session] secure_cookies = true` (default) the cookie is `__Host-ml_session`: the browser accepts it only with `Secure`, `Path=/` and no `Domain`, so a sibling subdomain cannot plant or overwrite it (roadmap 49). |
| `HttpOnly`, `SameSite=Lax` | Script cannot read the session; cross-site subresource and `fetch` requests do not carry it. |
| Bounded lifetime | `Max-Age` is the earlier of the token's expiry and `logged_in_at + [session] timeout_minutes` (default 60). `require()` also treats a token within `[session] token_margin_seconds` of expiry as expired, so no request starts with a dying token. A cookie without `logged_in_at` is refused rather than defaulting to "now" (roadmap 63). |
| Key handling | `cookie_key` must decode to at least 64 bytes. Without one a random key is generated at start-up, so sessions end on restart; the log says so. `Debug` of the settings hides it. |
| Size check | Browsers drop cookies over 4096 bytes. The WebSSO callback refuses a session that would not fit, with a logged reason, instead of looping the user back to `/login`. |
| Logout | The token's hash goes into the `revoked` cache (refused on this process at once), the per-token caches are dropped, the cookie is deleted under both names, and with `[session] revoke_on_logout` (default) Keystone is asked to revoke the token through `Exec`. |
| Scope switches | Switching project or region re-encrypts the same token after Keystone confirms the new scope works; the project must be in the user's own list, the region in the catalog. |
| Diagnostics without secrets | `why_not_signed_in` logs which case sent a user to the login page (no cookie, wrong key, logged out, expired), never the token. |

## Authentication

Code: `src/api.rs` (`login`, `logout`), `src/server/os.rs`, `src/server/throttle.rs`,
`src/server/websso.rs`.

### Password sign-in

| Measure | Detail |
| --- | --- |
| Password used once | Sent to Keystone as `v3password` for an unscoped token and dropped (`SecretString`). Only the token is kept. |
| Domain validation | `login_domain` enforces `[login] domain_hidden` and `domain_choices` on the server, not only in the form. |
| Throttle | After `[login] max_failures` (default 5) rejected attempts for a username, Moonlight stops asking Keystone for `lockout_seconds` (default 300) since the last failure. With `[log] client_ip_header` set, the client address is counted too, across usernames, and a success clears the username only. Counters are keyed by SHA-256 hashes and live per process. Only rejected credentials count, not an unreachable Keystone. |
| Form attributes | `password_autocomplete` is off by default; the password field never pre-fills. |

### WebSSO (Keystone federation)

| Measure | Detail |
| --- | --- |
| Keystone does the protocol | Moonlight never sees OIDC or SAML material, only the unscoped token Keystone posts back, which it validates by authenticating with it (`v3token`). |
| Fixed redirect targets | The start URL is built on the server from the settings; the browser's input only picks a configured choice ID. Unknown choices get `400`. |
| Nonce cookie | `__Host-ml_sso`: encrypted, `HttpOnly`, `Secure`, `SameSite=None` (it must come back on Keystone's cross-site POST), ten minutes, one use (`websso_used` cache, atomic insert). The callback refuses a token without a valid, unused nonce. |
| Origin check | The callback accepts `Origin` absent, `null` or exactly Keystone's origin. See [known limitations](#known-limitations-and-accepted-risks) for why this is weak and what roadmap 102 does about it. |
| Single exemption from the CSRF check | `same_origin_only` lets through exactly `POST /auth/websso`, and only with `[websso] enabled`. |
| Logout | A WebSSO session's logout can redirect to `[websso] logout_url` so the identity provider's session ends too. |

## Browser-facing protections

Code: `src/server/mod.rs` (`same_origin_only`, `security_headers`, `compression`),
`src/main.rs`.

| Measure | Detail |
| --- | --- |
| Same-origin check on every non-GET request | Server functions are POSTs. `same_origin` passes `GET`, `HEAD` and `OPTIONS`; anything else needs `Sec-Fetch-Site: same-origin` (or `none`, a URL the user typed), or, from a browser without that header, an `Origin` equal to `[session] public_url` or, unset, to the request's `Host`. No header to tell means `403`. A sibling subdomain (`same-site`) is refused (roadmap 48). |
| Clickjacking | `X-Frame-Options: DENY` and CSP `frame-ancestors 'none'` on every response. |
| CSP | `frame-ancestors 'none'; object-src 'none'; base-uri 'self'`. No `script-src`: Leptos hydration needs inline scripts, so script injection is prevented by escaping, not by policy. |
| Sniffing and referrers | `X-Content-Type-Options: nosniff`; `Referrer-Policy: same-origin`, because URLs carry resource IDs. |
| HSTS | `Strict-Transport-Security: max-age=31536000` when `[session] secure_cookies` is on, i.e. the deployment is HTTPS. |
| BREACH | Only the WASM, JavaScript and CSS bundles are gzipped. Pages and server-function results, which can hold a secret next to text the user typed, are never compressed. |
| No raw HTML from data | Leptos renders text nodes and attributes through the DOM, escaping everything. The single `inner_html` in the tree inserts built-in SVG icon markup, never cloud data. |
| Link targets | External links from configuration open with `rel="noopener noreferrer"`. The console URL Nova returns is framed only when it starts with `http://` or `https://`; a `javascript:` URL is an error. Download links are `data:` URLs built from text. |
| Redirects | Every redirect target is a fixed path or a configured URL; the login page takes no `next` parameter. |
| Request body size | Server functions read the body through axum's default limit of 2 MiB; Heat templates have their own byte cap; image uploads are bounded by their declared `Content-Length`. |
| Static files | Served by `tower-http`'s `ServeDir` from the site root, which rejects path traversal. |

## Requests to OpenStack

Code: `src/model.rs` (`check_id`, `OsAction::check_ids`), `src/server/paging.rs`,
`src/server/fetch.rs`, `src/server/actions.rs`, `src/server/os.rs`.

| Measure | Detail |
| --- | --- |
| IDs in paths | Every ID that ends up in a request path is checked first: not empty, not `.` or `..`, no `/`, `\`, `?`, `#`, `%` or control characters (roadmap 48). `check_ids` walks a serialised `OsAction` and checks every string under a key named `id`, `*_id`, `*_ids` or `kind`, plus key pair and stack names, so new variants are covered without a list to maintain (roadmap 67). Metadata namespaces have a stricter character class. |
| Query strings | Paged lists accept only filter fields and sort keys from a per-list table; a typo is an error, not an unfiltered list. Values are capped at 255 characters with no control characters; markers must be IDs. |
| Project scoping | Neutron shows an admin every project's resources, so Project panels filter explicitly by `project_id`; cross-project filters are refused in a project. |
| Bounded fan-out | Per-item calls run through `Bounded` with at most 6 in flight (roadmap 85). Bulk name lookups take at most 100 IDs; batches hold at most 100 actions and cannot nest. |
| Timeouts | Every server function runs inside a 20 s deadline (`os::REQUEST_TIMEOUT`); version discovery gets 5 s per service. The SDK's HTTP client has none of its own. |
| Typed endpoints | Writes use the SDK's typed request builders, so bodies are serialised, never concatenated. The few raw-path requests (Heat, Barbican, Aodh, FWaaS, metadata definitions) build their paths from checked IDs. |
| Every write tested | Each `OsAction` variant has a dry-run test asserting method, path and exact JSON body; a test fails if a variant lacks one. |
| TLS to the cloud | Certificates are verified against the system store or `[tls] ca_file`. `[tls] verify = false` exists for test clouds only. |
| No disk cache | Every SDK connection is built with `auth_cache: Some(false)` and `.disable_auth_cache(true)`. |

## Authorization

Code: `src/api.rs` (`run_action`, `require_admin`, `plugin_enabled`), `src/model.rs`.

The cloud's policies are the real gate. These checks make Moonlight refuse early, in every
mode including dry-run, so a hidden menu is never the only thing in the way.

| Measure | Detail |
| --- | --- |
| Admin dashboard | `require_admin` reads the `admin` role from the scoped token itself (`token_is_admin`), before any call. Admin lists, admin-only writes (`OsAction::admin_only`) and another project's quotas need it. Capability detection, which fails open for the UI, is never used for this. |
| Plugins | Panels Horizon gets from plugins are opt-in (`[plugins] enabled`). Their server functions and their actions (`OsAction::plugins`) refuse when the plugin is off. |
| Own application credentials | `set_user` overwrites the user ID in application credential actions with the session's, so nobody manages another user's. |
| Domain of new identity objects | `set_domain` puts new projects, users and groups in the domain context or the user's own domain, never Keystone's default. |
| Identity safety | `identity_safety` refuses deleting or disabling oneself or the current project, and removing one's own roles on it (stricter than Horizon). |
| Reserved image properties | Glance's `os_glance_*` and `[images] reserved_properties` cannot be written, whatever the dialog allowed. |
| Subnet allow-list | `[network] allowed_private_subnet_cidr` is enforced on the server. |
| Console and uploads | Instance log and console need `[console] enabled` **and** `[writes] mode = on`. Image file uploads need `[images] upload` set to `proxy` or `direct`, `mode = on`, a valid session, a checked image ID and a size within `max_upload_gb`. |

## Secrets

Code: `src/server/actions.rs` (`mask_secrets`, `wipe_secrets`, `hide_params`),
`src/model.rs` (`Hidden`, `UploadTicket`), `src/server/upload.rs`, `src/server/errors.rs`.

| Measure | Detail |
| --- | --- |
| In memory | Passwords and tokens travel as `SecretString`; request bodies under `adminPass`, `password`, `secret` and `payload` are zeroed when dropped (roadmap 55). |
| In previews | Request previews mask those keys, and hidden Heat parameters (`NoEcho`) are replaced by `********`. Previews are sent to the browser only with `[ui] show_requests = true` (off by default, roadmap 78) and never include headers. |
| In errors | The browser gets the error kind, service and request ID. The SDK's full text, which names internal endpoints, reaches it only with `[ui] show_error_details = true` (off by default); the log keeps it (roadmap 50). |
| In `Debug` output | `Session`, the settings' cookie key, `UploadTicket` and `Hidden` print `********`. |
| Secret payloads | *Show Payload* needs `[key_manager] allow_payload` (off by default), is a POST so no cache keeps it, answers with `Cache-Control: no-store`, and is logged as an event without the value. |
| Upload tickets | With `[images] upload = direct` the browser receives the session's scoped token for one `PUT` to Glance. It travels only in that response and is never logged or stored. The `proxy` mode keeps the token on the server and streams the body without buffering it. |
| Generated credentials | A new application credential's secret and a generated key pair's private key are shown once, in the action's result, and never kept. |
| Downloads | `openrc` and `clouds.yaml` are rendered from Jinja templates whose filters quote shell words and YAML scalars, so a hostile project name cannot execute when the file is sourced. Templates are checked at start-up. |

## Logging and caches

Code: `src/server/logging.rs`, `src/server/config.rs`, `src/server/os.rs` (`hash_key`).

| Measure | Detail |
| --- | --- |
| What is logged | Sign-ins, logouts, writes (user, project, action kind, mode, request count), console and payload reads, refused cross-origin requests, throttled sign-ins, and why a page load was sent to `/login`. With `[log] client_ip_header`, the client address. |
| What is never logged | Tokens, passwords, cookie values, secret payloads, Heat templates, upload tickets. |
| Client address | Taken from the configured header, `[log] trusted_proxies` entries from the right, so entries a client prepended are ignored (roadmap 63). Unset, the log shows `-`. |
| File permissions | A `[log] file` is created with the process umask tightened to `0640`; the directory must already exist. |
| Cache keys | Every `moka` cache is keyed by SHA-256 of its inputs (token, project, region, list name, nonce), so no token, username or address is ever a key. |
| Cache scoping | Name lookups are cached per token, project and region, so one user never sees what another's role revealed; they are cleared after a write and at logout. |
| Replay and revocation caches | `revoked` (24 h, 100k entries) and `websso_used` (10 min, 100k entries) are per process; their limits are discussed below. |

## Dependencies and supply chain

Code: `Cargo.toml`, `Cargo.lock`, `deny.toml`.

| Measure | Detail |
| --- | --- |
| Sources | Every crate comes from crates.io; `Cargo.lock` is committed, so a build resolves the audited set. `deny.toml` refuses any other registry and any git dependency. |
| Advisories | `cargo deny check` fails on a RustSec advisory or a yanked version in either the server or the browser graph (`all-features`). Ignores need a reason in `deny.toml`. |
| Licences | An allow list of permissive licences (Apache-2.0, MIT, BSD, ISC, Unicode-3.0, Zlib, BSL-1.0, CC0, Unlicense, MPL-2.0 for one crate, CDLA-Permissive-2.0 for the bundled root certificates). Anything else fails the check. |
| TLS stack | One: `rustls` with the `webpki` roots; `openssl` and `native-tls` are banned so a transitive dependency cannot add a second stack. |
| Duplicates | Several crates are present in two or three versions (`rand`, `sha2`, `getrandom`, `thiserror`, `syn`, …) because the OpenStack SDK and Leptos pin different lines. The check warns, not fails; they are reviewed when the SDK is bumped. |
| Where it runs | Locally per AGENTS.md §2 and in CI (roadmap 105). |

## Deployment checklist

What the code cannot do for you. Each line names the setting in `CONFIGURATION.md`.

- **Terminate TLS in front of Moonlight** and keep `[session] secure_cookies = true`. The
  binary speaks plain HTTP; the container listens on `0.0.0.0:3000`.
- **Set `[session] cookie_key`** from `openssl rand -base64 64 | tr -d '\n'`, share it across
  replicas, and keep it in a file readable only by the service user (`moonlight.conf.d/`
  with mode `0600`). The cookie is the only authentication state: the key forges any session.
- **Set `[session] public_url`** to the URL users open whenever a proxy rewrites `Host`, and
  always with WebSSO, where it also becomes the `origin` Keystone must trust.
- **Set `[log] client_ip_header` and `trusted_proxies`** to match the proxy chain, so the
  sign-in throttle counts addresses and logs name clients. Without them the throttle is
  per username only. Never set `trusted_proxies = 0` behind a proxy: the client picks the
  address.
- **Give the proxy the limits the server lacks:** header and request timeouts, connection
  limits and a request rate limit. See the known limitations.
- **Keep `[writes] mode = dry-run`** until the cloud is one you mean to change, and prefer
  `off` for a view-only deployment.
- **Leave the sensitive switches off** unless needed: `[ui] show_error_details`,
  `[ui] show_requests`, `[key_manager] allow_payload`, `[images] allow_location`,
  `[images] upload`, `[console] enabled`, `[tls] verify = false`.
- **Run the container as shipped:** a non-root user, the configuration mounted read-only, only
  the log directory writable.
- **Audit dependencies** with `cargo audit` or `cargo deny` before a release; the repository
  has no CI doing it yet.

## Known limitations and accepted risks

From the security review of 2026-11-11. Each entry says what an attacker can do, what bounds
it, and the decision taken.

| # | Limitation | Decision |
| --- | --- | --- |
| 1 | **WebSSO login CSRF.** The callback's origin check accepts `null`, which any hostile page can send, and the nonce can be planted in a victim's browser through a cross-site `GET` to the start route. An attacker can sign a victim into the attacker's account. | **Fix planned: roadmap 102.** The start route becomes a same-origin POST so the nonce cannot be planted; an optional strict origin check follows. A race against a user who is at the identity provider at that moment remains possible by design. |
| 2 | **Logout revokes the token only with `[writes] mode = on`.** In the default `dry-run`, a copied cookie stops working on the replica that served the logout, but stays valid on other replicas and after a restart until the token expires. | Accepted. Session lifetime is bounded by `timeout_minutes`; a single-replica deployment is covered until restart. |
| 3 | **Username lockout.** Anyone who knows a username can keep that user out for `lockout_seconds` at a time, unauthenticated, because the throttle blocks per username. | Accepted, as Keystone's own `lockout_failure_attempts` does. Operators who want a softer policy can raise `max_failures` or set it to `0` and rely on Keystone. |
| 4 | **No connection or header timeouts in the server.** Slow clients can hold worker tasks; authenticated users can hold half-open uploads. OpenStack calls are bounded, client connections are not. | Accepted; the reverse proxy provides them (see the checklist). |
| 5 | **Cookie key readable by other local users** when `moonlight.conf` is left world-readable. | Operational; the checklist says `0600`. |
| 6 | **Replay caches can be evicted.** `revoked` and `websso_used` are bounded in size; a flood of entries pushes older ones out, reviving a revoked cookie on that replica or allowing a nonce to be reused within ten minutes. | Accepted; it needs ~100k entries per process within the window. |
| 7 | **Unauthenticated Keystone amplification.** Each login attempt with a new username, and each WebSSO callback with a fresh nonce, costs one Keystone call. Without a client address header nothing limits the rate. | Accepted; rate-limit at the proxy. |
| 8 | **Direct image upload hands the browser a full scoped token.** Any script injection would obtain a token valid for everything the user can do. | Accepted, as Horizon's direct mode; `[images] upload` is off by default and `proxy` keeps the token on the server. |
| 9 | **No `Cache-Control: no-store` on most server-function responses.** Session info and lists are plain GET answers a browser or careless proxy may store. Only secret payloads set it. | Accepted for now; nothing in those answers is a credential. |
| 10 | **CSP has no `script-src`.** Hydration needs inline scripts, so escaping is the only defence against script injection. | Accepted; there is one `inner_html`, with static markup, and nothing renders cloud data as HTML. |
| 11 | **Server-side request forgery at the cloud, not at Moonlight.** `[images] allow_location` and Heat `template_url` make Glance or Heat fetch a user-typed URL, link-local addresses included. Moonlight only checks the scheme. | Accepted, as Horizon; both are off or inert unless the operator enables them. |
| 12 | **`Sec-Fetch-Site: none` passes the write check.** It is what a browser sends for a URL the user typed; a user-initiated cross-site POST from some clients is treated the same. | Accepted; low value to an attacker. |
| 13 | **A session is never re-keyed.** Switching project or region re-encrypts the same token, so an earlier copy of the cookie stays a valid session for the token's lifetime. | Accepted; inherent to the stateless design. |
| 14 | **No dependency audit in CI.** Nothing warns when a crate in `Cargo.lock` gets an advisory. | Operational; the checklist asks for `cargo audit` before a release. |

## Review history

| Date | Scope | Outcome |
| --- | --- | --- |
| 2026-11-11 | Whole code base: session, authentication, CSRF, headers, request building, authorization, secrets, logging, uploads, WebSSO, configuration, container | Five findings and nine smaller points, listed above. One fix planned (roadmap 102); the rest accepted or operational. |
