//! Data transferred between the server functions and the UI.
//! Kept deliberately flat: the server does all joining, the browser only renders.

use serde::{Deserialize, Serialize};

mod action;
mod admin;
mod alarm;
mod choice;
mod compute;
mod detail;
mod firewall;
mod heat;
mod identity;
mod keymgr;
mod lb;
mod network;
mod quota;
mod topology;
mod usage;
mod volume;
mod write;

pub use action::*;
pub use admin::*;
pub use alarm::*;
pub use choice::*;
pub use compute::*;
pub use detail::*;
pub use firewall::*;
pub use heat::*;
pub use identity::*;
pub use keymgr::*;
pub use lb::*;
pub use network::*;
pub use quota::*;
pub use topology::*;
pub use usage::*;
pub use volume::*;
pub use write::*;

/// A project the user can switch to (Keystone `auth/projects`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
}

/// Who is logged in and where: drives the header bar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub user: String,
    pub domain: String,
    pub project: ProjectRef,
    pub projects: Vec<ProjectRef>,
    pub region: String,
    pub regions: Vec<String>,
    pub write_mode: WriteMode,
    pub help_url: Option<String>,
    pub change_password_url: Option<String>,
    /// Set when the session's region left the catalog and Moonlight switched
    /// to `region`: the region that was lost.
    pub region_fallback: Option<String>,
    /// What this project and region offer: decides which panels and actions show.
    #[serde(default)]
    pub capabilities: Capabilities,
    /// `moonlight.conf` settings the browser needs (launch, network, images, polling).
    #[serde(default)]
    pub prefs: crate::prefs::Prefs,
    /// Days left before the password expires, inside `[session] password_expiry_warning_days`.
    #[serde(default)]
    pub password_expires_in_days: Option<i64>,
}

// ---------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------

/// Optional panels that Horizon gets from plugins. Opt-in: only those listed in
/// `[plugins] enabled` exist, and then only where the catalog has the service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Plugin {
    /// Octavia.
    LoadBalancer,
    /// Heat.
    Orchestration,
    /// Neutron's `qos` extension (Network QoS panel, QoS policy fields).
    Qos,
    /// Neutron's `trunk` extension (Trunks panel).
    Trunk,
    /// Barbican (Key Manager › Secrets, Containers;).
    KeyManager,
    /// Aodh (Monitoring › Alarms;).
    Alarming,
    /// Cinder backups (Volumes › Backups; Horizon's `enable_backup`).
    Backup,
    /// Neutron's `fwaas_v2` extension (Network › Firewall; neutron-fwaas-dashboard).
    Firewall,
}

impl Plugin {
    /// Every plugin, in menu order; `[plugins] enabled` names are checked against it.
    pub const ALL: [Plugin; 8] = [
        Plugin::LoadBalancer,
        Plugin::Orchestration,
        Plugin::Qos,
        Plugin::Trunk,
        Plugin::KeyManager,
        Plugin::Alarming,
        Plugin::Backup,
        Plugin::Firewall,
    ];

    /// The name used in `moonlight.conf` (for services, also the catalog type).
    pub fn name(self) -> &'static str {
        match self {
            Plugin::LoadBalancer => "load-balancer",
            Plugin::Orchestration => "orchestration",
            Plugin::Qos => "qos",
            Plugin::Trunk => "trunk",
            Plugin::KeyManager => "key-manager",
            Plugin::Alarming => "alarming",
            Plugin::Backup => "backup",
            Plugin::Firewall => "firewall",
        }
    }

    /// The plugin named `s` in `moonlight.conf` (`load-balancer`…), if any.
    pub fn parse(s: &str) -> Option<Plugin> {
        Plugin::ALL.into_iter().find(|p| p.name() == s.trim())
    }
}

/// What the cloud offers to the current project in the current region,
/// detected with GETs only and cached per connection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    /// `false` until detection succeeded: then every check passes (fail open),
    /// so a detection hiccup never hides what works.
    pub known: bool,
    /// Catalog service types with an endpoint in the region.
    pub services: Vec<String>,
    /// Neutron extension aliases (`router`, `trunk`, `extraroute-atomic`…).
    pub network_extensions: Vec<String>,
    /// Highest microversions the region supports, `(major, minor)`.
    pub compute_mv: Option<(u16, u16)>,
    pub volume_mv: Option<(u16, u16)>,
    /// Octavia's API version (`v2.27` → `(2, 27)`; it has no microversions).
    #[serde(default)]
    pub lb_mv: Option<(u16, u16)>,
    /// Enabled in `moonlight.conf`; enforced even when detection failed.
    #[serde(default)]
    pub plugins: Vec<Plugin>,
    /// Role names on the scoped token (implied roles included), lowercase.
    #[serde(default)]
    pub roles: Vec<String>,
    /// `[keystone] multidomain`: configuration, added per request.
    #[serde(default)]
    pub multidomain: bool,
    /// Glance's enabled import methods (`GET /v2/info/import`: `web-download`,
    /// `glance-direct`…). `None` when the call failed: the check then passes.
    #[serde(default)]
    pub image_import: Option<Vec<String>>,
}

/// Service types that name the same service across clouds and releases.
const SERVICE_ALIASES: &[&[&str]] = &[
    &["block-storage", "volumev3", "volumev2", "volume"],
    &["load-balancer", "octavia"],
];

