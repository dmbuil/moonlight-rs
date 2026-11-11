# Moonlight roadmap

Next steps after the PoC (read-only Project dashboard + dry-run writes). Each item has a
goal and a **brief implementation note** so the work can be resumed in a fresh session.
Read `AGENTS.md` first (safety rules, recipes, gotchas).

## Ground rules for every item

- Writes only through `OsAction` → `server::actions::apply` (server-enforced
  `off` / `dry-run` / `on`). Every new action gets a dry-run test asserting the exact JSON.
- Verify against production **in dry-run only**; real writes only against a test cloud.
- New UI strings go in `src/i18n.rs` (EN + ES), new resources are linked everywhere they appear.

| #   | Item                                                                                             | Priority | Size | Depends on                  |
| --- | ------------------------------------------------------------------------------------------------ | -------- | ---- | --------------------------- |
| 1   | ✅ Image name for volume-booted instances                                                         | —        | S    | —                           |
| 2   | ✅ Multi-step modal (wizard) for complex creation                                                 | —        | M    | 4 (nice to have)            |
| 3   | ✅ `moonlight.conf` settings, oslo.config-style layering                                          | —        | M    | —                           |
| 4   | ✅ Custom dropdown component                                                                      | —        | M    | —                           |
| 5   | ✅ Horizon feature parity: missing panels (LB, Trunks, QoS, Volume Groups, Backups)                 | —        | L    | 19, 2, 10                   |
| 6   | ✅ Availability-zone pills + lock icon in instances                                               | —        | S    | microversion header (see 1) |
| 7   | ✅ Key pair generation                                                                            | —        | S    | —                           |
| 8   | ✅ Image owner → project name                                                                     | —        | S    | —                           |
| 9   | ✅ Flavor details popover on hover                                                                | —        | S    | —                           |
| 10  | ✅ Bulk "Delete xxxx" buttons per section                                                         | —        | M    | —                           |
| 11  | ✅ Indeterminate progress bar for instances with an ongoing task                                  | —        | S    | —                           |
| 12  | ✅ Hide flavor extra specs (popover and instance detail)                                          | —        | XS   | —                           |
| 13  | ✅ 🐞 FIX: row action menus clipped by the table                                                   | —        | S    | —                           |
| 14  | ✅ Friendly error page with a "back to safety" button                                             | —        | M    | —                           |
| 15  | ✅ AZ pills: outline only, no fill                                                                | —        | XS   | —                           |
| 16  | ✅ Copy-to-clipboard on key fields of detail views                                                | —        | S    | —                           |
| 17  | ✅ Instances list: move the busy bar to the *Task* column                                         | —        | XS   | 11                          |
| 18  | ✅ Dark mode                                                                                      | —        | M    | —                           |
| 19  | ✅ Modules: show panels by service/extension availability                                         | —        | M    | 14                          |
| 20  | ✅ Network Topology (Topology + Graph tabs)                                                       | —        | M    | 18 (colours)                |
| 21  | ✅ Instance actions parity (resize, rebuild, shelve, interfaces, …)                               | —        | L    | 10 (bulk)                   |
| 22  | ✅ Volume actions parity (retype, upload to image, transfer, tabs)                                | —        | M    | 19 (groups)                 |
| 23  | ✅ Overview usage summary (date range + CSV)                                                      | —        | S    | —                           |
| 24  | ✅ API Access: credentials, `openrc` and `clouds.yaml` downloads                                  | —        | S    | —                           |
| 25  | ✅ Metadata editor (instances, images, volumes, snapshots) ([#1]) | —        | M    | —                           |
| 26  | ✅ Router static routes: add and delete                                                           | —        | S    | 19 (extension)              |
| 27  | ✅ Small parity gaps: large projects (columns and links ✅)                                        | —        | S    | 3, 19                       |
| 28  | ✅ Modal sizes fitted to their content (S / M / L)                                                | —        | S    | —                           |
| 29  | ✅ Horizon `local_settings.py` parity in `moonlight.conf`                                         | —        | M    | 3                           |
| 30  | ✅ *Enable SNAT* checkbox in *Edit Router*                                                        | —        | S    | 19 (extension)              |
| 31  | ✅ Instance *Log* and *Console* tabs                                                              | —        | M    | 19, 21                      |
| 32  | ✅ Flavor popover: drop the *Ephemeral Disk* and *Swap* rows                                      | —        | XS   | 9                           |
| 33  | ✅ *Edit Image* with Horizon's fields                                                             | —        | S    | — (metadata: 25)            |
| 34  | ✅ Hide actions on resources the project doesn't own (external networks, `default` group)         | —        | S    | —                           |
| 35  | Policy-aware actions: the cloud's custom roles (`power_user`)                                    | P2       | M    | 30                          |
| 36  | ✅ Usage Summary lists deleted instances                                                          | —        | XS   | 23                          |
| 37  | ✅ Instance detail: rules per security group, *Image ID*                                          | —        | XS   | —                           |
| 38  | ✅ Small gaps from the Horizon review (October 2026)                                              | —        | S    | —                           |
| 39  | ✅ Language and Theme inside the user menu                                                        | —        | S    | 18                          |
| 40  | ✅ *Any Availability Zone* instead of *— None —* in zone selectors                                | —        | XS   | —                           |
| 41  | ✅ Launch: load the customization script from a file                                              | —        | S    | 2                           |
| 42  | ✅ Instance menu: *Edit Security Groups* (all the instance's ports)                               | —        | S    | —                           |
| 43  | ✅ Security group pickers with a filter (*Allocated / Available*) when editing                    | —        | S    | 42                          |
| 44  | ✅ Instance *Interfaces* tab: *Edit Security Groups* and *Edit Port*, as Horizon                  | —        | M    | 43                          |
| 45  | 🚧 *Create Image* (from a file or a URL) ([#2])                                                            | P2       | L    | 33, 41                      |
| 46  | ✅ Refresh volumes and snapshots while they change state                                          | —        | S    | 11                          |
| 47  | ✅ 🐞 FIX: long row menus (instances) run off the screen                                           | —        | XS   | 13                          |
| 48  | ✅ 🔒 Request boundary: same-origin check and safe IDs in API paths                                | —        | M    | —                           |
| 49  | ✅ 🔒 Session and response hardening (secure cookies, headers, logout)                             | —        | S    | 29                          |
| 50  | ✅ 🔒 Sign-in throttling and error details kept on the server                                      | —        | S    | 14                          |
| 51  | ✅ Key pairs: show the *Type*, and a *Key Type* selector (SSH / X509)                             | —        | S    | 7                           |
| 52  | ✅ Server group detail page (name links to it; members listed)                                    | —        | S    | —                           |
| 53  | ✅ 🐞 FIX: *Create Load Balancer* fails without a pool name; *— None —* listed twice               | —        | XS   | 5                           |
| 54  | ✅ Load balancers to octavia-dashboard parity: members, options, TLS, L7, zones                   | —        | L    | 5, 53                       |
| 55  | ✅ Key Manager (Barbican): Secrets and Containers panels                                          | —        | M    | 19                          |
| 56  | ✅ Images list: no *Owner* column; a row drawer with the image's metadata                         | —        | S    | 25                          |
| 57  | ✅ Identity dashboard (Projects, Users, Groups, Roles, Application Credentials, Domains)          | —        | L    | 14, 19                      |
| 58  | Key Manager: *Orders* (Barbican generates a key or an RSA key pair) ([#3])                              | P3       | S    | 55                          |
| 59  | Key Manager: the project's preferred secret store (multiple-store Barbican) ([#4])                      | P3       | S    | 55                          |
| 60  | ✅ 🐞 FIX: *Usage Summary* shows `-0.00` for zero consumption                                      | —        | XS   | 23                          |
| 61  | ✅ Security group *Add Rule*: help text under every field                                         | —        | XS   | —                           |
| 62  | ✅ Orchestration (Heat): Stacks, Resource Types, Template Versions                                | —        | L    | 19                          |
| 63  | ✅ 🔒 Trusted client address, atomic sign-in counter, redacted session `Debug`                     | —        | S    | 50                          |
| 64  | ✅ Neutron name lookups filtered by project                                                       | —        | S    | —                           |
| 65  | ✅ `api.rs` server-function helper, automatic call counts, review cleanups                        | —        | M    | —                           |
| 66  | ✅ Short per-project cache for name lookups (flavors, images, networks, volumes)                  | —        | M    | 64                          |
| 67  | ✅ 🔒 Test that every `OsAction` path parameter goes through `check_id`                            | —        | S    | 48                          |
| 68  | ⏸ DNS (Designate): zones, record sets, reverse DNS of floating IPs ([#5])                                 | P3       | M    | 5, 19                       |
| 69  | ⏸ Object Store (Swift): containers and objects ([#6])                                                     | P3       | L    | 5, 19                       |
| 70  | ⏸ Shares (Manila): shares, snapshots, share networks ([#7])                                               | P3       | L    | 5, 19                       |
| 71  | ⏸ Container Infra (Magnum): clusters and cluster templates ([#8])                                         | P3       | M    | 5, 19                       |
| 72  | ✅ Instances list: the *Age* column wraps onto two lines so the table fits (Spanish)              | —        | XS   | —                           |
| 73  | ✅ Networks list: no selection checkbox on networks the project can't change (external, shared)   | —        | XS   | —                           |
| 74  | ✅ Network Topology: tell external and shared networks apart from the project's own               | —        | S    | 20                          |
| 75  | ✅ Key Manager: ACL management for secrets (and containers)                                       | —        | M    | 55, 57 (user picker)        |
| 76  | ✅ Expired session: server function calls get their error, dialogs say so                         | —        | XS   | —                           |
| 77  | Keep the session alive while the user is active (`[session] idle_minutes`)                       | P2       | M    | 49, 76                      |
| 78  | ✅ Result dialog: requests without numbers, and `[ui] show_requests`                              | —        | S    | —                           |
| 79  | ⏸ Alarming (Aodh): alarms list, detail, history, create/edit/delete ([#9])                              | P3       | L    | 62                          |
| 80  | ✅ 🔒 WebSSO sign-in (OIDC/SAML via Keystone)                                                      | —        | L    | 48, 49, 57                  |
| 81  | ✅ Write the server log to a file (`[log] file`, rotation)                                        | —        | S    | —                           |
| 82  | ✅ Project › Quotas: every quota of the project, with usage, editable with permission             | —        | M    | 57                          |
| 83  | ✅ WebSSO sessions are slower (≈0.5 s → 1.2 s): measure where the time goes, fix Moonlight's part | —        | S    | 80                          |
| 84  | ✅ Server log: with `keep = 1`, every restart deletes the current log file                        | —        | S    | 81                          |
| 85  | ✅ Admin dashboard (Horizon's *Admin*: Compute, Volume, Network, System)                          | —        | L    | 19, 57, 82                  |
| 86  | ✅ Usage Summary: localised thousands separators                                                  | —        | XS   | 23, 60                      |
| 87  | ✅ Long bulk actions run in the background, with a progress notification                         | —        | M    | —                           |
| 88  | ✅ Server-side paging and filters for the big Admin lists (Instances, Volumes, Floating IPs…) ([#10])     | —        | L    | 85                          |
| 89  | ✅ Admin dashboard refinements (item 85's leftovers) ([#11])                                                 | —        | M    | 85, 88                      |
| 90  | Volume Backups leftovers (from item 5) ([#12])                                                            | P3       | M    | 5                           |
| 91  | Scheduler Hints step in Launch Instance (from item 5) ([#13])                                            | P3       | S    | 5, 2                        |
| 92  | ✅ Heat Template Generator (from item 62) ([#14])                                                        | —        | L    | 62                          |
| 93  | 🚧 Firewall (FWaaS v2): firewall groups, policies and rules ([#15]) | P2 | L | 19 |
| 94 | Admin › Network › Firewall: every project's firewall groups, policies and rules ([#16]) | P3 | M | 93, 85 |
| 95 | *Create Image* › *File* (`direct`): a token that can only upload, not the session's ([#17]) | P3 | M | 45, 57 |
| 96 | ✅ Tab bars show a pointless scroll control | — | XS | — |
| 97 | Inline documentation: doc comments on every module and function, rustdoc clean | P3 | M | — |
| 98 | ✅ Subtle CSS motion on tabs, nav bars, menus and dialogs | — | S | 96 |
| 99 | ✅ 🐞 FIX: the VNC console doesn't fit its frame; the last lines are cut off | — | S | 31 |
| 100 | Network Topology: a note instead of the drawing when the project sees too many networks | P3 | S | 20, 29 |
| 101 | Phone view: fold the header's bars and menus into one or two buttons | P3 | M | 39, 96 |

[#1]: https://github.com/dmbuil/moonlight-rs/issues/1
[#2]: https://github.com/dmbuil/moonlight-rs/issues/2
[#3]: https://github.com/dmbuil/moonlight-rs/issues/3
[#4]: https://github.com/dmbuil/moonlight-rs/issues/4
[#5]: https://github.com/dmbuil/moonlight-rs/issues/5
[#6]: https://github.com/dmbuil/moonlight-rs/issues/6
[#7]: https://github.com/dmbuil/moonlight-rs/issues/7
[#8]: https://github.com/dmbuil/moonlight-rs/issues/8
[#9]: https://github.com/dmbuil/moonlight-rs/issues/9
[#10]: https://github.com/dmbuil/moonlight-rs/issues/10
[#11]: https://github.com/dmbuil/moonlight-rs/issues/11
[#12]: https://github.com/dmbuil/moonlight-rs/issues/12
[#13]: https://github.com/dmbuil/moonlight-rs/issues/13
[#14]: https://github.com/dmbuil/moonlight-rs/issues/14
[#15]: https://github.com/dmbuil/moonlight-rs/issues/15
[#16]: https://github.com/dmbuil/moonlight-rs/issues/16
[#17]: https://github.com/dmbuil/moonlight-rs/issues/17

Done: **1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
26, 27, 28, 29, 30, 31, 32, 33, 34, 36, 37, 38, 39, 40, 41, 42, 43, 44, 46, 47, 48, 49, 50, 51, 52,
53, 55, 54, 56, 57, 60, 61, 62, 63, 64, 65, 66, 67, 72, 73, 74, 75, 76, 78, 80, 81, 82, 83, 84,
85, 86, 87, 88, 89, 92, 96, 98, 99**.
Next: **35**.

⏸ = postponed: it cannot be done for now (see the item's note); it is not in `Next:`.

Priority: **P1** = used daily by operators, or blocks safe use; **P2** = Horizon parity users
will miss; **P3** = polish or rarely used. Items 21 to 27 come from a read-only review of the
production Horizon *Project* dashboard (all 22 panels, plus instance, volume, network and router
details).
(13 and 4 share the same floating-layer technique; do 13 first and reuse it in 4.)

---

## 1. Image name on the instance view for volume-booted instances

> ✅ **Done.** Nova reads use `compute 2.47` (`fetch::COMPUTE_MICROVERSION`); instances list joins
> Cinder volumes (4 calls). Boot volume = `root_device_name` match → `/dev/vda|sda|xvda|hda` → any
> bootable; name from `volume_image_metadata`. UI tag "from volume". Tests:
> `boot_from_volume_takes_the_image_from_the_boot_volume`.

**Goal.** Today the *Image Name* column shows "- (booted from volume)". Show the image the
boot volume was created from, like Horizon does.

**Implementation.**

- Nova returns `"image": ""` for boot-from-volume. The image is in Cinder:
  `volume.volume_image_metadata.image_name` (fallback `image_id` → Glance name).
- `fetch::instances`: add `volumes_raw(os)` to the existing `tokio::join!` (one extra call,
  not N+1). Find each server's **boot volume**: the attachment of
  `os-extended-volumes:volumes_attached` whose Cinder attachment `device` is the root device
  (`/dev/vda` / `/dev/sda`), or with microversion ≥ 2.3 `OS-EXT-SRV-ATTR:root_device_name`.
- Ask for compute microversion **2.47** on list/get (builder `.header("OpenStack-API-Version",
  "compute 2.47")`, available on every SDK `Request` builder). That also gives
  `locked` (≥ 2.9, needed by item 6), `root_device_name` and the embedded flavor (item 9;
  `mapping::flavor_of` already reads `original_name`).
- `mapping::instances(servers, flavors, images, volumes)`: new `Instance.image_source:
  Option<ImageRef { name, via_volume: bool }>`; UI shows the name plus a small "volume" tag.
- Tests: fixture with `"image": ""` + volume carrying `volume_image_metadata`.

## 2. Multi-step modal for complex creation (Instances, Networks)

> ✅ **Done** (checked in dry-run against Horizon, 2026-10-24). `Dialog::wizard(steps)` (step list
> with `*` / ✓ / !, Back/Next, submit enabled only when valid, steps that switch off),
> `Field::pick` / `pick_many` (*Allocated / Available* tables, `components/pick.rs`),
> `Field::yes_no`, `show_if` conditions, and `on_change` / `check` / `block` hooks. Launch
> Instance follows the production Horizon wizard (read 2026-10-24): 9 steps (Details, Source,
> Flavor, Networks, Network Ports, Security Groups, Key Pair, Configuration, Server Groups); boot
> sources Image / Instance Snapshot / Volume / Volume Snapshot; *Create New Volume* Yes by
> default; volume size raised to `ceil(max(size/1e9, min_disk))` when an image is allocated, never
> lowered, "must be at least N GB"; flavors too small for the image's `min_ram` (or its disk, when
> booting without a volume) greyed out with the reason; networks or ports required; customization
> script ≤ 16 KB, disk partition, config drive, description (compute 2.19). Create Network:
> Network → Subnet → Subnet Details. Launch images are queried like Horizon (public, community,
> shared-accepted, own: 24 here, without other projects' private images); Horizon listed 29, find
> the 5 extra once signed in to Horizon. `run_action` now sends JSON: the URL encoding dropped
> empty lists and broke e.g. a subnet without DNS servers. **Left out**: Scheduler Hints (item 91) and
> Metadata steps (item 25), *Create/Import Key Pair* buttons inside the Key Pair step, the
> quota-impact panel of the Flavor step, IPv6 address mode, host routes, MTU and AZ hints.

**Goal.** Horizon-like wizards: Launch Instance (Details · Source · Flavor · Networks ·
Security Groups · Key Pair · Server Groups) and Create Network (Network · Subnet · Subnet
Details), with a step sidebar, Back/Next and per-step validation.

**Implementation.**

- Extend `components::dialog::Dialog` with `steps: Vec<Step { title: String, fields:
  Vec<Field>, optional: bool }>`. Keep `fields` for single-page dialogs (a one-step wizard).
- `DialogView`: `current_step: RwSignal<usize>`; left column lists steps with ✓ / ! state;
  validation of `required && visible` runs per step on *Next* and for all steps on submit.
  The single `values` map and `build` closure stay as they are, so `ops.rs` and `OsAction`
  are untouched.
- Rich step bodies: allow `Kind::Table(Source)` — a selectable table (as Horizon's
  "Available / Allocated" lists) for flavors (with vCPU/RAM/disk columns and quota bars),
  images and networks; and a *summary* step rendering the dry-run preview before submitting.
- Port `ops::launch_dialog` and `ops::create_network_dialog` to steps; keep a test that
  builds the `OsAction` from the same `Values`.

## 3. `moonlight.conf` settings (OpenStack-style)

> ✅ **Done.** TOML with oslo semantics (`server/settings.rs`): sections `[keystone]`,
> `[session]`, `[writes]`, `[ui]` (the user-menu links of item 27), `[log]`;
> `deny_unknown_fields`, each file checked on its own so errors name it. `clap` takes repeatable
> `--config-file` / `--config-dir`; without them the **first** default file and `.conf.d`
> found are used (oslo's `_search_dirs`). Files, then directories sorted by name, then the
> `MOONLIGHT_*` variables (and `RUST_LOG`), deep-merged as `toml::Table`. `.env` support
> (`dotenvy`) was removed afterwards; the files read are logged, the cookie key never is (redacted
> `Debug`). `moonlight.conf.example`; tests for search order, alphabetical order, partial
> override, env precedence, unknown keys and invalid `[writes] mode`. Not done: `idle_minutes`
> and `[ui] default_language` (nothing uses them yet), and oslo's interleaving of files and
> directories in command-line order (files always come first).

**Goal.** Replace `.env` with config files like Nova/Neutron, layered so later files win.

**How OpenStack does it** (oslo.config): INI files; `--config-file` may be repeated and
`--config-dir` reads every `*.conf` in the directory **in alphabetical order**, after the
individual files; *later values always override earlier ones*. Defaults when nothing is
given (`find_config_files` / `find_config_dirs`): `~/.moonlight/moonlight.conf`,
`~/moonlight.conf`, `/etc/moonlight/moonlight.conf`, `/etc/moonlight.conf` and the matching
`moonlight.conf.d/` directories.

> Note: oslo.config files are **INI**, not TOML. Decision to take when starting: (a) TOML
> syntax with oslo semantics (sections map 1:1: `[DEFAULT]`, `[keystone]`, `[session]`,
> `[writes]`), parsed with the `toml` crate; or (b) INI for operator familiarity (`rust-ini`
> or `configparser`). The loader below is the same either way. Recommendation: TOML, as
> requested, with the oslo-style section layout.

**Implementation.**

- New `src/server/settings.rs`: `#[derive(Deserialize)] struct Settings { keystone: {
  url, default_domain, default_region }, session: { cookie_key, secure_cookies, idle_minutes },
  writes: { mode }, ui: { default_language }, log: { filter } }` with defaults.
- Resolution order (lowest → highest): built-in defaults → default paths above (in order) →
  each `--config-file` (in CLI order) → each `--config-dir/*.conf` sorted alphabetically →
  `MOONLIGHT_*` environment variables (for containers/secrets) → CLI flags.
- Merge **at the `toml::Value` level** (deep merge tables, scalars replaced) before
  deserialising, so a later file can override a single key.
- CLI via `clap` (`--config-file` repeatable, `--config-dir` repeatable). Keep `.env`
  support for one release with a deprecation warning.
- Log the files loaded (in order) at start-up; never log `cookie_key`.
- Ship `moonlight.conf.example`; tests for ordering (`10-base.conf` < `20-site.conf`),
  partial override, env precedence and invalid `writes.mode`.

## 4. Custom dropdown (no native `<select>`)

> ✅ **Done.** `components/select.rs::Combo` replaces every
> `Kind::Select`: combobox + listbox ARIA, search box from 8 options, ↑/↓/Home/End/Enter/Esc,
> type-ahead (also closed, like a native select), `position: fixed` popup that flips up,
> `data-value` on options. Esc closes the list, not the dialog. Header `<details>` menus
> still use their own styling.

**Goal.** A dropdown that matches the theme (header menus, dialogs, filters), searchable
for long lists (images, networks, flavors).

**Implementation.**

- `components/select.rs`: `Select { options: Signal<Vec<Choice>>, value: RwSignal<String>,
  placeholder, searchable, disabled }` and `MultiSelect` (chips).
- Follow the WAI-ARIA *combobox + listbox* pattern: `role="combobox"`, `aria-expanded`,
  `aria-activedescendant`; keyboard ↑/↓, Home/End, Enter, Esc, type-ahead; close on
  outside click (document listener registered on open, removed on close).
- Popover positioned under the trigger (flip upwards near the viewport bottom); render
  inside the modal so it scrolls with it.
- Replace `Kind::Select`/`Kind::Multi` rendering in `dialog.rs`, and reuse the same
  styling for the header `<details>` menus.
- Note from AGENTS.md: avoid focusing inputs during browser automation (1Password overlay);
  expose `data-value` on options so tests can pick by value.

## 5. Horizon feature parity: missing panels

> ✅ **Done.** Five slices; the leftovers moved to items 90 (Backups), 91 (Scheduler Hints) and 92
> (Template Generator). Slice 1, *Load Balancers* (read-only + delete): SDK feature
> `load_balancer`; Octavia discovered with the other services but not required to cache a
> connection. Panel under Network, gated by `Requires::Service("load-balancer")`; list
> filtered by `project_id` (admins see every project's). Detail tabs Overview (VIP with its
> floating IP, port, network, subnet) · Listeners · Pools · Members · Health Monitors, in two
> rounds of parallel GETs (members per pool via `JoinSet`); pure `mapping::load_balancer_detail`
> tested with Octavia-shaped JSON. `OsAction::LoadBalancerDelete { cascade }` (row menu, header,
> bulk; always cascades like octavia-dashboard), dry-run test. Request previews now show the
> query string (`?cascade=true`). Verified on production (dry-run, GET only): the panel lists
> (1 call, 0.48 s), but the current project has no load balancer, so the detail page is still
> unseen with real data.
>
> Slice 2, *Load Balancer writes*: the *Create Load Balancer* wizard (details, listener, pool,
> members, health monitor) sends one fully populated `POST /lbaas/loadbalancers`, so Octavia
> creates all or nothing; each step after the details can be switched off, the pool protocol
> follows the listener's, members are instance addresses (`ip@subnet`, one port and weight for
> all). Header menu: edit, associate / disassociate the VIP's floating IP, delete. Each tab
> has its create button and row actions (edit, delete) for listeners, pools, members and health
> monitors; validation in `model` (`Lb*Spec::check`, Octavia's rules). Listener and pool updates
> are JSON requests (they must send `null` to clear the default pool or the persistence).
> Dry-run tests assert every body. The rest (`TERMINATED_HTTPS` with Barbican, L7 policies,
> per-member ports, external members, VIPs on shared networks' subnets) moved to item 54. Next:
> Stacks.
>
> Slice 3, *Network QoS* and *Trunks*, both opt-in plugins (`qos`, `trunk`) that also need their
> Neutron extension. QoS: policies (own plus shared; shared ones of other projects read-only)
> with bandwidth-limit, DSCP-marking, minimum-bandwidth and minimum-packet-rate rules
> (`QosRuleSpec::body` keeps only each type's fields; valid DSCP marks and directions checked),
> and a *QoS Policy* field in *Create Router* (gateway, `qos-gateway-ip`) and *Allocate IP*
> (`qos-fip`). Trunks: list and detail, *Create Trunk* (parent port plus an optional first
> sub-port), edit, delete, add and remove sub-ports (`vlan` 1-4094 or `inherit`). The SDK has no
> trunk endpoints, so reads and writes go through `actions::JsonRequest` (now with query
> parameters; GET only in `fetch.rs`). Dry-run tests assert every body. Not yet: the QoS
> policy of networks and ports, and Horizon's multi-row sub-port step in *Create Trunk*.
>
> Slice 4, *Volume Groups* and *Group Snapshots* (Cinder generic volume groups), gated by
> `needs::VOLUME_GROUPS` (volume 3.27; core Cinder, not a plugin). Read: both lists (Horizon's
> `volume_groups` and `vg_snapshots` URLs), and detail pages with the group's volumes (volume
> 3.13 for `group_id`) and snapshots, or the snapshot's volume snapshots (3.14), in
> `pages/volume_groups.rs`. Writes, all typed SDK endpoints except the delete (`JsonRequest`:
> the SDK's `delete_313` has no ID in the path): *Create Group* (group type, zone, volume types
> by ID; `ChoiceSet::{GroupTypes, VolumeTypeIds}`), per row *Create Snapshot*, *Edit Group*,
> *Clone Group* (`create-from-src`), *Delete Group* (optional *Delete Volumes*); on the detail
> page also *Manage Volumes* (current volumes preselected, the save sends the difference as
> `add_volumes` / `remove_volumes`; `ChoiceSet::GroupVolumes` lists, per group, its volumes and
> the ones it can take) and *Remove Volumes from Group*; *Create Group* from a group snapshot.
> Dry-run tests assert every body (`actions::volume_group_writes`), dialog tests in `ops`. Not
> verified in the browser with real groups: on ttcp-dev the project has none; the
> lists load (4 calls · 0.26 s and 3 · 0.13 s) and *Create Group* lists the cloud's group types,
> zones and volume types. *Manage Volumes* is not offered on the list rows
> (the row doesn't know the volumes; Cinder's `list_volume` isn't in the SDK's list builder).
> Nothing is left on the item: Group Types (admin) is done in item 85, the *Scheduler Hints*
> launch step is item 91 and the Heat Template Generator item 92. DNS, Object Store, Shares and
> Container Infra moved to items 68 to 71.
>
> Slice 5, *Volume Backups*, the opt-in `backup` plugin (`Plugin::Backup`, `needs::BACKUP` =
> plugin plus the `block-storage` service). The catalog can't say whether `cinder-backup` runs
> (its `/os-services` is admin only), so the plugin is the real gate: it is off unless
> `[plugins] enabled` lists `backup`, which hides the panel and the volume menu entry and makes
> the server functions and `run_action` refuse. The repo's `moonlight.conf` (ttcp-dev, no
> backup service) does not list it. Read: Volumes › Backups (`/project/backups`, columns incl.
> volume, incremental) and detail (`BackupDetailPage`: failure reason, source snapshot,
> container), both polling while `creating` / `restoring` / `deleting`. Writes, typed SDK
> endpoints: *Create Volume Backup* (list header, volume picked; or the volume's menu, with
> *Force* preselected when attached; incremental), *Restore Backup* (new volume by name, or over
> a chosen volume), *Edit Backup* (volume 3.9), *Delete* (row and bulk). Dry-run tests assert
> every body (`actions::volume_backups_*`), plus mapper, dialog and nav gating tests. 501 tests
> pass, both clippy variants clean. Not verified in a browser (no backup-enabled cloud). The
> rest moved to item 90.
>
> **Plugins are opt-in** (`[plugins] enabled`, env `MOONLIGHT_PLUGINS`; none when unset):
> `model::Plugin` (`load-balancer`, `orchestration`), `Requires::Plugin` (checked even when
> detection failed), menu entries need the plugin *and* the service, and the server functions
> and `run_action` refuse a disabled plugin's calls. Unknown names stop start-up. Every new
> plugin panel (DNS, Object Store, Shares…) gets a `Plugin` variant.

**Goal.** Reach feature parity with Horizon's Project dashboard, including the panels that
Horizon plugins add when their service exists. Every panel is a module gated by item 19,
so a cloud without the service simply does not show it.

| Group           | Panel                                                         | Requires (item 19)               | SDK module (gtema 0.22)                                                                                |
| --------------- | ------------------------------------------------------------- | -------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Compute         | Server Groups ✅ (exists; gate actions only)                   | Nova                             | `compute::v2::server_group`                                                                            |
| Volumes         | Backups                                                       | `volumev3` + `backup` service up | `block_storage::v3::backup`                                                                            |
| Volumes         | Groups, Group Snapshots, Group Types                          | `volumev3` ≥ 3.14                | `block_storage::v3::{group, group_snapshot, group_type}`                                               |
| Network         | Network Topology                                              | Neutron                          | see item 20                                                                                            |
| Network         | Trunks                                                        | Neutron ext `trunk`              | `network::v2::trunk`                                                                                   |
| Network         | Network QoS (policies, bandwidth/DSCP/min-bw rules)           | Neutron ext `qos`                | `network::v2::qos::{policy, …rule}`                                                                    |
| Network         | Load Balancers (listeners, pools, members, monitors)          | `load-balancer`                  | `load_balancer::v2::{loadbalancer, listener, pool, healthmonitor, l7policy}` (`load_balancer` feature) |
| Network         | DNS zones and record sets                                     | `dns` (Designate)                | item 68                                                                                                |
| Orchestration   | Stacks, Resource Types, Template Versions, Template Generator | `orchestration` (Heat)           | raw calls (item 62)                                                                                    |
| Object Store    | Containers and objects                                        | `object-store` (Swift)           | item 69                                                                                                |
| Share           | Shares, snapshots, share networks                             | `sharev2` (Manila)               | item 70                                                                                                |
| Key Manager     | Secrets, containers                                           | `key-manager` (Barbican)         | raw calls (item 55)                                                                                    |
| Container Infra | Clusters, cluster templates                                   | `container-infra` (Magnum)       | item 71                                                                                                |

Before starting, check each module name against the crate's `pub mod` lines (AGENTS.md §6).

**Seen on the production Horizon** (read-only review): Groups and Group Snapshots, Load
Balancers (columns Name, IP Address, AZ, Operating/Provisioning Status, Admin State), Trunks,
Network QoS, and Orchestration with four panels: Stacks (Launch, Preview, Check, Suspend,
Resume, Change Template, Delete; bulk Check/Suspend/Resume/Delete), Resource Types (131),
Template Versions (13) and the drag-and-drop Template Generator. **Not present**: DNS, Object
Store, Shares, Key Manager, Container Infra: built last, only for other clouds, each now its own
item (Key Manager 55, DNS 68, Object Store 69, Shares 70, Container Infra 71). Order:
Load Balancers → Stacks (+ Resource Types, Template Versions) → Groups → Trunks → QoS →
Template Generator (largest, least used).

**Implementation.** One module per row, each following the "read-only panel" recipe in AGENTS.md
first and the "write action" recipe second. Register it in the module registry (item 19) with its
`requires`. Load balancers: list with operating/provisioning status pills, detail with tabs
(Overview · Listeners · Pools · Members · Monitors); creation needs the wizard (item 2). Heat (now
item 62): stack list and detail (Topology · Overview · Resources · Events · Template), launch from a
template URL or file via the wizard. Raw calls go in `fetch.rs` (GET) and `actions.rs` (writes, with
dry-run tests) like the typed ones. Filter Neutron lists by `tenant_id` (admins see all projects).

**Settings that come with these panels** (moved from item 29): the `backup` plugin with
Volume Backups (`OPENSTACK_CINDER_FEATURES["enable_backup"]`, done as the `backup` plugin);
`[launch] enable_scheduler_hints` with a *Scheduler Hints* step in *Launch Instance* (item 91).
(Swift's `SWIFT_FILE_TRANSFER_CHUNK_SIZE`
moved to item 69.)

## 6. Availability-zone pills and lock icon (instances)

> ✅ **Done.** `widgets::{AzPill, az_colors, LockIcon}`; pills on instances (list/detail) and
> volumes. `Instance.locked` from 2.47; menus now show only Lock *or* Unlock.

**Goal.** Colour-coded AZ pills in the instance list/detail; lock/unlock icon like Horizon.

**Implementation.**

- `widgets::AzPill { name }`: colour picked deterministically from a fixed, colour-blind-safe
  palette of 8 (hash of the AZ name, e.g. FNV-1a → index), so `alh1` is always the same
  colour on every page; the text stays visible (never colour-only). Use it in instances,
  volumes, networks.
- `locked` requires compute microversion ≥ 2.9 (see item 1); add `Instance.locked: bool`
  (+ `locked_reason` with ≥ 2.73 if available). Show an inline SVG lock/unlock icon next to
  the name with `title`/`aria-label` ("Locked"/"Unlocked"), and make `ops::instance_items`
  receive `Some(locked)` in the list too (today it gets `None` and shows both actions).

## 7. Key pair generation

> ✅ **Done.** `OsAction::KeyPairGenerate { name }`: Ed25519 via `ssh-key` (server only), public
> key sent with `keypair/create_210` (`type: ssh`); with writes `on`, `ActionOutcome.secret`
> carries `name.pem` once (download box in the result dialog), never in dry-run. `Secret`'s
> `Debug` is redacted. RSA-4096 left out (heavy `rsa` dependency); add it if old images need
> it. **Not yet tried with writes `on`**: check the download on a test cloud.

**Goal.** "Create Key Pair" (generate) besides "Import Public Key".

**Implementation.**

- Nova removed server-side generation in microversion 2.92. Generate **on the Moonlight
  server**: `ssh-key` crate (`Algorithm::Ed25519`, optional RSA-4096), then import the public
  key with the existing `OsAction::KeyPairImport` path.
- New `OsAction::KeyPairGenerate { name, algorithm }`: in `on` mode the response carries the
  private key **once** (`ActionOutcome.secret: Option<String>`), never logged or cached; the
  dialog offers "Download name.pem" (Blob + `download` attribute) and warns it will not be
  shown again. In `dry-run` show the import request with a freshly generated public key and
  **no** private key.
- Tests: dry-run body contains `public_key` starting with `ssh-ed25519 `; private key never
  appears in previews or logs.

## 8. Image owner shows the project name

> ✅ **Done.** `fetch::project_names` = user's projects + Keystone project list when allowed
> (admins). Unknown owners show a short ID with the full ID on hover. Observation: as admin,
> Glance lists other projects' private images too (34 here); filtering like Horizon (own, public,
> shared and community) is a follow-up.

**Goal.** *Owner* column (today "Project"/"-") shows the owning project's name.

**Implementation.**

- Glance `owner` is a project ID. Resolve names with the user's projects
  (`os::projects`, already cached 5 min); for admins optionally `identity::v3::project::list`
  (cache 5 min). Unknown owners → short ID with the full ID in `title`.
- `mapping::images(raw, project_id, project_names: &HashMap<String,String>)` →
  `Image.owner_name: Option<String>`; keep `owned_by_project` for sorting and for the
  edit/delete permission in `ops::image_items`.

## 9. Flavor details popover

> ✅ **Done.** `widgets::FlavorPopover`, data from the embedded flavor (2.47 includes extra specs)
> or the flavor list. `position: fixed`, placed on hover/focus and flipped upwards near the
> bottom, so table scrolling does not clip it. No lazy extra-spec fetch was needed.

**Goal.** Hovering (or focusing) the flavor in the instance list shows vCPUs, RAM, root
disk, ephemeral, swap, RX/TX factor, public/private and extra specs.

**Implementation.**

- Data: `Instance.flavor: FlavorInfo` (already built by `mapping::flavor_of`) extended with
  `ephemeral_gb`, `swap_mb`, `extra_specs` (embedded with microversion ≥ 2.47, else from the
  flavors list already fetched; extra specs need `flavor::extra_spec::list` per distinct
  flavor — fetch lazily on first hover via a `get_flavor(id)` server fn and cache in the page).
- `widgets::Popover { trigger, content }`: CSS-positioned card shown on `:hover` and
  `:focus-within` (keyboard accessible, `role="tooltip"`, `aria-describedby`), small delay
  to avoid flicker, flips near the viewport edge.

## 10. ✅ Bulk "Delete xxxx" buttons per section

> ✅ **Done.** `DataTable` takes an optional `Select { key, picked }`: a checkbox column, rows
> whose menu offers no delete are disabled (images of other projects), and the header box
> selects every visible row (filter respected), with an indeterminate state. Picks of rows
> that vanish on refetch are dropped. `OsAction::Batch(Vec<(label, OsAction)>)` runs each
> item and keeps going after a failure; `ActionOutcome.results` reports one `ItemResult` per
> item, and the outcome view lists the failures. Batches are capped at `MAX_BATCH` (100), and
> cannot be empty or nested. `BulkButton` ("Delete Volumes (2)") plus `ops::bulk_delete` are on
> all ten list panels. Not done: the tables inside detail tabs (subnets, ports, rules,
> interfaces) still delete one row at a time.

**Goal.** Like Horizon: row checkboxes + a "Delete Instances / Volumes / Networks…" button in
the page header that deletes the selection after confirmation.

**Implementation.**

- `DataTable`: optional `selectable: Option<fn(&T) -> String>` (row key) → first column with
  checkboxes, header "select all visible" (respects the filter), selection in
  `RwSignal<HashSet<String>>`, exposed to the page through a callback/signal.
- `OsAction::Batch(Vec<OsAction>)`: `apply` runs each item; in dry-run the outcome lists all
  requests; in `on` continue on error and report per-item results
  (`ActionOutcome.results: Vec<ItemResult { label, ok, error }>`). Cap the batch size (e.g. 100).
- `ops.rs`: `bulk_delete(lang, label_key, items: Vec<(id, name)>, make: fn(String) -> OsAction)`
  → `Dialog::confirm` listing every selected name.
- Header button disabled when nothing is selected or `WriteMode::Off`; label with count
  ("Delete 3 Instances"). Apply to every section that has a single delete today.
- Tests: batch dry-run produces one DELETE per item, order preserved; `off` builds nothing.

---

## 11. Progress bar for instances with an ongoing task

> ✅ **Done.** `widgets::{is_busy, Busy, poll_while_busy}`; `i18n::task_state`. Bar under the
> status pill (list) and next to the title (detail); refetch every 5 s while busy (max 120
> rounds), "Updating…" next to the timing chip.

**Goal.** While an instance has a task in progress (`OS-EXT-STS:task_state` set:
`scheduling`, `spawning`, `rebooting`, `deleting`, `image_snapshot`, …) show an
**indeterminate** progress bar, like Horizon's animated status.

**Implementation.**

- Data already there: `Instance.task_state` (list and detail). Also treat transitional
  statuses (`BUILD`, `REBOOT`, `HARD_REBOOT`, `RESIZE`, `MIGRATING`) as "busy" when
  `task_state` is missing: `fn is_busy(&Instance) -> bool` in `widgets.rs` (unit-tested).
- `widgets::Busy { label }`: a thin bar under the status pill with a CSS keyframe sliding
  stripe (`@keyframes indeterminate`), `role="progressbar"` **without** `aria-valuenow`
  (= indeterminate for assistive tech) and `aria-label` = the localised task. Static stripe
  under `prefers-reduced-motion`.
- Localise task names (`i18n::task_state(lang, &str)`, fallback to title case like statuses).
- Auto-refresh (GET only): on the instances list and detail, when any row is busy, refetch
  every 5 s (`set_interval` in an `Effect`, cleared via `on_cleanup` when nothing is busy or
  the page unmounts); show "Updating…" next to the timing chip. Cap at 10 min.

## 12. Hide flavor extra specs

> ✅ **Done.** Field removed from `FlavorInfo`, mapping, popover, detail and i18n.

**Goal.** Do not show flavor extra specs in the instance flavor popover nor in the
instance detail *Specs* card (they are operator-internal).

**Implementation.** Remove the `extra_specs` block from `widgets::FlavorPopover` and from
`instance_detail::overview`. Keep `FlavorInfo.extra_specs` in the model only if an admin
view needs it later; otherwise drop the field, its mapping and the test assertion in
`embedded_flavor_keeps_details_and_finds_the_id_by_name`. Remove the now-unused i18n key
`ExtraSpecs` (the compiler will not flag unused keys; grep).

## 13. 🐞 FIX: row action menus clipped by the table

> ✅ **Done.** `dialog::place_menu` positions the open menu with `position: fixed` (right-aligned,
> flips up). `install_menu_dismissal` (from `Layout`) closes menus on outside click, resize and
> scroll (capture phase, ignoring scrolls inside the menu); `Esc` closes and refocuses the
> trigger. A shared `floating.rs` is still a good refactor when item 4 lands.

**Problem.** `.row-menu .menu-list` is `position: absolute` inside `.table-wrap`, which has
`overflow-x: auto`. That makes it a clipping container (overflow-y becomes `auto` too), so
the menu of the last rows is cut or scrolls inside the table.

**Implementation.**

- Reuse the technique of `FlavorPopover` (item 9): render the menu with `position: fixed`
  and compute coordinates from the trigger's `getBoundingClientRect()` when the
  `<details>` opens (`on:toggle`); right-align to the trigger, flip upwards when
  `rect.bottom + menuHeight > innerHeight`.
- Close on scroll/resize (`window` listeners added on open, removed on close) and on
  outside click / `Escape` (keyboard users), and return focus to the trigger.
- Better long-term: a shared `components/floating.rs` (`Floating { anchor, open, placement }`)
  used by row menus, header menus, the flavor popover and the custom dropdown (item 4).
- Check: last row of a long table, a table narrower than the menu, and the header
  `page-actions` menu on detail pages.

## 14. ✅ Friendly error page with a "back to safety" button

**Goal.** When an API call fails (region not available, endpoint missing, 403, 404,
OpenStack 5xx, timeouts), show a clear page instead of the raw error string, with a
button that always leads somewhere that works.

**Implementation.**

- Typed errors: replace stringly `ServerFnError::new(..)` with a serialisable
  `AppError { kind: ErrorKind, detail: String, service: Option<String> }`
  (`Unauthenticated | Forbidden | NotFound | ServiceUnavailable | RegionUnavailable |
  Timeout | OpenStack(status) | Internal`) implementing Leptos 0.8's `FromServerFnError`,
  so server functions return `Result<T, AppError>`. Map in one place
  (`server::errors::classify`): SDK `OpenStackError`/`ApiError` → HTTP status, catalog
  "endpoint not found" → `ServiceUnavailable`/`RegionUnavailable`, connect/timeouts → `Timeout`.
- `widgets::ErrorPage { error }` replacing `ErrorBox` for page-level failures: icon, localised
  title and explanation per kind, collapsible "Technical details" (the raw message, the
  request id if present), and actions:
  - **Back to safety** → `/project` (Overview) — always available;
  - **Retry** (refetch) for `Timeout`/5xx; **Sign in** for `Unauthenticated`.
- Region safety: if the session's region is not in the catalog any more, `get_session_info`
  falls back to the first available region and shows a notice.
- Wrap `<Outlet/>` in an `<ErrorBoundary>` so panics/unexpected errors also land there.
- Partial failures stay inline (e.g. one quota section failing on Overview), as today.

**Done.** `model::AppError`/`ErrorKind` (plus `Invalid` for validation errors) is the error
type of every server fn; `server::errors::classify` parses the SDK's error text (status,
`request-id`, ``Service `x` is not configured``), and `sfe()` uses it. `widgets::ErrorPage`
is what `render()` shows; *Retry* bumps `data_version`. The page only offers *Retry* and *Back
to safety* (a separate *Switch region* button was removed on request). `api::ctx` and
`get_session_info` fall back to the first catalog region (`os::fallback_region`) and the layout
shows a notice.

*A region that is down* (still in the catalog, but not answering): every read server fn runs
under `os::within(REQUEST_TIMEOUT = 20 s)` and Keystone calls are bounded too (the SDK's HTTP
client has no timeout). Version discovery is capped at 5 s per service, and a connection is
only cached when discovery succeeded, so the header still lists regions and the region
recovers by itself. For `Timeout`/`ServiceUnavailable`/`RegionUnavailable` with other regions
available, *Back to safety* switches to the first
other region of the same project before reloading the Overview. To try it locally, map one
region's endpoint hosts to `127.0.0.1` (fails fast) or `10.255.255.1` (hangs) in
`/etc/hosts`.

## 15. AZ pills: outline only

> ✅ **Done.** Transparent background, 1px border, 12-hue palette (`widgets::az_color`, FNV-1a). A
> test guarantees the production zones (alh1, mad1, mer1, nova) get distinct colours; extend it
> when new zones appear.

**Goal.** Availability-zone pills with a coloured border and text, no fill.

**Implementation.** In `widgets::AzPill` set `border-color` and `color` from `az_colors`
(foreground value) and `background: transparent`; CSS `.az-pill { border: 1px solid; }`.
The current foreground colours are dark enough (≥ 4.5:1 on white). Keep the deterministic
colour per zone and the visible text.

## 16. Copy-to-clipboard on detail views

> ✅ **Done.** `widgets::Copyable` (Clipboard API with `execCommand` fallback, `aria-live`
> "Copied"). Used for name, ID, project in every detail page, plus MAC, CIDR, fingerprint and the
> public key.

**Goal.** In detail pages (instances, volumes, snapshots, images, networks, subnets, ports,
routers, security groups, key pairs), the key fields of the *Information* card — **name,
ID, project ID** (also IP/MAC/CIDR where present, and the public key) — get a copy button.

**Implementation.**

- `widgets::Copyable { value, label }`: renders the value plus a small icon button
  (`aria-label` "Copy ID", `title`), using `navigator.clipboard.writeText` (web-sys features
  `Navigator`, `Clipboard`; available on `127.0.0.1`/HTTPS secure contexts). Fallback when
  unavailable: select the text in a hidden input and `document.execCommand("copy")`.
- Feedback: icon turns into a check for 1.5 s and an `aria-live="polite"` region says
  "Copied" (localised). No server call involved.
- Use it in `details.rs::Kv` via a `copy` prop (`<Kv label=K::Id copy=true>`) and in
  `instance_detail.rs`; for IDs in tables, consider copy-on-hover later.

## 17. Instances list: busy bar in the *Task* column

> ✅ **Done.** `instances::task_cell`: task name (or the transitional status when there is no
> task) with the bar under it; *Status* shows only the pill; the column sorts by task. Test:
> `task_label_prefers_the_task_then_a_transitional_status`.

**Goal.** In the instances list the indeterminate bar (item 11) sits under the status pill;
move it to the *Task* column, under the task name it describes. The *Status* column shows
only the pill.

**Implementation.**

- `pages/instances.rs`: split `status_cell` into the pill alone and a new `task_cell(i, l)`
  that renders `dash(task name)` plus `<Busy label=…/>` when `is_busy(..)`. When busy without
  a `task_state` (transitional status such as `BUILD`), show the localised status as the
  task text so the bar is never unlabelled.
- Use `task_cell` for the `K::Task` column; keep `.sort_text` on the task.
- Instance detail is unchanged (bar next to the title). Check `status_cell` callers with grep
  before changing its signature; adjust `.status-cell` CSS (stacked layout) for the new cell.

## 18. ✅ Dark mode

**Goal.** A dark theme that follows the OS (`prefers-color-scheme`) with a manual override
(Light / Dark / System) in the user menu, remembered like the language.

**Implementation.**

- `style/main.css` already defines tokens on `:root` (`--ink`, `--bg`, `--line`, status
  `--ok/-bg` …), but ~40 hex colours are still hard-coded in rules: replace them with
  tokens first (no visual change), then add the dark values under
  `@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { … } }` and
  `:root[data-theme="dark"] { … }`. Set `color-scheme: light dark` so native controls and
  scrollbars follow.
- Override: an `ml_theme` cookie (`light|dark|system`), read on the server like `ml_lang`
  (`i18n::cookie_value`) and rendered as `data-theme` on `<html>` during SSR, so there is
  no flash and hydration matches. Menu entry next to the language switcher; i18n keys EN+ES.
- Check contrast (≥ 4.5:1 text) for status pills, AZ pill outlines (`widgets::az_color`
  returns fixed hues: add a dark-palette variant or use CSS variables per hue; extend the
  distinct-colour test), skeleton/busy animations, popovers, dialogs and the utility bar
  (`--utility` is already dark: keep it, adjust its border).
- Brand colours stay (primary `#0066ff`, lighten only if contrast on the dark background
  fails).

**Done.** Instead of two duplicated dark blocks, every token is `light-dark(light, dark)` on
`:root`, driven by `color-scheme` (`light dark`, forced by `[data-theme]`); lightningcss
transpiles it for older browsers. `--accent` (`#5c9dff` in dark) is the brand blue for text
and indicators, `--primary` stays for fills. `src/theme.rs` holds the `ml_theme` cookie and
`ThemeCtx`; `ThemeMenu` sits next to the language menu (header and login). AZ pills get a
(light, dark) pair per hue, with a WCAG contrast test.

## 19. ✅ Modules: panels shown by service availability

> ✅ **Done.** `os::capabilities` detects, per project and region and with GETs only, the
> catalog services, Neutron extension aliases (`GET /v2.0/extensions`) and the highest
> compute/volume microversions. Nova and Cinder report those in the version document's
> `version`, so they come from the discovered endpoint. The result is cached for 10 min (only
> when detection succeeded) and returned as `SessionInfo.capabilities`; the client keeps it in
> `Dialogs.caps`. `model::Requires` (`Service`, `NetExt`, `ComputeMv`, `VolumeMv`, `All`) has
> one pure, tested `met()`. Unknown capabilities fail open, so nothing is hidden by a
> detection hiccup. Nav panels and groups declare `requires`: Volumes needs `block-storage`
> (aliases included), Routers and Floating IPs need `router`, Security Groups need
> `security-group`, and Orchestration only shows where Heat exists. A direct URL to a hidden
> panel shows the *Service not available* page. Version-dependent actions live in
> `model::needs`: Rebuild (2.94), unshelve to a zone (2.77), upload to image (3.1), volume
> messages (3.3), static routes. A *Detected capabilities* card (versions, plugins, hidden
> panels, extensions; `lists::ModulesCard`) was on API Access and is now kept for the Admin
> dashboard. Production es-central-1: Nova ≤ 2.95, Cinder ≤ 3.70, 90
> extensions, and **no `extraroute-atomic`**: item 26's atomic route calls would have been
> rejected there. With plain `extraroute`, Moonlight now replaces the whole list
> (`RouterSetRoutes`). Still to do: `[modules]` overrides once item 3 exists, and gating
> future modules (LB, Trunks, Heat) as they are added in item 5.

**Goal.** Show a panel, detail tab or action only when the cloud and region support it, as
Horizon does: Load Balancers only with Octavia, Trunks only with the Neutron `trunk`
extension, Orchestration only with Heat, Cinder Groups only with a recent enough API.
New services become "modules" that switch on by themselves when the catalog has them.

**Implementation.**

- "Almost dynamic": modules are compiled in (Leptos routes are static) and **enabled at
  runtime**. No plugin loading or WASM splitting; cargo features per module can come later
  if the bundle grows.
- Capabilities, detected per scoped connection (project + region) and cached with it
  (moka, ~10 min), GET only:
  - services: catalog entries for the region (`load-balancer`, `orchestration`,
    `object-store`, `dns`, `sharev2`, `key-manager`, `container-infra`, …);
  - Neutron extensions: `GET /v2.0/extensions` aliases (`trunk`, `qos`, `router`,
    `port-security`, `dns-integration`, …);
  - max microversions from the version discovery that `os::discover` already does
    (compute, `volumev3`); optional `GET /os-services` for Cinder backup (admin only:
    treat 403 as "unknown" and show the panel).
  - Result: `model::Capabilities { services, network_extensions, compute_mv, volume_mv }`,
    returned by `get_session_info` so SSR and hydration agree.
- Registry: extend `nav::Panel` with `requires: Requires`, where `Requires` is
  `Always | Service(&str) | NetExt(&str) | VolumeMv(u16, u16) | All(&[Requires])`. One
  pure fn `Requires::met(&Capabilities) -> bool`, unit-tested with fixture capabilities.
  `nav.rs` hides groups whose panels are all hidden (Orchestration today is an empty,
  disabled group).
- Use the same check for detail tabs and `ops.rs` menu items (e.g. "Add to server group",
  "Create trunk" on a port, QoS policy field in network dialogs), and for Overview quota
  sections (load balancer quotas only with Octavia).
- Direct URL to a missing module: item 14's `ServiceUnavailable` page with "Back to safety".
- Override from `moonlight.conf` (item 3): `[modules] disabled = ["dns"]` to hide a module
  even when present, `enabled = [...]` to force it (e.g. a catalog entry missing in one
  region).
- Panel *Settings › Modules* (or the API Access page) lists detected modules and why each is
  on or off: easy to debug on a new cloud.

## 20. ✅ Network Topology (Topology and Graph tabs)

> ✅ **Re-checked (2026-11-03).** Real mouse and keyboard on the dev cloud, project admin2 (7
> external networks, no devices), not submitted: wheel zoom, drag pan, hover (node highlighted,
> the rest dimmed, popover with name, kind, status, CIDR), *Center Topology*, *Small*, *List*, the
> Graph tab and a click on a hovered node to its detail page, with no console panic (the disposed-
> signal crash that a JS `.click()` hid). Added a unit test with 12 networks and 130 servers
> (`a_project_with_over_a_hundred_instances_lays_out_quickly_and_in_bounds`: force layout bounded
> and finite, every device boxed, collapse shortens). Observed: *Small* labels are cut with an
> ellipsis at about 22 characters, not 8; the Graph canvas is 640 px tall, so on a short window
> its lower part is below the fold. Not verified, because the project has no devices and no
> Horizon tab was shared: devices and routers drawn on the cloud, *Toggle Network Collapse*
> against Horizon's, the side-by-side Horizon comparison, and a mock-served fixture testbed (the
> layout's pure functions have unit tests instead).
>
> 🔍 **Code review (2026-10-26).** Fixed: a device with several ports on one network (a router with
> an interface per IPv4 subnet) drew stacked wires with overlapping address labels; now
> `topo_layout::pairs` groups links per device and network and both tabs draw one line with all
> addresses (`Wire::links`). Wheel zoom is proportional to the distance
> (`topo_layout::wheel_factor`, pinch aware) instead of 1.2× per event, which jumped to the limit on
> a trackpad. *Toggle Labels* and a language change keep the zoom and pan (`KeptView`), and the
> readable 1:1 first view applies to every new canvas, not only before the first *Center Topology*.
> Canvas handlers use `try_` accessors, and the popover opens left of the pointer near the right
> edge. Tests: `two_ports_on_one_network_make_one_wire_with_both_links`,
> `wheel_zoom_is_proportional_to_the_distance`. Not yet checked in the browser (extension not
> connected); the rest of the list above is still open.
>
> ✅ **Done.** `get_topology` → `fetch::topology` (8 parallel GETs, Neutron lists filtered by
> `tenant_id`) → pure `mapping::topology`; router gateways come from `external_gateway_info`,
> because their port belongs to no project. Geometry is pure and tested in
> `components::topo_layout` (columns, collapse packing, Fruchterman–Reingold with a quadratic
> pull to the centre, `viewBox` zoom and pan). `pages/topology.rs` draws both tabs with the same
> SVG nodes: network colours come from the AZ palette, and the first view is 1:1 when fitting
> everything would shrink the text below 80 %. *Center Topology* fits all. Wheel zoom works
> without `undelegated` because Leptos' `delegation` feature is off.

**Goal.** Horizon's *Network › Network Topology*: a **Topology** tab (networks as vertical
bars, routers and instances as boxes connected to the networks they have ports on) and a
**Graph** tab (force-directed graph of the same nodes). Nodes link to their detail pages.

**Implementation.**

- Data: one `get_topology` server fn; `fetch::topology` runs the lists already used
  elsewhere in parallel (networks, subnets, routers, ports, servers, floating IPs), GET only,
  filtered by `tenant_id` like the other Neutron lists.
- Pure `mapping::topology(...) -> Topology { networks, routers, servers, links }`, where a
  link is a port (`device_owner` `network:router_interface*`, `network:router_gateway`,
  `compute:*`) with its fixed IPs. Unit-test with API-shaped JSON, including a router
  gateway on an external network, a server on two networks and a port with no device.
- Topology tab: inline SVG rendered by Leptos (no JS library). External networks first, then
  networks sorted by name; devices placed between the networks they connect; status
  colour on each node; pan with drag and zoom with the wheel via `viewBox`. Options as in
  Horizon: collapse networks, show labels.
- Graph tab: a small deterministic force layout written in Rust (Fruchterman–Reingold, fixed
  seed, ~300 iterations, run on the client), so SSR shows a placeholder and the layout is
  stable between reloads. Same SVG node components as the Topology tab.
- Interaction: hover highlights a node and its links; click opens the detail page; a
  popover shows name, status, IPs. Keyboard: nodes are links in tab order. Text fallback:
  a "List" toggle showing the same data as a table (accessibility and huge projects).
- Colours from the CSS tokens (item 18) so it works in dark mode.
- Horizon's toolbar, to match: *Small / Normal* node size, *Toggle Labels*, *Toggle Network
  Collapse*, *Center Topology*, and *Launch Instance*, *Create Network*, *Create Router*
  buttons (reuse the existing dialogs in `ops.rs`).

## 21. ✅ Instance actions parity

> ✅ **Done.** The menu comes from a pure, tested `ops::allowed(status, locked, busy) -> Vec<Op>`
> in Horizon's order: Confirm/Revert first while `VERIFY_RESIZE`, Unshelve only when
> `SHELVED*`, Unrescue only in `RESCUE`, and only Edit, (Un)lock and Delete while a task runs.
> There are 11 new `OsAction`s, each with a dry-run test: resize, confirm/revert, rebuild
> (`rebuild_294`, so volume-backed servers work too; the SDK sends the microversion), shelve,
> unshelve (with an AZ via `unshelve_277`), rescue/unrescue (the `adminPass` comes back as a
> one-time `Secret` of kind `Password`), attach/detach interface, and port security groups.
> Associating a floating IP from the instance either picks a free IP or allocates one with
> `port_id`, so Neutron associates it in the same request. *Launch* / *Launch as Instance*
> open the wizard preset on image, volume and snapshot rows. The Instances header gets a split
> button (Start / Shut Off / Soft Reboot) over the selection, using `OsAction::Batch`, and the
> Interfaces tab has a Detach action per row. The dialog engine gained `Source::Of(set, owner)`
> (the choices whose first cell is the owner ID, e.g. one instance's ports). Choices that are
> preselected or preset by the caller now run `on_change`, which fixed the port's groups not
> being prefilled. Bulk Start/Stop doesn't filter by status: Nova rejects the wrong ones, and
> the batch reports them per item.

**Goal.** Horizon's instance menu has 23 actions; Moonlight has 13 (start, shut off, soft/hard
reboot, pause, suspend, lock, their inverses, edit, snapshot, attach volume, delete). Add the
missing ones, in this order:

| Action                                           | Nova / Neutron call                       | Notes                                                                                                            |
| ------------------------------------------------ | ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Associate Floating IP (from the instance)        | Neutron FIP update `port_id`              | Reuse `FloatingIpAssociate`; pick an existing FIP or allocate one; choose the port when the instance has several |
| Attach / Detach Interface                        | `os-interface` POST / DELETE              | Network or existing port, optional fixed IP; detach from the *Interfaces* tab too                                |
| Edit Security Groups / Edit Port Security Groups | Neutron port update `security_groups`     | Per port, like Horizon's newer form; Nova `addSecurityGroup` only as fallback                                    |
| Resize, then Confirm / Revert                    | `resize`, `confirmResize`, `revertResize` | Status `VERIFY_RESIZE` shows Confirm/Revert first in the menu                                                    |
| Rebuild                                          | `rebuild`                                 | Image picker; boot-from-volume needs compute ≥ 2.93, else hide                                                   |
| Shelve / Unshelve                                | `shelve`, `unshelve`                      | Unshelve with optional AZ (≥ 2.77)                                                                               |
| Rescue / Unrescue                                | `rescue`, `unrescue`                      | Nova returns `adminPass`: show it once, never log it (as item 7)                                                 |
| Launch from Image / Volume / Snapshot rows       | `launch_dialog` with a preset source      | Horizon's *Launch* and *Launch as Instance*                                                                      |
| Bulk Start / Shut Off / Soft Reboot              | `OsAction::Batch` (item 10)               | Header "More Actions" in Horizon                                                                                 |

**Implementation.** One `OsAction` variant per row with a dry-run test (AGENTS.md "write
action" recipe); show only the actions valid for the current status and lock state (Horizon
hides Unshelve unless `SHELVED*`, Confirm/Revert unless `VERIFY_RESIZE`, …), with a pure,
tested `fn allowed(&Instance) -> Vec<Op>`. Console and Log (tabs already present, disabled)
are item 31: they are POSTs, so they need an explicit allow-list decision (AGENTS.md §1.3)
before they can be enabled.

## 22. ✅ Volume actions parity

> ✅ **Done.** Four new `OsAction`s with dry-run tests: `VolumeRetype` (`os-retype`, migration
> policy `never` / `on-demand`, rejects the current type), `VolumeUploadImage`
> (`os-volume_upload_image`, volume API 3.1, `container_format: bare`, *Force* only when
> attached), `VolumeTransferCreate` and `VolumeTransferAccept` (legacy `os-volume-transfer`,
> which works on every v3). The transfer ID and `auth_key` come back as a one-time `Secret` of
> kind `Transfer`. *Accept Transfer* is on the Volumes header. The menu follows Horizon:
> retype and upload for `available` / `in-use`, transfer only for `available`, and Detach per
> attachment (already there). The volume detail page lists Cinder's user messages for the
> volume (`GET /messages`, 3.3, filtered by `resource_uuid` in a pure mapper). A failure is
> logged and leaves the list empty. *Launch as Instance* and snapshot rows came with item 21.
> Still open: *Group* / *Group Snapshot* columns (need item 19), and listing or cancelling
> pending transfers.

**Goal.** Horizon's volume menu has 10 actions plus *Accept Transfer*; Moonlight has 5 (edit,
extend, snapshot, attach, delete). The volume detail also has *Snapshots* and *Messages* tabs.

**Implementation.**

- *Change Volume Type*: `os-retype { new_type, migration_policy: never | on-demand }`.
- *Upload to Image*: `os-volume_upload_image { image_name, disk_format, container_format,
  force }`; link the new image when done.
- *Manage Attachments*: list attachments with a Detach button (reuse `ServerDetachVolume`)
  next to the existing Attach.
- *Launch as Instance*: see item 21.
- *Create Transfer* / *Accept Transfer* (header): `os-volume-transfer`; the `auth_key` is a
  secret shown once, never logged (as item 7). Lower priority inside this item.
- Detail tabs: *Snapshots* (snapshots filtered by `volume_id`, GET) and *Messages*
  (`GET /v3/messages?resource_uuid=…`, volume ≥ 3.3): Cinder's user-facing error messages,
  very useful when a volume ends in `error`.
- Snapshot rows: *Launch as Instance* and *Create Volume* from the snapshot.
- Columns: *Group* on volumes, *Group Snapshot* on snapshots, only when item 19 says Cinder
  groups are available.

## 23. Overview usage summary

> ✅ **Done.** `fetch::usage` (GET `os-simple-tenant-usage`, compute 2.47, marker paging up to
> 10 × 1000); `model::{usage_range, UsageSummary}` with totals summed per server and
> `csv()`; GET form with `?start=&end=` (default: this month, UTC); `i18n::decimal`. Tests:
> range validation, totals, CSV quoting, mapper.

**Goal.** Horizon's Overview has, under the quota charts, a *Usage Summary*: a From/To date
range, totals (active instances, RAM, VCPU-hours, GB-hours) and a table per instance (Name,
VCPUs, Disk, RAM, Age), plus *Download CSV Summary*.

**Implementation.** `compute::v2::simple_tenant_usage` (GET, paged with `marker` from 2.40)
for the session project; default range: current month. Pure mapper with a test; dates as
`<input type="date">`. The CSV is built in the browser from the same data (Blob + `download`
attribute), no extra server call. Localised number and date formats (`Lang::EsEs`).

## 24. API Access: credentials and downloads

> ✅ **Done.** `get_credentials` (session only, no Keystone call); `model::Credentials::{openrc,
> clouds_yaml}` (shell/YAML quoting, no password; tested); *View Credentials* card with copy
> buttons; downloads are `data:` links (`widgets::data_url`), no extra request.

**Goal.** Horizon's API Access has *View Credentials* and *Download OpenStack RC File* /
*OpenStack clouds.yaml File*. Moonlight only shows the endpoint table.

**Implementation.** All generated from the session, **without any secret**: *View
Credentials* shows user name and ID, project name and ID, auth URL and region (with
`Copyable`, item 16). `openrc` (v3) prompts for the password with `read -sr` like Horizon's;
`clouds.yaml` has no `password` key. Both are plain-text downloads from a GET server fn (or
built in the browser). Tests: neither file ever contains a token or password.

## 25. Metadata editor

> 🔗 **Issue:** [#1]
>
> ✅ **Done.** *Update Metadata* is
> in the row menu and detail header of instances (`Op::UpdateMetadata`, after *Edit
> Instance*, not while a task runs), volumes and snapshots (before *Delete*), and images the
> project owns (after *Edit Image*; `os_glance_*` properties are hidden, as Glance manages
> them). The dialog engine has `Field::key_values` (`Kind::KeyValue`, rows in their own
> signals so typing keeps focus; the value is a JSON list read with `Values::pairs`). The list
> rows now carry the data (`Instance.metadata`, `Volume.metadata`, `Snapshot.metadata`,
> `Image.properties`, from the list responses, no extra calls). `model::validate_metadata`
> (trim, no empty or repeated keys, 255 characters) runs in the dialog and on the server;
> unchanged metadata is not submitted. Writes: `ServerSetMetadata`, `VolumeSetMetadata`,
> `SnapshotSetMetadata` replace everything (`PUT …/metadata`); `ImageSetProperties` sends a
> JSON-Patch built by `model::metadata_changes` (`add` / `remove`, keys escaped as JSON
> Pointers, Glance core keys refused). Tests: 2 in `model`, 1 dry-run in `actions` (exact paths
> and bodies, bad input), 1 in `ops` (prefill, nothing changed, image add/remove). 217 tests
> pass, clippy clean on both targets. Browser (dev cloud, writes `on`, not submitted): the
> dialog opens on `gengar`, rows are added, filled and removed. Catalog
> slice (read-only): `fetch::metadata_options` lists `GET /v2/metadefs/namespaces?resource_types=…`
> (`metadef_pages`, shared with Admin › Metadata Definitions) and then each namespace's
> `?resource_type=…`, [`FAN_OUT`] at a time; `mapping::metadef_options` types the properties
> (`MetadataOption`: title, enum, default, integer/number/boolean). Glance already prefixes the
> property names in that response, so the mapper must not apply `prefix` again (ttcp-dev showed
> `CIM_PASD_CIM_PASD_…`). Server fn `list_metadata_options` (any member; only the six types in
> `METADATA_RESOURCE_TYPES`). `Field::catalog(type)` turns `KeyValueRows` into an *Add from
> catalog…* select grouped by namespace (keys already in the rows are hidden), with a `<select>`
> value for enums and booleans and a number input for integer/number properties. Wired in *Update
> Metadata* of instances, volumes, snapshots, images, flavors (extra specs) and host aggregates, and
> in Create Image's *Metadata* step. If the catalog fails or is empty the editor stays plain.
> 492 tests pass (1 new, in `mapping`), clippy clean on both targets. Browser (ttcp-dev, volume
> dialog, not submitted): the picker lists 51 properties in 9 namespaces; after the prefix fix, an
> image's dialog lists 57 with undoubled keys (`hw_cpu_policy` adds a select preset to `shared`).
> The picker is the shared `Combo` in `multi` mode (search box, ticks stay while open,
> an *Add (n)* footer adds them all at once), next to an *Add custom +* button of the same
> height. Row signals are created under the editor's `Owner` (the combo's owner is disposed on
> close; it panicked before). Browser (not submitted): filter `cpu`, tick two, *Add (2)* adds both
> rows, added keys leave the list. Leftovers done: Create Image's picker skips the keys with their
> own fields (`Field::catalog_skip(&IMAGE_FIELD_PROPS)`); `os_glance_*` and the new `[images]
> reserved_properties` (`ImagePrefs`, documented in CONFIGURATION.md and `moonlight.conf.example`)
> are hidden from image *Update Metadata* and from the picker, and `run_action` refuses to write
> them (`OsAction::image_property_keys`, `model::is_reserved_image_key`); namespace `objects` are
> offered as `Object › Property` (`mapping::metadef_options`, keys deduplicated). Browser (Create
> Image, not submitted): 104 suggestions, none of the four dedicated keys, no `os_glance_*`,
> objects such as `sw_database_cassandra_admin` present. 495 tests pass, clippy clean on both
> targets. Decided to leave it so: *Update Metadata* stays on images the project owns; a Glance
> 403 shows "no permission" (`MetadataForbidden`). *Update Metadata* on images
> other projects own moved to item 35 (it needs the same role setting as *Edit Image*).

**Goal.** Horizon's *Update Metadata* on instances, images, volumes and snapshots: a two-column
editor (available namespaces from the Glance metadata catalog / existing metadata).

**Implementation.** Start with a simple key/value editor (`Field::kv` in the dialog engine)
and one `OsAction` per resource (Nova server metadata PUT, Glance JSON-Patch properties,
Cinder volume/snapshot metadata PUT), each with a dry-run test. Then, optionally, suggestions
from `GET /v2/metadefs/namespaces` filtered by resource type, like Horizon.

**Setting that comes with the catalog** (moved from item 29): `[images] reserved_properties`
(`IMAGE_RESERVED_CUSTOM_PROPERTIES`), keys the image metadata editor hides and never sends.

**Notes for the catalog discussion** (left by item 45's session, 2026-11-03).

- **Where it plugs in.** *Create Image* is now a wizard (`ops::create_image_form`) whose second
  step, *Metadata*, is just `Field::key_values("metadata")` (`Step::new(tr(K::Metadata),
  &["metadata"])`). The catalog picker replaces or wraps that field; the dialog's `build` already
  reads it with `Values::pairs` and `validate_metadata`. *Update Metadata* (`ops::pairs_item`) uses
  the same field for instances, volumes, snapshots and images, so one new field kind serves all.
- **Overlap to decide.** *Create* and *Edit Image* keep `description`, `architecture`,
  `kernel_id` and `ramdisk_id` as custom properties (`IMAGE_FIELD_PROPS`) and refuse a repeated
  key ("repeated key"). A catalog namespace that offers `architecture` or `hw_*` properties would
  clash with the dedicated fields: hide those keys in the picker, or merge them.
- **Horizon's behaviour** to match: two columns (*Available Metadata* from the namespaces, *Existing
  Metadata* with values), filtered by resource type (`OS::Glance::Image`, `OS::Nova::Server`,
  `OS::Cinder::Volume`, `OS::Cinder::Snapshot`), typed inputs (enums as selects, integers,
  booleans), and namespace `properties_target` / `prefix` applied to the key.
- **Calls.** Reads only (`GET /v2/metadefs/namespaces?resource_types=…`, then each namespace's
  properties): fetch them when the step opens, through `fetch::Bounded` (AGENTS.md §6: never fan out
  one call per namespace unbounded), with a `ChoiceSet` or a dedicated loader. No SDK check yet
  that `image::v2::metadef` has what is needed; read the module's `pub mod` lines first.
- **Layout.** The wizard's first step fits a 729 px window (648 px); the metadata step has the
  room. If the picker needs more width, `Dialog::fixed_size` is available.
- **`os_glance_*` keys** (`os_glance_failed_import`, …) are Glance's, not custom: `Image::properties`
  still lists them (`is_image_core_key` does not know the prefix), while *Update Metadata* hides
  them. The picker and `[images] reserved_properties` should treat the prefix as reserved too.

## 26. Router static routes

> ✅ **Done.** `OsAction::{RouterAddRoute, RouterRemoveRoute}` → `add_extraroutes` /
> `remove_extraroutes` (only the changed route); `model::validate_route` on the server and in
> the dialog; *Add Static Route* button and per-row delete in the tab. Dry-run tests assert
> both bodies. Uses the `extraroute-atomic` extension unconditionally: gate it with item 19.

**Goal.** The *Static Routes* tab exists (read-only); Horizon also has *Add Static Route* and
delete per route.

**Implementation.** With the `extraroute-atomic` extension (item 19):
`add_extraroutes` / `remove_extraroutes` (PUT, only the routes changed); otherwise update the
whole `routes` list on the router. Validate the destination CIDR and the next hop (inside one
of the router's subnets) before building the request; dry-run tests for both paths.

## 27. ✅ Small parity gaps

> ✅ **Done (2026-10-26).** Large projects: measured, paging not needed yet. Server-rendered
> pages fetched from the dev server on ttcp-dev (GET only), rows and server time per list:
> *pokemon* instances 5 (4 API, 0.72 s), volumes 6 (0.17 s), images 27 (0.29 s), networks 7
> (0.21 s), routers 3, security groups 3, topology 8 API in 0.73 s; *admin2* images 31
> (0.27 s), networks 7, everything else 0–1 rows; every page under 0.8 s. That is far below
> the ~1000-row threshold, so `limit`/`marker` paging and server-side instance filters moved
> to *Also pending*, to start when a project gets near it. The production cloud's largest
> project was not measured (the repo config points at ttcp-dev): measure it there before
> reopening. Image upload stays item 45.
>
> ✅ **Partly done (earlier).** Floating IPs show *Description*, *DNS Name*, *DNS Domain* (`-` when
> empty; the DNS ones only with the `dns-integration` extension, via `Col::requires` and
> `table::available`, tested). *Help* and *Change Password* in the user menu
> come from `[ui] help_url` / `change_password_url` (item 3; http(s) only, hidden when
> unset). Left: the two points below.

- Large projects: Horizon pages lists 20 at a time and filters instances server-side (IPv4,
  IPv6, image, flavor, *Changes Since*). Moonlight loads everything and filters in the
  browser, which is better while lists are small; measure with the largest project and add
  `limit`/`marker` paging plus server-side filters only past ~1000 rows.
- Image upload (*Create Image* in Horizon) is item 45; it needs streaming
  through the server and a size limit, so keep it separate.

## 28. Modal sizes fitted to their content

> ✅ **Done.** `DialogSize { Small, Medium, Large }` → `.modal.s` (460px), `.m` (620px), `.l`
> (1040px). `Dialog::size()` (pure, tested) picks `Large` for wizards, `Small` for
> confirmations and forms of up to three fields with no text area, checkbox list or picker,
> `Medium` otherwise; `.sized(…)` overrides it. The blank space came from `.wizard-pane`'s
> `min-height: 380px`, which every form had; it now only applies inside wizards. Results
> (dry-run request preview) open at least `Medium`. Measured: *Allocate IP* 620×537 → 460×297,
> *Create Snapshot* and *Attach Volume* 460×220, *Delete Volume* 460×312, *Create Volume* (9
> fields) 620×653, *Launch Instance* 1040×512.

**Goal.** Every dialog opens at `max-width: 620px` (`.modal`), so short forms such as *Allocate
IP* (two fields) leave a lot of blank space, while the wizard uses `.modal.wide` (1040px).
Size each dialog to what it holds.

**Implementation.** A `size: DialogSize { Small, Medium, Large }` on `Dialog` (default
`Medium`), mapped to classes `.modal.s` (~440px), `.modal.m` (620px) and `.modal.l` (wizard,
1040px). Pick it automatically in one pure, tested function: `Large` when the dialog has
`steps`, `Small` for confirmations and forms of up to ~3 short fields (no `area`/`multi`),
`Medium` otherwise; allow `.size(…)` to override it. Check that nothing reserves height
(`min-height` only on `.wizard-pane`) and that the request preview of dry-run still fits
(wraps or scrolls inside the body). Verify on phone width: every size stays `width: 100%`.

## 29. Horizon settings parity in `moonlight.conf`

> ✅ **Done (first batch; the rest moved, see below).** Reviewed against Horizon's settings
> reference (Rocky). Defaults live
> once, in `src/prefs.rs` (browser-side) and `server/settings.rs` (server-side); where Moonlight
> already behaved a certain way that stays the default, otherwise Horizon's. Tests: a file with
> only `[keystone] url` gets every default, an empty section equals a missing one, each invalid
> value stops start-up, and the shipped `moonlight.conf.example` must stay valid.
>
> - `[keystone] interface`; `[tls] ca_file`, `verify` (into the SDK's `CloudConfig`).
> - `[session] timeout_minutes` (absolute, from sign-in; cookie `max_age` follows),
>   `token_margin_seconds`, `password_expiry_warning_days` (banner; Keystone's
>   `password_expires_at` kept in the session; old cookies still decode).
> - `[login] domain_choices` / `domain_hidden` (also enforced by `login`),
>   `password_autocomplete`, `password_reveal`, `message_file`; served by `get_login_prefs`.
> - `[ui] bug_url`, `default_theme`, `default_language`, `poll_interval_ms`.
> - `[launch]` create_volume, config_drive, `disable_*` sources (a row's own source stays for
>   *Launch as Instance*), requires_keypair, can_set_password (new `Kind::Password`; `adminPass`
>   masked in previews), flavor_sort / flavor_sort_reverse (sorted on the server).
> - `[network]` default_dns_nameservers, enable_ipv6, fip_topology_check (ports on networks a
>   gateway router reaches), allowed_private_subnet_cidr (dialog check and `run_action`).
> - `[images]` disk_formats (Upload to Image keeps Cinder's), default_visibility (now sent by
>   *Upload to Image*, volume API 3.1).
> - `[api_access]` clouds_yaml_name / clouds_yaml_profile; downloads use `[keystone] interface`.
> - `[log] client_ip_header` (sign-in and write logs); `X-Frame-Options: DENY` and
>   `frame-ancestors 'none'` on every response (`DISALLOW_IFRAME_EMBED`, not configurable).

**Moved to the features that need them** (2026-10-26, so item 29 can close): paging settings to
*Also pending* (paging), image upload settings to item 45, the `backup` plugin, scheduler hints
and Swift's chunk size to item 5, `IMAGE_RESERVED_CUSTOM_PROPERTIES` to item 25, `POLICY_*` to
item 35, the Keystone admin settings to item 57, `WEBSSO_*`, `AVAILABLE_REGIONS`, `WEBROOT` and
the Admin dashboard's settings to *Also pending*. Each one is added with its feature, documented
in `CONFIGURATION.md` and `moonlight.conf.example`. Since then: `[session] public_url` (item 48).

**Not needed** (Django/AngularJS internals, or fixed by design): `ALLOWED_HOSTS`, `STATIC_*`,
`TEMPLATES` and loaders, `ADD_INSTALLED_APPS`, `NG_TEMPLATE_CACHE_AGE`, `js_files`,
`angular_modules`, `exceptions`, `REST_API_REQUIRED_SETTINGS`, `THEME_COLLECTION_DIR`,
`SELECTABLE_THEMES`, `LAUNCH_INSTANCE_LEGACY_ENABLED` / `NG_ENABLED`, `AUTHENTICATION_PLUGINS` /
`URLS`, `POLICY_CHECK_FUNCTION`, `ENABLE_CLIENT_TOKEN` (the token never reaches the browser),
`OPENSTACK_TOKEN_HASH_ALGORITHM`, `TOKEN_DELETION_DISABLED`, `ajax_queue_limit`,
`auto_fade_alerts`, `modal_backdrop`, `DROPDOWN_MAX_ITEMS`, `password_validator`, `user_home`,
`IMAGE_CUSTOM_PROPERTY_TITLES`, `IMAGES_LIST_FILTER_TENANTS`, `OPENSTACK_ENABLE_PASSWORD_RETRIEVE`,
`can_set_mount_point`, `OPENSTACK_INSTANCE_RETRIEVE_IP_ADDRESSES`, `enable_auto_allocated_network`,
`OPENSTACK_PROFILER`. Already equivalent: URL/domain/region, `SECRET_KEY`, `help_url`, logging,
plugin "enabled" files, `OPENSTACK_API_VERSIONS` and Neutron/Nova feature switches (detected,
item 19), themes, `OPERATION_LOG_*` (every write is logged), `OVERVIEW_DAYS_RANGE`.

## 30. *Enable SNAT* checkbox in *Edit Router*

> ✅ **Done.** `RouterRow` gained `external_network_id` and `snat` (moved from `RouterDetail`),
> filled by `mapping::routers` (tested: no key, `false`, `null` gateway, unknown network).
> `OsAction::RouterUpdate` takes `snat: Option<(network_id, bool)>` and then also sends
> `external_gateway_info: {network_id, enable_snat}`; `ops::router_items` adds the checkbox
> under *Enable Admin State* only for routers with a gateway, and sends it only when the value
> changed (dry-run tests: rename body unchanged, SNAT off). Gated by
> `needs::SNAT = ext-gw-mode + AnyRole(["admin"])`: `Capabilities.roles` holds the scoped
> token's role names (Keystone already adds implied roles), checked by the new
> `Requires::AnyRole` (tested). Neutron's policy can't be read through the API, so the role
> list mirrors its default; the server still answers 403 when a custom policy disagrees.
> Clippy clean on both targets, 208 tests pass. Browser (dev cloud, 2026-10-25): hidden for
> `admin2`, whose only role on *pokemon* is `power_user`, while `ext-gw-mode` is detected; so
> the gate works. Left: a `moonlight.conf` override of the roles for clouds with a custom
> policy (item 35), and confirming on a real cloud that a SNAT-only update keeps the
> gateway's fixed IPs and QoS policy.

**Goal.** *Edit Router* (row menu on Routers and the router detail header) shows *Name* and
*Enable Admin State*. Add an *Enable SNAT* checkbox right under *Enable Admin State*,
preselected with the router's current `external_gateway_info.enable_snat`, so operators can
turn source NAT on or off without recreating the gateway. Horizon offers the same switch in
*Set Gateway* (only with the `ext-gw-mode` extension and when policy
`update_router:external_gateway_info:enable_snat` allows it, admin-only by default).

**Implementation.**

- Model: `RouterRow` (`model.rs`) only has `external_network` as a display name; add
  `external_network_id: Option<String>` and `snat: Option<bool>` and fill them in the router
  list mapper (`server/mapping.rs`, next to `external_network`, reading `network_id` and
  `enable_snat`); extend its test with a gateway that has `enable_snat: false` and one without
  the key. The detail page builds the same `RouterRow` for `ops::router_items`
  (`pages/details.rs`), so fill it there too (`RouterDetail.snat` already exists).
- Write: extend `OsAction::RouterUpdate` with `snat: Option<bool>`. In
  `server/actions.rs::apply`, when it is `Some`, also send
  `external_gateway_info: {network_id, enable_snat}` through `router::set`
  (`ExternalGatewayInfoBuilder::enable_snat`); Neutron needs `network_id` in that dict, so
  carry the gateway network ID in the action (or a separate `RouterSetSnat { id, network_id,
  snat }` variant if that reads cleaner). Send it only when the value changed, so a plain
  rename keeps today's body. Dry-run tests: rename only (no gateway key), SNAT off (exact
  body with `network_id` and `enable_snat: false`).
- UI (`components/ops.rs::router_items`): `Field::check("snat", K::EnableSnat, current)` after
  `admin_up`, only when the router has a gateway; new i18n key *Enable SNAT* / *Habilitar
  SNAT*.
- Gating: `needs::SNAT = Requires::NetExt("ext-gw-mode")` checked with `caps()`.
- Open question: the default Neutron policy rejects `enable_snat` for non-admins (403). Show
  the box to everyone and let the error surface, or hide it for non-admins? The session does
  not keep the user's roles today, so hiding needs them read from the scoped token first. Ask
  before implementing.

## 31. Instance *Log* and *Console* tabs

> ✅ **Done** (Log, VNC/SPICE console). Decided with the user: sent only when
> `[console] enabled = true` (default off) **and** `[writes] mode = on`; dry-run shows the
> request instead, off rejects it. `server/console.rs` is the only home of these read-only
> POSTs and runs them through `Exec` (`call` is now `pub(super)`); AGENTS.md §1.3 says so.
> `api::run_instance_log(id, lines)` and `run_instance_console(id)` log user, project, client
> and mode like writes. Log: `os-getConsoleOutput` (`length` = `[console] log_length`, 35;
> none for *View Full Log*, shown in the same tab rather than a new one). Console:
> `remote-consoles` (`protocol`/`type` from `[console] type` = auto | vnc | spice; auto tries
> VNC, then SPICE when Nova refuses VNC), `os-getVNCConsole` below Nova 2.6
> (`needs::REMOTE_CONSOLE`); only for `ACTIVE` instances, in an iframe plus an *open alone*
> link; only http(s) URLs are accepted. Both are fetched with `LocalResource` when the tab
> opens (each console call makes a token). 5 tests in `console.rs` (exact bodies and paths,
> fallback, disabled, off, response parsing) plus settings tests; 213 tests pass, clippy clean
> on both targets. Not checked in the browser yet. Left: the serial console (WebSocket
> client), and confirming on the test cloud that the noVNC frame loads (the console proxy may
> refuse being framed by another origin).

**Goal.** The instance detail page has *Log* and *Console* tabs, but both only show
`K::ConsoleNotInPoc` (`pages/instance_detail.rs`, `Tab::Log | Tab::Console`). Horizon shows:

- *Log*: the last `INSTANCE_LOG_LENGTH` lines (35) of the serial console output, a *Log
  Length* box with *Go*, and *View Full Log* (the whole output as plain text in a new tab).
- *Console*: the noVNC (or SPICE / serial, per `CONSOLE_TYPE`) console in an iframe, with a
  link to open it alone ("Click here to show only console") and a hint that the keyboard needs
  a click inside first. Only for `ACTIVE` instances; others show a message.

**Implementation.**

- Both are POSTs to Nova `…/action` that read without changing the server, which AGENTS.md
  §1.1 and §1.3 forbid today. They need a narrow allow-list, decided with the user before
  coding: a read-only `server/console.rs` (not `fetch.rs`, not `OsAction`) with exactly these
  two calls, behind a `[console] enabled = true` switch in `moonlight.conf` (default off) and
  logged like writes. Update AGENTS.md §1 and `server/settings.rs` with it.
- Log: `server::os_get_console_output` (body `{"os-getConsoleOutput": {"length": N}}`;
  `length: None` for the full log). `#[server] get_instance_log(id, lines: Option<u32>)` in
  `api.rs`; the tab renders a `<pre>` with a length input (default 35, `[console]
  log_length`) and a *View Full Log* link to a plain-text route
  (`/project/instances/{id}/log`). Refresh on demand, not by polling.
- Console: `server::remote_console::create_26` (`POST /servers/{id}/remote-consoles`, Nova
  2.6+, protocol `vnc`/`spice`/`serial`, type `novnc`/`spice-html5`/`serial`) with
  `os_get_vncconsole_21` as the fallback below 2.6; gate with `needs::REMOTE_CONSOLE =
  ComputeMv(2, 6)` and the detected `compute_mv`. `[console] type = auto | vnc | spice |
  serial` mirrors `CONSOLE_TYPE`. The tab embeds the returned URL in an `<iframe>` (our CSP
  sets only `frame-ancestors`, so framing noVNC works) plus an *open alone* link. Fetch the URL
  only when the tab is opened: each call creates a console token on the cloud.
- Tests: request method, path and exact body for both calls (pure builders, no network),
  the microversion fallback, and the switch off returning `AppError::invalid`.
- Open questions: may these POSTs be sent while `[writes] mode = dry-run` (they don't change
  the server, but they do create console tokens), and should the switch default to off? The
  serial console needs a WebSocket client in the page: leave it for later unless asked.

## 32. Flavor popover: drop the *Ephemeral Disk* and *Swap* rows

> ✅ **Done.** The two `row(…)` lines are gone from `widgets::FlavorPopover`; it shows *ID*,
> *VCPUs*, *RAM* and *Root Disk* (checked with a real hover on the dev server). Fields,
> mapping, test and i18n keys stay for the instance *Specs* card, which still shows both rows:
> the open question below was not answered, so that card is unchanged. Clippy clean on both
> targets, 213 tests pass.

**Goal.** The flavor popover on the Instances list (item 9) shows *Ephemeral Disk* and *Swap*
rows, which are `-` or `0` on most flavors and only lengthen the card. Remove both rows from
the popover; vCPUs, RAM, root disk and the rest stay.

**Implementation.**

- `components/widgets.rs::FlavorPopover`: delete the `row(K::Ephemeral, …)` and
  `row(K::Swap, …)` lines.
- Keep `FlavorInfo.ephemeral_gb` / `swap_mb`, their mapping (`mapping::flavor_of`) and the test
  in `mapping.rs`: the instance detail *Specs* card (`pages/instance_detail.rs`) still uses
  them, and so do the `K::Ephemeral` / `K::Swap` i18n keys.
- Open question: should the *Specs* card on the instance detail page drop them too? If so,
  also remove the two fields, their mapping and test assertions, and the two i18n keys
  (`K::EphemeralDisk` stays: the flavor picker uses it).
- Verify in the browser: hover a flavor on the Instances list.

## 33. *Edit Image* with Horizon's fields

> ✅ **Done.** `ops::edit_image_dialog`: name, description, kernel and ramdisk IDs (only for
> `ami`/`aki`/`ari`), architecture, minimum disk and RAM, disk format (only while `queued`,
> from `[images] disk_formats`), visibility (`public` only with `needs::PUBLICIZE_IMAGE` =
> admin role, or when the image already is public) and protected. `Image` gained
> `min_disk_gb` / `min_ram_mb` (moved from `ImageDetail`) and `property()`.
> `OsAction::ImageUpdate { id, changes: ImageChanges }` carries only what changed; the pure
> `actions::image_patch` builds the JSON-Patch (`replace` for core fields, integers as
> integers, `add` / `remove` for the properties) and refuses an empty patch;
> `ImageSetProperties` (item 25) reuses it. No *Metadata* step: *Update Metadata* (item 25)
> covers it. Tests: `image_update_is_a_json_patch` (exact bodies), `edit_image_sends_only_the_changes`
> (fields per image type, `public` gating, diff). Not checked in the browser: neither *pokemon*
> nor *admin2* owns an image on the dev cloud. Left: `public` for custom roles (item 35).

**Goal.** *Edit Image* exists (row menu on Images and the image detail header, only for images
the project owns) but only edits *Name*, *Visibility* and *Protected*. Horizon's *Edit Image*
has two steps:

- *Image Detail*: *Image Name*, *Image Description*; *Image Requirements*: *Kernel ID* and
  *Ramdisk ID* (only for `aki`/`ari`/`ami` images), *Architecture*, *Minimum Disk (GB)*,
  *Minimum RAM (MB)*; *Image Sharing*: *Visibility*, *Protected*. *Format* is shown but
  only editable while the image is still `queued`.
- *Metadata*: the custom properties editor, which is item 25: link the two rather than build
  it twice.

**Implementation.**

- Data: the dialog is built from the list row (`model::Image`, `ops::image_items`), which lacks
  these fields. Add `description`, `architecture`, `kernel_id`, `ramdisk_id`, `min_disk_gb`,
  `min_ram_mb` to `Image` and fill them in `mapping::images` (`ImageDetail` already has
  `min_disk_gb` / `min_ram_mb`: move them into `Image` so both pages share them). Extend the
  mapping test with an image that lacks the optional properties.
- Write: replace the fixed patch in `server/actions.rs` (`ImageUpdate`, three `replace` ops) with
  one built from the changes only: `OsAction::ImageUpdate { id, changes: ImageChanges }` where
  every field is an `Option` (unchanged = `None`), and an empty text means "remove". Glance's
  JSON-Patch needs `add` for a property that may not exist (`add` replaces an existing one),
  `remove` to clear one (fails when it is absent, so only when the image had it), and
  `replace` for core fields (`name`, `visibility`, `protected`, `min_disk`, `min_ram`). Keep a
  pure `fn image_patch(old, changes) -> Vec<Value>` and test it: rename only (one op), clear
  the description (`remove`), set a new architecture (`add`), min disk to an integer (not a
  string), invalid visibility rejected, nothing changed (no request at all).
- UI: `Dialog::form` → a two-step dialog (`Step`, as *Create Network* does) or one form with
  section titles; kernel/ramdisk only when `disk_format` is `ami`/`aki`/`ari`; `Field::number`
  with `min 0` for the minimums; *Format* a select of `[images] disk_formats` only while the
  status is `queued`, else read-only text. New i18n keys in both languages (*Image
  Description*, *Architecture*, *Kernel ID*, *Ramdisk ID*, *Minimum Disk (GB)*, *Minimum RAM
  (MB)*, *Image Requirements*, *Image Sharing*).
- Visibility: only an admin may make an image `public` under Glance's default policy
  (`publicize_image`); offer `public` only with the admin role (`Requires::AnyRole`, item 30)
  and keep it when the image already is public.
- Open question: is the *Metadata* step needed in this item, or is item 25 enough later?

## 34. Hide actions on resources the project doesn't own

> ✅ **Done.** `NetworkRow.owned` (network `project_id` = the session's, from
> `mapping::networks(…, project_id)`); `ops::network_items(lang, &NetworkRow)` returns nothing
> for other projects' networks, in the list and the detail header, and the network detail
> hides *Create Subnet* and the subnet actions there (ports keep theirs: they may be the
> project's own ports on a shared network). `ops::security_group_items` gives `default` only
> *Manage Rules*. Tests: `networks_and_routers_resolve_names` (owned) and
> `foreign_networks_and_the_default_group_are_read_only`. Browser (dev cloud): no actions on
> the external and shared networks, *Manage Rules* only on `default`. Left: the subnet detail
> page of a foreign network still shows its menu (`SubnetRow` has no owner yet).

Found in the Horizon review of 2026-10-25 (dev cloud, project *pokemon*, user `admin2`).

**Goal.** Moonlight offers write actions that Horizon hides because the project can't perform
them:

- *Networks*: every row gets *Edit Network* (and its menu), including the external
  (`public-alh1`, `public-mad1`, `megatros-external1-alh1`, `public-dummy-alh1`) and shared
  (`t2c-cert-al-service-tenant`) networks of other projects. Horizon shows no actions on
  those rows, and *Edit Network*, *Create Subnet* and *Delete Network* only on the project's own
  (`route-1`, `route-2`).
- *Security Groups*: the `default` group gets *Edit Security Group* and *Delete Security
  Group*; Horizon only offers *Manage Rules* there (Neutron refuses to delete `default`, and
  renaming it is pointless).

**Implementation.** `ops::network_items(lang, id, name, admin_up)` has no owner: add the
network's `project_id` (or an `owned: bool` computed in `mapping::networks` from the session's
project, like `Image.owned_by_project`) to `NetworkRow` and return no items for foreign
networks; check the network detail header too, and that the subnet and port actions follow
the same rule. `ops::security_group_items`: no edit/delete when `name == "default"`. Tests
next to `allowed_actions_follow_horizon` for both. Then check the other panels for the same
pattern (subnets of shared networks, QoS policies already do it right: shared ones show no
actions).

## 35. Policy-aware actions: the cloud's custom roles

Found in the Horizon review of 2026-10-25.

**Goal.** On the dev cloud `admin2` has only the role `power_user` on *pokemon* (detected by
item 30's `Capabilities.roles`), not `admin`. Two consequences:

- Item 30's *Enable SNAT* never shows (`needs::SNAT` wants `admin`), although `ext-gw-mode`
  is there. Whether `power_user` may change `enable_snat` depends on the cloud's Neutron
  policy, which the API doesn't expose.
- Horizon offers *Edit Image* and *Delete Image* on public images owned by another project
  (Debian-13, RHEL-9, …; not on the protected `cirros-0.6.3`), so Horizon's policy files grant
  that to this role. Moonlight only offers them when `owned_by_project`.

Horizon decides with copies of the services' policy files (`POLICY_FILES`, `POLICY_DIRS`,
item 29); Moonlight mirrors the upstream defaults in `model::needs`.

**Implementation.**

- A `[policy]` section in `moonlight.conf` mapping the few rules Moonlight checks to roles,
  e.g. `snat = ["admin", "power_user"]`, `modify_public_image = ["admin"]`,
  `publicize_image = ["admin"]` (item 33), defaulting to the upstream rules. `needs` constants
  that depend on roles become functions of the configured lists (`Requires::AnyRole` takes a
  `&'static` slice today: keep the roles in `Capabilities` and check them against
  `prefs().policy` instead).
- Images: show *Edit Image* / *Delete Image* when owned, or when the role list allows it;
  `protected` images keep *Delete* hidden (Horizon does). *Update Metadata* (item 25,
  `ops::image_items`, today only when `owned_by_project`) follows the same rule as *Edit
  Image*; a Glance 403 already shows `MetadataForbidden`.
- The server still decides: a 403 surfaces as today.
- Open questions: which rules does this cloud grant `power_user` (ask the cloud operators, or
  read the policy files Horizon is deployed with)? Is reading oslo.policy YAML files worth it
  instead of a hand-written mapping?

**Settings that come with it** (moved from item 29): `POLICY_FILES` / `POLICY_DIRS`, if the
cloud's policies are read from files rather than detected.

## 36. Usage Summary lists deleted instances

> ✅ **Done.** `UsageSummary::listed()` keeps the servers whose state isn't `terminated`; the
> Overview table and the CSV rows use it, the period totals still count every server (as
> Horizon). Tested in `totals_count_only_active_servers_where_horizon_does`. Browser (dev cloud,
> *pokemon*): 5 rows, the same as Horizon, instead of 13.

Found in the Horizon review of 2026-10-25.

**Goal.** The Overview *Usage Summary* table shows 13 rows on *pokemon* (`dodrio-1..3`,
`dialga-1..2`, `test01`, `mewtwo` twice, …), Horizon 5: only the instances that still exist.
The extra rows are servers that ended during the period; `mewtwo` appears twice.

**Implementation.** `mapping::server_usages` (`server/mapping.rs`, fed by `fetch.rs` from
`os-simple-tenant-usage`) keeps every `server_usages` entry. Horizon lists only those with
`ended_at == null` in the table (the totals still count all of them). Filter in the mapper or
the page, keep the period totals unchanged, and extend
`server_usages_fall_back_to_the_id_and_tolerate_missing_fields` with an ended entry. The CSV
download should match Horizon too (it lists the same rows as the table).

## 37. Instance detail: rules per security group, *Image ID*

> ✅ **Done.** `fetch::instance` now also lists the project's security groups (8 calls in
> parallel, `soft`), and `mapping::instance_detail` fills
> `InstanceDetail.security_group_rules` (one entry per group name that matches exactly one
> listed group; remote groups by name through `describe_rule_with`). The *Security Groups*
> card lists each group with its rules. *Image ID* (copyable) comes from `boot_image_id`:
> Nova's `image.id`, else the boot volume's `volume_image_metadata.image_id` (`boot_volume`
> is now shared with `boot_image`). Tested in the instance detail mapping test. Browser:
> `test-win202-JL` shows the 4 rules of `default` and the image ID Horizon shows.

Found in the Horizon review of 2026-10-25.

**Goal.** On the instance detail *Overview*, Horizon lists the rules under each security group
(`default`: "ALLOW IPv4 from default", "ALLOW IPv6 to ::/0", …); Moonlight shows only the
group name. Horizon also shows *Image ID* next to *Image Name* (also for volume-booted
instances, from the boot volume's image metadata); Moonlight has no *Image ID*.

**Implementation.** `pages/instance_detail.rs::overview`; the rules come from the security
groups the fetcher may already load (`fetch::instance`), else one `security_groups` GET
filtered by name for the project, summarised with `mapping::describe_rule` (already used by
the Security Groups detail). The image ID is in `Instance.image_source` / the volume's
`volume_image_metadata.image_id` (item 1).

## 38. Small gaps from the Horizon review (October 2026)

> ✅ **Done**, all but the *Networks* point, which stays as it is by choice. *Server Groups*:
> *ID* column and humanised policies (`i18n::server_group_policy`). *Security Groups*:
> *Shared* column (`SecurityGroupRow.shared`). *Images*: visibility capitalised
> (`i18n::image_visibility`, list and detail); the detail shows *Description* (out of the
> custom properties), *Virtual Size* and *Tags*. *Instances*: `allowed` puts *Start Instance*
> first when it is offered (shut-off instances). *Router detail*: each external fixed IP with
> its subnet (`RouterDetail.gateway_ips` is `(ip, subnet_id)`). *API Access*: services without
> an endpoint in the region are listed with `-` (`os::catalog_listing`; detection keeps using
> `os::catalog`), so *Ceilometer* shows as in Horizon (12 rows). Tests for each mapper/helper
> (`visibility_and_policies_read_like_horizon`, `allowed_actions_follow_horizon`,
> `api_access_also_lists_services_without_an_endpoint_here`). Browser (dev cloud, project
> *admin2*): API Access 12 rows, image detail and list, security groups *Shared*; server
> groups and routers had no rows in that project.

Found in the Horizon review of 2026-10-25 (dev cloud, project *pokemon*). Each is small; take
them in any order, or fold the ones you like into item 27.

- *Server Groups*: Horizon has an *ID* column and shows the policy humanised (*Soft Anti
  Affinity*); Moonlight shows `soft-anti-affinity` (and a *Members* count Horizon lacks:
  keep it).
- *Security Groups*: Horizon has a *Shared* column (RBAC-shared groups); Moonlight has
  *Rules* instead. Add *Shared*.
- *Images*: *Visibility* is lowercase (`public`) where Horizon capitalises it (*Public*), in
  the list and the detail. The detail lacks *Virtual Size*, *Tags* and a proper *Description*
  row (it shows under *Custom Properties* as `description`).
- *Instances*: the split button's first action for a `SHUTOFF` instance is *Create Snapshot*;
  Horizon puts *Start Instance* first (the action most likely wanted). Order the menu by
  state like Horizon in `ops::allowed`.
- *Router detail*: Horizon shows the subnet of each external fixed IP; Moonlight only the IP.
- *API Access*: Horizon lists 12 services, Moonlight 11: *Ceilometer* is missing. Find out
  why (no endpoint in `es-central-1`, or a service type the catalog filter drops) and list it
  like Horizon.
- *Networks*: for external networks whose subnets the project can't see, Moonlight shows the
  subnet IDs, Horizon an empty cell. Keep the IDs (Moonlight's convention), unless they turn
  out to confuse users.
- Not gaps (checked): Key Pairs, Floating IPs, Trunks, Load Balancers and Routers lists match;
  Volumes and Snapshots lack Horizon's *Group* / *Group Snapshot* columns, which come with the
  Cinder Groups panels (item 5); Network QoS also lists other projects' shared policies
  (without actions), which Horizon doesn't: fine.

## 39. Language and Theme inside the user menu

> ✅ **Done.** The header keeps region, project and user. `layout::UserMenu` has a *Theme*
> segmented control (`ThemeSegments`: three joined icon buttons, half circle · sun · moon for
> Auto · Light · Dark, named by `aria-label` and `title`; `role="radiogroup"`, `.segmented`)
> and a *Language* dropdown nested in it (`<details class="submenu">`, current language on the
> right, closes after a pick); picks apply at once
> and leave the user menu open. *System* is now labelled *Auto* / *Automático* everywhere and
> `Theme::ALL` is ordered Auto, Light, Dark. The login page keeps `ThemeMenu` /
> `LanguageMenu`, now built on the shared `ThemeChoices` / `LanguageChoices`. Clippy clean on
> both targets, 213 tests pass. Browser (dev server): Dark and Español apply without reload,
> `ml_theme` / `ml_lang` cookies set, after reload SSR renders `data-theme="dark"` and
> `lang="es-ES"` with no console warnings. Not checked: the login page (would need signing
> out) and keyboard navigation inside the submenu.

**Goal.** The header has three menus side by side: Theme (icon only), Language (globe + `EN`)
and the user (`admin2`). Move Language and Theme into the user dropdown so the header only
keeps region, project and user. Horizon does the same: its user menu holds *Settings* (where
the language is chosen), *Help*, the theme picker and *Sign Out*. The choice must stay
one click away and be applied instantly, as today.

**Implementation.**

- `components/layout.rs`: `Layout` renders `<ThemeMenu/>` and `<LanguageMenu/>` before
  `UserMenu`; drop them there and add two radio groups to `UserMenu`'s `menu-list`, after the
  domain line and before the links: a *Theme* group (`Theme::ALL`, `role="menuitemradio"`,
  `aria-checked`, `theme.set(t)`, as `ThemeMenu` does) and a *Language* group (`Lang::ALL`,
  `native_name()`, `lang=` attribute, `i18n.set(l)`). Use `role="group"` with
  `aria-labelledby` on a small heading (`K::Theme`, `K::Language`, both exist) and separators
  (`menu-sep`) between groups and before *Sign Out*. Close the menu after a pick (a
  `NodeRef<Details>` for `UserMenu`, like the other menus' `close(details)`), or keep it open
  so both can be changed at once: decide while doing it.
- Keep `ThemeMenu` / `LanguageMenu` as they are for the **login page** (`pages/login.rs`), which
  has no user menu. Extract the two lists into small shared components (`ThemeChoices`,
  `LanguageChoices`) used by both, so the markup and behaviour stay in one place.
- CSS (`style/main.css`): group headings and the selected mark inside `.menu-list`; tokens
  only, no hex.
- Show the current values in the user menu summary? Probably not: the summary stays
  `admin2 ▾`. The header gets narrower, which helps phone widths.
- Verify in the browser: both choices apply without reload, the `ml_lang` / `ml_theme` cookies
  are set, hydration still matches after reload (SSR renders the same menu), keyboard
  navigation reaches the radios, and the login page is unchanged.

## 40. *Any Availability Zone* instead of *— None —* in zone selectors

> ✅ **Done.** `Field::empty_label` → `Combo`'s `empty_label` prop replaces *— None —*; set
> to `K::AnyAvailabilityZone` on the zone fields of *Launch Instance*, *Unshelve Instance* and
> *Create Volume*. Requests are unchanged. Tested in `zone_selectors_offer_any_zone`; browser:
> the launch wizard shows *Any Availability Zone*, `alh1`, `mad1`.

**Goal.** In *Launch Instance* (step *Details*), the *Availability Zone* dropdown starts with
*— None —* (`K::NoneOption`, the generic empty choice of every optional select). For a zone,
an empty value means "let the scheduler pick", so the label should say so: *Any Availability
Zone* / *Cualquier zona de disponibilidad*, as Horizon's launch wizard does. The same applies
to the other zone selectors: *Unshelve Instance* (keeps the instance's zone when empty: there
*Any* is also right, and its help `K::AnyZone` explains it) and *Create Volume*.

**Implementation.**

- `components/select.rs::Combo` inserts `Choice::new("", i18n.t(K::NoneOption))` for
  non-required fields; give it an optional `empty_label: Option<K>` prop that replaces
  `K::NoneOption`.
- `components/dialog.rs`: a `Field::empty_label(K)` builder storing it on `Field` (next to
  `help`), passed to `<Combo>` in `FieldView`'s `Kind::Select` arm.
- `components/ops.rs`: `.empty_label(K::AnyZone…)` on the three `"az"` fields (launch, unshelve,
  create volume). New i18n key `AnyAvailabilityZone` in both languages (`AnyZone` is the
  unshelve help text, keep it).
- Nothing changes in the request: an empty value still omits `availability_zone`.
- Test: the launch dialog's `az` field carries the label (`ops` tests), and the select shows it
  in the browser.

## 41. Launch: load the customization script from a file

> ✅ **Done.** `Field::from_file(limit)` adds `dialog::FileLoad` (*Load from a file*, a hidden
> `<input type="file">`) above the text area and `Script size: N bytes of 16.00 KB` under it
> (red past the limit). The file is read in the browser (`File::text()` through
> `wasm-bindgen-futures`, new optional dependency of `hydrate`; `web-sys` gained
> `HtmlInputElement`, `FileList`, `File`, `Blob`); too large files are refused before reading
> and non-text files after (`dialog::file_problem`). Used on the launch dialog's `user_data`.
> Tests: `script_files_are_checked_before_filling_the_field`. Browser: a 33-byte cloud-init
> file filled the text area and the counter; the dialog was cancelled.

**Goal.** *Launch Instance* → *Configuration* has a *Customization Script* text area
(`user_data`, 16 KB limit `MAX_USER_DATA_BYTES`, checked in the dialog and on the server), but
the script can only be typed or pasted. Horizon also offers *Load Customization Script from a
file*: a file picker whose contents fill the text area (still editable, and *Script size: N
bytes of 16.00 KB* under it). Add the same; the file never leaves the browser except as the
`user_data` the launch already sends.

**Implementation.**

- Dialog engine (`components/dialog.rs`): an option on `Field::area`, e.g.
  `.load_from_file(accept)`, rendering an `<input type="file">` (`accept=".sh,.yaml,.yml,
  .txt,.cfg,text/*"` or none, as Horizon) next to the label. On `change`, read the first file
  with `File::text()` (a `Promise`, awaited with `wasm_bindgen_futures::spawn_local` +
  `JsFuture`), and `put()` its text into the field, as typing would. Reject before reading
  when `file.size() > MAX_USER_DATA_BYTES` (`K::UserDataTooBig`, already translated), and
  show non-UTF-8 files as an error instead of mangled text.
- `Cargo.toml`: `web-sys` features `HtmlInputElement`, `FileList`, `File`, `Blob`; add
  `wasm-bindgen-futures` (and `js-sys` if needed) as regular dependencies, like `web-sys`
  (AGENTS.md: code compiled for both targets). The reading code is `#[cfg(feature =
  "hydrate")]`; SSR renders only the input.
- A live byte counter under the text area (`N / 16 KB`), shared with paste/typing, so the
  limit is visible before submitting.
- `components/ops.rs`: `.load_from_file(…)` on the `user_data` field of the launch dialog
  (line ~112). i18n keys: *Load from a file* / *Cargar desde un archivo*, *Script size* /
  *Tamaño del script*.
- Tests: the byte counting and size check as a pure function (`ops` tests); the file reading
  itself in the browser only (dry-run: pick a file, see the text area filled and the preview
  body carrying the base64 `user_data`).

## 42. Instance menu: *Edit Security Groups* (all the instance's ports)

> ✅ **Done.** `Instance.security_groups` (names, deduplicated: Nova repeats a group per port;
> `mapping::server_security_groups`, shared with the detail mapper). `Op::EditSecurityGroups`
> after *Edit Instance*, not for shelved instances (`ops::allowed`) and hidden when the
> instance has no addresses: Nova would answer 400 (the open question, settled that way).
> `ops::security_groups_dialog`: checkbox list of `ChoiceSet::SecurityGroups` (names) preset
> with the current ones; the action is the difference (`list_diff`), *Nothing changed*
> refused. `OsAction::ServerSetSecurityGroups { id, add, remove }` sends one
> `addSecurityGroup` / `removeSecurityGroup` per group and records each like a batch item
> (`+ web`, `- default`), so a failure names its group. Tests: the dialog's diff and its
> visibility, and the exact Nova bodies in dry-run. The filtered picker is item 43.
> Browser (ttcp-dev, 2026-10-26): the item follows *Edit Instance*, is hidden on an instance
> without addresses, presets `default`, and *Save* unchanged says *Nothing changed.* Not
> submitted.

**Goal.** The instance row menu and detail header have *Edit Port Security Groups* (one port at
a time) but no *Edit Security Groups* for the instance as a whole. Horizon puts that in *Edit
Instance* → *Security Groups* tab: the groups of the server, added or removed on every port at
once through Nova. Offer it as its own *Edit Security Groups* item next to *Edit Instance*
(Horizon's order: *Edit Instance*, *Update Metadata*, *Edit Port Security Groups*).

**Implementation.**

- Data: `Instance` has no security groups; add `security_groups: Vec<String>` (names, from
  the server's `security_groups`, deduplicated, as `InstanceDetail.security_groups` does in
  `mapping::instance_detail`) to the list row.
- Write: `OsAction::ServerSetSecurityGroups { id, add: Vec<String>, remove: Vec<String> }` →
  one Nova action per group, `POST /servers/{id}/action` with `{"addSecurityGroup": {"name":
  …}}` / `{"removeSecurityGroup": {"name": …}}` (SDK `compute::v2::server::add_security_group`
  / `remove_security_group`, both present in 0.22.5), run like a batch so a
  failure names the group. Nova takes groups **by name** (`ChoiceSet::SecurityGroups`, not
  `SecurityGroupIds`). Dry-run test with the exact bodies; refuse an empty change.
- UI: `Op::EditSecurityGroups` in `ops::allowed` (not while a task runs; Horizon also hides it
  for shelved instances, whose ports are unbound) and its dialog: `Field::pick_many`
  (`Source::Set(ChoiceSet::SecurityGroups)`) preset with the current names; the change is the
  diff. New i18n key *Edit Security Groups* / *Editar grupos de seguridad*.
- Open question: an instance without ports (or with port security off) can't take groups;
  Nova answers 400. Show the error, or hide the item when the instance has no addresses?

## 43. Security group pickers with a filter (*Allocated / Available*) when editing

> ✅ **Done** (asked for during item 42). *Edit Security Groups* and *Edit Port Security
> Groups* use `Field::pick_many` (the launch wizard's *Allocated / Available* tables): the
> filter matches name and description (the choices already carried the description cell),
> and the dialogs size themselves as medium. The port dialog's `on_change` preset works
> unchanged (same comma list). Also asked: the two menu items now sit together, *Edit Port
> Security Groups* right after *Edit Security Groups* (`ops::allowed`; still offered on
> shelved instances, unlike the instance-wide one). Test:
> `security_group_editors_are_filtered_tables_next_to_each_other`. Browser (ttcp-dev): the
> port's group preset under *Allocated*, filtering `ssh` leaves *Allow-SSH*; not submitted.

**Goal.** *Edit Port Security Groups* uses a checkbox list (`Field::multi`) of every security
group: long and unfiltered on projects with many groups. The launch wizard already has
Horizon's *Allocated / Available* tables with a search box (`Field::pick_many`, `PickTable`
in `components/pick.rs`). Use the same picker in *Edit Port Security Groups* and in item 42's
*Edit Security Groups*, so both dialogs look and filter alike: allocated groups on top (in
order), available ones below with a filter, a count, and the group descriptions as a second
cell.

**Implementation.**

- `ops::port_security_groups_dialog`: replace `Field::multi("groups", …)` with
  `Field::pick_many("groups", …, Source::Set(ChoiceSet::SecurityGroupIds))`; the `on_change`
  that presets the port's groups keeps working (the value is the same comma list).
- `fetch::choices` for `SecurityGroups` / `SecurityGroupIds`: make sure each `Choice` has the
  description as a cell, so the tables show it and the filter matches it (`pick::matches`).
- The dialog gets taller: check its size class (item 28) and that the picker scrolls inside it.
- Tests: the dialog's field kind and that a preset port fills *Allocated* (ops tests);
  browser check with a project that has several groups.

## 44. ✅ Instance *Interfaces* tab: *Edit Security Groups* and *Edit Port*, as Horizon

> ✅ **Done (2026-10-26).** `fetch::instance` adds `ports?device_id=` (9 parallel GETs, `soft`)
> and `mapping::port_settings` fills `Interface::port: Option<PortSettings>` (name, admin
> state, VNIC type, port security defaulting to `true`, group IDs); `PortRow` gained
> `port_security` and `security_group_ids` (`PortRow::settings`). `OsAction::PortUpdate` has
> only `Option` fields; `PortSettings::changes` builds it with what changed (groups compared
> as a set) and clears the groups when port security goes off, in the same PUT. One
> `ops::edit_port_dialog` (wizard, *Info* and *Security Groups*, medium size) serves the port
> page, the ports tables and `ops::instance_interface_items` (*Edit Security Groups* opens on
> the second tab through the new `Dialog::starting_at`; then *Edit Port*, *Detach Interface*
> last). The groups step is `enabled_if` port security is on. VNIC type needs
> `needs::PORT_BINDING` (`binding`), port security `needs::PORT_SECURITY` (`port-security`);
> an unlisted current type stays selectable. The tab now shows *Name* (else the start of the
> ID, linked) and *Admin State*. Tests: `port_changes_send_only_what_changed`,
> `port_security_off_clears_the_groups_in_the_same_request`,
> `port_update_sends_only_the_changed_fields` (rename only, port security off, groups only,
> VNIC type, unknown type refused) and the merged port in
> `detail_joins_volumes_interfaces_actions_and_fault`. Browser (ttcp-dev, dialogs opened, never
> submitted): *test-l4-24c* loads in 9 API · 0.60 s, *Edit Security Groups* opens on its tab
> with `default` allocated, unticking *Port Security* disables that tab, the port page opens on
> *Info*. The open question is settled: the instance menu keeps *Edit Port Security Groups*
> too. Also fixed: hovering the current wizard step made its label unreadable (all wizards).

**Goal.** The instance detail *Interfaces* tab only has *Detach Interface* per row. Horizon
has, per port, *Edit Security Groups* (the split button) and *Edit Port* (in the menu), both
opening one *Edit Port* dialog with two tabs:

- *Info*: *Name*, *Enable Admin State*, *Binding: VNIC Type* (*Normal*, *Direct*, *MacVTap*,
  …), *Port Security* (turning it off clears the port's groups; Horizon explains that in the
  side help);
- *Security Groups*: the port's groups in an *Allocated / Available* picker (item 43), shown only
  while *Port Security* is on.

*Edit Security Groups* opens it on the second tab. Keep *Detach Interface* last and red. The
tab also shows *Name* and *Admin State* columns in Horizon (Moonlight shows *Port ID*).

**Implementation.**

- Data: `Interface` (from `os-interface`) has no port name, admin state, VNIC type, port
  security or groups. Fetch the server's ports in `fetch::instance` (`ports?device_id={id}`,
  one more parallel GET, `soft`) and merge them in `mapping::instance_detail`: add `name`,
  `admin_up`, `vnic_type`, `port_security`, `security_group_ids` to `Interface`. Test with
  API-shaped JSON (port security missing → `true`).
- Write: extend `OsAction::PortUpdate` with `vnic_type: Option<String>`,
  `port_security: Option<bool>`, `security_group_ids: Option<Vec<String>>` (only changes are
  sent; port security off sends `security_groups: []` in the same PUT, as Neutron requires).
  Dry-run tests for: rename only, port security off, groups only.
- UI: `Dialog` steps as tabs (as *Launch Instance*), `initial_step` for *Edit Security
  Groups*; `Field::when("port_security", "true")` on the picker. Reuse the dialog from the
  port detail page (`ops::port_items`, which today only edits name and admin state).
- Gating: `binding:vnic_type` needs the `binding` extension; *Port Security* needs
  `port-security` (`model::needs`).
- Open question: Horizon lists *Edit Port Security Groups* in the instance menu too; keep both
  entry points?

## 45. *Create Image* (from a file or a URL)

> 🔗 **Issue:** [#2]
>
> 🚧 **In progress.** All four slices are built and a `proxy` upload works against the test
> cloud. What is left is a real `direct` upload, which needs CORS on a Glance (see the end of
> this note).
>
> `OsAction::ImageCreate(ImageSpec)` (`model.rs`) creates the record (`POST /v2/images`, typed
> SDK request) and, when the spec has a URL, the `web-download` import (`actions.rs`; formats,
> visibility and URL checked before sending; custom properties may not be Glance core keys).
> Dry-run test `image_create_records_then_imports_the_url` asserts both requests, and that
> without a URL only the record is sent. Dialog: `ops::create_image_dialog` (name,
> description, *Image Source*, URL, format, kernel/ramdisk for AMI, architecture, minimum disk
> and RAM, visibility, protected, metadata). The Images list polls `queued` / `saving` /
> `importing` rows (`widgets::image_busy`, tested) and shows the busy bar.
>
> **Settings gate the source, not the button.** *Create Image* is always on the Images list,
> as in Horizon. `[images] allow_location` (`IMAGES_ALLOW_LOCATION`, default `false`) only
> decides whether *URL* is offered in the dialog's *Image Source*; the other choice, *None*,
> creates the record alone (a `queued` image to fill by other means). Slice 3 adds *File* the
> same way, gated by `[images] upload`. A first version hid the whole button behind
> `allow_location`; that was wrong. Documented in `CONFIGURATION.md` and
> `moonlight.conf.example`.
>
> **Gaps closed.** `os::capabilities` reads `GET /v2/info/import` into
> `Capabilities::image_import` (fails open when unreadable); `needs::IMAGE_URL_IMPORT`
> (`Requires::ImageImport("web-download")`) hides *URL* in the dialog when Glance has the method
> off. `Image::import_failed` (from `os_glance_failed_import`) shows an *Import failed* pill,
> with Glance's reason as its tooltip, in the Images list's status column. Tests:
> `url_import_needs_glance_web_download`, `failed_import_reason_comes_from_os_glance_failed_import`,
> `import_methods_read_glances_wrapped_list`.
>
> **Slice 3 built (File upload).**
> `[images] upload = off | proxy | direct` (default `off`) and `max_upload_gb` (default 20;
> `prefs::ImagePrefs`, validated in `settings.rs`, documented in `CONFIGURATION.md` and
> `moonlight.conf.example`). The dialog (`ops::create_image_form`) offers *File* under *Image
> Source* when the mode isn't `off`, with a file field (`Kind::File`, `components/upload.rs`):
> picking a file preselects its disk format and a file over the limit is refused on the field
> and again in `build`. `OsAction::ImageCreate` carries `ImageSpec::upload_bytes`; `apply` creates
> the record and `Exec::note_upload` previews the streamed `PUT` (dry-run test
> `image_create_records_then_imports_the_url`). `ActionOutcome::created` returns the new ID, and
> the dialog then uploads with an `XMLHttpRequest` (progress bar, *Cancel upload*); a failed or
> cancelled upload shows *The upload did not finish* with *Delete Image* for the `queued` record.
>
> - `proxy`: `PUT /upload/image/{id}` (`server/upload.rs::proxy`, registered in `main.rs`) checks the
>   session, mode, `[writes] mode = on`, `Content-Length` and the limit, and streams the body to
>   `PUT /v2/images/{id}/file` with a `reqwest` client that honours `[tls]`. Nothing is buffered.
> - `direct`: server fn `get_image_upload_ticket` makes the same checks and returns Glance's URL
>   plus the scoped token (`UploadTicket`, its `Debug` hides the token); the browser `PUT`s to
>   Glance, which needs CORS (`[cors] allowed_origin`, `PUT`, `X-Auth-Token`, `Content-Type`).
> - AGENTS.md §1.2/§1.3 now name this write outside `Exec` and the token handed to the page.
>
> **Dialog layout.** *Create Image* is a wizard like Horizon's: *Image Details* and a
> *Metadata* step (the plain key/value editor; the catalog picker belongs to item 25).
> `Field::half` (`.wizard-pane > .field.half`) pairs *Image Source* with its *URL* / *File*,
> *Disk Format* with *Architecture* (its hint is a tooltip), *Minimum Disk* with *Minimum RAM*,
> and *Visibility* with *Protected*; *Description* has two rows (`Field::rows`). The first step
> is 648 px tall, so it fits a 729 px window without scrolling (an AMI image adds its kernel and
> ramdisk rows). The file field is an in-page *Choose a file…* button (`.file-load`), not the
> browser's native control. Seen in the browser in dry-run, not submitted.
>
> **Dialog capped to the window** (2026-11-05). Every `.modal` is a flex column with
> `max-height: calc(94dvh - 16px)` (the backdrop's padding); in a form only `.wizard-pane`
> scrolls, so the title and Cancel / Next / submit stay in view. The wizard's steady
> `min-height` becomes `min(380px, 100dvh - 240px)`. Seen in the browser at 1280×600 (pane
> 414 px with 520 px of content, footer at 562 of 598 px, page not scrolling) and 400×640.
> Popups in dialogs (combos, tooltips) are `position: fixed`, so the scroll box doesn't clip
> them.
>
> **Real upload, `proxy`** (2026-11-05, test cloud, `[writes] mode = on`, user approved): a
> 1 MiB raw file built in the page, *File* source. `POST /v2/images`, then
> `PUT /upload/image/{id}` → 204; the row reached *Active* (RAW, 1 MB) and was then deleted with
> *Delete Image*. `moonlight.conf` went back to `upload = "direct"`.
>
> **`direct` needs CORS the test cloud doesn't have.** A preflight
> (`OPTIONS …/v2/images/{id}/file`, `Origin` Moonlight's, asking for `PUT` with `x-auth-token`
> and `content-type`) answers 200 with no `Access-Control-Allow-*` header, for both the Tailscale
> and the `localhost` origin, so a browser refuses the `PUT`. Operators of such a cloud should use
> `proxy`.
>
> **`max_upload_gb` default: 20 GiB** (user's choice, "for now"; `prefs.rs`, `CONFIGURATION.md`,
> `moonlight.conf.example`).
>
> **Kept open until the test cloud's Glance allows CORS** from Moonlight's origin
> (`[cors] allowed_origin`, with `PUT`, `X-Auth-Token` and `Content-Type`); the user will add
> it. Then: a real `direct` upload in the browser, and close #2.
>
> Narrowing the token `direct` hands the page (an application credential with one access rule)
> moved to item 95 ([#17]).

**Goal.** The Images panel has no *Create Image* button; Horizon's is a dialog with:

- *Image Details*: *Image Name*, *Image Description*;
- *Image Source*: *File* (browse) or, when `IMAGES_ALLOW_LOCATION` is on, *URL*; *Format*
  (`disk_formats`, preselected from the file extension);
- *Image Requirements*: *Kernel* / *Ramdisk* (AMI-style formats), *Architecture*, *Minimum Disk
  (GB)*, *Minimum RAM (MB)*;
- *Image Sharing*: *Visibility* (`public` for admins), *Protected*;
- *Metadata* (custom properties).

After *Create Image* the row appears as `queued` / `saving` with a progress indication, then
`active`. Uploads of several GB must work without the server holding the file in memory.

**Implementation.** Split in slices; the fields reuse item 33 (`edit_image_dialog`'s fields
and `ImageChanges` rules) and item 41 (file input), the settings come from item 29.

1. *Record only*: `OsAction::ImageCreate(ImageSpec)` → `image::v2::image::create` (`POST
   /v2/images` with name, disk/container format, visibility, protected, min_disk, min_ram and
   custom properties). Dry-run test with the exact body. On its own it leaves a `queued`
   image, so it ships together with slice 2 or 3.
2. *From a URL*: Glance image import `web-download` (`image::v2::image::import::create`, `POST
   /v2/images/{id}/import` with `{"method": {"name": "web-download", "uri": …}}`), offered only
   when `GET /v2/info/import` lists the method (detected like the other capabilities, item 19)
   and `[images] allow_location` is on. Validate the URL (http/https only). Dry-run test with
   both requests (create, then import with the placeholder ID).
3. *From a file*, with the upload method set by the operator in `moonlight.conf`:
   `[images] upload = off | proxy | direct` (Horizon's `HORIZON_IMAGES_UPLOAD_MODE`: `off`,
   `legacy`, `direct`). `proxy` sends the file through Moonlight; `direct` has the browser
   send it straight to the image service's (Glance) endpoint. Default `off`, so nothing
   changes until an operator enables it. *File* is offered under *Image Source* (next to
   *URL* and *None*) only when the mode isn't `off`; `[images] max_upload_gb` limits both
   modes, checked in the dialog before the upload starts. The image record (slice 1) is
   always created by the server.
   - `proxy` (Horizon's `legacy`, through Moonlight): a dedicated axum route (not a server
     function: server functions buffer the body) receives the browser's file as a stream and
     forwards it to `PUT /v2/images/{id}/file` (`application/octet-stream`) with the
     session's scoped token, chunk by chunk. The SDK's endpoints take the whole body as
     `Vec<u8>`, so this is a raw streamed HTTP request to the catalog's image endpoint: a write
     outside `OsAction` / `Exec` that needs an explicit, narrow exception in AGENTS.md §1.2,
     like the console calls (§1.3), gated by `[writes] mode = on` (dry-run shows the create
     request and "would upload N bytes"). The route checks the session like the rest. Costs
     bandwidth and no disk (nothing is written locally).
   - `direct` (straight from the browser to the image service, as Horizon's `direct`): the
     server creates the record and returns the upload URL (catalog image endpoint +
     `/v2/images/{id}/file`) plus a token for that single request; the browser `PUT`s the file
     there itself. Needs the image service to allow CORS from Moonlight's origin
     (`allowed_origin` in Glance's `[cors]`), and it hands a scoped Keystone token to the
     page, which Moonlight otherwise never does (AGENTS.md §1.5–6: tokens stay in the
     encrypted cookie). So: document the trade-off in CONFIGURATION.md, send the token only
     in the response of that call (never stored, never logged), and prefer a token restricted
     to the image service if the cloud offers application credentials with access rules (item 95).
     `[writes] mode` still applies: in dry-run the server returns the preview and no token.
   - Progress in both modes from the browser's upload events (`XMLHttpRequest.upload`), with
     a cancel button; a cancelled or failed upload leaves a `queued` image that the dialog
     offers to delete.
4. Polling: the Images list polls `queued` / `saving` / `importing` rows like busy instances
   (`poll_while_busy`), and shows `os_glance_failed_import` when an import fails.

Decided: both upload modes, selected in `moonlight.conf` (`[images] upload`), default `off`.
Settled on 2026-11-05: `max_upload_gb` defaults to 20, and the dev cloud's Glance has no CORS,
so `direct` doesn't work there. "Straight into Cinder" was read as straight into the image
service (Glance): an upload into a Cinder volume would be a different feature (*Upload to
Image* goes the other way).

**Settings that come with it** (moved from item 29): `[images] upload` = off | proxy | direct
(`HORIZON_IMAGES_UPLOAD_MODE`), `max_upload_gb`, and `allow_location` (`IMAGES_ALLOW_LOCATION`, the
*From a URL* source). Document them in `CONFIGURATION.md` and `moonlight.conf.example`.

## 46. Refresh volumes and snapshots while they change state

> ✅ **Done.** `widgets::volume_busy` / `snapshot_busy` (the states listed below; tested,
> including the `error_*` states that must not poll). `list_page!` takes an optional
> `busy = fn(&T) -> bool`: it runs `poll_while_busy` and shows *Updating…* in the header
> (every list goes through it; lists without `busy` never poll). Volumes and Snapshots use it,
> and their *Status* cell is `lists::moving_status` (pill + busy bar). The volume and
> snapshot detail pages poll the same way and show the bar and *Updating…* next to the
> status; a volume deleted while its page is open ends in the friendly not-found page
> (`render` → `ErrorPage`), which also stops the polling. Browser: with every volume settled
> there is no polling and no chip; a running transition wasn't seen (it needs a write).

**Goal.** After *Create Volume*, *Delete Volume*, *Extend*, *Attach*, *Upload to Image*… the
Volumes list shows the state at the moment of the action (`creating`, `deleting`,
`extending`) and stays there until the user presses refresh. Instances already refresh
themselves while busy (item 11, `widgets::poll_while_busy`). Do the same for volumes and
volume snapshots: while any row (or the open detail page) is in a transitional state, refetch
every `[ui] poll_interval_ms` (5 s) for at most `POLL_FOR_MS` (10 min), show the
indeterminate bar in its *Status* cell, and stop as soon as nothing is moving. A deleted
volume disappears from the list on the first refresh after Cinder removes it (Horizon does the
same with its row-level AJAX updates).

**Implementation.**

- Transitional states, a pure `fn volume_busy(status) -> bool` in `widgets.rs` next to
  `is_busy` (tested): volumes `creating`, `deleting`, `extending`, `attaching`, `detaching`,
  `downloading`, `uploading`, `retyping`, `backing-up`, `restoring-backup`, `reserved`,
  `awaiting-transfer`, `maintenance`; snapshots `creating`, `deleting`, `updating`,
  `restoring`, `backing-up`, `unmanaging`.
- `pages/lists.rs`: the `list_page!` macro has no polling; add an optional `busy: fn(&T) ->
  bool` argument (or a second macro arm) that wraps `poll_while_busy(data, move || data rows
  any busy)`, and use it for `VolumesPage` and `SnapshotsPage`; show `<Busy/>` in the status
  column for busy rows (as `instances.rs` does) and the *Updating…* chip in the header.
- `pages/details.rs`: `VolumeDetailPage` / `SnapshotDetailPage` poll the same way; a volume
  that was deleted while its page is open turns the next fetch into a 404: show the friendly
  not-found page (item 14) instead of an error loop, and stop polling.
- Polling is GET only (check with `read_network_requests`, AGENTS.md); the rounds counter
  resets after every user action (`data_version` bump), so a new action starts a new window.
- Later, the same helper serves images (`queued` / `saving` / `importing`, item 45), floating
  IPs and load balancers (`PENDING_*`).

## 47. 🐞 FIX: long row menus (instances) run off the screen

> ✅ **Done.** `dialog::menu_box` (pure, tested in `long_menus_stay_reachable`) places the fixed row
> menu: below when its natural height (`scroll_height`; `offset_height` was already capped by the
> CSS 70vh) fits, above when only that fits, else on the side with more room with `max-height` set
> to that room so the list scrolls inside; `left` clamped to both viewport edges. `place_menu`
> applies it as `left/top/max-height`. Verified in a 729 px window on ttcp-dev: first instance row's
> menu (16 items, 593 px) opens below and ends at 721 px, scrolling (364 px visible); the last row's
> (18 items) opens above from 8 px to just over its button; the detail page's header menu ends at
> 721 px; focusing *Delete Instance* (out of view) scrolls it into view, and scrolling inside the
> menu keeps it open. Arrow-key navigation inside menus was not checked (focus and Tab reach every
> item).

**Goal.** The instance row menu has up to ~17 items (*Create Snapshot* … *Delete Instance*).
Opened from a row in the middle or lower half of the window, it runs past the bottom edge and
the last items (*Rebuild*, *Delete*) can't be reached: scrolling the page closes the menu (by
design, item 13, because the fixed menu would drift from its button). Every item must be
reachable at any window height, with the mouse and with the keyboard.

**Cause.** `dialog::place_menu` positions the `position: fixed` list below the button, or above
it only when the **whole** list fits above (`r.top() - 4 - mh > 8`). A tall list that fits
neither way stays below and overflows; its `max-height: 70vh` (`.menu-list`) is larger than the
space left under a lower row.

**Implementation.**

- `place_menu`: measure the space below (`vh - r.bottom() - 12`) and above (`r.top() - 12`);
  open where the list fits, else on the side with more room, and set the list's `max-height`
  to that space (it already has `overflow: auto`, and scrolling inside an open menu doesn't
  close it, see `install_menu_dismissal`). Keep `left` clamped to the viewport too.
- Make the placement a pure function `menu_box(trigger: (top, bottom, right), menu: (w, h),
  viewport: (w, h)) -> (left, top, max_height)` so it can be tested without a browser: below
  when it fits; above when only that fits; the bigger side, capped, when neither fits; never
  above the viewport top.
- Keyboard: with the list scrollable, the focused item must scroll into view (`focus()` does
  it); check *Arrow* navigation reaches *Delete Instance*.
- Verify in the browser with a short window (e.g. 700 px high), the menu of the last
  instance row and of a row in the middle, and the detail page's header menu.

## 48. 🔒 Request boundary: same-origin check and safe IDs in API paths

> ✅ **Done (2026-10-26).**
>
> - **Same origin:** `server::same_origin_only` (axum `from_fn`, layered in `main.rs` before
>   `security_headers`) refuses with `403` and a warning log every request other than
>   `GET`/`HEAD`/`OPTIONS` that `server::same_origin` rejects: `Sec-Fetch-Site` must be
>   `same-origin` or `none`; without it, `Origin` (not `null`) must equal `[session]
>   public_url` (new, `MOONLIGHT_PUBLIC_URL`, normalised by `settings::origin_of`) or, unset,
>   the request's `Host`; no header at all is refused. **Open question settled:** browsers that
>   send `Sec-Fetch-Site` (all current ones) don't depend on `Host`, so a proxy only matters for
>   the `Origin` fallback, which `public_url` covers. Documented in `CONFIGURATION.md` and
>   `moonlight.conf.example`.
> - **Safe IDs:** `model::check_id` refuses empty, `.`, `..`, and `/ \ ? # %` or control
>   characters. `api.rs` calls it first in the 15 `get_*` readers that take an `id`/`name` and in
>   `run_instance_log`/`run_instance_console`; `run_action` calls `OsAction::check_ids`, which
>   walks the serialised action and checks strings under `id`, `*_id`, `*_ids`, `kind` (QoS rule
>   paths) and key pair `name`s (in Nova's path). Empty strings pass there: the action rejects a
>   missing required ID. `switch_project` already accepts only the user's projects.
>
> Tests: `writes_must_come_from_the_same_origin` (cross-site, same-site sibling, `Origin: null`,
> no headers, a proxy with and without `public_url`), `public_url_becomes_an_origin`,
> `check_id_tests` (bad characters; nested `*_id`, `*_ids` lists, key pair names, QoS kind,
> inside a batch, empty optional IDs), and the actions tests' `dry()` now asserts `check_ids`
> passes for every valid action (no false positives across all of them). Both clippy variants
> clean, 250/250 tests. Live on the dev server: a cross-site POST and a POST without headers get
> 403, a POST whose `Origin` matches `Host` and the browser's own same-origin POST reach the
> handler, reads are untouched (the app and the *Launch Instance* choices load), and
> `get_keypair` with `../os-hypervisors` is refused as `Invalid` before any OpenStack call.

**Goal.** Two gaps found in the security review (October 2026) at the browser↔server boundary,
both needed before `[writes] mode = on` is used by real users:

1. **Cross-origin requests.** Server functions that change state (`run_action`,
   `run_instance_log`, `run_instance_console`, `switch_project`, `switch_region`, `login`,
   `logout`) are protected only by the session cookie's `SameSite=Lax`. That blocks other
   *sites* but not another origin of the same site (a sibling subdomain under the same
   registrable domain), and it allows login CSRF (signing the victim into the attacker's
   account). Horizon has Django's CSRF token.
2. **IDs pasted into URL paths.** The SDK builds paths with `format!("servers/{id}")`
   without percent-encoding (e.g. `openstack-sdk-compute-0.22.8/src/v2/server/get_20.rs`), and so
   do our `JsonRequest` paths (`actions.rs`, trunks and port forwarding). An `id` holding `/`,
   `..`, `?` or `#` sends the request to another path of the same service. Method and token
   stay the user's, so OpenStack's policy still applies, but it sidesteps Moonlight's own
   gates (disabled plugins, the fixed set of endpoints in AGENTS.md §1.3).

**Implementation.**

- **Same-origin middleware** in `server/mod.rs`, next to `security_headers`, layered in `main.rs`: for
  every method other than `GET`/`HEAD`, accept when `Sec-Fetch-Site` is `same-origin` or
  `none`; without that header, accept when `Origin`'s host equals the request's `Host`; else
  `403`. Pure function `same_origin(method, &HeaderMap) -> bool` with tests (cross-site,
  same-site, missing headers, `Origin: null`). The login `ActionForm` posts natively without
  JavaScript, and browsers send `Origin` on those POSTs too, so it keeps working.
- **Open question:** behind a reverse proxy that rewrites `Host`, compare against a configured
  `[server] public_url` (new setting, `server/settings.rs`) instead. Decide when deploying.
- **Safe IDs**: `model::check_id(&str) -> Result<(), AppError>` rejecting empty values, `/`,
  `\`, `?`, `#`, `%`, control characters and the segments `.`/`..`. Call it:
  - in `api.rs` on every `id`/`name` argument of the `get_*` functions and the console ones;
  - in `run_action` through `OsAction::check_ids()`, which serialises the action to
    `serde_json::Value` and checks every string under a key that is `id`, ends in `_id` or `_ids`
    (so new variants are covered without a list to maintain). Keypair names go through
    `check_id` too (Nova's allowed set has none of those characters).
- Tests: `check_id` cases; `check_ids` catches a nested `port_id` in a launch spec and a list of
  `security_group_ids`; one `dry(...)` per kind proves a valid action still builds.

## 49. 🔒 Session and response hardening (secure cookies, headers, logout)

> ✅ **Done.** `[session] secure_cookies` defaults to `true`; then the cookie is
> `__Host-ml_session` (`session::cookie_name`). `decode` reads both names, so switching
> doesn't sign anyone out, and logout deletes both. `server::security_headers` (was
> `no_framing`): `X-Frame-Options`, CSP `frame-ancestors 'none'; object-src 'none'; base-uri
> 'self'`, `nosniff`, `Referrer-Policy: same-origin`, HSTS (1 year) only with secure cookies;
> pure `add_security_headers` tested. Logout: the user chose revocation **on by default**,
> `[session] revoke_on_logout = true`: `actions::revoke_token` sends `DELETE /v3/auth/tokens`
> (`X-Subject-Token`, a sensitive header that previews never show) through `Exec`, so only
> with `[writes] mode = on`; best effort, logged. In every mode `os::forget` puts the
> token's hash in the new `AppState::revoked` cache (24 h) and `session::current` refuses
> it: without that a copied cookie kept working, because cached scoped connections don't
> ask Keystone. Docs: `CONFIGURATION.md`, `moonlight.conf.example`, README, AGENTS.md §1.3
> (the one write outside `OsAction`). **Not tried live:** the Keystone revocation itself
> (logging out would end the user's session on the test cloud); check that `auth/tokens`
> resolves to `/v3/auth/tokens` on the next real logout (log line `logout` vs `token not
> revoked`). Browser: the new headers are on every response (`curl -I`); HSTS absent, as
> expected with the dev config's `secure_cookies = false`.

**Goal.** Deployment defaults that fail safe, from the security review (October 2026):

- `[session] secure_cookies` defaults to `false` (`server/settings.rs`, `Session::default`):
  deployed behind TLS without setting it, the session cookie (which carries the encrypted
  Keystone token) is also sent over plain HTTP. No HSTS either.
- Responses only carry `X-Frame-Options` and `frame-ancestors`: no `X-Content-Type-Options`,
  `Referrer-Policy`, `object-src` or `base-uri`.
- `logout` only clears the cookie: a copied cookie keeps working until the token expires
  (~1 h) or `timeout_minutes`. Horizon revokes the token on logout (`TOKEN_DELETION_DISABLED`
  defaults to `False`).

**Implementation.**

- `secure_cookies` defaults to `true`; `moonlight.conf.example` and `CONFIGURATION.md` say to set
  it to `false` for plain-HTTP development. When it is on, name the cookie `__Host-ml_session`
  (needs `Secure`, `Path=/`, no `Domain`; `session.rs` already sets these). Accept the old name
  once while reading, so a restart doesn't sign everybody out. **Check first:** Chrome and
  Firefox keep `Secure` cookies on `http://127.0.0.1`, Safari doesn't; the dev setup keeps
  `false` in the repo-local `moonlight.conf`.
- `no_framing` becomes `security_headers` (`server/mod.rs`): also `X-Content-Type-Options:
  nosniff`, `Referrer-Policy: same-origin`, CSP `frame-ancestors 'none'; object-src 'none';
  base-uri 'self'`, and `Strict-Transport-Security: max-age=31536000` only when
  `secure_cookies` is on. Keep `or_insert` so a header set earlier wins; extend its test. A
  strict `script-src` is out of scope (Leptos hydration uses inline scripts).
- **Revoke on logout**: `identity::v3::auth::token::delete` with the session token as
  `X-Subject-Token`, best effort (log a failure, clear the cookie anyway), and drop the token's
  entries from `connections`, `projects`, `capabilities`.
  **This is a mutating Keystone call**: AGENTS.md §1 forbids it against the production cloud. Ask
  the user how to gate it before building (proposal: new `[session] revoke_on_logout`, default
  `false`, sent through `Exec` so dry-run only previews it) and test it only on a test cloud.
- Tests: the defaults (`settings.rs`), the header set, the cookie name under each setting.

## 50. 🔒 Sign-in throttling and error details kept on the server

> ✅ **Done.** `server/throttle.rs`: failure counters in a `moka` cache whose TTL restarts on
> each failure; keys are hashes of `domain + username` (case-insensitive) and of the client
> address when `[log] client_ip_header` gives one. Only rejected credentials (401) count, not
> a Keystone that is down; success clears the username only. Blocked: new
> `ErrorKind::TooManyAttempts`, *Too many failed sign-ins* on the login page, Keystone not
> called. `[login] max_failures` (5, 0 = off) and `lockout_seconds` (300). Error details: the
> user chose hiding with an opt-in, `[ui] show_error_details = false`: `sfe` passes errors
> through `errors::public`, which reduces `detail` to the context (`GET servers/x`) and keeps
> kind, service and request ID; the log keeps the full text. Tests: throttle limits, window,
> off, key hashing; `public` with and without the setting; the defaults. Browser: a missing
> volume's *Technical details* now read `GET volume`. The throttle wasn't tried live (it needs
> failed sign-ins and a logout).

**Goal.** Two low-risk findings from the security review (October 2026):

- `login` passes every attempt straight to Keystone: nothing slows down password guessing
  beyond Keystone's own lockout (`[security_compliance] lockout_failure_attempts`), which many
  clouds leave off.
- `errors::classify` puts the SDK's full message in `AppError.detail`, which reaches the
  browser: internal endpoint URIs (`uri: https://nova…`) and service details. Horizon shows a
  generic message.

**Implementation.**

- Throttle in `api::login`: an in-process `moka` cache (like `config::AppState`) counting
  failures per `hash(username)` and per client address (`session::client_ip`, only when
  `[log] client_ip_header` is set, else per username only); after N failures in a window,
  answer `429`-style `AppError` without calling Keystone. Settings `[login] max_failures`
  (default 5) and `lockout_seconds` (default 300). The state is per process: say so in
  `CONFIGURATION.md`. Pure counter logic with tests (window expiry, success resets).
- Error detail: keep the full text in the log (`sfe` already logs it), and send the browser
  only `kind`, `service` and `request_id` plus a short context (`"GET servers/x"` without
  host). The friendly error page (item 14) already shows kind and request ID. **Open
  question:** operators may want the full detail in the UI; if so, add `[ui]
  show_error_details` (default `false`).

**Reviewed and left out:** the leaked `cookie_key` in an un-ignored `moonlight.conf.bak`
(fixed in `.gitignore`; rotating the key is the operator's job); `http://` console URLs (the
browser blocks mixed content on HTTPS pages); detail views that open another project's
resource by ID with an admin token (Nova/Neutron decide, same as Horizon).

## 51. Key pairs: show the *Type*, and a *Key Type* selector (SSH / X509)

> ✅ **Done.** Key pair reads (`fetch::keypairs_raw`, used by the list and the launch choices,
> and `keypair_detail`) send `compute 2.2`, the version that adds `type`; not 2.47, because
> from 2.35 the list pages with `limit`/`marker`, which `paged` doesn't follow there. *Import
> Public Key* has *Key Type* (*SSH Key* / *X509 Certificate*) and a help text; the key is
> checked against the type in the dialog and in `apply` (`model::check_public_key`), and
> `OsAction::KeyPairImport` gained `key_type`, sent as `type`. *Create Key Pair* stays
> SSH-only (the user's choice); its help says X509 must be imported. Tests: the choices show
> the real type, import bodies for both types, mismatches refused. Browser: the import dialog
> shows *Key Type* and its help; the test project has no key pairs, so the *Type* column
> wasn't seen with data.

**Goal.** Two gaps on Project › Compute › Key Pairs:

1. The *Type* column (`lists.rs`, `col(K::Type, …)`) and the detail page's *Type* row
   (`details.rs`) show `-`, reported on a newly created key pair. **Cause:** `fetch::keypairs`,
   the `ChoiceSet::KeyPairs` arm of `fetch::choices` and `fetch::keypair_detail` use
   `keypair::list_20` / `get_20`, which ask for compute microversion 2.0; Nova only adds `type`
   from **2.2** on. So it is every key pair, not only new ones (confirm in the browser), and
   the launch dialog's key pair choices fall back to a hard-coded `"ssh"`
   (`mapping::keypair_choices`).
2. *Import Public Key* and *Create Key Pair* offer no type. Horizon's import dialog has *Key Type*
   (*SSH Key* / *X509 Certificate*), and the public key field takes a PEM certificate for X509.

**Implementation.**

- Reads: send `fetch::compute_mv()` (`compute 2.47`, the version servers already use) on the
  three key pair calls, through `list_where!(…, |b| b.header(k, v))` and the matching header on
  `get_20`. `mapping::keypairs` / `keypair_detail` already read `type`. Test the mapping
  with a 2.2-shaped list (`{"keypairs": [{"keypair": {"name", "fingerprint", "type"}}]}`).
- Import: `OsAction::KeyPairImport { name, public_key, key_type: KeyType }` (new enum in
  `model.rs`, `Ssh | X509`, serde `ssh`/`x509`), sent as `create_210`'s `_type`
  (`m::Type::Ssh` / `m::Type::X509`). Dialog (`ops::import_keypair_dialog`): `Field::select`
  *Key Type*, *SSH Key* first, with the public key field's help switching text by type
  (`.when`). Check the text before sending: SSH starts with `ssh-`, `ecdsa-` or `sk-`; X509
  with `-----BEGIN CERTIFICATE-----` (`AppError::invalid`, also checked in `apply`).
- Generate: Nova stopped generating keys at 2.92, so X509 generation would mean a self-signed
  certificate built on the Moonlight server (`rcgen` crate, a new dependency) and a PEM private
  key in `ActionOutcome.secret`, like Ed25519 today. **Open question:** offer it, or keep
  *Create Key Pair* SSH-only and say so in its help text (X509 key pairs are rare and only used
  by Windows/cloudbase-init images). Proposal: SSH-only for now.
- Gating: the type field needs compute 2.2; every supported cloud has it, so no `needs` entry
  unless detection shows otherwise.
- i18n: `KeyType`, `SshKey`, `X509Certificate` and the two help texts, English and Spanish.
- Tests in `actions.rs`: `dry(KeyPairImport { key_type: X509, … })` body is
  `{"keypair": {"name", "public_key", "type": "x509"}}`; the existing SSH test gains
  `"type": "ssh"`; a mismatched key and type is rejected. Verify the list, the detail page and
  both dialogs in the browser **in dry-run**.

## 52. Server group detail page

> ✅ **Done (2026-10-26).** `ServerGroupDetailPage` (`pages/details.rs`, route
> `server_groups/:id`) with an *Information* card (Name, ID, Policy, Rules as `key = value`,
> Project ID, User ID) and a *Members* table (name linked to the instance, else the ID; Nova
> status; ID). Data: `fetch::server_group_detail` (2 calls: `server_group::get_21` with
> `compute 2.64`, plus the detailed server list, `soft`, since the brief list has no status) →
> `mapping::server_group_detail`, served by `api::get_server_group`. The list's name links to it.
> Header menu: `ops::server_group_detail_items`, whose delete dialog uses the new
> `Dialog::then_go("/project/server_groups")` (navigates only after a real write, before the
> data refetch, so the page doesn't flash "not found"). **Open question settled:** the instance
> page's *Information* card shows *Server Group*, found by membership (`mapping::group_of` over
> `os-server-groups`, now a 10th parallel call on the instance page) instead of raising server
> reads from 2.47 to 2.71. Tests: `server_group_detail_names_members_and_reads_rules` (2.64 and
> pre-2.64 JSON, a missing member) and the instance detail test asserts the group. Both clippy
> variants clean, 239/239 tests. Browser (ttcp-dev, *pokemon*): the list name opens the page
> (2 API · 0.65 s; *Soft Anti Affinity*, Rules `-`, empty members table), the delete dialog opens
> with the acknowledgement box and was cancelled, and the instance page shows *Server Group* `-`
> at 10 API · 0.84 s.
> **Not seen:** a group with members (none on ttcp-dev), a real delete's return to the list, and
> Horizon's labels (no Horizon tab). The key pair detail page has the same refetch-after-delete
> issue and could use `then_go` too.

**Goal.** On Project › Compute › Server Groups the name is plain text (`lists.rs`,
`ServerGroupsPage`, `<span class="strong">`): the only thing the list says about members is
their count, so there is no way to see *which* instances are in a group, or its rules. Make the
name a link to `/project/server_groups/:id`, a detail page like the key pair one:

- *Information* card: Name, ID, Policy (humanised, `i18n::server_group_policy`), Rules
  (`max_server_per_host`, compute 2.64+; `-` when none), Project ID, User ID.
- *Members* table: instance name (link to `/project/instances/:id`; ID when the instance is
  gone or unnamed), status, and the ID; empty state when the group has no members.
- Header actions: the row menu (`ops::server_group_items`, *Delete Server Group*), which must
  go back to the list after deleting.
- Link the group from wherever it is mentioned (AGENTS.md §4 step 7): the instance detail page
  could show *Server Group* in its *Information* card. Nova only returns `server_groups` on a
  server from **2.71**, and server reads use 2.47 (`fetch::COMPUTE_MICROVERSION`), so either
  raise that version (check nothing else changes shape between 2.47 and 2.71) or find the group
  by membership in the group list. **Open question**, settle when implementing; the detail page
  doesn't need it.

Check how Horizon presents a server group (read-only, in the shared Horizon tab) and keep the
same labels.

**Implementation.** The read-only recipe (AGENTS.md §4):

- Model `ServerGroupDetail { group: ServerGroup, rules: Vec<(String, String)>, project_id,
  user_id, members: Vec<GroupMember { id, name: Option<String>, status: Option<String> }> }`.
- `mapping::server_group_detail(&Value, &[Value] /* servers */)`, tested with 2.64-shaped JSON
  (`policy` + `rules`) and older JSON (`policies`, no `rules`), and a member missing from the
  server list.
- `fetch::server_group_detail(os, id)`: `compute::v2::server_group::get_21` with the
  `openstack-api-version: compute 2.64` header (so `policy`/`rules` come back), in parallel with
  `servers_brief` wrapped in `soft()` for the member names.
- `#[server(input = GetUrl)] get_server_group(id)` in `api.rs` using `ctx()`.
- `ServerGroupDetailPage` in `pages/details.rs` (`DetailHeader`, `Card`/`Kv`, `DataTable`),
  route `server_groups/:id` in `app.rs`, list column switched to `link(...)`.
- i18n: `Rules`, `MaxServerPerHost`, `Members` (if not already there), `NoMembers`, both
  languages. Verify in the browser in dry-run (the delete dialog opens; don't submit).

## 53. 🐞 FIX: *Create Load Balancer* fails without a pool name; *— None —* listed twice

> ✅ **Done.** The wizard's *Pool* step has a required *Name* (`pool_name`, prefilled `pool-1`),
> sent as the default pool's `name`; `LbCreateSpec::check` refuses an unnamed or blank default
> pool, so `run_action` rejects it too, not only the dialog. The duplicate *— None —* came from
> `ops::none_or` / `static_refs(…, true)` adding an empty choice that `Combo` already adds to
> every optional select: `none_or` is gone, `static_refs` takes no flag. That also fixes *Edit
> Pool* (*Session Persistence*), *Edit Listener* (*Default Pool*) and *Create Pool*
> (*Listener*). Tests: `load_balancer_wizard_builds_one_spec_and_skips_what_is_switched_off`
> (name sent, field required with its default, no empty static choice, empty name refused) and
> `load_balancer_wizard_is_one_fully_populated_request` (unnamed and blank pool rejected in
> dry-run). Clippy clean on both targets; 223 tests pass. **Not checked in the browser**: the
> dev server ran with `[writes] mode = "on"`.

**Goal.** Reported in use: *Create Load Balancer* with a pool fails unless the pool has a name.
The wizard had no pool name field, so the default pool was always unnamed, and Octavia's
single-call create (one request with listener, pool, members and monitor) rejected it. The
*Session Persistence* select also listed *— None —* twice.

**Implementation.** `ops::create_load_balancer_dialog` (new `pool_name` field in the *Pool*
step), `model::LbCreateSpec::check` (name required with a pool), `ops::static_refs` and the
selects that used `none_or`.

## 54. Load balancers to octavia-dashboard parity: members, listener and pool options, TLS, L7

> ✅ **Done (2026-10-27), all five slices.** **Slice 1, Members: done (2026-10-26).** A members editor,
> `components/member_rows.rs` (`dialog::Kind::MemberRows`, `Field::member_rows`), laid out like
> Horizon's *Pool Members* step (compared with it open on ttcp-dev, read only):
>
> - *Allocated Members* (collapsible, with a count): a table of IP Address, Subnet, Port and
>   Weight with *Remove*. A chevron opens a details row with Monitor Address, Monitor Port,
>   *Admin State Up* and *Backup* (Yes / No) and Name. *Add external member* adds a row whose IP
>   is typed and whose subnet is a `Combo` over the new `ChoiceSet::LbMemberSubnets` (subnets of
>   the project's own, shared and external networks, by name: the same four Horizon lists),
>   blank until picked and required (`MemberRow::spec`). Instance addresses and existing members
>   show their address and subnet name as text (`MemberRow.typed` is false: changing an
>   existing member's address would replace it).
> - *Available Instances* (collapsible): a filter box and a Name / IP Address table with *Add*
>   per address (`ChoiceSet::LbMemberTargets`, whose choices now carry `[instance, ip, subnet
>   name]` cells). As in Horizon, an added instance stays listed, so the same address can be a
>   member again on another port.
>
> One difference on purpose: new rows start on the listener's port (the field named by
> `port_from`; else the first row's, else 80) where Horizon leaves it empty. The value is a JSON
> list of `model::MemberRow` (numbers kept as text while typing); `model::member_specs` checks
> each row (`MemberRow::spec` → `LbMemberSpec::check`) and refuses an address:port listed
> twice (Octavia's member key, also in `LbCreateSpec::check`). `LbMemberSpec` gained
> `monitor_address`, `monitor_port`, `admin_up`; `LbMember` maps `monitor_address` /
> `monitor_port`. Where it is used:
>
> - *Create Load Balancer*: the *Members* step is the editor (the shared *Member port* and
>   *Weight* fields are gone).
> - *Edit Members* (pool row menu, `ops::pool_members_dialog`): starts from the pool's members
>   (`MemberRow::from_member`, so weight, backup and monitor settings are sent back unchanged)
>   and sends `OsAction::PoolSetMembers`, Octavia's batch `PUT /lbaas/pools/{id}/members`
>   (SDK `pool::member::replace`). It is destructive (`danger`, an intro saying the list
>   replaces the pool's) and the editor lists live, under *Saving deletes these members:*, the
>   members a save removes (`model::removed_members`, matched by address and port).
> - *Add Member* takes an *External address* (IP + subnet) as well as an instance address; *Add*
>   and *Edit Member* set the monitor address/port. `MemberUpdate` is now a JSON request (not
>   `member::set`) so an emptied monitor address/port is sent as `null`.
>
> One `member_fields!` macro fills the three SDK member builders (create body, `member::create`,
> `member::replace`). Tests: `member_rows_tests` (rows → specs, bad rows, duplicates, round trip,
> removals), `pool_members_are_replaced_in_one_batch_request` and the updated wizard and
> `MemberUpdate` bodies (`actions.rs`), `edit_members_starts_from_the_pool_and_sends_one_batch`,
> `add_member_takes_an_external_address` and the wizard test with per-row ports and an external
> row (`ops.rs`), monitor fields in the load balancer mapping test. Both clippy variants clean,
> 246/246 tests. Browser (ttcp-dev, writes on, so nothing submitted), next to Horizon's wizard
> on the same step with the same picks: *Add* on an instance and *Add external member* give the
> same two rows, the details row has Horizon's fields in its order, the external subnet lists the
> same four subnets, dark mode reads well, no console errors. **Not seen in the browser:** *Edit
> Members* and *Add Member* (no load balancer on ttcp-dev; covered by the `ops.rs` tests), and
> the request preview (it needs a submit, and writes are on). An emptied monitor field is
> omitted from the batch, and whether Octavia then keeps or clears it is unverified: clearing a
> monitor goes through *Edit Member*, which sends `null`.
>
> **Slice 2, listener and pool options: done (2026-10-27).** `model::LbListenerOptions` (the
> four timeouts in ms, `insert_headers` from `LB_INSERT_HEADERS`, `allowed_cidrs` as
> `Option<Vec>`: `None` = not offered, never sent) is `LbListenerSpec::options` and
> `ListenerUpdate::options` (`None` = leave alone); `LbPoolSpec` and `PoolUpdate::tls` carry
> `tls_enabled` and `tls_ciphers`. Checks: `LbListenerOptions::check` (timeouts ≤ `i32::MAX` ms,
> since the SDK takes `i32` although Octavia allows a year; headers known and only on HTTP
> listeners; CIDRs parse with `prefs::Cidr`) and `model::check_pool_tls` (no TLS on UDP, ciphers
> only with TLS, colon-separated). Requests: macros `listener_options!` and `pool_tls!` fill the
> wizard body, `listener::create` and `pool::create`, sending only what is set; the JSON
> `ListenerUpdate` sends cleared values as `null` / `{}` / `[]` (Octavia's "back to default"),
> headers only for HTTP, CIDRs only when offered; `PoolUpdate` sends `tls_enabled` only when
> offered and ciphers only when set (they can't be cleared back to the default from the dialog).
> Gating: `Capabilities::lb_mv`, detected from the `load-balancer` endpoint's version (`v2.26`
> on ttcp-dev), and `Requires::LbMv` with `needs::LB_TIMEOUTS` (2.1), `LB_POOL_TLS` (2.8),
> `LB_ALLOWED_CIDRS` (2.12) and `LB_POOL_CIPHERS` (2.15); `lists::ModulesCard` shows it. UI:
> `ops::listener_option_fields` puts them under an *Advanced options* box (open when the
> listener has any), in octavia-dashboard's order and labels (read from its templates on
> ttcp-dev's Horizon); *Insert Headers* only for HTTP; `pool_tls_fields` adds *TLS Enabled* and
> *TLS Cipher String*. In the wizard's Listener and Pool steps and in *Create/Edit Listener* and
> *Create/Edit Pool*; `LbListener::options` and `LbPool::tls_*` (mapping `listener_options`)
> prefill the edits. Differences on purpose: the box (octavia-dashboard always shows the
> fields) and empty timeouts with the default in the help text (it prefills 50000 / 0 / 5000 /
> 50000 and always sends them). Tests: `listener_and_pool_options` (actions),
> `lb_option_tests` (model), `listener_options_read_headers_timeouts_and_cidrs` (mapping),
> `listener_and_pool_dialogs_carry_the_advanced_options` (ops). Browser (ttcp-dev, nothing
> submitted): the box shows the 4 timeouts, the 3 headers and Allowed CIDRs; choosing TCP
> hides the headers; the Pool step shows *TLS Enabled*, then *TLS Cipher String*; no console
> errors. **Not seen:** the edit dialogs (no load balancer on ttcp-dev; covered by the `ops.rs`
> test) and the request preview (it needs a submit).
>
> **Slice 3, TLS termination: done (2026-10-27).** Decided
> with the user: offered only when the `key-manager` plugin is enabled (and Barbican is in the
> catalog), `needs::LB_TLS_TERMINATION` = `needs::KEY_MANAGER`; Octavia should have Barbican
> access by default, to be confirmed together. `model::LB_TLS_PROTOCOL` (`TERMINATED_HTTPS`) is
> added to the protocol list by `ops::listener_protocols` when that holds; its pool is HTTP
> (`pool_protocol_for`). `LbListenerSpec` gained `default_tls_container_ref` and
> `sni_container_refs`, `LbListenerOptions::tls_ciphers` (Octavia 2.15), `ListenerUpdate::tls`
> `(certificate, SNI)`; `check_listener_tls` (a certificate required for and only for
> `TERMINATED_HTTPS`, Barbican URLs) and `lb_inserts_headers` (HTTP and TERMINATED_HTTPS).
> `OsAction::plugins` adds `KeyManager` for TLS listeners, and `get_choices` checks both
> plugins for the new `ChoiceSet::LbCertificates` (`mapping::lb_certificate_choices`: like
> octavia-dashboard, `certificate` containers holding a certificate and a private key, then
> opaque secrets not in one, labelled with the certificate's expiry). Macro
> `listener_certificates!` fills both SDK builders; the JSON update sends the refs. UI
> (`ops::listener_certificate_fields`): *Certificate* (required) and *SNI Certificates* after the
> port when the protocol is TERMINATED_HTTPS, in the wizard and *Create Listener*; *Edit
> Listener* offers them for such listeners; *TLS Cipher String* (`listener_ciphers`) under
> *Advanced options*. Tests: `tls_terminated_listeners_carry_their_certificates` (actions),
> `lb_certificates_are_complete_containers_and_loose_bundles` (mapping),
> `listener_certificates_belong_to_terminated_https` (model). Browser on ttcp-dev: with the
> user's go-ahead, a self-signed test certificate (CN `moonlight-test.example`, 30 days) was
> stored through Moonlight's Key Manager as secrets `moonlight-test-cert` (certificate) and
> `moonlight-test-key` (private) and container `moonlight-test-tls` (certificate); the wizard
> then offers TERMINATED_HTTPS, shows the certificate field and lists `moonlight-test-tls` (the
> key secret correctly not offered on its own). End to end, with the user: the wizard created
> `moonlight-tls-test` (VIP `route-1-subnet-1`, listener `https` TERMINATED_HTTPS:443 with
> `moonlight-test-tls`, HTTP pool `pool-1`, no members or monitor) in one `POST
> /lbaas/loadbalancers` whose preview carried `default_tls_container_ref`; it went from
> PENDING_CREATE to ACTIVE / ONLINE in about 3 minutes and the listener is ONLINE / ACTIVE, so
> Octavia reads Barbican containers with no ACL step. *Edit Listener* prefills the certificate
> and the default pool; Octavia reports its default timeouts (50000…) and cipher string, which
> the edit then shows as values. The load balancer and the test secrets are kept on ttcp-dev
> for slice 4. Small follow-up: the port stays 80 when TERMINATED_HTTPS is picked
> (octavia-dashboard leaves it empty); suggest 443.
>
> **Slice 4, L7 policies and rules: done (2026-10-27).** Model: `LbL7Policy` / `LbL7Rule`
> (`LoadBalancerDetail::l7policies`), specs `LbL7PolicySpec` (`check`: a pool for
> `REDIRECT_TO_POOL`, an http(s) URL for `REDIRECT_TO_URL`, position ≥ 1; `targets` keeps only
> the action's) and `LbL7RuleSpec` (`check`: a value, a key exactly for HEADER / COOKIE,
> FILE_TYPE with EQUAL_TO or REGEX), with octavia-dashboard's choices `LB_L7_ACTIONS`,
> `LB_L7_RULE_TYPES`, `LB_L7_COMPARE_TYPES`. Actions `L7PolicyCreate` / `L7RuleCreate` (SDK
> `l7policy::create`, `l7policy::rule::create`), `L7PolicyUpdate` / `L7RuleUpdate` (JSON, so
> the unused redirect target and a removed key are `null`; an emptied position keeps the
> current one), `L7PolicyDelete` / `L7RuleDelete`; `load-balancer` plugin. Fetch
> (`load_balancer_detail`): policies **by listener** in the second round, then each policy's
> rules, all spawned and counted. Not by the session's `project_id`: a policy belongs to its
> load balancer's project (Octavia sets it from the listener), and an admin can open another
> project's load balancer. On ttcp-dev, after signing in again, the session was scoped to
> another project (`317cb856…`, most likely `admin2`, the first in the list) while the load
> balancer and the policy are in `pokemon` (`1634f5…`), so the project-filtered list was empty.
> For the same reason the detail now fetches each pool's health monitor by its ID
> (`healthmonitor::get`, `healthmonitor_id` of the pools) instead of the session project's
> monitors, so an admin sees another project's monitors too (asked by the user). Not seen in
> the browser: no load balancer with a monitor exists on ttcp-dev now.
> Page: an *L7 Policies* tab on the load balancer detail (octavia-dashboard has them on a
> listener page, which Moonlight doesn't have): a policies table (name, listener, position,
> action, target, rules, status, admin state; *Create L7 Policy* when an HTTP or
> TERMINATED_HTTPS listener exists, `ops::l7_listeners`) and an *L7 Rules* table below
> (*Add L7 Rule* with a policy choice, also in each policy's menu with the policy preset).
> Dialogs `ops::l7_policy_dialog` (redirect pool / URL by action) and `ops::l7_rule_dialog`
> (*Key* for HEADER / COOKIE only). Tests: `l7_policies_and_rules` (actions),
> `l7_dialogs_build_policies_and_rules` (ops), the L7
> part of `load_balancer_detail_joins_listeners_pools_members_and_monitors` (mapping). On
> ttcp-dev with `moonlight-tls-test`: created policy `moonlight-test-redirect`
> (REDIRECT_TO_URL `https://www.example.com/` on the TERMINATED_HTTPS listener) and a PATH
> STARTS_WITH `/old` rule from the policy's menu; both went ONLINE / ACTIVE; *Edit L7 Policy*
> prefills everything; *Delete L7 Policy* worked, then the load balancer was deleted
> (cascade). The page has 13 calls with one listener and one policy. **Not done:** deleting
> container `moonlight-test-tls` and secrets `moonlight-test-cert` / `moonlight-test-key` (the
> session's permission check refused the delete; the user removes them). Follow-up: the
> detail page doesn't refresh while a policy or rule is PENDING (a reload shows ACTIVE).
>
> **Slice 5 and the follow-ups: done (2026-10-27).** The user dropped the provider select (no
> external providers on their clouds), so the wizard's details step gains only *Availability
> Zone* (`ChoiceSet::LbAvailabilityZones`, `mapping::lb_az_choices`: enabled Octavia zones by
> name) and *Enable Admin State*. The VIP subnet uses the new `ChoiceSet::LbVipSubnets`: the
> project's subnets and shared networks' (`subnets?shared=true`), like Horizon. *Admin State Up*
> on every create: `LbCreateSpec`, `LbListenerSpec`, `LbPoolSpec` and `LbMonitorSpec` gained
> `admin_up` (serde default `true`), sent only when off so default bodies are unchanged; boxes
> in the wizard's four steps, *Create Listener* / *Create Pool* (the existing edit box now also
> on create) and *Create Health Monitor* (`ops::admin_up_of`). Follow-ups: `ops::suggest_port`
> moves the listener port 80 ↔ 443 with HTTPS / TERMINATED_HTTPS (wizard and *Create
> Listener*), keeping a typed port; `model::lb_pending` and `LoadBalancerDetail::busy` make the
> load balancer list and detail page poll while anything is PENDING_* (*Updating…*). Tests:
> `wizard_sends_the_zone_and_admin_state_down` (actions), `lb_zones_are_the_enabled_ones_by_name`
> (mapping), `the_port_follows_the_protocol_until_typed` (ops),
> `a_pending_part_makes_the_load_balancer_busy` (model). On ttcp-dev (writes allowed by the
> user): the VIP list adds `public-dummy-alh1` and `t2c-cert-al-service-tenant`; zone `alh1`
> listed; `moonlight-test-monitor` created in `alh1` with an HTTP monitor; the list polled
> through PENDING_CREATE to ACTIVE / ONLINE without a reload; the detail's *Health Monitors*
> tab and the pool row show `moonlight-test-hm` (fetched by ID); *Create Listener* has *Enable
> Admin State* and moves to 443 for TERMINATED_HTTPS; then the load balancer was deleted.
> Still on ttcp-dev from slice 3: container `moonlight-test-tls` and secrets
> `moonlight-test-cert` / `moonlight-test-key`, for the user to delete.

**Goal.** Item 5 (slice 2) and item 53 give a working *Create Load Balancer* wizard and edit
dialogs, but only the basics: one port and weight for every member, members only from
instance addresses, no listener or pool options beyond protocol, port, algorithm and
persistence, no HTTPS termination and no L7 rules. Bring them to what octavia-dashboard offers
today (checked against its `master` source, October 2026:
`static/dashboard/project/lbaasv2/workflow/model.service.js` and `api/rest/lbaasv2.py`):

- **Load balancer**: name, description, VIP subnet and address, flavor, availability zone,
  provider, admin state.
- **Listener**: protocol (`HTTP`, `HTTPS`, `TCP`, `TERMINATED_HTTPS`, `UDP`, `SCTP`), port,
  connection limit, admin state; *Advanced*: the four timeouts (`timeout_client_data`,
  `timeout_member_connect`, `timeout_member_data`, `timeout_tcp_inspect`), `insert_headers`
  (`X-Forwarded-For`, `X-Forwarded-Port`, `X-Forwarded-Proto`), `allowed_cidrs`, and
  `tls_ciphers` (only for `TERMINATED_HTTPS`).
- **Pool**: name, description, algorithm, session persistence (+ cookie name for
  `APP_COOKIE`), admin state, `tls_enabled` (re-encrypt to the members) with `tls_ciphers`.
  For `TERMINATED_HTTPS` the pool protocol is `HTTP`.
- **Members**: a table, one row per member, each with its own port, weight, `backup`,
  `monitor_address` / `monitor_port`, admin state and name. Rows come from instance
  addresses (*Allocated / Available*) **and** external addresses typed by hand (IP + subnet),
  for back ends that aren't instances in this project. Editing a pool's members is one diff
  (add new, delete removed), like octavia-dashboard's `update_pool`.
- **Health monitor**: as today, plus admin state.
- **Certificates** (`TERMINATED_HTTPS` only, needs `key-manager` in the catalog): pick the
  listener's certificate from Barbican. octavia-dashboard lists certificate containers that
  have both `certificate` and `private_key`, plus opaque secrets (PKCS#12 bundles) not already
  in a container, each with its name and expiry; the first one becomes
  `default_tls_container_ref`. It sends no SNI list (`sni_container_refs` is always `None`);
  Moonlight could allow extra certificates as SNI.
- **L7 policies and rules** on a listener: policy (name, action `REDIRECT_TO_POOL` /
  `REDIRECT_TO_URL` / `REJECT`, position, redirect pool or URL) and its rules (type,
  compare type, key, value, invert). octavia-dashboard has them on the listener detail, not
  in the creation wizard.
- **VIPs on shared networks' subnets** (left over from item 5): the subnet choice for the VIP
  should include subnets of shared networks, as Horizon's does.

**Where Moonlight differs on purpose.** octavia-dashboard creates the load balancer, then waits
for `ACTIVE` in a background thread before creating the listener, pool, each member and the
monitor one by one (`poll_loadbalancer_status` chain); a failure halfway leaves a partial load
balancer. Moonlight keeps one fully populated `POST /lbaas/loadbalancers` (all or nothing; no
server-side polling thread). Octavia accepts every field above in that request, nested under
`listeners[].default_pool` (the SDK's `loadbalancer::create` builders have them all). Its
fields not offered by octavia-dashboard (`tls_versions`, `alpn_protocols`, HSTS, client
authentication) stay out unless asked for.

**Implementation**, in slices (each is shippable on its own, dry-run tested):

1. **Members.** A dialog field for rows of `(target, port, weight, backup, monitor address,
   monitor port)`: a new `dialog::Kind::Rows` (like `KeyValue`, value a JSON list) or an
   extension of `Pick` with per-row inputs; decide when building. External rows take an IP and
   a subnet (`ChoiceSet::Subnets`). `LbMemberSpec` gains `monitor_address`, `monitor_port`,
   `admin_up`; `member_dialog` accepts an external address too. Editing the member list: new
   `OsAction::PoolSetMembers { pool_id, members }` with Octavia's batch update,
   `pool::member::replace` (`PUT /lbaas/pools/{id}/members`: matches existing members by
   address and port, creates the new ones and **deletes the ones left out**, so it is a
   destructive dialog that lists the removals). Tests: per-row ports in the wizard body,
   external member, the batch body.
2. **Listener and pool options.** `LbListenerSpec` / `LbPoolSpec` gain the fields above;
   `Lb*Spec::check` validates them (timeouts ≥ 0, CIDRs parse, headers only for HTTP-based
   protocols, ciphers only with TLS). Dialog: an *Advanced* group (`.when("advanced",
   "true")`) in the wizard steps and in `listener_dialog` / `pool_dialog`. `ListenerUpdate` and
   `PoolUpdate` are JSON requests already: add the fields there. Gate by the Octavia API
   version (timeouts 2.1, `tls_enabled` 2.8, `allowed_cidrs` 2.12): detection doesn't read
   Octavia's version yet, so add `Capabilities::lb_mv` in `os::capabilities` (version document
   of the `load-balancer` endpoint) and `Requires::LbMv(major, minor)` in `model::needs`.
3. **TLS termination.** `TERMINATED_HTTPS` in `LB_PROTOCOLS` only when `key-manager` is in the
   catalog (`Requires::Service("key-manager")`). New `ChoiceSet::LbCertificates`: Barbican has
   no crate in the SDK, so raw `JsonRequest::get` calls (GET only, `fetch.rs`) to
   `/v1/containers?type=certificate` and `/v1/secrets?secret_type=opaque`, merged like
   octavia-dashboard, with the expiry in the choice's cells. `default_tls_container_ref`
   (+ optional `sni_container_refs`) in the listener spec and edit dialog. **Open
   questions:** does the cloud's Octavia use the `barbican_acl` certificate manager (Octavia
   then grants itself read access), or must the user add an ACL first? Check on the test cloud
   before promising it works. Creating certificates/secrets belongs to the Key Manager panels
   (item 55), not here.
4. **L7 policies and rules.** Listener detail (or an *L7 Policies* tab on the load balancer
   detail): list via `load_balancer::v2::l7policy` (+ its rules), create/edit/delete
   `OsAction`s with dry-run tests; position, action-dependent fields with `.when`.
5. **Small ones.** VIP subnets of shared networks (`ChoiceSet::Subnets` variant or a
   `LbVipSubnets` set using `merge_by_id` like the network list); availability zone and
   provider selects in the details step (`load_balancer::v2::availability_zone`,
   `provider`); admin state everywhere.

Verify each slice in the browser **in dry-run only**, checking the request preview against the
bodies above.

## 55. Key Manager (Barbican): Secrets and Containers panels

> ✅ **Done.** Opt-in plugin `key-manager` (`model::Plugin::KeyManager`, `needs::KEY_MANAGER`):
> a *Key Manager* group (the label the user chose over *Barbican*) with *Secrets* and
> *Containers* (`components/nav.rs`), refused by every server function unless enabled.
> Requests are `JsonRequest::key_manager` (`ServiceType::Other("key-manager")`, paths prefixed
> with `v1/`, no version discovery: the SDK then uses the catalog URL as is; a catalog that
> lists Barbican with `/v1` already is not supported). Lists page with `limit`/`offset` until
> `total` (`fetch::km_all`, `mapping::next_offset`, 100 per page, at most 50 pages); IDs come
> from refs (`mapping::ref_id`). Pages: lists in `pages/lists.rs` (expiration flagged within 30
> days or past, `model::expiry`), details in `pages/key_manager.rs` (secret: attributes, ref,
> `read` ACL, consumers or "not reported" where the API is missing; container: secrets by role
> linked to their pages, consumers). Writes: `SecretCreate` (text or pasted base64, checked
> before sending; empty payload = metadata only), `SecretDelete` (the detail page's dialog
> lists the consumers, the row and bulk ones warn), `ContainerCreate` (`ChoiceSet::Secrets`
> gives refs; roles per type checked on the server), `ContainerDelete` (says the secrets stay).
> *Show Payload* behind `[key_manager] allow_payload` (default off, the user's choice):
> `get_secret_payload` is a POST with `Cache-Control: no-store`, asks Barbican in the secret's
> content type, base64 for binary, logs only that it was read. Sensitive text is
> `model::Hidden` (no `Debug`, `zeroize` on drop, also in the browser); `JsonRequest` bodies
> wipe their `SECRET_KEYS` values on drop, `payload` is masked in previews. Tests: mappers
> (refs, two-page offsets, missing names, `null` dates, ACL defaults), dry-run bodies of the
> four actions, masking and wiping, expiry, container roles, settings. **Lesson:** the first
> detail pages, one large nested `view!` with tables in `match` arms, hung `rustc` for both
> targets (AGENTS.md §6); small components with columns built outside `view!` fixed it.
> **Verified on ttcp-dev** (project admin2, writes on): the user stored a text secret and a generic
> container through Moonlight (`failed=0`, the log names only the action); Secrets list 1 call 0.06
> s, secret detail 3 calls 0.55 s (consumers API present, empty), Containers list and detail 2 calls
> 0.04 s, the container's secret linked by role; the pages send only GETs. `v1/` on the unversioned
> catalog URL works. Not tried: *Show Payload* (`allow_payload` off), certificate/RSA containers,
> deletes.
> **Follow-up (user review):** *Show Payload* now asks first (a modal warning that the payload is
> sensitive; only confirming fetches it), the card sits in the card grid, and *Hide Payload* works:
> the card is one `Reveal` state (closed, confirm, loading, shown, failed) in its own `.payload`
> body instead of the `Card`'s `<dl class="kv">`; the old version's exact cause was not pinned down.
> *Store Secret* has help text on every field (type, format, algorithm, bit length, mode,
> expiration). Verified in the browser: modal, cancel, confirm, hide.
> **Left:** a file picker for binary payloads; orders (item 58), ACL editing (item 75); quotas
> and CAs are out of scope.

**Goal.** A *Key Manager* panel group with *Secrets* and *Containers*, for clouds that run
Barbican (`key-manager` in the catalog). Users store certificates, keys and passphrases there
and other services consume them: Octavia's `TERMINATED_HTTPS` listeners (item 54, slice 3),
Cinder encrypted volume types, Nova/Glance image signing. The production Horizon has no Key
Manager panel (item 5's review), so this is for other clouds; build it before item 54's TLS
slice, which needs certificates to pick from.

**About the reference plugin.** [barbican-ui](https://github.com/openstack/barbican-ui)
(`master`, September 2026) is **not a working implementation**. It is the Horizon plugin
cookiecutter, never filled in: its API client (`barbican_ui/api/client.py`) never calls
Barbican and keeps records in an in-memory dict (`STUB_DATA`), and the table, detail and form
still use the template's sample attributes (`size`, `temperature`, `base`, `flavor`,
`topping`). Take only its **layout**:

- a panel group of its own (`PANEL_GROUP = 'barbican'`) with one panel, *Secrets*;
- an Angular registry table, name linked to the detail page, sorted by name, with a row
  drawer (summary) and *Created* / *Updated* columns;
- a detail page with property groups (identity; attributes; ID and dates);
- actions: *Create* (a workflow with an info step), *Update*, *Delete* (also bulk).

The fields, filters and actions come from the **Barbican v1 API** instead. Keep Horizon's
naming: Horizon's own docs and octavia-dashboard call the service *Key Manager*. **Open
question:** label the group *Key Manager* (proposal) or *Barbican* like the plugin.

**What the user gets.**

- *Secrets* list: Name (ID when unnamed), Type (`symmetric`, `public`, `private`,
  `passphrase`, `certificate`, `opaque`), Algorithm / bit length / mode, Content type,
  Status, Expiration (warning style when within 30 days, red when past), Created. Search by
  name; filter by type.
- Secret detail: every attribute, the secret's `secret_ref`, its ACL (users with read access,
  `project-access`) and its consumers (who uses it: e.g. an Octavia listener).
- *Store Secret*: name, type, payload (text, or file for binary; base64 sent with
  `payload_content_encoding: base64`), content type, algorithm, bit length, mode, expiration.
  *Delete Secret* (bulk too), a destructive dialog listing the secrets **and their
  consumers**: deleting a secret in use breaks the consumer.
- *Containers* list and detail: Name, Type (`generic`, `certificate`, `rsa`), the secrets in
  it by role (`certificate`, `private_key`, `intermediates`, `private_key_passphrase`, linked to
  their detail), consumers. *Create Container* (pick secrets per role; for `certificate` the
  roles Octavia needs) and *Delete Container* (the secrets stay; say so in the dialog).
- Not now: orders (asymmetric key generation by Barbican), quotas, editing ACLs (show them
  read-only first), CAs.

**Implementation.**

- Opt-in plugin like Octavia and Heat (AGENTS.md §4): `model::Plugin::KeyManager`
  (`key-manager`), panels `requires` `Requires::All(&[Requires::Plugin(KeyManager),
  Requires::Service("key-manager")])` in `components/nav.rs`, `plugin_enabled(p)?` in every
  server function, its actions in `OsAction::plugins`.
- No SDK support: `openstack_sdk_core` has no `ServiceType` for it (use
  `ServiceType::Other("key-manager".into())`) and there is no Barbican API crate. Reads are
  `JsonRequest::get` in `fetch.rs` (GET only), writes `JsonRequest::new` through `Exec`. Check
  version discovery: Barbican's catalog endpoint is often unversioned (`https://…:9311`); if
  `discover_service_endpoint` can't handle `Other`, prefix paths with `v1/`.
- **Pagination differs from the rest of OpenStack**: `limit` / `offset` (not `marker`), default
  `limit` is **10**, and the response carries `total` and `next`. A small helper in `fetch.rs`
  loops on `offset` until `total`; a mapper test with a two-page response.
- IDs: Barbican returns `secret_ref` / `container_ref` URLs; the ID is the last path segment
  (`mapping::ref_id`, tested with and without a trailing slash).
- Reads: `GET v1/secrets`, `v1/secrets/{id}` (metadata only), `v1/secrets/{id}/acl`,
  `v1/secrets/{id}/consumers` (newer Barbican; `soft()` when missing), `v1/containers`,
  `v1/containers/{id}`. Models `SecretRow`, `SecretDetail`, `ContainerRow`,
  `ContainerDetail`; pure mappers with Barbican-shaped JSON (missing name, `expiration: null`,
  `content_types: {"default": …}`).
- Writes: `OsAction::{SecretCreate, SecretDelete, ContainerCreate, ContainerDelete}` with
  dry-run tests asserting the exact bodies. `payload` must be added to `SECRET_KEYS` in
  `actions.rs` so request previews mask it, and the payload must never be logged.
- **Reading a payload** (`GET v1/secrets/{id}/payload`) puts secret material in the browser.
  **Open question:** offer *Show / Download Payload* at all? Proposal: behind
  `[key_manager] allow_payload` (default `false`), fetched only on click, shown once like
  `ActionOutcome.secret`, never cached.
- i18n for every label (types and roles humanised), both languages. Verify in the browser
  **in dry-run only** on a cloud with Barbican; the production cloud has none.

## 56. Images list: no *Owner* column; a row drawer with the image's metadata, as Horizon

> ✅ **Done (2026-10-26).** `DataTable` has an optional `drawer: Option<DrawerFn<T>>` prop
> (`components/table.rs`): a first column with a chevron `<button aria-expanded>` (label *Show
> details* / *Hide details*; 12 px, `--ink-3` at 70 % opacity, turns down when open: the user
> asked for it to be subtle) and, for an open row, a `<tr class="drawer">` spanning every column.
> Open drawers are kept by row index in an `RwSignal`, so a sort keeps them; an `Effect` closes the
> ones the filter hides (`retain_visible`), and a refetch rebuilds the table, closing all. Each
> drawer is its own reactive block, so a toggle doesn't re-render the table and keeps keyboard
> focus. `list_page!` takes `drawer = fn` (like `busy = …`). The Images list drops the *Owner*
> column and passes `lists::image_drawer`: **metadata only** (custom properties sorted by key, or
> *No metadata.*; empty values show `-`); the user dropped ID, Owner and Min. Disk/RAM from the
> proposal below. The owner name and ID are in the filter's search string. CSS:
> `td.drawer-toggle`, `.btn-drawer`, `tr.drawer`, `.drawer-kv` (tokens only, so dark mode
> follows). i18n `ShowDetails`, `HideDetails`. Test:
> `drawers_stay_open_across_sorts_and_close_when_filtered_out` (`table.rs`).
> Both clippy variants clean, 240/240 tests. Browser (ttcp-dev, *admin2*, 31 images, 2 API ·
> 0.30 s): click opens/closes, Tab + Enter toggles and keeps focus with the label switching, a
> descending sort keeps the same image's drawer open, filtering it out closes it, dark mode
> reads well, no console errors. *No metadata.* wasn't seen (every image there has properties).
> **Open:** adopting the drawer on other lists (volumes, snapshots) if wanted.

**Goal.** Two changes to Project › Compute › Images, asked for by the user:

1. **Drop the *Owner* column** (`lists.rs`, `ImagesPage`, `col(K::Owner, … owner_cell)`). It
   takes width on every row for something rarely needed. The owner stays available: in the
   drawer below and on the image detail page (*Owner* row).
2. **A small toggle at the start of each row** (a chevron, ▸ / ▾) that opens a **drawer** under
   the row, like Horizon's Angular images table. Horizon's drawer shows a few properties and the
   image's metadata (custom properties) as a key/value list. Check the exact fields on the
   shared Horizon tab (read-only, AGENTS.md §1.4) and keep its labels; the proposal: *ID*,
   *Owner* (project name, else ID), *Min. Disk*, *Min. RAM*, then every custom property
   (`Image.properties`), sorted by key, or *No metadata.* when there is none. Several drawers
   can be open; they close when the list refreshes or the filter changes the rows.

**Implementation.**

- No new API calls: list rows already carry `owner_id` / `owner_name`, `min_disk_gb`,
  `min_ram_mb` and `properties` (`mapping::images`, roadmap 25).
- `components/table.rs`: an optional `DataTable` prop `drawer: Option<fn(&T, Lang) -> AnyView>`.
  When set, a narrow first column holds a `<button aria-expanded>` per row (keyboard-reachable,
  label *Show details* / *Hide details*), and an open row gets a second `<tr class="drawer">`
  with one cell spanning every column. Open rows are kept by row key in a signal, so sorting
  doesn't close them. Other lists can adopt it later (volumes, snapshots).
- `list_page!` gets an optional `drawer = fn` argument, like `busy = …` (roadmap 46), passed to
  `DataTable`. `ImagesPage` passes `image_drawer` (in `lists.rs`): a `Kv` grid of the fields
  above, values with `dash()`, sizes with `bytes`/`GiB`.
- Keep `owner_cell` for the drawer, and add the owner name to the search string (today:
  name, ID, status, visibility), so a user can still filter by owner without the column.
- i18n: `ShowDetails`, `HideDetails` (both languages); the rest reuses existing keys.
- Tests: `table.rs` (pure part: which rows are open after a re-sort, keyed by row key);
  `image_drawer` content for an image with and without properties (SSR render to string, or a
  pure `fn drawer_rows(&Image) -> Vec<(K, String)>` tested instead). Browser check on ttcp-dev:
  open, close, keyboard (Tab + Enter), sort with a drawer open, dark mode.

## 57. Identity dashboard (Projects, Users, Groups, Roles, Application Credentials)

> ✅ **Done (2026-10-26).** All five slices; the notes below say what was built and checked.
> Tested for real on ttcp-dev by the user: *Create Role*, *Create Project*, creating and deleting
> an application credential (with the time zone test). Not yet sent for real: editing or
> deleting projects and roles, users, groups, member and group assignments (all dry-run tested
> and opened in the browser). System-scoped clouds moved to *Also pending*.
>
> **Fixed after closing (2026-10-26):** the user's *MyTest* project, created from *ttcp-spain*,
> didn't show in the list: without `domain_id`, Keystone puts a project created with a
> project-scoped token in its default domain (`Default`), not the user's. Projects, users and
> groups now always carry `domain_id`: `run_action` sets it (`OsAction::needs_domain` /
> `set_domain`, whatever the browser sent) to the session's domain context, else the user's
> domain (`fetch::operation_domain`, a GET of `users/{me}`, so also in dry-run), as Horizon's
> `get_domain_id_for_operation`. Tests: `creations_are_put_in_the_users_domain` and the request
> bodies in `identity_admin_requests_are_horizons`. The *MyTest* project left in `Default` must be
> removed outside Moonlight (it lists only the user's domain).
> *Create Project*, *Create User* and *Create Group* now start with the domain, read-only, as
> Horizon's *Domain Name* / *Domain ID* (`Kind::Info` over `ChoiceSet::OperationDomain`, answered
> by `api::get_choices` with the same `fetch::operation_domain_ref` that `run_action` uses, so
> what is shown is where it goes): *ttcp-spain* and its ID in the browser. Test:
> `identity_create_dialogs_show_where_things_go`.
>
> Earlier progress notes (2026-10-26), kept for reference:
>
> **How Horizon decides, from its source** (`openstack_dashboard/dashboards/identity/*/panel.py`,
> `horizon/base.py`, `openstack_auth/policy.py`, checked on master): each panel's
> `policy_rules` are evaluated against Horizon's bundled default Keystone policy with the scoped
> token's roles, and a panel shows when **any** of its rules passes (an OR, says `base.py`). The
> target is always the signed-in user (`user_id`, the user's `domain_id`); the credentials carry
> no domain or system scope. So with a project-scoped token: Projects (no rule) for everyone,
> with the domain's projects when `identity:list_projects` passes (admin) else the user's own
> (`list_user_projects`) and *Set as Active Project*; Users for everyone (`get_user` passes as
> the owner), listing every user only with `list_users` (admin) else the user alone; Groups and
> Roles for admins; Application Credentials for everyone (owner); Domains only with a
> domain-scoped token; federation off. Confirmed on ttcp-dev's Horizon: in *admin2* (admin) the
> panels are Projects, Users, Groups, Roles, Application Credentials; in *pokemon*
> (member/power_user) Projects (the user's 6), Users (one row, themself), Application
> Credentials, and no Admin dashboard. That Horizon has no *User Credentials* panel (older
> than master).
>
> **Built:** `nav::IDENTITY_NAV` (one group, Horizon's order; Groups and Roles need
> `needs::IDENTITY_ADMIN` = `AnyRole(["admin"])`), `nav::dashboard(path)` picks the menu and
> breadcrumb (*Identity › Projects*), the top bar links *Identity* (active on `/identity`) and
> hides the disabled *Admin* from non-admins. Routes `/identity/{projects,users,groups,roles,
> application_credentials}` (`/identity` redirects to Projects) in `app.rs`; pages in
> `pages/identity.rs` (`identity_page!`). Server fns `list_identity_{projects,users,groups,
> roles}` and `list_app_credentials` (`api.rs`, admin = the scoped token's `admin` role via
> `os::capabilities`) → `fetch::identity_*` (GET only: `users/{me}` for the domain, `domains`
> soft, `projects?domain_id` or `users/{me}/projects`, `users?domain_id`, `groups?domain_id`,
> `roles`, `users/{me}/application_credentials`; an admin list refused with 403 falls back to the
> user's own) → `mapping::identity_*` (`enabled` absent = true, empty description = none,
> nameless → ID, domain names from the domain list plus the session's). *Set as Active Project*
> is a new `MenuItem::action` (`ItemKind::Run`, enabled even with writes off: it only changes
> Moonlight's session) calling `layout::set_active_project`, shared with the header menu. Tests:
> `identity_lists_follow_keystone_quirks` (`mapping.rs`), `identity_admin_panels_need_the_admin_role`
> (`nav.rs`). Both clippy variants clean, 252/252 tests.
>
> **User detail** (`/identity/users/:id`, the Users name links to it; `get_identity_user` →
> `fetch::identity_user` → `mapping::identity_user_detail`): Horizon's overview, field for field
> (Name, ID, Domain Name, Domain ID, Description, Email, Enabled, Password Expires At, Lock
> password from `options.lock_password`, Primary Project named when known, other attributes as
> extra rows). The deployed Horizon shows no tabs there for a member; master also has Role
> assignments, Groups and Credentials tabs (left for slice 2). `DetailHeader`'s breadcrumb now
> follows the dashboard (*Identity › Users › admin2*).
>
> **Application credentials (slice 3):** *Create Application Credential* (`ops::
> create_app_credential_dialog`, Horizon's fields: name, description, secret (empty = generated),
> expiration date and time (`model::app_credential_expiry`: a date alone is midnight, a time alone
> today, UTC) with themed pickers (`components/datetime.rs`, `Kind::Date` / `Kind::Time`: a month
> calendar with past days disabled, and hour/minute columns; typing still works; chosen over the
> browser's inputs by the user), roles from the new `ChoiceSet::TokenRoles` (the token's roles as
> Keystone spells them; none = all), access rules (a JSON or YAML list, Horizon's help and examples
> under the field via `Field::help_code`; parsed on the server by `actions::parse_access_rules` with
> `yaml-rust2` and checked against Keystone's schema in `AccessRule::check`), *Unrestricted
> (dangerous)* with a red label (`Field::danger`); the Kubernetes namespace not offered), bulk and
> row delete. `OsAction::AppCredentialCreate` / `AppCredentialDelete`; `run_action` sets `user_id`
> from the session (`OsAction::set_user`, also inside a batch), so users manage only their own. The
> secret comes back once in `ActionOutcome.secret` (`SecretKind::AppCredential`, `Secret.fields`
> shown with copy buttons, `Secret.extra` downloads); `secret` is masked in request previews
> (`SECRET_KEYS`) and never in `Debug`.
>
> **Download templates** (asked for by the user): `openrc` and `clouds.yaml`, for API Access and
> for a new application credential, are rendered on the server (`server/downloads.rs`,
> `minijinja`) from built-in templates in `server/templates/`, replaceable with `[api_access]
> openrc_template`, `clouds_yaml_template`, `app_credential_openrc_template`,
> `app_credential_clouds_yaml_template` (Horizon's `OPENRC_CUSTOM_TEMPLATE` /
> `OPENSTACK_CLOUDS_YAML_CUSTOM_TEMPLATE`). Horizon's variable names, its `shellfilter` and
> `{% load %}` are accepted, so its templates work unchanged; each template is compiled and
> rendered once at start-up (an error stops it). `Credentials` now carries the rendered files.
> Documented in `CONFIGURATION.md` and `moonlight.conf.example`. Also fixed: password inputs had
> no field styling (the *Secret* field here, *Launch*'s admin password, *Rescue*).
>
> Tests added: `user_detail_reads_options_primary_project_and_extras`,
> `application_credentials_are_created_with_the_secret_masked_and_deleted`, `expiry_follows_
> horizon`, `identity_actions_are_pointed_at_the_signed_in_user`, the `downloads` tests (the old
> `openrc`/`clouds.yaml` expectations, application credential files, a Horizon template
> unchanged, a broken template and a missing file refused), and the secret `Debug` test now
> covers `fields`/`extra`, `access_rules_are_read_as_json_or_yaml_like_horizon`, and the picker
> logic (`datetime.rs`: month grid, month stepping, hour/minute). Both clippy variants clean,
> 266/266 tests (now also `project_members_join_direct_and_group_roles`,
> `roles_are_created_renamed_and_deleted`, `utc_values_say_so`).
>
> **Time zone test (2026-10-26, a real create, authorized by the user).** Browser in
> Europe/Madrid (UTC+1). `moonlight-tz-test` (role `member`, description "safe to delete") was
> created with expiry 2026-10-27 03:15: sent `"expires_at": "2026-10-27T03:15:00"` (no zone:
> Keystone reads UTC), listed by Keystone/Horizon as `2026-10-27T03:15:00.000000`, by Moonlight
> as `2026-10-27 03:15` — consistent, all UTC. The secret box showed name, ID and secret with
> the three downloads (Horizon's templates; the secret never in the request preview). Fixes from
> the test: the Expiration column (and *Password Expires At*) now says `UTC` (`widgets::utc_time`)
> so a reader in another zone doesn't take 03:15 for local time; `app_credential_expiry` takes
> the current UTC time and refuses an expiry not later than now (Horizon sends those: a date
> alone is 00:00 UTC, so *today* alone was already past; a time alone that already passed);
> after a failed action the dialog shows *Back* (to the form, values kept) and *Close* instead
> of only × (every dialog). A *Now* button left of the clock fills the current UTC time (asked
> for by the user). The first click on *Create* once did nothing; not reproduced since. The
> user then deleted the test credential from the list (delete tested for real, OK).
>
> **Project detail** (asked for by the user): the name links to `/identity/projects/:id` when
> Horizon's `identity:get_project` would pass (admins, or the project signed in to);
> `get_identity_project` → `fetch::identity_project` → Overview (Name, ID, Domain Name, Domain ID,
> Enabled, Description) and, for admins, Users (direct roles, and *Roles from Groups* as
> `role (group)`: `role_assignments?scope.project.id&include_names` plus each group's users in
> parallel, `mapping::project_members`) and Groups tabs. **Roles writes** (asked for by the user,
> part of slice 4): *Create Role*, per row *Edit* (the name) and *Delete Role* (warns that its
> assignments go too), bulk *Delete Roles*; `OsAction::RoleCreate` / `RoleUpdate` / `RoleDelete`
> (`identity::v3::role::{create, set, delete}`), dry-run tested. *Create Role* tested for real by
> the user on ttcp-dev (OK); *Edit* and *Delete Role* only opened, not submitted.
> Identity writes follow the general `[writes] mode`, like every other write (decided below).
>
> **Browser (ttcp-dev, writes on, nothing submitted).** As a member (admin2 with only
> `power_user`): panels Projects, Users, Application Credentials, no *Admin*; Users lists only
> admin2 and its name opens the overview (5 API · 0.11 s) with the same values as Horizon's;
> API Access's downloads render from the templates. As an admin: Projects 123 (4 API · 0.23 s,
> *Set as Active Project* only on the user's other project), Users 144 (3 API · 0.20 s), Groups
> 316 (3 API · 0.22 s), Roles 7 (1 API · 0.05 s), *Admin* shown. Create dialog: the roles are
> the token's (`admin`, `member`, `reader`), the calendar flips up near the bottom with past days
> disabled and sets `2026-10-29`, the time picker sets `03:15` and closes, Access Rules' help
> and examples show, *Unrestricted* is red. **Not seen:** a real create and its secret box
> (writes are on). A server panic ("`sandboxed-arenas` is
> active, but no Arena is active") was logged right after two restarts during the hung builds
> and not since (6 restarts).
>
> **Items 1–3 of what was left (2026-10-26, built, tested and checked in the browser):**
>
> - **Reads.** A user's *Role assignments* (scope with project link, role, inherited:
>   `role_assignments?user.id&include_names`, `mapping::role_assignment_rows`) and *Groups* tabs for
>   admins; a group page (`/identity/groups/:id`: overview and members, the list's name links to
>   it); an *Implied Roles* column (`role_inferences`); the application credential page
>   (`/identity/application_credentials/:id`: overview and access rules); on the current project's
>   page, *Quotas* links to the Overview.
> - **Admin writes** (`OsAction`s with dry-run tests asserting each request): projects
>   (`ProjectCreate` in the user's domain, see the fix in the ✅ note, `ProjectUpdate` with enabled,
>   `ProjectDelete`, bulk delete; the current project can't be ticked), *Manage Members* / *Modify
>   Groups* on the project page (`components/assignment_rows.rs`, `Kind::Assignments`: members with
>   a checkbox per role, an *Available* list; `model::assignment_changes` sends only the grants and
>   revokes that changed, `ProjectRoleGrant` / `ProjectRoleRevoke`, as a batch), users (`UserCreate`
>   as JSON for Keystone's `email`, with password twice, primary project and a role granted on it
>   after the create, enabled, lock password; `UserUpdate`, `UserSetPassword` (masked),
>   `UserSetEnabled`, `UserDelete`, bulk delete), groups (`GroupCreate` / `Update` / `Delete`, bulk
>   delete, *Manage Members* as a Pick over users sending `GroupAddUser` / `GroupRemoveUser`).
>   Choice sets `IdentityRoles`, `IdentityUsers`, `IdentityGroups`, `IdentityProjects`.
> - **Safety rules** in `run_action` (`OsAction::identity_safety`, also inside batches): no deleting
>   or disabling one's own user or the current project, no removing one's own roles on it (stricter
>   than Horizon's `admin`-only rule: telling roles apart needs a lookup dry-run can't make).
> - **Domains** (`[keystone] multidomain`, Horizon's `OPENSTACK_KEYSTONE_MULTIDOMAIN_SUPPORT`;
>   `Requires::MultiDomain` doesn't fail open; `needs::DOMAINS` adds the admin role): the Domains
>   panel first in the menu, *Set Domain Context* per row and *Clear Domain Context*
>   (`set_domain_context`, stored in the session cookie as `domain_context`), which scopes the
>   Projects, Users and Groups lists (`fetch::list_domain`). Off on ttcp-dev (its admins can't list
>   domains anyway), so unverified there.
>
> Tests added: `role_assignments_name_their_scope`, `domains_mark_the_context`,
> `identity_admin_requests_are_horizons`, `nobody_locks_themselves_out`,
> `member_changes_are_only_what_changed`, `domains_need_the_setting_even_when_detection_failed`,
> implied roles in the identity list test. Both clippy variants clean, 272/272 tests.
>
> **Browser (ttcp-dev, admin2 in *admin2*, roles admin/member/reader; nothing submitted):**
> Users 144 with *Create User* and *Delete Users*; a user's menu (Edit, Change Password, Disable
> User, Delete User) and tabs (`joseluistestmember`: no direct assignments, group
> GR_TEF_Operators, 7 API · 0.22 s); the group page (4 API · 0.14 s) and its *Manage Members*
> (1 allocated, 143 available); Roles' implied column (admin → member, member → reader); Projects
> with *Create Project*, the current one not selectable, *Quotas* → Overview, *Manage Members*
> (admin and admin2 with `admin`) and *Modify Groups*; *Create User* refuses different passwords
> before sending and shows *Role* only with a primary project; Domains hidden (`multidomain`
> off); no console errors. Fixed from the check: the member pickers listed every domain's users
> (157, service users included): `ChoiceSet::Identity{Users,Groups,Projects}` now take the
> current project's domain (`fetch::project_domain`), 143 users as Horizon; the *Available*
> list now uses the Allocated/Available picker's layout, as a group's *Manage Members* (asked for by
> the user): *Allocated* with a checkbox per role and ↓, *Available* with its filter and ↑, each in
> the picker's scroll area (260 px) inside the dialog; every available row is listed (the
> 200-row cap is gone). Checked and approved by the user in the browser.
>
> **Decided (2026-10-26, by the user):** no separate `[identity] writes` switch; identity
> changes go through `Exec` and the general `[writes] mode` (`off` / `dry-run` / `on`) like every
> other write.
>
> **Left after closing the item:** see the ✅ note at the top (untested real writes) and *Also
> pending* (system-scoped clouds).

**Goal.** The top bar shows *Identity* greyed out (*Coming soon*, `components/layout.rs`).
Make it a dashboard like Horizon's Identity, with its panels and what each user may see:

- **Projects**: name, description, project ID, domain, enabled. Admins see every project of the
  domain; other users see their own (Horizon does the same). Detail: members (users and groups
  with their roles) and quotas link to the Overview when it's the current project.
- **Users** (admin): name, email, user ID, domain, enabled, password expiry. Detail: projects
  and roles (role assignments), groups.
- **Groups** (admin): name, description, members.
- **Roles** (admin): name, ID, implied roles.
- **Application Credentials** (every user, their own): name, project, description,
  expiration, roles, unrestricted. Create returns a **secret shown once**, like a generated key
  pair (item 7), plus Horizon's downloads (`openrc` and `clouds.yaml` for that credential).
- **Domains** (admin, multi-domain clouds only): list and *Set Domain Context* as Horizon.
- Not planned: identity providers, mappings and protocols (federation), which Horizon shows
  only when WebSSO is configured (dropped from item 80 by the user).

Keep Horizon's panel order, columns, breadcrumbs (*Identity › Projects*) and action names.

**Implementation**, in slices (read-only first; each slice dry-run tested):

1. **Navigation.** `components/nav.rs` models one dashboard (Project); add a second, *Identity*,
   with its own route prefix `/identity/...` in `app.rs` and its panels' `requires`. The top
   bar's *Identity* becomes a link, active on `/identity`. Gating: Keystone's policy can't be
   read through the API, so admin panels use `Requires::AnyRole(&["admin"])` (the scoped
   token's roles, `os::token_roles`) and still handle a 403 with the friendly forbidden page
   (item 14), never a broken page. *Projects* and *Application Credentials* show for everyone.
2. **Reads.** `fetch.rs` (GET only) with the SDK's `identity::v3` modules: `project::list`
   (admins; filtered by `domain_id`) or `user::project::list` for the user's own (try the
   admin list, fall back on 403), `user::list`, `group::list` + `group::user::list`,
   `role::list` + `role_inference`, `role_assignment::list` with `include_names`, and
   `user::application_credential::list`. Models and pure mappers in `model.rs` /
   `mapping.rs` with Keystone-shaped JSON tests (missing email, `password_expires_at: null`,
   `enabled` absent = true). Server fns `list_identity_*` / `get_identity_*` in `api.rs`.
   Pages in a new `pages/identity.rs` with `list_page!` and `DetailHeader`.
3. **Application credentials (writes any user can do).** `OsAction::{AppCredentialCreate,
   AppCredentialDelete}`: `user::application_credential::create` (name, description,
   expires_at, roles subset, unrestricted, access rules left out) returns `secret` once →
   `ActionOutcome.secret`, never logged, previews mask `secret` (add it to `SECRET_KEYS`).
   Downloads built from the session like *API Access* (item 24), with
   `auth_type: v3applicationcredential`.
4. **Admin writes.** Projects (create, edit, enable/disable, delete), users (create with a
   password typed by the admin, edit, change password, enable/disable, delete), groups
   (create, edit, add/remove users), role assignments (*Manage Members* on a project:
   user/group × role, a diff like *Edit Security Groups*). Horizon's safety rules, enforced in
   `apply` and not only in the dialogs: an admin can't delete or disable their own user or the
   project they are signed in to, and can't remove their own `admin` role on it. Every delete
   is a destructive dialog that lists the affected resources.
5. **Domains** when `[keystone]` multi-domain is in use (`domain::list`), *Set Domain Context*
   stored in the session cookie, scoping the identity lists.

**Open questions.**

- Clouds that enforce Keystone **system scope** (`[oslo_policy] enforce_scope = true`) refuse
  user/role admin calls with a project-scoped token, even for admins. Check how the test cloud
  is configured; supporting it means a second, system-scoped connection in `os.rs`.
- ~~**Identity writes on a shared cloud**: an extra `[identity] writes` switch?~~ Decided
  (2026-10-26): no; identity writes follow the general `[writes] mode` like every other write.
- Verify every slice in the browser on the test cloud; production stays dry-run.

**Settings that come with it** (moved from item 29): the Keystone admin settings
(`OPENSTACK_KEYSTONE_ADMIN_ROLES`, default role, domain admin, `ENFORCE_PASSWORD_CHECK`…).

## 58. Key Manager: *Orders* (Barbican generates a key or an RSA key pair)

> 🔗 **Issue:** [#3]

**Goal.** Let users ask Barbican to generate secret material instead of uploading it: an
*Order Key* dialog for a symmetric key (`type: key`, stored as a secret) or an RSA key pair
(`type: asymmetric`, stored as an `rsa` container with `public_key`, `private_key` and an
optional `private_key_passphrase`), and a read-only *Orders* panel. Low value, hence P3: Octavia's
TLS listeners need CA-signed certificates, which users upload (*Store Secret*, *Create
Container*, item 55); Cinder's encryption keys are created by Cinder itself. Neither Horizon
nor barbican-ui offers orders. **Certificate orders** (`type: certificate`, CA plugins) are
deprecated and rarely deployed: not offered.

**Implementation.**

- Same plugin and gating as item 55 (`Plugin::KeyManager`, `needs::KEY_MANAGER`); a third
  panel *Orders* in the *Key Manager* group (`/project/key_manager/orders`).
- Reads (`fetch.rs`, GET only): `GET v1/orders` through `km_all` (same `limit`/`offset`/`total`
  pages), `GET v1/orders/{id}`. Model `OrderRow`: ID from `order_ref` (`mapping::ref_id`),
  `type`, `status` (`PENDING`, `ACTIVE`, `ERROR`), `meta` (name, algorithm, bit length, mode,
  expiration), `error_status_code` / `error_reason`, `created`, and the result's
  `secret_ref` or `container_ref`, linked to its detail page. Mapper test with Barbican-shaped
  JSON (pending, active with a ref, error with a reason).
- Orders run asynchronously: poll the list while any order is `PENDING` (`list_page!`'s
  `busy`, roadmap 46).
- Writes: `OsAction::OrderCreate(OrderSpec)` → `POST v1/orders`
  `{"type": "key"|"asymmetric", "meta": {name, algorithm, bit_length, mode, expiration,
  payload_content_type}}`, `create` returning the ID from `order_ref`; `OsAction::OrderDelete`
  → `DELETE v1/orders/{id}` (deletes the order record only; the generated secret or container
  stays, say so in the dialog). Dry-run tests asserting both bodies and paths; server-side
  checks of type, algorithm (`aes` for keys, `rsa` for key pairs) and bit length
  (128/192/256 for AES; 2048/3072/4096 for RSA).
- *Order Key* dialog in `ops.rs`: type, name, algorithm (follows the type), bit length, mode
  (`cbc` for keys), expiration (reuse `app_credential_expiry`); also a *Create* button on the
  Secrets and Containers lists, or only on Orders (**open question**).
- i18n for every label and order status, both languages. Verify on ttcp-dev (it has Barbican);
  check that an asymmetric order's container shows its three roles on its page.

## 59. Key Manager: the project's preferred secret store (multiple-store Barbican)

> 🔗 **Issue:** [#4]

**Goal.** On a Barbican deployed with several secret stores (`[secretstore]
enable_multiple_secret_stores = True`; e.g. an HSM through PKCS#11 next to the software
`simple_crypto` store, or KMIP and Vault), show which stores exist and which one the current
project uses, and let a project admin pick the project's **preferred** store. New secrets of the
project then go to that store; existing secrets stay where they are (Barbican does not migrate
them; say so in the dialog). Without a preferred store the project uses the global default.
Neither Horizon nor barbican-ui offers this. Single-store deployments (the usual case, and
ttcp-dev unless it says otherwise) answer 404 on these endpoints: then nothing is shown.

What the user sees: a *Secret Stores* card or tab under *Key Manager* (**open question**, to settle
when implemented; the user's current lean: on the *Secrets* list page, not a third panel) listing
each store: name, secret store plugin, crypto plugin, status, *Global default* and *Preferred for
this project* badges. Actions: *Set as Preferred* on a store and *Unset Preferred* on the current
one, both `Dialog::simple` (non-destructive, but the confirmation text says which secrets are
affected).

**Implementation.**

- Barbican v1 *Secret Stores* API, through `JsonRequest::key_manager` like item 55:
  `GET v1/secret-stores` (`secret_stores`, each with `secret_store_ref`, `name`,
  `secret_store_plugin`, `crypto_plugin`, `global_default`, `status`, `created`, `updated`),
  `GET v1/secret-stores/global-default`, `GET v1/secret-stores/preferred` (404 when the
  project has none, which is not an error). Reads in `fetch.rs`, `soft()` on the preferred one.
  Detection: a 404 on the list means multiple stores are off; map it to an empty result plus a
  flag rather than an error page (a `Timed<SecretStores { enabled, stores, preferred_id }>`).
- Model `SecretStoreRow`; pure mapper (ID from `secret_store_ref` via `mapping::ref_id`) tested
  with Barbican-shaped JSON (two stores, one global default, a preferred one, `crypto_plugin:
  null` for KMIP).
- Writes: `OsAction::SecretStoreSetPreferred { id }` → `POST v1/secret-stores/{id}/preferred`
  (no body) and `OsAction::SecretStoreUnsetPreferred { id }` → `DELETE
  v1/secret-stores/{id}/preferred`; in `OsAction::plugins` as `KeyManager`; dry-run tests
  asserting method and path.
- Policy: Barbican lets only the project's admins set the preferred store (`secretstore_preferred:
  post`/`delete`, `rule:admin`), and listing stores may be admin-only too depending on the
  release. Gate the actions with `needs::IDENTITY_ADMIN`-style role checks (`Requires::AnyRole(&
  ["admin"])`) and show the read-only card to everyone who can read it; a 403 on the list hides
  the card like a 404.
- i18n for every label and badge, both languages. Verify on a cloud with multiple stores; on
  ttcp-dev first check whether `GET v1/secret-stores` answers (likely 404), and only open the
  dialogs.

## 60. 🐞 FIX: *Usage Summary* shows `-0.00` for zero consumption

> ✅ **Done.** `model::fixed(v, places)` formats without a negative zero (`-0.00` → `0.00`, a real
> `-1.50` stays); `i18n::decimal` and `UsageSummary::csv` (totals and per-instance hours) use it.
> Cause confirmed: an empty `f64` `.sum()` is `-0.0` (`an_empty_period_totals_zero_not_minus_zero`
> asserts it). Tests: `decimal` with `-0.0` in both languages, `-0.001`, `-1.5`, 0 places; the
> empty-period CSV. Verified on the Overview for January 2020 (no usage): the three totals read
> `0.00` and the CSV link has no `-0.00`.

**Goal.** Project › Overview › *Usage Summary* shows *VCPU-Hours*, *GB-Hours* and *RAM-Hours*
as `-0.00` when the period has no consumption; it must read `0.00` (`0,00` in Spanish), as
Horizon shows. The downloaded CSV has the same `-0.00` in its three totals.

**Cause (confirmed with a test program on rustc 1.98).** `UsageSummary::{vcpu_hours,
gb_hours, ram_mb_hours}` (`model.rs`) `.sum()` an `f64` iterator, and summing an **empty**
`f64` iterator yields `-0.0` (the IEEE additive identity Rust's `Sum` starts from), which
`format!("{:.2}")` prints as `-0.00`. A product with a negative zero, or a tiny negative
rounding to zero (`-0.001` → `-0.00`), prints the same way.

**Implementation.**

- Normalise at the formatter so every number in the UI benefits: in `i18n::decimal` (used by
  the Overview totals), add `0.0` to the value (`-0.0 + 0.0 == +0.0`) and treat a result that
  rounds to zero at `places` as `0`; same in `UsageSummary::csv` for the totals and the per-row
  hours (a small `fn fixed(v: f64, places: usize) -> String` in `model.rs` shared by both, or
  `decimal` calling it).
- Tests next to the code: `decimal(EnUs, -0.0, 2) == "0.00"`, `decimal(EsEs, -0.0, 2) ==
  "0,00"`, `decimal(EnUs, -0.001, 2) == "0.00"`, a real negative stays negative (`-1.5` →
  `-1.50`); `UsageSummary` with no servers: totals `0.0` formatted `0.00` and a CSV without
  `-0.00`. Name the cause in the test message (empty `f64` sum is `-0.0`).
- Check the other `decimal(` call sites (`grep -rn "decimal(" src`, six today) still read
  right, and verify on the Overview with a period that has no usage (dry-run is irrelevant:
  GET only).

## 61. Security group *Add Rule*: help text under every field

> ✅ **Done.** `ops::add_rule_dialog` puts a help text under each of its eight fields (keys
> `DirectionHelp` … `RuleDescriptionHelp`, English and Spanish, in `i18n.rs`), including the ICMP
> type and code for the port fields; the label is now *To Port* (the "empty = same" hint is in
> its help). Ports stay visible whatever the protocol. Test
> `every_rule_field_has_help_and_the_to_port_label_has_no_hint`. Checked on ttcp-dev: each
> help text shows under its field.

**Goal.** The *Add Rule* dialog (Network › Security Groups › a group › *Add Rule*) shows bare
fields; users don't know what *Ether Type*, *Remote CIDR* or *Remote Security Group* mean or
how they combine. Horizon explains them in a side panel ("Rules define which traffic is allowed
to instances…"). Put a short help text **under each field** (`Field::help`, as in *Store
Secret*, item 55), and move the hint now in a label, *To Port (empty = same)*, into the help
text: the label becomes *To Port*.

Help texts (English and Spanish; wording to refine when implementing):

- *Direction*: ingress = traffic arriving at the instances, egress = leaving them.
- *Ether Type*: IPv4 or IPv6; must match the remote CIDR's family.
- *IP Protocol*: empty = any protocol (then no ports); TCP, UDP or ICMP.
- *From Port* / *To Port*: a single port or a range, 1-65535; *To Port* empty = the same as
  *From Port*; both empty = every port. **For ICMP** they are the ICMP *type* and *code* (Neutron
  reuses `port_range_min`/`port_range_max`), e.g. type 8 = echo request (ping).
- *Remote CIDR*: the addresses the rule allows (`0.0.0.0/0` = anywhere in IPv4, `::/0` in IPv6).
- *Remote Security Group*: instead of a CIDR, allow traffic from instances in that group; when
  set, the CIDR is ignored (`ops::add_rule_dialog` already sends only the group).
- *Description*: optional, shown in the rules table.

**Implementation.**

- `components/ops.rs::add_rule_dialog`: `.help(tr(K::…Help))` on each `Field`; fix its doc
  comment, which mentions "Horizon's rule templates" that the dialog doesn't have.
- `i18n.rs`: `PortTo => "To Port", "Puerto final"` (it is used only here), plus one
  `…Help` key per field in both languages.
- The port fields stay visible whatever the protocol (the user's decision, for now): no
  `when` on them, only the help text saying they are ignored without a protocol.
- Verify by opening the dialog in the browser without submitting.

## 62. Orchestration (Heat): Stacks, Resource Types, Template Versions

> ✅ **Done (2026-10-28).** All four slices, checked on ttcp-dev; the Template Generator stays
> out (see the goal).
>
> *Stacks* (`pages/stacks.rs`): list and detail with Horizon's tabs (*Topology*, *Overview*,
> *Resources*, *Events*, *Template*), polling while `*_IN_PROGRESS`. Reads in `fetch.rs` through
> `JsonRequest::orchestration` (relative paths after Heat's `…/v1/<project_id>`); `stack` finds
> the name with the list's `id` filter, as Heat paths need `stacks/{name}/{id}`. Pure mappers
> in `mapping.rs`, tested with Heat-shaped JSON. Event statuses get no busy bar (an event is a
> past transition). The busy bar sits beside status pills (`.pill + .busy`, all pages).
>
> *Topology* is a graph: the stack in the middle, each resource with its icon from draw.io's
> OpenStack library (`mxgraph.openstack`, Apache 2.0; 18 stencils converted to SVG paths in the
> generated `components/heat_icons.rs`), coloured by status (in progress pulses), arrows to what
> it depends on, dashed links from the stack to the resources nothing requires (as Horizon).
> Other types show their initials (`LB`, `QSS`, `STK`). Pure parts in `components/stack_graph.rs`;
> layout `topo_layout::force_edges` (the network graph's, generalised); pan/zoom `Canvas` moved
> to `components/topo_canvas.rs`, shared with *Network Topology*. *List* shows the dependency
> table.
>
> The tab's toolbar has a legend (`graph_legend`): solid arrow *Depends on*, dashed *Part of the
> stack*, and the four status colours; icons are not listed (requested by the user).
>
> *Resource Types* and *Template Versions* (`pages/heat_catalog.rs`): lists (131 types and 13
> versions on ttcp-dev, 1 call each, like the production Horizon) and details: a type's
> properties and attributes as tables plus the raw schema, a version's functions.
>
> Actions (`OsAction::StackCheck/StackSuspend/StackResume/StackDelete`, `POST …/actions` and
> `DELETE stacks/{name}/{id}`): row and header menus (`ops::stack_items`; only *Delete* while
> Heat is busy, *Resume* only when suspended), bulk on the list (`ops::bulk_stack_items`; the
> selection key is `model::stack_key`, `name/id`). Delete warns that the stack's resources go
> too. The stack name is a path segment, so `check_ids` checks `name` for these variants.
>
> *Launch Stack* and *Change Stack Template* are a page (`pages/stack_launch.rs`,
> `/project/stacks/launch` and `/project/stacks/{id}/change`), because the parameter fields come
> from the template: step one takes the template (text, *Load from a file* or URL) and an
> optional environment; *Next* calls `run_stack_validate` (Heat's `POST validate`); step two
> takes name, timeout, rollback and the parameters (lists for `allowed_values`, password for
> hidden ones); *Preview Stack* calls `run_stack_preview` (`POST stacks/preview`). Both are
> read-only POSTs through `Exec` in `server/heat.rs` (AGENTS.md §1.3); in dry-run the
> parameters are read from the template text (`heat::local_info`). *Launch* opens the usual
> dialog running `OsAction::StackCreate` (`POST stacks`); *Change* runs `StackUpdate` as
> `PATCH stacks/{name}/{id}`, so blank parameters (hidden ones Heat masks) and the environment
> keep their values. Hidden values are masked in request previews (`Exec::hide_params`); bodies
> over Heat's 512 KiB are refused. Server fns taking a `StackSpec` use `input = Json`: the form
> encoding drops empty lists (found in the browser: `missing field hidden`).
>
> Checked on ttcp-dev, project pokemon (the user allowed it): validate, preview (8 resources)
> and launch of `moonlight-test-62` (router on `public-dummy-alh1`, tenant network, CirrOS
> 0.6.3 booted from a volume in `alh1`, floating IP; template in the session scratchpad):
> *Create Complete* in ~35 s with outputs, graph of 9 nodes. *Change Stack Template* loads
> prefilled. The user deleted the stack from Moonlight: removal worked. Relaunched, then run
> for real from the stack page: *Check* → *Check Complete*, *Suspend* → *Suspend Complete* (the
> menu then offered *Resume*), *Resume* → *Resume Complete*, and *Change Stack Template* (an
> output added, `flavor` left blank, timeout 45) → `PATCH`, *Update Complete*, the new output
> shown, `flavor` kept at `A0.1-0`, timeout 45 min.
>
> The debug server then crashed with `tokio-rt-worker has overflowed its stack` on the first
> request after a start (stack page open, live reload). Tokio's workers had 2 MiB; `main.rs` now
> builds the runtime with 16 MiB (`WORKER_STACK`). Verified: at 1 MiB the same restart crashes
> at once, at 16 MiB three restarts with the stack page open stayed up.
>
> Left (not planned): resources inside nested stacks (`nested_depth`) in the graph; `get_file`
> includes (Heat's `files` map) in launched templates; a preview for updates (`PUT …/preview`).

Split out of item 5, like 54 and 55. The production Horizon has this group, so it is parity
users will miss (P2).

**Goal.** Horizon's *Orchestration* group with its panels, for clouds that run Heat
(`orchestration` in the catalog). Today the group exists in `components/nav.rs` with no panels
(shown disabled where Heat is, behind the `orchestration` plugin). Seen on the production
Horizon (item 5's review): **Stacks** (*Launch Stack*, *Preview Stack*, *Check*, *Suspend*,
*Resume*, *Change Stack Template*, *Delete*; bulk *Check*/*Suspend*/*Resume*/*Delete*),
**Resource Types** (131 there), **Template Versions** (13) and the drag-and-drop **Template
Generator** (left out of this item, now item 92: largest, least used).

What the user gets:

- *Stacks* list: Name (linked), Created, Updated, Status (`CREATE_COMPLETE`, `*_IN_PROGRESS`
  with the busy bar and polling, `*_FAILED` red), actions per row.
- Stack detail with Horizon's tabs: *Topology* (resources as a graph; **open question**:
  reuse `topo_layout.rs` or a simple dependency list first), *Overview* (ID, description,
  status and reason, outputs, parameters, timeout, rollback, created/updated), *Resources*
  (name linked to the resource's own page when Moonlight has one: server, volume, network,
  port, router, floating IP, security group, load balancer, secret; type, status, reason),
  *Events* (newest first), *Template* (read-only text).
- *Resource Types* list (search; detail with the type's schema: properties and attributes)
  and *Template Versions* list (detail: the functions each version supports).
- *Launch Stack*: template from a file (`Field::area` with *Load from a file*), a URL or typed
  text; optional environment; then the template's parameters as fields (Horizon builds that
  form from Heat's validation); name, timeout, rollback on failure. *Preview Stack* shows
  what would be created.
- Row and header actions: *Check*, *Suspend*, *Resume*, *Change Stack Template* (same form as
  launch, prefilled), *Delete Stack* (destructive dialog listing the stacks; deleting a stack
  deletes every resource it created: say so).

**Implementation.**

- Gating exists: `model::Plugin::Orchestration`; the group's `requires` already needs the
  plugin and `Service("orchestration")`. Add the panels to the empty `panels: &[]`
  (`/project/stacks`, `/project/resource_types`, `/project/template_versions`), drop the
  "shown disabled" special case in `Group::visible` if nothing else uses it, call
  `plugin_enabled(Plugin::Orchestration)?` in every server function, list the actions in
  `OsAction::plugins`.
- No Heat API in the SDK: `JsonRequest` with `ServiceType::Other("orchestration")`, a
  `JsonRequest::orchestration` constructor like `key_manager` (item 55). Heat's catalog URL
  usually ends in `/v1/<project_id>` (the SDK keeps project-scoped endpoints as they are), so
  paths are relative: `stacks`, `resource_types`. **Check** on the cloud whether discovery is
  needed (likely not).
- Reads (`fetch.rs`, GET only): `GET stacks` (paged with `limit`/`marker`; `links` gives the
  next page), `GET stacks/{name}/{id}`, `…/resources?nested_depth=…`, `…/events?sort_dir=desc`,
  `…/template`, `GET resource_types`, `GET resource_types/{type}`, `GET template_versions`,
  `GET template_versions/{v}/functions`. Models `StackRow`, `StackDetail`, `StackResource`,
  `StackEvent`; pure mappers tested with Heat-shaped JSON (status split into action and state,
  `parameters` with Heat's own `OS::*` keys hidden, outputs with `output_error`).
- **Read-only POSTs:** Heat's *Preview* (`POST stacks/preview`) and template validation
  (`POST validate`, which also returns the parameter list for the launch form) change nothing,
  but AGENTS.md §1.3 allows read-only POSTs only after asking. **Open question for the user:**
  allow them (through `Exec`, like the console calls) or build the parameter form by parsing
  the template's `parameters` section in Moonlight (YAML, `yaml-rust2` is already a
  dependency) and skip *Preview*.
- Writes (`actions.rs`, dry-run tests asserting each body): `OsAction::StackCreate(StackSpec)`
  → `POST stacks` (`stack_name`, `template` or `template_url`, `environment`, `files`,
  `parameters`, `timeout_mins`, `disable_rollback`); `StackUpdate` → `PUT stacks/{n}/{id}`
  (*Change Stack Template*); `StackCheck`/`StackSuspend`/`StackResume` → `POST
  stacks/{n}/{id}/actions` with `{"check": null}` etc.; `StackDelete` → `DELETE stacks/{n}/{id}`.
  Heat paths take the stack's name **and** ID: both checked with `check_id` (names allow
  letters, digits, `_`, `-`, `.`). Template text can be large: check the body size.
- Polling while a stack is `*_IN_PROGRESS` (`list_page!`'s `busy`, roadmap 46); the detail
  page too.
- Slices, in order: (1) Stacks list and detail (read-only); (2) Resource Types and Template
  Versions; (3) *Check*/*Suspend*/*Resume*/*Delete*; (4) *Launch*/*Change Template* (after
  the open question). i18n for every label and status, both languages. Verify on ttcp-dev
  (check that it has Heat in the catalog; the production Horizon does) with writes only in
  dry-run unless the user says otherwise.

## 63. 🔒 Trusted client address, atomic sign-in counter, redacted session `Debug`

> ✅ **Done.** `[log] trusted_proxies` (default `1`, env `MOONLIGHT_TRUSTED_PROXIES`):
> `session::client_ip_from(headers, name, trusted)` joins every header line into one list and
> takes the entry `trusted` places from the right (leftmost when shorter, leftmost for `0`); used by
> `client_ip()` (logs and `throttle::keys`) and the cross-origin warning in `server/mod.rs`.
> `Throttle::failed` counts with moka's `entry().and_upsert_with`; the module comment notes the
> remaining `blocked`/`failed` burst gap. `Session` has a hand-written `Debug` with
> `token: "********"`. `settings::origin_of` uses `url::Url::origin()` and returns an error for an
> unparsable `public_url`. Old cookies: `logged_in_at` lost its `serde(default)`, so a cookie
> without it no longer decodes and signs out once (the simpler option). Documented in
> `CONFIGURATION.md` (core settings table) and `moonlight.conf.example`. Tests: hops with 0, 1, 2
> and 5 trusted proxies, single value, several header lines (`session.rs`); 10 parallel `failed`
> count 10 (`throttle.rs`); `Debug` hides the token; a cookie without `logged_in_at` is refused;
> origin with `:443`, `:80`, `:8443`, user info and upper case; default, file and env values of
> `trusted_proxies` (`settings.rs`). Nothing left; no UI change, so no browser check.

Follow-up of items 49 and 50, from the code review of October 2026.

**Goal.** Close the gaps the review found in the sign-in and session code:

- **Spoofable client address.** `session::client_ip_from` takes the **leftmost** entry of
  `[log] client_ip_header` (e.g. `X-Forwarded-For`). Proxies append the address they saw on
  the right; whatever is to the left was sent by the client. So the per-address throttle
  (`throttle::keys`) can be dodged by changing the header on each attempt, an attacker can lock
  out another user's address by sending it, and the sign-in and write logs record a forged
  `client`. The per-username counter still holds.
- **Racy counter.** `Throttle::failed` reads the counter and writes it back in two steps;
  parallel failed sign-ins overwrite each other and are undercounted.
- **Token in `Debug`.** `session::Session` derives `Debug`, token included. Nothing prints it
  today, but `settings::Session` already hides `cookie_key` (test `the_cookie_key_is_never_printed`)
  and the session cookie should get the same protection.
- **`public_url` origin.** `settings::origin_of` keeps an explicit default port:
  `https://cloud.example:443/` becomes `https://cloud.example:443`, which never matches the
  `Origin: https://cloud.example` browsers send, so every POST would get a 403.
- **Old cookies.** `Session::logged_in_at` defaults to `Utc::now()` on every read of a cookie
  that lacks it, so `[session] timeout_minutes` never applies to it (the token's own expiry, about
  1 h, still does). Low: only cookies from before item 49.

**Implementation.**

- `[log] trusted_proxies` (number of proxies that append to the header, counted from the
  right): `client_ip_from(headers, name, trusted)` returns the entry `trusted` places from the
  right, or the leftmost one when the list is shorter. Settings in `server/settings.rs`
  (`MOONLIGHT_TRUSTED_PROXIES` in the env table), `config::Config`, `CONFIGURATION.md` and
  `moonlight.conf.example`. **Open question:** the default, `1` (the usual single balancer)
  or `0` = keep today's leftmost behaviour for compatibility. Single-value headers
  (`X-Real-IP`) work with `1`.
- `throttle.rs::failed`: moka's atomic `self.failures.entry(k.clone()).and_upsert_with(|e|
  async move { e.map_or(1, |e| e.into_value().saturating_add(1)) })`. The check-then-call gap
  (a burst of parallel attempts all passing `blocked` before any fails) stays: it allows at
  most the limit plus one burst; say so in the module comment.
- `session.rs`: a manual `impl Debug for Session` printing `token: "********"` (like
  `model::Hidden`), plus a test next to the existing ones.
- `settings.rs::origin_of`: use `url::Url::parse(u)?.origin().ascii_serialization()` (`url`
  is already a dependency); it drops default ports and user info. Test with `:443`, `:80`,
  `:8443` and upper case.
- Old cookies: in `session::current`, when `logged_in_at` was defaulted, store the cookie again
  so the timeout counts from that first read (as its doc comment already claims), or drop the
  `serde(default)` and let such cookies sign out once. Pick the simpler one.
- Tests: rightmost hop with 0, 1 and 2 trusted proxies and a short list; concurrent
  `failed` calls (`tokio::join!` of 10) count 10; `Debug` hides the token; the origin cases.

## 64. Neutron name lookups filtered by project (admins fetch the whole cloud)

> ✅ **Done.** In `server/fetch.rs`: `networks_raw` and `subnets_raw` are gone.
> `visible_networks(os, project_id)` (own, `shared=true`, `router:external=true`, merged by ID)
> replaces the three hand-written copies and every name lookup (instance,
> router, floating IP, subnet details, Routers, Floating IPs, *Subnets* choices).
> `net_lookups` adds the subnets: `tenant_id` and `shared=true` with the networks, then, **for
> admins only** (`os::token_is_admin`), by `network_id` for each visible network whose subnets
> are still missing (`mapping::networks_without_subnets`, tested): the admin project's external
> networks. Neutron hides those subnets from everyone else, so for members that round cost
> three empty calls on ttcp-dev and was dropped. Used by Networks, Topology, router, port and
> load balancer details and the LB member choices; *Networks* choices (launch, ports) take own
> and shared networks and subnets only. `get_subnet` and `get_router` pass the project.
>
> Measured on ttcp-dev as `admin2` (role `power_user`, project with 7 visible networks):
> Networks 5 calls · 0.18 s (8 · 0.39 s with the unconditional second round), Routers 4 · 0.32 s,
> Floating IPs 5 · 0.23 s, Topology 9 · 0.67 s, router detail 7 · 0.25 s. External network names
> resolve (router gateways show `public-alh1`, `public-mad1`); the subnets of external networks
> show as IDs, as before, because Neutron doesn't show them to a member.
>
> Measured again on 2026-10-27 with `admin2` now holding `admin` (same project): the second
> round runs for the 5 external networks whose subnets weren't fetched, and their subnets are
> named (`subnet-195-235-141-16`, `megatros-ext-subnet-alh1`…). Networks 10 calls · 0.29 s,
> Topology 14 · 0.39 s, Routers 4 · 0.15 s, Floating IPs 5 · 0.16 s. Against the old unfiltered
> lists (a temporary probe on the Networks page, 6 loads, both sets fetched each time): the old
> `GET /networks` + `GET /subnets` returned 96 networks and 103 subnets, 138.8 kB of JSON, in
> 309–606 ms; the filtered lookups return 7 and 7, 9.8 kB (14× less), in 282–367 ms with
> 10 calls instead of 2. The gap grows with the cloud, since the old lists hold every project.

**Goal.** Several pages fetch every network and subnet the token can see only to turn IDs into
names. For admins Neutron returns **every project's** resources (AGENTS.md §6), so on a large
cloud each Instances, Volumes, Routers or Floating IPs load downloads the whole cloud's
networks. The main lists are already filtered; the lookups aren't.

**Implementation.**

- `server/fetch.rs`: `networks_raw(os)` (unfiltered, about 11 callers) and the inline
  `list_all!(…subnet::list, "subnets")` calls (`networks`, `ChoiceSet::LbMemberTargets`,
  `ChoiceSet::LbMemberSubnets`) while `subnets_raw` exists.
- Add `visible_networks(os, project_id)`: own (`tenant_id`) + `shared=true` +
  `router:external=true`, merged with `merge_by_id`. That code is written out three times
  today (`networks`, `topology`, `ChoiceSet::LbMemberSubnets`): replace all three. Subnets the
  same way (own + those of the visible networks, or `tenant_id` + shared), in a
  `visible_subnets`.
- Switch the lookups to them. Mind the call counts passed to `timed` (3 calls instead of 1).
- Check: names of external and shared networks still resolve (floating IP pools, router
  gateways); compare the lists before/after on ttcp-dev as admin and as a member, and give the
  measured times and reply sizes.

## 65. `api.rs` server-function helper, automatic call counts, review cleanups

> ✅ **Done.** `api.rs`: SSR-only `scoped(name, |s, c| …)` (session, scoped connection,
> `REQUEST_TIMEOUT`, call counter) and `scoped_id(name, id, |s, c, id| …)`, which runs `check_id`
> first; `cfg()` replaces `config::state().config`. 54 server functions use them (999 → 923
> lines). `fetch.rs`: calls are counted, not typed. A task-local `CALLS` counter is set up per
> server function by `fetch::counting` (inside `scoped`); every request goes through `counted(…)`
> (all 28 `query_async` sites and the macros), `spawn_counted` carries the counter into
> `JoinSet` tasks, and `timed(t, data)` reads it. `NETWORK_CALLS`, `NetLookups::calls` and the
> page counts of `km_all` and the identity fetchers are gone. `list_all!` is `list_where!` with
> an empty filter. Test `timed_counts_every_request_of_the_call` (joined and spawned calls, a
> fresh counter per call). Cleanups: `os::scoped_key`, `os::hash_key` (also used by
> `throttle::keys`; same keys as before), `token_roles` maps over `token_role_names` with one
> doc comment, and `actions::each_secret` walks the JSON for `mask_secrets` and `wipe_secrets`.
> On ttcp-dev the counts equal the old hand-typed ones: Overview 3 + 1, Instances 4, Images 2,
> Key Pairs 1, Server Groups 1, Volumes 2, Snapshots 2, Groups 4, Group Snapshots 3, Routers 4,
> Security Groups 1, Floating IPs 5, Networks 10 and Topology 14 (as admin, item 64), network
> detail 5, security group detail 2, image detail 2. Not committed: the item suggests one commit
> per bullet.

**Goal.** Less repeated code, so new panels are shorter to write and harder to get wrong. No
behaviour change.

**Implementation.**

- `src/api.rs`: 51 server functions repeat `os::within(os::REQUEST_TIMEOUT, "name", async move {
  let (s, c) = ctx().await?; … }).await?`, and 24 repeat `check_id(&id)?` with the same
  comment. Add an SSR-only helper, e.g. `scoped(name, |s, c| async move { … })`, and
  `scoped_id(name, &id, …)` that checks the ID first. Also one `fn cfg() -> &'static Config`
  instead of 25 copies of `crate::server::config::state().config` (`get_session_info` has 7).
  Keep the `#[server]` fns themselves: they are the API.
- `server/fetch.rs`: 36 call counts passed to `timed(t, N, …)` are typed by hand and drift
  when a fetch changes. Count them instead: e.g. a small `Calls` counter (`AtomicU32`) the
  `list_all!` / `list_where!` / `get_one!` macros bump, read by `timed`. Rewrite `list_all!`
  as `list_where!` with an empty filter.
- Cleanups found in the same review:
  - `os.rs`: the scoped cache key `hash(&[token, project, region])` built in `forget`,
    `scoped_to` and `capabilities`: one `scoped_key` fn. The doc comments of `token_role_names`
    and `token_roles` are merged into one (the "lowercase" sentence belongs to `token_roles`);
    `token_roles` can map over `token_role_names`.
  - `actions.rs`: a stray doc line about `SECRET_KEYS` sits above `parse_access_rules`;
    `mask_secrets` and `wipe_secrets` walk the JSON the same way, so one walker taking a closure.
  - `throttle::keys` hashes usernames with `os::token_key`: rename it to a general
    `os::hash_key` (or add one).
- Done when clippy (both targets) and the tests pass unchanged; one commit per bullet makes
  review easier.

## 66. Short per-project cache for name lookups (flavors, images, networks, volumes)

> ✅ **Done.** `config::AppState::lookups`: a `moka` cache of `Arc<Vec<Value>>`, keyed by
> `os::scoped_key` (hash of token, project, region) + `/` + the list's name, weighed by item
> count (`LOOKUP_ITEMS` = 200 000 items over every session), time to live `[cache]
> lookup_seconds` (default 30, `0` = off; `Config::lookup_ttl`, documented in
> `CONFIGURATION.md` and `moonlight.conf.example`). In `fetch.rs`, `lookup(name, fut)` wraps
> only name lookups: flavors, images and volumes of Instances and instance detail, networks of
> instance detail, Routers, Floating IPs, floating IP and subnet details, and volumes of
> Snapshots, group snapshot and snapshot details. A page's main list, `net_lookups` (the
> Networks page) and dialog choices stay uncached. The session scope travels in the task-local
> of item 65 (`fetch::serving(scope, …)`, set by `api::scoped`), and only successes are kept.
> `fetch::forget_lookups` empties a scope after a successful write (`run_action` with
> `[writes] mode = on`) and in `os::forget` at logout. Tests: `cached_lookups_skip_the_request_until_they_expire`,
> `without_a_scope_lookups_are_not_cached`, and the setting's default and `0` in `settings.rs`.
> Measured on ttcp-dev (admin, project pokemon): Instances 4 calls · 0.67 s on the first load,
> 1 · 0.23 s on the second, 4 again after 32 s; Routers 4 → 1; Floating IPs then 2 (networks
> reused from Routers); Snapshots 1 (volumes reused). The clearing after a write is not
> exercised in the browser (no write submitted). **Replicas:** the cache is per process, like
> `connections` and `capabilities`. Several replicas each fill their own (fewer hits), and a write
> clears only the replica that served it, so without session affinity another replica can show
> an old name for up to `lookup_seconds`. A shared cache (e.g. Redis) would be a new item.

**Goal.** Each list page fetches its lookup lists again on every load and every refresh poll
(the Instances list makes 4 calls, 3 of them only for names). Flavors, images, networks and
volumes change rarely compared with how often pages load. A cache of 30 to 60 s would remove
most of those calls without users noticing stale names.

**Implementation.**

- `config::AppState`: a `moka` cache keyed like `connections` (hash of token, project,
  region) plus the lookup's name, value `Arc<Vec<Value>>`, `time_to_live` 30 to 60 s. Keyed by
  token, so one user never sees another's lookups (visibility differs per role).
- `fetch.rs`: wrap only the **lookups** used for names (`flavors`, `images_raw`,
  `visible_networks` from item 64, `volumes_raw` when it is a lookup). Never the main list of
  a page, and never anything a write just changed: after `run_action` succeeds, invalidate the
  session's entries (or skip the cache when `data_version` changed).
- `os::forget` drops them at logout, like `capabilities`.
- `Timed.calls` then counts only real calls (item 65 makes that automatic).
- **Open question:** the TTL, and whether to make it a setting (`[cache] lookup_seconds`). Also
  check memory: images can be thousands of entries on public clouds.
- Measure on ttcp-dev: calls and time for Instances before and after, first and second load.

## 67. 🔒 Test that every `OsAction` path parameter goes through `check_id`

> ✅ **Done.** Instead of a separate `samples()` list, the check runs on the actions the tests
> already build. In `server/actions.rs` tests, `dry()` calls `assert_paths_checked`: each string
> of the action, one at a time, becomes `zz/zz` (a `/`; dot segments would be normalised away
> by the preview URL), and the action must then fail `check_ids` or `apply`, or keep the marker
> out of every request path; otherwise the test names the variant, the field and the path.
> Completeness, the open question: `every_action_has_a_dry_run_test` reads every variant name
> from serde's unknown-variant error (no exhaustive match, no new dependency) and fails for any
> variant not mentioned as `OsAction::Name` in the tests module. It found 8 (`VolumeUpdate`,
> `SnapshotUpdate`, `NetworkUpdate`, `SubnetCreate`, `SubnetUpdate`, `RouterSetGateway`,
> `SecurityGroupCreate`, `SecurityGroupUpdate`), now covered by
> `renames_subnets_gateways_and_security_groups` with method, path and body. No path parameter
> slipped through `check_ids`, so nothing in the code needed fixing. To check that the test
> detects one, `check_ids` was briefly made to skip `id`: 30 tests failed, each naming the field
> (e.g. `/ImageDelete/id` reaching `/images/zz/zz`). Limit: a variant that a test mentions but
> never dry-runs counts as covered.

**Goal.** Item 48 refuses IDs that would change a request path (`/`, `?`, `%`, `..`). For
writes, `OsAction::check_ids` (`model.rs`) decides which fields to check **by their JSON names**
(`id`, `*_id`, `*_ids`, `kind`, and `name` for key pairs). A new action whose path parameter
is named another way (`pool`, `stack_name`, Heat's names in item 62) would skip the check
without any test failing.

**Implementation.**

- In `server/actions.rs` tests: for a sample of **every** `OsAction` variant (a
  `fn samples() -> Vec<OsAction>`), serialise it, set every string to a marker such as
  `"x/../y"`, deserialise it back, and assert: either `check_ids()` refuses it, or no
  request that `apply` builds in dry-run (`dry(...)`) has the marker in its path. A field that
  reaches a path without being checked then fails the test with the variant's name.
- Keep `samples()` complete: build it with an exhaustive `match` on a dummy value so a new
  variant fails to compile until it has a sample (or a `strum`-like helper; no new dependency
  unless needed). **Open question:** whether that exhaustiveness trick is worth it, or a
  count assertion is enough.
- Fix whatever the test finds (rename the field or add it to `check_ids`), each with a
  comment naming the path.

## 68. DNS (Designate): zones, record sets, reverse DNS of floating IPs

> 🔗 **Issue:** [#5]
>
> ⏸ **Postponed (2026-11-03).** The user said this cannot be implemented for now; no reason was
> recorded beyond what the goal says (the service is not on the production cloud, so it could only
> be tested on another cloud or with fixtures). Nothing is started. It stays out of `Next:` until
> the user reopens it.

**Goal.** designate-dashboard's panels for clouds with Designate: Project › DNS › *Zones*
(Name, Type, Status, Action; *Create Zone* with name ending in a dot, description, email, TTL,
type `PRIMARY` or `SECONDARY` with masters), a zone detail page (Overview · Record Sets) with
*Create Record Set* (type A, AAAA, CNAME, MX, NS, PTR, SRV, SSHFP, TXT; name, description,
TTL, one record per line), *Update* and *Delete* per record set, and *Reverse DNS* (the PTR
record of each of the project's floating IPs, set or unset). Not on the production cloud: test
it on another cloud or with JSON fixtures only.

**Implementation.**

- SDK feature `dns` (add it to `openstack_sdk` in `Cargo.toml`): `dns::v2::zone::{list, get,
  create, set, delete}`, `dns::v2::zone::recordset::{list, get, create, set, delete}` and
  `dns::v2::reverse::floatingip::{list, get, set}`. Check the `pub mod` lines first (AGENTS.md
  §6) and whether lists are `Pageable` (Designate pages with `links.next`).
- Opt-in plugin: `model::Plugin::Dns` (`dns`), menu entry under Network (designate-dashboard's
  place) with `Requires::All(&[Requires::Plugin(Plugin::Dns), Requires::Service("dns")])`,
  `plugin_enabled(Plugin::Dns)?` in the server functions, actions listed in `OsAction::plugins`.
- Read-only first (recipe in AGENTS.md: model, pure mapper tested with Designate-shaped JSON,
  `fetch.rs`, `api.rs`, pages), then the writes: `ZoneCreate`, `ZoneUpdate`, `ZoneDelete`,
  `RecordSetCreate`, `RecordSetUpdate`, `RecordSetDelete`, `FloatingIpPtrSet` (unset sends
  `ptrdname: null`), each with a dry-run test of its body. Validation in `model` (zone and record
  names end in `.`, email, TTL ≥ 1, records valid for the type).
- Designate writes are asynchronous (`PENDING` until the backend applies them): reuse
  `poll_while_busy` on the zone and record set statuses.
- **Open question:** whether to show *Reverse DNS* on the Floating IPs page (a column and a row
  action) instead of a separate panel, as Horizon does it in its own panel.

## 69. Object Store (Swift): containers and objects

> 🔗 **Issue:** [#6]
>
> ⏸ **Postponed (2026-11-03).** The user said this cannot be implemented for now; no reason was
> recorded beyond what the goal says (the service is not on the production cloud, so it could only
> be tested on another cloud or with fixtures). Nothing is started. It stays out of `Next:` until
> the user reopens it.

**Goal.** Horizon's Project › Object Store › *Containers*: the containers in a left list (name,
object count, size, public or private), the selected container's objects and pseudo-folders on
the right with breadcrumbs, *Create Container* (name, public access), *Create Folder*, *Upload
File*, *Download*, *View Details*, *Edit* (replace the content), *Copy*, *Delete* (objects,
folders and empty containers), and the container's public URL when it is public. Not on the
production cloud.

**Implementation.**

- SDK feature `object_store`: `object_store::v1::account::get` (container list, `?format=json`),
  `container::{get, head, create, set, delete}`, `object::{get, head, put, set, delete}`. The
  account path is the catalog endpoint (`AUTH_<project>`), which the SDK resolves.
- Opt-in plugin `Plugin::ObjectStore` (`object-store`), gated as in item 68 with
  `Requires::Service("object-store")`.
- Folders are object name prefixes: list with `prefix` and `delimiter=/`; *Create Folder* puts an
  empty object named `folder/`.
- Uploads and downloads stream through the server, never buffered whole: a dedicated axum route
  (like `server/downloads.rs`) rather than a `#[server]` fn, with a size limit and Horizon's
  `SWIFT_FILE_TRANSFER_CHUNK_SIZE` as `[object_store] chunk_size` (document it in
  `CONFIGURATION.md` and `moonlight.conf.example`). Uploads are writes: they go through
  `OsAction`/`Exec`, so dry-run previews the request without the body.
- Public access is the container read ACL `.r:*,.rlistings` (`X-Container-Read`); show and copy
  the public URL.
- Shares the upload machinery with item 45 (*Create Image* from a file): build whichever comes
  first so the other reuses it.

## 70. Shares (Manila): shares, snapshots, share networks

> 🔗 **Issue:** [#7]
>
> ⏸ **Postponed (2026-11-03).** The user said this cannot be implemented for now; no reason was
> recorded beyond what the goal says (the service is not on the production cloud, so it could only
> be tested on another cloud or with fixtures). Nothing is started. It stays out of `Next:` until
> the user reopens it.

**Goal.** manila-ui's Project › Share panels: *Shares* (Name, Size, Status, Protocol, Visibility,
Share Network; *Create Share* with protocol, size, share type, AZ, share network, metadata;
*Edit*, *Extend*, *Manage Rules* (access rules: ip, user, cert, cephx with read-write or
read-only), *Delete*), *Share Snapshots* (create, edit, delete, create a share from one),
*Share Networks* (Neutron network and subnet; create, edit, delete) and *Security Services*.
The share detail shows its export locations. Not on the production cloud.

**Implementation.**

- **Not in the SDK** (gtema 0.22 has no shared file system crate): raw calls with
  `actions::JsonRequest` for writes and a GET helper in `fetch.rs`, as Trunks (item 5, slice 3)
  and Barbican (item 55) do. The catalog type is `sharev2`; it needs version discovery and an
  `X-OpenStack-Manila-API-Version` microversion header (Manila's own header name).
- Opt-in plugin `Plugin::Shares` (`shares`), gated with `Requires::Service("sharev2")`.
- Slices: shares read-only with export locations → share writes and access rules → snapshots →
  share networks → security services. Dry-run tests for every body.
- **Open question:** which Manila microversion to pin (manila-ui uses the highest it knows); the
  minimum should cover share types by ID and `share_group_id`.

## 71. Container Infra (Magnum): clusters and cluster templates

> 🔗 **Issue:** [#8]
>
> ⏸ **Postponed (2026-11-03).** The user said this cannot be implemented for now; no reason was
> recorded beyond what the goal says (the service is not on the production cloud, so it could only
> be tested on another cloud or with fixtures). Nothing is started. It stays out of `Next:` until
> the user reopens it.

**Goal.** magnum-ui's Project › Container Infra: *Clusters* (Name, Template, Status, Health,
Master and Node counts; *Create Cluster* wizard with template, keypair, sizes, flavors, network
options and labels; *Resize* the node count, *Upgrade* to another template, *Show Certificate*
and *Sign Certificate* for `kubectl`, *Delete*) and *Cluster Templates* (create from scratch or
by copying, edit, delete). Not on the production cloud.

**Implementation.**

- SDK feature `container_infra`: `container_infrastructure_management::v1::cluster::{list, get,
  create, set, delete}`, `cluster::action::{resize, upgrade}`, `cluster::nodegroup::*`,
  `clustertemplate::{list, get, create, delete}` and `certificate`. Magnum needs the
  `OpenStack-API-Version: container-infra <v>` header for actions (1.7+ for resize).
- Opt-in plugin `Plugin::ContainerInfra` (`container-infra`), gated with
  `Requires::Service("container-infra")`.
- *Create Cluster* uses the wizard (item 2); cluster status moves for minutes
  (`CREATE_IN_PROGRESS`), so the list and detail poll while busy.
- Downloading a kubeconfig is a read that signs a CSR (`POST /certificates`): ask before adding
  it (AGENTS.md §1.3), as it would be a POST outside `OsAction`.

## 72. Instances list: the *Age* column wraps onto two lines so the table fits (Spanish)

> ✅ **Done.** `i18n::age_parts` returns the units (`["3 semanas", "2 días"]`, never empty) and
> `age` joins them. `widgets::age_cell` / `age_cell_since` render each unit in a `nowrap` span
> with the comma kept on the first, so the cell breaks only between units; used by the
> Instances *Age* column (its `nowrap` class is gone) and the Overview's *Usage Summary*. That
> alone left the Spanish table 83 px too wide at 1512 px, so `.table th` no longer has
> `white-space: nowrap`: the auto layout wraps headers at spaces only when a table would scroll
> sideways (`.th-sort` keeps `text-align: inherit`). Measured on ttcp-dev at 1512 px:
> Spanish `scrollWidth` 1553 → 1470 = `clientWidth`; English 1470 = 1470 with no header
> wrapped. Test: `age_parts_are_the_units_that_age_joins`. The open question (target width)
> was settled at the user's 1512 px screen; narrower screens still scroll.

**Goal.** In Spanish (and any longer language), the Instances table is wider than the page and
scrolls sideways. One cause is the *Age* column (*Antigüedad*): its value is two units
(`3 semanas, 2 días`) kept on one line. Let it break after the comma, one unit per line, so
the column is as narrow as its longest unit and the table fits the width at common screen sizes,
as it does in English. Never break inside a unit (`3` / `semanas`).

**Implementation.**

- `src/pages/instances.rs`: the *Age* column (`col(K::Age, …)`) has `.class("nowrap")`.
  Drop it and render the parts of the age as separate pieces that may wrap between them and
  not inside: e.g. `i18n::age` gains a variant returning the parts (`age_parts(lang, secs) ->
  Vec<String>`, with `age` joining them with `", "`), and the cell renders each part in a
  `<span class="nowrap">`, the comma after the first part. A non-breaking space inside each
  part (`3\u{a0}semanas`) is the CSS-free alternative; pick one when building.
- The same cell on the Overview's *Usage Summary* (`pages/overview.rs`, `age(l, s.uptime)`)
  for consistency; the instance detail (`instance_detail.rs`, a key/value row) needs no change.
- Check the other long Spanish headers of the table while there (`Zona de disponibilidad`,
  `Estado de energía`, *Task*): `.table th` is `white-space: nowrap` (`style/main.css`). If
  the table still overflows after the *Age* change at 1440 px, consider letting headers wrap
  at spaces (`th { white-space: normal }` for those columns, or a `th.wrap` class) rather than
  shortening the labels. **Open question:** the target width (1440 px? 1280 px?).
- Tests: `age_parts` in `i18n.rs` (one and two parts, singular and plural, both languages; that
  `age` still joins them as today). Verify in the browser in Spanish and English at the target
  width: `.table-wrap` has no horizontal scroll (`scrollWidth <= clientWidth`).

## 73. Networks list: no selection checkbox on networks the project can't change (external, shared)

> ✅ **Done.** `pages/lists.rs`: `network_bulk_key` (only `NetworkRow::owned`, the flag that
> also hides the row menu) and `security_group_bulk_key` (not the `default` group, which has no
> *Delete*). The other `key!(id, name)` lists only hold the project's own resources. Tests in
> `lists.rs` (`only_owned_networks_…`, `default_security_group_…`). On ttcp-dev: the four
> external networks and `t2c-cert-al-service-tenant` show a disabled checkbox, *Select all*
> picks only `route-1` and `route-2` (*Delete Networks (2)*), and `default` is disabled on
> Security Groups.

**Goal.** On Project › Network › Networks, external networks and networks shared from other
projects have no row actions (no *Edit Network*), but their selection checkbox is enabled, so
they can be picked for *Delete Networks* and the delete then fails in Neutron. Disable the
checkbox on those rows, as the Images list already does for images the project doesn't own.

**Implementation.**

- `src/pages/lists.rs`, the Networks `list_page!`: the bulk key is `key!(id, name)` (every
  row selectable). Use `|n: &NetworkRow| n.owned.then(|| (n.id.clone(), n.name.clone()))`,
  like the Images list (`i.owned_by_project.then(…)`). `NetworkRow::owned` is the same flag
  `ops::network_items` uses to hide the row menu, so the checkbox and the menu agree. A `None`
  key already renders a disabled checkbox (`table::Select`), so nothing else changes.
- While there, check the other lists whose bulk key is `key!(id, name)` for rows that can't be
  deleted either: the `default` security group (`ops::security_group_items` gives it no
  *Delete*) is the likely one; others list only the project's own resources.
- Test: the key function as a named fn (e.g. `network_bulk_key`) with an owned and a shared
  row, next to the list. Verify on ttcp-dev: the external networks (`public-alh1`, …) show a
  disabled checkbox and *Select all* skips them.

## 74. Network Topology: tell external and shared networks apart from the project's own

> ✅ **Done.** The user chose a hatched fill plus a badge. `TopoNetwork::owned` (new, from
> `project_id`/`tenant_id`; `TopologyInputs::project_id`) lets `NetKind::of` return `[Own]` or
> the marks `External` (`router:external`) and `Shared` (another project's network; on an
> external network, Neutron's `shared` flag). A network that is both gets both badges, and both
> stripe directions overlap into a cross-hatch (user request). Each mark draws stripes
> (`<pattern>`s `topo-hatch-ext` / `topo-hatch-shared` in the canvas `<defs>`) over the network's
> own colour and a badge (globe / share glyph on a `--surface` disc). The badges stack in the
> top cap of a bar and run along the start of a pill. At the user's request the Topology bars
> are now pills like the Graph's (`rx = bar_w / 2`). The label gets a stroke in the network
> colour so the stripes don't cut through it. The node's `aria-label` adds the kinds
> (`net_label`), and a legend in `.topo-meta` lists the kinds on screen (`legend_kinds`; hidden
> when all are own, and in *List*). The list and tooltip share `net_kind_text` (*External* ·
> *Shared*). CSS uses tokens only. Tests: `net_kind_marks_…`, `legend_lists_only_…`,
> `net_kind_text_joins_both_flags` in `topology.rs`, `owned` in the mapping test. Checked on
> ttcp-dev in both views, light and dark: `public-dummy-alh1` (external + shared) has two badges
> and the cross-hatch. Keyboard focus and the tooltip were not re-checked by hand (their code is
> unchanged).

**Goal.** In Project › Network › Network Topology, both the *Topology* view (vertical network
bars) and the *Graph* view (network pills) draw every network the same way: one colour per
network from the AZ pill palette (`net_style(name)` hashes the name). Only the hover tooltip
says *External* or *Shared*. Make it visible at a glance which networks are the project's own
and which are external (provider/floating IP networks) or shared from another project, in both
views, in light and dark mode, and without relying on colour alone (accessibility).

**Implementation.**

- `src/pages/topology.rs`: the bars (`TopologyView`, `rect class="topo-bar"` around line 400)
  and the pills (`GraphView`, line ~510) get a class from `TopoNetwork::external` /
  `shared` (already in the model and mapped): e.g. `topo-net ext` / `topo-net shared`.
  Proposed look (to confirm in the browser): keep each network's own colour, and add a marker
  that doesn't depend on hue: a dashed or double outline plus a small badge (a globe glyph for
  external, a share glyph for shared) at the top of the bar / start of the pill; the label gets
  the same word as the tooltip for screen readers (`aria-label` on `Node`).
- A small legend in `.topo-meta` (own · external · shared), only listing kinds that appear.
- The networks table under the views (line ~624) shows *External* but not *Shared*: use the
  same wording as `TopoTip` (both, joined with ` · `).
- CSS in `style/main.css` (`/* Network topology */`, `.topo-bar`): new rules with tokens on
  `:root` as `light-dark(…)`, no hex in rules (AGENTS.md §7).
- **Open question:** badge + outline vs. a hatched fill (an SVG `<pattern>`) for external
  networks; and whether own networks should instead be the ones highlighted. Decide with the
  user on the test cloud (ttcp-dev has external `public-alh1`, `public-mad1`,
  `public-dummy-alh1` and shared `t2c-cert-al-service-tenant`).
- Tests: the class/legend choice as a pure fn of `TopoNetwork` (e.g. `net_kind(&n) ->
  NetKind`) unit-tested; `topo_layout` tests unaffected. Verify both views and both themes in
  the browser, plus keyboard focus and the tooltip.

## 75. Key Manager: ACL management for secrets (and containers)

> ✅ **Done (2026-10-29).** Secrets and containers. Settled by the user: user IDs typed (no
> picker), actions hidden unless allowed; containers and an "in use" warning added on request.
>
> Built: `model::KmAcl` (users, `project_access`, `user_names`, `can_edit`, `in_use`) in
> `SecretDetail.acl` and the new `ContainerDetail { container, acl }` (`get_container` returns it).
> Writes: `SecretAclSet`/`ContainerAclSet { id, project_access, users }` → `PUT
> v1/{secrets,containers}/{id}/acl` with `{"read": {"project-access", "users"}}` (replaces the
> ACL; `actions::acl_put` checks each user ID) and `SecretAclDelete`/`ContainerAclDelete` →
> `DELETE …/acl`; dry-run test `secret_acl_set_and_reset`. Reads: `fetch::km_acl` names users
> (soft `GET v3/users/{id}`, parallel, at most 20; 403 or deleted → ID). `can_edit` from
> `model::can_edit_secret_acl` (creator or `admin`, Barbican's default policy) in `get_secret` /
> `get_container`. Menus (`ops::secret_detail_items`, `ops::container_detail_items`, shared
> `acl_items`): *Edit Access* (project-access checkbox, user IDs parsed by
> `model::parse_user_ids`, *Nothing changed* when untouched; on containers a note that each
> secret has its own ACL and a load balancer needs both), *Reset Access* (only when an ACL is
> set; lists the users who lose access). Cards: `AclRows` (names with IDs, *Private* badge) on
> both detail pages.
>
> **What uses it.** Octavia never registers a Barbican consumer (upstream: its API calls
> `set_acls` then `get_cert(check_only=True)`, and the amphora path also uses `check_only=True`;
> only the legacy driver registers, with `check_only=False`), so on ttcp-dev an active TLS load
> balancer left every consumer list empty. With the `load-balancer` plugin, the detail pages ask
> Octavia: the project's listeners and pools whose certificate refs point at the resource,
> directly or (for a secret) through a container (`mapping::octavia_cert_refs`,
> `mapping::octavia_uses`, `fetch::octavia_cert_refs`, soft) → `KmAcl.octavia`, a list of
> `OctaviaUse` shown under *Consumers* as *Load balancers using it* (load balancer linked,
> listener or pool, *Through Container* linked); "No service is registered" only shows when
> both lists are empty. `KmAcl.in_use` keeps Barbican's own consumers. Both dialogs warn when
> either is non-empty, pointing at *Consumers* (`ops::in_use_warning`). Tests for each helper
> and `access_dialogs_warn_when_something_uses_it`. Verified with a second `moonlight-test-lb`
> (subnet `prj-test-a-tenant`, the first one and the stack had been deleted): the container page
> lists it (load balancer linked, *Listener*, its ID as name), the secret page adds *Through
> Container* `moonlight-test-tls` (linked), and *Edit Access* shows the warning; the secret
> page took 8 OpenStack calls, 0.64 s. After the first load balancer was deleted, Octavia's
> user stayed on the container's ACL: leftover entries are normal, the table is what tells.
>
> Checked on ttcp-dev (project pokemon), with the user's go-ahead: secret `moonlight-test-key`
> made private with an extra user and restored; load balancer `moonlight-test-lb` created from
> Moonlight (TLS listener on container `moonlight-test-tls`, *Active* in about 1.5 min, left
> running); the container's and its secrets' dialogs then showed the warning; container *Edit
> Access* adding a user and restoring (`PUT …/containers/{id}/acl` accepted, card refreshed).
> The container already had user `3d0246…` (very likely Octavia's) from earlier TLS tests. Not
> submitted: *Reset Access* (it would remove that user). Detail pages: 6–8 OpenStack calls,
> under 0.25 s.

**Goal.** Let the owner of a Barbican secret decide who can read it, from the secret's detail
page. Today item 55 shows the `read` ACL read-only (*Access Control* card: *Project access*
yes/no, *Users with read access* as raw user IDs). Barbican's default is that every user with a
role in the project can read the secret; the ACL narrows that (`project-access: false`, the
secret becomes *private* to its creator and the listed users) or widens it to named users of
other projects (e.g. a service user such as Octavia's or Magnum's, or a colleague). Neither
Horizon nor barbican-ui offers this; the `openstack acl …` CLI is what users use now.

What the user sees:

- The *Access Control* card lists the users **by name** (ID as fallback, linked to the
  Identity user page when the viewer is an admin) and a badge *Private* when
  `project-access` is `false`.
- *Edit Access* on the secret's menu (`DetailHeader items`), a `Dialog::form`: a checkbox
  *Every member of the project can read it* (`project-access`) and a user picker for the users
  with read access. Preselected with the current ACL; *Nothing changed* when untouched.
- *Reset Access* (back to Barbican's default: project access, no users): destructive-style
  confirmation (`Dialog::confirm`, lists the users who lose access), only when an ACL is set.
- The same card and dialogs on the container detail page (containers have their own ACL,
  `v1/containers/{id}/acl`; Octavia needs read access to the container **and** its secrets,
  so the container dialog says so). **Open question:** do containers in the same slice, or
  secrets first.

**Implementation.**

- API (Barbican v1 ACLs): `GET v1/secrets/{id}/acl` (already read by `fetch::secret`),
  `PUT v1/secrets/{id}/acl` with `{"read": {"project-access": bool, "users": [ids]}}` replaces
  the whole ACL, `PATCH` merges, `DELETE v1/secrets/{id}/acl` removes it (default again). Same
  paths under `v1/containers/{id}/acl`. Use `PUT` (the dialog sends the full desired state, so
  no diffing, unlike *Manage Members*) and `DELETE` for reset.
- Writes: `OsAction::SecretAclSet { id, project_access, users }`, `SecretAclDelete { id }`
  (and `ContainerAclSet` / `ContainerAclDelete`) in `model.rs`, arms in
  `server/actions.rs::apply` built with `JsonRequest::key_manager` like `SecretDelete`, listed
  in `OsAction::plugins` as `KeyManager` (`model.rs:2952`). User IDs go in the body, not the
  path, but validate them as IDs on the server (`AppError::invalid`). Dry-run tests asserting
  method, path and exact body (`every_action_has_a_dry_run_test`).
- Reads: `ContainerDetail` gets the same ACL fields as `SecretDetail` (`acl_users`,
  `project_access`; reuse `mapping::secret_acl`, maybe renamed `km_acl`), fetched with `soft()`
  in `fetch::container`. User names: resolve `acl_users` with a soft Keystone lookup (`GET
  v3/users/{id}` each, in parallel, or the cached name lookups of item 66); a non-admin gets
  403 there, so fall back to the ID.
- **User picker, open question.** `ChoiceSet::IdentityUsers` (item 57) lists the project
  domain's users, which needs Keystone admin rights; a plain member can't list users. Proposal:
  the picker when the list loads, else a text field of user IDs (comma-separated, validated as
  IDs), and help text saying where to find a user's ID. Also: users of other domains (service
  users live in `Default` or `service`) are not in that list, so keep the free-text entry for
  admins too (a combo that accepts typed IDs, if `Field::pick_many` allows it).
- Policy: Barbican lets the secret's creator and the project admin change its ACL
  (`secret_acls:put_patch` / `delete`; `rule:secret_project_admin or
  rule:secret_project_creator` in the default policy). Show the actions to everyone who sees
  the secret and let a 403 come back as the classified error (item 35 handles role-aware
  hiding in general), or hide them when the viewer is neither the creator
  (`creator_id` = session user) nor has `admin`: **open question**, proposal: hide.
- UI in `components/ops.rs` (`secret_items` at ~line 3856, `container_items` ~3918) and the
  cards in `pages/key_manager.rs` (`SecretCards`, ~line 80). i18n for every label, both
  languages (`ReadUsers`, `ProjectAccess` exist).
- Tests: mapper (ACL with users, `project-access` missing → `true`, unreadable → `None`),
  dry-run bodies of the four actions, ID validation. Verify on ttcp-dev (Barbican, writes on):
  open the dialogs and read the previews; submit only if the user asks, on a test secret, then
  check with `GET …/acl` that the result matches and that the list/detail pages stay GET-only.

## 76. Expired session: server function calls get their error, dialogs say so

> ✅ **Done (2026-10-28).** `session::require` set `302 FOUND` on every request without a valid
> session, server function calls included, so `fetch` followed it and parsed the login page: a
> dialog submitted after the Keystone token expired (about an hour) showed *error deserializing
> server function results: expected value at line 1 column 1*, and client-side page loads failed
> the same way. Now only page loads (`Accept: text/html`, `session::wants_html`, tested) get the
> 302; server function calls get leptos_axum's redirect header and the JSON `Unauthenticated`
> error, so the router's redirect hook goes to `/login`. Dialogs show *Your session has expired*
> with *Sign In* (`Phase::Expired` in `components/dialog.rs`) instead of the raw error.
> Checked in the browser: a call without the cookie returns `500` with the JSON error and
> `serverfnredirect`, a page load still `302`. The dialog's new message was not seen (the
> HttpOnly cookie can't be dropped from the page); it shows on `ErrorKind::Unauthenticated`.

Found while testing roadmap 62: Check Stack failed with that message an hour after sign-in.

## 77. Keep the session alive while the user is active (`[session] idle_minutes`)

**Goal.** Today a sign-in ends when the Keystone token does (about 1 h, `[token] expiration`),
even in the middle of work: the next action shows *Your session has expired* (roadmap 76) and
whatever was typed (a stack template, a launch form) is lost. The user wants the session to go
on while they are active: as long as Moonlight sees activity within a configurable window, the
token is renewed before it expires; after that much idle time, or after an absolute maximum,
the session ends as now. Horizon has nothing like it: its session ends with the token, and
operators raise Keystone's `[token] expiration` instead (still possible, and worth a line in
`CONFIGURATION.md` either way).

**Why it needs a credential.** Keystone does not extend tokens: authenticating with method
`token` returns a new token with the **same** `expires_at`. Renewing means signing in again
with something that can mint tokens. Options:

- **A. The password, held in server memory** (recommended). At sign-in (`os::login`, `v3password`)
  keep the password in a `moka` cache keyed by the session's `sha256` token key, as
  `secrecy::SecretString` (zeroised on drop), with time-to-idle = `idle_minutes`. Never in the
  cookie, never on disk, never logged (AGENTS.md §1.5–1.6 spirit). When a request finds the
  token within a lead time of expiring (say 5 min) and the password is still cached, sign in
  again, rescope, and write the new token into the cookie (server functions can set cookies
  through `ResponseOptions`); optionally revoke the old one. Lost on restart and per replica:
  then the session ends as today. Fails for users whose sign-in needs more than a password
  (TOTP, federation), who keep today's behaviour.
- **B. An application credential** created at sign-in (`POST /v3/users/{id}/application_credentials`,
  `expires_at` = the absolute maximum, deleted on logout), its secret in the encrypted cookie.
  Bound to **one project**: Moonlight rescopes per project, so switching project would need one
  credential per project or a new sign-in. Visible to the user in Identity › Application
  Credentials (roadmap 57); Keystone policy may forbid it. Not recommended.
- **C. Trusts**: also project-bound, and need the delegation API. Not recommended.

**WebSSO sessions (item 80)** can't be renewed this way: no password reaches Moonlight, and
re-authenticating with the federated token (`os::login_with_token`) keeps its `expires_at`. They
end with the token, as in Horizon (`Session::websso` tells them apart). The only renewal would be
a new trip through the identity provider (`/auth/websso/start` with the session's choice), which
is silent while the IdP's own session lasts but is a full-page redirect, so it would lose
unsaved forms just the same; out of scope unless the user asks.

**Implementation (option A).**

- Settings (`server/settings.rs`, `server/config.rs`): `[session] idle_minutes` (unset = off,
  today's behaviour; it was planned in roadmap 18 and never used) and an absolute cap, either
  reusing `timeout_minutes` (today "a sign-in lasts at most this long") or a new
  `max_minutes`. Document both in `CONFIGURATION.md` and `moonlight.conf.example`.
- Activity: what renews the idle timer. Page loads and server functions the user triggers, but
  **not** the background polling (`poll_while_busy`, roadmap 46) nor the header's periodic
  calls, or an open tab would keep the session alive for ever. Needs a way to tell them apart
  (a header the polling sets, or polling server fns that skip the touch).
- Renewal in `session::require` / `os::scoped` (`server/session.rs`, `server/os.rs`): single
  flight per session (a lock in the cache entry) so parallel requests renew once; on failure
  (password changed, account disabled) drop the cached password and let the token run out.
- Logout (`api.rs` `Logout`) and *Sign out* everywhere drop the cached password; `revoke_on_logout`
  unchanged. Password changes (roadmap 57, *Change Password*) replace it.
- Tests: pure renewal decision (`fn should_renew(now, expires_at, last_active, idle, cap) ->
  bool`) with fixed times; cache eviction on idle; no password in any `Debug`/log output.
- Verify on ttcp-dev with a short Keystone token or a large lead time; the user signs in.

**Open questions for the user.** Option A or B. Whether keeping passwords in server memory is
acceptable for the operators (it is opt-in). The defaults: idle window and absolute cap.

## 78. Result dialog: requests without numbers, and `[ui] show_requests`

> ✅ **Done (2026-10-29).** Settled by the user: the setting applies everywhere, **dry-run
> included** (off: a dry-run result only says nothing was sent).
>
> `RequestList` (`components/dialog.rs`) is an unnumbered `<ul class="requests">` (CSS
> `list-style: none; padding-left: 0`) and renders nothing when empty; the method badge stays.
> `[ui] show_requests` (`server/settings.rs` `Ui`, `server/config.rs`; no environment variable,
> like `show_error_details`), default `false`, `true` in the repo's `moonlight.conf`, documented
> in `CONFIGURATION.md` and `moonlight.conf.example`. Applied on the server so hidden requests
> never reach the browser: `ActionOutcome::shown` / `ConsoleReply::shown` (`model.rs`, tested in
> `request_tests`) in `run_action`, `run_instance_log`, `run_instance_console`,
> `run_stack_validate` and `run_stack_preview` (`api.rs`); settings test for the default and the
> parse. AGENTS.md's write recipe now says the dry-run preview needs the setting on.
> Browser: the list style checked on the page (no markers, no indent). Not seen end to end:
> writes are on in the dev config, so a result dialog needs a real write; the next write shows
> it.

**Goal.** After an action runs (create, edit, delete…) the result dialog lists the REST requests
it sent as a numbered list (`1.`, `2.`…), which reads like steps. Keep each request's method badge
(`POST`, `PATCH`, `DELETE`), path, service and body, but drop the numbers. Then make the list
optional: a new `[ui] show_requests` setting, **off by default** (users see only *Done* or the
errors, like Horizon's success message), **on** in the repo's `moonlight.conf` (the
development cloud, where the requests are what is being checked).

**Implementation.**

- Numbers: `RequestList` (`components/dialog.rs`) renders `<ol class="requests">`; the CSS
  (`style/main.css`, `.requests { … padding-left: 20px; … }`) keeps the markers. Use
  `list-style: none; padding-left: 0` (or a `<ul class="plain">`). Same component in the instance
  *Log*/*Console* tabs (`pages/instance_detail.rs`) and *Preview Stack* (`pages/stack_launch.rs`).
- Setting, server-side like `[ui] show_error_details` (`server/config.rs`, `server/errors.rs`),
  so hidden requests never reach the browser: `run_action` (`api.rs`) clears
  `ActionOutcome.requests` when the setting is off and `mode = on`; same for `ConsoleReply`
  (`run_instance_log`, `run_instance_console`, `run_stack_validate`, `run_stack_preview`). Add it
  to `server/settings.rs` (`[ui]`, env `MOONLIGHT_SHOW_REQUESTS`), `server/config.rs`,
  `CONFIGURATION.md` (Moonlight-only setting, no Horizon equivalent), `moonlight.conf.example`
  (`show_requests = false`) and the repo's `moonlight.conf` (`show_requests = true`).
- **Dry-run always shows them** (the preview is all dry-run produces, and AGENTS.md's recipes
  verify writes by reading it); the setting only applies to requests really sent. Say so in
  `CONFIGURATION.md`.
- Tests: settings parse and default (`false`); a pure helper deciding whether to keep the
  requests (`mode`, setting) used by every server fn above; `Outcome` shows no empty list.
- Verify in the browser in dry-run (list without numbers) and with the setting off and on.

**Open question for the user.** Whether dry-run should ignore the setting as proposed.

## 79. Alarming (Aodh): alarms list, detail, history, create/edit/delete

> 🔗 **Issue:** [#9]
>
> ⏸ **Postponed (2026-11-03).** The user said the rest of this cannot be implemented for now; no
> reason was recorded. Built: slices 1 (read-only) and 3 (create/edit), described below. Left:
> slice 2 (delete, enable/disable, set state) and 4 (Heat `OS::Aodh::*` links), plus the checks
> the notes below list as not verified in the browser. It stays out of `Next:` until the user
> reopens it.
>
> 🚧 **In progress (2026-10-30): slice 1 (read-only) built.** The user chose a new *Monitoring*
> group: Project › Monitoring › Alarms (`/project/alarms`, `/project/alarms/:id`). Gating:
> `Plugin::Alarming` (`alarming`), `needs::ALARMING` (plugin and the `alarming` service), the
> group's `requires` in `components/nav.rs`, `plugin_enabled` in `api::list_alarms` /
> `api::get_alarm`. Requests: `JsonRequest::alarming` (`server/actions.rs`, paths prefixed
> `v2/`, no discovery, like Barbican); `fetch::alarms` pages `GET v2/alarms` with
> `q.field=project_id` (admins see every project's), `limit=100` and `marker`;
> `fetch::alarm` gets `v2/alarms/{id}` and `…/history?limit=200` in parallel (history
> soft-fails). Models `AlarmRow`, `AlarmRule` (Gnocchi thresholds in one variant, `Prometheus`,
> `Event`, `Raw` JSON for `composite` and the rest), `AlarmTimeConstraint`, `AlarmChange`,
> `AlarmDetail`; mappers `mapping::alarms` / `alarm_detail`, two tests with Aodh-shaped JSON
> (every rule type, string thresholds, empty names and severities, history order and its JSON
> `detail`), plus a nav test. Page `pages/alarms.rs`: list (Name, Type, State pill, Severity
> pill, Enabled, one-line Condition such as `mean(cpu) > 80 · instance <id>`, State since),
> detail with *Overview* (Information, Rule per type with a link to the instance for Gnocchi
> `instance` resources, Actions on state change, Time constraints) and *History*. i18n in both
> languages; `[plugins] enabled` documents `alarming` (CONFIGURATION.md,
> `moonlight.conf.example`), and the repo-local `moonlight.conf` (ttcp-dev) enables it.
> Results: both clippy runs clean, `cargo nextest run --features ssr` 387/387 passed.
> **Not verified in the browser** (the extension was not connected): check on ttcp-dev that
> Aodh's catalog URL is unversioned (else drop the `v2/` prefix), that the `project_id` query
> and `marker` paging are accepted, and which alarm types the cloud uses. Left: slices 2
> (delete, enable/disable, set state), 3 (create/edit) and 4 (Heat `OS::Aodh::*` links).
>
> **Slice 3 built (2026-10-30), before slice 2 as the user asked:** *Create Alarm* (list header)
> and *Edit Alarm* (row menu and detail page), `ops::alarm_dialog`, for the types the user
> chose: `prometheus`, `gnocchi_resources_threshold`, both Gnocchi aggregation types and
> `event` (`model::ALARM_TYPES`). The fields follow the type (`show_if`/`when`); on edit the
> type is fixed and only its fields show. `OsAction::AlarmCreate(AlarmSpec)` → `POST
> v2/alarms`, `AlarmUpdate { id, spec }` → `PUT v2/alarms/{id}` with the whole alarm (Aodh
> replaces it), so `AlarmRow::spec` (mapped from the list's full alarms, `mapping::alarm_spec`)
> carries what the form doesn't edit: the time constraints. `AlarmSpec::body` builds and
> checks the body in the dialog and on the server: name, type, severity, operator, a finite
> threshold, Gnocchi granularity and periods above 0, the Gnocchi search as valid JSON (sent
> as text, as Aodh stores it), event filters `field op value` (symbols or Aodh names, the
> format the detail page shows), action URLs with Aodh's notifier schemes (`http(s)`,
> `trust+http(s)`, `log`, `zaqar`, `trust+zaqar`). Thresholds accept a decimal comma. Every
> field has a help text in both languages (guarded by the dialog test). The complex fields
> (type, PromQL, metric, metric IDs, resource type and ID, Gnocchi search, aggregation,
> threshold, granularity, evaluation periods, event type and filters, alarm actions) also have
> an (i) tooltip with metric names, units and examples: `Field::tip`, rendered by
> `widgets::InfoTip`, whose text takes lines, list items (a leading dash) and inline bold,
> italic and code (`widgets::tip_inline`; tests guard the parsing and that no tip leaves its
> markup unpaired). Tests: dry-run bodies of every type and the refusals (`actions.rs`), the
> dialog (`ops.rs`), the spec round trip of an unchanged alarm (`mapping.rs`). Both clippy runs
> clean, 392/392 tests passed.
>
> **Verified on ttcp-dev (2026-10-30).** Aodh's catalog URL is unversioned (the `v2/` prefix is
> right), the `project_id` query works, and the cloud has no Gnocchi (`metric`), so its alarms
> are `prometheus`. At the user's request one alarm was created from the dialog:
> `whismur-cpu-high` (`c49714af-…`), query
> `rate(ceilometer_cpu{resource="<whismur's ID>"}[5m]) / 10000000` > 80, moderate, no actions;
> list, detail and history show it (*Insufficient data* until Aodh evaluates it; whether the
> `ceilometer_cpu` name exists in this Prometheus is still to be seen when the user loads the
> instance). Two bugs found and fixed: Aodh's create returns `alarm_id`, so `Exec::create`
> reported a created alarm as a failure (`actions::created_id`, tested); and Aodh refuses
> `log://` actions to non-admins with a **401**, which `errors::classify` took for an expired
> session and signed the user out (a 401 saying "not authorized to" is now `Forbidden`,
> tested). The tooltip and help say `log://` is for admins only. 394/394 tests passed.
> Still open: the transient "Service not available" seen once on the detail page right after
> creating; the error text of a refused write is generic unless `[ui] show_error_details`.
>
> **Confirmed by the user (2026-10-28):** ttcp-dev has Aodh in the catalog, its endpoint is
> reachable and alarms can be created there, so slices can be verified on it (writes only when
> the user says so). Still open: the alarm types the cloud uses (the panel's place was settled on 2026-10-30).

**Goal.** Manage the project's Aodh alarms from Moonlight, for clouds that run Aodh (`alarming`
in the catalog): see which alarms exist and their state (`ok`, `alarm`, `insufficient data`),
what they watch, their history, and create, edit, enable/disable, reset the state of and delete
them. Upstream Horizon has no maintained Alarming panel (the old *Telemetry* one was removed), so
there is no Horizon layout to copy; the production Horizon of item 5's review has no such group
either (check). Alarms also appear in Heat templates (`OS::Aodh::*` resources, roadmap 62), whose
*Resources* tab could then link to them.

What the user gets:

- *Alarms* list (new *Monitoring* group, or under *Project › Compute*; **open question**): Name
  (linked), Type, State (pill: `alarm` red, `ok` green, `insufficient data` grey), Severity
  (`low`, `moderate`, `critical`), Enabled, what it watches (resource or query, short), Actions.
- Detail: overview (rule shown per type: metric, aggregation, comparison, threshold, granularity,
  evaluation periods; or the PromQL query and threshold for `prometheus` alarms), actions
  (`alarm_actions`, `ok_actions`, `insufficient_data_actions` URLs: webhooks, Heat scaling
  signals, Zaqar), repeat actions, time constraints, and a *History* tab (state changes and rule
  changes, newest first).
- Actions: *Create Alarm* and *Edit Alarm* (form per type, see below), *Enable*/*Disable*,
  *Set State* (Aodh's `PUT …/state`, mostly for testing a webhook), *Delete Alarm* (destructive
  dialog, bulk delete on the list).

**Implementation.**

- Gating like Heat (roadmap 62): `model::Plugin::Alarming` (`name()` `alarming`, also the
  catalog type), `needs::ALARMING = All[Plugin(Alarming), Service("alarming")]`, the group's or
  panel's `requires` in `components/nav.rs`, `plugin_enabled(Plugin::Alarming)?` in every server
  fn, the actions in `OsAction::plugins`. Off by default (`[plugins] enabled`).
- No Aodh API in the SDK: `JsonRequest::alarming` like `orchestration`/`key_manager`
  (`server/actions.rs`). **Check** what Aodh's catalog URL looks like on the cloud (versioned or
  not; paths are `v2/alarms…`).
- Reads (`fetch.rs`, GET only): `GET v2/alarms` (Aodh filters by query; the project's by default,
  but admins may see more: filter by `project_id`, like Neutron in AGENTS.md §6),
  `GET v2/alarms/{id}`, `GET v2/alarms/{id}/history` (paged with `limit`/`marker`, newest first),
  `GET v2/alarms/{id}/state`. Models `AlarmRow`, `AlarmDetail` (rule as an enum per type),
  `AlarmChange`; pure mappers tested with Aodh-shaped JSON, unknown types shown as raw JSON.
- Writes (`OsAction`, dry-run tests asserting each body): `AlarmCreate(AlarmSpec)` → `POST
  v2/alarms`; `AlarmUpdate { id, spec }` → `PUT v2/alarms/{id}` (Aodh replaces the alarm:
  send the whole spec); `AlarmSetEnabled { id, enabled }` (the same PUT with `enabled`);
  `AlarmSetState { id, state }` → `PUT v2/alarms/{id}/state` with the bare JSON string;
  `AlarmDelete { id }` → `DELETE v2/alarms/{id}`. Action URLs (`http(s)://`, `trust+http(s)://`,
  `log://`, `zaqar://`) checked server-side.
- Types: start with the ones the cloud runs. **Open question, check on ttcp-dev:** RHOSO's
  telemetry is Prometheus-based (`prometheus` alarms with a PromQL `query`), older clouds use
  Gnocchi (`gnocchi_resources_threshold`, `gnocchi_aggregation_by_*_threshold`); `event`,
  `composite` and `loadbalancer_member_health` exist too. The form needs one set of fields per
  type (`Field::…when("type", …)`); list and detail can show every type from the start.
- Slices, in order: (1) list and detail with history (read-only); (2) delete, enable/disable,
  set state; (3) create/edit for the cloud's type(s); (4) links from Heat's `OS::Aodh::*`
  resources (`mapping::resource_href`).
- i18n for every label, state and severity, both languages. Verify on ttcp-dev in dry-run.

**Open questions for the user.** Where the panel goes (a new *Monitoring* group or under
*Compute*). Which alarm types matter (depends on the cloud's telemetry). Whether ttcp-dev and the
production cloud have Aodh at all: answered for ttcp-dev (yes, see the note above); the
production cloud is unknown.

## 80. 🔒 WebSSO sign-in (OIDC/SAML via Keystone)

> ✅ **Done.** Slices 1 to 4 done (2026-10-29/30), checked end to
> end on ttcp-dev with `tef-adfs` (OIDC, ADFS): sign-in, logout to ADFS's
> `/adfs/oauth2/logout`, and a new sign-in that asked for the password again. Settings: `[websso] enabled`,
> `choices` (`{ id, label }`), `initial_choice`, `idp_mapping` (`id → { idp, protocol }`),
> `keystone_url` (Horizon's `WEBSSO_KEYSTONE_URL`, default `[keystone] url`, `/v3` optional)
> and `logout_url`, validated by `settings::websso` into `Config::websso`
> (`server/websso.rs::WebSso`) and `LoginPrefs::{websso_choices, websso_initial}`. Enabled
> needs `[session] public_url`; IDs are `[A-Za-z0-9._-]`, unique, at least one besides
> `credentials`; mapping keys must be federated choices. Documented in `CONFIGURATION.md`
> (*WebSSO*) and `moonlight.conf.example` (off). The login page shows *Authenticate using*
> when there are two or more choices (no `name`, so `login` never gets it); a federated
> choice hides domain, user and password (`.login-fields`, `display: contents`) and the
> button becomes a link (`rel="external"`, or the router catches it and shows its 404) to
> `GET /auth/websso/start?choice=…` (`prefs::WEBSSO_START`, axum route in `main.rs`). That
> route sets the nonce cookie and answers `303` to `WebSso::start_url` (both Keystone URL
> shapes, IdP/protocol segments percent-encoded, `origin` = public origin + `/auth/websso`,
> query-escaped); `400` for any other choice, `404` with WebSSO off.
>
> Callback (slice 3, login-CSRF guard chosen by the user: nonce cookie). `start` sets
> `__Host-ml_sso` (`HttpOnly; Secure; SameSite=None; Path=/; Max-Age=600`), a private
> (AES-GCM) cookie holding only its expiry; `POST /auth/websso` (`websso::callback`, the only
> path `same_origin_only` lets through, via `websso::is_callback`, and only with WebSSO on)
> needs that cookie valid and unexpired, not yet used (`AppState::websso_used`, keyed by
> hash of the cookie value, atomic `get_with`; per process), and `Origin` absent, `null` or
> Keystone's (`origin_allowed`). Then `os::login_with_token` (`v3token`, unscoped: validates
> the token and returns a new unscoped one) and `os::new_session` (factored out of
> `api::login`); the session is refused if its cookie would pass 4096 bytes
> (`session::fits_in_a_cookie`: federated tokens can carry IdP groups; 940 bytes seen).
> The callback sets the session cookie in its answer to Keystone's POST and `303`s to
> `/project` (`websso::signed_in`), as Horizon does; the browser keeps that `SameSite=Lax`
> cookie. Pitfall found on ttcp-dev: axum's `[(SET_COOKIE, a), (SET_COOKIE, b)]` *inserts*,
> so the nonce removal replaced the session cookie; `signed_in` uses `AppendHeaders` (test
> `signing_in_sets_the_session_cookie_and_deletes_the_nonce`). A same-site hop page tried
> before finding that was removed again. `start` logs `websso start` with the choice. Refusals go to
> `/login?sso=expired|failed|noprojects` (`Refusal::code`), shown by the login page
> (`K::SsoNotStarted`, `LoginFailed`, `NoProjects`). The session's domain is the token
> user's domain name, else `Federated`. Logged as `login` with `method = "websso"`, never the
> token. `session::require` now logs why a page load went to the login page (no cookie,
> doesn't decrypt, logged out, expired). Tests in `server/websso.rs` and `server/session.rs`.
>
> Slice 4. `Session::websso: Option<SsoVia { identity_provider, protocol }>` (serde default
> `None`) marks WebSSO sessions; the choice travels in the nonce (`Nonce::choice`) and
> `WebSso::via` turns it into the provider (only with `idp_mapping`) and protocol. Domain: the
> token's user domain name, else its ID (`os::login_result`), which is what the Identity panels
> pair with `users/{me}`'s `domain_id`; the login form's domain logic never reads the
> session. API Access (user's choice): a WebSSO session gets a `v3websso` `clouds.yaml`
> (`downloads::WEBSSO_CLOUDS_YAML`, `templates/websso_clouds.yaml.j2`, after the user's
> ttcp-dev example; `[api_access] websso_clouds_yaml_template`; template variables
> `identity_provider`, `protocol`) and a note that `openrc` needs a Keystone password, linking
> to application credentials (`Credentials::federated`, `K::FederatedApiAccessNote`). Logout:
> `api::logout` revokes as before, then redirects WebSSO sessions to `[websso] logout_url`
> when set (Leptos follows an off-site server-function redirect with `set_href`), since
> Moonlight can't delete the IdP's cookies. Documented with the `mod_auth_openidc`
> `?logout=` form; on ttcp-dev ADFS's own logout URL was enough.
>
> API Access checked in the browser by the user. Item 77 has the note on why keep-alive can't
> renew WebSSO sessions. Login page polish (user's request): *Authenticate using* and the
> `[login] domain_choices` dropdown use the themed `Combo` (`components/select.rs`; the domain
> one with a hidden `domain` input), labels from `pages::login::sso_label` (tested), and the
> "PoC is read-only" footer is gone (`K::ReadOnlyHint` stays as the header badge's tooltip).
> Slice 5 dropped by the user. Nothing left. Slice 5 (Identity › Federation panels) was
> dropped by the user (2026-10-29): Moonlight will not have them.

**Goal.** Let users of a federated cloud sign in through their identity provider (Keycloak,
Azure AD, ADFS…) instead of a Keystone password, as Horizon does with `WEBSSO_ENABLED`. Horizon's
behaviour:

- The login form gets an *Authenticate using* select (`WEBSSO_CHOICES`, e.g. *Keystone
  Credentials*, *OpenID Connect*, *Security Assertion Markup Language*), preselected with
  `WEBSSO_INITIAL_CHOICE`. Credentials keep today's form; any other choice hides username and
  password and sends the browser to Keystone.
- The redirect is `{keystone}/v3/auth/OS-FEDERATION/websso/{protocol}?origin={dashboard}/auth/websso/`,
  or, when `WEBSSO_IDP_MAPPING` maps the choice to an `(idp, protocol)` pair,
  `…/OS-FEDERATION/identity_providers/{idp}/protocols/{protocol}/websso?origin=…`. Keystone's
  Apache (`mod_auth_openidc`, `mod_shib`) does the IdP dance; Keystone then answers with an
  auto-submitting HTML form that **POSTs** `token=<unscoped token>` to `origin`, which must be
  listed in Keystone's `[federation] trusted_dashboard`.
- Horizon validates that token (`GET /v3/auth/tokens`, `X-Subject-Token`), lists
  `GET /v3/auth/projects` and continues as after a password sign-in. Logout revokes the token;
  `WEBSSO_DEFAULT_REDIRECT_LOGOUT` can also send the browser to the IdP's logout URL.
- Horizon also has Identity › Federation (identity providers, mappings, protocols) for admins.
  **Out of scope** by the user's decision (2026-10-29): Moonlight will not have those panels;
  federation is configured in Keystone directly.

**Implementation**, in slices:

1. **Settings.** `[websso] enabled` (default `false`), `choices` (list of `{ id, label }`, the
   first being `credentials`), `initial_choice`, `idp_mapping` (`id → { idp, protocol }`),
   `logout_url`. `[session] public_url` (item 48) becomes required when enabled: it builds
   `origin` and must equal Keystone's `trusted_dashboard` byte for byte. Validated in
   `server/settings.rs::config` into `config::Config`, documented in `CONFIGURATION.md` and
   `moonlight.conf.example`, sent to the login page with `LoginPrefs`.
2. **Login page** (`pages/login.rs`): the *Authenticate using* select (i18n both languages);
   for a WebSSO choice, a plain link or GET navigation to a new axum route
   `GET /auth/websso/start?choice=…` that answers `302` to the Keystone URL above (built and
   escaped on the server from the settings, never from the browser's input beyond the choice ID).
3. **Callback.** Axum route `POST /auth/websso` (in `main.rs`, next to `leptos_routes`) reading
   the form field `token`. It is a **cross-site** POST from Keystone's origin, so
   `server::same_origin_only` must let exactly this path through; that makes it a login-CSRF
   target (an attacker posts their own token). **Open question:** the guard. Keystone checks
   `origin` by exact string, so a nonce in the URL breaks it; a short-lived `__Host-ml_sso`
   nonce cookie set by `/auth/websso/start` must be `SameSite=None; Secure` to come back on that
   cross-site POST. Accept only with that cookie (and `Origin` = the Keystone host), then delete
   it. Then: validate the token with `v3token` (`os::connect`, `auth_cache: Some(false)`, as in
   `os::projects`), list projects with `list_projects_with`, build the `Session` as `api::login`
   does (factor the shared part out), `session::store`, `303` to `/`. Log `login` with
   `method = "websso"`. Never log the token (§1.5). The throttle (item 50) does not apply: no
   password reaches Moonlight.
4. **Session details.** Federated users have no `password_expires_at`; `user.domain` comes from
   the token (`Federated` in Keystone's response); `domain` in the `Session` must not break the
   username-domain logic of the login form or Identity. Logout: revoke as today
   (`actions::revoke_token`), then redirect to `[websso] logout_url` when set. Item 77's renewal
   by cached password can't work here: such sessions end with the token (say so in item 77).
5. ~~**Federation panels**~~: dropped (see the goal); do not add them.

**Tests.** Settings: defaults, a full `[websso]` section, `enabled` without `public_url`
refused. The start URL for both URL shapes, with escaping. The callback guard: missing,
wrong or reused nonce refused, `same_origin_only` still refusing every other cross-site POST.
Session building from a federated token JSON (mapping test).

**Open questions** (settled). The login-CSRF guard: the nonce cookie. Test cloud: ttcp-dev
federates with ADFS (`tef-adfs`), enough to verify slices 2 to 4. `WEBSSO_KEYSTONE_URL`:
supported as `[websso] keystone_url`.

## 81. Write the server log to a file (`[log] file`, rotation)

> ✅ **Done.** `[log] file` (env `MOONLIGHT_LOG_FILE`), `rotation` (`never` | `hourly` |
> `daily`, default `daily`), `keep` (default 7, 0 = all), `stdout` (default `false`) and `format`
> (`text` | `json`) in `settings.rs::Log` (`Rotation`, `LogFormat` enums: an unknown value is
> refused when the file is read). `server/logging.rs::subscriber` builds a registry with the
> `EnvFilter` and boxed `fmt` layers: the file through `tracing-appender`'s `rolling::Builder`
> (prefix = the file name, so `moonlight.log.2026-10-29`; `never` writes `moonlight.log`) behind
> `non_blocking`, without ANSI; stdout when there is no file or `stdout = true`. `main.rs` keeps
> the `WorkerGuard` for the life of `serve`. A missing directory or an unwritable file stops
> start-up with `configuration error: [log] file …` (exit 1); `tracing-appender` would have
> created the directory. Mode `0640`: `tracing-appender` has no mode option, so `logging.rs`
> tightens the process umask (`rustix`, `| 0o027`) before the first file is opened. Decisions on
> the open questions: rotation by `tracing-appender`, with `never` for `logrotate`
> (`copytruncate`, no `SIGHUP` reopen); `format` shipped now (`tracing-subscriber`'s `json`).
> Tests: `settings::the_log_goes_to_stdout_unless_a_file_is_set` (defaults, full section, unknown
> `rotation`/`format`, the variable) and three in `logging.rs` (text file without colours and not
> group-writable or readable by others, dated JSON file, missing directory). Checked with the
> real binary on port 3010 and the repo's `moonlight.conf`: the start-up lines went to
> `moonlight.log.2026-10-29` (`-rw-r-----`) and nothing to stdout. Documented in
> `CONFIGURATION.md` and `moonlight.conf.example`. Not done: graceful shutdown on `SIGTERM`
> (the guard is only dropped when `serve` returns, so a kill can lose the last queued lines).

**Goal.** Today the log only goes to stdout (`tracing_subscriber::fmt()` in `main.rs::serve`),
which suits systemd's journal and containers but not a plain VM or a `nohup` start, where the
sign-in, write and *cross-origin request refused* lines (items 48, 50, 63) are lost unless the
operator redirects them. Horizon logs through Django's `LOGGING` dict, usually to
`/var/log/horizon/horizon.log`. Operators should be able to point Moonlight at a file with one
setting, keep it from growing forever, and still read the console while developing.

**Implementation.**

- Settings in `server/settings.rs::Log`: `file` (path; unset or `""` = stdout only, today's
  behaviour), `rotation` (`never` | `hourly` | `daily`, default `daily`), `keep` (rotated files
  kept, default `7`), `stdout` (also write to stdout when `file` is set, default `false`), and
  `format` (`text` | `json`, `json` for log shippers; `tracing-subscriber`'s `json` feature).
  Env `MOONLIGHT_LOG_FILE` in the `ENV` table. Documented in `CONFIGURATION.md` (core table,
  next to `[log] filter`) and `moonlight.conf.example`.
- `main.rs::serve`: build the subscriber from layers (`tracing_subscriber::registry()` with the
  `EnvFilter`, a file layer and an optional stdout layer) instead of `fmt().init()`. File layer:
  `tracing-appender`'s `rolling::Builder` (`rotation`, `filename_prefix`, `max_log_files`) behind
  `non_blocking`, so a slow disk never stalls a request; keep its `WorkerGuard` alive in
  `serve` so the last lines are flushed on exit. No ANSI colours in the file (`with_ansi(false)`).
- Fail at start-up, before serving, when the directory doesn't exist or isn't writable: print
  `configuration error: [log] file …` to stderr and exit 1, like the other settings. Create the
  file with mode `0640`: it holds usernames and client addresses (never tokens, §1.5).
- Keep the start-up lines (`configuration files, in order: …`) going to the configured outputs:
  logging is set up right after `settings::load`, as today.
- Tests in `settings.rs`: defaults, a full `[log]` section, unknown `rotation`/`format`
  refused, `MOONLIGHT_LOG_FILE`. A unit test for the writer factory (a temp dir gets a file
  after one event and the guard is dropped).

**Open questions.** Rotation by Moonlight (`tracing-appender`, proposed) or by the system's
`logrotate`: the latter needs `copytruncate` or reopening the file on `SIGHUP`, which
`tracing-appender` doesn't do; `rotation = "never"` leaves that path open. Whether the format
setting is wanted now or later.

## 82. Project › Quotas: every quota of the project, with usage, editable with permission

> ✅ **Done** (2026-11-01; started 2026-10-29). **Built:** `/project/quotas` (`pages/quotas.rs`, nav
> between Overview and API Access): a card per service with *Quota*, *Used*, *Reserved* (only when
> non-zero), *Limit* and a bar (70 % / 90 % as the Overview's donuts). `fetch::quotas` reads Nova
> `quota_set::details_20`, Cinder `quota_set::get?usage=true`, Neutron `quota::details` and, with
> the `load-balancer` plugin and Octavia in the catalog, `quota::get` plus `lbaas/quotas/defaults`
> for `null` limits, in parallel, each failing on its own. `mapping::quota_rows` keeps every key
> but Nova's pre-2.36 network proxies and Octavia's deprecated `loadbalancer` / `healthmonitor`
> (`health_monitor` is the current name: ttcp-dev returns only that), editable keys first
> (`QuotaService::keys`), then Cinder's per-type keys grouped by type. Labels in
> `ops::quota_label` (unknown keys show the API name). *Edit Quotas* per card
> (`ops::edit_quotas_dialog`, everyone sees it) sends `OsAction::QuotaUpdate { service,
> project_id, values }` with only the changed keys (`model::quota_changes`, refusing a limit below
> usage unless `-1`); `actions::quota_update` sends a JSON `PUT` (`os-quota-sets/{p}` for Nova and
> Cinder, `quotas/{p}`, `lbaas/quotas/{p}`) and refuses keys outside `keys()`. A 403 shows
> *You have no permission to edit quotas* through the new `Dialog::forbidden`. Identity's
> project page links *Quotas* here. Tests: mapper shapes of the four services, `quota_changes`,
> units, dialog fields and build, labels for every key, bar percentages, the dry-run of each
> service's `PUT`. **Measured on ttcp-dev** (admin2): 4 cards (Compute 10, Volumes 19 with 4
> volume types, Network 10, Load Balancers 7), 5 API calls in 0.25–0.30 s; dialogs open with the
> current limits (unlimited as `-1`); no console errors; only GETs. **Saves tried on ttcp-dev**
> (2026-10-29, admin2 with `power_user` on project *pokemon*, +1 on Instances, Volumes and
> Networks): the three `PUT`s reached the right URLs and all were refused with 403 by each
> service's policy (`os_compute_api:os-quota-sets:update`, `volume_extension:quotas:update`,
> `rule:update_quota`). Each dialog showed *You have no permission to edit quotas*, the limits
> stayed as they were, no console errors. **Barbican** (2026-10-29): with the `key-manager`
> plugin and Barbican in the catalog, a *Key Manager* card (`QuotaService::KeyManager`: secrets,
> containers, orders, consumers, CAs). Barbican reports no usage, so `mapping::key_manager_quota_rows`
> reads `GET v1/quotas` (the token's project, defaults applied) plus the `total` of
> `secrets` / `containers` / `orders?limit=1`, each soft (`-` when one fails); consumers and CAs
> show `-`. Those lists skip secrets whose creator turned project access off (unless shared with
> the user), so a note under the table (`K::KeyManagerUsageNote`) says *Used* covers only what
> the user can see. *Edit Quotas* sends `PUT v1/project-quotas/{p}` `{"project_quotas": {…}}` (Barbican
> updates only the keys sent), in `OsAction::plugins` as `KeyManager`. Tests: the mapper, the
> dry-run, the plugin, labels. Measured on *pokemon*: 5 cards, 9 API calls in 0.53 s; Secrets 2
> and Containers 1 match their panels, all unlimited; dialog opened (not submitted). **Not
> verified:** a successful save (needs a user the policy lets update quotas, e.g. `admin`), on
> any service.
>
> **Modify Quotas from Identity built (2026-11-01; page, as the user chose over a tabbed
> dialog).** With the `admin` role (`needs::ADMIN`), Identity › Projects shows *Modify Quotas*
> after *Edit Project* (Horizon's order) in each row menu and on the project page
> (`ops::identity_project_items(…, admin)`), a link to `/identity/projects/{id}/quotas`
> (`pages/quotas.rs::IdentityProjectQuotasPage`: breadcrumbs Identity › Projects › name ›
> Quotas, the same cards through `QuotaCards`, each *Edit Quotas* for that project). The project
> page's *Quotas* link goes there for another project (admins), to Project › Quotas for the
> current one. Server: `api::list_project_quotas(id)` (`scoped_id`, then `require_admin` before
> any OpenStack call), `fetch::quotas` gains `session_project`, `fetch::project_name` names the
> project (ID on failure), `model::ProjectQuotas`. Barbican for another project:
> `project-quotas/{p}` (404 = nothing set) with no *Used*; keys without their own value use the
> defaults only when the session's project has none of its own (then `v1/quotas` are the
> defaults), else they are left out (`mapping::key_manager_project_rows`; card note
> `K::KeyManagerOtherProjectNote`). Writes: `QuotaUpdate` unchanged; `run_action` requires the
> role when `OsAction::other_projects_quotas` (also inside a batch). Tests: the menu (admins only,
> order, link), `other_projects_quotas`, the Barbican mapper (defaults known, unknown, nothing
> set); both clippy runs clean, 478 tests pass. **Browser** (ttcp-dev, `zc01001` on *pokemon*,
> no `admin`): no *Modify Quotas* in the menu, and the page opened by URL shows *Access denied*.
> As an admin, tested by the user on 2026-11-01: OK.
>
> ✅ **Done** (2026-11-01). DNS and Swift quotas join their panels when those are built.

**Goal.** A *Quotas* panel under Project (after Overview, `/project/quotas`) listing **every**
quota the cloud's services report for the current project, not only the nine of the Overview's
*Limit Summary*: one card per service (Compute, Volume, Network, and Load Balancer with
Octavia), each a table of *Quota*, *Used*, *Reserved* (when the service reports it), *Limit*
(`Unlimited` for `-1`) and a usage bar, with an *Edit Quotas* button that opens a dialog with
one number field per quota and a tab per service. Today *Quotas* on the project page (item 57)
only links to the Overview.

Horizon spreads this over three places: the Overview's charts (read-only, a subset), Identity ›
Projects › *Modify Quotas* (admin, a workflow with Compute / Volume / Network tabs) and Admin ›
Defaults (the default quota class). This panel replaces the first's detail and the second's
form for the current project; the project page's *Quotas* link and its row action point here.
Default quotas stay out of scope (Admin dashboard, *Also pending*).

**Implementation.**

- **Reads**, in parallel in a new `fetch::quotas(os, project_id)`, each service on its own so
  one failing keeps the others (`QuotaSection::error`, as `fetch::overview`):
  - Nova `compute::v2::quota_set::details_20` (`os-quota-sets/{id}/detail`: `in_use`,
    `reserved`, `limit` per key: instances, cores, ram, key_pairs, server_groups,
    server_group_members, metadata_items, injected_files…; drop `id`, and the keys removed from
    2.36 on, floating_ips, fixed_ips, security_groups, security_group_rules, networks).
  - Cinder `block_storage::v3::quota_set::get` with `usage=true` (volumes, snapshots,
    gigabytes, backups, backup_gigabytes, groups, per_volume_gigabytes, **plus the per-type
    keys** `volumes_<type>`, `gigabytes_<type>`, `snapshots_<type>`, shown grouped under their
    volume type).
  - Neutron `network::v2::quota::details` (already used): every resource it returns, including
    subnet, subnetpool, rbac_policy, trunk with the extension.
  - Octavia `load_balancer::v2::quota::get` when `Plugin::LoadBalancer` is enabled and the
    service is in the catalog (load_balancer, listener, member, pool, health_monitor,
    l7policy, l7rule); usage isn't reported, so *Used* is `-`.
  - Barbican (`key-manager` plugin): done, see the note above. Open: DNS (`dns::v2::quota`) and
    Swift when their panels arrive.
- **Model.** Keep `Quota`/`QuotaSection`, but the Overview's closed `QuotaKey` can't hold
  per-type keys: add a `QuotaRow { service, key: String, used, reserved, limit, unit }` with
  the API's key, a label lookup in `i18n.rs` for known keys (both languages; unknown keys
  shown as the API key, so a new service release doesn't break the page) and units (`ram` MiB,
  `gigabytes*` GiB). Mappers in `server/mapping.rs` with API-shaped JSON tests (`-1`,
  missing `reserved`, per-type keys, Nova 2.36 drops).
- **Page** `pages/quotas.rs`: `Resource` in a `<Transition>`, a `Card` per service with a
  `DataTable`; route in `app.rs`, menu entry in `components/nav.rs` (Project group). Usage bars
  reuse the Overview's `overview::ratio` and `fmt_amount`; 100 % and over shown as Horizon's
  red.
- **Writes**, for admins or whoever the cloud's policy lets update quotas on the project. That
  can't be known beforehand yet (item 35), so the button shows for everyone and nothing is gated
  on `api::is_admin`; a refusal (403, `ErrorKind::Forbidden`) is shown in the dialog as a
  specific error, *You have no permission to edit quotas* (new i18n key, both languages),
  instead of the generic `ErrForbiddenText`, which speaks of *seeing*. When item 35 lands, the
  button follows the policy check instead.
  `OsAction::ComputeQuotaSet { project_id, values }` (`quota_set::set_257` when the cloud's
  compute microversion allows, else `set_236`/`set_20`; a `needs::` entry),
  `VolumeQuotaSet` (`quota_set::set`), `NetworkQuotaSet` (`network::v2::quota::set`) and
  `LoadBalancerQuotaSet` (`load_balancer::v2::quota::set`, in `OsAction::plugins`). `values`
  sends **only the changed keys** (as `model::assignment_changes` does for roles), and
  validation refuses a limit below *Used* unless it is `-1` (Horizon's check; Nova refuses it
  anyway). One dialog submits the services that changed as a batch of actions. Dry-run tests in
  `actions.rs` asserting the method, path and exact body of each (`every_action_has_a_dry_run_test`;
  `project_id` goes through `check_ids`).
- Identity: the project page's *Quotas* link and a *Modify Quotas* row action on Identity ›
  Projects open this panel's dialog for **that** project: the server fns take an optional
  `project_id` (default the session's), so admins can edit another project's quotas as in
  Horizon; the same permission error applies. Details in *Modify Quotas from Identity* below.

**Decided** (2026-10-29): editable by admins or whoever has enough permission on the project;
no gating before item 35, just the permission error on a refused attempt. Everyone sees the
panel, since it is the only place listing every limit.

**Modify Quotas from Identity** (requested 2026-10-31; it belongs to this item, decided then).
With the `admin` role, Identity › Projects shows *Modify Quotas* in each project's row menu and
on its detail page, next to *Manage Members*, *Modify Groups* and *Edit Project*
(`ops::identity_project_items`, `src/components/ops.rs`), so an admin edits **any** project's
quotas, not only the session's. Without the role the item is hidden (`needs::ADMIN`, as the
Admin dashboard), and the server refuses anyway.

- **Server:** `api::list_quotas` (today `fetch::quotas(&c, &s.project_id, …)`) gains an
  optional `project_id`; another project than the session's needs `require_admin` first (as
  roadmap 85's server fns) and goes through `check_id`. `fetch::quotas` already takes the
  project. Barbican's `v1/quotas` reads only the token's project: for another one read
  `project-quotas/{p}` (no usage there; *Used* stays `-`). The writes need no change:
  `OsAction::QuotaUpdate` already carries `project_id`; keep it out of `admin_only` (the
  session's own project stays editable by whoever the policy allows) but have `run_action`
  require the role when `project_id` is not the session's.
- **UI:** the dialog needs the project's current limits and usage, which a menu item can't load
  before opening. Proposed: *Modify Quotas* navigates to `/identity/projects/{id}/quotas`, the
  same `pages/quotas.rs` page (cards, bars, each card's *Edit Quotas*) for that project, with
  the project's name in the header and breadcrumbs (Identity › Projects › name › Quotas); the
  project page's *Quotas* link goes there too for other projects. Alternative closer to
  Horizon: one dialog with a tab per service that loads the cards when it opens (needs a
  dialog that fetches before showing its fields; the engine only loads choice sets today).
- **Tests:** the menu shows *Modify Quotas* only with the role; the server fn refuses another
  project without it (before any call); the Barbican path for another project; dry-run of a
  `QuotaUpdate` for another project (unchanged request). Browser on ttcp-dev with `admin2`:
  another project's page shows its limits, dialogs open (not submitted).

**Open question.** Page (proposed) or tabbed dialog for *Modify Quotas*.

## 83. WebSSO sessions are slower (≈0.5 s → 1.2 s): measure where the time goes, fix Moonlight's part

> ✅ **Done** (2026-10-30): measured; the extra time is not Moonlight's, nothing to fix there.
> Instrumentation kept, at debug level: `api::scoped` logs each server function's `connect_ms`
> (session + connection), `body_ms` and request count (`fetch::serving_counted`), with `user`
> and `websso`; `os::scoped_to` logs connection cache hits and, on a miss, `rescope_ms`,
> `discovery_ms` and whether it was cached. The SDK's own `http_request` events
> (`openstack_sdk::openstack_async=info`) give each request's URL and duration; its token
> header is marked sensitive and bodies are only logged at TRACE (no token found in the log).
> Measured on ttcp-dev, same project, `admin2` (password) vs `zc01001` (WebSSO, `tef-adfs`),
> with `RUST_LOG="moonlight=debug,openstack_sdk::openstack_async=info,warn"`: both hit the
> connection cache (`connect_ms` 0–2) and make the same requests (Instances 1, Overview 3).
> Requests **without** a token (version documents of Keystone, Nova, Neutron, Glance) take
> 35–59 ms for both; every request **with** the token is ≈0.4–0.6 s slower for the WebSSO user
> (Cinder `/v3/` 156 → 714 ms, Neutron `/v2.0/extensions` 110 → 590, Glance `/v2/images`
> 139–171 → 639–854, Nova `flavors/detail` 201 → 675, `servers/detail` 595–837 → 1140–1701;
> Keystone rescope 172 → 234–586). The cost is in the services' token validation
> (keystonemiddleware asking Keystone, which for a federated user resolves roles through its
> IdP groups) and it repeats on every call, so the services don't seem to cache validations.
> For the operators: check `[keystone_authtoken] memcached_servers` / `token_cache_time` in
> Nova, Cinder, Glance and Neutron, and Keystone's own caching (`[cache]`, `[identity]` and
> `[assignment]` caching) for federated users. Not proven: whether it is federation or this
> user (many groups or role assignments); a federated user with few groups would tell. The
> user's view (2026-10-30): Keystone's token caching. To confirm without Moonlight, time
> `GET /v3/auth/tokens` (`X-Auth-Token` and `X-Subject-Token` = the same token) a few times per
> user: a WebSSO token that stays slow on every repeat means Keystone doesn't cache
> validations (`[cache] enabled` with memcached, `[token] caching`, `cache_on_issue`).

**Goal.** The user measured (2026-10-30, ttcp-dev, `tef-adfs`) that requests which take about
0.5 s for a password user take about 1.2 s for a user who signed in through WebSSO (item 80).
Find out where the extra 0.7 s goes, with numbers, and remove whatever part of it is Moonlight's.
If it is Keystone's or the services', say so with measurements and the Keystone or
keystonemiddleware setting that would help, since Moonlight can't change it.

What is known (from the code, 2026-10-30): both kinds of session take the same path after
sign-in. `os::scoped_to` rescopes the session's unscoped token (`v3token` + project) once per
token, project and region and caches the connection in `AppState::connections` (15 min idle),
but only if `os::discover` reaches every service; `AppState::projects` and `capabilities` are
keyed the same way. The only WebSSO differences are the token itself (a federated token,
re-issued by `os::login_with_token`) and the user's roles, which come from IdP groups through
the mapping. That day's log had no `discovery of … failed` lines, so a connection that is never
cached is not proven.

**Implementation**, measure first:

1. **Numbers.** Same project and region, one password session and one WebSSO session (the user
   signs in; never type credentials): compare the `.timing` of Overview, Instances, Networks and
   Volumes (milliseconds and call count from `fetch::timed`), first load and reload. Equal call
   counts with a higher time point at the services or Keystone; more calls point at Moonlight.
2. **Where.** Debug-level timing (no tokens in it, AGENTS.md §1.5): in `os::scoped_to`, cache
   hit or miss and how long the rescope and `discover` took; per request in `fetch::counted`,
   the service and duration. Enable with `[log] filter = "moonlight=debug,…"`.
3. **Likely causes and what to do:**
   - *Connection cache misses* (Moonlight's): a federated scoped token whose catalog makes
     `discover` fail for one service, so every request rescopes and rediscovers. A rescope plus
     discovery costs several round trips, which would fit 0.7 s. Fix: cache the connection
     anyway with a shorter TTL, or rediscover only the missing service.
   - *Token validation in the services* (not Moonlight's): each service's keystonemiddleware
     validates the token against Keystone; for federated tokens Keystone recomputes the
     group-based roles. Without the services' token cache (`[keystone_authtoken]
     memcached_servers`, `token_cache_time`) every call pays it. Report with the numbers.
   - *Different data*: the WebSSO user's roles (admin or not) change what lists return. Compare
     the call counts and row counts before blaming the token.
4. Record the measurements here and in the item's progress note either way.

**Tests.** A pure helper for whatever caching rule changes (e.g. the TTL decision), with fixed
times; no network.

**Open questions.** Which pages show it (all, or some services more than others). Whether the
password user and the WebSSO user being compared have the same roles in the same project.

## 84. Server log: a restart deletes the current log file with `keep = 1`

> ✅ **Done** (built earlier, recorded 2026-11-01). Moonlight prunes old files itself
> (`server/logging.rs::prune`, at start-up and then hourly), never the current one, and no longer
> passes `max_log_files` to `tracing-appender`, so a restart appends to the current file with every
> rotation, `never` included. `[log] keep` is now a yes/no (append on restart, or start empty);
> the file count moved to `[log] max_files` (default 7, `0` = all). A numeric `keep` is read as
> `max_files` with a start-up warning (`main.rs`). Documented in `CONFIGURATION.md` and
> `moonlight.conf.example`. Tests: `a_restart_keeps_the_current_file_unless_keep_is_off` (daily
> and `never`, `max_files = 1`) and `pruning_keeps_the_newest_files_and_ignores_others`; 469 tests
> pass, both clippy runs clean.

**Goal.** Every restart of Moonlight (each `cargo leptos watch` rebuild in development, each
deploy in production) must append to the current log file, not start an empty one. Seen on
2026-10-30 with `rotation = "daily"`, `keep = 1`: `moonlight.log.2026-10-30` only held the lines
since the last start, so the Aodh error of roadmap 79 (a refused `log://` action) was gone by the
time it was looked for. The log is also the record of who wrote what (`write failed`, `login`),
so losing it on restart is losing that record.

**Cause (checked in `tracing-appender` 0.2.5, `src/rolling.rs`).** `Inner::new` runs
`prune_old_logs(max_files)` when the appender is built, before opening the current file, and the
pruning keeps `max_files - 1` files ("because we will create another log file"), oldest deleted
first. With `max_log_files(1)` (our `keep = 1`) it keeps none, so the current file is deleted and
recreated empty on every start: today's `moonlight.log.<date>` with `daily`/`hourly`, and
`moonlight.log` itself with `never`, which also defeats `logrotate` with `copytruncate`, the
setup `moonlight.conf.example` recommends for `never`. With `keep` of 2 or more the current file
survives (it is the newest); `refresh_writer` prunes the same way at each rotation, which is
right there (the previous period's file goes).

**Implementation.**

- `server/logging.rs::appender`: don't hand `keep` to `max_log_files` as it is. Options, to
  decide: (a) pass `keep + 1`, so the build-time pruning keeps `keep` files, and document that a
  rotation then briefly leaves `keep + 1`; (b) for `never`, never pass `max_log_files` (there is
  only one file, and `logrotate` owns rotation); (c) do the start-up pruning ourselves (skipping
  the current period's file) and pass `max_log_files` only for rotations. (b) is needed in any
  case; (a) is the smallest change for daily/hourly.
- Settings: `keep = 0` (keep all) stays unlimited; check how `settings.rs` validates `keep`.
- Test in `logging.rs` with a temporary directory: write a line, build the appender again with
  the same settings (a restart), and assert the first line is still in the file, for `daily`
  with `keep = 1` and for `never`; and that a stale file of an older period is still pruned.
- Docs: `CONFIGURATION.md` and `moonlight.conf.example` say what `keep` counts after the change.

## 85. Admin dashboard (Horizon's *Admin*: Compute, Volume, Network, System)

> ✅ **Done** (2026-11-01; started 2026-10-30). **Slice 1 built:** the dashboard shell and
> System › System Information. `nav::ADMIN_NAV` (one group so far, *System*, gated by
> `needs::ADMIN` = `AnyRole(["admin"])`), `/admin` routes in `app.rs` (`/admin` redirected to
> `/admin/info` until Admin › Overview existed), *Admin* in the dashboard switcher between Project
> and Identity (`layout.rs`, only with the role). `pages/admin.rs::SystemInfoPage`: tabs *Services*
> (the token's catalog, every region and interface: `os::catalog_services`, no extra call, as
> Horizon), *Compute Services* (Nova `service::list_20`), *Block Storage Services* (Cinder
> `service::list`, body unwrapped from `services`), *Network Agents* (Neutron `agent::list`,
> needs the `agent` extension, `needs::NETWORK_AGENTS`), and *Detected capabilities*
> (`ModulesCard`, moved here from `lists.rs`). `fetch::system_info` runs the three in parallel,
> each failing on its own (`model::Fallible`), and skips a service the catalog lacks (its tab is
> hidden). Timestamps normalised by `mapping::utc_rfc3339` (Nova/Cinder naive UTC, Neutron with a
> space). `api::get_system_info` calls `require_admin` (the token's roles, before any call).
> Also fixed: a page refused for a missing role (Admin, Identity › Groups/Roles) said *Service not
> available in this region*; `nav::unavailable` + `Requires::lacks_role` now show *Forbidden*,
> through a `Memo` in `layout.rs` so navigation doesn't rebuild the outlet. Tests: mappers
> (timestamps, `os-services`, agents), catalog rows per region, tabs per cloud, `Fallible`, the
> nav gate and the refusal kind. Both clippy runs clean, 403 tests pass. **Browser** (ttcp-dev,
> project with `admin`, 2026-10-30): *Admin* active in the switcher, group *System*, five tabs;
> rows match Horizon's System Information (Compute Services 15, Block Storage 6, Network Agents
> 18, Services 11 once endpoint-less services are skipped like Horizon: ttcp-dev's `ceilometer`
> has none), 3 API calls in 0.35–0.73 s, only `GET`s, no console errors. In a project without
> `admin` the switcher hides *Admin*; the *Forbidden* page itself is covered by tests, not seen.
> **System group complete (read-only), 2026-10-30:** *Defaults* (`/admin/defaults`,
> `DefaultQuotasPage`: Compute / Volume / Network Quotas tabs, *Quota Name* and *Limit*;
> `fetch::default_quotas` reads Nova's and Cinder's `default` quota class and Neutron's
> `quotas/{project}/default` in parallel, each failing on its own, reusing `mapping::quota_rows`
> and `ops::quota_label`; Nova at **2.50**, since 2.1 lacks `server_groups` and
> `server_group_members`, and 2.57 would drop the injected-file quotas Horizon lists) and
> *Metadata Definitions* (`/admin/metadata_defs`, `MetadefsPage`, needs Glance: display name
> with the namespace under it, description, resource types, public, protected;
> `fetch::metadef_namespaces` pages Glance with `limit`/`marker` through a raw GET, as the SDK's
> list has no paging). Checked on ttcp-dev against Horizon: Defaults 10 / 19 / 10 rows, same
> values (3 API calls, 0.39 s); 33 namespaces on one page (Horizon pages 20), 1 call in 2 s; no
> console errors. 406 tests pass, both clippy runs clean. Not built yet: *Update Defaults*, the
> namespace detail page and its actions (slice 4).
> **Slices 2 and 3 built (read-only), 2026-10-30:** all four groups in `nav::ADMIN_NAV`, Horizon's
> URLs in `app.rs` (`/admin` opened Hypervisors until Overview existed), one
> `#[server]` per panel in `api.rs` (`require_admin` first) and a fetcher per panel in `fetch.rs`
> (`admin_*`). Cloud-wide: *Hypervisors* (compute **2.53**, still with the usage fields gone in
> 2.88; tabs *Hypervisor* and *Compute Host*), *Host Aggregates* with the *Availability Zones*
> table, *Flavors* (`is_public=None`, compute **2.61** for `extra_specs` as *Metadata*), *Volume
> Types* with *QoS Specs* (encryption: one GET per type, as Horizon), *Group Types*, *RBAC
> Policies* (projects and networks named, `*` as *All projects*). Every project's resources:
> Instances, Images, Volumes, Snapshots, Groups and Group Snapshots (volume **3.58** for
> `project_id`), Networks, Routers, Floating IPs, Trunks, through `mapping::owned` (rows of the
> Project mappers plus *Project*, named from Keystone, and *Host*) and the `admin_list!` macro in
> `pages/admin.rs`; rows link to the Project detail pages, which open for other projects'
> instances, volumes and networks (checked). Mapper tests next to the code (owned rows,
> hypervisors/aggregates/zones, flavors, volume types/QoS/group types, RBAC).
> **Also fixed** (`components/layout.rs`): the page gate read `dialogs.caps`, filled only by a
> browser effect, so SSR used the default capabilities, where `Requires::Plugin` fails: every
> direct load of a plugin page (Trunks, Load Balancers, Stacks…) rendered *Service not
> available* until hydration. It now reads `info` inside a `<Transition>`; SSR and client agree.
> **Browser** (ttcp-dev, 2026-10-30, page loads, API calls, server time): Hypervisors 6 rows (2,
> 0.34 s), Aggregates 19 + 3 zones (2, 0.72 s), Instances 186 (4, 3.15 s), Flavors 213 (1, 1.47 s),
> Images 31 (2, 2.49 s), Volumes 322 (3, 1.22 s), Snapshots 15 (3, 1.72 s), Volume Types 4 + 1 QoS
> (6, 2.37 s), Groups 0, Group Snapshots 0 (the first count, 1, was the empty-table row; Horizon
> agrees), Group Types 1, Networks 112 (3, 1.41 s), Routers 64, Floating IPs 37, Trunks 0 (none in
> any project), RBAC 33 (3, 0.51 s). Only `GET`s, no console warnings. Row counts not compared with
> Horizon this time. 411 tests pass, both clippy runs clean.
> **Overview built, 2026-10-30, not yet seen in the browser** (compiled and tested on
> 2026-10-31 with the rest, below):
> `/admin` is `AdminOverviewPage`, in a first nav group *Admin* (`nav::matches` gives a
> dashboard's root panel only its exact path, test added); the switcher links to it.
> `api::get_admin_usage` (`require_admin`, `model::usage_range`: empty dates = this month) calls
> `fetch::admin_usage`: Nova `simple_tenant_usage::list_21` with `detailed=1`, paged by instance
> like the per-project `usage` (`limit` counts server usages across projects, so one project can
> span two pages: `mapping::tenant_usages` merges by `tenant_id` and names from Keystone, IDs for
> unknown projects). `model::ProjectUsage` sums like Horizon's `NovaUsage` (VCPUs, disk and RAM
> of servers that still exist; hours of all), `GlobalUsage::csv` follows Horizon's
> `GlobalUsageCsvRenderer` (totals, then *Project Name, VCPUs, RAM (MB), Disk (GB), Usage
> (Hours)*). The CSV is a data URL, as Project's, so `downloads.rs` is not involved.
> `pages/overview.rs` now exports `period_form` and `UsageTotals`, shared by both overviews.
> Tests: page merging and sums (`tenant_usages_merge_pages_and_name_projects`), totals and CSV
> (`global_usage_totals_every_project_and_csv_has_a_line_each`).
> **Also, 2026-10-30:** (a) the header read `dialogs.caps`, filled only after hydration, so
> after a project switch (a full reload) a non-admin saw *Admin* for a couple of seconds, and the
> plugin groups (Key Manager, Orchestration, Monitoring) appeared late. The switcher and
> `PrimaryNav` now read `info` in a `<Transition>`, and the gate sets `dialogs.caps` before
> rendering the outlet, so pages' `caps()` are right on the server too. Checked: server HTML
> and live header identical, and in project *pokemon* (no `admin`) no *Admin* link and
> `/admin/hypervisors` shows *Access denied*. (b) Zones are `AzPill`s in every table, as Project
> › Instances: Admin's Compute Host, System Information services and agents, aggregates, the
> *Availability Zones* table, volume groups, networks and routers (`widgets::AzPills`, one per
> zone), and Project's volume groups, networks, routers and load balancers.
> Trunks: ttcp-dev does have the `trunk` extension; 0 trunks across projects is real.
> **Written 2026-10-30 without builds, compiled and tested 2026-10-31** (fixes: a duplicate
> `K::SegmentationId`, two `search=` closures with generic types that `view!` can't parse, now
> named functions, four clippy lints, a nav test indexing `ADMIN_NAV` by position; the *Admin*
> group now also needs compute, as Overview is Nova's usage). Both clippy runs clean, 434 tests
> pass. Not yet seen in the browser:
> (1) Networks' *DHCP Agents* column (`NetworkRow::dhcp_agents`, `needs::DHCP_AGENTS` =
> `dhcp_agent_scheduler`): the DHCP agents are listed (`agent::list`, `agent_type`) and each
> one's `agent::dhcp_network::list` asked, one call per agent instead of Horizon's one per network
> (`mapping::dhcp_agent_counts`, tested). (2) Flavor access: `FlavorRow::access`, the project names
> under *No* in *Public*, one `flavor_access::list_21` per private flavor in parallel
> (`mapping::flavor_access`, tested). (3) Hypervisors' *Resource Provider* tab, shown when the
> catalog has `placement`: raw GETs (`resource_providers`, then each one's `inventories` and
> `usages`; the SDK's `placement` feature is off), `mapping::resource_provider` (tested), cells
> `used / total` with reserved and allocation ratio. (4) Horizon's *Hypervisor Summary* donuts
> (VCPU, memory, local disk; `Hypervisors::totals`, tested; `overview::Donut` now public with a
> `label`). (5) Admin › Networks rebuilt by hand (`AdminNetworksPage`): row checkboxes and bulk
> *Delete Networks*, *Create Network* (`ops::admin_create_network_dialog`: project, provider
> type, physical network for flat/VLAN, segmentation ID, admin state, shared, external, then
> Project's subnet steps) and each row's *Edit Network*, *Create Subnet*, *Delete Network*
> (`ops::admin_network_items`, Project's menu minus *Create Port*, for every project's
> network). New `OsAction::AdminNetworkCreate(AdminNetworkSpec)`: `POST /networks` with
> `tenant_id`, `shared`, `router:external` and `provider:*`, validated by
> `ProviderNetwork::validate` (Horizon's default types and segmentation ranges,
> `PROVIDER_NETWORK_TYPES`); dry-run test `admin_network_sends_owner_provider_and_flags`, dialog
> test, validation test. `OsAction::admin_only` and `run_action` refuse it without the `admin`
> role on the token. (6) `admin_list!` now filters columns with `available` (Floating IPs' DNS
> columns showed on clouds without DNS). (7) Admin *Edit Network*, as read from the ttcp-dev
> Horizon's form (GET only): *Name*, *Enable Admin State*, *Shared*, *External Network*;
> `OsAction::AdminNetworkUpdate` (`PUT /networks/{id}` with `shared` and `router:external`,
> dry-run test; `admin_only`), `ops::admin_edit_network_dialog`. Horizon has no provider fields
> there and ML2 refuses to change `provider:*`, so they are shown read-only in the dialog's intro
> (`i18n::provider_summary`), from `NetworkRow::provider` (`mapping::provider_network`, tested;
> Neutron returns them to admins only). (8) Horizon's *Create Network* gaps, read the same way:
> *Local* preselected; *MTU* (`AdminNetworkSpec::mtu`) and *Availability Zone Hints* (`az_hints`, new
> `ChoiceSet::NetworkZones` from Neutron's `availability_zone::list` with `resource=network`,
> `mapping::zone_choices`, tested; shown with `needs::NETWORK_AZ`); in the subnet steps,
> shared with Project's dialogs, *IPv6 Address Configuration Mode* (IPv6 only, Horizon's five
> pairs, `model::ipv6_modes`, tested) and *Host Routes* (`destination,nexthop` lines, checked by
> `validate_route` in the dialog and again in `create_subnet`): `SubnetSpec` gained
> `ipv6_ra_mode`, `ipv6_address_mode` and `host_routes`; dry-run test
> `ipv6_modes_and_host_routes_reach_the_subnet`, dialog test
> `subnet_steps_read_ipv6_modes_and_host_routes` (9) RBAC Policies named only networks: QoS
> policies and security groups now too (`mapping::rbac_policies` takes a name list per
> `object_type`; the lists are fetched only when a policy uses that type), linked to their
> Project pages; other types keep their ID. (10) RBAC writes, as Horizon's form on ttcp-dev: the
> dialog *Create RBAC Policy* (`ops::create_rbac_policy_dialog`: *Target Project* from
> `ChoiceSet::RbacTargets`, `*` first; *Action and Object Type* from `model::RBAC_KINDS`, *Shared
> QoS Policy* only with `qos`; the network from `ChoiceSet::AllNetworks` or the policy from
> `AllQosPolicies`, labelled `name (project)` by `mapping::owned_choices`, tested),
> `OsAction::RbacPolicyCreate` (`POST /rbac-policies`, kind checked on the server) and
> `RbacPolicyDelete`, both `admin_only`, with dry-run tests; `AdminRbacPoliciesPage` rebuilt by hand
> with row checkboxes, bulk *Delete RBAC Policies* and each row's *Delete RBAC Policy*. Each row
> also has *Edit Policy* (Horizon's *Update RBAC Policy*: the target project only, preselected from
> `RbacPolicyRow::target_id`; `OsAction::RbacPolicyUpdate`, `PUT /rbac-policies/{id}` with
> `target_tenant`, dry-run and dialog tests). (11) Admin › Floating IPs' controls, as Horizon's page
> and allocate form (read from ttcp-dev): the button `Allocate IP To Project`
> (`ops::admin_allocate_ip_dialog`: pool, project from the new `ChoiceSet::AllProjects`, every
> domain's, also used now by admin Create Network; optional address, checked as an IP on the server;
> description), `OsAction::AdminFloatingIpAllocate` (`POST /floatingips` with `tenant_id`,
> `admin_only`, dry-run test); row checkboxes with the bulk `Release Floating IPs`; each row's
> `Disassociate` (when mapped) and `Release Floating IP` (`ops::admin_floating_ip_items`; no
> Associate, as its ports would be the session project's). Horizon's server-side filters (project,
> IP, network, router, port, status) are not built: the table's search covers them on the loaded
> rows.
> **Browser, 2026-10-31 (partial):** Host Aggregates (19 rows + 3 zones, 21 zone pills, 2 calls)
> and Flavors (213 rows, 5 calls: the list plus each private flavor's access) render; then
> ttcp-dev stopped answering from this machine (curl to Keystone, Nova, Placement, Neutron timed
> out), so Overview and Hypervisors timed out and nothing else was checked. Fix from the log:
> Placement's calls are now bounded on their own (`PLACEMENT_TIMEOUT`, 8 s), so a slow Placement
> fails only its tab instead of the whole Hypervisors page. Also from the console: the shared
> `overview::UsageTotals` read the language in its body (Leptos warned "outside a reactive
> tracking context" on every Project › Overview); the hours are now formatted in closures; no
> new warning since. Switching dashboards made the screen blink: each of Project, Admin and
> Identity had its own `<ParentRoute view=Layout>`, so a switch rebuilt the header and fetched
> `get_session_info` again, and the header's `<Transition>`s (added for the *Admin* switcher)
> rendered nothing meanwhile. `app.rs` now nests the three dashboards (`view=Outlet`) under one
> `<ParentRoute path="" view=Layout>`; checked: three switches keep the same header element, the
> panel bar never empties, and only each page's own call is made. **Cloud refusing this machine
> for minutes** (the user saw it after Host Aggregates): the log's bursts (1,229 failed requests
> in one minute) were all Placement `inventories` / `usages`, from the *Resource Provider* tab
> loading with every Hypervisors page, two GETs per provider, all at once; Hypervisors is the
> first Compute panel. Now the tab loads only when opened (`api::get_admin_resource_providers`,
> `Hypervisors::placement` flag), and every per-item fan-out (Placement, flavor access, volume
> type encryption, DHCP agents, `net_lookups`' subnets) runs through `fetch::Bounded`, at most
> `FAN_OUT` (6) in flight (test `bounded_fan_out_caps_requests_in_flight`); AGENTS.md §6 says so.
> Lists stay one paged call each, unaffected.
> **Resource Provider tab, vGPUs** (ttcp-dev's GPU hosts): Placement calls now carry
> `placement 1.14` (`fetch::placement_get`): without it the list lacks
> `parent_provider_uuid`, and `…/traits` (1.6) failed for all 199 GPUs. Rows are a tree
> (`model::provider_tree`, tested): each compute node, collapsed by default with a ▸/▾ toggle and
> its device count, its devices under it named `pci 0000:26:01.0`; columns *vGPUs* (`VGPU`
> used/total, a host's being its devices' sum), *vGPU Type* (the `CUSTOM_*` traits, e.g.
> `CUSTOM_NVIDIA_H100_47C`; a host lists its devices' types) and *Other Resources* (any other
> class), each only when some provider has it; a *vGPU Usage* donut with "199 vGPU devices
> (PCI) on 6 hosts" (`vgpu_summary`: devices are SR-IOV functions here, not physical cards).
> Traits are asked for each host's first GPU, and for the rest only when it has custom traits;
> on ttcp-dev all do: 610 calls, 6 in flight, 4.1–4.5 s, no warnings.
> Admin › Instances' name now has Horizon's padlock (`LockIcon`, locked or unlocked), as
> Project › Instances; compiled and tested, not yet seen (the session had expired).
> **Timings vs Horizon** (ttcp-dev, 2026-10-31, full page GET each, one at a time): Moonlight
> faster on 15 of 16 Admin panels, 2–5× usually, Host Aggregates 15.0 s → 0.8 s, Networks
> 19.8 s → 1.4 s, while loading every row where Horizon pages 20 (Instances, Flavors, Images,
> Volumes, Networks). Slower only on Trunks (2.1 s vs 1.2 s: all ports are fetched even with no
> trunks).
> **Admin › Routers writes, 2026-10-31:** `AdminRoutersPage` rebuilt by hand (as Networks): row
> checkboxes with the bulk *Delete Routers*, the button *Create Router*
> (`ops::admin_create_router_dialog`, Horizon's admin form: *Project* from `ChoiceSet::AllProjects`,
> name, admin state, external network; *Enable SNAT* only with a gateway and `needs::SNAT`;
> *Router Type* with `needs::DVR`, *High Availability Mode* with `needs::L3_HA`, both *Use Server
> Default* / on / off; *Availability Zone Hints* with `needs::ROUTER_AZ` from the new
> `ChoiceSet::RouterZones`), and each row's *Edit Router* and *Delete Router*
> (`ops::admin_router_items`; Project's edit and delete, now `edit_router_item` /
> `delete_router_item`; no gateway or interface items, whose choices would be the session
> project's). New `OsAction::AdminRouterCreate(AdminRouterSpec)`: `POST /routers` with
> `tenant_id`, `enable_snat`, `availability_zone_hints`, `distributed`, `ha`, each left out when
> unset; `admin_only`. Tests: dry-run `admin_router_sends_owner_gateway_and_horizon_options`,
> dialog `admin_create_router_sends_server_defaults_as_nothing`, menu
> `admin_router_menu_is_horizons_edit_and_delete`, zones `zones_are_named_once_and_only_for_their_resource`
> (`mapping::network_zone_choices` became `zone_choices(raw, resource)` and also drops zones not
> `available`). **Fix from the browser:** ttcp-dev's Neutron answers 400 to
> `availability_zones?resource=router&state=available` while advertising
> `router_availability_zone`, so router zones are listed unfiltered and filtered by the mapper.
> **Browser** (ttcp-dev, `admin2`, dialog opened, not submitted): 64 routers, 3 API calls in
> 1.17 s; the dialog shows Name, Project, Enable Admin State, External Network, High
> Availability Mode and Availability Zone Hints (no *Router Type*: no `dvr` there); picking
> `public-alh1` shows *Enable SNAT*; row menu *Edit Router* / *Delete Router*; choice calls are
> `GET`s. Both clippy runs clean, 439 tests pass.
>
> **Slice 4, Compute writes, 2026-10-31:** every one an `OsAction` in `admin_only`, with dry-run
> tests in `actions.rs` (`flavors_are_created_private_for_projects_and_edited_like_horizon`,
> `aggregates_hosts_and_metadata`, `compute_services_and_migrations`) and menu/dialog tests in
> `ops.rs`. *Flavors*: `FlavorCreate(FlavorSpec)` (`POST /flavors`, then `addTenantAccess` per
> project: with projects the flavor is private), `FlavorDelete` (also bulk), `FlavorSetExtraSpecs`
> (*Update Metadata*), `FlavorSetAccess` (*Modify Access*, private flavors only: Nova can't change
> `is_public`). *Host Aggregates*: `AggregateCreate` (name, zone, then hosts), `AggregateUpdate`
> (an empty zone sends `null`), `AggregateSetHosts` (*Manage Hosts*), `AggregateSetMetadata`
> (removed keys `null`), `AggregateDelete`. *Hypervisors › Compute Host*: `ComputeServiceSet`
> (*Enable* / *Disable Service*, `PUT /os-services/{uuid}`, compute 2.53; no item without a UUID).
> *Instances*: `ServerMigrate` (`host` needs 2.56) and `ServerLiveMigrate` (2.30, `block_migration`
> `"auto"` unless chosen); Admin's instance menu is Project's minus the items that pick from the
> session project's resources. Two dry-run tests expected the placeholder ID raw in the path; the
> SDK encodes it (`%3Cnew%20…%3E`), as the identity test already says. **Create Flavor's layout**
> (user request): the six sizes share lines three by three (new `Field::short`; `.wizard-pane` is
> now a six-column grid, short fields span two, three below 520 px), and every `Multi` checkbox
> list longer than 8 choices gets a filter box (label or ID, every word, case-insensitive:
> `dialog::filter_matches`, tested) with an *n selected* count, so *Flavor Access* and *Modify
> Access* can find a project among ttcp-dev's 138. **Browser** (ttcp-dev, `admin2`, dialog opened,
> not submitted): two lines of three sizes; `poke` narrows 138 projects to 1. Not yet seen: the
> aggregate, compute service and migrate dialogs. Both clippy runs clean, 449 tests pass.
>
> **Slice 4 completed, 2026-10-31** (every write Horizon's Admin dashboard has; each an `OsAction`
> in `admin_only` with a dry-run test in `actions.rs`, and menu/dialog tests in `ops.rs`):
> *Volumes* (`AdminVolumesPage` by hand: row checkboxes, bulk *Delete Volumes*, *Manage Volume*;
> rows: `VolumeResetStatus` with Horizon's statuses less the current one, `VolumeMigrate` to a
> pool of the new `ChoiceSet::CinderPools` (`scheduler-stats/get_pools`) with the current host
> shown, *Update Metadata*, `VolumeUnmanage`, delete; `VolumeManage(ManageVolumeSpec)` is `POST
> /os-volume-manage` with `ref` by name or ID). *Snapshots*: `SnapshotResetStatus`, metadata,
> delete, bulk delete. *Volume Types*: `VolumeTypeCreate` (raw `POST /types`: the SDK's
> `type::create` posts to `types/{id}/action`), `VolumeTypeUpdate`, `VolumeTypeDelete`,
> `VolumeTypeSetExtraSpecs` (*View Extra Specs*, also a new *Extra Specs* column),
> `VolumeTypeSetQos` (*Manage QoS Spec Association*: disassociate, then associate; Cinder's GETs
> with `vol_type_id`, sent through `Exec` as writes; new `ChoiceSet::QosSpecs`),
> `VolumeTypeEncryptionSet` / `…Delete` (`VolumeTypeRow::encryption`, read by
> `mapping::encryption_spec`, prefills *Update Encryption*; update is `PUT
> …/encryption/provider`, as cinderclient). *QoS Specs*: `QosSpecCreate`, `QosSpecSetConsumer`,
> `QosSpecSetSpecs` (`PUT` then `delete_keys`), `QosSpecDelete`; *Create QoS Spec* in the table's
> toolbar. *Group Types*: create, edit, delete, `GroupTypeSetSpecs` (*View Specs*;
> `GroupTypeRow::specs`). *Images*: every project's image gets *Edit Image* (with *Public*),
> *Update Metadata*, *Delete Image*, bulk delete (`ops::admin_image_items`, Project's owner
> items shared). *Defaults*: *Update Defaults* on the Compute and Volume tabs
> (`DefaultQuotaUpdate`, `PUT os-quota-class-sets/default`; item 82's quota form shared as
> `ops::quotas_form`; Neutron's defaults are configuration, no button). *Metadata Definitions*:
> *Import Namespace* (JSON typed or from a file, checked in the dialog and on the server; the
> form's *Public* / *Protected* win), *Edit Namespace* (Glance replaces the fields, so all are
> sent), *Update Associations* (`ChoiceSet::MetadefResourceTypes`; added and removed resource
> types, no prefix editing), *Delete Namespace* (hidden, and not bulk-selectable, when
> protected). *Floating IPs*: admin *Associate* among the owner's instance ports
> (`ChoiceSet::AllAssociablePorts`, first cell the port's project, `Source::Of(set,
> project_id)`; `Owned` now carries `project_id`). **Settings** (`[network]`):
> `supported_provider_types` (default `["*"]`) and `segmentation_id_range` (per type `[min,
> max]`), checked at start-up (`NetworkPrefs::check_provider_types`) and used by the dialog and
> the server (`ProviderNetwork::validate(types)`); in `CONFIGURATION.md` and
> `moonlight.conf.example`. **Load:** Trunks lists every port only when there are trunks.
> **Browser** (ttcp-dev, `admin2`, dialogs opened, never submitted): Volumes (322 rows; menu
> *Update Volume Status* without the current *In-use*, *Migrate Volume* with 3 pools and the
> current host), Volume Types (4 types, 1 QoS spec; *Update Encryption* prefilled luks /
> front-end / aes-xts-plain64 / 256; QoS association preselects), Group Types; only `GET`s, no
> console errors. Then the token expired: Defaults, Images, Metadata Definitions, Floating IPs'
> *Associate* and Trunks are compiled and tested, not seen. Both clippy runs clean, 466 tests
> pass.
>
> **Browser pass and fixes, 2026-11-01** (ttcp-dev, `admin2`, dialogs opened, never submitted).
> Seen working: Overview (54 projects, 2 API calls, 0.85 s); Instances (286 rows, padlock 8
> locked, 4 calls, 3.07 s; *Migrate* and *Live Migrate* open); Hypervisors' Compute Host (6 rows,
> zone pills, *Disable Service* with a reason); Host Aggregates (create, edit and *Manage Hosts*
> prefilled, *Update Metadata* prefilled, bulk delete lists the selection); Flavors (213 rows, 5
> calls, 1.00 s; *Update Metadata* prefilled, *Modify Access* with 138 projects); Networks (112
> rows, 4 calls, 1.01 s; DHCP Agents 0 everywhere, right for OVN, which has no DHCP agents; *Edit
> Network* with the provider summary); System Information (zone pills on Compute 15 and Block
> Storage 6 rows; OVN agents have no zone); RBAC (33 rows, create and edit); Floating IPs (37
> rows, 4 calls, 2.01 s; admin *Associate* lists the owner's ports); Defaults (*Update Defaults*
> on Compute and Volume); Images (31 rows); Metadata Definitions (33 rows, the 30 protected
> without delete; *Import Namespace*, *Update Associations* prefilled); Trunks (0 rows, 0.99 s,
> was 2.1 s); Snapshots (15 rows, *Update Status* without the current one).
> **Fixed in that pass:** a required select preselected its first choice even for a choice the
> user must make: admin *Create Network*, *Create Router* and *Allocate IP To Project* picked the
> first project, and *Create RBAC Policy* picked `*` (every project) and the first network. A
> required select with an `empty_label` is now a placeholder with nothing preselected
> (`dialog::preselects`), and a required `Combo` with nothing picked says *— Select —*, not *—
> None —*. *Migrate* / *Live Migrate Instance* left out the current host and *Migrate Volume*
> the current pool (new `Source::Except(set, value)`; Nova and Cinder refuse them, Horizon leaves
> them out). *Availability Zone Hints* is hidden when its set loads empty (`Field::hide_when_empty`;
> ttcp-dev has no router zones), as Horizon drops it. Protected images get no *Delete Image* and
> can't be bulk-selected (Admin and Project). Wording: *Update Defaults* no longer says a limit
> can't be below what is used; *Import Namespace*'s counter says *Size*, not *Script size*; the
> reset-status help speaks of a resource, not a volume. Tests next to each fix (`dialog.rs`,
> `ops.rs`). Both clippy runs clean, 468 tests pass. The fixed dialogs
> are not yet seen in the browser (the token expired before the recheck: item 89).
>
> ✅ **Done, 2026-11-01.** Paging and filters for the big lists are item 88; the remaining gaps
> (RBAC object names, admin router type / HA, metadata namespace details, Horizon settings not
> built, Overview against Horizon, …) are item 89. Neutron provider attributes stay read-only
> after creation by design: ML2 refuses the update, and Horizon has no such edit either.

**Goal.** Horizon's third dashboard, next to *Project* and *Identity*, for cloud operators: every
project's resources and the cloud-wide objects (flavors, aggregates, volume types, default
quotas, services). Shown only to tokens with the `admin` role, as Horizon does. Panels as read
from the ttcp-dev Horizon on 2026-10-30 (GET only), with each table's columns and actions:

| Group / panel             | Columns                                                                                                    | Actions (table · row)                                                                                                                                                                          |
| ------------------------- | ---------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Admin › Overview          | Project Name, VCPUs, Disk, RAM, VCPU Hours, Disk GB Hours, Memory MB Hours (per project, date range)       | Download CSV Summary                                                                                                                                                                           |
| Compute › Hypervisors     | tabs *Hypervisor* (Hostname, Type, RAM used/total, Local Storage used/total, Instances; summary charts), *Compute Host* (Host, Zone, Status, State, Time since update), *Resource Provider* | · Disable Service (compute host)                                                                                                                       |
| Compute › Host Aggregates | aggregates: Name, Availability Zone, Hosts, Metadata; *Availability Zones*: Name, Hosts, Available         | Create, Delete · Edit, Manage Hosts, Update Metadata                                                                                                                                           |
| Compute › Instances       | Project, Host, Name, Image Name, IP Address, Flavor, Status, Task, Power State, Age                        | server-side filters (project, host, ID, name, image, IPs, flavor, key pair, status, AZ, changes since), Delete · Project's row actions plus Migrate, Live Migrate                              |
| Compute › Flavors         | Flavor Name, VCPUs, RAM, Root/Ephemeral/Swap Disk, RX/TX factor, ID, Public, Metadata                      | Create, Delete · Update Metadata, (Edit with `ENABLE_FLAVOR_EDIT`), flavor access (projects)                                                                                                  |
| Compute › Images          | Angular panel: every project's images, owner column                                                        | Create, Delete · Edit, Update Metadata                                                                                                                                                         |
| Volume › Volumes          | Project, Host, Name, Size, Status, Group, Type, Attached To, Bootable, Encrypted                           | filters, Manage Volume, Delete · Update Volume Status, Unmanage, Migrate, Update Metadata                                                                                                     |
| Volume › Snapshots        | Project, Host, Name, Description, Size, Status, Group Snapshot, Volume Name                                | filters, Delete · Update Status, Update Metadata                                                                                                                                               |
| Volume › Volume Types     | types: Name, Description, Associated QoS Spec, Encryption, Public; *QoS Specs*: Name, Consumer, Specs      | Create · View Extra Specs, Manage QoS Spec Association, Edit, Create/Update/Delete Encryption, Update Metadata; QoS: Create · Manage Specs, Edit Consumer, Delete                                |
| Volume › Groups           | Project, Name, Description, Status, Availability Zone, Volume Type(s), Has Snapshots                       | as Project's (item 5)                                                                                                                                                                          |
| Volume › Group Snapshots  | Project, Name, Description, Status, Group                                                                  | as Project's                                                                                                                                                                                   |
| Volume › Group Types      | Name, Description, Public                                                                                  | Create, Delete · View Specs, Edit                                                                                                                                                              |
| Network › Networks        | Project, Network Name, Subnets Associated, DHCP Agents, Shared, External, Status, Admin State, AZs         | filters, Create (provider network type, physical network, segmentation ID; project), Delete · Edit, Create Subnet                                                                             |
| Network › Routers         | Project, Name, Status, External Network, Availability Zones, Admin State                                   | filters, Create, Delete · Edit                                                                                                                                                                 |
| Network › Floating IPs    | Project, IP Address, Description, Mapped Fixed IP, Pool, Status, DNS Name, DNS Domain                      | filters, Allocate IP To Project, Release · Disassociate                                                                                                                                       |
| Network › Trunks          | Angular panel, every project's trunks                                                                      | as Project's (item 33)                                                                                                                                                                         |
| Network › RBAC Policies   | Project, ID, Object Type, Object, Target Project                                                           | Create, Delete · Edit                                                                                                                                                                          |
| System › Defaults         | tabs Compute / Volume / Network Quotas: Quota Name, Limit                                                  | Update Defaults                                                                                                                                                                                |
| System › Metadata Definitions | Name, Description, Resource Types, Public, Protected                                                   | Import Namespace, Delete · Edit, Update Associations                                                                                                                                          |
| System › System Information | tabs Services (Name, Service, Region, Endpoints), Compute Services, Block Storage Services (Name, Host, Zone, Status, State, Last Updated), Network Agents (Type, Name, Host, Zone, Status, State, Last Updated) | — (agents: none on ttcp-dev)                                                                                                         |

Moonlight's *Detected capabilities* card (`lists::ModulesCard`, roadmap 19) moves here, under
System Information, as *Also pending* already planned.

**Implementation.** L: in slices, read-only first.

1. **Shell and gating.** `components/nav.rs`: an `ADMIN_NAV` with Horizon's four groups, the group
   requiring a new `needs::ADMIN` (`Requires::AnyRole(&["admin"])`, as `needs::IDENTITY_ADMIN`),
   each panel its service (`Service("compute")`…) or microversion (groups: `VolumeMv(3, 27)`, like
   item 5); `nav::dashboard` maps `/admin/…`; the dashboard switcher in `components/layout.rs` (line
   ~86, *Project* / *Identity*) gets *Admin* between them, Horizon's order, when `needs::ADMIN` is
   met. Routes under a `ParentRoute path!("/admin")` in `app.rs`, Horizon's URLs
   (`/admin/hypervisors`, `/admin/aggregates`, `/admin/volume_types`, `/admin/defaults`,
   `/admin/metadata_defs`, `/admin/info`…). Every admin server function also refuses a token without
   the role (`os::token_is_admin`, `ErrorKind::Forbidden`) before any call: the menu is not the
   gate, and a 403 from Nova should not be the first answer either.
2. **System and cloud-wide reads.** System Information (`identity::v3::service`/`endpoint`
   lists, `compute::v2::service::list_20`, `block_storage::v3::service`, `network::v2::agent`),
   Hypervisors (`compute::v2::hypervisor::list_detailed_21`; resource providers via Placement,
   which the SDK may lack: check, else a raw GET through a `JsonRequest`-style read), Host
   Aggregates (`aggregate::list_21`, `availability_zone` detail), Flavors (`flavor` list with
   `is_public=None` to include private ones), Defaults (`quota_class_set` of `default` for
   compute and volume; Neutron `quota` defaults), Metadata Definitions (`image::v2::metadef`),
   Volume Types and QoS (`block_storage::v3::r#type`, `qos_spec`), Group Types (`group_type`),
   RBAC Policies (`network::v2::rbac_policy`), and the Overview usage
   (`simple_tenant_usage` list, as `fetch.rs` already reads one project's around line 1012).
3. **Every project's resources.** Instances (`all_tenants=true`), Volumes and Snapshots
   (`all_tenants`), Groups, Group Snapshots, Images (no owner filter), Networks, Routers,
   Floating IPs, Trunks (no `tenant_id` filter, the opposite of AGENTS.md §6's rule for Project
   panels). Reuse the Project mappers and add a *Project* column with names from Keystone's
   project list (`soft()`, IDs on failure) and *Host* where Horizon shows it
   (`OS-EXT-SRV-ATTR:host`, `os-vol-host-attr:host`). Lists can be large: decide whether
   Horizon's server-side filters and `FILTER_DATA_FIRST` (an empty table until a filter is
   given) are needed now or join the paging bullet of *Also pending*; measure on ttcp-dev
   (instances 20+, routers 64, floating IPs 37 seen).
4. **Writes**, each an `OsAction` with a dry-run test: flavors (create, delete, extra specs,
   access), aggregates (create, update, add/remove host, metadata), compute service
   enable/disable (`service::set_253`), instance migrate / live-migrate, volume reset status,
   migrate, manage/unmanage, snapshot reset status, volume types and encryption, QoS specs and
   association, group types, default quotas (`quota_class_set` update; item 82's quota dialog),
   network create with provider attributes, RBAC policies, metadata namespaces. Destructive
   ones through `Dialog::confirm`.

Settings (`CONFIGURATION.md`, `moonlight.conf.example`): Horizon's `ENABLE_FLAVOR_EDIT`,
`FILTER_DATA_FIRST`, and the Neutron provider network types and segmentation ranges
(`OPENSTACK_NEUTRON_NETWORK` `supported_provider_types`, `segmentation_id_range`).

Open questions: whether the Admin views of Project resources reuse the Project pages with a
"scope" flag or get their own list functions (the detail pages can probably be shared); Keystone
system scope on clouds that enforce it (*Also pending*); whether *Overview*'s CSV download goes
through `downloads.rs`.

## 86. Usage Summary: localised thousands separators

> ✅ **Done** (2026-11-01). `i18n::grouped(lang, v, places)` groups `decimal`'s integer part:
> `1,234,567.89` in English, `1.234.567,89` in Spanish, four-digit numbers ungrouped in Spanish
> (`1234,50`, RAE / CLDR). It keeps the sign and never shows `-0.00` (roadmap 60).
> `pages/overview.rs::UsageTotals` uses it for *Active Instances* and the three hour totals,
> read reactively; the CSV stays plain. Horizon's `_usage_summary.html` uses Django's
> `floatformat:2`, ungrouped by default (read in its source, not seen). Test
> `grouped_splits_thousands_by_language`; 469 tests pass, both clippy runs clean. Not seen in the
> browser: the session had expired. The helper can be reused elsewhere (quota counts, Admin
> Overview), one call each.

**Goal.** Project › Overview › *Usage Summary* shows its totals without digit grouping: a
month of a busy project reads `123456.78` VCPU-hours, hard to scan. Group the integer part by
the UI language: `123,456.78` in English (US), `123.456,78` in Spanish (Spain). Check what
Horizon shows first (not checked when the item was added).

**Implementation.**

- `i18n.rs`: a grouped variant next to `decimal` (e.g. `grouped(lang, v, places)`, or a flag
  on `decimal`), splitting `model::fixed`'s output at the decimal point and inserting `,` (EN)
  or `.` (ES) every three digits from the right, keeping the sign and the "never `-0.00`" rule
  of roadmap 60. Spanish (RAE, and CLDR's `es` with minimum grouping digits 2) leaves
  four-digit numbers ungrouped (`1234,50`, but `12.345,50`): follow that.
- `pages/overview.rs::UsageSection`: the three hour totals (`decimal(l, …, 2)`) and
  *Active Instances* use it; *Active RAM* goes through `mib`, which is already short.
- The CSV (`model::UsageSummary::csv`) stays ungrouped with a decimal point: it is read by
  spreadsheets and scripts, as Horizon's is.
- Tests in `i18n.rs` next to `decimal_uses_a_comma_in_spanish`: `1234567.891` → `1,234,567.89` /
  `1.234.567,89`; `1234.5` → `1,234.50` / `1234,50`; `999.99`; negatives; `-0.0` → `0.00`.

Open question: the same grouping could apply elsewhere (quota donuts' counts, Admin Overview
of item 85, table cells with large counts); this item covers the Usage Summary only, the helper
makes the rest a one-line change each.

## 87. Long bulk actions run in the background, with a progress notification

> ✅ **Done** (2026-11-01). `[ui] background_batch_from` (default 10,
> `0` = every batch in its dialog) in `prefs::Prefs` and `settings::Ui`, documented in
> `CONFIGURATION.md` and `moonlight.conf.example`. `components/jobs.rs`: a `Jobs` context (provided
> in `layout.rs` next to `Dialogs`) and `JobsTray`, fixed bottom-right (`z-index: 30`, below menus
> and modals; `style/main.css`), one card per job with `role="status"` / `aria-live="polite"`.
> `dialog.rs`'s submit hands an `OsAction::Batch` of at least that many items (changed from *more
> than* on 2026-11-01: a limit of 5 kept a 5-item delete in the dialog) to `Jobs::start` and closes
> the dialog. The items are sent in order, `jobs::CHUNK` = 5 per
> `run_action(OsAction::Batch(chunk))`, so every server check runs unchanged and no job state lives
> on the server. The card shows the title, a bar with `n / m` and *keep this tab open* while
> running; failures inline (label: OpenStack's error) as they come; at the end *n done, m failed*.
> It fades after 6 s when everything was sent and nothing failed, and stays until closed otherwise.
> Dry-run says nothing was sent, with the requests in a `<details>` when `[ui] show_requests`. An
> expired session stops the job (*Stopped after n of m*, sign-in link); a chunk refused as a whole
> (writes off, forbidden) stops it with that error. `dialogs.version` is bumped after each chunk
> sent for real, so the table refreshes as rows go. Decided: every `OsAction::Batch` follows the
> rule (start, stop, release, delete); the chunk size is a constant, not a setting; failures are
> listed on the card, not in a dialog. Tests (`jobs.rs`, `i18n.rs`, `settings.rs`): threshold (`0`,
> below, equal, above, not a batch), chunks (order, sizes), counters with failures, fading vs
> dry-run, expired and refused chunks, the card texts. Both clippy runs clean, 475 tests pass.
> **Browser** (ttcp-dev, writes on, run by the user, 2026-11-01): with
> `background_batch_from = 5`, a 6-instance *Delete Instances* closed the dialog and the card
> counted up; the log shows two `run_action` calls 0.4 s apart (5 items, then 1), twice. A
> 5-item delete stayed in the dialog, as the first rule (*more than*) said, so the rule became
> *at least*; that change is tested, not yet seen. Still capped at `MAX_BATCH` = 100 per
> selection (`ops::bulk`); chunking would allow more, a separate decision.
> **Style** (2026-11-01, approved by the user): a plain toast, even 1px border, no coloured edge;
> the state is a dot before the title (blue pulsing while running, green done, red failed) and
> the bar's colour (`.job-card` `--job`).

**Goal.** A bulk action over many rows (*Delete Instances*, *Delete Volumes*, *Release Floating
IPs*…) is one `run_action` call today: the server runs the `OsAction::Batch` items one after the
other (`actions::batch`, at most `MAX_BATCH` = 100) and answers only when all are done, so the
dialog sits on *Working…* for as long as that takes (tens of seconds for 40 instances) and looks
stuck, with no way to see how far it got. With more than a configurable number of items (default
10), the dialog should close on submit and a notification in a corner of the screen should show
the progress instead: the action's title, `12 / 40` with a bar, failures as they happen (the
item's name and OpenStack's error), and at the end a summary (`38 deleted, 2 failed`) that stays
until dismissed when something failed and fades out otherwise. The user can keep working,
navigate, and start another bulk action, which gets its own notification. Horizon has nothing
like it (it submits one request per selected row and reloads the table); this is Moonlight's own
improvement. At or under the threshold the dialog keeps today's behaviour (per-item results in
the dialog).

**Implementation.**

1. **Setting:** `[ui] background_batch_from` (default `10`; `0` keeps every batch in the dialog),
   in `prefs::Prefs` so the browser has it (`server/settings.rs` validates it,
   `CONFIGURATION.md` and `moonlight.conf.example` document it, as roadmap 29's other `[ui]` keys).
2. **Progress needs several requests:** split the batch in the browser and send its items in
   order (or in small chunks, e.g. 5 at a time, still `run_action(OsAction::Batch(chunk))`), so
   each answer advances the counter. Every chunk goes through `run_action` as now: the checks
   (`check_ids`, `admin_only`, plugins, identity safety, `[writes] mode`) stay on the server,
   unchanged, and dry-run still answers each chunk with its previews. No server-side job state,
   so nothing to clean up if the user closes the tab (the rest just isn't sent: say so in the
   notification's help). Alternative to weigh: one streamed server function; more moving parts
   (axum streaming, Leptos server-fn streaming), so prefer the chunked calls unless measurements
   show the per-request overhead (session rescope, about 0.1 s) matters.
3. **Where it lives:** `components/dialog.rs`'s `submit` (around `run_action(action)`): when the
   built action is an `OsAction::Batch` longer than the threshold, close the dialog and hand the
   items to a new `components/jobs.rs`, a `Jobs` context (next to `Dialogs`, provided in
   `components/layout.rs`) holding a list of jobs (`title`, `total`, `done`, `failed:
   Vec<ItemResult>`, `mode`), and a `JobsTray` component rendered once in the layout, fixed at
   the bottom-right corner (`position: fixed`, above the footer, below modals), one card per job
   with `role="status"` / `aria-live="polite"`, a close button once finished. Bump
   `dialogs.version` after each chunk in `[writes] mode = on`, so the table behind refreshes as
   rows go away. Dry-run: the card says nothing was sent and links to the previews (`Outcome`'s
   view, in a dialog on click).
4. **Errors:** an `Unauthenticated` answer (token expired mid-way) stops the job and says how many
   were done and that the session must be renewed; other errors are per item, as now
   (`ItemResult`), and the job continues.
5. **i18n** (both languages): the card's texts (`Deleting… n / m`, `n done, m failed`, `Nothing
   was sent (dry-run)`, `Stopped: session expired`).
6. **Tests:** pure helpers next to the code: the split into chunks (order kept, sizes), the
   threshold decision (`0`, equal, above), the job counters from a sequence of `ActionOutcome`s
   (with failures and an expired session). Browser check in dry-run with a selection above the
   threshold (Admin › Volumes has 322 rows on ttcp-dev): the dialog closes, the card counts up,
   and `read_network_requests` shows one `run_action` per chunk.

Open questions: the threshold's default (10 as proposed) and whether the chunk size should be a
setting too; whether a finished card with failures should also list the failed items in the
dialog's result view (clickable) or only inline; whether non-delete bulk actions (*Start
Instances*, *Release Floating IPs*) follow the same rule (proposed: every `OsAction::Batch`).

## 88. Server-side paging and filters for the big Admin lists

> 🔗 **Issue:** [#10]
>
> ✅ **Done (2026-11-03/05), slices A–E: every Admin list of the table below is paged,
> filters and sorts on the server and counts on request, and so are the big Project panels with
> `[ui] paged_lists = "all"`; checked on ttcp-dev.** The design
> (settled with the user): *Prev* uses a marker stack kept by the page (the URL holds only the
> current marker); filtering runs on the server across every record (no "this page only" search
> box on paged lists); `page_size` defaults to 100; paging covers the Admin lists by default and
> the Project panels only when `[ui] paged_lists = "all"`.
>
> Built as the slice A plan below: `[ui] page_size` / `paged_lists` (`server/settings.rs`,
> `prefs::Prefs::page_size_for`, `CONFIGURATION.md`, `moonlight.conf.example`); `ListQuery`,
> `Page<T>`, `PagedList` with its filter and sort tables (`model.rs`); `server/paging.rs`
> (`params`, `cut`); `fetch::page_raw` (one raw GET with `limit = page_size + 1`) and
> `admin_instances_page`; `api::list_admin_instances(q)`; `components/pager.rs` (`FilterBar`,
> `Pager`, `use_list_query`, `with_marker`, `with_filter`); `AdminInstancesPage`. With
> `page_size = 0` the page renders today's whole table.
>
> Changes from the plan, found on ttcp-dev:
>
> - **Boot-from-volume images are filled in after the table shows** (the user's choice). ttcp-dev
>   boots every server from a volume, so reading each row's volumes before answering cost
>   100–113 calls and 9.5–11 s a page, against ≈3 s for the old whole list. Cinder can't batch
>   them: a probe of `volumes/detail?all_tenants&id=a&id=b…` (GET) answered 1 of 50, it keeps
>   only the last `id=`. Now the page leaves the *Image* of those rows empty with their attached
>   volumes in `Instance::boot_volume_ids`, and the page calls `api::get_admin_boot_images` in
>   chunks of `model::BOOT_IMAGE_CHUNK` (50) volume IDs, a server never split across two
>   (`pages/admin.rs::boot_chunks`). The server reads them `FAN_OUT` at a time
>   (`fetch::admin_boot_images`, at most `MAX_BOOT_VOLUMES` = 100 a call), and the pure
>   `mapping::boot_images` picks each server's boot volume from the volumes' attachments. The
>   cell shows "…" until then, and "-" if nothing was found or the call failed.
> - **`with_marker` used `ParamsMap::replace`**: `insert` adds a value, so each *Next* piled up
>   `?marker=a&marker=b`.
> - **The filter bar navigates through the router**: a plain GET `<form>` reloaded the whole app
>   on every filter. It stays a GET form for clicks before hydration.
>
> Tests: `paging::tests` (exact pairs, `limit`, refused field / sort / marker, bounded values,
> Nova sort, `cut`), `pager::tests` (query round trip, one marker after *Next*, a new filter
> keeps the sort and drops the marker, row numbers), `mapping` (`instances_page` keeps Nova's
> order and marks only boot-from-volume rows, `boot_images`), the chunking test in
> `pages/admin.rs`, settings (defaults, 1000 cap, `0` = whole list).
>
> Measured on ttcp-dev (GET only, `page_size = 50`, more than 200 instances): each page 2 API
> calls, 1.4–2.5 s on the server (one 3.9 s), against ≈3 s for the whole list. Page 2's 48
> boot-volume images were all in by 7.3 s after the click (≈4 s for ~50 volumes). *Next* /
> *Previous* walk pages 1–3 and back with the right rows and one `marker`. Filters: host 9
> rows, all on that host; name `WIN-2025` 6 rows (Nova matches substrings); project
> *public-images* 12 rows, all its own; *Clear* back to page 1; `?f=tenant_id` refused with
> *Invalid request* before reaching OpenStack. `read_network_requests` shows only GETs.
>
> **Slice B (2026-11-04).** `PagedList::{Volumes, Snapshots, FloatingIps}` with their filter
> and sort tables (`model.rs`); `fetch::page_raw` takes the four lists and returns `RawPage`
> (rows, next marker, Cinder's `count`); `admin_volumes_page`, `admin_snapshots_page`,
> `admin_floating_ips_page`; `api::list_admin_{volumes,snapshots,floating_ips}(q)` (whole lists
> through `api::whole` when `page_size = 0`); the pages share `pages/admin.rs::paged_table`.
> Probes on ttcp-dev (GET): Nova keeps only the last `uuid=` and ignores `id=`, so like Cinder it
> can't name several servers in one request; Cinder 3.45 takes `with_count` with `name~`.
> Hence:
>
> - **Names fill in after the table** (the rule the user chose for boot images in slice A):
>   Volumes' *Attached To* servers, Floating IPs' instances, Snapshots' volume and host. One
>   generic `api::get_admin_names(NameKind, ids)` (`fetch::admin_names`, `by_ids` reads
>   `servers/{id}` or `volumes/{id}` `FAN_OUT` at a time, at most `MAX_LATE_NAMES` = 100), sent
>   in chunks of 50 by `components/pager.rs::load_late` into a `LateNames` map; `late_text`
>   shows "…", then the name, or the ID when not found. `LateNames.active` is false on a whole
>   list, whose rows come named.
> - **Floating IPs' pools** come in one `networks?id=…&id=…&fields=id&fields=name` GET (Neutron
>   repeats a filter as *any of*), with the page.
> - **Cinder versions** (`paging::cinder`, pure): from 3.45 `with_count=true` (the pager reads
>   "of N"), from 3.34 `name~` (substring), below or unknown the exact `name` and no header.
> - **Exact filters say so**: `FilterKind::Exact` (addresses, hosts, IDs, zones) shows *Exact
>   value* instead of *Search* (Neutron found nothing for a partial address).
>
> Tests: `paging` (Cinder colon sort and `name~`, Neutron without `all_tenants`, `cinder` per
> microversion, which filters are exact), `mapping` (page variants keep the API's order,
> `pool_ids`, `late_name`), `pager::late_names_go_in_chunks_each_id_once`.
>
> Measured on ttcp-dev (`page_size = 50`; "before" with the same build on a second port and
> `page_size = 0`):
>
> | List         | Rows | Before (whole list)  | After (one page)                                      |
> | ------------ | ---- | -------------------- | ----------------------------------------------------- |
> | Volumes      | 346  | 3 calls, 1.58–2.90 s | 2 calls, 0.46–1.00 s, "of 346"; 39 servers in ≈3 s    |
> | Snapshots    | 14   | 3 calls, 1.85–3.28 s | 2 calls, 0.86 s; 28 volume names and hosts by 1.9 s   |
> | Floating IPs | 35   | 4 calls, 0.84–1.16 s | 3 calls, 0.77–1.59 s; 9 instances by 4.3 s            |
>
> Floating IPs cost the same on a cloud this small; the page no longer grows with it. Filters
> checked: Volumes name `image-` (16 of 16, all matching), host (94, one host), status
> *available* (125), bootable *false* (39); Floating IPs address (1; a partial address finds 0),
> status *DOWN* (23), project (5, all its own). *Next* / *Previous* on Volumes keep one marker
> and "of 346".
>
> **Slice C (2026-11-04).** Tried by the user on ttcp-dev, who found it working (2026-11-05); no
> timings were recorded. `PagedList::{Networks, Routers, Trunks, Images, VolumeGroups,
> GroupSnapshots}` with their filter and sort tables (`model.rs`; Neutron's `NEUTRON_SORTS` are
> name and status); `page_raw` arms (groups send `GROUP_PROJECT_MICROVERSION`, volume 3.58);
> `fetch::admin_{networks,routers,trunks,images,volume_groups,group_snapshots}_page`;
> `api::list_admin_*` of those six take `ListQuery` (whole lists through `whole` with
> `page_size = 0`). Lookups bounded by the page, all on the server before answering (no late
> names: each is one or two GETs):
>
> - Networks: subnets by `subnets?network_id=…&fields=id&fields=name&fields=cidr`; the DHCP
>   counts stay one call per agent.
> - Routers: gateways by `networks?id=…` (`mapping::gateway_ids`).
> - Trunks: parent and sub-ports by `ports?id=…&fields=id&fields=name`
>   (`mapping::trunk_port_ids`), instead of every port of the cloud.
> - Group Snapshots: their groups by `groups/{id}`, `FAN_OUT` at a time.
> - Groups: types stay whole, and so does `group_snapshots/detail?all_tenants` for *Has
>   Snapshots* (Cinder filters group snapshots by one `group_id` only); it grows with the
>   cloud's group snapshots, usually a handful.
>
> `fetch::neutron_where(path, filter, values, fields)` generalises the old `networks_by_ids`.
> The pure mappers got order-keeping `_page` variants (`networks_page`, `routers_page`,
> `trunks_page`, `images_page`, `volume_groups_page`, `group_snapshots_page`) and the ID helpers
> above, sharing `mapping::distinct`. `pages/admin.rs`: Images, Networks and Routers use
> `paged_table`; `admin_list!` (Trunks, Groups, Group Snapshots) is now the paged macro, and
> `paged_table` takes `Option<Select>` so these lists still have no checkbox column. Filters:
> Neutron's and Glance's names are exact (`FilterKind::Exact`), Glance's project is `owner`;
> Cinder groups take `name`, `project_id`, `status` exact (an admin may filter on any column,
> per `reject_invalid_filters`; unverified). New label `K::FilterPortId`.
>
> Tests: `paging::slice_c_lists_map_their_filters_and_sorts`,
> `paging::every_list_names_by_its_first_filter_and_only_cinder_volumes_take_like` (also: no key
> twice, every list sorts) and
> `mapping::slice_c_pages_keep_the_apis_order_and_name_from_the_pages_lookups`. 529 tests pass.
>
> Slice C's filters were tried with slice D's checks below; no before/after timings (not
> needed, the user's call, 2026-11-05).
>
> **Found while doing slice C, fixed 2026-11-05:** at `page_size = 1000` the request asked for
> `limit=1001`, but Nova and Glance cap a page at 1000 by default (`max_limit`,
> `api_limit_max`), so `cut` saw no extra row and never offered *Next*. `[ui] page_size` is now
> 1–999 (`settings::MAX_PAGE_SIZE`, its test, `CONFIGURATION.md`, `moonlight.conf.example`).
>
> **Slice D (2026-11-05).**
>
> - **Server-side sort.** `Col::server_sort(key)` and the `DataTable` prop `server_sort:
>   ServerSort { keys, current, on_sort }` (`components/table.rs`). On a paged table, a header
>   whose key is in `PagedList::sorts` sorts every record: the click cycles ascending,
>   descending, the API's order (`table::next_server_sort`) and navigates with
>   `pager::with_sort`, which keeps the filter and drops the marker (the stack resets when the
>   marker goes). Its title says *Sorts every row, on the server*; the other sortable headers
>   keep the client sort with *Sorts only the rows on this page*. The keys come from the headers
>   (`pages/admin.rs::server_key`: *Name*, *Status*, *Size*, *Age* / *Created*, *Host*,
>   *IP Address*), applied by `with_server_keys` in `paged_table` and on Admin › Instances; a
>   key the list's API lacks is ignored, so e.g. Volumes' *Host* still sorts the page.
> - ***Count all.*** `api::count_admin(list, q)` → `fetch::admin_count`: the filter of `q` (not
>   its sort or marker, `paging::count_params`), the lightest list (`list_request` with
>   `brief`: Nova `servers`, Cinder without `/detail`, Neutron `fields=id`), 1000 rows a request
>   until an empty answer (`paging::count_step`: a short page may be the API's own cap), at most
>   `MAX_COUNT_PAGES` = 100 requests or 15 s (`COUNT_BUDGET`, under the 20 s
>   `REQUEST_TIMEOUT`); stopping early returns `RowCount { more: true }`, read "of 100000+".
>   Cinder at 3.45 answers `count` in one request. `page_raw` and the count share
>   `fetch::list_request`.
> - The pager (`components/pager.rs`): `PagerState { markers, total }` replaces `MarkerStack`,
>   created by the page so a count survives *Next* and a new sort. *Count all* shows only
>   without the API's count; while running it reads *Counting…*, on failure *Could not count*
>   and the link again. `shown_total` keeps a count only for its filter and `data_version` (a
>   write makes it stale). Style `.pager .count-all` (`--accent`).
>
> Tests: `table::a_server_sorted_header_cycles_like_a_client_one`,
> `pager::a_new_sort_keeps_the_filter_and_drops_the_marker`,
> `pager::a_count_shows_only_for_its_own_filter_and_data`,
> `paging::a_count_ignores_the_sort_and_asks_neutron_for_ids_only`,
> `paging::a_count_goes_on_until_an_empty_answer`,
> `admin::every_server_sort_key_comes_from_a_header`, and `page_range`'s "of N+". 535 pass.
>
> **Checked on ttcp-dev (2026-11-05, GET only, `page_size = 50`, session in an admin project).**
> Every list × server sort × direction was fetched; two sorts the cloud refuses were dropped
> from `PagedList::sorts` (their headers now sort the page, and a hand-made `?sort=` gets
> *Invalid request* before OpenStack):
>
> - Instances by *Host*: Nova answers `sort_key=host` with HTTP 500 (multi-cell listings merge
>   the cells' rows in Python, and a server with no host can't be compared).
> - Snapshots by *Size*: HTTP 400, a snapshot's column is `volume_size`.
>
> The rest order as expected: Instances by name across pages 1–3 and back (Nova uses the
> database collation: case-insensitive, `_` after letters), by status (Nova's `vm_state`, so
> *Error* and *Rebuild* rows, both `vm_state = error`, interleave), by age; Volumes by size and
> name (unnamed volumes, shown by ID, first); Images by size and name; Networks, Routers by
> name and status; Floating IPs by address (Neutron compares text: `192…`, `217…`, `80…`).
> *Count all*: Instances 189 in 0.82 s, kept across *Next* and a new sort, dropped by a new
> filter (name `lgr`: 35 in 0.41 s); Networks 115 in 0.72 s; Routers 64 in 0.41 s; Images 31,
> Floating IPs 36, Trunks 1 in 0.2–0.3 s; Groups and Group Snapshots 0 (the brief Cinder
> lists take `limit` / `marker` without error). `/api/` requests: GETs only.
>
> Filters tried one by one, all applied (a made-up value finds 0, not the whole list):
> Instances `ip` (matches a part: `10.10.0.` 18 rows), `zone` (alh1, mad1), `ip6` (0 rows; the
> cloud has no IPv6 to confirm a match); Volumes `zone`; Snapshots `volume`; Floating IPs
> `fixed` (whole address only), `network` (28, 3 and 3 rows per pool, each its own), `router`
> (4, among them the one whose detail names that router); Trunks `port`. Groups' `name` and
> `status` answer without error but the cloud has no groups to match.
>
> **Instances' `flavor` and `image` take IDs only**: a typed *A3.4-16* or *cirros-0.6.3* found
> 0. `FilterKind::Project` became `FilterKind::Pick(ChoiceSet)`: Project
> (`AllProjects`), Flavor (`Flavors`) and Image (`Images`) are picked by name in a `Combo` that
> sends the ID (`pager::FilterBar` loads the field's set on choosing it; a late answer for a
> field left since is dropped). Checked: flavor *A3.4-16* → `?f=flavor&q=12003`, 9 rows, all
> that flavor; image *cirros-0.6.3* → 1 row; the projects' list (140) still loads.
>
> Tests: `paging::sorts_the_test_cloud_refused_are_not_offered`,
> `paging::nova_flavor_and_image_filters_are_picked_by_name_and_send_the_id`, the settings test
> at 999. 538 pass.
>
> **Slice E (2026-11-05), built and checked on ttcp-dev.** With `[ui] paged_lists = "all"`,
> Project › Instances, Volumes, Snapshots, Floating IPs, Routers and Images are paged as the
> Admin lists;
> with `"admin"` (the default) their server functions answer the whole list (`api::whole`) and
> the page shows its search box as before.
>
> - **Server.** `paging::params_in(list, project, q, limit)`: in a project, Nova and Cinder drop
>   `all_tenants` (a project token lists its own), Neutron adds `project_id` (AGENTS.md §6),
>   Glance keeps the whole Project › Images query. `model::ListScope` and
>   `PagedList::filters_in(scope)` drop *Project* and *Host* on a Project panel, and `params_in`
>   refuses them there. `fetch::{instances,volumes,snapshots,floating_ips,routers,images}_page`;
>   `page_raw` and `count_rows` (was `admin_count`) take the project.
> - **API.** `list_instances`, `list_volumes`, `list_snapshots`, `list_floating_ips`,
>   `list_routers`, `list_images` take the URL's `ListQuery` and answer a `Page`.
>   `count_admin` became `count_rows(list, scope, q)` (Admin scope still requires the role);
>   `get_admin_names` / `get_admin_boot_images` became `get_late_names` / `get_boot_images`
>   without `require_admin`: they are `servers/{id}` / `volumes/{id}` GETs with the session's
>   token, so OpenStack decides what each user may read.
> - **Client.** `pager::paged_table`, `with_server_keys` and `server_key` moved from
>   `pages/admin.rs` to `components/pager.rs` with a `scope` (and a row drawer, for Images);
>   `FilterBar` and `Pager` take `scope` (default Admin). `lists::paged_page!` is `list_page!`
>   on a `Page`, with an optional `late = (NameKind, ids)` for cells named after the table
>   shows (Volumes' attachments, Snapshots' volumes, Floating IPs' instances: `late_attachments`
>   moved to `lists.rs`). Project › Instances uses `paged_table` and the boot-image chunks
>   (`late_image_cell`, `boot_chunks`, `load_boot_images`, moved to `pages/instances.rs`).
> - **Left whole on purpose.** Project › Networks merges three Neutron lists (own, shared,
>   external) that one marker can't page. There is no Project › Ports panel, so the Ports the
>   plan named have nothing to page.
>
> Tests: `paging::a_project_panel_lists_its_own_project_only` (scoping, refused filters, a
> count keeps the scope); `server_key`'s and `boot_chunks`' tests moved with them. Both clippy
> runs clean, 539 pass.
>
> **Checked on ttcp-dev (2026-11-05, GET only, `paged_lists = "all"`, `page_size = 3`,
> session in *pokemon*, a project without the admin role).** Instances: 7 servers in three
> pages, *Next* / *Previous* with the right row numbers, *Count all* 7 (the Admin count is 189),
> server sort by name both ways across pages, status `SHUTOFF` → the 2 stopped servers, the 7
> boot-from-volume images filled in after the table; a hand-made `?f=project&q=…` gets *Invalid
> request* before OpenStack. Volumes: Cinder's own total (8), attached instances named. Floating
> IPs: 1 (the cloud has 36); Routers: the project's 3 (64 cloud-wide), external networks named;
> Images: 23 visible in Glance's order, row drawers, *Next*. Filter fields have no *Project* /
> *Host*. `/api/` requests: GETs only (`count_rows?…&scope=Project`), no console errors. Admin ›
> Instances in *admin2* unchanged after the refactor (9 filter fields, count 189, boot images).
>
> Not checked: Snapshots' late volume names (*pokemon* has no snapshots), a floating IP's late
> instance name (its only one is unassociated), and timings before and after on a large project
> (the test cloud's biggest project has 7 servers).
>
> Item 89's `FILTER_DATA_FIRST` waits for this item.

**Goal.** Every Admin list loads every project's resources in one go (`Pagination::All` in
`fetch::list_where!` / `list_all!`), so its load time grows with the cloud: on ttcp-dev Admin ›
Instances takes about 3 s with 186 instances and 322 volumes, adding 100 instances in *pokemon*
added about 10%, and a cloud with thousands of servers would keep the page waiting well past
10 s (plus the browser drawing every row). A page should cost the same whatever the cloud's
size: show one page of rows (100 by default, configurable), with *Prev* / *Next*, and filter and
sort on the server. Horizon does this with `API_RESULT_PAGE_SIZE` (20) and marker links, shows
no total, and filters Admin › Instances server-side (project, host, name, IP, image, flavor,
status, availability zone…).

Lists in scope, in this order (the largest first; measure each on ttcp-dev before and after):

| List                    | Fetcher (`server/fetch.rs`) | List API, paging                                    | Lookups that also grow             |
| ----------------------- | --------------------------- | --------------------------------------------------- | ---------------------------------- |
| Compute › Instances     | `admin_instances`           | Nova `servers/detail?all_tenants`, `limit`/`marker` | every project's volumes, images    |
| Volume › Volumes        | `admin_volumes`             | Cinder `volumes/detail?all_tenants`, same           | `all_servers_brief`                |
| Volume › Snapshots      | `admin_snapshots`           | Cinder `snapshots/detail?all_tenants`, same         | every project's volumes            |
| Network › Floating IPs  | `admin_floating_ips`        | Neutron `floatingips`, `limit`/`marker`             | `all_networks`, `all_servers_brief` |
| Network › Networks      | `admin_networks`            | Neutron `networks`, same                            | every subnet                       |
| Network › Routers       | `admin_routers`             | Neutron `routers`, same                             | `all_networks`                     |
| Network › Trunks        | `admin_trunks`              | Neutron `trunks`, same                              | every port (only with trunks)      |
| Compute › Images        | `admin_images`              | Glance `images`, `limit`/`marker`                   | —                                  |
| Volume › Groups, Group Snapshots | `admin_volume_groups`, `admin_group_snapshots` | Cinder, `limit`/`marker` | groups, types               |

Small cloud-wide lists (flavors, aggregates, hypervisors, volume types, RBAC, metadefs) stay
whole. Project panels stay whole too (item 27 measured them small); they can reuse this later.

**Implementation.** The old *Also pending* paging bullet (`page_size`, `result_limit`) is folded
into this item.

**Decisions.**

- *Prev*: the page keeps the markers it walked in a signal (a stack). *Next* pushes the current
  marker and navigates to `?marker=<last row id>`; *Prev* pops one. On a pasted or reloaded URL
  with a marker and an empty stack, *Prev* reads *First page* and drops the marker. No reversed
  queries on the server.
- Search: a paged list has no client-side search box. Its toolbar shows a **filter bar**, one
  field and one value at a time like Horizon's admin filter, sent to the API, so it searches
  every record. The default field is *Name*, so typing in the box and pressing Enter searches
  names cloud-wide.
- `page_size = 0`, or a list not covered by `paged_lists`, keeps today's whole list with the
  client search box, unchanged.
- One filter at a time (Horizon's model). Several at once can come later; `ListQuery` keeps one.

**Slice A: infrastructure and Admin › Instances.**

1. **Settings** (`server/settings.rs`, `prefs::Prefs`, `CONFIGURATION.md`,
   `moonlight.conf.example`, tests next to the `background_batch_from` ones):
   `[ui] page_size` (`u32`, default `100`, `0` = whole lists, otherwise 1–1000: reject anything
   above at start-up like the other range checks) and `[ui] paged_lists` (`"admin"` default,
   `"all"`: the Project panels of slice E as well). Both go to `Prefs` (the client needs them to
   pick the toolbar) and are read on the server through `cfg()`.
2. **Model** (`model.rs`, DTOs as AGENTS.md §4):
   - `ListQuery { marker: Option<String>, field: Option<String>, value: Option<String>,
     sort: Option<String>, desc: bool }`, flat on purpose: server functions use `GetUrl`, which
     encodes arguments with `serde_qs`, where tuples encode badly. `field` and `sort` are
     neutral keys (`name`, `project`, `host`…), not API parameter names.
   - `Page<T> { rows: Vec<T>, next: Option<String>, count: Option<u64>, paged: bool }`: `next`
     is the marker of the following page (`None`: last page); `count` the total when the API
     gives it cheaply (Cinder, slice B); `paged = false` when `page_size = 0` (every row,
     `next = None`), so the page shows the old toolbar.
   - `PagedList` enum (`Instances`, `Volumes`, `Snapshots`, `FloatingIps`, `Networks`,
     `Routers`, `Trunks`, `Images`, `VolumeGroups`, `GroupSnapshots`) with
     `fn filters(self) -> &'static [(&'static str, FilterKind)]` (neutral key, how the value is
     picked) and `fn sorts(self) -> &'static [&'static str]` (neutral keys sortable on the
     server). `FilterKind::{Text, Project, Choice(&'static [&'static str])}`: `Project` is a
     picker over `ChoiceSet::AllProjects` sending the ID; `Choice` a `<select>` of API values
     (statuses, visibility). No `K` in `model.rs`: the client maps neutral keys to labels
     (`i18n::filter_label(key) -> K`, both languages).
3. **Query building, pure** (new `server/paging.rs`, `//!` header: "pure query building for
   paged lists; GETs only, through `fetch`"):
   `pub fn params(list: PagedList, q: &ListQuery, limit: u32) -> Result<Vec<(&'static str,
   String)>, AppError>`. It validates first: `marker` with `model::check_id` (it comes from
   the URL); `field` and `sort` must be in `list.filters()` / `list.sorts()`, else
   `AppError::invalid`; `value` at most 255 characters, no control characters, trimmed, empty
   means no filter. Then maps per service:

   | List          | Fixed parameters                       | Neutral filter → API parameter                                                                                                    | Sort                       |
   | ------------- | -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | -------------------------- |
   | Instances     | `all_tenants=true`                     | `name`→`name` (Nova regex: substring), `project`→`project_id`, `host`→`host`, `status`→`status`, `ip`→`ip`, `ip6`→`ip6`, `flavor`→`flavor` (ID), `image`→`image` (ID), `zone`→`availability_zone` | `sort_key`/`sort_dir`; `name`→`display_name`, `created`→`created_at`, `host`→`host`, `status`→`vm_state` |
   | Volumes       | `all_tenants=true`                     | `name`→`name~` (volume 3.34: *like*), `project`→`project_id`, `host`→`host`, `status`→`status`, `bootable`→`bootable`, `zone`→`availability_zone` | `sort=<key>:<dir>`; `name`, `size`, `created_at`, `status` |
   | Snapshots     | `all_tenants=true`                     | `name`→`name~`, `project`→`project_id`, `status`→`status`, `volume`→`volume_id`                                                 | as Volumes                 |
   | Floating IPs  | —                                      | `project`→`project_id`, `address`→`floating_ip_address`, `fixed`→`fixed_ip_address`, `network`→`floating_network_id`, `router`→`router_id`, `status`→`status` | `sort_key`/`sort_dir`      |
   | Networks      | —                                      | `name`→`name` (exact), `project`→`project_id`, `status`→`status`, `shared`→`shared`, `external`→`router:external`               | `sort_key`/`sort_dir`      |
   | Routers       | —                                      | `name`, `project`→`project_id`, `status`                                                                                          | `sort_key`/`sort_dir`      |
   | Trunks        | —                                      | `name`, `project`→`project_id`, `status`, `port`→`port_id`                                                                        | `sort_key`/`sort_dir`      |
   | Images        | —                                      | `name` (exact), `project`→`owner`, `visibility`, `status`, `disk_format`                                                          | `sort_key`/`sort_dir`      |
   | Groups, Group Snapshots | `all_tenants=true`           | `project`→`project_id`, `status`                                                                                                  | `sort=<key>:<dir>`         |

   Plus `limit=<page_size + 1>` (the extra row only says whether there is a *Next*) and
   `marker`. Neutron and Glance take `name` as an exact match, Nova as a regex, Cinder `name~`
   as *like*: the filter bar's placeholder says *exact name* where it is exact. Verify on
   ttcp-dev (GET only) that Cinder's admin `host` filter and Nova's `ip6` work before keeping
   them; drop a filter that the cloud ignores rather than showing a filter that does nothing.
   `pub fn cut(rows: Vec<Value>, page_size: usize) -> (Vec<Value>, Option<String>)`: keeps
   `page_size` rows and returns the last kept row's `id` as `next` when there were more.
4. **One page per request** (`fetch.rs`, next to `list_where!`): `async fn page_raw(os,
   list, q, page_size) -> R<(Vec<Value>, Option<String>, Option<u64>)>` sends one raw
   `actions::JsonRequest::get(service, path)` with `.param(…)` for each pair from
   `paging::params` (the SDK builders lack Cinder's filters, so all lists use the raw request),
   through `get_json` (counted, GET-only assertion), then takes the list from the body
   (`servers`, `volumes`, `snapshots`, `floatingips`, `networks`, `routers`, `trunks`, `images`,
   `groups`, `group_snapshots`) and runs `paging::cut`. Paths and headers: Nova
   `servers/detail` with `compute_mv()`; Cinder `volumes/detail`, `snapshots/detail` with
   `volume 3.45` when `needs` says the cloud has it (`count`, and `name~` from 3.34), else
   `volume 3.34` or no header (and then no `name` filter: `params` gets the capabilities, or
   the list hides that field); groups at `GROUP_PROJECT_MICROVERSION`. With `page_size = 0`
   it calls today's `list_where!` (`Pagination::All`) instead, so nothing changes.
5. **Lookups bounded by the page** (`fetch.rs`): the mappers already take slices of raw
   lookups, so give them only what the page references.
   - `images_by_ids(os, ids)`: Glance `images?id=in:<a>,<b>…`, one GET per 50 IDs.
   - `boot_volumes(os, servers)`: `volumes/{id}` through `Bounded` (`FAN_OUT`) only for rows
     booted from a volume (`image` empty); add a pure `mapping::boot_volume_ids(servers)` next
     to `boot_volume` and test it.
   - `project_names` stays (one Keystone list).
   - Admin › Instances then costs: servers page + project names + images by ID + the boot
     volumes on the page; no `volumes/detail?all_tenants`, no full image list.
6. **API** (`api.rs`): `list_admin_instances(q: ListQuery) -> Result<Timed<Page<Owned<Instance>>>,
   AppError>`, still `require_admin` first. `fetch::admin_instances(os, q, page_size)`.
7. **UI** (new `components/pager.rs`, `//!` header):
   - `use_list_query() -> Memo<ListQuery>` from `use_query_map` (`marker`, `f`, `q`, `sort`,
     `dir`), and `fn href(q: &ListQuery) -> String` (pure, tested) to build links.
   - `<FilterBar list=PagedList::… />`: field `<select>` (labels from `filter_label`), then a
     text box, a project `Combo` (`ChoiceSet::AllProjects`) or a `<select>` by `FilterKind`;
     Enter or *Filter* navigates to the query with the new field/value and no marker; *Clear*
     drops them. Changing filter or sort clears the marker stack.
   - `<Pager next=… shown=n count=… stack=…/>` under the toolbar count: *Prev* / *First page*,
     *Next* (disabled when `next` is `None`), and the range "Items 101–200" (from the stack depth
     × page size; "100 items on this page" when the depth is unknown after a deep link;
     "of 322" with `count`). i18n keys in both languages.
   - `DataTable` (`components/table.rs`): a new optional prop `server: Option<ServerTable>`;
     with it the client filter box is not drawn (the filter bar goes in `extra`), the count reads
     from the pager, and columns whose header maps to a sortable neutral key (slice D) sort on
     the server. Everything else in `DataTable` stays as it is. Keep the logic in pure helpers
     with tests, like `visible_rows`.
   - `AdminInstancesPage`: stop using `admin_list!` for it; `Resource::new(move ||
     (query.get(), data_version()), |(q, _)| list_admin_instances(q))`; when `page.paged` is
     false render exactly today's table. Bulk actions and *Select all* act on the page's rows
     (`retain_present` already drops picks from other pages).
8. **Tests** (pure, next to the code): `params` per list with exact pairs (incl. `limit=101`,
   unknown field / sort refused, a marker with `/` refused, empty value dropped, Cinder
   `sort=name:desc`, Neutron `sort_key`+`sort_dir`); `cut` with 0, `page_size` and
   `page_size + 1` rows; `pager::href` round trip; `boot_volume_ids`; settings defaults and the
   1000 cap; `page_size = 0` takes the old path.
9. **Measure** on ttcp-dev (GET only, dry-run config) with `page_size = 50` (186 instances: four
   pages): time and calls per page against today's ≈3 s; *Next* / *Prev*; a project and a host
   filter; `read_network_requests` shows `limit=51` and no `volumes/detail?all_tenants`. Record
   the numbers here.

**Slice B: Volumes, Snapshots, Floating IPs.** Same pattern. Volumes: attached servers by ID
(`servers/{id}` through `Bounded`, distinct IDs on the page; a 404 leaves the ID) instead of
`all_servers_brief`; `count` from `with_count=true` (volume 3.45) shown as "of N". Snapshots:
their volumes by ID (`volumes/{id}`, bounded) for name and host, instead of every project's
volumes. Floating IPs: networks with one `networks?id=…&id=…` GET (Neutron repeats a filter
for *any of*), servers by ID from `port_details.device_id`. Measure each before and after.

**Slice C: Networks, Routers, Trunks, Images, Groups, Group Snapshots.** Networks: subnets with
`subnets?network_id=…` for the page's networks; the DHCP-agent counts stay as they are (one
call per agent). Routers: external networks by `id=…`. Trunks: parent ports by `ports?id=…`
(also drops the "every port" call). Images: no lookups. Groups and group snapshots: types stay
(small lists).

**Slice D: server-side sort and *Count all*.** Column headers whose neutral key is in
`list.sorts()` set `sort`/`dir` in the URL (and clear the marker stack); other columns keep
sorting the current page on the client, with a `title` saying so. *Count all* (lists without a
cheap count): a link that calls `count_admin(list, field, value)` on demand, never before the
table shows: Nova `servers?all_tenants` brief, Neutron with `fields=id`, Glance `images` with
the same filter, all `Pagination::All`; the result replaces the link.

**Slice E: Project panels (`paged_lists = "all"`).** The big Project panels (Instances, Volumes,
Snapshots, Floating IPs, Images, Networks, Routers, Ports) take `ListQuery` the same way, with
`params` variants that keep each panel's project scoping (`tenant_id` filters, AGENTS.md §6) and
no `all_tenants`; `list_page!` in `pages/lists.rs` gets the paged branch. With `"admin"` they
stay whole. Measure Project › Instances on the production cloud's size class in dry-run.

## 89. Admin dashboard refinements

> 🔗 **Issue:** [#11]
>
> ✅ **Done** (2026-11-10). Bullets 5, 6 and 7 on 2026-11-08, the rest on 2026-11-10; bullet 10
> was already built and is left as it is (below). `[admin] filter_first` stays, off by
> default; the user doesn't want it on. **5:** a multi field (`dialog.rs`, `Kind::Multi`) keeps
> the values that were ticked when the dialog opened but are missing from its set as rows of their own,
> labelled by ID (`with_unknown`, test `multi_keeps_selected_values_missing_from_the_set`):
> *Modify Access* on a flavor shared with a deleted project shows that ID ticked, and unticking
> it removes it.
> **6:** `AllAssociablePorts` choices are keyed by `model::associable_port_key(project, pool)`, one
> per pool that reaches the port; `FloatingIpRow.pool_id` is new and the admin *Associate* dialog
> asks for the owner's ports its pool reaches. With `[network] fip_topology_check` (default on)
> the fetch also lists every project's routers, and `mapping::reachable_subnets_by_pool` keeps
> the subnets a router with its gateway on that pool has an interface on (Horizon's
> `_get_reachable_subnets`, subnet-level); with it off, every external network reaches every
> port (`PoolReach::Every`, one `networks?router:external` call). HA routers' interfaces
> (`network:ha_router_replicated_interface`) now count, in the Project check too
> (`is_router_interface`). Test `admin_associate_offers_the_ports_the_pool_reaches`. **7:**
> `fetch::dhcp_agent_counts` returns no counts when the cloud has no *DHCP agent* (OVN still
> advertises `dhcp_agent_scheduler`), and `AdminNetworksPage` drops the column when no row has a
> count (`drop_empty_dhcp_col`, test `dhcp_agents_column_only_with_counts`); a failed count hides
> it too. Both clippy runs clean, 572 tests pass. Checked in Brave on ttcp-dev (2026-11-10,
> nothing submitted): **5** *Modify Access* on `test-vms` lists 140 projects, the extra one a
> deleted project shown by its ID and ticked ("1 selected"); the other private flavors show
> none. **6** the server returned 64 choices under 18 project/pool keys; 80.80.80.116 (PROYECTO
> QA CC 001 AUTOGEST, `public-dummy-alh1`) offers `qaimagennewwin01: 10.10.0.42`, and pokemon's
> 80.80.80.242 on the same pool offers none, rightly: pokemon's free instance ports sit behind
> its gateways on two other pools, and poke-gw-4 reaches none of them. **7** Admin › Networks
> has no *DHCP Agents* column on this OVN cloud.
>
> 2026-11-10, checked in Brave on ttcp-dev (nothing submitted):
>
> - **1** Item 85's fixes, as intended: admin *Create Network* and *Create Router* show *Select
>   a project* and no zone hints (no zones here); *Create RBAC Policy* its placeholders and
>   help; *Live Migrate* offers 5 hosts, not the instance's own; *Migrate Volume* shows the
>   current pool and offers the other two; the 3 protected images have no *Delete Image* and
>   a disabled checkbox (28 others both). Admin › Overview against Horizon's *Usage Overview*
>   for 2026-11-09 → 2026-11-10: the same 54 projects, VCPUs, disk and RAM equal row by row,
>   the hours ahead by the same 0.0104 h per VCPU everywhere (Moonlight read ~37 s later).
>   One difference, fixed: Horizon writes a project Keystone no longer has as
>   `<id> (Deleted)`; `ProjectUsage.deleted` (set only when Keystone named other projects,
>   so a failed lookup marks none) adds `K::DeletedSuffix` in the table and `(Deleted)` in
>   the CSV (tests `tenant_usages_merge_pages_and_name_projects`,
>   `global_usage_totals_every_project_and_csv_has_a_line_each`). Seen after the
>   reload: `dc87707b… (Deleted)` in the table and the CSV.
> - **2** `fetch::admin_rbac_policies` also names `address_scope`, `subnetpool` and
>   `address_group` objects, each list read only when a policy uses that type (test
>   `rbac_policies_name_projects_networks_and_the_wildcard`). Horizon's *Create RBAC Policy*
>   offers only networks and QoS policies, as Moonlight's: no new pickers. ttcp-dev has only
>   network and QoS policies (41, still 4 API calls).
> - **3** Admin *Edit Router* (`ops::edit_router_item(.., admin)`) adds *Router Type* (with
>   `dvr`) and *High Availability* (with `l3-ha`) when Neutron returns the router's
>   `distributed` / `ha` (`RouterRow`, admins only), each with "only while down" help; a
>   change on a router that is up is refused in the dialog (Neutron checks the stored state,
>   so unticking *Enable Admin State* in the same save isn't enough). `RouterUpdate` sends
>   `distributed` / `ha` only when they change (dry-run test `ports_routers_and_interfaces`,
>   dialog test `admin_edit_router_changes_type_and_ha_only_while_down`). ttcp-dev has
>   `l3-ha` but no `dvr`: the dialog shows *High Availability* only, as admin *Create Router*.
> - **8** Admin › Images › *Create Image* (`ops::admin_create_image_dialog`): Project's form
>   plus a *Project* picker (`ChoiceSet::AllProjects`, empty = the session's), sent as
>   Glance's `owner` (`ImageSpec.owner`; dry-run test `image_create_records_then_imports_the_url`,
>   dialog test `admin_create_image_picks_the_owner`). Opened on ttcp-dev, not submitted.
>
> - **4** Metadata Definitions › a namespace (`/admin/metadata_defs/{namespace}`,
>   `pages/admin.rs::MetadefDetailPage`; the list's *Name* links to it), Horizon's two tabs:
>   *Overview* (display name, namespace, description, public, protected, created, updated;
>   the resource types with prefix and properties target) and *Contents* (properties: name,
>   title, type, default, allowed values, description; objects with their properties'
>   names; the definition's JSON, collapsed). The header has the row's menu. Data:
>   `api::get_metadef_namespace` (admins) → `fetch::metadef_namespace`, one raw
>   `GET metadefs/namespaces/{ns}` (name checked by `fetch::plain_namespace`, shared with
>   `metadata_options`) → `mapping::metadef_namespace_detail` → `MetadefNamespaceDetail`
>   (test `metadef_namespace_detail_lists_properties_objects_and_associations`; nav test: the
>   detail is under System › Metadata Definitions and the Glance gate). *Update
>   Associations* has a *Prefixes* key/value field (resource type → prefix, prefilled from the
>   list's new `MetadefNamespace.prefixes`); a changed prefix removes and re-adds the
>   association (Glance has no update), so `MetadefNamespaceSetTypes` now sends removals
>   first and a `prefix` with each addition (dry-run test
>   `metadata_definitions_import_edit_associate_and_delete`, dialog test
>   `metadef_menu_hides_delete_when_protected_and_sends_association_changes`). Checked on
>   ttcp-dev: `OS::Compute::LibvirtImage` has 16 properties and no objects, as Horizon's
>   page; CPU Pinning shows its three associations with `hw_`, `hw_`, `hw:`, and *Update
>   Associations* opens with them (cancelled).
>
> - **9** A new `[admin]` section (`prefs::AdminPrefs`, read and checked in `settings.rs`,
>   documented in `CONFIGURATION.md` and `moonlight.conf.example`). `flavor_edit` (default
>   `false`, Horizon's `ENABLE_FLAVOR_EDIT`) puts *Edit Flavor* first in a flavor's menu
>   (`ops::admin_flavor_menu`): *Create Flavor*'s two steps prefilled, without the ID;
>   saving is `OsAction::FlavorReplace`, Horizon's `UpdateFlavor`: `DELETE /flavors/{id}`,
>   `POST /flavors` with the same ID, its extra specs, then `addTenantAccess`. The create
>   request and the extra specs are checked before the delete (`actions::flavor_request`,
>   shared with *Create Flavor*), so a refused value leaves the flavor as it was. Tests: dry
>   run in `flavors_are_created_private_for_projects_and_edited_like_horizon`, dialog
>   `edit_flavor_only_when_enabled_and_recreates_with_the_same_id`. `filter_first` (default
>   `[]`, Horizon's `FILTER_DATA_FIRST`) names paged Admin lists (`PagedList::KEYS`:
>   `instances`, `volumes`, …) that answer with an empty page and no OpenStack call until the
>   query has a filter (`api::waits_for_filter`, `Page.needs_filter`); the table says why
>   (`pager::filter_bar`, `K::FilterFirstNote`). Only for paged lists: with `page_size = 0`
>   the whole list loads and is searched in the browser. Settings test
>   `admin_section_is_horizons_flavor_edit_and_filter_first`. Checked on ttcp-dev with a
>   temporary `~/.moonlight/moonlight.conf.d/` file (`filter_first = ["instances"]`): Admin ›
>   Instances shows the note and *0 API*; `?f=name&q=prueba2` lists it (2 API). With
>   `flavor_edit = true`, *Edit Flavor* didn't show at first: the settings reach the browser
>   in an `Effect` after the server-rendered table built its menus with the defaults, and
>   `dialog::prefs()` reads them untracked. Fixed with `dialog::prefs_tracked()` (the menu is
>   rebuilt once they arrive). Then on `test-vms`: *Edit Flavor* first in the menu; the
>   dialog shows the ID and the re-creation warning, prefilled 3 VCPUs, 5000 MB, 10 GB, 0, 0,
>   1; *Flavor Access* has its one project ticked (the deleted `99c95913…` of bullet 5, which
>   a save would add back, as Horizon's would). Cancelled; the temporary file is removed.
> - **10** Already built: `fetch::admin_resource_providers` reads each host's first GPU's
>   traits, and the host's other GPUs only when that one has a `CUSTOM_*` trait. ttcp-dev now
>   has vGPU types (`CUSTOM_NVIDIA_H100_47C`, `CUSTOM_NVIDIA_L4_24Q`…), so every GPU is
>   read: *Resource Provider* took 520 calls, 1.42 s (6 hosts, 169 GPUs: 1 + 2 × 175 + 169).
>   Reading one GPU per host (6 instead of 169) would assume one vGPU type per host; not done.
>
> Both clippy runs clean, 579 tests pass.

**Goal.** Close the gaps item 85 left in the Admin dashboard, found by reading Horizon and the
browser pass on ttcp-dev (2026-11-01). None blocks daily use; each is a place where Horizon shows
or offers a bit more:

1. **Recheck and Overview against Horizon.** See item 85's last fixes in the browser (admin
   *Create Network* / *Create Router* placeholders and hidden zone hints, *Create RBAC Policy*,
   *Live Migrate* and *Migrate Volume* without the current host / pool, protected images).
   Compare Admin › Overview's per-project numbers (VCPUs, disk, RAM, hours) and the CSV with
   Horizon's *Usage Overview* for the same month. Not done in item 85: the session expired and
   Horizon needs the user's sign-in.
2. **RBAC object names.** Objects other than networks, QoS policies and security groups (address
   scopes, subnet pools, address groups) show their ID in the *Object* column and pickers.
3. **Admin Edit Router.** Admin's *Edit Router* is Project's: no *Router Type* (distributed) or
   *High Availability* changes. Neutron allows them only with the router down, so the dialog
   says so, and they need the `dvr` / `l3-ha` extensions (`needs::…`).
4. **Metadata Definitions.** The rows are not clickable: there is no namespace detail view.
   As in Horizon (`/admin/metadata_defs/<namespace>/detail`), the *Name* links to a page with
   two tabs: *Overview* (display name, namespace, description, public, protected, created and
   updated, and the associated resource types with their prefix and properties target) and
   *Contents* (the namespace's properties and objects, Horizon shows the definition's JSON).
   The row menu stays the same and the detail header offers it too. Separately, *Update
   Associations* can't set the per-resource-type prefix.
5. **Flavor Modify Access with deleted projects.** A flavor whose access list names a project
   that no longer exists says *1 selected* but shows nothing ticked, and saving silently drops
   it. Show unknown selected IDs as their own rows (ID as label), as the multi-select does for
   unknown values elsewhere.
6. **Admin Associate ports.** Horizon offers only the ports whose network reaches the floating
   IP's pool through a router; Moonlight lists every instance port of the owner, so Neutron
   may refuse the association. Filter by router reachability (the owner's routers with that
   external gateway, and their subnets).
7. **DHCP Agents column on OVN.** Networks' *DHCP Agents* is 0 everywhere on OVN clouds, which
   have no DHCP agents; hide the column when the cloud has no agent of type *DHCP agent*.
8. **Admin Create Image.** Horizon's Admin › Images has *Create Image*; Moonlight's admin list
   has none (Project's exists: reuse it, with a project picker for the owner).
9. **Horizon settings not built:** `ENABLE_FLAVOR_EDIT` (an *Edit Flavor* that deletes and
   re-creates, as Horizon) and `FILTER_DATA_FIRST` (an empty admin table until a filter is
   given; with item 88's server filters).
10. **Resource Provider load:** optionally read the traits of one GPU per host only (6 calls
    instead of 199 on ttcp-dev; assumes one vGPU profile per host).

**Implementation.** Independent bullets, each S, in this order of use: 5 and 6 (they can lose or
refuse a user's change), 1, 7, 2, 3, 8, then 4, 9 and 10.

- 1: browser only (the user signs in to Horizon; GET only, AGENTS.md §1.4); fix
  `model::ProjectUsage` / `mapping::tenant_usages` if numbers differ.
- 2: `mapping` / `fetch::admin_rbac` look up names of the other object types (`address-scopes`,
  `subnetpools`, `address-groups`, each `soft()`), and `ChoiceSet`s for their pickers.
- 3: new `OsAction` fields on the admin router update (`distributed`, `ha`), with a dry-run test;
  `ops.rs` admin router items; gate on the extensions.
- 4: route `metadata_defs/:id` in `app.rs` (under `/admin`) to a new `MetadefDetailPage` in
  `pages/admin.rs`, shaped like `heat_catalog::ResourceTypeDetailPage` (`DetailHeader` with
  `ops::metadef_items` as its menu, `Tabs` as `identity.rs`'s detail pages, `Card` / `Kv` for
  the overview). In `metadef_columns` the *Name* cell becomes a `link` to
  `/admin/metadata_defs/{namespace}`. Data: a server fn `get_metadef_namespace(id)` in `api.rs`
  (admin, like `list_metadef_namespaces`) calling a new `fetch::metadef_namespace`, one raw
  `GET metadefs/namespaces/{namespace}` (the call `fetch::metadata_options` already makes,
  without `resource_type`; reuse its check on the name's characters) and a
  `mapping::metadef_namespace_detail` into a new `model::MetadefNamespaceDetail`: the list
  fields, `created_at` / `updated_at`, `resource_type_associations` (name, `prefix`,
  `properties_target`), properties (name, title, type, description, default, enum) and objects
  (name, description, their properties), plus the pretty JSON for a collapsed *Raw* block as
  the resource type page has. Contents as two `DataTable`s (properties; objects with their
  property names). Mapping test with a Glance fixture (`OS::Compute::Watchdog` has properties,
  `OS::Software::DBMS` objects); nav test that the detail is under the Glance gate.
  Prefixes in associations: `MetadefResourceTypes` choices with a `prefix` (the `prefix` field of
  the association body), with a dry-run test.
- 5: `dialog.rs` multi field: selected values missing from the loaded set become extra choices
  labelled by ID (test in `dialog.rs`).
- 6: a new `ChoiceSet` (or a filter on `AllAssociablePorts`) keyed by the floating IP's network;
  reachability from the owner's routers' gateways and interfaces, as Horizon's
  `FloatingIpManager._get_reachable_subnets`.
- 7: `pages/admin.rs` networks columns read the agent list (already fetched for System
  Information) or a `has_dhcp_agents` flag in the admin network fetch.
- 8: `ops.rs` image create dialog with an owner `project_id` field (`ChoiceSet::AllProjects`,
  placeholder, as admin *Create Network*).
- 9: `[admin]` settings in `server/settings.rs`, documented in `CONFIGURATION.md` and
  `moonlight.conf.example`.
- 10: `fetch.rs` Resource Provider tab, one trait read per host.

Tests next to each change; browser check on ttcp-dev with dialogs opened, never submitted.

## 90. Volume Backups leftovers

> 🔗 **Issue:** [#12]

**Goal.** Finish what item 5's slice 5 left out of the `backup` plugin (Volumes › Backups), none
of which Horizon's Project panel needs for daily use:

1. **Back up from a snapshot.** `OsAction::BackupCreate` already takes `snapshot_id` (dry-run
   tested); the UI never sets it. Add a *Snapshot* select to *Create Volume Backup* (new
   `ChoiceSet` of the chosen volume's snapshots via `Source::Of`, as the instance ports do) and
   a *Create Backup* entry in the snapshot menu (`ops::snapshot_items`).
2. **Metadata and availability zone.** `create_351` accepts `metadata` (volume 3.43) and
   `availability_zone` (3.51): gate both with `needs` constants and add the fields; show them
   on `BackupDetailPage` (`BackupRow` needs `metadata`).
3. **Export / import records** (`backup::export_record`, `import_record`), admin-oriented:
   *Export Record* shows the two strings (`backup_service`, `backup_url`, as a secret-style
   dialog) and *Import Record* takes them. Ask before building: Horizon has neither.
4. **Admin › Backups.** Horizon's admin dashboard has no Backups panel; `all_tenants=true` would
   give one, with *Reset Status* (`os_reset_status`) and *Force Delete* (`os_force_delete`).
   Same `backup` plugin; filter nothing, show the project.
5. **Verify in a browser** on a cloud that runs `cinder-backup` (ttcp-dev does not, and
   `moonlight.conf` leaves the plugin off): list, detail, and each dialog in dry-run, polling
   while `creating` / `restoring`.

**Implementation.** Follow AGENTS.md's recipes: model in `model.rs`, mapper in `mapping.rs`,
GETs in `fetch.rs::backups` / `backup`, writes in `actions.rs` (typed endpoints under
`block_storage::v3::backup`) with a dry-run test per body, dialogs in `ops.rs`
(`backup_form`, `backup_items`), i18n in both languages. Everything stays behind
`needs::BACKUP` / `Plugin::Backup`. Open question: whether items 3 and 4 are worth building at
all.

## 91. Scheduler Hints step in Launch Instance

> 🔗 **Issue:** [#13]

**Goal.** Horizon's *Launch Instance* has a *Scheduler Hints* step when
`LAUNCH_INSTANCE_DEFAULTS["enable_scheduler_hints"]` is on: the user picks hints from a
catalog (`OS::Nova::...` metadata definitions with the `scheduler_hints` property target) and
sets their values (`same_host`, `different_host`, `query`, `build_near_host_ip`…), and Nova
gets them as `os:scheduler_hints`. Moonlight sends only the server group hint today.

**Implementation.**

1. Setting `[launch] enable_scheduler_hints` (default `false`, as Horizon), in
   `server/settings.rs`, `CONFIGURATION.md` and `moonlight.conf.example`; served with the other
   launch preferences (`prefs.rs`), so the step is absent when off.
2. A `scheduler_hints: Vec<(String, String)>` in `LaunchSpec` (`model.rs`), validated like
   metadata (`validate_metadata`: trimmed, no empty or repeated keys). Open question: values
   such as `same_host` take instance IDs; a first version can take free key/value rows
   (`Field::key_values`) and leave the catalog to a second step.
3. `actions.rs::launch` (near `OsSchedulerHintsBuilder`, where the server group is set): the SDK
   builder has typed fields for the known hints and `additional_properties` for the rest; check
   the module (`compute::v2::server::create_*`) before choosing. The server group's `group`
   hint must keep working, and a user-supplied `group` must not override it.
4. Dialog step in `ops.rs` (`launch_instance`, after *Server Groups*), i18n in both languages.
5. Tests: a dry-run test asserting `os:scheduler_hints` with a group plus extra hints; a dialog
   test for the step's presence only when the setting is on.

## 92. Heat Template Generator

> 🔗 **Issue:** [#14]
>
> ✅ **Done** (2026-11-10). Slice 1 (model and form) and slice 2 (the canvas), checked in the
> browser on desktop, phone width with touch, and in Spanish.
>
> *Orchestration › Template Generator* (`/project/template_generator`, `pages/template_generator.rs`,
> panel after *Template Versions*, `needs::ORCHESTRATION` like the rest of the group; *Resource
> Types* links to it). Reads only: the type list, the template versions and each type's schema
> (`get_resource_type`, once per type in the draft); nothing is sent to Heat from the page.
>
> - Palette: Horizon's 18 generator types, by service, with heat-dashboard's own SVG icons
>   (`components/hot_icons.rs`, see slice 2; `TypeIcon`), less the types this Heat doesn't
>   list. *Other type…* (a `Combo`) adds any of the cloud's types, drawn with their initials.
> - Each resource's form comes from its type's schema: required properties first, then
>   *Show all properties*. `TypeProperty` gained `allowed` (select) and `custom`
>   (`custom_constraint`), tested in `mapping::resource_type_schema_tables`. A string
>   property's value is typed, a `get_param`, or a `get_resource` to a draft resource of the
>   type its constraint names (`hot::constraint_types`; any resource when unconstrained);
>   `list`/`map` take JSON. Plus name (rename follows every reference), `depends_on`,
>   parameters and outputs (`get_attr`, attributes from the schema) as small tables.
> - The pure model is `components/hot.rs`: `Draft` → HOT YAML (own small writer: keys in
>   template order, flow style for `get_*` and lists of scalars, literal blocks for
>   `user_data`, YAML 1.1 quoting), `problems` (no resources, empty or duplicate names,
>   missing required properties, values that don't fit their type, dependency cycles through
>   `depends_on` and `get_resource`, incomplete outputs), 14 tests.
> - Live preview with the problems, *Copy*, *Download* (`data:` URL, `template.yaml`) and
>   *Launch Stack*, which opens *Launch Stack* with the template as direct input
>   (`stack_launch::hand_template`). The draft is kept in `localStorage` (web-sys `Storage`).
>
> Checked in Brave on ttcp-dev (dry-run): Net, Subnet and Server added; the subnet's `network`
> offers only the Net; renaming it to `private` rewrote `{get_resource: private}`; a `flavor`
> parameter used by the server; an output on `server_1.addresses`; no problems left, and
> *Launch Stack* opened with the same YAML (*Next* not pressed). The draft survived a reload.
> Fixed then: selects whose options load later (version, parameters, resources, attributes)
> now mark the chosen option `selected`. That last fix passed both clippy runs and the
> tests, but the browser connection dropped before it could be rechecked in the page.
>
> Slice 2, the canvas (`DraftCanvas` in the *Resources* card, on `topo_canvas::Canvas`; the
> list stays under it for the keyboard):
>
> - Icons: heat-dashboard's own SVGs (`template_generator/js/resources/os__*/os__*.svg`,
>   Apache 2.0), not draw.io's look-alikes that `heat_icons.rs` keeps for the stack *Topology*.
>   `scripts/gen_hot_icons.py <heat-dashboard checkout>` writes `components/hot_icons.rs`
>   (markup per type, inserted with `inner_html`; black → `currentColor`, white → classes
>   `bg-f`/`bg-s`), so CSS colours them: accent, or gray while the resource has missing or bad
>   properties (Horizon's gray until edited). Other types keep their initials in a ring.
> - Nodes at `DraftResource.pos` (`Draft::add` takes the first free cell of a 130-unit grid,
>   6 to a row; `Draft::positions` places drafts saved without one). Drag a node to move it
>   (committed on release); drag a palette type onto the canvas (HTML drag and drop, web-sys
>   `DragEvent`/`DataTransfer`) or click it. Pan and zoom are the shared canvas'.
> - Links: drag a node's + handle onto another. `Draft::link_choices` applies the only
>   reference that fits, either way round (port → server still sets the server's
>   `networks[].port`), falls back to `depends_on`, or asks in a small menu. Choices come from
>   `link_options`: string properties whose `custom_constraint` names the target's type (or,
>   unconstrained, whose name contains it: `floatingip_id`), then the list items of
>   `hot::NESTED` (server `networks[].port/network/subnet/floating_ip`,
>   `block_device_mapping_v2[].volume_id`, `security_groups[]`; port `fixed_ips[].subnet`,
>   `security_groups[]`), then `depends_on`.
> - One arrow per pair (`edge_pairs`, labels joined, dashed when only `depends_on`); clicking it
>   offers *Remove link* (`Draft::unlink`). `get_resource` inside `list`/`map` JSON now counts
>   everywhere: edges, `needs` and so cycles, `rename` (rewrites the JSON, keys re-sorted by
>   serde_json) and `remove` (drops the items that point at it; an emptied list is unset).
> - Tests: 6 new in `hot` (edges inside lists, link options, link choices, unlink/rename/remove
>   in lists, grid cells, pairs and hit testing) and `hot_icons::every_palette_type_has_an_icon`.
>   Both clippy runs clean, 569 tests pass.
>
> Browser pass (2026-11-10, Brave on ttcp-dev, nothing sent to Heat): the `selected` fix holds
> after a reload (version, parameter type, output resource and attribute). Found and fixed:
>
> - **The canvas had no usable `viewBox`.** `topo_canvas::Canvas` measured its `<svg>` before
>   layout (0 × 0), so the first view was `0 0 0 0`; the browser ignores that and draws in raw
>   units, so it *looked* right, but `to_world` mapped every pointer wrong: linking by the +
>   handle never found the target, drops and moves landed elsewhere, and a wheel zoom turned it
>   into `-104 NaN 104 NaN` (`zoom` computed `h = w · 0 / 0`). Moves then saved `y` = `NODE_R`
>   for every node (NaN clamped by `max`). Now a 0 × 0 box is measured again on the next frame
>   (`request_animation_frame`), `zoom` leaves an empty view alone (test
>   `zoom_leaves_an_empty_view_alone`), and `to_world` drops non-finite points. *Network
>   Topology* shares the canvas and still opens at actual size.
> - **Two links between the same nodes, one each way** (a `depends_on` against a
>   `get_resource`) were drawn on the same line, labels on top of each other. `hot::edge_place`
>   moves each `EDGE_GAP` to its own right, with its label further out on that side
>   (`text-anchor` start/end beside a rather vertical line); test
>   `edges_both_ways_are_drawn_apart`. Labels are drawn after the nodes, so a short edge's
>   label is no longer hidden by them.
>
> Then checked: adding from the palette (a Port, gray until `network` is set; while selected the
> selection ring makes it look blue), linking port → net by the handle (sets `network`, the
> problem goes away), a `depends_on` link the other way (the cycle is reported), *Remove link*,
> and moving nodes (they land where dropped). Both clippy runs clean, 574 tests pass.
>
> Phone and touch (2026-11-10, Brave emulating 390 × 844 with touch, real touch input through
> DevTools' `Input.dispatchTouchEvent`):
>
> - The canvases use pointer events instead of mouse events: `topo_canvas::Canvas` (pan; a
>   second finger is ignored, `is_primary`) and the generator's nodes, + handle and edges.
>   *Network Topology* and the stack *Topology* pan by finger too. The handle has a
>   finger-sized transparent target (`tg-handle-hit`, r = 16) around the visible one. Checked:
>   a swipe on the empty canvas pans it by the swipe's length; dragging port_1 moves it by
>   exactly the drag and doesn't pan; tapping selects it; dragging its handle onto server_1
>   opens *Link as* (cancelled).
> - The first view on a phone was the whole drawing at a third of its size: the canvas is
>   laid out several frames after its `Effect` here, so the one-frame re-measure still saw
>   0 × 0. It now retries each frame until the box has a size (up to 60 frames,
>   `topo_canvas::after_layout`), and opens at actual size (`0 0 332 520`), panned for the rest.
> - The layout is one column below 760 px with no horizontal page scroll; the parameter and
>   output tables scroll in their own `table-wrap`.
> - *Link as* offered `server_1.software_config_transport → port_1`: unconstrained properties
>   matched any name *containing* the type's. Now the name must be the type's, `_id` or not
>   (`hot::names_type`: `port`, `port_id`, `floatingip_id`), tested in
>   `drawing_a_link_offers_what_fits_the_target`.
>
> Both clippy runs clean, 574 tests pass.
>
> Last checks (2026-11-10): in Spanish the whole page is translated (the palette keeps Heat's
> type names, as Horizon); every string is a `K::` key, the only literals are `×`, `-` and `*`.
> Palette drag-and-drop with a real mouse (DevTools' drag interception: `mousePressed` on
> *Volume*, Chrome starts the HTML drag with `application/x-moonlight-heat-type`, dropped on
> the canvas): volume_1 lands within 2 px of the drop point, selected. On a phone, where HTML
> drag-and-drop isn't available, tapping a palette type adds it. Not done, a possible follow-up:
> the stack *Topology* could move to these icons too; nodes placed very close let an edge label
> cover a node's name (labels are drawn above nodes).

**Goal.** Horizon's *Orchestration › Template Generator*: a drag-and-drop canvas where the
user adds resources (from the *Resource Types* list), connects them, fills their properties and
downloads or launches the resulting HOT template. It was left out of item 62 as the largest and
least used piece, and the production Horizon shows it. Whether to build it at all is an open
question: *Launch Stack* with a template file already covers the use.

**Implementation.** Everything it needs on the server already exists from item 62: the
resource type list and the type schema (`GET /resource_types/{type}`, in `server/heat.rs` and
`fetch.rs` through `JsonRequest::orchestration`) and *Preview Stack*, which validates the
template (`POST validate`, `OsAction` through `Exec`, AGENTS.md §1.3). New work, all client
side:

1. A `pages/template_generator.rs` page under Orchestration (route in `app.rs`, a panel in
   `components/nav.rs` with `needs::ORCHESTRATION`), reached from Resource Types too.
2. A canvas: nodes per resource, edges for `get_resource` / `depends_on`. Reuse the SVG layout
   of `components/topo_layout.rs` (the Network Topology and the stack Topology tab); drag and
   drop needs pointer events through `web-sys` (mind AGENTS.md §6: disposed signals on
   `mouseleave`/`blur`, so use `try_get`/`try_set`).
3. A properties form per resource, generated from the type's schema (property type, required,
   default, constraints); parameters and outputs as small tables.
4. Output: a YAML HOT template (`heat_template_version`, parameters, resources, outputs) to
   copy, download, or send to *Launch Stack* / *Preview Stack* (`pages/stack_launch.rs`).
5. Tests: a pure function from the canvas model to the template (unit-tested, including
   `get_resource` references and a dependency cycle), and the schema-to-field mapping with
   Heat-shaped JSON.

Split in two slices when started: the pure model plus the form first, the canvas after.

## 93. Firewall (FWaaS v2): firewall groups, policies and rules

> 🔗 **Issue:** [#15]
>
> 🚧 **In progress.** All three slices are built and checked in the browser on ttcp-dev
> (2026-11-06); what is left needs a router or an instance in the project. Plugin `firewall`
> (`model::Plugin::Firewall`, `needs::FIREWALL` = plugin and `fwaas_v2`), documented in
> `CONFIGURATION.md` and `moonlight.conf.example`, and on in the repo's `moonlight.conf`.
> *Network › Firewall* (`nav.rs`) has three link tabs (`pages/firewall.rs::FirewallTabs`, CSS
> `.tabs a`): `/project/firewall` (groups, polling while `PENDING_*`),
> `/project/firewall/policies`, `/project/firewall/rules`, built with `list_page!` and its
> `above` slot (`lists.rs`). Detail pages `/project/firewall/{groups,policies,rules}/:id`: a
> group's ports resolved to their router or instance (*Remove Port* per row, *Add Port*), a
> policy's rules in evaluation order (*Remove Rule* per row, *Insert Rule*), a rule's match and
> the policies using it. Reads: `fetch::firewall_*` (raw GETs under `fwaas/`, own rows by
> `project_id` merged with `shared=true`), mappers in `mapping.rs` with 5 tests. Writes: the 12
> `OsAction::Firewall*` through `actions::fwaas` (JSON requests, `ServiceType::Network`),
> dry-run test `firewall_rules_policies_and_groups`; validation in `FirewallRuleSpec::body`
> (ports only on TCP/UDP, `22` or `8000:8080`, addresses of the rule's IP version), run by the
> dialog too. Dialogs in `ops.rs` (test `firewall_dialogs`): **no *Shared* anywhere** (it
> failed on ttcp-dev for groups, and the user dropped it for policies and rules too, so
> `FirewallRuleSpec`, `FirewallPolicyCreate` / `Update` and `FirewallGroupCreate` / `Update`
> have no `shared` field and never send it; the lists and detail pages still show Neutron's
> value). Another project's shared rows have no actions and can't be bulk-selected. In
> the *Add / Edit Rule* dialog, protocol, IP version and action share one line
> (`Field::short`), then source address and port, then destination address and port, each pair
> on its own line (`Field::new_line`, new: keeps the destination on its line when the ports are
> hidden for *Any* / ICMP). *Insert Rule* offers *At the top* or *After* each rule (Neutron's
> `insert_after`), defaulting to the end, and leaves out rules the policy has
> (`dialog::Source::Without`). Rule pickers: `firewall_rule_choices` labels a rule by name and
> gives the picker Name, Action and Protocol columns translated in the browser
> (`pick::Fmt::FwAction`, `Fmt::Protocol`; the label used to carry English `(allow, any)`).
> Ports: `ChoiceSet::FirewallPorts` (router interfaces and instance ports that no group has;
> columns Device, Type, IP Addresses and Port, `Fmt::Device`); Neutron takes the whole list, so
> *Add* / *Remove Port* send `FirewallGroupSetPorts`. *Create Firewall Group* is a two-step
> wizard: the group (name, description, ingress and egress policy with an (i) on the direction,
> admin state), then its ports (optional). Status `INACTIVE` reads *Inactive* / *Sin aplicar*,
> grey (Neutron also reports it for a group with ports but no policy, so not *No ports*); the
> group detail explains it (`K::FirewallInactiveHelp`). Neutron's `default` group and its
> `default ingress` / `default egress` policies (neutron-fwaas matches them by name) are
> protected like the `default` security group (item 34): `FirewallGroupRow` /
> `FirewallPolicyRow::is_default` and `deletable`; the group offers only *Add* / *Remove Port*
> and the policies only *Insert* / *Remove Rule* (Neutron refuses the rest:
> `FirewallGroupCannotRemoveDefault`, `FirewallDefaultObjectUpdateRestricted`), neither can be
> bulk-selected for delete, and the group detail says why (`K::FirewallDefaultGroupHelp`). The
> picker's empty *Available* table says why it is empty (`pick::available_hint`: nothing to
> choose, all selected, or the filter) instead of "Select one or more…". Found while checking
> the dialogs: closing any dialog before its choice lists loaded panicked
> (`DialogView` read the disposed `choices` signal after the await) and killed the page; it now
> uses `try_get_untracked`. **Checked in Brave** (ttcp-dev, project *admin2*, Spanish, nothing
> submitted): the three tabs with Neutron's `default` group, its two policies and four rules;
> the three detail pages; *Create Firewall Group* (both steps), *Edit Firewall Group*,
> *Add / Edit Rule* (TCP and *Any*), *Add / Edit Policy*, *Insert Rule*, *Remove Rule*; dialogs
> opened and closed at once three times with no panic; no console errors. Clippy (both builds)
> clean, 546 tests pass. The `default` protection, the *Inactive* wording and the picker's
> empty states (2026-11-06, later) pass clippy and 547 tests, and were **checked in Brave**
> the same day (*admin2*, English and Spanish, nothing submitted): the `default` group and
> both policies have their checkboxes and *Select all* disabled; the group offers only *Add
> Port* (no ports, so no *Remove Port*), each policy only *Insert* / *Remove Rule*, in the list
> and on its detail page; *Inactive* / *Sin aplicar* is a grey pill, and the group detail shows
> both explanations; *Add Port* with no ports says *There is nothing to choose from.*,
> *Add Policy* says *No matching items* for a filter that hides everything and *Every item is
> already selected.* with all four rules allocated; no console errors. **Port picker checked
> on *pokemon*** (it has routers and instances): *Add Port* and step 2 of *Create Firewall
> Group* list 9 ports (3 router interfaces, 6 instance ports) with device, type, IP and port
> name. One router interface's router is not in the project's router list (another
> project's), so its Device cell shows the router's UUID: could say *Router (another
> project)* instead. **Left**, all needing a write (not done; ask before submitting): *Add
> Port* / *Remove Port* for real and the group detail's port table with rows; what Neutron
> does with no policy in a direction (the select says *None (no filtering)*); and that an
> instance port's direction matches the (i). Use a throwaway instance's port, never
> `porygon` (the dev VM: the `default ingress` policy denies everything). Later: link a router
> or instance port's page to its group; the policy detail could explain the `default` limits
> as the group detail does.

Requested 2026-11-05. ttcp-dev has it: Neutron lists the `fwaas_v2` extension (checked that day
through `get_session_info`, project *pokemon*, user `admin2`, GET only).

**Goal.** A *Project › Network › Firewall* panel, for clouds that run FWaaS v2, with
neutron-fwaas-dashboard's three tabs: *Firewall Groups*, *Firewall Policies* and *Firewall
Rules*. The Admin side is item 94.

- *Firewall Rules*: Name (linked), Protocol (`tcp`, `udp`, `icmp`, any), Source and Destination
  IP / port, Action (`allow`, `deny`, `reject`), IP version, Enabled, Shared, and the policy that
  uses it. *Add Rule*, *Edit Rule*, *Delete Rule* (bulk).
- *Firewall Policies*: Name, the ordered rules, Audited, Shared. *Add Policy* (name,
  description, rules picked and ordered), *Edit Policy*, *Insert Rule* (before or after a rule
  already in it), *Remove Rule*, *Delete Policy*.
- *Firewall Groups*: Name, Ingress and Egress policies (linked), Ports, Status (`ACTIVE`,
  `INACTIVE`, `PENDING_CREATE` / `_UPDATE` / `_DELETE`, `ERROR`), Admin state, Shared.
  *Create Firewall Group* (name, description, ingress and egress policy, ports, admin state),
  *Edit*, *Add Port* / *Remove Port*, *Delete*.
- A detail page per group, policy and rule, cross-linked; a router or instance port's page
  could link to the group that protects it (later).

**Implementation.**

- Gating like trunks: `model::Plugin::Firewall` (`firewall` in `[plugins] enabled`, added to
  `Plugin::ALL`, documented in `CONFIGURATION.md` and `moonlight.conf.example`),
  `needs::FIREWALL = All[Plugin(Firewall), NetExt("fwaas_v2")]`, a panel in the Network group of
  `components/nav.rs` (*Firewall*, `/project/firewall`, next to `K::Trunks`), `plugin_enabled` in
  every server fn, the actions in `OsAction::plugins`.
- **Not in the SDK** (`openstack-sdk-network` 0.22.8 has no `fwaas` module): raw requests with
  `actions::JsonRequest` (`ServiceType::Network`) for writes and `fetch::get_json` for reads, as
  `fetch::trunks` does. Paths under `fwaas/`: `firewall_groups`, `firewall_policies`,
  `firewall_rules`, each filtered by `project_id` (AGENTS.md §6: Neutron returns every project's
  rows to admins). Shared policies and rules of other projects are readable too: list them
  with `shared=true` for the pickers.
- Models `FirewallGroupRow`, `FirewallPolicyRow`, `FirewallRuleRow`; pure mappers in
  `mapping.rs` tested with Neutron-shaped JSON (rule with `null` ports and IPs read as *any*,
  policy rule order kept, group ports resolved to their device: router or instance).
- Writes, one `OsAction` each with a dry-run test of the exact path and body:
  `FirewallRuleCreate` / `Update` / `Delete`, `FirewallPolicyCreate` / `Update` / `Delete`,
  `FirewallPolicyInsertRule { id, rule_id, before, after }` (`PUT
  fwaas/firewall_policies/{id}/insert_rule`), `FirewallPolicyRemoveRule` (`…/remove_rule`),
  `FirewallGroupCreate` / `Update` / `Delete` (ports are a full list in the update body, so *Add
  Port* / *Remove Port* send the new list). Validation in `model`: protocol and port ranges
  (`22`, `8000:8080`; none for `icmp` or any), CIDRs matching the IP version.
- Groups move through `PENDING_*`: the list polls while any row is busy (`list_page!`'s `busy`,
  roadmap 46).
- Ports for a group: the project's router interface ports and instance ports, not ports already
  in another group (Neutron refuses those); a new `ChoiceSet`, labelled by device name and IP.
- *Shared* is not offered (see the progress note): Moonlight never sends `shared`.
- i18n for every label, status and action, both languages. Verify on ttcp-dev: read-only first,
  then open each dialog (writes are allowed there, but do not submit unless the user asks).

**Slices.** (1) the three tabs and detail pages, read-only; (2) rules and policies writes;
(3) groups writes and ports.

**Open questions.** Whether the production cloud runs FWaaS (only ttcp-dev is known).

## 94. Admin › Network › Firewall

> 🔗 **Issue:** [#16]

Split from item 93 (2026-11-05): not for now.

**Goal.** An *Admin › Network › Firewall* panel with the same three tabs as item 93 (*Firewall
Groups*, *Firewall Policies*, *Firewall Rules*), listing every project's rows with a *Project*
column and filter, so operators can see which ports a firewall group protects and clean up
leftovers. neutron-fwaas-dashboard has no admin panel; this goes beyond Horizon.

**Implementation.**

- Reuses item 93's models, mappers, dialogs and `OsAction`s; the reads drop the `project_id`
  filter (admin token) and resolve project names with `project_names`, as the other Admin lists.
- A panel in `nav::ADMIN_NAV`'s Network group gated by `needs::ADMIN` and `needs::FIREWALL`;
  `require_admin` in its server fns.
- Paged like the other Admin lists (item 88): a `PagedList::FirewallGroups` (and policies,
  rules) with Neutron's `sort_key` / `sort_dir` and `project`, `name`, `status` filters.
- Writes: *Edit* and *Delete* across projects. Dry-run tests as in item 93.
- ***Shared* lives here only** (user's decision, 2026-11-06). The Project panel (item 93)
  never offers it, not even to admins, and never sends `shared`, so Neutron's default
  (`false`) applies on create and an edit leaves the value as it is. To add it here:
  - put `shared: Option<bool>` back on `FirewallRuleSpec` (inserted into the body only when
    `Some`), `FirewallPolicyCreate` / `Update` and `FirewallGroupCreate` / `Update`
    (`model.rs`, `actions.rs`); item 93 removed it on 2026-11-06 before it was ever committed,
    so write it again (`if let Some(sh) = shared { body.insert("shared", sh) }`). Project
    dialogs pass `None`;
  - give the dialog builders in `ops.rs` (`firewall_rule_dialog`,
    `create_firewall_policy_dialog`, `firewall_policy_items`, `create_firewall_group_dialog`,
    `firewall_group_items`) a view flag (Project / Admin), not a role check: an admin in the
    Project panel still gets no checkbox;
  - dry-run tests: Project sends no `shared`, Admin sends the checkbox's value;
  - **find out first why sharing a group failed on ttcp-dev** (item 93, 2026-11-06; the
    error wasn't recorded): Neutron's policy, or the FWaaS driver refusing shared groups. If
    the driver can't share groups, offer *Shared* only on policies and rules.

**Open question.** Whether operators need it at all, or Project's panel with the admin's own
project is enough.

## 95. *Create Image* › *File* (`direct`): a token that can only upload, not the session's

> 🔗 **Issue:** [#17]

**Goal.** With `[images] upload = direct` (item 45) the browser `PUT`s the file straight to
Glance, and to sign that request `get_image_upload_ticket` hands the page the session's
**scoped Keystone token**. Until it expires (about an hour) that token can do anything the user
can do in the project, so a malicious browser extension or an XSS bug that reads it could
delete instances, change networks, and so on. Everywhere else Moonlight keeps tokens on the
server (AGENTS.md §1.5–6); this is the one exception (§1.3). Give the page a token that can only
upload to that one image instead.

This **improves on Horizon**, which does the same as Moonlight today:
`openstack_dashboard/api/glance.py`, `ExternallyUploadedImage`, returns
`token_id = request.user.token.id` (the full session token) with `upload_url` to the Angular
client, with no restriction or shorter lifetime. Horizon has nothing like this item.

**Implementation.** Keystone application credentials with access rules (Keystone and
keystonemiddleware Train or later):

1. On `get_image_upload_ticket` (`server/upload.rs::ticket`), create an application
   credential for the session user on the current project with one access rule
   `{"service": "image", "method": "PUT", "path": "/v2/images/<id>/file"}` (the image ID
   literally, not a wildcard), `unrestricted: false`, the user's roles, a generated secret
   and `expires_at` a few minutes ahead, sized from `bytes`. Item 57 already builds this
   request: `OsAction::AppCredentialCreate(AppCredentialSpec)` with `access_rules`
   (`actions.rs`, its dry-run test), so reuse it through `Exec`.
2. Exchange it for a token (`POST /v3/auth/tokens`, method `application_credential`, `id` +
   `secret`) and return that token in `UploadTicket` instead of the session's. The secret stays
   on the server and is never logged.
3. When the upload ends (success, failure or *Cancel upload*), the dialog calls a server fn
   that deletes the credential (`OsAction::AppCredentialDelete`) and its access rule, which
   Keystone keeps per user after the credential is gone
   (`DELETE /v3/users/{user_id}/access_rules/{id}`; a new `OsAction`, with a dry-run test). The
   short `expires_at` covers a closed tab.
4. Dry-run: the ticket preview lists the credential request, the token request and the
   eventual deletes, and returns no token (as today).
5. Update AGENTS.md §1.3 (the token handed to the page becomes the restricted one) and the
   `direct` paragraph of `CONFIGURATION.md`.
6. Tests: the access rule names exactly this image's path; the secret and the token never
   reach `Debug` or logs; the delete runs on every way the upload can end.

Open questions:

- **Fallback.** Keystone may forbid users from creating application credentials (policy), or
  Glance's keystonemiddleware may not enforce access rules (it needs `service_type = image`
  in `[keystone_authtoken]`; tokens with access rules are then refused rather than let
  through). Choose between failing the upload with a clear message and falling back to the
  session token, perhaps as a setting (`[images] direct_token = restricted | session`). This
  is the user's decision.
- Whether the test cloud's Keystone and Glance support it: check once its Glance has CORS for
  `direct` (item 45).
- Cost: three or four extra Keystone calls per upload, which is negligible next to a multi-GB
  `PUT`.

## 96. Tab bars show a pointless scroll control

> ✅ **Done.** Cause confirmed in Brave on *Project › Network › Firewall*: `.tabs` had
> `overflow-x: auto`, so `overflow-y` computed to `auto`, and the tabs' `margin-bottom: -1px`
> made the content 42px tall in a 41px box (a 1px vertical scroll). `style/main.css` now gives
> `.tabs` `flex-wrap: wrap` and no `overflow`, as Horizon does (comment at the rule): the
> `-1px` underline overlap stays, but there is no scroll box. Checked with
> `getComputedStyle` (`overflow-y: visible`) on Firewall (link tabs), a network detail and
> Admin › System Information (button tabs) at 1502px, and at 360px phone width in dark and
> light theme: the Firewall tabs wrap to two rows, no horizontal page scroll. Not checked: an
> instance detail (the test project has no instances), but it uses the same class.

**Goal.** Every tab bar (`.tabs`) shows a small scroll control even when all its tabs fit, as on
*Project › Network › Firewall* (*Firewall Groups* / *Firewall Policies* / *Firewall Rules*).
Scrolling it moves the tabs by about a pixel, so it does nothing useful and looks broken. The
bar should show no scrollbar while the tabs fit, and on a narrow window still let the user
reach every tab (sideways scroll, or wrapping), with no vertical scroll ever. Horizon's tabs
(Bootstrap `nav-tabs`) wrap and never scroll.

**Implementation.** One rule fixes all of them, since every tab bar uses the same class:
the link tabs in `pages/firewall.rs` (`tab_bar`, its `TABS`), the button tabs of
`pages/details.rs::Tabs` (detail pages: volumes, networks, routers, load balancers, Admin
System Information and Defaults…) and the instance tabs in `pages/instance_detail.rs`.

- Likely cause, to confirm in the browser: `style/main.css` gives `.tabs` `overflow-x: auto`,
  which makes `overflow-y` compute to `auto` too, and its `button` / `a` children have
  `margin-bottom: -1px` so their active underline covers the bar's `border-bottom`. Each child
  then sticks out 1px below the scroll box and the bar gets a 1px vertical scroll (the
  control the user sees, with scrollbars set to always show).
- Fix: draw the bar's line without overflowing, e.g. drop `border-bottom` and the `-1px`
  margins and paint the line with `box-shadow: inset 0 -1px 0 var(--line-2)` on `.tabs`
  (the active tab's 2px border then sits over it), or add `overflow-y: hidden`. Decide
  between keeping the sideways scroll on narrow windows (hide its scrollbar with
  `scrollbar-width: thin` or none) and `flex-wrap: wrap` as Horizon does.
- Check in Brave with the macOS *Show scroll bars: Always* setting and at phone width, in
  light and dark theme, on Firewall, a volume detail and an instance detail.

## 97. Inline documentation: doc comments on every module and function, rustdoc clean

**Goal.** A contributor (or a fresh session) can read any module or function and learn what it
is for, its contract and its gotchas from its doc comment, without reverse-engineering the code.
Most of the code already has good `///` comments (they explain *why*: Neutron quirks, Horizon
behaviour, roadmap numbers), but coverage is uneven, and nothing keeps it from slipping.
Improve what is there, fill the gaps that matter, and add a lint so new code keeps up.

Measured on 2026-11-06 with `cargo clippy --features ssr --lib -- -W missing_docs
-W clippy::missing_docs_in_private_items` (the `ssr` build only; `hydrate`-only code is not
counted): 2,338 items lack a doc comment. 1,745 are struct fields and enum variants, mostly the
self-explanatory row fields in `model.rs` (1,506 of the total), which are not worth a comment
each. The rest is what matters: 269 functions, 13 methods, 22 constants, 9 modules, 8 structs,
6 enums and 8 type aliases. Functions and types by file: `server/mapping.rs` 33,
`components/ops.rs` 33, `pages/admin.rs` 32, `server/fetch.rs` 23, `model.rs` 18,
`pages/details.rs` 14, `pages/identity.rs` 13, `server/actions.rs` 11, then 10 or fewer each.
Files with no `//!` module comment: `main.rs`, `app.rs`, `components/mod.rs`,
`components/widgets.rs`, `pages/mod.rs`, `pages/login.rs`, `pages/instances.rs`,
`pages/instance_detail.rs`, `pages/overview.rs`, `server/config.rs`.

**Implementation.**

1. Module comments (`//!`) for the ten files above: what the module holds and how it fits
   (AGENTS.md describes the layers; link to it rather than repeat it).
2. Doc comments for every function, method, type, constant and enum, worst files first.
   Follow the house style: one summary line, then the contract and the *why* (OpenStack
   behaviour, Horizon parity, safety rule), with the roadmap item when it explains the
   design. Don't restate the signature. Name the `OsAction` and the endpoint a server fn
   reaches. Mark panics and the `ssr` / `hydrate` split where it matters.
3. Struct fields and enum variants: only where the name doesn't say it all (units, `None`
   meaning, Neutron's `null` as *any*, IDs vs names). Leave plain row fields alone.
4. Review the existing comments for drift while there: comments that cite a removed field or
   behaviour (e.g. after item 93 dropped `shared` from the firewall actions) or a renamed
   function.
5. Make rustdoc clean: `cargo doc --no-deps --features ssr` with
   `RUSTDOCFLAGS="-D warnings"` (broken intra-doc links such as `` [`Field::new_line`] ``,
   bare URLs, invalid code blocks). It could not run in this session (permission denied
   writing the doc fingerprint under `target/`), so its warning count is still unknown;
   check it first. Also run it for `hydrate`.
6. Keep it from slipping: in `Cargo.toml` `[lints]`, `rust.missing_docs = "warn"` and
   `clippy.missing_docs_in_private_items` for functions only if clippy can scope it
   (it can't per item kind, so probably keep it off and rely on review), and add the rustdoc
   check to AGENTS.md's verify commands (and CI if there is one).
7. Split into slices by layer to keep reviews small: `server/` first (it carries the safety
   rules), then `components/`, then `pages/`, then `model.rs` and the rest.

**Open questions.** Whether to enforce with a lint (`missing_docs` only covers the public API,
and almost everything here is `pub` inside one crate, so it would also demand the 1,745 fields
and variants) or by review only. Whether doc comments may stay in English while the UI is
bilingual (they should: code is English throughout).

## 98. Subtle CSS motion on tabs, nav bars, menus and dialogs

> ✅ **Done** (2026-11-10), CSS only (`style/main.css`). Tokens on `:root`: `--motion-fast` 120ms,
> `--motion` 160ms, `--ease` `cubic-bezier(.2, 0, 0, 1)`, all `0ms` in one global
> `prefers-reduced-motion: reduce` block. A *Motion* section at the end of the file: colour
> transitions on tabs and the secondary bar (`color`, `border-bottom-color`: the underline
> fades in place), the primary bar, the dashboards and the utility menus (`background-color`,
> `color`) and `.btn` (`background-color`, `border-color`, `color`); one-shot `motion-drop`
> (opacity and 4px `translateY`) on `details[open] .menu-list` (utility and row menus) and
> `.combo-pop`; `motion-fade` on `.modal-backdrop` and `motion-rise` (opacity, `scale(.98)`) on
> `.modal`, on open only. The busy bar animates `transform: translateX(-100% → 250%)` instead
> of `left` (the same path; no layout per frame). Checked in Brave: the computed transitions
> and the animations on a row menu and *Create Volume* (160ms each), every duration `0s` and
> no animation under emulated reduced motion, the busy bar's keyframes `transform` only.
> Left out on purpose: the tab pane fade (step 4) and the wizard step fade. Without a wrapper
> per pane, `.tabs + *` would replay on every refetch of a polled detail page (item 46), and
> giving every `Tabs` user a `tab-pane` wrapper wasn't worth it for this. No DevTools
> *Performance* trace was recorded; the keyframes animate only `opacity` and `transform`, and
> the transitions only colours.

**Goal.** Switching a tab, a panel group (primary bar) or a panel (secondary bar), opening a
menu or a dialog changes state instantly today: the active underline or background just jumps.
A short, quiet transition makes the change easier to follow without slowing anyone down.
Moonlight is a tool for operators, so the motion must stay barely noticeable: 100 to 160 ms,
no bounce, no slide-ins across the page, nothing that delays a click or a keyboard user.
Horizon has none of this (Bootstrap's own fades aside); this is polish, not parity.

Hard limit: the animations must never load the browser. Only `opacity`, `transform` and colours
are animated (compositor or paint only, never `width`, `height`, `left`, `top`, `padding` or
`margin`, which re-run layout); no new infinite animations; no `transition: all`; no permanent
`will-change`; nothing on table rows (lists reach hundreds of rows, item 88); and everything
off under `prefers-reduced-motion: reduce`.

**Implementation.** CSS only, in `style/main.css`; Rust only where an element needs a class.

1. Motion tokens on `:root` next to the colour tokens: `--motion-fast: 120ms`,
   `--motion: 160ms`, `--ease: cubic-bezier(.2, 0, 0, 1)`; one global
   `@media (prefers-reduced-motion: reduce)` block that sets them to `0ms`, so new rules
   inherit the opt-out (the per-rule `reduced-motion` blocks that exist today can stay).
2. Tabs (`.tabs button`, `.tabs a`, `Tabs` in `pages/details.rs`) and the secondary bar
   (`.secondary-bar .panel`): transition `color` and `border-bottom-color`. The underline
   fades in place rather than sliding from the old tab: a sliding indicator needs a measured
   position (JS, layout reads) and that is the kind of cost to avoid.
3. Primary bar (`.primary-bar .group`, `PrimaryNav` in `components/layout.rs`) and the
   utility bar's `.dashboards > *` and `.menu > summary`: transition `background-color` and
   `color` on hover and active.
4. Tab content: when the active tab changes, fade the new pane in (`opacity` 0 → 1 with a
   2–4px `translateY`, `--motion`) with a one-shot `@keyframes` on the pane's wrapper. Find
   where each `Tabs` user renders its pane (details pages, instance tabs, volume tabs) and give
   the wrapper a `tab-pane` class; the animation replays because Leptos inserts a new node.
   Check it does not replay on every refetch of a polled detail (item 46): only when the pane
   node is replaced.
5. Menus (`.menu-list` under `details.menu[open]` and `.row-menu`, and the custom dropdown in
   `components/select.rs`): one-shot fade plus 4px `translateY` on open. No close animation
   (the native `<details>` removes the list at once; faking it needs JS).
6. Dialogs (`.modal-backdrop`, `.modal` in `components/dialog.rs`): backdrop fades in, the
   dialog fades and scales from `.98`, on open only. The wizard's step change (`.wizard-pane`)
   can reuse the `tab-pane` fade.
7. Buttons (`.btn`): `background-color`, `border-color` and `color` on hover, `--motion-fast`.
8. While there, the busy bar (`.busy-bar`, item 11) animates `left`, which triggers layout on
   every frame for every busy row: switch it to `transform: translateX(…)` with the same look.
9. Check: Chrome DevTools *Performance* trace of switching tabs and opening a menu and a dialog
   shows no *Layout* or *Recalculate Style* caused by the animations beyond the class change,
   and *Rendering › Paint flashing* only flashes the changed control. Test light and dark
   (item 18), with `prefers-reduced-motion` emulated, and on a list with many rows.

**Open questions.** Whether the tab pane fade (4) and the dialog scale (6) are wanted at all,
or only the colour transitions on the controls themselves; decide by looking at them in the
browser.

## 99. 🐞 FIX: the VNC console doesn't fit its frame; the last lines are cut off

> ✅ **Done** (2026-11-10). ttcp-dev serves noVNC's `vnc_lite.html`, the token inside its `path`
> parameter. The *Console* tab's frame loads `RemoteConsole::frame_url()` (`model.rs`): Nova's
> URL with `scale=true&resize=scale` added for a noVNC console (`kind` `novnc` or `protocol`
> `vnc`), every existing parameter kept, one already set not repeated, a fragment kept last,
> SPICE untouched (test `the_frame_asks_novnc_to_scale_and_keeps_the_token` in
> `server/console.rs`). *Open the console alone* keeps Nova's URL as it is. `.console-frame` is
> `max(360px, calc(100dvh - 370px))` tall with `resize: vertical`: on a 1502 × 758 window the
> page is exactly the window's height (it was 27px taller with a 340px offset, and the whole
> frame ran past the bottom before), and the guest's 1024 × 768 screen shows whole, scaled
> into the 388px frame. At 390px wide (emulated phone) it fits the frame's width with no
> horizontal scroll; the page still scrolls there because of the tall header (item 101).
> Keyboard input not typed into (an instance at its login prompt, in a shared project); the
> change doesn't touch it. A *Fit / 1:1* toggle in `.console-bar` (`.console-zoom`, noVNC only,
> since SPICE can't scale): *1:1* loads Nova's URL as it is (native size, sharp text, scrolling
> inside the frame), *Fit* the scaled one; the choice is kept in this browser's
> `localStorage` (`moonlight.console.fit`, best effort; *Fit* by default). A new `src` reloads
> noVNC, which reconnects with the same token: checked both ways on ttcp-dev ("Connected to
> QEMU" each time).

**Goal.** The instance *Console* tab (item 31) shows noVNC in an iframe, but the remote screen
is not scaled to the frame: the bottom rows of the console (the prompt you are typing at) fall
below the frame's edge. Today the only ways to work with it are scrolling inside the frame
(which hides the top while you read the bottom) or *open alone*, a full browser tab.
The console in the tab should be usable as it is: the whole remote screen visible inside the
frame, scaled down when the frame is smaller than the guest's resolution, at any window size,
and following the window when it is resized, with no scrollbar inside the frame. *Open alone*
stays for those who want it bigger.

**Implementation.** Where it is now: `ConsoleTab` in `pages/instance_detail.rs` renders
`<iframe class="console-frame" src=c.url>` (`c: RemoteConsole`, `model.rs`, from
`server/console.rs`); `.console-frame` in `style/main.css` is `width: 100%;
height: min(75vh, 820px)`. The frame is another origin (Nova's noVNC proxy), so Moonlight
cannot reach inside it; the fix goes through noVNC's own URL options and the frame's size.

1. Ask noVNC to scale. noVNC reads options from the query string: `vnc_lite.html` (and the
   old `vnc_auto.html`) take `scale=true` (`rfb.scaleViewport`); `vnc.html` takes
   `resize=scale` (and `resize=remote` would ask the guest to change its resolution: don't,
   it changes the instance). Add a pure `fn console_frame_url(url: &str) -> String` next to
   `remote_console` in `server/console.rs` (or in the page) that keeps every existing query
   parameter (the `token` above all) and appends `scale=true&resize=scale` for a noVNC URL
   (`protocol` `vnc`, `type` `novnc`); unknown options are ignored by both pages. Leave SPICE
   (`spice_auto.html` has no such option) unchanged. Use it for the iframe only; *open alone*
   keeps Nova's URL as it is (or also scaled: decide when testing). Tests: token kept, a URL
   with and without `?`, an existing `scale`/`resize` not duplicated, SPICE untouched.
2. Size the frame to the space left, not a fixed cap: `height: calc(100dvh - <header, tabs and
   console bar>)` with a sensible `min-height` (≈ 360px), so scaling has room and the page
   itself does not scroll to reach the console. Optionally `resize: vertical` so the user can
   enlarge it. noVNC rescales on its own window's resize, so the frame following the window is
   enough.
3. Check first on the test cloud which page `novncproxy_base_url` serves (`vnc_lite.html`,
   `vnc_auto.html` or `vnc.html`) and the noVNC version, then verify in the browser: the full
   screen visible in the tab at a laptop-size window and a narrow one, text still readable,
   keyboard input works after a click, *open alone* unchanged. Opening the tab creates a
   console token, so it only works with `[console] enabled = true` and `[writes] mode = on`
   (item 31): do this on the test cloud only.

**Open questions.** Whether an old noVNC on the cloud honours `scale` (if not, the fallback is
only the larger frame from step 2). Whether to offer a *Fit / 1:1* toggle in `.console-bar` for
small fonts that become hard to read when scaled down.

## 100. Network Topology: a note instead of the drawing when the project sees too many networks

**Goal.** *Project › Network › Network Topology* (item 20) draws every network the project can
see (its own, shared and external ones, as on the Networks panel) with their routers and
instances. With many networks the *Topology* columns and the *Graph* become unreadable, slow to
lay out and heavy for the browser (one SVG node per network, device and link). Above a limit
set by the operator, both tabs show a note instead of the drawing: the project sees *N*
networks, more than the topology draws (the limit), with a link to the *Networks* panel.
Under the limit nothing changes. Horizon has no such limit (it just draws everything).

**Implementation.**

1. Setting: `[network] topology_max_networks = 50` (`0` = no limit) in `NetworkPrefs`
   (`src/prefs.rs`, default in its `Default`, a defaults test), documented in
   `moonlight.conf.example` and `moonlight.conf` next to `fip_topology_check`, and in
   `server/settings.rs` if it lists the keys. It reaches the page through `dialog::prefs()`
   like the other `[network]` options.
2. What counts: the networks the drawing would show, `Topology.networks` as
   `mapping::topology` builds it from `fetch::topology` (`server/fetch.rs`, own + shared +
   external from `net_lookups`). The session is scoped to one region, so this is already the
   count per region.
3. Server side, so a big project costs neither the client nor the cloud: in
   `fetch::topology`, run `net_lookups` first; when the count is over the limit, skip the
   router, port, server and floating IP GETs and return a `Topology` with no devices or links
   and a new `too_many: Option<usize>` (the count; `None` when drawn). The price is one
   sequential round trip before the others for every project; if that shows in the timing
   chip, keep the five GETs in parallel and only drop devices and links from the reply.
   Decide by measuring.
4. Page: in `TopoBody` (`pages/topology.rs`), when `too_many` is set, render a
   `<p class="muted topo-empty">` note (new `K::TopologyTooMany` with the count and the limit,
   EN + ES) and a link to `/project/networks` instead of `ColumnsView` / `GraphView`. Disable
   the drawing toolbar buttons (size, labels, collapse, centre) while the note is shown.
5. Tests: a pure `fn over_limit(count, max) -> bool` (0 = never; at the limit is still drawn)
   and the mapping/fetch path that leaves devices and links empty; i18n keys in both languages.
   In the browser: set the limit below the test project's network count in `moonlight.conf`
   and check both tabs show the note, then restore it.

**Open questions.** Whether the *List* view (`TopoList`, a table, cheap to render) should stay
available over the limit (then step 3 must keep fetching devices and links, and only the two
drawings are replaced). Whether shared and external networks count towards the limit or only
the project's own (the request says every network the project sees; external ones are usually
few, shared ones may be many).

## 101. Phone view: fold the header's bars and menus into one or two buttons

**Goal.** On a phone the header takes most of the screen. Below 720px the utility bar wraps
(brand, region, project and user menus on one row, *Project / Admin / Identity* on another),
then come the primary bar (panel groups: *Compute*, *Volumes*, *Network*…) and the secondary
bar (the group's panels), and `.header` is `position: sticky`, so all of it stays on screen
while scrolling. At phone width it should fold into one slim bar: the brand, the current place
(e.g. *Compute › Instances*), and one or two buttons that open the rest:

- **☰ Navigation**: the dashboards (*Project*, *Admin* for admins, *Identity*), and under the
  current one its groups and panels as a two-level list, the current panel marked. Picking a
  panel navigates and closes it.
- **Account / context** (the project name or the user icon): *Region*, *Project* (with its
  search), and what the user menu holds today (*Language*, *Theme*, *Help*, *Change Password*,
  *Sign Out*).

Wider screens are unchanged. Horizon's Bootstrap header collapses behind a toggle on narrow
screens too (check how its side menu behaves before copying it). The read-only / dry-run badge,
hidden today below 720px (`.badge-ro { display: none; }`), must stay visible in the slim bar
(the eye icon alone): it is a safety signal, not decoration.

**Implementation.** Where the header is: `Layout`, `PrimaryNav`, `RegionMenu`, `ProjectMenu`,
`UserMenu` in `src/components/layout.rs`; navigation data in `src/components/nav.rs`
(`dashboard(path)`, `active(path)`, `Group::visible(&caps)`); CSS in `style/main.css`
(`.utility-bar`, `.dashboards`, `.primary-bar`, `.secondary-bar`, the
`@media (max-width: 720px)` block).

1. Decide the layout by CSS, not by JS. The server does not know the screen width, so a
   width signal would render one header on the server and another after hydration (a
   hydration mismatch, AGENTS.md). Render both: the current bars and a new
   `<div class="mobile-bar">`; the media query shows one and hides the other (`display: none`
   keeps the hidden copy out of the accessibility tree and tab order).
2. Menus as native `<details class="menu">`, like the others, so they work before hydration
   and get the existing outside-click and Escape handling (`install_menu_dismissal` in
   `components/dialog.rs`). The ☰ panel is a full-width sheet under the bar
   (`position: fixed`, `max-height` of the viewport, scrolling inside), not a dropdown.
3. Reuse, don't copy: build the ☰ list from the same `nav::dashboard` / `visible(&caps)` data
   `PrimaryNav` uses (from `info`, in a `<Transition>`, so SSR and client agree, as item 85
   learned), and extract the menus' bodies (`RegionMenu`'s list, `ProjectMenu`'s search and
   list, `UserMenu`'s items, `ThemeChoices`, `LanguageChoices`) into components used by both
   headers. Close the sheet on navigation (watch `location.pathname`).
4. The slim bar's text: the project name (from `info`, as `ProjectMenu` shows it) and the
   current place, `nav::active(path)`'s group and panel labels (detail pages fall back to the
   group). Two short lines, or one line `project · Compute › Instances`, each part with its
   own ellipsis so a long project name can't push out the panel.
5. Sticky: only the slim bar stays sticky at phone width (≈44px; measure today's full
   header height first, to report the gain).
6. i18n: `K::Navigation`, `K::Account` (or reuse existing keys) for the buttons'
   `aria-label`s, EN + ES.
7. Verify in the browser at 390×844 and 360×740 (Chrome DevTools device emulation) and at
   721px and up (unchanged): every dashboard, group and panel reachable, the project switch
   and sign-out work, the dry-run eye shows, no horizontal scroll, keyboard and screen reader
   reach both buttons, light and dark. Also check a page that already has its own tab bar
   (item 96) and the jobs tray (item 87) at phone width.

**Decided with the user (2026-11-10).** Two buttons for now (☰ *Navigation* and *Account*),
the fold at the existing 720px breakpoint, and the slim bar shows the project name as well as
the current place.

## Also pending (from the PoC)

- Validate `MOONLIGHT_WRITES=on` end to end on DevStack or a lab project.
- OIDC login (Keystone WebSSO, Horizon's `WEBSSO_*` settings): item 80. Token renewal: item 77.
- The login page's `<Title text=…>` closure (`pages/login.rs`) sometimes panics during SSR
  with `the sandboxed-arenas feature is active, but no Arena is active` (reactive_graph 0.2.16,
  backtrace through `I18n::t`): it runs after its owner is gone. Seen twice per dev run while
  testing item 80; sign-in unaffected. Likely the same disposed-signal pattern as AGENTS.md §6
  (`try_get`).
- Identity on clouds that enforce Keystone system scope (`[oslo_policy] enforce_scope = true`):
  user and role admin calls need a system-scoped token, so a second, system-scoped connection in
  `os.rs` (from item 57; ttcp-dev doesn't enforce it).
- Admin dashboard: item 85.
- `scripts/dev.sh status` said `stopped` while the `cargo leptos watch` started by `restart` was
  still compiling (2026-10-31, after `target/` was deleted); `stop` killed it. The status check
  should find the watch process during the first build too.
- Disk: `target/` reached 35 GB on 2026-10-31 (19 GB free); per AGENTS.md, delete the
  `incremental` directories first when space runs low.
- Paging for large projects (from item 27): item 88 does it for the Admin lists; Project panels
  can reuse it once a project nears ~1000 rows (in October 2026 the largest had 31, 0.3 s).
- `[server] base_path` (Horizon's `WEBROOT`): serve Moonlight under a sub-path such as
  `/dashboard` (routes, `/pkg` assets, server function URLs, cookie path).
- Several Keystones selectable at login (Horizon's `AVAILABLE_REGIONS`): login form, session and
  the connection cache keyed by Keystone URL.
- Instance resize, rebuild, migrate. (Console and log: item 31; image upload: item 45.)
- Container image and OpenShift deployment; optional shared cache.
