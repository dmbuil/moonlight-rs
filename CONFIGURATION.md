# Moonlight configuration

Moonlight reads TOML files named `moonlight.conf`.
[`moonlight.conf.example`](moonlight.conf.example) lists every key with its default and the
Horizon setting it replaces. Only `[keystone] url` is required: a missing key, or a missing
section, always gets the default shown below.

## Where settings come from

Files are layered the way oslo.config layers Nova's and Neutron's; later values win, key by key.
Lowest to highest:

1. Built-in defaults.
2. The files: each `--config-file PATH` in command-line order, or else the first that exists
   of `~/.moonlight/moonlight.conf`, `~/moonlight.conf`, `/etc/moonlight/moonlight.conf`,
   `/etc/moonlight.conf`.
3. Every `*.conf` in each `--config-dir DIR`, or else in the first `moonlight.conf.d/` found
   in the same places, sorted by name (`10-base.conf` before `20-site.conf`).
4. The environment variables of the first table (containers, secrets). Empty ones count as
   unset.

The start-up log lists the files read, in order; the cookie key is never logged. Unknown keys
and invalid values stop the server and name the file that caused them.

For development, keep a git-ignored `moonlight.conf` in the repository and start the server
with it:

```bash
cp moonlight.conf.example moonlight.conf
cargo leptos watch -- --config-file moonlight.conf
```

## Core settings

These can also be set with an environment variable, which overrides every file.