impl Capabilities {
    /// Whether the catalog has `ty` (e.g. `load-balancer`). Unknown capabilities
    /// (`known: false`) count as having everything, so a failed detection hides nothing.
    pub fn has_service(&self, ty: &str) -> bool {
        let names = SERVICE_ALIASES
            .iter()
            .find(|a| a.contains(&ty))
            .copied()
            .unwrap_or(std::slice::from_ref(&ty));
        self.services.iter().any(|s| names.contains(&s.as_str()))
    }
}

/// When a panel, tab or action is available. Pure, so every rule is unit-tested.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Requires {
    Always,
    /// A catalog service type (aliases included, e.g. `volumev3` for `block-storage`).
    Service(&'static str),
    /// A Neutron extension alias.
    NetExt(&'static str),
    /// At least this compute (Nova) microversion.
    ComputeMv(u16, u16),
    /// At least this volume (Cinder) microversion.
    VolumeMv(u16, u16),
    /// At least this load-balancer (Octavia) API version.
    LbMv(u16, u16),
    /// Enabled in `[plugins] enabled`: configuration, so checked even when
    /// detection failed (it does not fail open).
    Plugin(Plugin),
    /// Glance has this import method enabled (`web-download`). Fails open when
    /// the list could not be read, like the microversions.
    ImageImport(&'static str),
    /// One of these roles on the scoped token. Neutron's policy can't be read
    /// through the API, so this mirrors its defaults; the server still decides.
    AnyRole(&'static [&'static str]),
    /// `[keystone] multidomain` is on: configuration, so it doesn't fail open.
    MultiDomain,
    All(&'static [Requires]),
}

impl Requires {
    /// Whether the cloud offers this. Fails open on what wasn't detected (no
    /// version range, `known: false`), except [`Requires::Plugin`], which is opt-in.
    pub fn met(&self, c: &Capabilities) -> bool {
        // A service that reports no version range (or none discovered) passes.
        let at_least = |have: Option<(u16, u16)>, want: (u16, u16)| have.is_none_or(|h| h >= want);
        match *self {
            Requires::Plugin(p) => c.plugins.contains(&p),
            Requires::MultiDomain => c.multidomain,
            Requires::All(list) => list.iter().all(|r| r.met(c)),
            // Detection failed: everything the cloud may offer passes.
            _ if !c.known => true,
            Requires::Always => true,
            Requires::Service(ty) => c.has_service(ty),
            Requires::NetExt(alias) => c.network_extensions.iter().any(|e| e == alias),
            Requires::ComputeMv(a, b) => at_least(c.compute_mv, (a, b)),
            Requires::VolumeMv(a, b) => at_least(c.volume_mv, (a, b)),
            Requires::LbMv(a, b) => at_least(c.lb_mv, (a, b)),
            Requires::AnyRole(roles) => roles.iter().any(|r| c.roles.iter().any(|h| h == r)),
            Requires::ImageImport(m) => c.image_import.as_ref().is_none_or(|l| l.iter().any(|x| x == m)),
        }
    }

    /// Whether a role this needs is missing, as opposed to a service, version
    /// or setting: the user is refused something the cloud has.
    pub fn lacks_role(&self, c: &Capabilities) -> bool {
        match *self {
            Requires::AnyRole(_) => !self.met(c),
            Requires::All(list) => list.iter().any(|r| r.lacks_role(c)),
            _ => false,
        }
    }
}

/// `"2.95"` → `(2, 95)`.
pub fn parse_microversion(s: &str) -> Option<(u16, u16)> {
    let (a, b) = s.trim().split_once('.')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// Requirements of version-dependent actions, in one place.
pub mod needs {
    use super::Requires;
    /// `rebuild_294`: also rebuilds volume-backed servers (>= 2.93).
    pub const REBUILD: Requires = Requires::ComputeMv(2, 94);
    /// Unshelve into a chosen availability zone.
    pub const UNSHELVE_TO_ZONE: Requires = Requires::ComputeMv(2, 77);
    /// `os-volume_upload_image` with visibility/protected (3.1).
    pub const UPLOAD_TO_IMAGE: Requires = Requires::VolumeMv(3, 1);
    /// Cinder user messages.
    pub const VOLUME_MESSAGES: Requires = Requires::VolumeMv(3, 3);
    /// Volumes › Groups and Group Snapshots (generic volume groups). Cinder has
    /// them from 3.13/3.14; Horizon shows the panels from 3.27 (its `groups`
    /// microversion feature), and so do we.
    pub const VOLUME_GROUPS: Requires = Requires::VolumeMv(3, 27);
    /// Listener timeouts (Octavia API 2.1).
    pub const LB_TIMEOUTS: Requires = Requires::LbMv(2, 1);
    /// `tls_enabled` on pools: re-encrypt to the members (Octavia 2.8).
    pub const LB_POOL_TLS: Requires = Requires::LbMv(2, 8);
    /// `allowed_cidrs` on listeners (Octavia 2.12).
    pub const LB_ALLOWED_CIDRS: Requires = Requires::LbMv(2, 12);
    /// `TERMINATED_HTTPS` listeners: certificates come from Barbican, so the
    /// `key-manager` plugin is on and the service is in the catalog.
    pub const LB_TLS_TERMINATION: Requires = KEY_MANAGER;
    /// `tls_ciphers` on pools and listeners (Octavia 2.15).
    pub const LB_POOL_CIPHERS: Requires = Requires::LbMv(2, 15);
    /// Atomic add/remove of one static route.
    pub const STATIC_ROUTES: Requires = Requires::NetExt("extraroute-atomic");
    /// Static routes by replacing the whole list (older clouds).
    pub const STATIC_ROUTES_LIST: Requires = Requires::NetExt("extraroute");
    /// Floating IP port forwarding rules, and port ranges in them.
    pub const PORT_FORWARDING: Requires = Requires::NetExt("floating-ip-port-forwarding");
    /// Port *ranges* in port forwarding (Neutron extension of that name).
    pub const PORT_FORWARDING_RANGES: Requires = Requires::NetExt("floating-ip-port-forwarding-port-ranges");
    /// Network QoS panel, and QoS policies on router gateways and floating IPs.
    pub const QOS: Requires = Requires::All(&[Requires::Plugin(super::Plugin::Qos), Requires::NetExt("qos")]);
    /// *QoS Policy* on a router's external gateway (`qos-gateway-ip`).
    pub const QOS_GATEWAY: Requires = Requires::All(&[
        Requires::Plugin(super::Plugin::Qos),
        Requires::NetExt("qos"),
        Requires::NetExt("qos-gateway-ip"),
    ]);
    /// *QoS Policy* on a floating IP (`qos-fip`).
    pub const QOS_FIP: Requires = Requires::All(&[
        Requires::Plugin(super::Plugin::Qos),
        Requires::NetExt("qos"),
        Requires::NetExt("qos-fip"),
    ]);
    /// Key Manager panels (Barbican).
    pub const KEY_MANAGER: Requires = Requires::All(&[
        Requires::Plugin(super::Plugin::KeyManager),
        Requires::Service("key-manager"),
    ]);
    /// Orchestration panels (Heat).
    pub const ORCHESTRATION: Requires = Requires::All(&[
        Requires::Plugin(super::Plugin::Orchestration),
        Requires::Service("orchestration"),
    ]);
    /// Monitoring › Alarms (Aodh).
    pub const ALARMING: Requires =
        Requires::All(&[Requires::Plugin(super::Plugin::Alarming), Requires::Service("alarming")]);
    /// Volumes › Backups (Cinder's backup service). The catalog only says
    /// Cinder exists, not that `cinder-backup` runs: a missing service shows as
    /// an error on *Create Backup*, like in Horizon.
    pub const BACKUP: Requires = Requires::All(&[
        Requires::Plugin(super::Plugin::Backup),
        Requires::Service("block-storage"),
    ]);
    /// Trunks panel.
    pub const TRUNK: Requires = Requires::All(&[Requires::Plugin(super::Plugin::Trunk), Requires::NetExt("trunk")]);
    /// Network › Firewall (FWaaS v2).
    pub const FIREWALL: Requires =
        Requires::All(&[Requires::Plugin(super::Plugin::Firewall), Requires::NetExt("fwaas_v2")]);
    /// `dns_name` / `dns_domain` on floating IPs and ports.
    pub const DNS: Requires = Requires::NetExt("dns-integration");
    /// Making an image `public`: Glance's default policy `publicize_image` is admin only.
    pub const PUBLICIZE_IMAGE: Requires = Requires::AnyRole(&["admin"]);
    /// *Create Image* › *URL*: without `web-download` Glance rejects the import.
    pub const IMAGE_URL_IMPORT: Requires = Requires::ImageImport("web-download");
    /// `POST /servers/{id}/remote-consoles`; older clouds get `os-getVNCConsole`.
    pub const REMOTE_CONSOLE: Requires = Requires::ComputeMv(2, 6);
    /// `enable_snat` on a router gateway: `ext-gw-mode`, and Neutron's default
    /// policy `update_router:external_gateway_info:enable_snat` is admin only.
    pub const SNAT: Requires = Requires::All(&[Requires::NetExt("ext-gw-mode"), Requires::AnyRole(&["admin"])]);
    /// Identity › Groups and Roles. Horizon shows them when its
    /// bundled Keystone policy passes `identity:list_groups` / `list_roles`,
    /// which with a project-scoped token means the `admin` role.
    pub const IDENTITY_ADMIN: Requires = Requires::AnyRole(&["admin"]);
    /// Identity › Domains: admins on a multi-domain cloud (Horizon shows it with
    /// `OPENSTACK_KEYSTONE_MULTIDOMAIN_SUPPORT`).
    pub const DOMAINS: Requires = Requires::All(&[Requires::MultiDomain, Requires::AnyRole(&["admin"])]);
    /// *Binding: VNIC Type* in *Edit Port* (`binding:vnic_type`).
    pub const PORT_BINDING: Requires = Requires::NetExt("binding");
    /// *Port Security* in *Edit Port* (`port_security_enabled`).
    pub const PORT_SECURITY: Requires = Requires::NetExt("port-security");
    /// The Admin dashboard: Horizon shows it to the `admin` role.
    /// The menu only; every admin server function checks the token itself.
    pub const ADMIN: Requires = Requires::AnyRole(&["admin"]);
    /// Admin › System Information › *Network Agents*: Neutron's `agent` extension.
    pub const NETWORK_AGENTS: Requires = Requires::NetExt("agent");
    /// *Availability Zone Hints* on a new network.
    pub const NETWORK_AZ: Requires = Requires::NetExt("network_availability_zone");
    /// Admin › Networks' *DHCP Agents* column (`agents/{id}/dhcp-networks`).
    pub const DHCP_AGENTS: Requires = Requires::NetExt("dhcp_agent_scheduler");
    /// *Availability Zone Hints* on a new router (Admin › Routers).
    pub const ROUTER_AZ: Requires = Requires::NetExt("router_availability_zone");
    /// *Router Type* (`distributed`) on a new router: Neutron's `dvr`; its
    /// default policy `create_router:distributed` is admin only.
    pub const DVR: Requires = Requires::All(&[Requires::NetExt("dvr"), Requires::AnyRole(&["admin"])]);
    /// *High Availability Mode* (`ha`) on a new router: Neutron's `l3-ha`;
    /// `create_router:ha` is admin only.
    /// *Enable* / *Disable Service* on Hypervisors › Compute Host: service
    /// UUIDs in `PUT /os-services/{id}` arrived in compute 2.53.
    pub const SERVICE_TOGGLE: Requires = Requires::ComputeMv(2, 53);
    /// *Live Migrate Instance*: `block_migration: "auto"` and scheduler-checked
    /// target hosts (compute 2.30).
    pub const LIVE_MIGRATE: Requires = Requires::ComputeMv(2, 30);
    /// A target host in *Migrate Instance* (compute 2.56); without, the
    /// scheduler picks one.
    pub const MIGRATE_HOST: Requires = Requires::ComputeMv(2, 56);
    /// *HA Mode* on a new router and the router's HA column: Neutron's `l3-ha`; like `dvr`, its
    /// default policy (`create_router:ha`) is admin only.
    pub const L3_HA: Requires = Requires::All(&[Requires::NetExt("l3-ha"), Requires::AnyRole(&["admin"])]);
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// What went wrong, as far as the UI cares: it picks the title, explanation
/// and the actions offered on the error page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorKind {
    /// No session, or the Keystone token expired.
    Unauthenticated,
    Forbidden,
    NotFound,
    /// The service is missing from the catalog of the current region.
    ServiceUnavailable,
    RegionUnavailable,
    /// The cloud did not answer (timeout, connection refused, DNS…).
    Timeout,
    /// Any other HTTP error status returned by OpenStack.
    OpenStack(u16),
    /// The request was rejected before reaching OpenStack (validation).
    Invalid,
    /// Too many failed sign-ins: `login` refuses without asking Keystone
    /// for a while (`[login] max_failures`).
    TooManyAttempts,
    /// A full list has more items than `[ui] max_list_items` allows.
    TooLarge,
    Internal,
}

/// The error type of every server function.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppError {
    pub kind: ErrorKind,
    /// Raw technical message, shown under "Technical details".
    pub detail: String,
    /// Service type involved (`compute`, `network`…), when known.
    pub service: Option<String>,
    /// OpenStack request ID (`req-…`), when the API returned one.
    pub request_id: Option<String>,
}

impl AppError {
    /// An error of `kind` with its technical `detail` (shown under *Technical details*).
    pub fn new(kind: ErrorKind, detail: impl Into<String>) -> Self {
        AppError {
            kind,
            detail: detail.into(),
            service: None,
            request_id: None,
        }
    }

    /// Rejected by Moonlight before reaching OpenStack (validation).
    pub fn invalid(detail: impl Into<String>) -> Self {
        Self::new(ErrorKind::Invalid, detail)
    }

    /// A bug or an answer Moonlight can't use (shown as *Something went wrong*).
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, detail)
    }

    /// Worth trying again as is: the cloud was slow or failed on its side.
    pub fn retryable(&self) -> bool {
        matches!(self.kind, ErrorKind::Timeout | ErrorKind::Internal)
            || matches!(self.kind, ErrorKind::OpenStack(s) if s >= 500)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for AppError {}

impl leptos::server_fn::error::FromServerFnError for AppError {
    type Encoder = leptos::server_fn::codec::JsonEncoding;

    fn from_server_fn_error(e: leptos::server_fn::error::ServerFnErrorErr) -> Self {
        use leptos::server_fn::error::ServerFnErrorErr as E;
        match e {
            // The browser could not reach Moonlight (or the reply was cut).
            E::Request(m) | E::Response(m) => AppError::new(ErrorKind::Timeout, m),
            other => AppError::internal(other.to_string()),
        }
    }
}

/// Wraps every list with how long the backend took (shown discreetly in the UI).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timed<T> {
    pub data: T,
    pub millis: u64,
    pub calls: u32,
}

// --- Paged lists ------------------------------------------------------

/// Which page of a paged list to load, and how it is filtered and sorted. Flat
/// (no tuples) because server functions encode their arguments in the URL. It
/// mirrors the page's query string (`marker`, `f`, `q`, `sort`, `dir`), so a page
/// and its filters can be linked and reloaded. `field` and `sort` are neutral keys
/// from [`PagedList::filters`] / [`PagedList::sorts`], never API parameter names.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListQuery {
    /// ID of the last row of the previous page; `None` = first page.
    #[serde(default)]
    pub marker: Option<String>,
    /// Neutral key of the filter (one at a time, as Horizon's admin filter).
    #[serde(default)]
    pub field: Option<String>,
    /// The filter's value; empty or `None` = no filter.
    #[serde(default)]
    pub value: Option<String>,
    /// Neutral key of the server-side sort; `None` = the API's default order.
    #[serde(default)]
    pub sort: Option<String>,
    /// Sort descending.
    #[serde(default)]
    pub desc: bool,
}

/// One page of a list. `paged` is false when the whole list came back
/// (`[ui] page_size = 0`, or a list `[ui] paged_lists` leaves whole): the page
/// then shows the client-side table as before.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    pub rows: Vec<T>,
    /// Marker (the last row's ID) of the following page; `None` on the last one.
    pub next: Option<String>,
    /// Total rows, when the API gives it cheaply (Cinder, volume 3.45).
    #[serde(default)]
    pub count: Option<u64>,
    pub paged: bool,
    /// Rows per page that was asked for; `0` when not paged.
    #[serde(default)]
    pub page_size: u32,
    /// `[admin] filter_first`: empty because no filter was given;
    /// OpenStack was not asked.
    #[serde(default)]
    pub needs_filter: bool,
}

/// What *Count all* found: the rows matching a paged list's filter.
/// `more` = the count stopped early (`fetch::MAX_COUNT_PAGES` requests, or its
/// time budget), so there are at least `count`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowCount {
    pub count: u64,
    pub more: bool,
}

/// How a filter's value is picked in the filter bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterKind {
    /// Free text the API matches as a substring (Nova's regex names, Cinder's `name~`).
    Text,
    /// Free text the API matches whole: an address, a host, an ID. The box says so,
    /// so a partial value finding nothing is no surprise.
    Exact,
    /// A resource picked by name from a choice set; the value sent is its ID.
    /// `AllProjects` for projects; `Flavors` and `Images` for Nova's `flavor` and
    /// `image` filters, which take IDs only (a typed name found nothing).
    Pick(ChoiceSet),
    /// One of the API's values, e.g. a status.
    Choice(&'static [&'static str]),
}