| Key | Variable | Default | Meaning |
|---|---|---|---|
| `[keystone] url` | `MOONLIGHT_KEYSTONE_URL` | — (required) | Keystone public endpoint, **without** `/v3` (the SDK runs version discovery). |
| `[keystone] default_domain` | `MOONLIGHT_DEFAULT_DOMAIN` | `Default` | Pre-filled domain on the login form. |
| `[keystone] default_region` | `MOONLIGHT_DEFAULT_REGION` | first region in the catalog | Region selected after login. |
| `[keystone] multidomain` | `MOONLIGHT_MULTIDOMAIN` | `false` | Several Keystone domains in use (Horizon's `OPENSTACK_KEYSTONE_MULTIDOMAIN_SUPPORT`): admins get Identity › Domains and *Set Domain Context*, which scopes the Projects, Users and Groups lists to that domain. |
| `[session] cookie_key` | `MOONLIGHT_COOKIE_KEY` | random per start-up | Base64, ≥ 64 bytes. Encrypts the session cookie. If unset, sessions end on every restart. |
| `[session] secure_cookies` | `MOONLIGHT_SECURE_COOKIES` | `true` | `Secure` session cookie named `__Host-ml_session`, plus HSTS. Set `false` only for plain-HTTP development. |
| `[session] public_url` | `MOONLIGHT_PUBLIC_URL` | unset (the request's `Host`) | The URL users open (`https://cloud.example.com`). Writes (every request but `GET`, `HEAD` and `OPTIONS`) must come from this origin: browsers that send `Sec-Fetch-Site` are checked with it, others by comparing `Origin` with this URL, or with `Host` when unset. Set it when a reverse proxy rewrites `Host`. |
| `[writes] mode` | `MOONLIGHT_WRITES` | `dry-run` | `off`, `dry-run` or `on` (see the README, *Writes*). |
| `[ui] help_url` | `MOONLIGHT_HELP_URL` | unset (hidden) | *Help* link in the user menu (http(s) only). |
| `[ui] change_password_url` | `MOONLIGHT_CHANGE_PASSWORD_URL` | unset (hidden) | *Change Password* link in the user menu. |
| `[plugins] enabled` | `MOONLIGHT_PLUGINS` (comma-separated) | none | Opt-in panels: `load-balancer` (Octavia), `orchestration` (Heat), `qos` (Network QoS, and the QoS policy of router gateways and floating IPs) `trunk` (Trunks), `key-manager` (Barbican's Secrets and Containers) `alarming` (Aodh: Monitoring › Alarms), `firewall` (FWaaS v2: Network › Firewall, where Neutron has the `fwaas_v2` extension) and `backup` (Cinder: Volumes › Backups, and *Create Volume Backup* in a volume's menu; enable it only where `cinder-backup` runs, since the catalog can't tell). Each shows only where the cloud offers it (the service in the catalog, or the Neutron extension); its server functions refuse calls otherwise. |
| `[log] filter` | `RUST_LOG` | `moonlight=info,warn` | Log filter (`tracing` syntax). |
| `[log] file` | `MOONLIGHT_LOG_FILE` | unset (stdout only) | Write the log to this file instead of stdout, e.g. `/var/log/moonlight/moonlight.log`. The directory must exist and be writable, or Moonlight does not start. Files are created `0640` at most: they name users and client addresses (never tokens). Rotation and format: `[log] rotation`, `keep`, `stdout`, `format` below. |
| `[log] trusted_proxies` | `MOONLIGHT_TRUSTED_PROXIES` | `1` | Proxies in front of Moonlight that append to `[log] client_ip_header`. The client is the entry this many places from the right (the leftmost when the list is shorter): entries further left were sent by the client and can be forged. `2` for a CDN plus a balancer; `0` takes the leftmost entry, which a client can set to dodge the sign-in throttle. |

## Settings after Horizon's `local_settings.py`

File only, no environment variable. The last column names the Horizon equivalent.

### Connection and session

| Key | Default | Meaning | Horizon |
|---|---|---|---|
| `[keystone] interface` | `public` | Catalog interface for every service: `public`, `internal` or `admin`. | `OPENSTACK_ENDPOINT_TYPE` |
| `[tls] ca_file` | unset (system store) | PEM bundle for a cloud signed by a private CA. | `OPENSTACK_SSL_CACERT` |
| `[tls] verify` | `true` | `false` disables certificate checks: test clouds only. | `OPENSTACK_SSL_NO_VERIFY` |
| `[session] revoke_on_logout` | `true` | Keystone revokes the token on logout (`DELETE /v3/auth/tokens`, sent only with `[writes] mode = "on"`). The server forgets the token in every mode. | `TOKEN_DELETION_DISABLED` (inverted) |
| `[session] timeout_minutes` | `60` | A sign-in lasts at most this long, even if the token lives longer. | `SESSION_TIMEOUT` |
| `[session] token_margin_seconds` | `60` | A token this close to expiring counts as expired. | `TOKEN_TIMEOUT_MARGIN` |
| `[session] password_expiry_warning_days` | unset (off) | Banner this many days before the password expires. | `PASSWORD_EXPIRES_WARNING_THRESHOLD_DAYS` |
| `[cache] lookup_seconds` | `30` | How long the flavors, images, networks and volumes a page fetches only to show names are reused, per token, project and region. A write through Moonlight clears them at once on the replica that served it; changes made elsewhere, or through another replica without session affinity, show within this time. `0` = off. | — |

### Login page

| Key | Default | Meaning | Horizon |
|---|---|---|---|
| `[login] domain_choices` | `[]` (free text) | Domains offered as a dropdown; the server refuses any other. | `OPENSTACK_KEYSTONE_DOMAIN_CHOICES` |
| `[login] domain_hidden` | `false` | No domain field: everyone signs in to `[keystone] default_domain`. Not with `domain_choices`. | `OPENSTACK_KEYSTONE_MULTIDOMAIN_SUPPORT` |
| `[login] password_autocomplete` | `false` | Let the browser remember the password. | `HORIZON_CONFIG["password_autocomplete"]` |
| `[login] password_reveal` | `true` | Show the button that reveals the password. | `HORIZON_CONFIG["disable_password_reveal"]` |
| `[login] max_failures` | `5` | Failed sign-ins per username (and per address, with `[log] client_ip_header`) before Moonlight stops asking Keystone. `0` = off. Counted per process. | — (Keystone's `lockout_failure_attempts`) |
| `[login] lockout_seconds` | `300` | How long that lasts, from the last failure. | — |
| `[login] message_file` | unset | Text file shown above the form, e.g. a maintenance notice. | `MESSAGES_PATH` |

### WebSSO (federated sign-in)

Sign-in through the identity provider Keystone federates with (OIDC, SAML), as Horizon's
WebSSO (roadmap 80). Moonlight has no Identity › Federation panels: identity providers,
mappings and protocols are managed in Keystone directly. Users who signed in this way get a
`v3websso` `clouds.yaml` on API Access (`[api_access] websso_clouds_yaml_template`); its
`openrc` still needs a Keystone password, which the page tells them, pointing to application
credentials.

With `enabled`, a federated choice sends the browser to
`{keystone_url}/v3/auth/OS-FEDERATION/websso/{id}?origin=…` (or, with `idp_mapping`,
`…/identity_providers/{idp}/protocols/{protocol}/websso?origin=…`). `origin` is
`[session] public_url`'s origin followed by `/auth/websso` (e.g.
`https://cloud.example.com/auth/websso`), so `public_url` is required, and Keystone's
`[federation] trusted_dashboard` must list that exact URL.

Keystone sends the token back with a cross-site POST, which Moonlight accepts on that one path
only from a browser that started the sign-in there: the redirect sets a one-use, encrypted
`__Host-ml_sso` cookie (`SameSite=None; Secure`, 10 minutes). Signing in at the identity provider
must therefore finish within 10 minutes, in the same browser. Browsers accept that `Secure`
cookie on `http://localhost` and `http://127.0.0.1`, so plain-HTTP development works.

| Key | Default | Meaning | Horizon |
|---|---|---|---|
| `[websso] enabled` | `false` | An *Authenticate using* select on the login page. | `WEBSSO_ENABLED` |
| `[websso] choices` | `[]` | Its options, in order: `{ id, label }`. `credentials` is the password form; any other ID (letters, digits, `.`, `_`, `-`) is a Keystone federation protocol or a key of `idp_mapping`. At least one besides `credentials`. An empty `label` shows the ID, or *Keystone Credentials* (translated) for `credentials`. | `WEBSSO_CHOICES` |
| `[websso] initial_choice` | the first choice | Preselected choice. | `WEBSSO_INITIAL_CHOICE` |
| `[websso] idp_mapping` | `{}` | Choice ID → `{ idp, protocol }`, for Keystone's per-provider WebSSO URL. | `WEBSSO_IDP_MAPPING` |
| `[websso] keystone_url` | `[keystone] url` | Keystone as the browser reaches it (with or without `/v3`). | `WEBSSO_KEYSTONE_URL` |
| `[websso] logout_url` | unset (`/login`) | Where logout sends a user who signed in through WebSSO, after Moonlight's own logout. Moonlight can't delete the identity provider's cookies (another site), so without it the next WebSSO sign-in goes through without a password. With OIDC through Keystone's `mod_auth_openidc`, use its redirect URI with `?logout=<URL-encoded return URL>`: it ends its own session and the provider's (its `end_session_endpoint`, e.g. ADFS's `/adfs/oauth2/logout`). | `WEBSSO_DEFAULT_REDIRECT_LOGOUT` |

### Interface

| Key | Default | Meaning | Horizon |
|---|---|---|---|
| `[ui] bug_url` | unset (hidden) | *Report a Bug* link in the user menu. | `HORIZON_CONFIG["bug_url"]` |
| `[ui] default_theme` | `system` | `system`, `light` or `dark`, until the user picks one. | `DEFAULT_THEME` |
| `[ui] default_language` | the browser's | `en` or `es`, until the user picks one. | — |
| `[ui] poll_interval_ms` | `5000` | Refresh of resources in transition (building, resizing…), at least 1000. | `HORIZON_CONFIG["ajax_poll_interval"]` |
| `[ui] show_error_details` | `false` | Send OpenStack's full error text, which names internal endpoints, to the browser. Off: kind, service and request ID only; the log keeps the full text. | — |
| `[ui] show_requests` | `false` | Show the REST requests an action sent (method, path, body; secrets masked) in its result dialog, the instance *Log*/*Console* tabs and *Preview Stack*, and in dry-run the requests it would send. Off: they never reach the browser; results show only *Done* or the errors. | — |
| `[ui] background_batch_from` | `10` | A bulk action with this many items or more runs in the background: the dialog closes and a card at the bottom right counts the items done (sent 5 at a time), lists failures and ends with a summary. Closing the tab stops it; the items not yet sent are not sent. `0` = every batch in its dialog. | — (Moonlight's own) |
| `[ui] page_size` | `100` | Rows per page of the paged lists (roadmap 88): the Admin lists load one page and filter and sort on the server, whatever the cloud's size. 1–999: a page asks for one row more than this, and the APIs cap a request at 1000. `0` = no paging, every row at once with a search box on the page. | `API_RESULT_PAGE_SIZE` (Horizon: 20) |
| `[ui] paged_lists` | `admin` | Which lists `page_size` applies to: `admin` (the big Admin lists) or `all` (the Project panels too). | — (Moonlight's own) |

### Launch Instance

Horizon's `LAUNCH_INSTANCE_DEFAULTS`, `OPENSTACK_HYPERVISOR_FEATURES` and
`CREATE_INSTANCE_FLAVOR_SORT`.

| Key | Default | Meaning |
|---|---|---|
| `[launch] create_volume` | `true` | Image sources boot from a new volume. |
| `[launch] config_drive` | `false` | *Configuration Drive* preselected. |
| `[launch] disable_image` | `false` | Hide images as a boot source. |
| `[launch] disable_instance_snapshot` | `false` | Hide instance snapshots as a boot source. |
| `[launch] disable_volume` | `false` | Hide volumes as a boot source. |
| `[launch] disable_volume_snapshot` | `false` | Hide volume snapshots as a boot source. At least one source must stay. |
| `[launch] requires_keypair` | `false` | A key pair must be chosen. |
| `[launch] can_set_password` | `false` | Offer an admin password field (masked in request previews). |
| `[launch] flavor_sort` | `ram` | Flavor order: `id`, `name`, `ram`, `disk` or `vcpus`. |
| `[launch] flavor_sort_reverse` | `false` | Reverse that order. |

### Network

Horizon's `OPENSTACK_NEUTRON_NETWORK` and `ALLOWED_PRIVATE_SUBNET_CIDR`.

| Key | Default | Meaning |
|---|---|---|
| `[network] default_dns_nameservers` | `[]` | Prefilled DNS servers when creating a subnet. |
| `[network] enable_ipv6` | `true` | Offer IPv6 when creating subnets. |
| `[network] fip_topology_check` | `true` | *Associate Floating IP* only offers ports that a router with an external gateway reaches. |
| `[network] topology_max_networks` | `50` | *Network Topology* draws its *Topology* and *Graph* tabs only up to this many networks (own, shared and external); above it they show a note instead, and the *List* view stays available. `0` = no limit. |
| `[network] supported_provider_types` | `["*"]` (every type) | Admin › *Create Network*'s provider types, in this order: `local`, `flat`, `vlan`, `gre`, `vxlan`, `geneve`. Checked by the server too. |
| `[network.segmentation_id_range] <type>` | the type's full range | `[min, max]` segmentation IDs admins may pick for that type, e.g. `vlan = [1000, 1999]`; must lie within the type's own range. |
| `[network.allowed_private_subnet_cidr] ipv4` | `[]` (anything) | IPv4 ranges new subnets must fall in; checked by the server too. |
| `[network.allowed_private_subnet_cidr] ipv6` | `[]` (anything) | The same for IPv6. |

### Admin dashboard

| Key | Default | Meaning | Horizon |
|---|---|---|---|
| `[admin] flavor_edit` | `false` | Admin › Flavors' *Edit Flavor*. Nova can't change a flavor: saving deletes it and creates it again with the same ID, then puts its extra specs and project access back (the values are checked first, so a refused one leaves the flavor alone). Running instances keep their size. | `ENABLE_FLAVOR_EDIT` |
| `[admin] filter_first` | `[]` | Paged Admin lists that show nothing, without asking the cloud, until a filter is given: any of `instances`, `volumes`, `snapshots`, `floating_ips`, `networks`, `routers`, `trunks`, `images`, `volume_groups`, `group_snapshots`. Applies while `[ui] page_size` pages the list (with `0` the whole list loads and is searched in the browser). | `FILTER_DATA_FIRST` |

### Images, console, API access and logs

| Key | Default | Meaning | Horizon |
|---|---|---|---|
| `[images] disk_formats` | `raw`, `qcow2`, `vdi`, `vhd`, `vmdk`, `iso`, `aki`, `ami`, `ari`, `docker`, `ploop` | Formats offered; *Upload to Image* keeps those Cinder supports. | `OPENSTACK_IMAGE_BACKEND["image_formats"]` |
| `[images] default_visibility` | `private` | Preselected visibility of new images: `private`, `shared`, `community`, `public`. | `CREATE_IMAGE_DEFAULTS["image_visibility"]` |
| `[images] allow_location` | `false` | Offers *URL* as an *Image Source* in *Create Image* (the button is always there): Glance downloads the image from an `http(s)` address (`web-download` import; the cloud must enable it). | `IMAGES_ALLOW_LOCATION` |
| `[images] upload` | `off` | Offers *File* as an *Image Source* in *Create Image*. `proxy`: the browser sends the file to Moonlight, which streams it to Glance without keeping any of it. `direct`: the browser sends it straight to Glance's endpoint with a scoped token for that request; Glance needs CORS for Moonlight's origin (`[cors] allowed_origin`, with `PUT` and `X-Auth-Token` allowed), and the page receives that token (never stored or logged; it is what the user's own session could do). Both are sent only with `[writes] mode = on`. | `HORIZON_IMAGES_UPLOAD_MODE` (`off`, `legacy`, `direct`) |
| `[images] max_upload_gb` | `20` | Largest file *File* accepts, in GiB; checked before the upload starts and again by the server. | — |
| `[images] reserved_properties` | `[]` | Image property keys that *Update Metadata* hides and that Moonlight refuses to write (on the server too), besides Glance's own `os_glance_*`, which are always reserved. The catalog picker leaves them out. | `IMAGE_RESERVED_CUSTOM_PROPERTIES` |
| `[console] enabled` | `false` | Instance *Log* and *Console* tabs. Both are Nova POSTs, sent only when this is on **and** `[writes] mode = on`; dry-run shows the request. | — |
| `[console] type` | `auto` | Remote console: `auto` (VNC, then SPICE), `vnc` or `spice`. | `CONSOLE_TYPE` |
| `[console] log_length` | `35` | Lines the *Log* tab shows first. | `INSTANCE_LOG_LENGTH` |
| `[key_manager] allow_payload` | `false` | *Show Payload* on a secret's page (`key-manager` plugin): the secret material reaches the browser, fetched on click only, never cached, and wiped from Moonlight's memory when dropped. | — |
| `[api_access] clouds_yaml_name` | `openstack` | Entry name in the downloaded `clouds.yaml`. | `OPENSTACK_CLOUDS_YAML_NAME` |
| `[api_access] clouds_yaml_profile` | unset | os-client-config vendor profile in `clouds.yaml`. | `OPENSTACK_CLOUDS_YAML_PROFILE` |
| `[api_access] openrc_template` | built-in | Path to a Jinja template for API Access's `openrc` (see below). | `OPENRC_CUSTOM_TEMPLATE` |
| `[api_access] clouds_yaml_template` | built-in | Path to a Jinja template for API Access's `clouds.yaml`. | `OPENSTACK_CLOUDS_YAML_CUSTOM_TEMPLATE` |
| `[api_access] app_credential_openrc_template` | built-in | Path to a Jinja template for a new application credential's `openrc`. | (fixed in Horizon) |
| `[api_access] app_credential_clouds_yaml_template` | built-in | Path to a Jinja template for a new application credential's `clouds.yaml`. | (fixed in Horizon) |
| `[api_access] websso_clouds_yaml_template` | built-in | Path to a Jinja template for API Access's `clouds.yaml` when the user signed in through WebSSO. The built-in one uses `auth_type: v3websso` (the `keystoneauth-websso` plugin signs in through the browser) with the sign-in's `identity_provider` (empty without `[websso] idp_mapping`) and `protocol`, which are also template variables. | — |
| `[log] client_ip_header` | unset | Header with the client's address behind a proxy, e.g. `X-Forwarded-For` or `X-Real-IP`; logged with sign-ins and writes, and counted by `[login] max_failures`. Which entry is used: `[log] trusted_proxies`. | `SECURE_PROXY_ADDR_HEADER` |
| `[log] rotation` | `daily` | With `[log] file`: `daily` or `hourly` start a new file named after the date (`moonlight.log.2026-10-29`, `…-13` hourly, UTC); `never` writes `moonlight.log` itself, for the system's `logrotate` (with `copytruncate`: Moonlight does not reopen the file on `SIGHUP`). | `LOGGING` handlers |
| `[log] keep` | `true` | On restart, append to the current log file (`true`, `yes`, `enabled`, `on`) or start it empty (`false`, `no`, `disabled`, `off`). A number is the old meaning of `keep` (files kept): it is read as `max_files`, with a warning at start-up. | — |
| `[log] max_files` | `7` | Log files kept, the current one included; older ones are deleted at start-up and then every hour, never the current one. `0` = all. With `never`, there is one file, so this only matters if old dated files are in the directory. | `backupCount` |
| `[log] stdout` | `false` | Also write to stdout when `[log] file` is set (e.g. while developing). | — |
| `[log] format` | `text` | `text`, or `json` (one object per line) for log shippers; applies to every output. | `LOGGING` formatters |

**Download templates.** The `openrc` and `clouds.yaml` files are rendered on the server from
Jinja templates ([minijinja](https://docs.rs/minijinja)); the built-in ones are in
`src/server/templates/`. A template from Horizon works unchanged: its variables have the same
names (`auth_url`, `tenant_id`, `tenant_name`, `project_id`, `project_name`, `user.username`,
`user.id`, `user_domain_name`, `project_domain_id`, `region`, `regions`, `interface`,
`cloud_name`, `profile`, `os_identity_api_version`, `os_auth_version`, and for application
credentials `application_credential_id`, `application_credential_name`,
`application_credential_secret`, and for WebSSO sessions `identity_provider` and `protocol`),
Horizon's `shellfilter` filter exists and `{% load … %}` lines
are ignored. Moonlight's own templates quote with the `sh` (single-quoted shell word) and `yaml`
(double-quoted scalar) filters. Blocks trim their own line (`trim_blocks`, `lstrip_blocks`). Each
template is compiled and rendered once with sample values at start-up: an unreadable file or a
syntax error stops Moonlight with the file and the error.

Every response also carries `X-Frame-Options: DENY` and `frame-ancestors 'none'` (Horizon's
`DISALLOW_IFRAME_EMBED`, always on).

## Leptos

Leptos itself is configured in `[package.metadata.leptos]` in `Cargo.toml` (address, ports,
output names) and can be overridden with `LEPTOS_*` variables.