/// A filter the server applies: `key` is neutral and `param` the API's query parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilterDef {
    pub key: &'static str,
    pub param: &'static str,
    pub kind: FilterKind,
}

/// A sort the API can do: `key` is neutral and `param` the API's sort key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortDef {
    pub key: &'static str,
    pub param: &'static str,
}

/// How an API takes its sort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortStyle {
    /// `sort_key=<k>&sort_dir=asc|desc` (Nova, Neutron, Glance).
    KeyDir,
    /// `sort=<k>:asc|desc` (Cinder).
    Colon,
}

/// Nova's server statuses offered by the instance filter bar's *Status* (the API's upper case).
const SERVER_STATUSES: &[&str] = &[
    "ACTIVE",
    "BUILD",
    "ERROR",
    "SHUTOFF",
    "PAUSED",
    "SUSPENDED",
    "SHELVED_OFFLOADED",
    "RESIZE",
    "VERIFY_RESIZE",
    "REBOOT",
    "HARD_REBOOT",
    "MIGRATING",
    "REBUILD",
];

/// Shorthand for a [`FilterDef`] in the tables below (a `const fn`, so they stay `const`).
const fn filter(key: &'static str, param: &'static str, kind: FilterKind) -> FilterDef {
    FilterDef { key, param, kind }
}

/// Nova's `name` is a regular expression, so a substring works; `flavor` and `image`
/// take IDs, so they are picked by name.
const INSTANCE_FILTERS: &[FilterDef] = &[
    filter("name", "name", FilterKind::Text),
    filter("project", "project_id", FilterKind::Pick(ChoiceSet::AllProjects)),
    filter("host", "host", FilterKind::Exact),
    filter("status", "status", FilterKind::Choice(SERVER_STATUSES)),
    filter("ip", "ip", FilterKind::Text),
    filter("ip6", "ip6", FilterKind::Text),
    filter("flavor", "flavor", FilterKind::Pick(ChoiceSet::Flavors)),
    filter("image", "image", FilterKind::Pick(ChoiceSet::Images)),
    filter("zone", "availability_zone", FilterKind::Exact),
];

/// Every volume status, for the filter bar (the *Reset Status* choices are fewer).
const FILTER_VOLUME_STATUSES: &[&str] = &[
    "available",
    "in-use",
    "creating",
    "deleting",
    "error",
    "error_deleting",
    "attaching",
    "detaching",
    "extending",
    "error_extending",
    "maintenance",
    "reserved",
    "backing-up",
    "restoring-backup",
    "error_restoring",
    "downloading",
    "uploading",
    "retyping",
    "awaiting-transfer",
];
/// Every snapshot status, for the filter bar.
const FILTER_SNAPSHOT_STATUSES: &[&str] = &[
    "available",
    "creating",
    "deleting",
    "error",
    "error_deleting",
    "backing-up",
    "restoring",
    "unmanaging",
];

/// Cinder's `name~` is *like* (a substring) from volume 3.34; `paging::cinder` turns
/// it into the exact `name` on an older Cinder.
const VOLUME_FILTERS: &[FilterDef] = &[
    filter("name", "name~", FilterKind::Text),
    filter("project", "project_id", FilterKind::Pick(ChoiceSet::AllProjects)),
    filter("host", "host", FilterKind::Exact),
    filter("status", "status", FilterKind::Choice(FILTER_VOLUME_STATUSES)),
    filter("bootable", "bootable", FilterKind::Choice(&["true", "false"])),
    filter("zone", "availability_zone", FilterKind::Exact),
];

/// `volume` takes the volume's ID.
const SNAPSHOT_FILTERS: &[FilterDef] = &[
    filter("name", "name~", FilterKind::Text),
    filter("project", "project_id", FilterKind::Pick(ChoiceSet::AllProjects)),
    filter("status", "status", FilterKind::Choice(FILTER_SNAPSHOT_STATUSES)),
    filter("volume", "volume_id", FilterKind::Exact),
];

/// Neutron matches exactly; `network` (the pool) and `router` take IDs.
const FLOATING_IP_FILTERS: &[FilterDef] = &[
    filter("address", "floating_ip_address", FilterKind::Exact),
    filter("project", "project_id", FilterKind::Pick(ChoiceSet::AllProjects)),
    filter("fixed", "fixed_ip_address", FilterKind::Exact),
    filter("network", "floating_network_id", FilterKind::Exact),
    filter("router", "router_id", FilterKind::Exact),
    filter("status", "status", FilterKind::Choice(&["ACTIVE", "DOWN", "ERROR"])),
];

/// Neutron matches whole values (a name too); `shared` and `external` take `true` / `false`.
const NETWORK_FILTERS: &[FilterDef] = &[
    filter("name", "name", FilterKind::Exact),
    filter("project", "project_id", FilterKind::Pick(ChoiceSet::AllProjects)),
    filter(
        "status",
        "status",
        FilterKind::Choice(&["ACTIVE", "DOWN", "BUILD", "ERROR"]),
    ),
    filter("shared", "shared", FilterKind::Choice(&["true", "false"])),
    filter("external", "router:external", FilterKind::Choice(&["true", "false"])),
];

/// Neutron matches whole values; `status` is one of a router's four.
const ROUTER_FILTERS: &[FilterDef] = &[
    filter("name", "name", FilterKind::Exact),
    filter("project", "project_id", FilterKind::Pick(ChoiceSet::AllProjects)),
    filter(
        "status",
        "status",
        FilterKind::Choice(&["ACTIVE", "ALLOCATING", "DOWN", "ERROR"]),
    ),
];

/// `port` takes the parent port's ID.
const TRUNK_FILTERS: &[FilterDef] = &[
    filter("name", "name", FilterKind::Exact),
    filter("project", "project_id", FilterKind::Pick(ChoiceSet::AllProjects)),
    filter(
        "status",
        "status",
        FilterKind::Choice(&["ACTIVE", "DOWN", "BUILD", "DEGRADED", "ERROR"]),
    ),
    filter("port", "port_id", FilterKind::Exact),
];

/// Glance matches the name whole; the project is the image's `owner`.
const IMAGE_FILTERS: &[FilterDef] = &[
    filter("name", "name", FilterKind::Exact),
    filter("project", "owner", FilterKind::Pick(ChoiceSet::AllProjects)),
    filter(
        "visibility",
        "visibility",
        FilterKind::Choice(&["public", "private", "shared", "community"]),
    ),
    filter(
        "status",
        "status",
        FilterKind::Choice(&[
            "active",
            "queued",
            "saving",
            "uploading",
            "importing",
            "killed",
            "deactivated",
            "pending_delete",
            "deleted",
        ]),
    ),
    filter(
        "disk_format",
        "disk_format",
        FilterKind::Choice(&[
            "raw", "qcow2", "vmdk", "vdi", "vhd", "vhdx", "iso", "ploop", "ami", "ari", "aki",
        ]),
    ),
];

/// Cinder lets an admin filter groups by any column, matched whole.
const VOLUME_GROUP_FILTERS: &[FilterDef] = &[
    filter("name", "name", FilterKind::Exact),
    filter("project", "project_id", FilterKind::Pick(ChoiceSet::AllProjects)),
    filter(
        "status",
        "status",
        FilterKind::Choice(&[
            "available",
            "creating",
            "updating",
            "deleting",
            "error",
            "error_deleting",
        ]),
    ),
];

/// Cinder's group snapshots, matched whole like the groups.
const GROUP_SNAPSHOT_FILTERS: &[FilterDef] = &[
    filter("name", "name", FilterKind::Exact),
    filter("project", "project_id", FilterKind::Pick(ChoiceSet::AllProjects)),
    filter(
        "status",
        "status",
        FilterKind::Choice(&["available", "creating", "deleting", "error", "error_deleting"]),
    ),
];

/// Neutron's sorts for its lists (`allow_sorting`, on by default).
const NEUTRON_SORTS: &[SortDef] = &[
    SortDef {
        key: "name",
        param: "name",
    },
    SortDef {
        key: "status",
        param: "status",
    },
];

/// The big lists that load one page at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PagedList {
    /// Admin › Compute › Instances (Nova `servers/detail?all_tenants`).
    Instances,
    /// Admin › Volume › Volumes (Cinder `volumes/detail?all_tenants`).
    Volumes,
    /// Admin › Volume › Snapshots (Cinder `snapshots/detail?all_tenants`).
    Snapshots,
    /// Admin › Network › Floating IPs (Neutron `floatingips`; an admin sees every project's).
    FloatingIps,
    /// Admin › Network › Networks (Neutron `networks`).
    Networks,
    /// Admin › Network › Routers (Neutron `routers`).
    Routers,
    /// Admin › Network › Trunks (Neutron `trunks`, `trunk` plugin).
    Trunks,
    /// Admin › Compute › Images (Glance `images`; an admin sees every project's).
    Images,
    /// Admin › Volume › Groups (Cinder `groups/detail?all_tenants`, volume 3.58).
    VolumeGroups,
    /// Admin › Volume › Group Snapshots (Cinder `group_snapshots/detail?all_tenants`).
    GroupSnapshots,
}

impl PagedList {
    /// The name `[admin] filter_first` uses for it.
    pub const KEYS: &[(&str, PagedList)] = &[
        ("instances", PagedList::Instances),
        ("volumes", PagedList::Volumes),
        ("snapshots", PagedList::Snapshots),
        ("floating_ips", PagedList::FloatingIps),
        ("networks", PagedList::Networks),
        ("routers", PagedList::Routers),
        ("trunks", PagedList::Trunks),
        ("images", PagedList::Images),
        ("volume_groups", PagedList::VolumeGroups),
        ("group_snapshots", PagedList::GroupSnapshots),
    ];
}

/// Whose rows a paged list shows: an Admin panel's, every project's;
/// a Project panel's (`[ui] paged_lists = "all"`), the session project's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListScope {
    #[default]
    Admin,
    Project,
}

/// Filters that only make sense across projects: the project itself, and the host,
/// which Nova and Cinder show only to admins.
const ADMIN_ONLY_FILTERS: &[&str] = &["project", "host"];

impl PagedList {
    /// [`Self::filters`] offered in `scope`: a Project panel drops the
    /// [`ADMIN_ONLY_FILTERS`]. `paging::params` refuses the others there.
    pub fn filters_in(self, scope: ListScope) -> Vec<FilterDef> {
        self.filters()
            .iter()
            .copied()
            .filter(|d| scope == ListScope::Admin || !ADMIN_ONLY_FILTERS.contains(&d.key))
            .collect()
    }

    /// The filters the server accepts, in the filter bar's order; the first is the default.
    pub fn filters(self) -> &'static [FilterDef] {
        match self {
            PagedList::Instances => INSTANCE_FILTERS,
            PagedList::Volumes => VOLUME_FILTERS,
            PagedList::Snapshots => SNAPSHOT_FILTERS,
            PagedList::FloatingIps => FLOATING_IP_FILTERS,
            PagedList::Networks => NETWORK_FILTERS,
            PagedList::Routers => ROUTER_FILTERS,
            PagedList::Trunks => TRUNK_FILTERS,
            PagedList::Images => IMAGE_FILTERS,
            PagedList::VolumeGroups => VOLUME_GROUP_FILTERS,
            PagedList::GroupSnapshots => GROUP_SNAPSHOT_FILTERS,
        }
    }

    /// The sorts the API does for this list.
    pub fn sorts(self) -> &'static [SortDef] {
        match self {
            PagedList::Instances => &[
                SortDef {
                    key: "name",
                    param: "display_name",
                },
                SortDef {
                    key: "created",
                    param: "created_at",
                },
                // Not `host`: the test cloud's Nova answers `sort_key=host` with HTTP 500
                // (multi-cell listings merge the cells' rows in Python, and a server with no
                // host can't be compared); its header sorts the page instead.
                SortDef {
                    key: "status",
                    param: "vm_state",
                },
            ],
            PagedList::Volumes => &[
                SortDef {
                    key: "name",
                    param: "name",
                },
                SortDef {
                    key: "size",
                    param: "size",
                },
                SortDef {
                    key: "created",
                    param: "created_at",
                },
                SortDef {
                    key: "status",
                    param: "status",
                },
            ],
            // No `size`: a snapshot's column is `volume_size`, and Cinder refuses
            // `sort=size` on snapshots with HTTP 400 (seen on the test cloud).
            PagedList::Snapshots => &[
                SortDef {
                    key: "name",
                    param: "name",
                },
                SortDef {
                    key: "created",
                    param: "created_at",
                },
                SortDef {
                    key: "status",
                    param: "status",
                },
            ],
            PagedList::FloatingIps => &[
                SortDef {
                    key: "address",
                    param: "floating_ip_address",
                },
                SortDef {
                    key: "status",
                    param: "status",
                },
            ],
            PagedList::Networks | PagedList::Routers | PagedList::Trunks => NEUTRON_SORTS,
            PagedList::Images => &[
                SortDef {
                    key: "name",
                    param: "name",
                },
                SortDef {
                    key: "created",
                    param: "created_at",
                },
                SortDef {
                    key: "size",
                    param: "size",
                },
                SortDef {
                    key: "status",
                    param: "status",
                },
            ],
            PagedList::VolumeGroups | PagedList::GroupSnapshots => &[
                SortDef {
                    key: "name",
                    param: "name",
                },
                SortDef {
                    key: "created",
                    param: "created_at",
                },
                SortDef {
                    key: "status",
                    param: "status",
                },
            ],
        }
    }

    /// How the list's API takes a sort.
    pub fn sort_style(self) -> SortStyle {
        match self {
            PagedList::Instances
            | PagedList::FloatingIps
            | PagedList::Networks
            | PagedList::Routers
            | PagedList::Trunks
            | PagedList::Images => SortStyle::KeyDir,
            PagedList::Volumes | PagedList::Snapshots | PagedList::VolumeGroups | PagedList::GroupSnapshots => {
                SortStyle::Colon
            }
        }
    }

    /// Query parameters every request of the list carries (cross-project listing).
    /// Neutron and Glance need none: an admin's list already holds every project's.
    pub fn fixed(self) -> &'static [(&'static str, &'static str)] {
        match self {
            PagedList::Instances
            | PagedList::Volumes
            | PagedList::Snapshots
            | PagedList::VolumeGroups
            | PagedList::GroupSnapshots => &[("all_tenants", "true")],
            PagedList::FloatingIps
            | PagedList::Networks
            | PagedList::Routers
            | PagedList::Trunks
            | PagedList::Images => &[],
        }
    }

    /// The list is Cinder's volumes or snapshots: its name filter (`name~`) and total
    /// (`with_count`) depend on the cloud's volume microversion (`paging::cinder`).
    /// Groups are Cinder's too but have neither: they always send volume 3.58.
    pub fn is_cinder(self) -> bool {
        matches!(self, PagedList::Volumes | PagedList::Snapshots)
    }

    /// The list is Neutron's, which can return only the fields asked for (`fields=id`).
    pub fn is_neutron(self) -> bool {
        matches!(
            self,
            PagedList::FloatingIps | PagedList::Networks | PagedList::Routers | PagedList::Trunks
        )
    }
}

/// Refuses a value that would change the URL it is pasted into:
/// the SDK and our JSON requests build paths with `format!("servers/{id}")`,
/// unencoded, so `/`, `..`, `?` or `#` would reach another endpoint of the same
/// service, past Moonlight's own gates. OpenStack IDs (UUIDs) and names never
/// need these characters.
pub fn check_id(v: &str) -> Result<(), AppError> {
    let bad = v.is_empty()
        || v == "."
        || v == ".."
        || v.chars()
            .any(|c| matches!(c, '/' | '\\' | '?' | '#' | '%') || c.is_control());
    if bad {
        return Err(AppError::invalid(format!("not a valid ID: {v:?}")));
    }
    Ok(())
}

#[cfg(test)]
mod check_id_tests {
    use super::*;

    #[test]
    fn ids_that_would_change_the_path_are_refused() {
        for ok in [
            "3f2a9c1e-0000-0000-0000-000000000c91",
            "my key",
            "web-1.example",
            "a..b",
        ] {
            assert!(check_id(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            ".",
            "..",
            "a/b",
            "../servers",
            "x?all_tenants=1",
            "x#y",
            "a%2Fb",
            "a\\b",
            "a\nb",
        ] {
            assert!(check_id(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn actions_are_checked_by_key_name_wherever_the_id_is() {
        assert!(
            OsAction::ServerDelete {
                id: "../os-hypervisors".into()
            }
            .check_ids()
            .is_err()
        );
        let ok = OsAction::PoolSetMembers {
            pool_id: "pool-1".into(),
            members: vec![LbMemberSpec {
                name: Some("web/1".into()),
                address: "10.0.0.5".into(),
                port: 80,
                subnet_id: Some("sub-1".into()),
                weight: None,
                backup: false,
                monitor_address: None,
                monitor_port: None,
                admin_up: true,
            }],
        };
        assert!(
            ok.check_ids().is_ok(),
            "a member *name* isn't in any path, so it may hold a slash"
        );
        let OsAction::PoolSetMembers { pool_id, mut members } = ok else {
            unreachable!()
        };
        members[0].subnet_id = Some("sub?x".into());
        assert!(
            OsAction::PoolSetMembers { pool_id, members }.check_ids().is_err(),
            "a nested `*_id`"
        );
        let groups = OsAction::PortUpdate {
            id: "p1".into(),
            name: None,
            admin_up: None,
            vnic_type: None,
            port_security: None,
            security_group_ids: Some(vec!["g1".into(), "g2/../x".into()]),
        };
        assert!(groups.check_ids().is_err(), "a list under `*_ids`");
        assert!(
            OsAction::KeyPairDelete { name: "a/b".into() }.check_ids().is_err(),
            "key pair names are in the path"
        );
        assert!(
            OsAction::QosRuleDelete {
                policy_id: "p".into(),
                kind: "x/../y".into(),
                id: "r".into()
            }
            .check_ids()
            .is_err()
        );
        let batch = OsAction::Batch(vec![("vol a".into(), OsAction::VolumeDelete { id: "v/1".into() })]);
        assert!(batch.check_ids().is_err(), "inside a batch");
        let empty = OsAction::ListenerUpdate {
            id: "ls1".into(),
            name: String::new(),
            description: String::new(),
            connection_limit: None,
            default_pool_id: Some(String::new()),
            admin_up: true,
            protocol: "HTTP".into(),
            options: None,
            tls: None,
        };
        assert!(empty.check_ids().is_ok(), "empty optional IDs are left to the action");
    }
}
