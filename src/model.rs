//! Data transferred between the server functions and the UI.
//! Kept deliberately flat: the server does all joining, the browser only renders.

use serde::{Deserialize, Serialize};

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
// Capabilities (roadmap 19)
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
    /// Barbican (Key Manager › Secrets, Containers; roadmap 55).
    KeyManager,
    /// Aodh (Monitoring › Alarms; roadmap 79).
    Alarming,
    /// Cinder backups (Volumes › Backups; Horizon's `enable_backup`, roadmap 5).
    Backup,
    /// Neutron's `fwaas_v2` extension (Network › Firewall; neutron-fwaas-dashboard, roadmap 93).
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
    /// `glance-direct`…). `None` when the call failed: the check then passes (roadmap 45).
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
    /// Glance has this import method enabled (`web-download`, roadmap 45). Fails open when
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
    /// or setting: the user is refused something the cloud has (roadmap 85).
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
    /// microversion feature), and so do we (roadmap 5).
    pub const VOLUME_GROUPS: Requires = Requires::VolumeMv(3, 27);
    /// Listener timeouts (Octavia API 2.1, roadmap 54).
    pub const LB_TIMEOUTS: Requires = Requires::LbMv(2, 1);
    /// `tls_enabled` on pools: re-encrypt to the members (Octavia 2.8).
    pub const LB_POOL_TLS: Requires = Requires::LbMv(2, 8);
    /// `allowed_cidrs` on listeners (Octavia 2.12).
    pub const LB_ALLOWED_CIDRS: Requires = Requires::LbMv(2, 12);
    /// `TERMINATED_HTTPS` listeners: certificates come from Barbican, so the
    /// `key-manager` plugin is on and the service is in the catalog (roadmap 54).
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
    /// Key Manager panels (Barbican, roadmap 55).
    pub const KEY_MANAGER: Requires = Requires::All(&[
        Requires::Plugin(super::Plugin::KeyManager),
        Requires::Service("key-manager"),
    ]);
    /// Orchestration panels (Heat, roadmap 62).
    pub const ORCHESTRATION: Requires = Requires::All(&[
        Requires::Plugin(super::Plugin::Orchestration),
        Requires::Service("orchestration"),
    ]);
    /// Monitoring › Alarms (Aodh, roadmap 79).
    pub const ALARMING: Requires =
        Requires::All(&[Requires::Plugin(super::Plugin::Alarming), Requires::Service("alarming")]);
    /// Volumes › Backups (Cinder's backup service, roadmap 5). The catalog only says
    /// Cinder exists, not that `cinder-backup` runs: a missing service shows as
    /// an error on *Create Backup*, like in Horizon.
    pub const BACKUP: Requires = Requires::All(&[
        Requires::Plugin(super::Plugin::Backup),
        Requires::Service("block-storage"),
    ]);
    /// Trunks panel.
    pub const TRUNK: Requires = Requires::All(&[Requires::Plugin(super::Plugin::Trunk), Requires::NetExt("trunk")]);
    /// Network › Firewall (FWaaS v2, roadmap 93).
    pub const FIREWALL: Requires =
        Requires::All(&[Requires::Plugin(super::Plugin::Firewall), Requires::NetExt("fwaas_v2")]);
    /// `dns_name` / `dns_domain` on floating IPs and ports.
    pub const DNS: Requires = Requires::NetExt("dns-integration");
    /// Making an image `public`: Glance's default policy `publicize_image` is admin only.
    pub const PUBLICIZE_IMAGE: Requires = Requires::AnyRole(&["admin"]);
    /// *Create Image* › *URL*: without `web-download` Glance rejects the import (roadmap 45).
    pub const IMAGE_URL_IMPORT: Requires = Requires::ImageImport("web-download");
    /// `POST /servers/{id}/remote-consoles`; older clouds get `os-getVNCConsole`.
    pub const REMOTE_CONSOLE: Requires = Requires::ComputeMv(2, 6);
    /// `enable_snat` on a router gateway: `ext-gw-mode`, and Neutron's default
    /// policy `update_router:external_gateway_info:enable_snat` is admin only.
    pub const SNAT: Requires = Requires::All(&[Requires::NetExt("ext-gw-mode"), Requires::AnyRole(&["admin"])]);
    /// Identity › Groups and Roles (roadmap 57). Horizon shows them when its
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
    /// The Admin dashboard (roadmap 85): Horizon shows it to the `admin` role.
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
    /// for a while (`[login] max_failures`, roadmap 50).
    TooManyAttempts,
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

// --- Paged lists (roadmap 88) ------------------------------------------------------

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
    /// `[admin] filter_first` (roadmap 89): empty because no filter was given;
    /// OpenStack was not asked.
    #[serde(default)]
    pub needs_filter: bool,
}

/// What *Count all* found (roadmap 88): the rows matching a paged list's filter.
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
    /// `image` filters, which take IDs only (a typed name found nothing, roadmap 88).
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

/// The big lists that load one page at a time (roadmap 88).
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
    /// The name `[admin] filter_first` uses for it (roadmap 89).
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

/// Whose rows a paged list shows (roadmap 88): an Admin panel's, every project's;
/// a Project panel's (`[ui] paged_lists = "all"`, slice E), the session project's.
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
                // host can't be compared); its header sorts the page instead (roadmap 88).
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
            // `sort=size` on snapshots with HTTP 400 (seen on the test cloud, roadmap 88).
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

/// One address of an instance, from Nova's `addresses` (per network).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Address {
    pub network: String,
    pub addr: String,
    pub floating: bool,
    pub mac: Option<String>,
}

/// A row of the Instances list (Nova `servers/detail` at 2.47, with names resolved).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    /// `None` when the server boots from a volume.
    pub image_name: Option<String>,
    pub addresses: Vec<Address>,
    pub flavor_name: String,
    /// Full flavor (embedded by Nova ≥ 2.47, else from the flavor list), for the popover.
    pub flavor: FlavorInfo,
    /// `image_name` was resolved through the boot volume's image metadata.
    pub image_from_volume: bool,
    pub locked: bool,
    pub key_name: Option<String>,
    pub status: String,
    pub availability_zone: Option<String>,
    pub task_state: Option<String>,
    pub power_state: i64,
    /// RFC 3339
    pub created: String,
    /// Nova server metadata (roadmap 25).
    #[serde(default)]
    pub metadata: Vec<(String, String)>,
    /// Security group **names** of the server (Nova lists one entry per port, so
    /// deduplicated and sorted), for *Edit Security Groups* (roadmap 42).
    #[serde(default)]
    pub security_groups: Vec<String>,
    /// Volumes still to read to name the image of a server booted from a volume:
    /// set only by a paged list (roadmap 88), which shows the page first and fills
    /// the *Image* cell later (`api::get_boot_images`). Empty when the image
    /// is known or the server has no volume.
    #[serde(default)]
    pub boot_volume_ids: Vec<String>,
}

/// Volume IDs per `api::get_boot_images` call: a paged Instances page sends
/// its rows' `boot_volume_ids` in chunks this size, so each GET's query string stays
/// a few KiB and the *Image* cells fill in chunk by chunk (roadmap 88).
pub const BOOT_IMAGE_CHUNK: usize = 50;

/// What `api::get_late_names` looks up: a paged page names the resources its rows
/// point at after the table shows, since neither Nova nor Cinder lists several
/// IDs in one request (probed on ttcp-dev, roadmap 88).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NameKind {
    /// Nova `servers/{id}`: a volume's attachments, a floating IP's instance.
    Servers,
    /// Cinder `volumes/{id}`: a snapshot's volume, with its host.
    Volumes,
}

/// A name `api::get_late_names` found. `host` is the volume's
/// `os-vol-host-attr:host` (a snapshot's host column); `None` for servers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LateName {
    pub id: String,
    pub name: String,
    pub host: Option<String>,
}

/// The flavor of an instance: embedded by Nova ≥ 2.47, else from the flavor list
/// (for the flavor popover).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlavorInfo {
    pub id: String,
    pub name: String,
    pub vcpus: Option<i64>,
    pub ram_mb: Option<i64>,
    pub disk_gb: Option<i64>,
    pub ephemeral_gb: Option<i64>,
    pub swap_mb: Option<i64>,
}

/// Nova's `fault` on an instance in `ERROR` (message, code, date).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fault {
    pub code: i64,
    pub message: String,
}

/// Everything the instance detail page shows (overview, interfaces, volumes, actions).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstanceDetail {
    pub base: Instance,
    pub description: Option<String>,
    pub project_id: String,
    pub host: Option<String>,
    pub locked: bool,
    pub updated: Option<String>,
    pub flavor: FlavorInfo,
    pub image_id: Option<String>,
    pub security_groups: Vec<String>,
    /// Rules of each of `security_groups` the project can list (roadmap 37).
    #[serde(default)]
    pub security_group_rules: Vec<(String, Vec<String>)>,
    pub volumes: Vec<AttachedVolume>,
    pub metadata: Vec<(String, String)>,
    pub fault: Option<Fault>,
    pub interfaces: Vec<Interface>,
    pub actions: Vec<InstanceAction>,
    /// `(id, name)` of the server group holding the instance, found by
    /// membership in the group list (Nova only puts it on the server from 2.71).
    #[serde(default)]
    pub server_group: Option<(String, String)>,
}

/// A volume attached to an instance, with its name and device.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttachedVolume {
    pub id: String,
    pub name: Option<String>,
    pub device: Option<String>,
    pub size_gb: Option<i64>,
}

/// A port of an instance (*Interfaces* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interface {
    pub port_id: String,
    pub network_id: String,
    pub network_name: Option<String>,
    pub fixed_ips: Vec<String>,
    pub mac: String,
    pub state: String,
    /// From the Neutron port (`os-interface` has none of it); `None` when the
    /// port list failed or doesn't have it, and then the row can't be edited.
    pub port: Option<PortSettings>,
}

/// What *Edit Port* edits (Horizon's *Info* and *Security Groups* tabs).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PortSettings {
    pub name: String,
    pub admin_up: bool,
    /// `binding:vnic_type` (`binding` extension); `None` when not returned.
    pub vnic_type: Option<String>,
    /// `port_security_enabled`; `true` when missing, Neutron's default.
    pub port_security: bool,
    pub security_group_ids: Vec<String>,
}

impl PortSettings {
    /// The `PortUpdate` from `self` to `after`, with only what changed; `None`
    /// when nothing did. Turning port security off clears the groups in the same
    /// request: Neutron refuses `port_security_enabled: false` on a port that
    /// still has groups.
    pub fn changes(&self, id: &str, after: &PortSettings) -> Option<OsAction> {
        let name = after.name.trim();
        let ps_change = (after.port_security != self.port_security).then_some(after.port_security);
        let groups = if !after.port_security {
            (!self.security_group_ids.is_empty()).then(Vec::new)
        } else {
            let same = after.security_group_ids.len() == self.security_group_ids.len()
                && after
                    .security_group_ids
                    .iter()
                    .all(|g| self.security_group_ids.contains(g));
            (!same).then(|| after.security_group_ids.clone())
        };
        let action = OsAction::PortUpdate {
            id: id.to_string(),
            name: (name != self.name).then(|| name.to_string()),
            admin_up: (after.admin_up != self.admin_up).then_some(after.admin_up),
            vnic_type: after.vnic_type.clone().filter(|v| Some(v) != self.vnic_type.as_ref()),
            port_security: ps_change,
            security_group_ids: groups,
        };
        match &action {
            OsAction::PortUpdate {
                name: None,
                admin_up: None,
                vnic_type: None,
                port_security: None,
                security_group_ids: None,
                ..
            } => None,
            _ => Some(action),
        }
    }
}

/// One entry of the instance's action log (`os-instance-actions`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstanceAction {
    pub request_id: String,
    pub action: String,
    pub start_time: String,
    pub user_id: Option<String>,
    pub message: Option<String>,
}

/// A row of the Images list (Glance v2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Image {
    pub id: String,
    pub name: String,
    pub is_snapshot: bool,
    pub status: String,
    pub visibility: String,
    pub protected: bool,
    pub disk_format: Option<String>,
    pub size: Option<u64>,
    pub owned_by_project: bool,
    pub owner_id: Option<String>,
    /// Project name of the owner, when Keystone lets us resolve it.
    pub owner_name: Option<String>,
    /// Custom properties: every key that is not a Glance core field (roadmap 25).
    #[serde(default)]
    pub properties: Vec<(String, String)>,
    #[serde(default)]
    pub min_disk_gb: Option<i64>,
    #[serde(default)]
    pub min_ram_mb: Option<i64>,
    /// Why the last import failed (`os_glance_failed_import`, Glance sets it on
    /// `web-download` / `glance-direct` errors); `None` when it has not (roadmap 45).
    #[serde(default)]
    pub import_failed: Option<String>,
}

impl Image {
    /// A custom property (`description`, `architecture`, `kernel_id`…).
    pub fn property(&self, k: &str) -> Option<&str> {
        self.properties.iter().find(|(p, _)| p == k).map(|(_, v)| v.as_str())
    }
}

/// A row of the Key Pairs list; `key_type` needs compute 2.2 (roadmap 51).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyPair {
    pub name: String,
    pub fingerprint: String,
    pub key_type: Option<String>,
}

/// A row of the Server Groups list. `policies` holds the single `policy` from 2.64.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerGroup {
    pub id: String,
    pub name: String,
    pub policies: Vec<String>,
    pub members: Vec<String>,
}

/// A row of Identity › Projects (roadmap 57): every project of the domain for
/// admins, the user's own otherwise (like Horizon).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityProject {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Domain name when known, else its ID.
    pub domain: String,
    pub enabled: bool,
    /// The session's project (*Set as Active Project* is not offered on it).
    pub current: bool,
    /// The user has a role on it, so it can be made the active project.
    pub member: bool,
}

/// A row of Identity › Users: every user of the domain for admins, only the
/// user themself otherwise (Horizon's `get_user` fallback).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityUser {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub email: Option<String>,
    pub enabled: bool,
    pub domain: String,
}

/// Identity › Projects › a project: Horizon's *Overview*, and for admins its
/// *Users* and *Groups* tabs (roadmap 57).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityProjectDetail {
    pub project: IdentityProject,
    pub domain_id: Option<String>,
    /// `None` for non-admins (Keystone won't list the assignments): no tab.
    pub users: Option<Vec<ProjectMember>>,
    pub groups: Option<Vec<ProjectGroup>>,
}

/// A user with roles on a project, directly or through a group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectMember {
    pub id: String,
    pub name: String,
    /// Roles given to the user on the project.
    pub roles: Vec<String>,
    /// Their IDs, for *Manage Members*.
    #[serde(default)]
    pub role_ids: Vec<String>,
    /// `(role, group)` for roles that come from a group's assignment.
    pub group_roles: Vec<(String, String)>,
}

/// A group with roles on a project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectGroup {
    pub id: String,
    pub name: String,
    pub roles: Vec<String>,
    /// Their IDs, for *Modify Groups*.
    #[serde(default)]
    pub role_ids: Vec<String>,
}

/// Identity › Users › a user: Horizon's *Overview* (roadmap 57).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityUserDetail {
    pub user: IdentityUser,
    pub domain_id: Option<String>,
    /// `None`: the password doesn't expire.
    pub password_expires_at: Option<String>,
    /// Keystone's `options.lock_password`: the user can't change their password.
    pub lock_password: bool,
    /// `default_project_id`, named when the project is known.
    pub primary_project: Option<Ref>,
    /// Other attributes Keystone returns (Horizon lists them too), by key.
    pub extra: Vec<(String, String)>,
    /// Admins only (`None` = no tab): the user's role assignments and groups.
    #[serde(default)]
    pub assignments: Option<Vec<RoleAssignmentRow>>,
    #[serde(default)]
    pub groups: Option<Vec<Ref>>,
    /// Keystone's `default_project_id` as an ID (the edit dialog's preset).
    #[serde(default)]
    pub primary_project_id: Option<String>,
}

/// One of a user's role assignments (Horizon's *Role assignments* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoleAssignmentRow {
    pub role: String,
    /// `project`, `domain` or `system`.
    pub scope_kind: String,
    /// The project or domain name (else ID); `all` for the system.
    pub scope: String,
    /// The project's ID, to link it.
    pub project_id: Option<String>,
    /// Through `OS-INHERIT` (inherited to the scope's projects).
    pub inherited: bool,
}

/// Identity › Groups › a group: its overview and members (roadmap 57).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityGroupDetail {
    pub group: IdentityGroup,
    pub domain_id: Option<String>,
    pub users: Vec<IdentityUser>,
}

/// Identity › Application Credentials › one: Horizon's detail page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppCredentialDetail {
    pub credential: AppCredential,
    pub access_rules: Vec<AccessRule>,
}

/// A row of Identity › Domains (multi-domain clouds).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityDomain {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    /// The session's domain context (*Set Domain Context*).
    pub context: bool,
}

/// A row of Identity › Groups (admins).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityGroup {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub domain: String,
}

/// A row of Identity › Roles (admins).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityRole {
    pub id: String,
    pub name: String,
    /// Roles this one implies (`role_inferences`), by name.
    #[serde(default)]
    pub implies: Vec<String>,
}

/// A row of Identity › Application Credentials: the user's own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppCredential {
    pub id: String,
    pub name: String,
    pub project_id: Option<String>,
    pub description: Option<String>,
    /// `None` = never expires.
    pub expires_at: Option<String>,
    pub roles: Vec<String>,
    pub unrestricted: bool,
}

/// *Create Application Credential* (roadmap 57), as Horizon's form. `user_id`
/// is set by `run_action` from the session: users create their own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppCredentialSpec {
    #[serde(default)]
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    /// `None`: Keystone generates one.
    pub secret: Option<String>,
    /// ISO 8601 in UTC (`2026-12-31T00:00:00`); `None` = never expires.
    pub expires_at: Option<String>,
    /// Role names, a subset of the user's on the project; empty = all of them.
    pub roles: Vec<String>,
    /// Lets the credential create other credentials and trusts (Horizon warns: dangerous).
    pub unrestricted: bool,
    /// Access rules as typed (a JSON or YAML list of `{service, method, path}`),
    /// parsed and checked on the server; `None` = no restriction.
    #[serde(default)]
    pub access_rules: Option<String>,
}

/// Horizon's *Create User*: a password typed by the admin, and optionally a
/// primary project with a role on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserSpec {
    /// Set by `run_action` (see [`OsAction::set_domain`]).
    #[serde(default)]
    pub domain_id: String,
    pub name: String,
    pub description: Option<String>,
    pub email: Option<String>,
    pub password: String,
    pub project_id: Option<String>,
    /// Granted on `project_id` once the user exists.
    pub role_id: Option<String>,
    pub enabled: bool,
    pub lock_password: bool,
}

/// One row of *Manage Members* / *Modify Groups*: a user or group and its role
/// IDs on the project (the field's value is a JSON list of these).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssignmentRow {
    pub actor_id: String,
    pub label: String,
    pub role_ids: Vec<String>,
}

/// The grants and revokes that turn `before` into `after` on `project_id`, as
/// `(label, action)` for a batch; `group` says whether actors are groups.
pub fn assignment_changes(
    project_id: &str,
    group: bool,
    before: &[AssignmentRow],
    after: &[AssignmentRow],
) -> Vec<(String, OsAction)> {
    let roles_of = |rows: &[AssignmentRow], id: &str| {
        rows.iter()
            .find(|r| r.actor_id == id)
            .map(|r| r.role_ids.clone())
            .unwrap_or_default()
    };
    let actor = |id: &str| {
        if group {
            (None, Some(id.to_string()))
        } else {
            (Some(id.to_string()), None)
        }
    };
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for row in after.iter().chain(before) {
        if !seen.insert(row.actor_id.as_str()) {
            continue;
        }
        let (old, new) = (roles_of(before, &row.actor_id), roles_of(after, &row.actor_id));
        for r in new.iter().filter(|r| !old.contains(r)) {
            let (user_id, group_id) = actor(&row.actor_id);
            out.push((
                format!("+ {} {r}", row.label),
                OsAction::ProjectRoleGrant {
                    project_id: project_id.into(),
                    user_id,
                    group_id,
                    role_id: r.clone(),
                },
            ));
        }
        for r in old.iter().filter(|r| !new.contains(r)) {
            let (user_id, group_id) = actor(&row.actor_id);
            out.push((
                format!("- {} {r}", row.label),
                OsAction::ProjectRoleRevoke {
                    project_id: project_id.into(),
                    user_id,
                    group_id,
                    role_id: r.clone(),
                },
            ));
        }
    }
    out
}

/// One application credential access rule (Keystone API 3.13).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccessRule {
    pub service: String,
    pub method: String,
    pub path: String,
}

impl AccessRule {
    /// Keystone's schema: a service type, a known HTTP method, a path from `/`.
    pub fn check(&self) -> Result<(), String> {
        if self.service.is_empty() || !self.service.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(format!("access rule: not a service type: {:?}", self.service));
        }
        one_of(
            &self.method,
            &["DELETE", "GET", "HEAD", "PATCH", "POST", "PUT"],
            "access rule method",
        )?;
        if !self.path.starts_with('/') || self.path.len() > 225 {
            return Err(format!("access rule: the path must start with / (got {:?})", self.path));
        }
        Ok(())
    }
}

/// Horizon's expiration rule: a date alone means midnight, a time alone means
/// today, both combine; neither = never. All in UTC, like `now` (the current UTC
/// time). The result must be later than `now`: "today" alone, or a time that
/// already passed, would be in the past. Returns `YYYY-MM-DDTHH:MM:SS`.
pub fn app_credential_expiry(date: &str, time: &str, now: chrono::NaiveDateTime) -> Result<Option<String>, String> {
    let today = now.date();
    let (date, time) = (date.trim(), time.trim());
    if date.is_empty() && time.is_empty() {
        return Ok(None);
    }
    let d = if date.is_empty() {
        today
    } else {
        chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| format!("not a date (YYYY-MM-DD): {date}"))?
    };
    let t = if time.is_empty() {
        chrono::NaiveTime::MIN
    } else {
        chrono::NaiveTime::parse_from_str(time, "%H:%M")
            .or_else(|_| chrono::NaiveTime::parse_from_str(time, "%H:%M:%S"))
            .map_err(|_| format!("not a time (HH:MM): {time}"))?
    };
    let at = d.and_time(t);
    if at <= now {
        return Err(format!(
            "the expiration ({} UTC) must be later than now ({} UTC)",
            at.format("%Y-%m-%d %H:%M"),
            now.format("%Y-%m-%d %H:%M")
        ));
    }
    Ok(Some(at.format("%Y-%m-%dT%H:%M:%S").to_string()))
}

/// The server group detail page (roadmap 52).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerGroupDetail {
    pub group: ServerGroup,
    /// Policy rules, e.g. `("max_server_per_host", "2")`; only compute 2.64+
    /// returns them, so empty on older clouds and when none were set.
    pub rules: Vec<(String, String)>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub members: Vec<GroupMember>,
}

/// An instance of a server group. `name`/`status` are `None` when the server
/// list doesn't have it (deleted, or the list failed): the page shows the ID.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupMember {
    pub id: String,
    pub name: Option<String>,
    pub status: Option<String>,
}

/// Where a volume is attached (server and device), names resolved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeAttachment {
    pub server_id: String,
    pub server_name: Option<String>,
    pub device: Option<String>,
}

/// A row of the Volumes list (Cinder v3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Volume {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub size_gb: i64,
    pub status: String,
    pub volume_type: Option<String>,
    pub attachments: Vec<VolumeAttachment>,
    pub availability_zone: Option<String>,
    pub bootable: bool,
    pub encrypted: bool,
    /// Cinder volume metadata (roadmap 25).
    #[serde(default)]
    pub metadata: Vec<(String, String)>,
}

/// A row of the Volume Snapshots list (Cinder v3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub size_gb: i64,
    pub status: String,
    pub volume_id: String,
    pub volume_name: Option<String>,
    pub created: String,
    /// Cinder snapshot metadata (roadmap 25).
    #[serde(default)]
    pub metadata: Vec<(String, String)>,
}

/// A row of Volumes › Backups (Cinder, roadmap 5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackupRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub size_gb: i64,
    /// The volume it was taken of; the name falls back to the ID (deleted or unlisted).
    pub volume: Ref,
    /// `true` for an incremental backup (`is_incremental`).
    pub incremental: bool,
    /// Swift container holding the data (`None` with other backends).
    pub container: Option<String>,
    /// Source snapshot, when the backup was made from one.
    pub snapshot_id: Option<String>,
    pub availability_zone: Option<String>,
    /// Why the backup failed (`fail_reason`), if it did.
    pub fail_reason: Option<String>,
    pub created: String,
}

/// A row of the Networks list: the project's own, shared and external networks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetworkRow {
    pub id: String,
    pub name: String,
    /// "name cidr"
    pub subnets: Vec<String>,
    pub shared: bool,
    pub external: bool,
    pub status: String,
    pub admin_up: bool,
    pub availability_zones: Vec<String>,
    /// The session's project owns it: only then are there write actions
    /// (external and shared networks of other projects are read-only, as in Horizon).
    #[serde(default)]
    pub owned: bool,
    /// DHCP agents hosting it (Admin › Networks, Neutron `dhcp_agent_scheduler`);
    /// `None` on Project's list or without the extension.
    #[serde(default)]
    pub dhcp_agents: Option<usize>,
    /// `provider:*`, which Neutron shows to admins only; `None` otherwise.
    #[serde(default)]
    pub provider: Option<ProviderNetwork>,
}

/// A row of the Routers list (filtered by `tenant_id`: admins see every project's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouterRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub external_network: Option<String>,
    pub external_network_id: Option<String>,
    /// The gateway's `enable_snat`; `None` without a gateway.
    pub snat: Option<bool>,
    pub admin_up: bool,
    pub availability_zones: Vec<String>,
    /// `distributed` / `ha`: Neutron shows them to admins only, and only with
    /// the `dvr` / `l3-ha` extensions; `None` when absent (roadmap 89).
    #[serde(default)]
    pub distributed: Option<bool>,
    #[serde(default)]
    pub ha: Option<bool>,
}

/// An Octavia load balancer, as on Horizon's *Load Balancers* list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoadBalancerRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub vip_address: Option<String>,
    pub availability_zone: Option<String>,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
    pub provider: Option<String>,
}

/// A trunk's sub-port: a port carried on the parent's VLAN `segmentation_id`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subport {
    pub port_id: String,
    /// The port's name, else its ID.
    pub port_name: String,
    pub segmentation_type: String,
    pub segmentation_id: Option<i64>,
}

/// A Neutron trunk (`trunk` extension).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrunkRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// The parent port.
    pub parent: Ref,
    pub status: String,
    pub admin_up: bool,
    pub subports: Vec<Subport>,
}

// --- Firewall (FWaaS v2, roadmap 93) --------------------------------------------

/// A FWaaS v2 firewall rule. `None` addresses, ports and protocol mean *any*.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirewallRuleRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// `tcp`, `udp`, `icmp`; `None` = any protocol.
    pub protocol: Option<String>,
    /// 4 or 6.
    pub ip_version: u8,
    pub source_ip: Option<String>,
    pub source_port: Option<String>,
    pub destination_ip: Option<String>,
    pub destination_port: Option<String>,
    /// `allow`, `deny` or `reject`.
    pub action: String,
    pub enabled: bool,
    pub shared: bool,
    /// The policies that use it (FWaaS v2 lets a rule be in several), named
    /// from the policies the project can see, else their IDs.
    pub policies: Vec<Ref>,
    /// The project's own; another project's shared rule has no edit or delete.
    pub owned: bool,
}

/// A FWaaS v2 firewall policy: an ordered list of rules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirewallPolicyRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// In evaluation order, named from the rules the project can see.
    pub rules: Vec<Ref>,
    pub audited: bool,
    pub shared: bool,
    pub owned: bool,
}

/// A firewall policy's page: the policy and its rules in full, in evaluation order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirewallPolicyDetail {
    pub policy: FirewallPolicyRow,
    /// Rules the project can't see (another project's, not shared) are left out.
    pub rules: Vec<FirewallRuleRow>,
}

/// A port a firewall group protects, with the router or instance it belongs to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirewallPort {
    /// The port, by name (else ID).
    pub port: Ref,
    /// Its fixed IPs.
    pub ips: Vec<String>,
    /// `router` or `instance` (from `device_owner`), `None` when unknown.
    pub kind: Option<String>,
    /// The router or instance, by name when known.
    pub device: Option<Ref>,
}

impl FirewallPort {
    /// Where the device's page is (`None` for an unknown kind).
    pub fn device_href(&self) -> Option<String> {
        let d = self.device.as_ref()?;
        match self.kind.as_deref()? {
            "router" => Some(format!("/project/routers/{}", d.id)),
            "instance" => Some(format!("/project/instances/{}", d.id)),
            _ => None,
        }
    }
}

/// A FWaaS v2 firewall group: an ingress and an egress policy applied to ports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirewallGroupRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub ingress: Option<Ref>,
    pub egress: Option<Ref>,
    /// Resolved to their router or instance on the detail page; the list
    /// only names the ports.
    pub ports: Vec<FirewallPort>,
    /// `ACTIVE`, `INACTIVE` (no ports, or neither policy), `DOWN`, `PENDING_CREATE` / `_UPDATE` / `_DELETE`, `ERROR`.
    pub status: String,
    pub admin_up: bool,
    pub shared: bool,
    pub owned: bool,
}

impl FirewallGroupRow {
    /// Neutron moves it through `PENDING_*` on its own: the list polls meanwhile.
    pub fn busy(&self) -> bool {
        self.status.starts_with("PENDING")
    }
    /// Neutron's per-project `default` group (neutron-fwaas matches it by name): only its
    /// ports can change, and only an admin can delete it (`FirewallGroupCannotRemoveDefault`,
    /// `FirewallDefaultObjectUpdateRestricted`).
    pub fn is_default(&self) -> bool {
        self.name == "default"
    }
    /// Can be deleted from the Project panel: the project's own, not `default`.
    pub fn deletable(&self) -> bool {
        self.owned && !self.is_default()
    }
}

impl FirewallPolicyRow {
    /// The default group's `default ingress` / `default egress` policies (matched by name, as
    /// neutron-fwaas does): Neutron refuses to update them; their rules can still change.
    pub fn is_default(&self) -> bool {
        matches!(self.name.as_str(), "default ingress" | "default egress")
    }
    /// Can be deleted from the Project panel: the project's own, not a default policy (the
    /// default group uses it, so Neutron would refuse anyway).
    pub fn deletable(&self) -> bool {
        self.owned && !self.is_default()
    }
}

/// What *Add Rule* and *Edit Rule* send. Empty strings mean *any*.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FirewallRuleSpec {
    pub name: String,
    pub description: String,
    /// `tcp`, `udp`, `icmp`, or `any` / empty.
    pub protocol: String,
    /// `allow`, `deny` or `reject`.
    pub action: String,
    /// 4 or 6.
    pub ip_version: u8,
    pub source_ip: String,
    pub source_port: String,
    pub destination_ip: String,
    pub destination_port: String,
    pub enabled: bool,
}

impl FirewallRuleSpec {
    /// The `firewall_rule` body Neutron expects, validated: protocol and action
    /// from Neutron's lists, ports (`22`, `8000:8080`) only for TCP or UDP, and
    /// addresses (an IP or a CIDR) of the rule's IP version. *Any* is `null`, so
    /// an update clears a field.
    pub fn body(&self) -> Result<serde_json::Value, String> {
        use serde_json::{Value, json};
        let protocol = match self.protocol.trim() {
            "" | "any" => None,
            p @ ("tcp" | "udp" | "icmp") => Some(p),
            other => return Err(format!("protocol must be tcp, udp, icmp or any (got {other})")),
        };
        if !matches!(self.action.as_str(), "allow" | "deny" | "reject") {
            return Err(format!("action must be allow, deny or reject (got {})", self.action));
        }
        if !matches!(self.ip_version, 4 | 6) {
            return Err(format!("IP version must be 4 or 6 (got {})", self.ip_version));
        }
        let addr = |a: &str| -> Result<Value, String> {
            let a = a.trim();
            if a.is_empty() {
                return Ok(Value::Null);
            }
            validate_ip_or_cidr(a, self.ip_version)?;
            Ok(json!(a))
        };
        // Neutron refuses ports on ICMP or *any* protocol rules.
        let port = |p: &str| -> Result<Value, String> {
            let p = p.trim();
            if p.is_empty() {
                return Ok(Value::Null);
            }
            if !matches!(protocol, Some("tcp" | "udp")) {
                return Err("ports need the TCP or UDP protocol".into());
            }
            validate_port_range(p)?;
            Ok(json!(p))
        };
        let mut r = serde_json::Map::new();
        r.insert("name".into(), json!(self.name.trim()));
        r.insert("description".into(), json!(self.description.trim()));
        r.insert("protocol".into(), json!(protocol));
        r.insert("action".into(), json!(self.action));
        r.insert("ip_version".into(), json!(self.ip_version));
        r.insert("source_ip_address".into(), addr(&self.source_ip)?);
        r.insert("source_port".into(), port(&self.source_port)?);
        r.insert("destination_ip_address".into(), addr(&self.destination_ip)?);
        r.insert("destination_port".into(), port(&self.destination_port)?);
        r.insert("enabled".into(), json!(self.enabled));
        Ok(json!({ "firewall_rule": r }))
    }
}

/// A port (`22`) or a range (`8000:8080`, low to high), 1-65535.
pub fn validate_port_range(p: &str) -> Result<(), String> {
    let one = |x: &str| x.trim().parse::<u16>().ok().filter(|n| *n > 0);
    let bad = || format!("not a port or a port range (22, 8000:8080): {p}");
    match p.split_once(':') {
        None => one(p).map(|_| ()).ok_or_else(bad),
        Some((a, b)) => match (one(a), one(b)) {
            (Some(a), Some(b)) if a <= b => Ok(()),
            _ => Err(bad()),
        },
    }
}

/// An IP address or a CIDR of IP version `version` (4 or 6).
pub fn validate_ip_or_cidr(a: &str, version: u8) -> Result<(), String> {
    use std::net::IpAddr;
    let (ip, len) = match a.split_once('/') {
        Some((ip, len)) => (ip, Some(len)),
        None => (a, None),
    };
    let ip: IpAddr = ip.parse().map_err(|_| format!("not an IP address or a CIDR: {a}"))?;
    let max = if ip.is_ipv4() { 32 } else { 128 };
    if let Some(len) = len {
        len.parse::<u8>()
            .ok()
            .filter(|l| *l <= max)
            .ok_or_else(|| format!("not an IP address or a CIDR: {a}"))?;
    }
    if (version == 4) != ip.is_ipv4() {
        return Err(format!("{a} is not an IPv{version} address"));
    }
    Ok(())
}

// --- Orchestration (Heat, roadmap 62) ------------------------------------------

/// A Heat stack, as the *Stacks* list shows it. Heat's times are UTC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackRow {
    pub id: String,
    /// `stack_name` (Heat requires one).
    pub name: String,
    pub description: Option<String>,
    /// Heat's `stack_status`: action and state, e.g. `CREATE_COMPLETE`.
    pub status: String,
    pub status_reason: Option<String>,
    pub created: String,
    /// `None` until the stack is first updated.
    pub updated: Option<String>,
}

/// Whether Heat is still working on the stack or resource (`*_IN_PROGRESS`):
/// pages poll while it is (roadmap 46).
pub fn stack_busy(status: &str) -> bool {
    status.ends_with("_IN_PROGRESS")
}

/// Whether *Resume Stack* applies: the stack is suspended, or a resume failed.
pub fn stack_suspended(status: &str) -> bool {
    matches!(status, "SUSPEND_COMPLETE" | "RESUME_FAILED")
}

/// A stack as one string, `name/id`, for the list's selection (whose key is a
/// single string) and back: Heat names can't contain `/`.
pub fn stack_key(name: &str, id: &str) -> String {
    format!("{name}/{id}")
}

/// [`stack_key`]'s parts; empty ones (refused by the server) when malformed.
pub fn split_stack_key(key: &str) -> (String, String) {
    let (name, id) = key.split_once('/').unwrap_or_default();
    (name.to_string(), id.to_string())
}

/// A row of Orchestration › Resource Types (`GET resource_types?with_description=true`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceTypeRow {
    /// `OS::Nova::Server`, also the detail page's path segment.
    pub name: String,
    /// `None` when Heat lists bare names (before `with_description`, Pike).
    pub description: Option<String>,
}

/// A resource type's schema (`GET resource_types/{type}`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceTypeDetail {
    pub name: String,
    pub description: Option<String>,
    /// `support_status.status` (`SUPPORTED`, `DEPRECATED`…) with the version it changed in.
    pub support: Option<String>,
    /// Top-level properties, by name; nested schemas are in `raw`.
    pub properties: Vec<TypeProperty>,
    /// Attributes (`get_attr`), by name.
    pub attributes: Vec<TypeAttribute>,
    /// The whole schema, pretty JSON, for what the tables leave out.
    pub raw: String,
}

/// One property of a resource type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypeProperty {
    pub name: String,
    /// Heat's `type`: `string`, `number`, `map`, `list`, `boolean`, `integer`.
    pub kind: String,
    pub required: bool,
    /// `update_allowed`: changed in place by a stack update (else replaced).
    pub update_allowed: bool,
    /// The default as text; `None` when there is none.
    pub default: Option<String>,
    pub description: Option<String>,
    /// Each constraint compact (`allowed_values: [a, b]`, `custom_constraint: nova.flavor`).
    pub constraints: Vec<String>,
    /// The `allowed_values` constraint's values as text, for a select in the
    /// Template Generator (roadmap 92); empty when unconstrained.
    #[serde(default)]
    pub allowed: Vec<String>,
    /// The `custom_constraint` (`neutron.network`, `nova.flavor`): what the value
    /// names, so the Template Generator can offer a `get_resource` to a matching resource.
    #[serde(default)]
    pub custom: Option<String>,
}

/// One attribute of a resource type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypeAttribute {
    pub name: String,
    /// `None` on Heat releases that don't report it.
    pub kind: Option<String>,
    pub description: Option<String>,
}

/// A row of Orchestration › Template Versions (`GET template_versions`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateVersion {
    /// `heat_template_version.2018-08-31`, also the detail page's path segment.
    pub version: String,
    /// `hot` or `cfn`.
    pub kind: String,
    /// Other names for it (`heat_template_version.rocky`).
    pub aliases: Vec<String>,
}

/// An intrinsic function of a template version (`GET template_versions/{v}/functions`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateFunction {
    pub name: String,
    pub description: Option<String>,
}

/// Where a stack's template comes from (*Launch Stack*, *Change Stack Template*,
/// validation, preview).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplateSource {
    /// Template text, YAML or JSON (Heat parses either); unused when `url` is set.
    pub template: String,
    /// `template_url`: Heat downloads the template itself.
    pub url: Option<String>,
    /// Environment text (YAML), if any. In an update, none keeps the stack's.
    pub environment: Option<String>,
}

/// The largest template plus environment Moonlight sends, in bytes: Heat's
/// default `max_template_size` (512 KiB).
pub const MAX_TEMPLATE_BYTES: usize = 512 * 1024;

/// A stack to create, or the new template and values of one (*Change Stack Template*).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StackSpec {
    /// `stack_name`: a letter, then letters, digits, `_`, `-` or `.`; part of
    /// Heat's paths. Unchanged by an update.
    pub name: String,
    pub source: TemplateSource,
    /// Parameter values as text (Heat converts them). Left out: the template's
    /// default, or in an update the stack's current value.
    pub parameters: Vec<(String, String)>,
    /// Hidden (`NoEcho`) parameters, masked in request previews.
    pub hidden: Vec<String>,
    /// `timeout_mins`; `None` is Heat's default (60).
    pub timeout_mins: Option<u32>,
    pub disable_rollback: bool,
}

/// What Heat's `POST validate` says about a template: its description and
/// parameters. `local` when Moonlight read them from the template itself
/// (dry-run, where the request is only previewed).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub description: Option<String>,
    pub params: Vec<TemplateParam>,
    pub local: bool,
}

/// One template parameter, as the launch form shows it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateParam {
    pub name: String,
    pub label: Option<String>,
    pub description: Option<String>,
    /// `string`, `number`, `json`, `comma_delimited_list` or `boolean`.
    pub kind: String,
    /// The default as text; `None` when the parameter has none (then required).
    pub default: Option<String>,
    /// `hidden` (`NoEcho`): a password field, masked in previews.
    pub hidden: bool,
    /// `allowed_values`: the form offers a list instead of a text box.
    pub allowed: Vec<String>,
}

/// A resource *Preview Stack* says the template would create; `depth` > 0
/// inside nested stacks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreviewResource {
    pub name: String,
    pub kind: String,
    pub depth: u32,
}

/// Whether `name` is a valid Heat stack name (Heat's own rule).
pub fn valid_stack_name(name: &str) -> bool {
    let mut c = name.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic())
        && name.len() <= 255
        && c.all(|x| x.is_ascii_alphanumeric() || matches!(x, '_' | '-' | '.'))
}

/// One output of a stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackOutput {
    pub key: String,
    /// Strings as they are, other values as JSON; `-` when Heat sends none.
    pub value: String,
    pub description: Option<String>,
    /// `output_error`: why Heat could not resolve the value.
    pub error: Option<String>,
}

/// A resource of a stack (*Resources* tab; top level only).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackResource {
    /// `resource_name`, the template's logical name.
    pub name: String,
    /// The ID in its own service; `None` before it is created.
    pub physical_id: Option<String>,
    /// `OS::Nova::Server`, a nested template's file name…
    pub resource_type: String,
    pub status: String,
    pub status_reason: Option<String>,
    pub updated: String,
    /// The resources that depend on this one (`required_by`).
    pub required_by: Vec<String>,
    /// The resources this one depends on: the inverse of `required_by`.
    pub depends_on: Vec<String>,
    /// Moonlight's page for the physical resource, when there is one.
    pub href: Option<String>,
}

/// One stack event (*Events* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackEvent {
    pub id: String,
    pub time: String,
    pub resource_name: String,
    pub physical_id: Option<String>,
    pub status: String,
    pub status_reason: Option<String>,
}

/// The stack detail page: Horizon's *Topology*, *Overview*, *Resources*,
/// *Events* and *Template* tabs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackDetail {
    pub stack: StackRow,
    /// The stack's parameters, sorted; Heat's own `OS::*` ones left out.
    /// Heat masks hidden parameters itself (`******`).
    pub parameters: Vec<(String, String)>,
    pub outputs: Vec<StackOutput>,
    pub timeout_mins: Option<i64>,
    pub disable_rollback: bool,
    /// The parent stack's ID, for a nested stack.
    pub parent: Option<String>,
    pub tags: Vec<String>,
    pub resources: Vec<StackResource>,
    /// Newest first.
    pub events: Vec<StackEvent>,
    /// The template as Heat returns it (JSON), pretty-printed.
    pub template: String,
}

// --- Alarming (Aodh, roadmap 79) ----------------------------------------------

/// An Aodh alarm, as the *Alarms* list shows it. Aodh's times have no zone; they are UTC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlarmRow {
    /// `alarm_id`.
    pub id: String,
    /// The name (Aodh requires one), else the ID.
    pub name: String,
    pub description: Option<String>,
    /// Aodh's `type`: `prometheus`, `gnocchi_resources_threshold`, `event`, `composite`…
    pub alarm_type: String,
    /// `ok`, `alarm` or `insufficient data`.
    pub state: String,
    pub state_reason: Option<String>,
    /// When the state last changed.
    pub state_since: Option<String>,
    /// `low`, `moderate` or `critical`; `None` when unset.
    pub severity: Option<String>,
    pub enabled: bool,
    /// What the alarm watches, one line and language-neutral
    /// (`mean(cpu) > 80`, the PromQL query, the event type); `-` when unknown.
    pub condition: String,
    /// When the alarm last changed (Aodh's `timestamp`).
    pub updated: Option<String>,
    /// The alarm as *Edit Alarm* starts from (the list's GET returns whole alarms).
    pub spec: AlarmSpec,
}

/// An alarm's rule, per type. Aodh keeps it under `<type>_rule`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AlarmRule {
    /// Gnocchi threshold alarms: `gnocchi_resources_threshold` (one resource),
    /// `gnocchi_aggregation_by_metrics_threshold` (`metrics` IDs) and
    /// `gnocchi_aggregation_by_resources_threshold` (resources matching `query`).
    Threshold {
        metrics: Vec<String>,
        resource_type: Option<String>,
        resource_id: Option<String>,
        /// The resource's page in Moonlight, when it has one (an instance).
        resource_href: Option<String>,
        /// The Gnocchi search of `…_by_resources_…`, as Aodh stores it (JSON text).
        query: Option<String>,
        aggregation: Option<String>,
        /// Aodh's operator (`gt`, `le`…).
        comparison: Option<String>,
        threshold: Option<f64>,
        /// Seconds.
        granularity: Option<i64>,
        evaluation_periods: Option<i64>,
    },
    /// `prometheus`: a PromQL query compared with a threshold (RHOSO's telemetry).
    Prometheus {
        query: String,
        comparison: Option<String>,
        threshold: Option<f64>,
    },
    /// `event`: fires on a notification of `event_type` matching `query`.
    Event { event_type: String, query: Vec<String> },
    /// Any other type (`composite`, `loadbalancer_member_health`…): the rule as
    /// Aodh returns it, pretty-printed JSON; empty when the alarm has none.
    Raw(String),
}

/// A time window in which the alarm is evaluated (`time_constraints`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlarmTimeConstraint {
    pub name: String,
    /// Cron expression of the window's start.
    pub start: String,
    /// Seconds.
    pub duration: Option<i64>,
    pub timezone: Option<String>,
    pub description: Option<String>,
}

/// One entry of an alarm's history (`GET v2/alarms/{id}/history`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlarmChange {
    pub time: String,
    /// `creation`, `rule change`, `state transition` or `deletion`.
    pub kind: String,
    /// The new state, for a state transition.
    pub state: Option<String>,
    /// A state transition's reason, or a rule change's new fields (compact JSON).
    pub detail: Option<String>,
    /// Who made the change; `None` for the evaluator's state transitions.
    pub user_id: Option<String>,
}

/// The alarm detail page: *Overview* and *History*.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlarmDetail {
    pub alarm: AlarmRow,
    pub rule: AlarmRule,
    /// URLs called on entering `alarm`, `ok` and `insufficient data`.
    pub alarm_actions: Vec<String>,
    pub ok_actions: Vec<String>,
    pub insufficient_data_actions: Vec<String>,
    /// Call the actions on every evaluation while in the state, not only on entering it.
    pub repeat_actions: bool,
    pub time_constraints: Vec<AlarmTimeConstraint>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    /// Newest first.
    pub history: Vec<AlarmChange>,
}

/// Aodh's comparison operator as a symbol (`gt` → `>`); unknown ones as they are.
pub fn comparison_symbol(op: &str) -> &str {
    match op {
        "lt" => "<",
        "le" => "≤",
        "eq" => "=",
        "ne" => "≠",
        "ge" => "≥",
        "gt" => ">",
        other => other,
    }
}

/// Alarm types the *Create Alarm* form offers (roadmap 79); the list and detail show every type.
pub const ALARM_TYPES: [&str; 5] = [
    "prometheus",
    "gnocchi_resources_threshold",
    "gnocchi_aggregation_by_metrics_threshold",
    "gnocchi_aggregation_by_resources_threshold",
    "event",
];
/// Aodh's severities.
pub const ALARM_SEVERITIES: [&str; 3] = ["low", "moderate", "critical"];
/// Aodh's comparison operators.
pub const ALARM_COMPARISONS: [&str; 6] = ["gt", "ge", "eq", "ne", "le", "lt"];
/// Gnocchi's usual aggregation methods (`rate:` variants included); other
/// values are accepted too, since archive policies can define more.
pub const ALARM_AGGREGATIONS: [&str; 11] = [
    "mean",
    "max",
    "min",
    "sum",
    "last",
    "first",
    "median",
    "std",
    "count",
    "rate:mean",
    "rate:last",
];
/// Notifier schemes Aodh can call (`test://` left out: it only exists for Aodh's own tests).
pub const ALARM_ACTION_SCHEMES: [&str; 7] = [
    "http://",
    "https://",
    "trust+http://",
    "trust+https://",
    "log://",
    "zaqar://",
    "trust+zaqar://",
];

/// An alarm to create (`POST v2/alarms`) or replace (`PUT v2/alarms/{id}`):
/// Aodh's PUT replaces the whole alarm, so an edit sends every field, including
/// the time constraints the form doesn't edit. [`AlarmSpec::body`] keeps only
/// the rule fields of `alarm_type` and checks them, in the dialog and on the server.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AlarmSpec {
    pub name: String,
    pub description: String,
    /// One of [`ALARM_TYPES`].
    pub alarm_type: String,
    /// One of [`ALARM_SEVERITIES`].
    pub severity: String,
    pub enabled: bool,
    pub repeat_actions: bool,
    /// URLs, each with one of [`ALARM_ACTION_SCHEMES`].
    pub alarm_actions: Vec<String>,
    pub ok_actions: Vec<String>,
    pub insufficient_data_actions: Vec<String>,
    /// Kept as they are (not edited by the form).
    pub time_constraints: Vec<AlarmTimeConstraint>,
    /// `prometheus`: the PromQL query; `…_by_resources_threshold`: the Gnocchi
    /// search, JSON text (Aodh stores it as a string).
    pub query: String,
    /// One of [`ALARM_COMPARISONS`] (threshold types).
    pub comparison: String,
    pub threshold: Option<f64>,
    /// Gnocchi: the metric name (`…_resources_…`, `…_by_resources_…`).
    pub metric: String,
    /// `…_by_metrics_threshold`: Gnocchi metric IDs.
    pub metrics: Vec<String>,
    pub resource_type: String,
    /// `gnocchi_resources_threshold`: the Gnocchi resource (for `instance`, the server ID).
    pub resource_id: String,
    pub aggregation: String,
    /// Seconds.
    pub granularity: Option<i64>,
    pub evaluation_periods: Option<i64>,
    /// `event`: the notification type (`*` = any).
    pub event_type: String,
    /// `event`: trait filters, one `field op value` each (`traits.instance_id = abc`).
    pub event_query: Vec<String>,
}

impl AlarmSpec {
    /// Aodh's alarm body, or what is wrong with the spec.
    pub fn body(&self) -> Result<serde_json::Value, String> {
        use serde_json::{Map, Value, json};
        let req = |v: &str, what: &str| {
            if v.trim().is_empty() {
                Err(format!("{what} is required"))
            } else {
                Ok(v.trim().to_string())
            }
        };
        let name = req(&self.name, "name")?;
        one_of(&self.alarm_type, &ALARM_TYPES, "alarm type")?;
        one_of(&self.severity, &ALARM_SEVERITIES, "severity")?;
        for url in self
            .alarm_actions
            .iter()
            .chain(&self.ok_actions)
            .chain(&self.insufficient_data_actions)
        {
            if !ALARM_ACTION_SCHEMES
                .iter()
                .any(|s| url.starts_with(s) && url.len() > s.len() || *s == "log://" && url == "log://")
            {
                return Err(format!(
                    "action {url} must start with one of {}",
                    ALARM_ACTION_SCHEMES.join(", ")
                ));
            }
        }
        let mut rule = Map::new();
        let mut put = |k: &str, v: Value| {
            rule.insert(k.into(), v);
        };
        let gnocchi = self.alarm_type.starts_with("gnocchi_");
        if gnocchi || self.alarm_type == "prometheus" {
            one_of(&self.comparison, &ALARM_COMPARISONS, "comparison")?;
            let t = self
                .threshold
                .filter(|t| t.is_finite())
                .ok_or("threshold is required")?;
            put("comparison_operator", json!(self.comparison));
            put("threshold", json!(t));
        }
        if gnocchi {
            put("aggregation_method", json!(req(&self.aggregation, "aggregation")?));
            put(
                "granularity",
                json!(
                    self.granularity
                        .filter(|g| *g > 0)
                        .ok_or("granularity must be above 0")?
                ),
            );
            put(
                "evaluation_periods",
                json!(
                    self.evaluation_periods
                        .filter(|n| *n > 0)
                        .ok_or("evaluation periods must be above 0")?
                ),
            );
        }
        match self.alarm_type.as_str() {
            "prometheus" => put("query", json!(req(&self.query, "query")?)),
            "gnocchi_resources_threshold" => {
                put("metric", json!(req(&self.metric, "metric")?));
                put("resource_type", json!(req(&self.resource_type, "resource type")?));
                put("resource_id", json!(req(&self.resource_id, "resource ID")?));
            }
            "gnocchi_aggregation_by_metrics_threshold" => {
                let metrics: Vec<&str> = self
                    .metrics
                    .iter()
                    .map(|m| m.trim())
                    .filter(|m| !m.is_empty())
                    .collect();
                if metrics.is_empty() {
                    return Err("at least one metric ID is required".into());
                }
                put("metrics", json!(metrics));
            }
            "gnocchi_aggregation_by_resources_threshold" => {
                put("metric", json!(req(&self.metric, "metric")?));
                put("resource_type", json!(req(&self.resource_type, "resource type")?));
                let q = req(&self.query, "query")?;
                serde_json::from_str::<Value>(&q).map_err(|e| format!("query is not valid JSON: {e}"))?;
                put("query", json!(q));
            }
            _ => {
                let event_type = Some(self.event_type.trim()).filter(|e| !e.is_empty()).unwrap_or("*");
                let query = self
                    .event_query
                    .iter()
                    .map(|q| event_filter(q))
                    .collect::<Result<Vec<_>, _>>()?;
                put("event_type", json!(event_type));
                put("query", json!(query));
            }
        }
        let constraints: Vec<Value> = self
            .time_constraints
            .iter()
            .map(|c| {
                let mut m = Map::new();
                m.insert("name".into(), json!(c.name));
                m.insert("start".into(), json!(c.start));
                for (k, v) in [
                    ("duration", c.duration.map(|d| json!(d))),
                    ("timezone", c.timezone.as_ref().map(|z| json!(z))),
                    ("description", c.description.as_ref().map(|d| json!(d))),
                ] {
                    if let Some(v) = v {
                        m.insert(k.into(), v);
                    }
                }
                Value::Object(m)
            })
            .collect();
        let mut body = json!({
            "name": name,
            "description": self.description.trim(),
            "type": self.alarm_type,
            "severity": self.severity,
            "enabled": self.enabled,
            "repeat_actions": self.repeat_actions,
            "alarm_actions": self.alarm_actions,
            "ok_actions": self.ok_actions,
            "insufficient_data_actions": self.insufficient_data_actions,
            "time_constraints": constraints,
        });
        body[format!("{}_rule", self.alarm_type)] = Value::Object(rule);
        Ok(body)
    }
}

/// One event trait filter, `field op value` with `op` as a symbol (`=`, `!=`,
/// `≠`, `<`, `<=`, `≤`, `>`, `>=`, `≥`) or Aodh's name (`eq`…), as the detail
/// page shows them → Aodh's `{field, op, value}`.
fn event_filter(line: &str) -> Result<serde_json::Value, String> {
    let mut parts = line.trim().splitn(3, char::is_whitespace).filter(|p| !p.is_empty());
    let (Some(field), Some(op), Some(value)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(format!("filter {line:?} must be `field op value`"));
    };
    let op = match op {
        "=" | "==" | "eq" => "eq",
        "!=" | "≠" | "ne" => "ne",
        "<" | "lt" => "lt",
        "<=" | "≤" | "le" => "le",
        ">" | "gt" => "gt",
        ">=" | "≥" | "ge" => "ge",
        other => return Err(format!("unknown operator {other} in filter {line:?}")),
    };
    Ok(serde_json::json!({"field": field, "op": op, "value": value.trim()}))
}

// --- Key Manager (Barbican, roadmap 55) ----------------------------------------

/// A Barbican secret's metadata (never its payload). Barbican's dates have no
/// zone; they are UTC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretRow {
    /// The last segment of `secret_ref`.
    pub id: String,
    /// The name, else the ID.
    pub name: String,
    /// `symmetric`, `public`, `private`, `passphrase`, `certificate` or `opaque`.
    pub secret_type: String,
    pub algorithm: Option<String>,
    pub bit_length: Option<i64>,
    pub mode: Option<String>,
    /// `content_types.default`; `None` while the secret has no payload.
    pub content_type: Option<String>,
    pub status: String,
    /// `None` = never expires.
    pub expiration: Option<String>,
    pub created: String,
    pub updated: Option<String>,
    pub creator_id: Option<String>,
    /// The secret's URL, as other services take it (Octavia's `default_tls_container_ref`).
    pub secret_ref: String,
}

/// How close a secret is to its expiration (list and detail styling).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expiry {
    Never,
    Later,
    /// Within [`EXPIRY_WARNING_DAYS`].
    Soon,
    Past,
}

/// Days before its expiration a secret is flagged.
pub const EXPIRY_WARNING_DAYS: i64 = 30;

/// Barbican's `expiration` (UTC, with or without a zone) against `now`.
/// Unreadable dates count as `Later`: nothing to flag.
pub fn expiry(expiration: Option<&str>, now: chrono::NaiveDateTime) -> Expiry {
    let Some(e) = expiration else { return Expiry::Never };
    let at = chrono::DateTime::parse_from_rfc3339(e)
        .map(|d| d.naive_utc())
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(e, "%Y-%m-%dT%H:%M:%S%.f"));
    match at {
        Ok(at) if at <= now => Expiry::Past,
        Ok(at) if at - now <= chrono::Duration::days(EXPIRY_WARNING_DAYS) => Expiry::Soon,
        _ => Expiry::Later,
    }
}

/// A service using a secret (Barbican's consumers API, newer releases).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretConsumer {
    pub service: String,
    pub resource_type: String,
    pub resource_id: String,
}

/// Key Manager › Secrets › one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretDetail {
    pub secret: SecretRow,
    /// The `read` ACL, shown and edited on the detail page (roadmap 75).
    pub acl: KmAcl,
    /// `None` when this Barbican has no consumers API for secrets.
    pub consumers: Option<Vec<SecretConsumer>>,
    /// Whether `[key_manager] allow_payload` offers *Show Payload*.
    pub payload_allowed: bool,
}

/// A secret's or container's `read` ACL as the detail pages show and edit it
/// (roadmap 75).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KmAcl {
    /// Users granted read access besides the project.
    pub users: Vec<String>,
    /// `project-access`: whether every project member can read it (`None` when
    /// the ACL could not be read).
    pub project_access: Option<bool>,
    /// `(id, name)` of the `users` Keystone let the viewer read (`users/{id}`);
    /// the others are shown by ID (a non-admin gets 403 there).
    pub user_names: Vec<(String, String)>,
    /// Whether the viewer may change it: [`can_edit_secret_acl`].
    pub can_edit: bool,
    /// Barbican's own record of what uses it: its consumers, and for a secret the
    /// containers holding it that have consumers.
    pub in_use: Vec<String>,
    /// The Octavia listeners and pools whose certificate refs point at it,
    /// directly or (for a secret) through a container. Octavia never registers a
    /// Barbican consumer (it reads with `check_only=True`), so this is how a load
    /// balancer is found; it reads with its own user from the ACL, and loses the
    /// certificate at its next read (a failover) when that user is removed.
    #[serde(default)]
    pub octavia: Vec<OctaviaUse>,
}

/// An Octavia listener or pool using a Barbican secret or container (roadmap 75).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OctaviaUse {
    /// The load balancer's ID, for the link; `None` when Octavia didn't say.
    pub lb_id: Option<String>,
    /// Its name, else its ID.
    pub lb_name: String,
    /// `listener` or `pool`.
    pub kind: String,
    /// The listener's or pool's name, else its ID.
    pub name: String,
    /// For a secret used through a container: `(container ID, name)`.
    pub via: Option<(String, String)>,
}

impl KmAcl {
    /// Whether an ACL is set at all (else it is Barbican's default and *Reset
    /// Access* has nothing to do).
    pub fn is_set(&self) -> bool {
        self.project_access == Some(false) || !self.users.is_empty()
    }
    /// Whether anything is known to use it (the dialogs then warn).
    pub fn in_use(&self) -> bool {
        !self.in_use.is_empty() || !self.octavia.is_empty()
    }
    /// A user as `name (id)` when Keystone named it, else the ID.
    pub fn user_label(&self, id: &str) -> String {
        match self.user_names.iter().find(|(i, _)| i == id) {
            Some((_, name)) => format!("{name} ({id})"),
            None => id.to_string(),
        }
    }
}

/// Key Manager › Containers › one: the container and its `read` ACL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerDetail {
    pub container: ContainerRow,
    pub acl: KmAcl,
}

/// Barbican's default policy (`secret_acls:put_patch` / `delete`, and the same
/// for `container_acls`): the creator or a project admin. *Edit Access* and *Reset Access* are hidden for
/// everyone else rather than failing with 403 (roadmap 75).
pub fn can_edit_secret_acl(creator_id: Option<&str>, user_id: &str, roles: &[String]) -> bool {
    creator_id == Some(user_id) || roles.iter().any(|r| r == "admin")
}

/// User IDs typed for a secret's ACL: split on commas, spaces and new lines,
/// deduplicated, in the order given.
pub fn parse_user_ids(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for id in text
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|x| !x.is_empty())
    {
        if !out.iter().any(|o| o == id) {
            out.push(id.to_string());
        }
    }
    out
}

/// A secret in a container, under its role (`certificate`, `private_key`…).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerSecret {
    pub role: String,
    pub id: String,
    /// The secret's name, else its ID.
    pub name: String,
}

/// A service registered as using a container (`{name, URL}`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerConsumer {
    pub name: String,
    pub url: String,
}

/// A Barbican container: secrets grouped by role.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerRow {
    /// The last segment of `container_ref`.
    pub id: String,
    /// The name, else the ID.
    pub name: String,
    /// `generic`, `certificate` or `rsa`.
    pub container_type: String,
    pub status: String,
    pub created: String,
    pub updated: Option<String>,
    pub creator_id: Option<String>,
    pub container_ref: String,
    pub secrets: Vec<ContainerSecret>,
    pub consumers: Vec<ContainerConsumer>,
}

/// Sensitive text (a secret's payload): never shown in `Debug` output (logs),
/// and overwritten with zeros when dropped, on the server and in the browser.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
#[serde(transparent)]
pub struct Hidden(pub String);

impl std::fmt::Debug for Hidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("\"********\"")
    }
}

/// *Show Payload*'s answer (roadmap 55).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretPayload {
    pub payload: Hidden,
    /// The type Barbican returned it in (`text/plain`, `application/octet-stream`…).
    pub content_type: String,
    /// Binary payloads are sent base64-encoded.
    pub base64: bool,
}

/// *Store Secret*, as Barbican's `POST /v1/secrets` takes it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretSpec {
    pub name: Option<String>,
    pub secret_type: String,
    /// `None`: metadata only (the payload can be added later; Barbican's two-step store).
    pub payload: Option<Hidden>,
    /// `text/plain` or `application/octet-stream` (then `payload` is base64).
    pub payload_content_type: String,
    pub algorithm: Option<String>,
    pub bit_length: Option<i64>,
    pub mode: Option<String>,
    /// ISO 8601 in UTC (`2026-12-31T00:00:00`); `None` = never expires.
    pub expiration: Option<String>,
}

/// A sub-port to add: `vlan` needs an ID (1-4094); `inherit` takes the
/// segmentation of the port's network and no ID.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubportSpec {
    pub port_id: String,
    pub segmentation_type: String,
    pub segmentation_id: Option<u16>,
}

impl SubportSpec {
    /// The `sub_ports` entry Neutron expects; validates the port and segmentation.
    pub fn body(&self) -> Result<serde_json::Value, String> {
        use serde_json::json;
        if self.port_id.trim().is_empty() {
            return Err("a sub-port needs a port".into());
        }
        match (self.segmentation_type.as_str(), self.segmentation_id) {
            ("vlan", Some(id)) if (1..=4094).contains(&id) => {
                Ok(json!({"port_id": self.port_id.trim(), "segmentation_type": "vlan", "segmentation_id": id}))
            }
            ("vlan", _) => Err("a VLAN sub-port needs a segmentation ID, 1-4094".into()),
            ("inherit", None) => Ok(json!({"port_id": self.port_id.trim(), "segmentation_type": "inherit"})),
            ("inherit", Some(_)) => Err("an inherit sub-port takes no segmentation ID".into()),
            (other, _) => Err(format!("segmentation type must be vlan or inherit (got {other})")),
        }
    }
}

/// Neutron QoS rule types, as in the API paths (`…/bandwidth_limit_rules`).
pub const QOS_RULE_TYPES: [&str; 4] = [
    "bandwidth_limit",
    "dscp_marking",
    "minimum_bandwidth",
    "minimum_packet_rate",
];
/// The DSCP marks Neutron accepts.
pub const DSCP_MARKS: [u8; 21] = [
    0, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 46, 48, 56,
];

/// One rule of a QoS policy, its type-specific fields already formatted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QosRule {
    pub id: String,
    /// One of [`QOS_RULE_TYPES`].
    pub kind: String,
    pub direction: Option<String>,
    pub max_kbps: Option<i64>,
    pub max_burst_kbps: Option<i64>,
    pub dscp_mark: Option<i64>,
    pub min_kbps: Option<i64>,
    pub min_kpps: Option<i64>,
}

/// A row of the Network QoS list (the project's policies and shared ones).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QosPolicyRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub shared: bool,
    pub is_default: bool,
    /// The project's own policy (shared ones of other projects are read-only).
    pub owned: bool,
    pub rules: Vec<QosRule>,
}

/// A rule to create or change; [`QosRuleSpec::body`] keeps only the fields of its type.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct QosRuleSpec {
    pub kind: String,
    pub direction: Option<String>,
    pub max_kbps: Option<u32>,
    pub max_burst_kbps: Option<u32>,
    pub dscp_mark: Option<u8>,
    pub min_kbps: Option<u32>,
    pub min_kpps: Option<u32>,
}

impl QosRuleSpec {
    /// Directions each type accepts (none for DSCP marking).
    pub fn directions(kind: &str) -> &'static [&'static str] {
        match kind {
            "bandwidth_limit" | "minimum_bandwidth" => &["egress", "ingress"],
            "minimum_packet_rate" => &["egress", "ingress", "any"],
            _ => &[],
        }
    }

    /// `{"<type>_rule": {...}}` with the fields Neutron expects for the type.
    pub fn body(&self) -> Result<serde_json::Value, String> {
        use serde_json::{Map, json};
        one_of(&self.kind, &QOS_RULE_TYPES, "rule type")?;
        let mut r = Map::new();
        let dirs = Self::directions(&self.kind);
        if let Some(d) = self.direction.as_deref() {
            if !dirs.contains(&d) {
                return Err(format!("direction {d} is not valid for {}", self.kind));
            }
            r.insert("direction".into(), json!(d));
        }
        let need = |v: Option<u32>, what: &str| {
            v.filter(|n| *n > 0)
                .ok_or_else(|| format!("{what} is required and must be above 0"))
        };
        match self.kind.as_str() {
            "bandwidth_limit" => {
                r.insert("max_kbps".into(), json!(need(self.max_kbps, "max_kbps")?));
                if let Some(b) = self.max_burst_kbps {
                    r.insert("max_burst_kbps".into(), json!(b));
                }
            }
            "dscp_marking" => {
                let m = self.dscp_mark.ok_or("dscp_mark is required")?;
                if !DSCP_MARKS.contains(&m) {
                    return Err(format!("{m} is not a valid DSCP mark"));
                }
                r.insert("dscp_mark".into(), json!(m));
            }
            "minimum_bandwidth" => {
                r.insert("min_kbps".into(), json!(need(self.min_kbps, "min_kbps")?));
            }
            _ => {
                r.insert("min_kpps".into(), json!(need(self.min_kpps, "min_kpps")?));
            }
        }
        Ok(json!({ format!("{}_rule", self.kind): r }))
    }
}

/// Listener and pool protocols offered. `HTTPS` passes TLS through to the members.
pub const LB_PROTOCOLS: [&str; 4] = ["HTTP", "HTTPS", "TCP", "UDP"];
/// HTTPS ended at the load balancer with a Barbican certificate (roadmap 54):
/// listeners only (the pool is `HTTP`), offered with `needs::LB_TLS_TERMINATION`.
pub const LB_TLS_PROTOCOL: &str = "TERMINATED_HTTPS";
/// Octavia `lb_algorithm` values offered in the dialogs.
pub const LB_ALGORITHMS: [&str; 3] = ["ROUND_ROBIN", "LEAST_CONNECTIONS", "SOURCE_IP"];
/// Octavia `session_persistence.type` values; `APP_COOKIE` needs a cookie name.
pub const LB_PERSISTENCE: [&str; 3] = ["SOURCE_IP", "HTTP_COOKIE", "APP_COOKIE"];
/// Nova key pair types (`type`, compute 2.2).
pub const KEYPAIR_TYPES: [&str; 2] = ["ssh", "x509"];

/// The pasted key matches its type: an OpenSSH public key for `ssh`, a PEM
/// certificate for `x509` (what Nova expects in `public_key`).
pub fn check_public_key(kind: &str, key: &str) -> Result<(), String> {
    one_of(kind, &KEYPAIR_TYPES, "key type")?;
    let key = key.trim_start();
    let ok = match kind {
        "x509" => key.starts_with("-----BEGIN CERTIFICATE-----"),
        _ => ["ssh-", "ecdsa-", "sk-"].iter().any(|p| key.starts_with(p)),
    };
    if ok {
        Ok(())
    } else if kind == "x509" {
        Err("an X509 key pair needs a PEM certificate (-----BEGIN CERTIFICATE-----)".into())
    } else {
        Err("an SSH key pair needs an OpenSSH public key (ssh-…, ecdsa-… or sk-…)".into())
    }
}
/// Octavia health monitor `type` values; the HTTP fields apply to `HTTP`/`HTTPS` only.
pub const LB_MONITOR_TYPES: [&str; 6] = ["HTTP", "HTTPS", "PING", "TCP", "TLS-HELLO", "UDP-CONNECT"];
/// HTTP methods a health monitor may use.
pub const LB_HTTP_METHODS: [&str; 3] = ["GET", "HEAD", "POST"];

/// The pool protocol that goes with a listener's (Horizon pairs them the same way).
pub fn pool_protocol_for(listener: &str) -> &'static str {
    match listener {
        "HTTPS" => "HTTPS",
        "TCP" => "TCP",
        "UDP" => "UDP",
        _ => "HTTP",
    }
}

fn one_of(v: &str, allowed: &[&str], what: &str) -> Result<(), String> {
    if allowed.contains(&v) {
        Ok(())
    } else {
        Err(format!("{what} must be one of {} (got {v})", allowed.join(", ")))
    }
}

/// A listener to create (wizard step or *Create Listener*).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbListenerSpec {
    pub name: Option<String>,
    pub description: Option<String>,
    pub protocol: String,
    pub port: u16,
    /// `None` = unlimited.
    pub connection_limit: Option<u32>,
    /// The *Advanced* fields (roadmap 54).
    #[serde(default)]
    pub options: LbListenerOptions,
    /// `TERMINATED_HTTPS` only: the Barbican container or secret URL of the
    /// certificate, required (roadmap 54).
    #[serde(default)]
    pub default_tls_container_ref: Option<String>,
    /// `TERMINATED_HTTPS` only: more certificates, chosen by the name the client
    /// asks for (SNI).
    #[serde(default)]
    pub sni_container_refs: Vec<String>,
    /// *Admin State Up* (roadmap 54); only `false` is sent.
    #[serde(default = "yes")]
    pub admin_up: bool,
}

/// Headers Octavia can add to requests sent to the members (`insert_headers`),
/// as octavia-dashboard offers them; HTTP listeners only.
pub const LB_INSERT_HEADERS: [&str; 3] = ["X-Forwarded-For", "X-Forwarded-Port", "X-Forwarded-Proto"];
/// Largest timeout, ms: Octavia takes up to a year, but the SDK's builders
/// take an `i32` (about 24.8 days).
pub const LB_TIMEOUT_MAX: u32 = i32::MAX as u32;

/// A listener's *Advanced* options, as octavia-dashboard has them (roadmap 54).
/// `None` / empty = Octavia's default: not sent on create, and on edit sent as
/// `null` / `[]` / `{}`, which Octavia takes as "back to the default".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LbListenerOptions {
    /// Milliseconds (Octavia API 2.1; defaults 50 000, 0, 5 000 and 50 000).
    pub timeout_client_data: Option<u32>,
    pub timeout_tcp_inspect: Option<u32>,
    pub timeout_member_connect: Option<u32>,
    pub timeout_member_data: Option<u32>,
    /// Some of [`LB_INSERT_HEADERS`], each sent as `"true"`.
    pub insert_headers: Vec<String>,
    /// Client networks allowed in (Octavia 2.12); empty = everyone. `None`: not
    /// offered (older Octavia, which would refuse the key), so never sent.
    pub allowed_cidrs: Option<Vec<String>>,
    /// OpenSSL cipher string of a `TERMINATED_HTTPS` listener (Octavia 2.15);
    /// `None` = the cloud's default.
    pub tls_ciphers: Option<String>,
}

impl LbListenerOptions {
    /// Timeouts up to 7 days, headers known and for HTTP listeners only, CIDRs
    /// that parse.
    pub fn check(&self, protocol: &str) -> Result<(), String> {
        let timeouts = [
            self.timeout_client_data,
            self.timeout_member_connect,
            self.timeout_member_data,
            self.timeout_tcp_inspect,
        ];
        if timeouts.iter().flatten().any(|&t| t > LB_TIMEOUT_MAX) {
            return Err(format!("timeouts are at most {LB_TIMEOUT_MAX} ms"));
        }
        if !self.insert_headers.is_empty() && !lb_inserts_headers(protocol) {
            return Err("headers can only be inserted by HTTP and TERMINATED_HTTPS listeners".into());
        }
        if self.tls_ciphers.as_deref().is_some_and(|c| !c.trim().is_empty()) && protocol != LB_TLS_PROTOCOL {
            return Err("TLS ciphers are for TERMINATED_HTTPS listeners".into());
        }
        self.insert_headers
            .iter()
            .try_for_each(|h| one_of(h, &LB_INSERT_HEADERS, "header"))?;
        for c in self.allowed_cidrs.iter().flatten() {
            crate::prefs::Cidr::parse(c)?;
        }
        Ok(())
    }

    /// The cipher string, if one was typed.
    pub fn ciphers(&self) -> Option<&str> {
        self.tls_ciphers.as_deref().map(str::trim).filter(|c| !c.is_empty())
    }

    /// `insert_headers` as Octavia wants it: `{"X-Forwarded-For": "true", …}`.
    pub fn headers_map(&self) -> serde_json::Map<String, serde_json::Value> {
        self.insert_headers
            .iter()
            .map(|h| (h.clone(), serde_json::Value::String("true".into())))
            .collect()
    }
}

/// A pool to create; in the wizard the listener's default pool, which must be named
/// (roadmap 53).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbPoolSpec {
    pub name: Option<String>,
    pub description: Option<String>,
    pub protocol: String,
    pub algorithm: String,
    /// `SOURCE_IP`, `HTTP_COOKIE` or `APP_COOKIE` (with `cookie_name`).
    pub session_persistence: Option<String>,
    pub cookie_name: Option<String>,
    /// Re-encrypt to the members (Octavia 2.8, roadmap 54).
    #[serde(default)]
    pub tls_enabled: bool,
    /// OpenSSL cipher string for `tls_enabled` pools (Octavia 2.15); `None` = the
    /// cloud's default.
    #[serde(default)]
    pub tls_ciphers: Option<String>,
    /// *Admin State Up* (roadmap 54); only `false` is sent.
    #[serde(default = "yes")]
    pub admin_up: bool,
}

/// A pool member to create: an address on a subnet, its port and weight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbMemberSpec {
    pub name: Option<String>,
    pub address: String,
    pub port: u16,
    pub subnet_id: Option<String>,
    /// 0-256; `None` = Octavia's default (1).
    pub weight: Option<u16>,
    pub backup: bool,
    /// Health-check this address/port instead of the member's own (roadmap 54).
    #[serde(default)]
    pub monitor_address: Option<String>,
    #[serde(default)]
    pub monitor_port: Option<u16>,
    #[serde(default = "yes")]
    pub admin_up: bool,
}

fn yes() -> bool {
    true
}

/// One row of the members editor (roadmap 54), as typed: numbers stay text
/// until submit, so a half-typed port isn't lost. The field's value is a JSON
/// list of these.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MemberRow {
    pub address: String,
    /// Empty = Octavia uses the load balancer's VIP subnet.
    #[serde(default)]
    pub subnet_id: String,
    /// Shown for rows whose address isn't typed (instance addresses, existing members).
    #[serde(default)]
    pub subnet_name: String,
    /// The instance the address belongs to, when picked from *Available Instances*.
    #[serde(default)]
    pub label: Option<String>,
    /// An *external member*: address and subnet are typed in the row. Instance
    /// addresses and existing members keep theirs (changing an existing member's
    /// address would replace it).
    #[serde(default)]
    pub typed: bool,
    #[serde(default)]
    pub name: String,
    pub port: String,
    /// Empty = Octavia's default (1).
    #[serde(default)]
    pub weight: String,
    #[serde(default)]
    pub backup: bool,
    #[serde(default = "yes")]
    pub admin_up: bool,
    #[serde(default)]
    pub monitor_address: String,
    #[serde(default)]
    pub monitor_port: String,
}

impl MemberRow {
    /// A row for an existing member, so *Edit Members* sends it back unchanged.
    pub fn from_member(m: &LbMember) -> Self {
        MemberRow {
            address: m.address.clone(),
            subnet_id: m.subnet.as_ref().map(|s| s.id.clone()).unwrap_or_default(),
            subnet_name: m.subnet.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
            label: None,
            typed: false,
            // `LbMember.name` falls back to the ID for display; don't send that as a name.
            name: if m.name == m.id { String::new() } else { m.name.clone() },
            port: m.port.map(|p| p.to_string()).unwrap_or_default(),
            weight: m.weight.map(|w| w.to_string()).unwrap_or_else(|| "1".into()),
            backup: m.backup,
            admin_up: m.admin_up,
            monitor_address: m.monitor_address.clone().unwrap_or_default(),
            monitor_port: m.monitor_port.map(|p| p.to_string()).unwrap_or_default(),
        }
    }

    /// The member to send, checked ([`LbMemberSpec::check`]).
    pub fn spec(&self) -> Result<LbMemberSpec, String> {
        let address = self.address.trim();
        let port_of = |s: &str, what: &str| -> Result<Option<u16>, String> {
            let s = s.trim();
            if s.is_empty() {
                return Ok(None);
            }
            s.parse::<u16>()
                .map(Some)
                .map_err(|_| format!("{address}: the {what} must be 1-65535"))
        };
        let weight = match self.weight.trim() {
            "" => None,
            w => Some(
                w.parse::<u16>()
                    .map_err(|_| format!("{address}: the weight must be 0-256"))?,
            ),
        };
        let nonempty = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
        // Horizon requires a subnet for an external member; ours too.
        if self.typed && self.subnet_id.trim().is_empty() {
            return Err(format!("{address}: pick the member's subnet"));
        }
        let spec = LbMemberSpec {
            name: nonempty(&self.name),
            address: address.to_string(),
            port: port_of(&self.port, "port")?.ok_or_else(|| format!("{address}: the port is required"))?,
            subnet_id: nonempty(&self.subnet_id),
            weight,
            backup: self.backup,
            monitor_address: nonempty(&self.monitor_address),
            monitor_port: port_of(&self.monitor_port, "monitor port")?,
            admin_up: self.admin_up,
        };
        spec.check()?;
        Ok(spec)
    }
}

/// The members editor's value as members to send: every row checked, and no
/// address/port twice (Octavia's key for a member).
pub fn member_specs(json: &str) -> Result<Vec<LbMemberSpec>, String> {
    let rows: Vec<MemberRow> = if json.trim().is_empty() {
        vec![]
    } else {
        serde_json::from_str(json).map_err(|e| e.to_string())?
    };
    let specs = rows.iter().map(MemberRow::spec).collect::<Result<Vec<_>, _>>()?;
    unique_members(&specs)?;
    Ok(specs)
}

fn unique_members(specs: &[LbMemberSpec]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    match specs.iter().find(|m| !seen.insert((m.address.as_str(), m.port))) {
        Some(m) => Err(format!("{}:{} is listed twice", m.address, m.port)),
        None => Ok(()),
    }
}

/// Existing members that a batch update with `keep` deletes: Octavia matches
/// members by address and port, and removes those left out.
pub fn removed_members<'a>(existing: &'a [LbMember], keep: &[LbMemberSpec]) -> Vec<&'a LbMember> {
    existing
        .iter()
        .filter(|m| {
            !keep
                .iter()
                .any(|k| k.address == m.address && m.port == Some(i64::from(k.port)))
        })
        .collect()
}

/// A health monitor to create; Octavia's rules are in [`LbMonitorSpec::check`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbMonitorSpec {
    pub name: Option<String>,
    pub kind: String,
    pub delay: u32,
    pub timeout: u32,
    pub max_retries: u8,
    pub max_retries_down: Option<u8>,
    /// HTTP(S) monitors only.
    pub http_method: Option<String>,
    pub url_path: Option<String>,
    pub expected_codes: Option<String>,
    /// *Admin State Up* (roadmap 54); only `false` is sent on create.
    #[serde(default = "yes")]
    pub admin_up: bool,
}

/// Horizon's *Create Load Balancer* wizard, sent as one fully populated request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbCreateSpec {
    pub name: Option<String>,
    pub description: Option<String>,
    pub vip_subnet_id: String,
    pub vip_address: Option<String>,
    pub flavor_id: Option<String>,
    /// An Octavia availability zone (`ChoiceSet::LbAvailabilityZones`); `None` = any.
    pub availability_zone: Option<String>,
    /// *Admin State Up* of the load balancer (roadmap 54); only `false` is sent.
    #[serde(default = "yes")]
    pub admin_up: bool,
    pub listener: Option<LbListenerSpec>,
    /// Needs a listener.
    pub pool: Option<LbPoolSpec>,
    /// Need a pool.
    pub members: Vec<LbMemberSpec>,
    pub monitor: Option<LbMonitorSpec>,
}

impl LbListenerSpec {
    /// Octavia's rules for a listener: known protocol, port 1-65535, its
    /// options (`LbListenerOptions::check`) and certificates (`check_listener_tls`).
    pub fn check(&self) -> Result<(), String> {
        if self.protocol != LB_TLS_PROTOCOL {
            one_of(&self.protocol, &LB_PROTOCOLS, "listener protocol")?;
        }
        if self.port == 0 {
            return Err("the listener port must be 1-65535".into());
        }
        self.options.check(&self.protocol)?;
        check_listener_tls(
            &self.protocol,
            self.default_tls_container_ref.as_deref(),
            &self.sni_container_refs,
        )
    }
}

impl LbPoolSpec {
    /// Known protocol, algorithm and persistence; `APP_COOKIE` needs a cookie name.
    pub fn check(&self) -> Result<(), String> {
        one_of(&self.protocol, &LB_PROTOCOLS, "pool protocol")?;
        one_of(&self.algorithm, &LB_ALGORITHMS, "algorithm")?;
        if let Some(p) = self.session_persistence.as_deref() {
            one_of(p, &LB_PERSISTENCE, "session persistence")?;
            if p == "APP_COOKIE" && self.cookie_name.as_deref().is_none_or(|c| c.trim().is_empty()) {
                return Err("APP_COOKIE persistence needs a cookie name".into());
            }
        }
        check_pool_tls(&self.protocol, self.tls_enabled, self.tls_ciphers.as_deref())
    }
}

/// Whether a listener of this protocol can insert headers (Octavia: the HTTP
/// ones, where it sees the requests).
pub fn lb_inserts_headers(protocol: &str) -> bool {
    protocol == "HTTP" || protocol == LB_TLS_PROTOCOL
}

/// A listener's certificates (roadmap 54): `TERMINATED_HTTPS` needs one, other
/// protocols take none, and each is a Barbican URL.
pub fn check_listener_tls(protocol: &str, default_ref: Option<&str>, sni: &[String]) -> Result<(), String> {
    let default_ref = default_ref.map(str::trim).filter(|r| !r.is_empty());
    if protocol != LB_TLS_PROTOCOL {
        return if default_ref.is_some() || !sni.is_empty() {
            Err("certificates are for TERMINATED_HTTPS listeners".into())
        } else {
            Ok(())
        };
    }
    let Some(d) = default_ref else {
        return Err("a TERMINATED_HTTPS listener needs a certificate".into());
    };
    for r in std::iter::once(d).chain(sni.iter().map(|s| s.trim())) {
        if !(r.starts_with("https://") || r.starts_with("http://")) {
            return Err(format!("not a Barbican reference: {r}"));
        }
    }
    Ok(())
}

/// Octavia's rules for re-encryption: not for UDP pools, and ciphers only when
/// it is on (a colon-separated OpenSSL list, at most 2048 characters).
pub fn check_pool_tls(protocol: &str, enabled: bool, ciphers: Option<&str>) -> Result<(), String> {
    if enabled && protocol == "UDP" {
        return Err("UDP pools can't re-encrypt to their members".into());
    }
    match ciphers.map(str::trim).filter(|c| !c.is_empty()) {
        Some(_) if !enabled => Err("TLS ciphers need TLS to the members".into()),
        Some(c) if c.len() > 2048 || c.contains(char::is_whitespace) => {
            Err("TLS ciphers are a colon-separated list, at most 2048 characters".into())
        }
        _ => Ok(()),
    }
}

impl LbMemberSpec {
    /// A valid IP address, port 1-65535 and weight 0-256; the monitor address
    /// an IP and its port 1-65535 when given.
    pub fn check(&self) -> Result<(), String> {
        self.address
            .trim()
            .parse::<std::net::IpAddr>()
            .map_err(|_| format!("not an IP address: {}", self.address))?;
        if self.port == 0 {
            return Err("the member port must be 1-65535".into());
        }
        if self.weight.is_some_and(|w| w > 256) {
            return Err("the weight must be 0-256".into());
        }
        if let Some(a) = self.monitor_address.as_deref() {
            a.trim()
                .parse::<std::net::IpAddr>()
                .map_err(|_| format!("not an IP address: {a}"))?;
        }
        if self.monitor_port == Some(0) {
            return Err("the monitor port must be 1-65535".into());
        }
        Ok(())
    }
}

impl LbMonitorSpec {
    /// Octavia's rules: delay ≥ timeout, retries 1-10, HTTP fields only for HTTP(S).
    pub fn check(&self) -> Result<(), String> {
        one_of(&self.kind, &LB_MONITOR_TYPES, "monitor type")?;
        if self.delay == 0 || self.timeout == 0 || self.timeout > self.delay {
            return Err("the delay and timeout must be at least 1 s, and the timeout not longer than the delay".into());
        }
        let retries = |n: u8| (1..=10).contains(&n);
        if !retries(self.max_retries) || self.max_retries_down.is_some_and(|n| !retries(n)) {
            return Err("retries must be 1-10".into());
        }
        let http = self.kind.starts_with("HTTP");
        if !http && (self.http_method.is_some() || self.url_path.is_some() || self.expected_codes.is_some()) {
            return Err("HTTP method, URL path and expected codes are for HTTP(S) monitors only".into());
        }
        if let Some(m) = self.http_method.as_deref() {
            one_of(m, &LB_HTTP_METHODS, "HTTP method")?;
        }
        if self.url_path.as_deref().is_some_and(|p| !p.starts_with('/')) {
            return Err("the URL path must start with /".into());
        }
        Ok(())
    }
}

impl LbCreateSpec {
    /// The whole wizard: subnet given, VIP address valid, every part checked, and
    /// nothing that needs a missing parent (a pool without a listener, members
    /// without a pool). The default pool must be named (roadmap 53).
    pub fn check(&self) -> Result<(), String> {
        if self.vip_subnet_id.trim().is_empty() {
            return Err("a subnet is required".into());
        }
        if let Some(a) = self.vip_address.as_deref() {
            a.trim()
                .parse::<std::net::IpAddr>()
                .map_err(|_| format!("not an IP address: {a}"))?;
        }
        if let Some(l) = &self.listener {
            l.check()?;
        }
        match (&self.listener, &self.pool) {
            (None, Some(_)) => return Err("a pool needs a listener".into()),
            // Octavia's single-call create rejects an unnamed default pool.
            (_, Some(p)) if p.name.as_deref().is_none_or(|n| n.trim().is_empty()) => {
                return Err("the pool needs a name".into());
            }
            (_, Some(p)) => p.check()?,
            _ => {}
        }
        if self.pool.is_none() && (!self.members.is_empty() || self.monitor.is_some()) {
            return Err("members and monitors need a pool".into());
        }
        self.members.iter().try_for_each(LbMemberSpec::check)?;
        unique_members(&self.members)?;
        if let Some(m) = &self.monitor {
            m.check()?;
        }
        Ok(())
    }
}

/// A resource referenced by ID, with its name when known (else the ID).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ref {
    pub id: String,
    pub name: String,
}

/// A listener of a load balancer (detail page, *Listeners* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbListener {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub protocol: String,
    pub port: Option<i64>,
    pub default_pool: Option<Ref>,
    /// `-1` (unlimited) → `None`.
    pub connection_limit: Option<i64>,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
    /// The *Advanced* options, for the edit dialog and the listener's row.
    #[serde(default)]
    pub options: LbListenerOptions,
    /// `TERMINATED_HTTPS`: the certificate and the SNI ones (Barbican URLs).
    #[serde(default)]
    pub default_tls_container_ref: Option<String>,
    #[serde(default)]
    pub sni_container_refs: Vec<String>,
}

/// A pool of a load balancer (detail page, *Pools* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbPool {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub algorithm: String,
    /// `SOURCE_IP`, `HTTP_COOKIE`, `APP_COOKIE (name)`.
    pub session_persistence: Option<String>,
    /// The same, split, for the edit dialog.
    #[serde(default)]
    pub persistence_type: Option<String>,
    #[serde(default)]
    pub cookie_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub listeners: Vec<Ref>,
    pub members: usize,
    pub monitor: Option<Ref>,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
    /// Re-encrypts to the members (Octavia 2.8), with these ciphers.
    #[serde(default)]
    pub tls_enabled: bool,
    #[serde(default)]
    pub tls_ciphers: Option<String>,
}

/// A member of a pool (detail page, *Members* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbMember {
    pub id: String,
    pub name: String,
    pub pool: Ref,
    pub address: String,
    pub port: Option<i64>,
    pub weight: Option<i64>,
    pub subnet: Option<Ref>,
    pub backup: bool,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
    /// Where it is health-checked instead of `address`/`port` (`None` = itself).
    #[serde(default)]
    pub monitor_address: Option<String>,
    #[serde(default)]
    pub monitor_port: Option<i64>,
}

/// An L7 policy of one of the load balancer's listeners (detail page, *L7
/// Policies* tab, roadmap 54), with its rules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbL7Policy {
    pub id: String,
    /// The name, else the ID.
    pub name: String,
    pub description: Option<String>,
    pub listener: Ref,
    /// One of [`LB_L7_ACTIONS`] (or another Octavia action, shown as is).
    pub action: String,
    /// 1 = evaluated first.
    pub position: Option<i64>,
    pub redirect_pool: Option<Ref>,
    pub redirect_url: Option<String>,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
    pub rules: Vec<LbL7Rule>,
}

/// A rule of an L7 policy: the policy applies when all its rules match.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbL7Rule {
    pub id: String,
    /// `type`: one of [`LB_L7_RULE_TYPES`] (others shown as is).
    pub rule_type: String,
    pub compare_type: String,
    /// The header or cookie name (`HEADER`, `COOKIE`).
    pub key: Option<String>,
    pub value: String,
    pub invert: bool,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
}

/// L7 policy actions octavia-dashboard offers.
pub const LB_L7_ACTIONS: [&str; 3] = ["REDIRECT_TO_POOL", "REDIRECT_TO_URL", "REJECT"];
/// L7 rule types octavia-dashboard offers; `HEADER` and `COOKIE` need a key.
pub const LB_L7_RULE_TYPES: [&str; 5] = ["HOST_NAME", "PATH", "FILE_TYPE", "HEADER", "COOKIE"];
/// How an L7 rule compares its value; `FILE_TYPE` takes `EQUAL_TO` or `REGEX` only.
pub const LB_L7_COMPARE_TYPES: [&str; 5] = ["REGEX", "STARTS_WITH", "ENDS_WITH", "CONTAINS", "EQUAL_TO"];

/// An L7 policy to create or save (roadmap 54).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbL7PolicySpec {
    pub name: Option<String>,
    pub description: Option<String>,
    pub action: String,
    /// `None` = last.
    pub position: Option<u32>,
    /// `REDIRECT_TO_POOL` only.
    pub redirect_pool_id: Option<String>,
    /// `REDIRECT_TO_URL` only.
    pub redirect_url: Option<String>,
    pub admin_up: bool,
}

impl LbL7PolicySpec {
    /// A known action with its target: a pool for `REDIRECT_TO_POOL`, an
    /// `http(s)://` URL for `REDIRECT_TO_URL`, neither for `REJECT`.
    pub fn check(&self) -> Result<(), String> {
        one_of(&self.action, &LB_L7_ACTIONS, "L7 action")?;
        let pool = self
            .redirect_pool_id
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty());
        let url = self.redirect_url.as_deref().map(str::trim).filter(|u| !u.is_empty());
        match self.action.as_str() {
            "REDIRECT_TO_POOL" if pool.is_none() => return Err("redirecting to a pool needs the pool".into()),
            "REDIRECT_TO_URL" => match url {
                Some(u) if u.starts_with("http://") || u.starts_with("https://") => {}
                _ => return Err("redirecting to a URL needs an http:// or https:// URL".into()),
            },
            _ => {}
        }
        if self.position == Some(0) {
            return Err("the position starts at 1".into());
        }
        Ok(())
    }

    /// The redirect target that goes with the action; the other one is `None`.
    pub fn targets(&self) -> (Option<&str>, Option<&str>) {
        fn t(v: &Option<String>) -> Option<&str> {
            v.as_deref().map(str::trim).filter(|x| !x.is_empty())
        }
        match self.action.as_str() {
            "REDIRECT_TO_POOL" => (t(&self.redirect_pool_id), None),
            "REDIRECT_TO_URL" => (None, t(&self.redirect_url)),
            _ => (None, None),
        }
    }
}

/// An L7 rule to create or save (roadmap 54).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbL7RuleSpec {
    pub rule_type: String,
    pub compare_type: String,
    pub key: Option<String>,
    pub value: String,
    pub invert: bool,
    pub admin_up: bool,
}

impl LbL7RuleSpec {
    /// Known type and comparison, a value, a key exactly for `HEADER` and
    /// `COOKIE`, and `FILE_TYPE` compared with `EQUAL_TO` or `REGEX` (Octavia's rules).
    pub fn check(&self) -> Result<(), String> {
        one_of(&self.rule_type, &LB_L7_RULE_TYPES, "rule type")?;
        one_of(&self.compare_type, &LB_L7_COMPARE_TYPES, "compare type")?;
        if self.value.trim().is_empty() {
            return Err("the rule needs a value".into());
        }
        let keyed = matches!(self.rule_type.as_str(), "HEADER" | "COOKIE");
        let key = self.key.as_deref().is_some_and(|k| !k.trim().is_empty());
        if keyed && !key {
            return Err(format!(
                "a {} rule needs a key (the {} name)",
                self.rule_type,
                self.rule_type.to_lowercase()
            ));
        }
        if !keyed && key {
            return Err("only HEADER and COOKIE rules take a key".into());
        }
        if self.rule_type == "FILE_TYPE" && !matches!(self.compare_type.as_str(), "EQUAL_TO" | "REGEX") {
            return Err("FILE_TYPE rules compare with EQUAL_TO or REGEX".into());
        }
        Ok(())
    }
}

/// A health monitor (detail page, *Monitors* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbMonitor {
    pub id: String,
    pub name: String,
    /// `HTTP`, `HTTPS`, `PING`, `TCP`, `TLS-HELLO`, `UDP-CONNECT`, `SCTP`.
    pub kind: String,
    pub delay: Option<i64>,
    pub timeout: Option<i64>,
    pub max_retries: Option<i64>,
    pub max_retries_down: Option<i64>,
    /// HTTP(S) checks: `GET /healthz → 200`.
    pub http_check: Option<String>,
    /// The same, split, for the edit dialog.
    #[serde(default)]
    pub http_method: Option<String>,
    #[serde(default)]
    pub url_path: Option<String>,
    #[serde(default)]
    pub expected_codes: Option<String>,
    pub pools: Vec<Ref>,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
}

/// Everything the load balancer detail page shows, fetched in two rounds of GETs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoadBalancerDetail {
    pub row: LoadBalancerRow,
    pub project_id: String,
    pub flavor_id: Option<String>,
    pub vip_port_id: Option<String>,
    pub vip_network: Option<Ref>,
    pub vip_subnet: Option<Ref>,
    pub floating_ip: Option<String>,
    /// Its ID, to disassociate it.
    #[serde(default)]
    pub floating_ip_id: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub listeners: Vec<LbListener>,
    pub pools: Vec<LbPool>,
    pub members: Vec<LbMember>,
    pub monitors: Vec<LbMonitor>,
    /// Of every listener, by listener then position (roadmap 54).
    #[serde(default)]
    pub l7policies: Vec<LbL7Policy>,
}

/// Octavia's transitional states (`PENDING_CREATE`, `PENDING_UPDATE`,
/// `PENDING_DELETE`): the pages refetch until they end (roadmap 54).
pub fn lb_pending(provisioning_status: &str) -> bool {
    provisioning_status.starts_with("PENDING")
}

impl LoadBalancerDetail {
    /// Whether the load balancer or any of its parts is still being changed.
    pub fn busy(&self) -> bool {
        let p = lb_pending;
        p(&self.row.provisioning_status)
            || self.listeners.iter().any(|x| p(&x.provisioning_status))
            || self.pools.iter().any(|x| p(&x.provisioning_status))
            || self.members.iter().any(|x| p(&x.provisioning_status))
            || self.monitors.iter().any(|x| p(&x.provisioning_status))
            || self
                .l7policies
                .iter()
                .any(|x| p(&x.provisioning_status) || x.rules.iter().any(|r| p(&r.provisioning_status)))
    }
}

/// A row of the Security Groups list (the project's own, filtered by `tenant_id`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecurityGroupRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub rules: Vec<String>,
    /// Shared with the project through Neutron RBAC (`shared`).
    #[serde(default)]
    pub shared: bool,
}

/// A row of the Floating IPs list, with what it is associated to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloatingIpRow {
    pub id: String,
    pub ip: String,
    pub fixed_ip: Option<String>,
    /// Instance the port belongs to, when it is a Nova port.
    pub instance_id: Option<String>,
    pub instance_name: Option<String>,
    pub pool: String,
    /// The pool's network ID (`floating_network_id`): admin *Associate* offers
    /// only the ports it reaches (roadmap 89).
    #[serde(default)]
    pub pool_id: String,
    pub status: String,
    pub description: Option<String>,
    /// `dns-integration` extension; absent otherwise.
    pub dns_name: Option<String>,
    pub dns_domain: Option<String>,
}

/// A floating IP port forwarding rule (Neutron `floating-ip-port-forwarding`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortForwardingRow {
    pub id: String,
    pub protocol: String,
    /// `2222` or, with the port-ranges extension, `8080:8090`.
    pub external: String,
    pub internal_ip: String,
    pub internal: String,
    pub internal_port_id: String,
    /// The instance owning the internal port, when known.
    pub instance: Option<Ref>,
    pub description: Option<String>,
}

/// The floating IP detail page, with its port forwardings when supported.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloatingIpDetail {
    pub row: FloatingIpRow,
    pub port_id: Option<String>,
    pub router_id: Option<String>,
    pub created: Option<String>,
    pub forwardings: Vec<PortForwardingRow>,
}

/// A port or port range of a forwarding rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortSpec {
    One(u16),
    Range(u16, u16),
}

impl PortSpec {
    /// `22` or `8080:8090` (also `8080-8090`); ranges only when the cloud has
    /// `floating-ip-port-forwarding-port-ranges`.
    pub fn parse(s: &str, ranges: bool) -> Result<PortSpec, String> {
        let s = s.trim();
        let port = |p: &str| match p.trim().parse::<u16>() {
            Ok(n) if n > 0 => Ok(n),
            _ => Err(format!("not a port number (1-65535): {p}")),
        };
        match s.split_once([':', '-']) {
            None => Ok(PortSpec::One(port(s)?)),
            Some(_) if !ranges => Err("this cloud does not support port ranges".into()),
            Some((a, b)) => {
                let (a, b) = (port(a)?, port(b)?);
                match a.cmp(&b) {
                    std::cmp::Ordering::Less => Ok(PortSpec::Range(a, b)),
                    std::cmp::Ordering::Equal => Ok(PortSpec::One(a)),
                    std::cmp::Ordering::Greater => Err(format!("the range must go up: {s}")),
                }
            }
        }
    }

    /// How many ports it covers.
    pub fn count(self) -> u32 {
        match self {
            PortSpec::One(_) => 1,
            PortSpec::Range(a, b) => u32::from(b - a) + 1,
        }
    }
}

impl std::fmt::Display for PortSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortSpec::One(p) => write!(f, "{p}"),
            PortSpec::Range(a, b) => write!(f, "{a}:{b}"),
        }
    }
}

/// What a forwarding rule sends, already checked by [`PortForwardingSpec::ports`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortForwardingSpec {
    pub floating_ip_id: String,
    /// `tcp` or `udp`.
    pub protocol: String,
    pub internal_port_id: String,
    pub internal_ip: String,
    pub internal: String,
    pub external: String,
    pub description: Option<String>,
    /// The cloud has `floating-ip-port-forwarding-port-ranges`.
    pub ranges: bool,
}

impl PortForwardingSpec {
    /// Validated `(internal, external)`: both single ports, or ranges of the same size.
    pub fn ports(&self) -> Result<(PortSpec, PortSpec), String> {
        if !matches!(self.protocol.as_str(), "tcp" | "udp") {
            return Err(format!("protocol must be tcp or udp (got {})", self.protocol));
        }
        self.internal_ip
            .trim()
            .parse::<std::net::Ipv4Addr>()
            .map_err(|_| format!("not an IPv4 address: {}", self.internal_ip))?;
        let (i, e) = (
            PortSpec::parse(&self.internal, self.ranges)?,
            PortSpec::parse(&self.external, self.ranges)?,
        );
        if i.count() != e.count() {
            return Err(format!(
                "internal ({i}) and external ({e}) ports must cover the same number of ports"
            ));
        }
        Ok((i, e))
    }
}

/// How a quota's numbers are displayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaUnit {
    Count,
    /// value is in MiB
    Mib,
    /// value is in GiB
    Gib,
}

/// The quotas the Overview shows, as in Horizon's *Limit Summary*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaKey {
    Instances,
    Vcpus,
    Ram,
    Volumes,
    VolumeSnapshots,
    VolumeStorage,
    FloatingIps,
    SecurityGroups,
    SecurityGroupRules,
    Networks,
    Ports,
    Routers,
}

/// The service a quota belongs to (one card each on the Overview).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaGroup {
    Compute,
    Volume,
    Network,
}

/// One quota: used and limit, in `unit`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quota {
    pub key: QuotaKey,
    pub used: i64,
    /// `None` = unlimited (`-1` in the APIs)
    pub limit: Option<i64>,
    pub unit: QuotaUnit,
}

/// The quotas of one service; a failing service keeps the rest of the page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuotaSection {
    pub group: QuotaGroup,
    pub quotas: Vec<Quota>,
    /// Set when that service failed: the rest of the page still renders.
    pub error: Option<String>,
}

/// A service with quotas: one card each on Project › Quotas (roadmap 82).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaService {
    Compute,
    Volume,
    Network,
    /// Octavia, with the `load-balancer` plugin.
    LoadBalancer,
    /// Barbican, with the `key-manager` plugin.
    KeyManager,
}

impl QuotaService {
    /// The keys *Edit Quotas* offers, in display order (Horizon's *Modify
    /// Quotas* fields, plus the newer Neutron and Octavia resources). Other keys
    /// the service reports (Cinder's per-type `gigabytes_<type>`…) are shown
    /// read-only after these. The server refuses any key not listed here.
    pub fn keys(self) -> &'static [&'static str] {
        match self {
            QuotaService::Compute => &[
                "instances",
                "cores",
                "ram",
                "key_pairs",
                "server_groups",
                "server_group_members",
                "metadata_items",
                "injected_files",
                "injected_file_content_bytes",
                "injected_file_path_bytes",
            ],
            QuotaService::Volume => &[
                "volumes",
                "snapshots",
                "gigabytes",
                "backups",
                "backup_gigabytes",
                "groups",
                "per_volume_gigabytes",
            ],
            QuotaService::Network => &[
                "network",
                "subnet",
                "subnetpool",
                "port",
                "router",
                "floatingip",
                "security_group",
                "security_group_rule",
                "rbac_policy",
                "trunk",
            ],
            QuotaService::LoadBalancer => &[
                "load_balancer",
                "listener",
                "pool",
                "member",
                "health_monitor",
                "l7policy",
                "l7rule",
            ],
            // Barbican's `project_quotas` (Horizon has no form for them).
            QuotaService::KeyManager => &["secrets", "containers", "orders", "consumers", "cas"],
        }
    }

    /// How `key`'s numbers read: Nova's `ram` is MiB, Cinder's `*gigabytes*` GiB.
    pub fn unit(self, key: &str) -> QuotaUnit {
        match self {
            QuotaService::Compute if key == "ram" => QuotaUnit::Mib,
            QuotaService::Volume if key.contains("gigabytes") => QuotaUnit::Gib,
            _ => QuotaUnit::Count,
        }
    }
}

/// One quota of Project › Quotas, under the API's own key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuotaRow {
    pub key: String,
    /// `None` when the service doesn't report usage (Octavia, Barbican's
    /// consumers and CAs).
    pub used: Option<i64>,
    /// Nova's and Cinder's reservations in flight; `None` when not reported.
    pub reserved: Option<i64>,
    /// `None` = unlimited (`-1` in the APIs)
    pub limit: Option<i64>,
}

/// One service's quotas; a failing service keeps the rest of the page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuotaCard {
    pub service: QuotaService,
    /// The project these quotas belong to (what *Edit Quotas* updates).
    pub project_id: String,
    pub rows: Vec<QuotaRow>,
    /// Set when that service failed.
    pub error: Option<String>,
}

/// Identity › Projects › *Modify Quotas* (roadmap 82): another project's
/// quota cards, with its name (the ID when Keystone didn't say).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectQuotas {
    pub project_id: String,
    pub project_name: String,
    pub cards: Vec<QuotaCard>,
}

/// *Edit Quotas*: the limits that changed (`-1` = unlimited), refused when one
/// is below what is used. Empty when nothing changed.
pub fn quota_changes(
    rows: &[QuotaRow],
    wanted: &[(String, i64)],
) -> Result<std::collections::BTreeMap<String, i64>, String> {
    let mut out = std::collections::BTreeMap::new();
    for (k, v) in wanted {
        let Some(row) = rows.iter().find(|r| r.key == *k) else {
            continue;
        };
        if *v < -1 {
            return Err(format!("{k}: -1 (unlimited) or more"));
        }
        if row.limit.unwrap_or(-1) == *v {
            continue;
        }
        // Horizon's check; Nova refuses it anyway, Neutron would accept it.
        if let Some(used) = row.used.filter(|u| *v != -1 && *v < *u) {
            return Err(format!("{k}: {v} is below the {used} in use"));
        }
        out.insert(k.clone(), *v);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Admin dashboard (roadmap 85)
// ---------------------------------------------------------------------------

/// Admin › System › System Information: Horizon's four tabs. Each list that
/// failed keeps the others, as Project › Quotas does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SystemInfo {
    /// *Services*: the token's catalog, every region.
    pub services: Vec<CatalogService>,
    /// *Compute Services* (Nova `os-services`); `None` without Nova in the catalog.
    pub compute: Option<Fallible<HostService>>,
    /// *Block Storage Services* (Cinder `os-services`); `None` without Cinder.
    pub volume: Option<Fallible<HostService>>,
    /// *Network Agents* (Neutron `agents`); `None` without Neutron or its
    /// `agent` extension.
    pub agents: Option<Fallible<NetworkAgent>>,
}

/// A list that may have failed on its own: `error` set, `rows` empty.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fallible<T> {
    pub rows: Vec<T>,
    pub error: Option<String>,
}

impl<T> From<Result<Vec<T>, AppError>> for Fallible<T> {
    fn from(r: Result<Vec<T>, AppError>) -> Self {
        match r {
            Ok(rows) => Fallible { rows, error: None },
            Err(e) => Fallible {
                rows: vec![],
                error: Some(e.to_string()),
            },
        }
    }
}

/// One catalog service in one region, with each of its endpoints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogService {
    pub name: String,
    pub service_type: String,
    /// `-` when the endpoints name no region.
    pub region: String,
    /// `(interface, url)`, sorted by interface (`admin`, `internal`, `public`).
    pub endpoints: Vec<(String, String)>,
}

/// A Nova or Cinder `os-services` entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostService {
    /// Nova's service ID: a UUID from compute 2.53 (Admin's list asks for it),
    /// the target of *Enable* / *Disable Service*. Empty when Nova sent none.
    #[serde(default)]
    pub id: String,
    pub binary: String,
    pub host: String,
    pub zone: Option<String>,
    /// `status` is `enabled` (an operator can disable a service).
    pub enabled: bool,
    /// `state` is `up` (the service reports in).
    pub up: bool,
    /// Last report, RFC 3339 UTC; `None` when never reported.
    pub updated_at: Option<String>,
    pub disabled_reason: Option<String>,
}

/// A Neutron agent (`GET /v2.0/agents`, extension `agent`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetworkAgent {
    pub agent_type: String,
    pub binary: String,
    pub host: String,
    pub zone: Option<String>,
    pub admin_up: bool,
    pub alive: bool,
    /// Last heartbeat, RFC 3339 UTC.
    pub heartbeat: Option<String>,
}

/// A Glance metadata definitions namespace (Admin › System › Metadata
/// Definitions): the vocabularies offered by *Update Metadata* dialogs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetadefNamespace {
    /// The namespace's ID (`OS::Compute::Watchdog`).
    pub namespace: String,
    /// `display_name`, else the namespace.
    pub name: String,
    pub description: Option<String>,
    /// Resource types it applies to (`OS::Nova::Flavor`, `OS::Glance::Image`…).
    pub resource_types: Vec<String>,
    /// `visibility` is `public`.
    pub public: bool,
    pub protected: bool,
    /// The associations that have a `prefix` (`OS::Nova::Flavor` → `hw:`), for
    /// *Update Associations* (roadmap 89).
    #[serde(default)]
    pub prefixes: Vec<(String, String)>,
}

/// Admin › Metadata Definitions › a namespace (roadmap 89): Glance's
/// `GET metadefs/namespaces/{namespace}`, shown as Horizon's *Overview* and
/// *Contents* tabs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetadefNamespaceDetail {
    /// The list's fields (for the row menu, too).
    pub namespace: MetadefNamespace,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub associations: Vec<MetadefAssociation>,
    /// The namespace's own properties, by name.
    pub properties: Vec<MetadefProperty>,
    /// Its objects, by name.
    pub objects: Vec<MetadefObject>,
    /// The whole definition, pretty-printed (Horizon's *Contents* shows it).
    pub raw: String,
}

/// A resource type a namespace applies to, with how its properties are named there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetadefAssociation {
    pub name: String,
    /// Put before each property's name on that resource type (`hw:`).
    pub prefix: Option<String>,
    /// Where on the resource they go (`image`, `scheduler_hints`…).
    pub properties_target: Option<String>,
}

/// A metadef property: the key a resource can carry and what its value may be.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetadefProperty {
    pub name: String,
    pub title: Option<String>,
    /// JSON-schema `type` (`string`, `integer`, `boolean`, `array`…).
    pub kind: String,
    pub description: Option<String>,
    /// The default, as text (JSON for a non-string).
    pub default: Option<String>,
    /// `enum` (or an array's `items.enum`), as text.
    pub allowed: Vec<String>,
}

/// A metadef object: a named group of properties.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetadefObject {
    pub name: String,
    pub description: Option<String>,
    /// Its properties' names, sorted.
    pub properties: Vec<String>,
}

/// How a catalog property is typed (Glance metadef `type`): picks the value input of the
/// *Update Metadata* picker (roadmap 25). Arrays and objects fall back to `Text`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataKind {
    Text,
    Integer,
    Number,
    Boolean,
}

/// One property a Glance metadata definitions namespace offers for a resource type
/// (`OS::Glance::Image`, `OS::Nova::Server`, `OS::Cinder::Volume`, `OS::Cinder::Snapshot`):
/// a suggestion in *Update Metadata*, as Horizon's *Available Metadata* column.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetadataOption {
    /// The key to store: the property name with the association's `prefix` applied
    /// (`hw:` + `cpu_policy`).
    pub key: String,
    /// `title`, else the property name.
    pub title: String,
    pub description: Option<String>,
    pub kind: MetadataKind,
    /// Allowed values (`enum`), as text; empty = free input.
    pub choices: Vec<String>,
    /// `default`, as text.
    pub default: Option<String>,
    /// The namespace's display name, to group the suggestions.
    pub namespace: String,
}

/// Prefix of the image properties Glance manages itself (`os_glance_failed_import`, …).
pub const GLANCE_PROPERTY_PREFIX: &str = "os_glance_";

/// Whether the image metadata editors hide this key and the server refuses to write it:
/// Glance's own `os_glance_*`, or one of `[images] reserved_properties` (roadmap 25).
pub fn is_reserved_image_key(key: &str, reserved: &[String]) -> bool {
    key.starts_with(GLANCE_PROPERTY_PREFIX) || reserved.iter().any(|r| r == key)
}

/// The resource types whose metadata the catalog can suggest for (the only values
/// `list_metadata_options` accepts): the ones Moonlight has a key/value editor for.
pub const METADATA_RESOURCE_TYPES: [&str; 6] = [
    "OS::Glance::Image",
    "OS::Nova::Server",
    "OS::Cinder::Volume",
    "OS::Cinder::Snapshot",
    "OS::Nova::Flavor",
    "OS::Nova::Aggregate",
];

/// A row of an Admin list of every project's resources: the Project panel's
/// row plus Horizon's *Project* and, where it has one, *Host* column.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Owned<T> {
    /// The owner's name, else its ID; `-` when the API doesn't say.
    pub project: String,
    /// Nova's `OS-EXT-SRV-ATTR:host`, Cinder's `os-vol-host-attr:host`.
    pub host: Option<String>,
    /// The owner's ID (admin *Associate* offers that project's ports).
    #[serde(default)]
    pub project_id: Option<String>,
    pub row: T,
}

/// Admin › Compute › Hypervisors: the hypervisors and Horizon's *Compute Host*
/// tab (the `nova-compute` services).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hypervisors {
    pub hypervisors: Vec<HypervisorRow>,
    pub hosts: Fallible<HostService>,
    /// Whether the catalog has Placement: the *Resource Provider* tab, loaded
    /// only when opened (`get_admin_resource_providers`; two calls per provider).
    #[serde(default)]
    pub placement: bool,
}

impl Hypervisors {
    /// Horizon's *Hypervisor Summary*: VCPUs, RAM (MiB) and local disk (GiB),
    /// `(used, total)` summed over every hypervisor.
    pub fn totals(&self) -> [(i64, i64); 3] {
        let sum = |f: fn(&HypervisorRow) -> (i64, i64)| {
            self.hypervisors
                .iter()
                .map(f)
                .fold((0, 0), |(u, t), (a, b)| (u + a, t + b))
        };
        [
            sum(|h| (h.vcpus_used, h.vcpus)),
            sum(|h| (h.ram_used_mb, h.ram_mb)),
            sum(|h| (h.disk_used_gb, h.disk_gb)),
        ]
    }
}

/// One resource class of a Placement provider: its usage and inventory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderResource {
    pub used: i64,
    pub reserved: i64,
    pub total: i64,
    /// Overcommit factor (`16.0` for VCPUs by default): what can be allocated
    /// is `(total - reserved) × allocation_ratio`.
    pub allocation_ratio: f64,
}

/// A Placement resource provider (compute node, or a nested one such as a
/// GPU or a NUMA cell), with Horizon's VCPU, RAM and disk figures.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceProviderRow {
    pub uuid: String,
    pub name: String,
    /// `None` when the provider has no inventory of that class (nested ones).
    pub vcpus: Option<ProviderResource>,
    pub memory_mb: Option<ProviderResource>,
    pub disk_gb: Option<ProviderResource>,
    /// `parent_provider_uuid` (Placement 1.14); `None` for a compute node.
    #[serde(default)]
    pub parent: Option<String>,
    /// 0 for a compute node, 1 for its children (a GPU device)…: set by
    /// [`provider_tree`], which also shortens the children's names.
    #[serde(default)]
    pub depth: u8,
    /// The compute node's UUID (its own for a node): the tab collapses a host's
    /// rows by it. Set by [`provider_tree`].
    #[serde(default)]
    pub host: String,
    /// How many providers sit under this one (all levels): a host's toggle
    /// shows it. Set by [`provider_tree`].
    #[serde(default)]
    pub descendants: usize,
    /// `VGPU` (one unit per vGPU the physical GPU can host). On a compute node
    /// with GPUs, the sum of its children's.
    #[serde(default)]
    pub vgpus: Option<ProviderResource>,
    /// Every other class (PCI devices, `CUSTOM_*`…), by name.
    #[serde(default)]
    pub others: Vec<(String, ProviderResource)>,
    /// The provider's `CUSTOM_*` traits (on a GPU, how operators and Nova name
    /// its vGPU type, e.g. `CUSTOM_NVIDIA_468`); asked only for GPU providers.
    #[serde(default)]
    pub custom_traits: Vec<String>,
}

/// The tab's rows as a tree: each compute node (no parent) by name, then its
/// descendants under it, children named without their host's prefix
/// (`cmp-0_pci_0000_84_00_0` → `pci 0000:84:00.0`, Nova's naming). A node with
/// GPUs gets the sum of their `VGPU` as its own. Orphans (parent not listed) stay
/// at the top level.
pub fn provider_tree(rows: Vec<ResourceProviderRow>) -> Vec<ResourceProviderRow> {
    let known: std::collections::HashSet<String> = rows.iter().map(|r| r.uuid.clone()).collect();
    let (mut roots, children): (Vec<_>, Vec<_>) = rows
        .into_iter()
        .partition(|r| r.parent.as_ref().is_none_or(|p| !known.contains(p)));
    roots.sort_by_key(|r| r.name.to_lowercase());
    let mut out = Vec::new();
    fn add(
        node: ResourceProviderRow,
        depth: u8,
        host: (&str, &str),
        children: &[ResourceProviderRow],
        out: &mut Vec<ResourceProviderRow>,
    ) {
        let mut kids: Vec<ResourceProviderRow> = children
            .iter()
            .filter(|c| c.parent.as_deref() == Some(node.uuid.as_str()))
            .cloned()
            .collect();
        kids.sort_by_key(|c| c.name.to_lowercase());
        let (host_uuid, host_name) = if depth == 0 {
            (node.uuid.clone(), node.name.clone())
        } else {
            (host.0.to_string(), host.1.to_string())
        };
        let mut node = node;
        node.depth = depth;
        node.host = host_uuid.clone();
        if depth > 0 {
            node.name = short_provider_name(&host_name, &node.name);
        }
        if node.vgpus.is_none() && !kids.is_empty() {
            node.vgpus = sum_resources(kids.iter().filter_map(|k| k.vgpus.as_ref()));
        }
        let at = out.len();
        out.push(node);
        for k in kids {
            add(k, depth.saturating_add(1), (&host_uuid, &host_name), children, out);
        }
        out[at].descendants = out.len() - at - 1;
        // A collapsed host still says which vGPU types it has: its devices' traits.
        if out[at].custom_traits.is_empty() {
            let mut types: Vec<String> = out[at + 1..].iter().flat_map(|d| d.custom_traits.clone()).collect();
            types.sort();
            types.dedup();
            out[at].custom_traits = types;
        }
    }
    for r in roots {
        add(r, 0, ("", ""), &children, &mut out);
    }
    out
}

/// `cmp-0_pci_0000_84_00_0` under `cmp-0` → `pci 0000:84:00.0`; other names
/// lose only the host prefix.
fn short_provider_name(host: &str, name: &str) -> String {
    let rest = name
        .strip_prefix(host)
        .and_then(|r| r.strip_prefix('_'))
        .unwrap_or(name);
    match rest.strip_prefix("pci_") {
        Some(addr) => {
            let p: Vec<&str> = addr.split('_').collect();
            match p.as_slice() {
                [domain, bus, slot, func] => format!("pci {domain}:{bus}:{slot}.{func}"),
                _ => rest.to_string(),
            }
        }
        None => rest.to_string(),
    }
}

/// Used, reserved and total summed (allocation ratio of the first); `None` when empty.
fn sum_resources<'a>(rs: impl Iterator<Item = &'a ProviderResource>) -> Option<ProviderResource> {
    rs.fold(None, |acc: Option<ProviderResource>, r| {
        Some(match acc {
            None => r.clone(),
            Some(a) => ProviderResource {
                used: a.used + r.used,
                reserved: a.reserved + r.reserved,
                total: a.total + r.total,
                allocation_ratio: a.allocation_ratio,
            },
        })
    })
}

/// The tab's vGPU summary over [`provider_tree`]'s rows: `(in use, total, GPUs,
/// hosts)`. A GPU is a provider holding `VGPU` itself, not a host's sum of its
/// children (older Nova puts `VGPU` on the compute node, which then counts as one).
pub fn vgpu_summary(rows: &[ResourceProviderRow]) -> (i64, i64, usize, usize) {
    let (mut used, mut total, mut gpus, mut hosts) = (0, 0, 0, 0);
    for (i, r) in rows.iter().enumerate() {
        let Some(v) = &r.vgpus else { continue };
        if r.depth == 0 {
            hosts += 1;
        }
        let summed = rows[i + 1..]
            .iter()
            .take_while(|c| c.depth > r.depth)
            .any(|c| c.vgpus.is_some());
        if !summed {
            gpus += 1;
            used += v.used;
            total += v.total;
        }
    }
    (used, total, gpus, hosts)
}

/// One hypervisor (compute 2.53: usage fields are gone from 2.88).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HypervisorRow {
    pub id: String,
    pub hostname: String,
    /// `hypervisor_type` (`QEMU`, `ironic`…).
    pub kind: String,
    pub vcpus_used: i64,
    pub vcpus: i64,
    pub ram_used_mb: i64,
    pub ram_mb: i64,
    pub disk_used_gb: i64,
    pub disk_gb: i64,
    /// `running_vms`.
    pub instances: i64,
    pub up: bool,
    pub enabled: bool,
}

/// Admin › Compute › Host Aggregates, with Horizon's *Availability Zones* table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aggregates {
    pub aggregates: Vec<AggregateRow>,
    pub zones: Fallible<ZoneRow>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AggregateRow {
    pub id: String,
    pub name: String,
    pub zone: Option<String>,
    pub hosts: Vec<String>,
    /// Sorted by key; `availability_zone` included, as Horizon shows it.
    pub metadata: Vec<(String, String)>,
}

/// A compute availability zone (`os-availability-zone/detail`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZoneRow {
    pub name: String,
    pub hosts: Vec<String>,
    pub available: bool,
}

/// Admin › Compute › Flavors: public and private flavors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlavorRow {
    pub id: String,
    pub name: String,
    pub vcpus: i64,
    pub ram_mb: i64,
    pub disk_gb: i64,
    pub ephemeral_gb: i64,
    /// MB; Nova sends `""` for none.
    pub swap_mb: i64,
    pub rxtx_factor: Option<f64>,
    pub public: bool,
    /// Extra specs, sorted by key (Horizon's *Metadata* column says whether
    /// there are any).
    pub extra_specs: Vec<(String, String)>,
    /// Projects allowed to use a private flavor (`os-flavor-access`): `(ID,
    /// name)`, the ID as name when Keystone doesn't name it, sorted by name.
    /// Empty for public flavors, and when the access list could not be read.
    #[serde(default)]
    pub access: Vec<(String, String)>,
}

/// Admin › Volume › Volume Types, with Horizon's *QoS Specs* table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeTypes {
    pub types: Vec<VolumeTypeRow>,
    pub qos: Fallible<QosSpecRow>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeTypeRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// The associated QoS spec (`qos_specs_id`), named when the list has it.
    pub qos: Option<Ref>,
    /// Has an encryption spec (`types/{id}/encryption` is not empty).
    pub encrypted: bool,
    pub public: bool,
    pub extra_specs: Vec<(String, String)>,
    /// The encryption spec, when `encrypted` (prefills *Update Encryption*).
    #[serde(default)]
    pub encryption: Option<EncryptionSpec>,
}

/// A volume type's encryption (`types/{id}/encryption`), Horizon's form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncryptionSpec {
    /// The encryptor: `luks`, `plain`, or a class path
    /// (`nova.volume.encryptors.luks.LuksEncryptor`).
    pub provider: String,
    /// `front-end` (Nova encrypts) or `back-end` (the storage does).
    pub control_location: String,
    /// e.g. `aes-xts-plain64`; `None`: the provider's default.
    pub cipher: Option<String>,
    /// Bits; `None`: the provider's default.
    pub key_size: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QosSpecRow {
    pub id: String,
    pub name: String,
    /// `front-end`, `back-end` or `both`.
    pub consumer: String,
    pub specs: Vec<(String, String)>,
}

/// Admin › Volume › Group Types.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupTypeRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub public: bool,
    /// `group_specs` (e.g. `consistent_group_snapshot_enabled`).
    #[serde(default)]
    pub specs: Vec<(String, String)>,
}

/// Admin › Network › RBAC Policies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RbacPolicyRow {
    pub id: String,
    /// The policy's owner, named.
    pub project: String,
    /// `network`, `qos_policy`, `security_group`, `address_scope`…
    pub object_type: String,
    /// The object, named when the list knows it (networks, QoS policies,
    /// security groups), else its ID.
    pub object: Ref,
    /// The project it is shared with, named; `*` for every project.
    pub target: String,
    /// The target's ID (`*` for every project): *Edit Policy* preselects it.
    #[serde(default)]
    pub target_id: String,
    /// `access_as_shared` or `access_as_external`.
    pub action: String,
}

/// A catalog endpoint for *API Access* (empty `url`: no endpoint in this region).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub service_type: String,
    pub name: Option<String>,
    pub url: String,
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

/// How the server treats mutating actions (`MOONLIGHT_WRITES`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WriteMode {
    /// Actions are rejected.
    Off,
    /// Requests are built with the SDK and returned, never sent.
    #[default]
    DryRun,
    /// Requests are sent to OpenStack.
    On,
}

impl WriteMode {
    /// `[writes] mode` / `MOONLIGHT_WRITES`, with common spellings (`dry_run`,
    /// `true`, `read-only`…); `None` for anything else.
    pub fn parse(s: &str) -> Option<WriteMode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" | "false" | "0" | "read-only" | "readonly" => Some(WriteMode::Off),
            "dry-run" | "dryrun" | "dry_run" => Some(WriteMode::DryRun),
            "on" | "true" | "1" | "live" => Some(WriteMode::On),
            _ => None,
        }
    }
}

/// One HTTP request an action produces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RequestPreview {
    pub service: String,
    pub method: String,
    pub path: String,
    /// Pretty-printed JSON body, if any.
    pub body: Option<String>,
}

/// What an instance *Log* / *Console* call returned (roadmap 31): `data` is
/// `None` unless the request was sent (`[writes] mode = on`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConsoleReply<T> {
    pub mode: WriteMode,
    pub requests: Vec<RequestPreview>,
    pub data: Option<T>,
}

/// A remote console URL from Nova (`vnc` / `novnc`, `spice` / `spice-html5`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteConsole {
    pub protocol: String,
    pub kind: String,
    pub url: String,
}

impl RemoteConsole {
    /// The URL for the *Console* tab's frame (roadmap 99): noVNC asked to scale
    /// the remote screen to the frame (`scale=true` for `vnc_lite.html` and
    /// `vnc_auto.html`, `resize=scale` for `vnc.html`; each page ignores the
    /// other's). Never `resize=remote`, which changes the guest's resolution.
    /// Every parameter Nova put there (the token) is kept, one already set is not
    /// repeated, and SPICE's page, which has no such option, is left as it is.
    pub fn frame_url(&self) -> String {
        if !(self.kind == "novnc" || self.protocol == "vnc") {
            return self.url.clone();
        }
        let (base, fragment) = self
            .url
            .split_once('#')
            .map_or((self.url.as_str(), None), |(b, f)| (b, Some(f)));
        let query = base.split_once('?').map(|(_, q)| q).unwrap_or("");
        let has = |key: &str| query.split('&').any(|p| p.split('=').next() == Some(key));
        let mut out = base.to_string();
        for (key, value) in [("scale", "true"), ("resize", "scale")] {
            if !has(key) {
                out.push(if out.contains('?') { '&' } else { '?' });
                out.push_str(&format!("{key}={value}"));
            }
        }
        if let Some(f) = fragment {
            out.push('#');
            out.push_str(f);
        }
        out
    }
}

/// What `run_action` did: the requests built (sent or not, per `mode`), a one-time
/// secret, and per-item results for batches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActionOutcome {
    pub mode: WriteMode,
    pub requests: Vec<RequestPreview>,
    /// Shown to the user once (e.g. a generated private key); never logged.
    pub secret: Option<Secret>,
    /// One entry per item of an [`OsAction::Batch`], in order; empty otherwise.
    #[serde(default)]
    pub results: Vec<ItemResult>,
    /// The ID of the last resource the action created (`None` in dry-run, where nothing
    /// exists): *Create Image* › *File* uploads to it (roadmap 45).
    #[serde(default)]
    pub created: Option<String>,
}

impl ActionOutcome {
    /// Without the REST requests unless `[ui] show_requests` (roadmap 78): the
    /// server clears them, so they never reach the browser. Dry-run included.
    pub fn shown(mut self, show_requests: bool) -> Self {
        if !show_requests {
            self.requests.clear();
        }
        self
    }
}

impl<T> ConsoleReply<T> {
    /// As [`ActionOutcome::shown`].
    pub fn shown(mut self, show_requests: bool) -> Self {
        if !show_requests {
            self.requests.clear();
        }
        self
    }
}

/// How one item of a batch went (`error` is `None` when it succeeded).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemResult {
    pub label: String,
    pub error: Option<String>,
}

/// Most items a single [`OsAction::Batch`] may carry.
pub const MAX_BATCH: usize = 100;

/// A file the user must save now. `Debug` hides the content so it cannot end
/// up in a log line by accident.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Secret {
    pub file_name: String,
    pub content: String,
    #[serde(default)]
    pub kind: SecretKind,
    /// Values shown in the box, `(key, value)`; keys `id`, `name`, `secret`.
    #[serde(default)]
    pub fields: Vec<(String, String)>,
    /// More downloads, `(file name, content)` (an application credential's
    /// `openrc` and `clouds.yaml`, roadmap 57).
    #[serde(default)]
    pub extra: Vec<(String, String)>,
}

/// What the one-time secret is, for the wording of the download box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecretKind {
    #[default]
    PrivateKey,
    /// Nova's `adminPass` (rescue).
    Password,
    /// A volume transfer's ID and authorization key, for the receiving project.
    Transfer,
    /// An application credential's ID and secret (roadmap 57).
    AppCredential,
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secret")
            .field("file_name", &self.file_name)
            .field("content", &"<redacted>")
            .finish()
    }
}

/// *Soft* (ACPI) or *Hard* (power cycle) reboot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebootKind {
    Soft,
    Hard,
}

/// What a new instance boots from (*Launch Instance* → *Source*).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootSource {
    Image,
    Volume,
    VolumeSnapshot,
}

/// The *Launch Instance* wizard's result, turned into one Nova `POST /servers`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LaunchSpec {
    pub name: String,
    pub availability_zone: Option<String>,
    pub count: u32,
    pub source: BootSource,
    pub source_id: String,
    /// Image source only: create a new volume from the image (boot from volume).
    pub create_volume: bool,
    pub volume_size_gb: Option<u32>,
    pub delete_volume_on_terminate: bool,
    pub flavor_id: String,
    pub network_ids: Vec<String>,
    /// Existing ports (Horizon's *Network Ports* step), after the networks.
    #[serde(default)]
    pub port_ids: Vec<String>,
    pub security_groups: Vec<String>,
    pub key_name: Option<String>,
    pub server_group_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Customization script (cloud-init user data), plain text; base64 on send.
    #[serde(default)]
    pub user_data: Option<String>,
    #[serde(default)]
    pub config_drive: bool,
    /// `OS-DCF:diskConfig`: `true` = MANUAL, `false` = AUTO (Horizon's default).
    #[serde(default)]
    pub manual_disk_partition: bool,
    /// `adminPass` (only offered with `[launch] can_set_password`). Masked in previews.
    #[serde(default)]
    pub admin_pass: Option<String>,
}

/// Horizon's limit for the customization script.
pub const MAX_USER_DATA_BYTES: usize = 16 * 1024;

/// What a new volume starts from (*Create Volume* → *Volume Source*).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolumeSource {
    Empty,
    Image,
    Snapshot,
    Volume,
}

/// Horizon's *Action and Object Type* choices of *Create RBAC Policy*:
/// `(form value, Neutron object_type, Neutron action)`.
pub const RBAC_KINDS: &[(&str, &str, &str)] = &[
    ("shared_network", "network", "access_as_shared"),
    ("external_network", "network", "access_as_external"),
    ("shared_qos_policy", "qos_policy", "access_as_shared"),
];

/// A [`RBAC_KINDS`] form value → Neutron's `(object_type, action)`.
pub fn rbac_kind(kind: &str) -> Result<(&'static str, &'static str), String> {
    RBAC_KINDS
        .iter()
        .find(|(k, _, _)| *k == kind)
        .map(|(_, ty, action)| (*ty, *action))
        .ok_or_else(|| format!("unknown RBAC policy kind {kind:?}"))
}

/// Provider network types Horizon offers (`OPENSTACK_NEUTRON_NETWORK`
/// `supported_provider_types` = `*`), with their segmentation ID range
/// (Horizon's `segmentation_id_range` defaults); `None` = no segmentation ID.
/// Horizon's *Update Volume Status* choices (admin `VolumeStatusForm`):
/// Cinder's `os-reset_status` accepts any string, Horizon offers these.
pub const VOLUME_STATUSES: &[&str] = &[
    "attaching",
    "available",
    "creating",
    "deleting",
    "detaching",
    "error",
    "error_deleting",
    "error_extending",
    "in-use",
    "maintenance",
    "reserved",
];

/// A QoS spec's `consumer`, Horizon's choices.
pub const QOS_CONSUMERS: &[&str] = &["back-end", "front-end", "both"];

/// An encryption spec's `control_location`.
pub const CONTROL_LOCATIONS: &[&str] = &["front-end", "back-end"];

/// Horizon's *Update Status* choices for a volume snapshot.
pub const SNAPSHOT_STATUSES: &[&str] = &["available", "creating", "deleting", "error", "error_deleting"];

/// Admin › Volumes' *Manage Volume* (roadmap 85), Horizon's form: a volume
/// that exists on a back end but not in Cinder, found by name or ID there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManageVolumeSpec {
    /// The back end's name or ID of the volume (`ref`).
    pub identifier: String,
    /// `true`: `identifier` is the back end's ID (`source-id`), else its name
    /// (`source-name`).
    pub by_id: bool,
    /// The Cinder host it lives on, `host@backend#pool`.
    pub host: String,
    pub name: Option<String>,
    pub description: Option<String>,
    /// `None`: Cinder's default type.
    pub volume_type: Option<String>,
    pub availability_zone: Option<String>,
    pub bootable: bool,
    pub metadata: Vec<(String, String)>,
}

pub const PROVIDER_NETWORK_TYPES: &[(&str, Option<(u32, u32)>)] = &[
    ("local", None),
    ("flat", None),
    ("vlan", Some((1, 4094))),
    ("gre", Some((1, u32::MAX))),
    ("vxlan", Some((1, (1 << 24) - 1))),
    ("geneve", Some((1, (1 << 24) - 1))),
];

/// Neutron's `provider:*` attributes of a new network.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderNetwork {
    /// One of [`PROVIDER_NETWORK_TYPES`].
    pub network_type: String,
    /// Required for `flat` and `vlan`, not sent for the others.
    pub physical_network: Option<String>,
    /// Optional for the types that have one (Neutron picks it otherwise).
    pub segmentation_id: Option<u32>,
}

impl ProviderNetwork {
    /// Horizon's checks of its *Create Network* form: a type of `types` (the
    /// configured ones, `NetworkPrefs::provider_types`), a physical network
    /// exactly where the type needs one, a segmentation ID in its range.
    pub fn validate(&self, types: &[crate::prefs::ProviderType]) -> Result<(), String> {
        let Some((_, range)) = types.iter().find(|(t, _)| *t == self.network_type) else {
            return Err(format!("unknown provider network type {:?}", self.network_type));
        };
        let needs_physical = matches!(self.network_type.as_str(), "flat" | "vlan");
        match (&self.physical_network, needs_physical) {
            (None, true) => return Err(format!("a {} network needs a physical network", self.network_type)),
            (Some(_), false) => return Err(format!("a {} network has no physical network", self.network_type)),
            _ => {}
        }
        match (self.segmentation_id, range) {
            (Some(_), None) => Err(format!("a {} network has no segmentation ID", self.network_type)),
            (Some(id), Some((lo, hi))) if id < *lo || id > *hi => Err(format!(
                "segmentation ID {id} is outside {lo}–{hi} for {}",
                self.network_type
            )),
            _ => Ok(()),
        }
    }
}

/// Admin › Networks' *Create Network*: Project's fields plus the owner, the
/// provider attributes and the admin-only flags.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdminNetworkSpec {
    pub name: String,
    /// The owner (`project_id` in the body).
    pub project_id: String,
    /// `None`: Neutron's default tenant network type.
    pub provider: Option<ProviderNetwork>,
    pub admin_up: bool,
    pub shared: bool,
    /// `router:external`.
    pub external: bool,
    /// `None`: Neutron's default (the physical network's, less overhead).
    #[serde(default)]
    pub mtu: Option<u32>,
    /// `availability_zone_hints` (extension `network_availability_zone`); empty = none sent.
    #[serde(default)]
    pub az_hints: Vec<String>,
    pub subnet: Option<SubnetSpec>,
}

/// Admin › Flavors' *Create Flavor* (roadmap 85), Horizon's form. A flavor
/// with `projects` is private and those projects get access (Horizon's
/// *Flavor Access* tab); without, it is public.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlavorSpec {
    pub name: String,
    /// `None`: Nova generates a UUID (Horizon's `auto`).
    pub flavor_id: Option<String>,
    pub vcpus: i64,
    pub ram_mb: i64,
    /// Root disk, GB.
    pub disk_gb: i64,
    pub ephemeral_gb: i64,
    pub swap_mb: i64,
    /// `None`: Nova's default (1.0).
    pub rxtx_factor: Option<f64>,
    /// Project IDs given access; empty = public.
    pub projects: Vec<String>,
}

/// Admin › Routers' *Create Router* (roadmap 85): Project's fields plus the
/// owner and Horizon's admin options. `None` and empty fields are left out of
/// the body, so Neutron applies its defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdminRouterSpec {
    pub name: String,
    /// The owner (`tenant_id` in the body).
    pub project_id: String,
    pub admin_up: bool,
    /// The gateway's network; `None`: no gateway.
    pub external_network_id: Option<String>,
    /// The gateway's `enable_snat` (`ext-gw-mode`); only with a gateway.
    pub enable_snat: Option<bool>,
    /// `availability_zone_hints` (extension `router_availability_zone`).
    pub az_hints: Vec<String>,
    /// *Router Type*: `Some(true)` distributed, `Some(false)` centralized (`dvr`).
    pub distributed: Option<bool>,
    /// *High Availability Mode* (`l3-ha`).
    pub ha: Option<bool>,
}

/// A subnet to create, alone or within *Create Network*.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubnetSpec {
    pub name: String,
    pub cidr: String,
    pub ip_version: u8,
    /// `None` = let Neutron pick the first address; `Some("")` = no gateway.
    pub gateway_ip: Option<String>,
    pub enable_dhcp: bool,
    pub dns_nameservers: Vec<String>,
    /// "start,end" per pool
    pub allocation_pools: Vec<(String, String)>,
    /// IPv6 only: Horizon's *IPv6 Address Configuration Mode*, as Neutron's
    /// `ipv6_ra_mode` and `ipv6_address_mode` (`slaac`, `dhcpv6-stateful`,
    /// `dhcpv6-stateless`); `None` = not sent.
    #[serde(default)]
    pub ipv6_ra_mode: Option<String>,
    #[serde(default)]
    pub ipv6_address_mode: Option<String>,
    /// Extra routes handed out by DHCP: `(destination CIDR, next hop)`.
    #[serde(default)]
    pub host_routes: Vec<(String, String)>,
}

/// Horizon's *IPv6 Address Configuration Mode* choices: the form value
/// `ra/address` (`none` = not sent) and what each one means.
pub const IPV6_MODES: &[&str] = &[
    "none/none",
    "slaac/slaac",
    "none/slaac",
    "dhcpv6-stateful/dhcpv6-stateful",
    "dhcpv6-stateless/dhcpv6-stateless",
];

/// `ra/address` (an [`IPV6_MODES`] value) → `(ipv6_ra_mode, ipv6_address_mode)`.
pub fn ipv6_modes(v: &str) -> Result<(Option<String>, Option<String>), String> {
    if !IPV6_MODES.contains(&v) {
        return Err(format!("unknown IPv6 address mode {v:?}"));
    }
    let (ra, addr) = v.split_once('/').unwrap_or((v, v));
    let some = |m: &str| (m != "none").then(|| m.to_string());
    Ok((some(ra), some(addr)))
}

/// A security group rule to create, Horizon's *Add Rule* resolved to Neutron fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuleSpec {
    pub security_group_id: String,
    pub direction: String,
    pub ethertype: String,
    /// `None` = any protocol
    pub protocol: Option<String>,
    pub port_min: Option<i32>,
    pub port_max: Option<i32>,
    pub remote_ip_prefix: Option<String>,
    pub remote_group_id: Option<String>,
    pub description: Option<String>,
}

/// Every mutating operation Moonlight can perform.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OsAction {
    /// Project › Quotas' *Edit Quotas* (roadmap 82): only the changed keys,
    /// each one of `service.keys()`.
    QuotaUpdate {
        service: QuotaService,
        project_id: String,
        values: std::collections::BTreeMap<String, i64>,
    },
    /// Admin › Defaults' *Update Defaults* (roadmap 85): the `default` quota
    /// class of Compute or Volume (`PUT os-quota-class-sets/default`); Neutron's
    /// defaults live in its configuration, so it has none. Admins only.
    DefaultQuotaUpdate {
        service: QuotaService,
        values: std::collections::BTreeMap<String, i64>,
    },
    // Admin › Metadata Definitions (roadmap 85); `id` is the namespace
    // (`OS::Compute::Watchdog`). Every one `admin_only`.
    /// *Import Namespace*: `POST /v2/metadefs/namespaces` with `definition`
    /// (Glance's JSON: namespace, properties, objects…), its `visibility` and
    /// `protected` set from the form, as Horizon.
    MetadefNamespaceImport {
        definition: String,
        public: bool,
        protected: bool,
    },
    /// *Edit Namespace*: `PUT …/namespaces/{id}`. Glance replaces the
    /// namespace's fields, so the dialog sends them all.
    MetadefNamespaceUpdate {
        id: String,
        display_name: Option<String>,
        description: Option<String>,
        public: bool,
        protected: bool,
    },
    /// `DELETE …/namespaces/{id}` (Glance refuses a protected one).
    MetadefNamespaceDelete {
        id: String,
    },
    /// *Update Associations*: `DELETE …/{id}/resource_types/{name}` per removed
    /// resource type, then `POST …/{id}/resource_types` per added one (names
    /// such as `OS::Nova::Flavor`, in `*_type_ids` so `check_ids` vets them).
    /// `prefixes` gives an added type its `prefix` (roadmap 89); Glance can't
    /// change one in place, so a new prefix is a removal and an addition.
    MetadefNamespaceSetTypes {
        id: String,
        add_type_ids: Vec<String>,
        remove_type_ids: Vec<String>,
        #[serde(default)]
        prefixes: Vec<(String, String)>,
    },
    // Instances
    ServerLaunch(LaunchSpec),
    ServerUpdate {
        id: String,
        name: String,
    },
    ServerStart {
        id: String,
    },
    ServerStop {
        id: String,
    },
    ServerReboot {
        id: String,
        kind: RebootKind,
    },
    ServerPause {
        id: String,
    },
    ServerUnpause {
        id: String,
    },
    ServerSuspend {
        id: String,
    },
    ServerResume {
        id: String,
    },
    ServerLock {
        id: String,
    },
    ServerUnlock {
        id: String,
    },
    ServerSnapshot {
        id: String,
        name: String,
    },
    ServerDelete {
        id: String,
    },
    /// Replaces all of the server's metadata (`PUT /servers/{id}/metadata`).
    ServerSetMetadata {
        id: String,
        metadata: Vec<(String, String)>,
    },
    /// *Edit Security Groups* (roadmap 42): Nova adds/removes each group **by name** on every
    /// port of the server, one `addSecurityGroup` / `removeSecurityGroup` action per group.
    ServerSetSecurityGroups {
        id: String,
        add: Vec<String>,
        remove: Vec<String>,
    },
    ServerAttachVolume {
        server_id: String,
        volume_id: String,
    },
    ServerDetachVolume {
        server_id: String,
        volume_id: String,
    },
    ServerResize {
        id: String,
        flavor_id: String,
    },
    ServerConfirmResize {
        id: String,
    },
    ServerRevertResize {
        id: String,
    },
    /// Nova >= 2.94 (sent by the SDK), which also rebuilds volume-backed servers (>= 2.93).
    ServerRebuild {
        id: String,
        image_id: String,
    },
    ServerShelve {
        id: String,
    },
    /// With an availability zone, Nova >= 2.77.
    ServerUnshelve {
        id: String,
        availability_zone: Option<String>,
    },
    /// Nova returns the rescue `adminPass`: shown once, never logged.
    ServerRescue {
        id: String,
        image_id: Option<String>,
    },
    ServerUnrescue {
        id: String,
    },
    /// Exactly one of `network_id` / `port_id`; `fixed_ip` only with a network.
    ServerAttachInterface {
        id: String,
        network_id: Option<String>,
        port_id: Option<String>,
        fixed_ip: Option<String>,
    },
    ServerDetachInterface {
        id: String,
        port_id: String,
    },
    // Images
    /// *Edit Image*: only the fields that changed (roadmap 33).
    ImageUpdate {
        id: String,
        changes: ImageChanges,
    },
    ImageDelete {
        id: String,
    },
    /// *Create Image*: `POST /v2/images`, then the `web-download` import when it has a URL (roadmap 45).
    ImageCreate(ImageSpec),
    /// Custom properties, as JSON-Patch: `set` adds or replaces, `remove` deletes.
    ImageSetProperties {
        id: String,
        set: Vec<(String, String)>,
        remove: Vec<String>,
    },
    // Key pairs & server groups
    /// `key_type`: `ssh` or `x509` (`KEYPAIR_TYPES`).
    KeyPairImport {
        name: String,
        public_key: String,
        key_type: String,
    },
    /// Generated on the Moonlight server (Nova >= 2.92 no longer generates
    /// keys); only the public key is sent, the private key is returned once.
    KeyPairGenerate {
        name: String,
    },
    KeyPairDelete {
        name: String,
    },
    ServerGroupCreate {
        name: String,
        policy: String,
    },
    ServerGroupDelete {
        id: String,
    },
    // Volumes & snapshots
    VolumeCreate {
        name: String,
        description: Option<String>,
        size_gb: u32,
        volume_type: Option<String>,
        availability_zone: Option<String>,
        source: VolumeSource,
        source_id: Option<String>,
    },
    VolumeUpdate {
        id: String,
        name: String,
        description: Option<String>,
    },
    VolumeExtend {
        id: String,
        new_size_gb: u32,
    },
    VolumeDelete {
        id: String,
    },
    /// Replaces all of the volume's metadata (`PUT /volumes/{id}/metadata`).
    VolumeSetMetadata {
        id: String,
        metadata: Vec<(String, String)>,
    },
    /// `migrate`: Cinder may move the data to another backend (`on-demand`).
    VolumeRetype {
        id: String,
        new_type: String,
        migrate: bool,
    },
    /// Creates a Glance image from the volume; `force` uploads an in-use volume.
    VolumeUploadImage {
        id: String,
        image_name: String,
        disk_format: String,
        force: bool,
        /// Glance visibility of the new image (preset from `[images] default_visibility`).
        #[serde(default)]
        visibility: Option<String>,
    },
    /// The answer carries the transfer ID and `auth_key`: shown once, never logged.
    VolumeTransferCreate {
        volume_id: String,
        name: Option<String>,
    },
    /// `auth_key` is typed by the user; `Debug` would show it, so only `kind()`
    /// may be logged (as for every action).
    VolumeTransferAccept {
        transfer_id: String,
        auth_key: String,
    },
    VolumeSnapshotCreate {
        volume_id: String,
        name: String,
        description: Option<String>,
        force: bool,
    },
    SnapshotUpdate {
        id: String,
        name: String,
        description: Option<String>,
    },
    SnapshotDelete {
        id: String,
    },
    /// Replaces all of the snapshot's metadata (`PUT /snapshots/{id}/metadata`).
    SnapshotSetMetadata {
        id: String,
        metadata: Vec<(String, String)>,
    },
    /// `POST /groups/{id}/action` `{"delete": {"delete-volumes": …}}` (volume 3.13).
    /// Without `delete_volumes` Cinder refuses a group that still has volumes.
    VolumeGroupDelete {
        id: String,
        delete_volumes: bool,
    },
    /// *Create Group*: `POST /groups` (volume 3.13). Cinder needs the group type
    /// and at least one volume type; `None` zone lets Cinder pick.
    VolumeGroupCreate {
        name: String,
        description: Option<String>,
        group_type_id: String,
        volume_type_ids: Vec<String>,
        availability_zone: Option<String>,
    },
    /// *Clone Group* (`source_group_id`) or *Create Group* from a group snapshot
    /// (`group_snapshot_id`): `POST /groups/action` `create-from-src` (volume
    /// 3.14). Exactly one source must be set.
    VolumeGroupCreateFrom {
        name: String,
        description: Option<String>,
        source_group_id: Option<String>,
        group_snapshot_id: Option<String>,
    },
    /// *Edit Group*: `PUT /groups/{id}` with the name and description.
    VolumeGroupUpdate {
        id: String,
        name: String,
        description: Option<String>,
    },
    /// *Manage Volumes* / *Remove Volumes from Group*: `PUT /groups/{id}` with
    /// `add_volumes` / `remove_volumes` as comma-separated IDs (Cinder's format);
    /// an empty list is left out of the body.
    VolumeGroupSetVolumes {
        id: String,
        add_volume_ids: Vec<String>,
        remove_volume_ids: Vec<String>,
    },
    /// *Create Snapshot* of a group: `POST /group_snapshots` (volume 3.14).
    GroupSnapshotCreate {
        group_id: String,
        name: String,
        description: Option<String>,
    },
    /// `DELETE /group_snapshots/{id}` (volume 3.14).
    GroupSnapshotDelete {
        id: String,
    },
    /// *Create Backup*: `POST /backups`. `force` backs up an attached volume,
    /// `incremental` needs an earlier full backup of the volume; `snapshot_id`
    /// backs up that snapshot instead of the volume's current data.
    BackupCreate {
        volume_id: String,
        name: String,
        description: Option<String>,
        incremental: bool,
        force: bool,
        snapshot_id: Option<String>,
    },
    /// *Edit Backup*: `PUT /backups/{id}` (name and description; volume 3.9).
    BackupUpdate {
        id: String,
        name: String,
        description: Option<String>,
    },
    /// *Restore Backup*: `POST /backups/{id}/restore`. Without `volume_id` Cinder
    /// creates a new volume (named `name`); with it, that volume is overwritten.
    BackupRestore {
        id: String,
        volume_id: Option<String>,
        name: Option<String>,
    },
    /// `DELETE /backups/{id}`.
    BackupDelete {
        id: String,
    },
    // Networks, subnets, ports
    NetworkCreate {
        name: String,
        admin_up: bool,
        subnet: Option<SubnetSpec>,
    },
    /// Admin › Networks' *Create Network* (roadmap 85): for any project, with
    /// Neutron's `provider` attributes, *Shared* and *External*. Admins only
    /// (`admin_only`), whatever Neutron's policy says.
    AdminNetworkCreate(Box<AdminNetworkSpec>),
    NetworkUpdate {
        id: String,
        name: String,
        admin_up: bool,
    },
    /// Admin › Networks' *Edit Network*: Horizon's admin form, which adds
    /// *Shared* and *External Network* (`router:external`). Provider attributes
    /// are not editable: ML2 refuses to change them. Admins only.
    AdminNetworkUpdate {
        id: String,
        name: String,
        admin_up: bool,
        shared: bool,
        external: bool,
    },
    /// Admin › RBAC Policies' *Create RBAC Policy*: `POST /rbac-policies`.
    /// `kind` is one of [`RBAC_KINDS`] (Horizon's *Action and Object Type*);
    /// `target_project` a project ID or `*` (every project). Admins only.
    RbacPolicyCreate {
        kind: String,
        object_id: String,
        target_project: String,
    },
    /// *Edit Policy* (Horizon's *Update RBAC Policy*): `PUT /rbac-policies/{id}`
    /// with a new `target_tenant`, a project ID or `*`. Admins only.
    RbacPolicyUpdate {
        id: String,
        target_project: String,
    },
    /// `DELETE /rbac-policies/{id}`. Admins only.
    RbacPolicyDelete {
        id: String,
    },
    NetworkDelete {
        id: String,
    },
    SubnetCreate {
        network_id: String,
        spec: SubnetSpec,
    },
    SubnetUpdate {
        id: String,
        name: String,
        enable_dhcp: bool,
        dns_nameservers: Vec<String>,
    },
    SubnetDelete {
        id: String,
    },
    PortCreate {
        network_id: String,
        name: String,
        admin_up: bool,
        fixed_ip: Option<String>,
    },
    /// *Edit Port*: only the `Some` fields are sent (see [`PortSettings::changes`]).
    PortUpdate {
        id: String,
        name: Option<String>,
        admin_up: Option<bool>,
        /// `binding:vnic_type` (`normal`, `direct`, `macvtap`…); needs `binding`.
        vnic_type: Option<String>,
        /// Needs the `port-security` extension.
        port_security: Option<bool>,
        security_group_ids: Option<Vec<String>>,
    },
    /// Replaces the port's security groups (an empty list removes them all).
    PortSetSecurityGroups {
        id: String,
        security_group_ids: Vec<String>,
    },
    PortDelete {
        id: String,
    },
    // Routers
    RouterCreate {
        name: String,
        admin_up: bool,
        external_network_id: Option<String>,
        /// The gateway's QoS policy (`qos-gateway-ip`; needs the `qos` plugin).
        #[serde(default)]
        qos_policy_id: Option<String>,
    },
    /// Admin › Routers' *Create Router* (roadmap 85): for any project, with
    /// SNAT, zone hints, DVR and HA. Admins only (`admin_only`).
    AdminRouterCreate(Box<AdminRouterSpec>),
    RouterUpdate {
        id: String,
        name: String,
        admin_up: bool,
        /// `(gateway network ID, enable_snat)`, only when SNAT changes: Neutron
        /// wants the network in the same `external_gateway_info`.
        #[serde(default)]
        snat: Option<(String, bool)>,
        /// Admin *Edit Router* (roadmap 89): the new router type (`true` =
        /// distributed) and HA mode, only when they change. Neutron accepts
        /// them only while the router is down.
        #[serde(default)]
        distributed: Option<bool>,
        #[serde(default)]
        ha: Option<bool>,
    },
    RouterSetGateway {
        id: String,
        network_id: String,
    },
    RouterClearGateway {
        id: String,
    },
    RouterAddInterface {
        id: String,
        subnet_id: String,
    },
    RouterRemoveInterface {
        id: String,
        port_id: String,
    },
    /// Atomic (`extraroute-atomic`): only this route changes.
    RouterAddRoute {
        id: String,
        destination: String,
        nexthop: String,
    },
    RouterRemoveRoute {
        id: String,
        destination: String,
        nexthop: String,
    },
    /// Plain `extraroute` (no `-atomic`): replaces the router's whole route list.
    RouterSetRoutes {
        id: String,
        routes: Vec<(String, String)>,
    },
    RouterDelete {
        id: String,
    },
    // Security groups
    SecurityGroupCreate {
        name: String,
        description: Option<String>,
    },
    SecurityGroupUpdate {
        id: String,
        name: String,
        description: Option<String>,
    },
    SecurityGroupDelete {
        id: String,
    },
    RuleCreate(RuleSpec),
    RuleDelete {
        id: String,
    },
    // Floating IPs
    /// With `port_id`, Neutron allocates and associates in one request.
    FloatingIpAllocate {
        network_id: String,
        description: Option<String>,
        #[serde(default)]
        port_id: Option<String>,
        /// `qos-fip`; needs the `qos` plugin.
        #[serde(default)]
        qos_policy_id: Option<String>,
    },
    /// Admin › Floating IPs' *Allocate IP To Project* (roadmap 85): for any
    /// project, optionally a given address of the pool. Admins only.
    AdminFloatingIpAllocate {
        network_id: String,
        project_id: String,
        address: Option<String>,
        description: Option<String>,
    },
    // Admin › Compute (roadmap 85); every one `admin_only`.
    /// *Create Flavor*: `POST /flavors`, then `addTenantAccess` per project.
    FlavorCreate(Box<FlavorSpec>),
    /// *Edit Flavor* (`[admin] flavor_edit`, roadmap 89), Horizon's: Nova can't
    /// change a flavor, so `DELETE /flavors/{id}`, `POST /flavors` with the same
    /// ID (`spec.flavor_id`), then its `extra_specs` and `addTenantAccess` back.
    FlavorReplace {
        spec: Box<FlavorSpec>,
        extra_specs: Vec<(String, String)>,
    },
    /// `DELETE /flavors/{id}`.
    FlavorDelete {
        id: String,
    },
    /// *Update Metadata* of a flavor: `POST …/os-extra_specs` with `set`, then
    /// one `DELETE …/os-extra_specs/{key}` per key in `unset`.
    FlavorSetExtraSpecs {
        id: String,
        set: Vec<(String, String)>,
        unset: Vec<String>,
    },
    /// *Modify Access* of a private flavor: `addTenantAccess` /
    /// `removeTenantAccess` per project ID.
    FlavorSetAccess {
        id: String,
        add: Vec<String>,
        remove: Vec<String>,
    },
    /// *Create Host Aggregate*: `POST /os-aggregates`, then `add_host` per host.
    AggregateCreate {
        name: String,
        zone: Option<String>,
        hosts: Vec<String>,
    },
    /// *Edit Host Aggregate*: `PUT /os-aggregates/{id}`; `zone: None` clears it.
    AggregateUpdate {
        id: String,
        name: String,
        zone: Option<String>,
    },
    /// *Manage Hosts*: `add_host` / `remove_host` actions, one per host.
    AggregateSetHosts {
        id: String,
        add: Vec<String>,
        remove: Vec<String>,
    },
    /// *Update Metadata* of an aggregate: one `set_metadata`, `unset` keys as `null`.
    AggregateSetMetadata {
        id: String,
        set: Vec<(String, String)>,
        unset: Vec<String>,
    },
    /// `DELETE /os-aggregates/{id}` (Nova refuses one that still has hosts).
    AggregateDelete {
        id: String,
    },
    /// *Enable* / *Disable Service* of a `nova-compute`: `PUT /os-services/{id}`
    /// (compute 2.53, UUID `id`); `reason` only when disabling.
    ComputeServiceSet {
        id: String,
        enabled: bool,
        reason: Option<String>,
    },
    /// *Migrate Instance* (cold): `migrate` action; `host` needs compute 2.56.
    ServerMigrate {
        id: String,
        host: Option<String>,
    },
    /// *Live Migrate Instance*: `os-migrateLive` at compute 2.30. `host: None`
    /// lets the scheduler pick; `block_migration: None` sends `"auto"` (2.25).
    ServerLiveMigrate {
        id: String,
        host: Option<String>,
        block_migration: Option<bool>,
    },
    // Admin › Volume (roadmap 85); every one `admin_only`.
    /// *Update Volume Status*: `os-reset_status` with `status` only (Cinder's
    /// record, not the back end), one of [`VOLUME_STATUSES`].
    VolumeResetStatus {
        id: String,
        status: String,
    },
    /// *Migrate Volume*: `os-migrate_volume` to a back-end pool
    /// (`host@backend#pool`); `force_host_copy` skips the driver's own migration.
    VolumeMigrate {
        id: String,
        host: String,
        force_host_copy: bool,
    },
    /// *Manage Volume*: `POST /os-volume-manage`, a back-end volume Cinder
    /// doesn't know yet becomes one, in the session's project.
    VolumeManage(Box<ManageVolumeSpec>),
    /// *Unmanage Volume*: `os-unmanage`; Cinder forgets it, the back end keeps it.
    VolumeUnmanage {
        id: String,
    },
    /// *Update Status* of a snapshot: `os-reset_status`, one of [`SNAPSHOT_STATUSES`].
    SnapshotResetStatus {
        id: String,
        status: String,
    },
    /// *Create Volume Type*: `POST /types` (raw: the SDK's `type::create` posts
    /// to `types/{id}/action`).
    VolumeTypeCreate {
        name: String,
        description: Option<String>,
        public: bool,
    },
    /// *Edit Volume Type*: `PUT /types/{id}`; an empty description clears it.
    VolumeTypeUpdate {
        id: String,
        name: String,
        description: Option<String>,
        public: bool,
    },
    /// `DELETE /types/{id}` (Cinder refuses one still in use).
    VolumeTypeDelete {
        id: String,
    },
    /// *View Extra Specs*: `POST …/extra_specs` with `set`, then one `DELETE
    /// …/extra_specs/{key}` per key in `unset`.
    VolumeTypeSetExtraSpecs {
        id: String,
        set: Vec<(String, String)>,
        unset: Vec<String>,
    },
    /// *Manage QoS Spec Association*: disassociate `current_qos_id`, then
    /// associate `qos_id` (`None`: leave it without). Cinder makes both GETs
    /// with `vol_type_id` (they change state: sent through `Exec`, as writes).
    VolumeTypeSetQos {
        id: String,
        qos_id: Option<String>,
        current_qos_id: Option<String>,
    },
    /// *Create Encryption* (`POST …/encryption`) or, with `update`, *Update
    /// Encryption* (`PUT …/encryption/provider`, as cinderclient: Cinder
    /// ignores the last segment).
    VolumeTypeEncryptionSet {
        id: String,
        spec: EncryptionSpec,
        update: bool,
    },
    /// *Delete Encryption*: `DELETE …/encryption/provider`.
    VolumeTypeEncryptionDelete {
        id: String,
    },
    /// *Create QoS Spec*: `POST /qos-specs` with the consumer and specs.
    QosSpecCreate {
        name: String,
        consumer: String,
        specs: Vec<(String, String)>,
    },
    /// *Edit Consumer*: `PUT /qos-specs/{id}` with `consumer`.
    QosSpecSetConsumer {
        id: String,
        consumer: String,
    },
    /// *Manage Specs*: `PUT /qos-specs/{id}` with `set`, then `PUT
    /// …/delete_keys` with `unset`.
    QosSpecSetSpecs {
        id: String,
        set: Vec<(String, String)>,
        unset: Vec<String>,
    },
    /// `DELETE /qos-specs/{id}` (Cinder refuses one still associated).
    QosSpecDelete {
        id: String,
    },
    /// *Create Group Type*: `POST /group_types` (volume 3.11).
    GroupTypeCreate {
        name: String,
        description: Option<String>,
        public: bool,
    },
    /// *Edit Group Type*: `PUT /group_types/{id}`.
    GroupTypeUpdate {
        id: String,
        name: String,
        description: Option<String>,
        public: bool,
    },
    /// `DELETE /group_types/{id}`.
    GroupTypeDelete {
        id: String,
    },
    /// *View Specs* of a group type: `POST …/group_specs` with `set`, then one
    /// `DELETE …/group_specs/{key}` per key in `unset`.
    GroupTypeSetSpecs {
        id: String,
        set: Vec<(String, String)>,
        unset: Vec<String>,
    },
    FloatingIpAssociate {
        id: String,
        port_id: String,
    },
    FloatingIpDisassociate {
        id: String,
    },
    FloatingIpRelease {
        id: String,
    },
    PortForwardingCreate(PortForwardingSpec),
    PortForwardingUpdate {
        id: String,
        spec: PortForwardingSpec,
    },
    PortForwardingDelete {
        floating_ip_id: String,
        id: String,
    },
    // Load balancers
    /// `cascade` also deletes listeners, pools, members and monitors (Octavia
    /// refuses to delete a load balancer that still has them otherwise).
    LoadBalancerDelete {
        id: String,
        cascade: bool,
    },
    /// Boxed: by far the largest action.
    LoadBalancerCreate(Box<LbCreateSpec>),
    LoadBalancerUpdate {
        id: String,
        name: String,
        description: String,
        admin_up: bool,
    },
    ListenerCreate {
        loadbalancer_id: String,
        spec: LbListenerSpec,
        default_pool_id: Option<String>,
    },
    ListenerUpdate {
        id: String,
        name: String,
        description: String,
        connection_limit: Option<u32>,
        default_pool_id: Option<String>,
        admin_up: bool,
        /// The listener's protocol, to check the options (not sent).
        #[serde(default)]
        protocol: String,
        /// `None`: leave the options alone (the cloud's Octavia is too old for
        /// them, or the dialog didn't show them).
        #[serde(default)]
        options: Option<LbListenerOptions>,
        /// `TERMINATED_HTTPS` listeners: `(certificate, SNI certificates)`;
        /// `None` = unchanged.
        #[serde(default)]
        tls: Option<(String, Vec<String>)>,
    },
    ListenerDelete {
        id: String,
    },
    /// `listener_id`: also becomes that listener's default pool.
    PoolCreate {
        loadbalancer_id: String,
        listener_id: Option<String>,
        spec: LbPoolSpec,
    },
    PoolUpdate {
        id: String,
        name: String,
        description: String,
        algorithm: String,
        session_persistence: Option<String>,
        cookie_name: Option<String>,
        admin_up: bool,
        /// The pool's protocol, to check `tls_enabled` (not sent).
        #[serde(default)]
        protocol: String,
        /// `(tls_enabled, tls_ciphers)`; `None`: left alone (Octavia < 2.8).
        #[serde(default)]
        tls: Option<(bool, Option<String>)>,
    },
    PoolDelete {
        id: String,
    },
    MemberCreate {
        pool_id: String,
        spec: LbMemberSpec,
    },
    /// `None` monitor fields are sent as `null` (health-check the member itself).
    MemberUpdate {
        pool_id: String,
        id: String,
        name: String,
        weight: u16,
        backup: bool,
        admin_up: bool,
        #[serde(default)]
        monitor_address: Option<String>,
        #[serde(default)]
        monitor_port: Option<u16>,
    },
    /// Octavia's batch update (`PUT /lbaas/pools/{id}/members`): creates the new
    /// members, updates those matched by address and port, and **deletes the
    /// ones left out** (roadmap 54).
    PoolSetMembers {
        pool_id: String,
        members: Vec<LbMemberSpec>,
    },
    /// `POST /v3/users/{user_id}/application_credentials`; the secret comes back
    /// once (`ActionOutcome.secret`, with `openrc` and `clouds.yaml`).
    AppCredentialCreate(Box<AppCredentialSpec>),
    /// Identity › Projects (admins, roadmap 57): `POST /v3/projects` in
    /// `domain_id`, set by `run_action` (see [`OsAction::set_domain`]).
    ProjectCreate {
        name: String,
        description: Option<String>,
        enabled: bool,
        #[serde(default)]
        domain_id: String,
    },
    /// `PATCH /v3/projects/{id}` (Horizon's *Edit Project*, including enabled).
    ProjectUpdate {
        id: String,
        name: String,
        description: String,
        enabled: bool,
    },
    /// `DELETE /v3/projects/{id}`.
    ProjectDelete {
        id: String,
    },
    /// `PUT /v3/projects/{p}/users/{u}/roles/{r}` or `…/groups/{g}/…` (one of
    /// `user_id` / `group_id`): *Manage Members* / *Modify Groups*.
    ProjectRoleGrant {
        project_id: String,
        user_id: Option<String>,
        group_id: Option<String>,
        role_id: String,
    },
    /// The same with `DELETE`.
    ProjectRoleRevoke {
        project_id: String,
        user_id: Option<String>,
        group_id: Option<String>,
        role_id: String,
    },
    /// Identity › Users: `POST /v3/users`, then the role on the primary project.
    UserCreate(Box<UserSpec>),
    /// `PATCH /v3/users/{id}` (name, email, description, primary project).
    UserUpdate {
        id: String,
        name: String,
        email: String,
        description: String,
        project_id: Option<String>,
    },
    /// `PATCH /v3/users/{id}` with `enabled`.
    UserSetEnabled {
        id: String,
        enabled: bool,
    },
    /// `PATCH /v3/users/{id}` with `password` (an admin setting it; masked).
    UserSetPassword {
        id: String,
        password: String,
    },
    /// `DELETE /v3/users/{id}`.
    UserDelete {
        id: String,
    },
    /// Identity › Groups: `POST /v3/groups` in `domain_id` (set by `run_action`).
    GroupCreate {
        name: String,
        description: Option<String>,
        #[serde(default)]
        domain_id: String,
    },
    /// `PATCH /v3/groups/{id}`.
    GroupUpdate {
        id: String,
        name: String,
        description: String,
    },
    /// `DELETE /v3/groups/{id}`.
    GroupDelete {
        id: String,
    },
    /// `PUT /v3/groups/{g}/users/{u}`.
    GroupAddUser {
        group_id: String,
        user_id: String,
    },
    /// `DELETE /v3/groups/{g}/users/{u}`.
    GroupRemoveUser {
        group_id: String,
        user_id: String,
    },
    /// Identity › Roles (admins, roadmap 57): `POST /v3/roles`.
    RoleCreate {
        name: String,
    },
    /// `PATCH /v3/roles/{id}` (the name; Horizon's *Edit*).
    RoleUpdate {
        id: String,
        name: String,
    },
    /// `DELETE /v3/roles/{id}`: also removes every assignment of the role.
    RoleDelete {
        id: String,
    },
    /// `DELETE /v3/users/{user_id}/application_credentials/{id}`.
    AppCredentialDelete {
        #[serde(default)]
        user_id: String,
        id: String,
    },
    MemberDelete {
        pool_id: String,
        id: String,
    },
    MonitorCreate {
        pool_id: String,
        spec: LbMonitorSpec,
    },
    /// The type cannot change; every other field can.
    MonitorUpdate {
        id: String,
        spec: LbMonitorSpec,
        admin_up: bool,
    },
    MonitorDelete {
        id: String,
    },
    /// L7 policies and rules (roadmap 54).
    L7PolicyCreate {
        listener_id: String,
        spec: LbL7PolicySpec,
    },
    /// JSON: the unused redirect target is sent as `null`.
    L7PolicyUpdate {
        id: String,
        spec: LbL7PolicySpec,
    },
    L7PolicyDelete {
        id: String,
    },
    L7RuleCreate {
        policy_id: String,
        spec: LbL7RuleSpec,
    },
    /// JSON: a removed key is sent as `null`.
    L7RuleUpdate {
        policy_id: String,
        id: String,
        spec: LbL7RuleSpec,
    },
    L7RuleDelete {
        policy_id: String,
        id: String,
    },
    // Trunks
    TrunkCreate {
        name: String,
        description: Option<String>,
        port_id: String,
        admin_up: bool,
        subports: Vec<SubportSpec>,
    },
    TrunkUpdate {
        id: String,
        name: String,
        description: String,
        admin_up: bool,
    },
    TrunkDelete {
        id: String,
    },
    TrunkAddSubports {
        id: String,
        subports: Vec<SubportSpec>,
    },
    TrunkRemoveSubports {
        id: String,
        port_ids: Vec<String>,
    },
    // Firewall (FWaaS v2, roadmap 93)
    FirewallRuleCreate(FirewallRuleSpec),
    FirewallRuleUpdate {
        id: String,
        spec: FirewallRuleSpec,
    },
    FirewallRuleDelete {
        id: String,
    },
    /// `rule_ids` in evaluation order.
    FirewallPolicyCreate {
        name: String,
        description: String,
        rule_ids: Vec<String>,
        audited: bool,
    },
    FirewallPolicyUpdate {
        id: String,
        name: String,
        description: String,
        audited: bool,
    },
    FirewallPolicyDelete {
        id: String,
    },
    /// Inserts a rule before or after one already in the policy (neither: first).
    FirewallPolicyInsertRule {
        id: String,
        rule_id: String,
        before_id: Option<String>,
        after_id: Option<String>,
    },
    FirewallPolicyRemoveRule {
        id: String,
        rule_id: String,
    },
    /// `None` policies: no ingress / egress filtering.
    FirewallGroupCreate {
        name: String,
        description: String,
        ingress_policy_id: Option<String>,
        egress_policy_id: Option<String>,
        port_ids: Vec<String>,
        admin_up: bool,
    },
    /// `None` policies are cleared (`null`).
    FirewallGroupUpdate {
        id: String,
        name: String,
        description: String,
        ingress_policy_id: Option<String>,
        egress_policy_id: Option<String>,
        admin_up: bool,
    },
    /// *Add Port* / *Remove Port*: Neutron takes the whole new list.
    FirewallGroupSetPorts {
        id: String,
        port_ids: Vec<String>,
    },
    FirewallGroupDelete {
        id: String,
    },
    // Network QoS
    QosPolicyCreate {
        name: String,
        description: Option<String>,
        shared: bool,
    },
    QosPolicyUpdate {
        id: String,
        name: String,
        description: String,
        shared: bool,
    },
    QosPolicyDelete {
        id: String,
    },
    QosRuleCreate {
        policy_id: String,
        spec: QosRuleSpec,
    },
    QosRuleUpdate {
        policy_id: String,
        id: String,
        spec: QosRuleSpec,
    },
    QosRuleDelete {
        policy_id: String,
        kind: String,
        id: String,
    },
    // Key Manager (roadmap 55)
    SecretCreate(SecretSpec),
    SecretDelete {
        id: String,
    },
    /// `secrets`: (role, `secret_ref` URL) pairs; Barbican takes refs, not IDs.
    ContainerCreate {
        name: Option<String>,
        container_type: String,
        secrets: Vec<(String, String)>,
    },
    /// Deletes the container only; its secrets stay.
    ContainerDelete {
        id: String,
    },
    /// *Edit Access* (roadmap 75): `PUT v1/secrets/{id}/acl`, which replaces the
    /// whole `read` ACL. `users` are Keystone user IDs (checked like IDs).
    SecretAclSet {
        id: String,
        project_access: bool,
        users: Vec<String>,
    },
    /// *Reset Access*: `DELETE v1/secrets/{id}/acl`, back to Barbican's default
    /// (every project member, no extra users).
    SecretAclDelete {
        id: String,
    },
    /// *Edit Access* on a container: `PUT v1/containers/{id}/acl`. The secrets in
    /// it have their own ACLs (Octavia needs both).
    ContainerAclSet {
        id: String,
        project_access: bool,
        users: Vec<String>,
    },
    /// *Reset Access* on a container: `DELETE v1/containers/{id}/acl`.
    ContainerAclDelete {
        id: String,
    },
    // Orchestration (Heat, roadmap 62). Heat paths take the stack's name and
    // ID (`stacks/{name}/{id}`), so both travel; `name` is checked like an ID.
    /// `POST …/actions {"check": null}`: Heat checks each resource still exists.
    StackCheck {
        name: String,
        id: String,
    },
    /// `{"suspend": null}`: suspends the resources that can be (servers are paused…).
    StackSuspend {
        name: String,
        id: String,
    },
    /// `{"resume": null}`.
    StackResume {
        name: String,
        id: String,
    },
    /// Deletes the stack and every resource it created.
    StackDelete {
        name: String,
        id: String,
    },
    /// `POST stacks`.
    StackCreate(StackSpec),
    /// *Change Stack Template*: `PATCH stacks/{name}/{id}`, so parameters left
    /// out (hidden ones the form can't show) and the environment keep their values.
    StackUpdate {
        id: String,
        spec: StackSpec,
    },
    // Alarming (Aodh, roadmap 79)
    /// `POST v2/alarms`.
    AlarmCreate(AlarmSpec),
    /// `PUT v2/alarms/{id}`: Aodh replaces the alarm, so `spec` is all of it.
    AlarmUpdate {
        id: String,
        spec: AlarmSpec,
    },
    // Bulk
    /// Runs every action in order, continuing after a failure (Horizon's bulk
    /// *Delete …*). Each item carries its display name for the results list.
    /// Not nestable; at most [`MAX_BATCH`] items.
    Batch(Vec<(String, OsAction)>),
}

impl OsAction {
    /// Variant name for logs (no IDs, names or keys).
    /// The custom image property keys an action writes or removes (checked against
    /// `[images] reserved_properties` on the server, roadmap 25).
    pub fn image_property_keys(&self) -> Vec<&str> {
        match self {
            OsAction::ImageCreate(spec) => spec.properties.iter().map(|(k, _)| k.as_str()).collect(),
            OsAction::ImageSetProperties { set, remove, .. } => set
                .iter()
                .map(|(k, _)| k.as_str())
                .chain(remove.iter().map(String::as_str))
                .collect(),
            OsAction::Batch(items) => items.iter().flat_map(|(_, a)| a.image_property_keys()).collect(),
            _ => vec![],
        }
    }

    /// CIDRs of the subnets an action creates (checked against `[network]` on the server).
    pub fn new_subnet_cidrs(&self) -> Vec<&str> {
        match self {
            OsAction::NetworkCreate { subnet: Some(s), .. } => vec![s.cidr.as_str()],
            OsAction::AdminNetworkCreate(spec) => spec.subnet.iter().map(|s| s.cidr.as_str()).collect(),
            OsAction::SubnetCreate { spec, .. } => vec![spec.cidr.as_str()],
            OsAction::Batch(items) => items.iter().flat_map(|(_, a)| a.new_subnet_cidrs()).collect(),
            _ => vec![],
        }
    }

    /// Points identity actions at the signed-in user (roadmap 57): users manage
    /// their own application credentials, whatever the browser sent.
    pub fn set_user(&mut self, me: &str) {
        match self {
            OsAction::AppCredentialCreate(spec) => spec.user_id = me.to_string(),
            OsAction::AppCredentialDelete { user_id, .. } => *user_id = me.to_string(),
            OsAction::Batch(items) => items.iter_mut().for_each(|(_, a)| a.set_user(me)),
            _ => {}
        }
    }

    /// Whether the action creates something owned by a domain (a project, user
    /// or group), so `run_action` must say which.
    pub fn needs_domain(&self) -> bool {
        match self {
            OsAction::ProjectCreate { .. } | OsAction::UserCreate(_) | OsAction::GroupCreate { .. } => true,
            OsAction::Batch(items) => items.iter().any(|(_, a)| a.needs_domain()),
            _ => false,
        }
    }

    /// Puts projects, users and groups in `domain`: the session's domain context,
    /// else the user's own domain, like Horizon. Without it Keystone would use
    /// its default domain (`Default`) for a project-scoped token, not the user's
    /// (roadmap 57: a project created from *ttcp-spain* landed in `Default`).
    pub fn set_domain(&mut self, domain: &str) {
        match self {
            OsAction::ProjectCreate { domain_id, .. } | OsAction::GroupCreate { domain_id, .. } => {
                *domain_id = domain.to_string()
            }
            OsAction::UserCreate(spec) => spec.domain_id = domain.to_string(),
            OsAction::Batch(items) => items.iter_mut().for_each(|(_, a)| a.set_domain(domain)),
            _ => {}
        }
    }

    /// Horizon's identity safety rules (roadmap 57), checked by `run_action`
    /// with the signed-in user and project: no deleting or disabling oneself or
    /// the current project, and no removing one's own roles there (stricter than
    /// Horizon, which guards `admin` only: telling roles apart needs a lookup
    /// dry-run can't make, and this way nobody locks themselves out).
    pub fn identity_safety(&self, me: &str, project: &str) -> Result<(), String> {
        match self {
            OsAction::UserDelete { id } if id == me => Err("you cannot delete your own user".into()),
            OsAction::UserSetEnabled { id, enabled: false } if id == me => {
                Err("you cannot disable your own user".into())
            }
            OsAction::ProjectDelete { id } if id == project => {
                Err("you cannot delete the project you are signed in to".into())
            }
            OsAction::ProjectUpdate { id, enabled: false, .. } if id == project => {
                Err("you cannot disable the project you are signed in to".into())
            }
            OsAction::ProjectRoleRevoke {
                project_id,
                user_id: Some(u),
                ..
            } if project_id == project && u == me => {
                Err("you cannot remove your own roles on the project you are signed in to".into())
            }
            OsAction::Batch(items) => items.iter().try_for_each(|(_, a)| a.identity_safety(me, project)),
            _ => Ok(()),
        }
    }

    /// Every ID in the action is safe to paste into a URL path (roadmap 48): the
    /// strings under a key that is `id`, ends in `_id`/`_ids`, or is `kind` (QoS
    /// rule paths), and key pair names. Found by walking the serialised action,
    /// so new variants are covered without a list to keep. Empty strings pass:
    /// the action itself rejects a missing required ID.
    pub fn check_ids(&self) -> Result<(), AppError> {
        fn walk(v: &serde_json::Value, key: &str, keypair: bool) -> Result<(), AppError> {
            use serde_json::Value;
            let path_key = key == "id"
                || key.ends_with("_id")
                || key.ends_with("_ids")
                || key == "kind"
                || (keypair && key == "name");
            match v {
                Value::String(s) if path_key && !s.is_empty() => check_id(s),
                Value::Array(items) => items.iter().try_for_each(|x| walk(x, key, keypair)),
                Value::Object(m) => m.iter().try_for_each(|(k, x)| walk(x, k, keypair)),
                _ => Ok(()),
            }
        }
        // Nova puts key pair names in the path (`os-keypairs/{name}`), Heat stack
        // names (`stacks/{name}/{id}`).
        let keypair = matches!(
            self,
            OsAction::KeyPairImport { .. }
                | OsAction::KeyPairGenerate { .. }
                | OsAction::KeyPairDelete { .. }
                | OsAction::StackCheck { .. }
                | OsAction::StackSuspend { .. }
                | OsAction::StackResume { .. }
                | OsAction::StackDelete { .. }
                | OsAction::StackCreate(_)
                | OsAction::StackUpdate { .. }
        );
        let v = serde_json::to_value(self).map_err(|e| AppError::invalid(e.to_string()))?;
        walk(&v, "", keypair)
    }

    /// The plugin an action belongs to, if any (checked on the server).
    pub fn plugins(&self) -> Vec<Plugin> {
        let tls = |l: &LbListenerSpec| l.protocol == LB_TLS_PROTOCOL;
        match self {
            // Certificates come from Barbican (roadmap 54).
            OsAction::ListenerCreate { spec, .. } if tls(spec) => vec![Plugin::LoadBalancer, Plugin::KeyManager],
            OsAction::LoadBalancerCreate(s) if s.listener.as_ref().is_some_and(tls) => {
                vec![Plugin::LoadBalancer, Plugin::KeyManager]
            }
            OsAction::ListenerUpdate { tls: Some(_), .. } => vec![Plugin::LoadBalancer, Plugin::KeyManager],
            OsAction::LoadBalancerDelete { .. }
            | OsAction::LoadBalancerCreate(_)
            | OsAction::LoadBalancerUpdate { .. }
            | OsAction::ListenerCreate { .. }
            | OsAction::ListenerUpdate { .. }
            | OsAction::ListenerDelete { .. }
            | OsAction::PoolCreate { .. }
            | OsAction::PoolUpdate { .. }
            | OsAction::PoolDelete { .. }
            | OsAction::MemberCreate { .. }
            | OsAction::MemberUpdate { .. }
            | OsAction::PoolSetMembers { .. }
            | OsAction::MemberDelete { .. }
            | OsAction::MonitorCreate { .. }
            | OsAction::MonitorUpdate { .. }
            | OsAction::MonitorDelete { .. }
            | OsAction::L7PolicyCreate { .. }
            | OsAction::L7PolicyUpdate { .. }
            | OsAction::L7PolicyDelete { .. }
            | OsAction::L7RuleCreate { .. }
            | OsAction::L7RuleUpdate { .. }
            | OsAction::L7RuleDelete { .. } => vec![Plugin::LoadBalancer],
            OsAction::QosPolicyCreate { .. }
            | OsAction::QosPolicyUpdate { .. }
            | OsAction::QosPolicyDelete { .. }
            | OsAction::QosRuleCreate { .. }
            | OsAction::QosRuleUpdate { .. }
            | OsAction::QosRuleDelete { .. } => vec![Plugin::Qos],
            OsAction::TrunkCreate { .. }
            | OsAction::TrunkUpdate { .. }
            | OsAction::TrunkDelete { .. }
            | OsAction::TrunkAddSubports { .. }
            | OsAction::TrunkRemoveSubports { .. } => vec![Plugin::Trunk],
            OsAction::FirewallRuleCreate(_)
            | OsAction::FirewallRuleUpdate { .. }
            | OsAction::FirewallRuleDelete { .. }
            | OsAction::FirewallPolicyCreate { .. }
            | OsAction::FirewallPolicyUpdate { .. }
            | OsAction::FirewallPolicyDelete { .. }
            | OsAction::FirewallPolicyInsertRule { .. }
            | OsAction::FirewallPolicyRemoveRule { .. }
            | OsAction::FirewallGroupCreate { .. }
            | OsAction::FirewallGroupUpdate { .. }
            | OsAction::FirewallGroupSetPorts { .. }
            | OsAction::FirewallGroupDelete { .. } => vec![Plugin::Firewall],
            OsAction::SecretCreate(_)
            | OsAction::SecretDelete { .. }
            | OsAction::SecretAclSet { .. }
            | OsAction::SecretAclDelete { .. }
            | OsAction::ContainerAclSet { .. }
            | OsAction::ContainerAclDelete { .. }
            | OsAction::ContainerCreate { .. }
            | OsAction::ContainerDelete { .. } => {
                vec![Plugin::KeyManager]
            }
            OsAction::StackCheck { .. }
            | OsAction::StackSuspend { .. }
            | OsAction::StackResume { .. }
            | OsAction::StackDelete { .. }
            | OsAction::StackCreate(_)
            | OsAction::StackUpdate { .. } => vec![Plugin::Orchestration],
            OsAction::AlarmCreate(_) | OsAction::AlarmUpdate { .. } => vec![Plugin::Alarming],
            OsAction::BackupCreate { .. }
            | OsAction::BackupUpdate { .. }
            | OsAction::BackupRestore { .. }
            | OsAction::BackupDelete { .. } => vec![Plugin::Backup],
            OsAction::QuotaUpdate {
                service: QuotaService::LoadBalancer,
                ..
            } => vec![Plugin::LoadBalancer],
            OsAction::QuotaUpdate {
                service: QuotaService::KeyManager,
                ..
            } => vec![Plugin::KeyManager],
            OsAction::RouterCreate {
                qos_policy_id: Some(_), ..
            }
            | OsAction::FloatingIpAllocate {
                qos_policy_id: Some(_), ..
            } => vec![Plugin::Qos],
            OsAction::Batch(items) => items.iter().flat_map(|(_, a)| a.plugins()).collect(),
            _ => vec![],
        }
    }

    /// Whether it changes the quotas of a project other than `session_project`
    /// (Identity › *Modify Quotas*, roadmap 82): `run_action` then requires the
    /// `admin` role. The session's own project stays editable by whoever the
    /// cloud's policy allows, as Project › Quotas.
    pub fn other_projects_quotas(&self, session_project: &str) -> bool {
        match self {
            OsAction::QuotaUpdate { project_id, .. } => project_id != session_project,
            OsAction::Batch(items) => items.iter().any(|(_, a)| a.other_projects_quotas(session_project)),
            _ => false,
        }
    }

    /// Writes of the Admin dashboard (roadmap 85): `run_action` refuses them
    /// without the `admin` role on the token, before any request.
    pub fn admin_only(&self) -> bool {
        match self {
            OsAction::AdminNetworkCreate(_)
            | OsAction::AdminNetworkUpdate { .. }
            | OsAction::RbacPolicyCreate { .. }
            | OsAction::RbacPolicyUpdate { .. }
            | OsAction::RbacPolicyDelete { .. }
            | OsAction::AdminFloatingIpAllocate { .. }
            | OsAction::AdminRouterCreate(_)
            | OsAction::FlavorCreate(_)
            | OsAction::FlavorReplace { .. }
            | OsAction::FlavorDelete { .. }
            | OsAction::FlavorSetExtraSpecs { .. }
            | OsAction::FlavorSetAccess { .. }
            | OsAction::AggregateCreate { .. }
            | OsAction::AggregateUpdate { .. }
            | OsAction::AggregateSetHosts { .. }
            | OsAction::AggregateSetMetadata { .. }
            | OsAction::AggregateDelete { .. }
            | OsAction::ComputeServiceSet { .. }
            | OsAction::ServerMigrate { .. }
            | OsAction::ServerLiveMigrate { .. }
            | OsAction::VolumeResetStatus { .. }
            | OsAction::VolumeMigrate { .. }
            | OsAction::VolumeManage(_)
            | OsAction::VolumeUnmanage { .. }
            | OsAction::SnapshotResetStatus { .. }
            | OsAction::VolumeTypeCreate { .. }
            | OsAction::VolumeTypeUpdate { .. }
            | OsAction::VolumeTypeDelete { .. }
            | OsAction::VolumeTypeSetExtraSpecs { .. }
            | OsAction::VolumeTypeSetQos { .. }
            | OsAction::VolumeTypeEncryptionSet { .. }
            | OsAction::VolumeTypeEncryptionDelete { .. }
            | OsAction::QosSpecCreate { .. }
            | OsAction::QosSpecSetConsumer { .. }
            | OsAction::QosSpecSetSpecs { .. }
            | OsAction::QosSpecDelete { .. }
            | OsAction::GroupTypeCreate { .. }
            | OsAction::GroupTypeUpdate { .. }
            | OsAction::GroupTypeDelete { .. }
            | OsAction::GroupTypeSetSpecs { .. }
            | OsAction::DefaultQuotaUpdate { .. }
            | OsAction::MetadefNamespaceImport { .. }
            | OsAction::MetadefNamespaceUpdate { .. }
            | OsAction::MetadefNamespaceDelete { .. }
            | OsAction::MetadefNamespaceSetTypes { .. } => true,
            OsAction::Batch(items) => items.iter().any(|(_, a)| a.admin_only()),
            _ => false,
        }
    }

    /// The variant's name (`ServerDelete`), for logs: never the payload.
    pub fn kind(&self) -> String {
        let dbg = format!("{self:?}");
        dbg.split(['(', ' ', '{']).next().unwrap_or("").to_string()
    }
}

// ---------------------------------------------------------------------------
// Form choices
// ---------------------------------------------------------------------------

/// Option lists a dialog can load (`get_choices`); each is one or a few GETs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChoiceSet {
    Flavors,
    Images,
    /// Networks a port/instance can use (project + shared)
    Networks,
    ExternalNetworks,
    Subnets,
    KeyPairs,
    /// By name (Nova launch takes names)
    SecurityGroups,
    /// By ID (rules reference groups by ID)
    SecurityGroupIds,
    ServerGroups,
    ComputeZones,
    VolumeZones,
    /// Neutron's network availability zones (`resource=network`), by name
    NetworkZones,
    /// Neutron's router availability zones (`resource=router`), by name
    RouterZones,
    /// Enabled, up `nova-compute` hosts (an admin's migration targets)
    ComputeHosts,
    /// Every `nova-compute` host, whatever its state (aggregate members)
    AllComputeHosts,
    /// *Create RBAC Policy*'s *Target Project*: `*` (every project), then the projects
    RbacTargets,
    /// Every project of every domain, by name, for an admin (the Admin
    /// dashboard's *Project* fields, as Horizon lists them)
    AllProjects,
    /// Every project's networks, for an admin (*Create RBAC Policy*)
    AllNetworks,
    /// Every project's QoS policies, for an admin (*Create RBAC Policy*)
    AllQosPolicies,
    /// Cinder's back-end pools (`host@backend#pool`), an admin's volume
    /// migration targets (`scheduler-stats/get_pools`)
    CinderPools,
    /// Cinder's QoS specs (value ID, label name), for a volume type's association
    QosSpecs,
    /// Glance's metadata definition resource types (`OS::Nova::Flavor`…), by name
    MetadefResourceTypes,
    /// Every project's instance ports without a floating IP, for an admin's
    /// *Associate*. The first cell is [`associable_port_key`]: a port reached
    /// from several pools comes once per pool, and the dialog keeps the
    /// floating IP owner's ports its pool reaches with `Source::Of`
    AllAssociablePorts,
    VolumeTypes,
    AvailableVolumes,
    VolumeSnapshots,
    Servers,
    /// Instance ports without a floating IP, for association
    AssociablePorts,
    /// Fixed IPv4 addresses of instance ports, for port forwarding: value `port_id/ip`
    ForwardablePorts,
    /// QoS policies the project can use (its own and shared ones)
    QosPolicies,
    /// Load balancer members: each fixed IP of an instance port, value `ip@subnet_id`;
    /// cells `[instance, ip, subnet name]`
    LbMemberTargets,
    /// The scoped token's roles, for an application credential's roles (roadmap 57)
    TokenRoles,
    /// The domain new projects, users and groups go to (one choice: value ID,
    /// label name), shown read-only in their create dialogs like Horizon
    OperationDomain,
    /// Identity admin pickers (roadmap 57): every role (value ID, label name)
    IdentityRoles,
    /// Users visible to an admin (value ID, label name)
    IdentityUsers,
    /// Groups visible to an admin
    IdentityGroups,
    /// Projects visible to an admin (a user's primary project)
    IdentityProjects,
    /// Subnets an external member can be on, by name: those of the project's own,
    /// shared and external networks, as Horizon's members step lists them
    LbMemberSubnets,
    /// Octavia flavors that are enabled
    LbFlavors,
    /// Subnets a load balancer's VIP can be on: the project's own and shared
    /// networks' (roadmap 54, like Horizon)
    LbVipSubnets,
    /// Octavia availability zones that are enabled
    LbAvailabilityZones,
    /// Certificates a `TERMINATED_HTTPS` listener can use (Barbican URLs, roadmap 54);
    /// first cell = expiry, `""` when it never expires
    LbCertificates,
    /// Every instance port; first cell = instance ID, second = its security group IDs
    InstancePorts,
    /// Floating IPs not associated with a port
    FreeFloatingIps,
    /// Launch wizard: bootable images (not instance snapshots), like Horizon's *Image*
    LaunchImages,
    /// Launch wizard: images with `image_type=snapshot`
    InstanceSnapshots,
    /// Launch wizard: project ports not attached to anything
    FreePorts,
    /// Barbican secrets for a container (roadmap 55): value `secret_ref`, label
    /// the name; cells `[type]`
    Secrets,
    /// Cinder group types (value ID, label name), for *Create Group*
    GroupTypes,
    /// Volume types by ID (`VolumeTypes` gives names), for *Create Group*
    VolumeTypeIds,
    /// *Manage Volumes*: for each volume group, its volumes plus those it can
    /// take (no group, one of its volume types, `available` or `in-use`); a
    /// volume appears once per group, first cell = the group's ID (`Source::Of`)
    GroupVolumes,
    /// Firewall rules the project can use: its own and shared ones (roadmap 93);
    /// cells `[action, protocol]`
    FirewallRules,
    /// Firewall policies the project can use: its own and shared ones
    FirewallPolicies,
    /// Ports a firewall group can take: the project's router interfaces and
    /// instance ports no group has (Neutron refuses those); cells
    /// `[device, kind, IPs, port]`
    FirewallPorts,
}

/// One option of a select or pick table. `cells` feed the wizard tables (one
/// per column of `ChoiceSet`'s table, raw: booleans as `true`/`false`, the UI
/// localises them); `facts` feed Horizon's sizing rules.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub cells: Vec<String>,
    #[serde(default)]
    pub facts: Facts,
}

impl Choice {
    /// A choice with only a value and a label (no table cells).
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Choice {
            value: value.into(),
            label: label.into(),
            ..Default::default()
        }
    }
}

/// The `Source::Of` key of a [`ChoiceSet::AllAssociablePorts`] choice: the
/// port's project and a floating IP pool (external network ID) that reaches it.
/// Neutron associates only within one project and through a router with its
/// gateway on the pool (roadmap 89).
pub fn associable_port_key(project_id: &str, pool_id: &str) -> String {
    format!("{project_id} {pool_id}")
}

/// Numbers the launch wizard needs about images and flavors.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Facts {
    pub size_bytes: Option<u64>,
    pub min_disk_gb: Option<i64>,
    pub min_ram_mb: Option<i64>,
    pub vcpus: Option<i64>,
    pub ram_mb: Option<i64>,
    pub disk_gb: Option<i64>,
}

/// Horizon's minimum boot-volume size for an image: the larger of its size in
/// (decimal) GB and its `min_disk`, rounded up, at least 1.
pub fn min_volume_gb(image: &Facts) -> u32 {
    let size_gb = image.size_bytes.unwrap_or(0) as f64 / 1e9;
    let min_disk = image.min_disk_gb.unwrap_or(0).max(0) as f64;
    (size_gb.max(min_disk).ceil() as u32).max(1)
}

/// Why a flavor cannot boot the selected source (Horizon disables it).
#[derive(Clone, Debug, PartialEq)]
pub enum FlavorIssue {
    /// The image needs at least this much RAM (MB).
    Ram(i64),
    /// Booting from the image without a volume needs this much root disk (GB).
    Disk(i64),
}

/// `boot_from_volume`: the root disk is a volume, so the flavor's disk does not
/// matter (Nova only checks it for local disks). Flavor disk 0 = image size.
pub fn flavor_issue(flavor: &Facts, image: Option<&Facts>, boot_from_volume: bool) -> Option<FlavorIssue> {
    let image = image?;
    let ram = flavor.ram_mb.unwrap_or(0);
    if let Some(need) = image.min_ram_mb.filter(|m| *m > ram) {
        return Some(FlavorIssue::Ram(need));
    }
    let disk = flavor.disk_gb.unwrap_or(0);
    if !boot_from_volume && disk > 0 {
        let need = min_volume_gb(image) as i64;
        if need > disk {
            return Some(FlavorIssue::Disk(need));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Network topology
// ---------------------------------------------------------------------------

/// What a topology node other than a network is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TopoKind {
    Router,
    Server,
}

/// A network in the Network Topology, with its subnets' CIDRs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TopoNetwork {
    pub id: String,
    pub name: String,
    pub external: bool,
    pub shared: bool,
    /// The session's project owns it (a project's own network may be `shared` too);
    /// the topology marks the others (roadmap 74).
    #[serde(default)]
    pub owned: bool,
    pub status: String,
    pub cidrs: Vec<String>,
}

/// A router or an instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TopoDevice {
    pub id: String,
    pub name: String,
    pub kind: TopoKind,
    pub status: String,
}

/// A port joining a device to a network.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TopoLink {
    pub device_id: String,
    pub network_id: String,
    pub port_id: Option<String>,
    pub ips: Vec<String>,
    /// Floating IPs associated with the port.
    pub floating: Vec<String>,
    /// Router gateway on an external network.
    pub gateway: bool,
}

/// Networks ordered external first, then by name; devices routers first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Topology {
    pub networks: Vec<TopoNetwork>,
    pub devices: Vec<TopoDevice>,
    pub links: Vec<TopoLink>,
    /// `[network] topology_max_networks` (roadmap 100), sent with the data so
    /// the server render and the hydrated page agree; `0`: no limit.
    #[serde(default)]
    pub max_networks: usize,
}

impl Topology {
    /// The network count when it is over [`Topology::max_networks`]: the page
    /// then shows a note instead of the drawings. At the limit it still draws.
    pub fn too_many(&self) -> Option<usize> {
        let n = self.networks.len();
        (self.max_networks > 0 && n > self.max_networks).then_some(n)
    }

    /// The links that touch `id`, a device or a network.
    pub fn links_of<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a TopoLink> + 'a {
        self.links
            .iter()
            .filter(move |l| l.device_id == id || l.network_id == id)
    }
}

// ---------------------------------------------------------------------------
// Detail views
// ---------------------------------------------------------------------------

/// A subnet: row of the network detail's *Subnets* tab, and the subnet detail page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubnetRow {
    pub id: String,
    pub name: String,
    pub network_id: String,
    pub network_name: Option<String>,
    pub cidr: String,
    pub ip_version: i64,
    pub gateway_ip: Option<String>,
    pub enable_dhcp: bool,
    pub allocation_pools: Vec<String>,
    pub dns_nameservers: Vec<String>,
    pub host_routes: Vec<String>,
    pub description: Option<String>,
}

/// A port: row of the network detail's *Ports* tab, and the port detail page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortRow {
    pub id: String,
    pub name: String,
    pub network_id: String,
    pub network_name: Option<String>,
    /// "ip (subnet name)"
    pub fixed_ips: Vec<String>,
    pub subnet_ids: Vec<String>,
    pub mac: String,
    pub status: String,
    pub admin_up: bool,
    pub device_owner: Option<String>,
    pub device_id: Option<String>,
    /// Instance name when the device is a Nova server
    pub device_name: Option<String>,
    pub security_groups: Vec<String>,
    pub vnic_type: Option<String>,
    pub description: Option<String>,
    /// `port_security_enabled`; `true` when missing (Neutron's default).
    pub port_security: bool,
    /// The IDs behind `security_groups` (which holds names).
    pub security_group_ids: Vec<String>,
}

impl PortRow {
    /// What *Edit Port* starts from.
    pub fn settings(&self) -> PortSettings {
        PortSettings {
            name: self.name.clone(),
            admin_up: self.admin_up,
            vnic_type: self.vnic_type.clone(),
            port_security: self.port_security,
            security_group_ids: self.security_group_ids.clone(),
        }
    }
}

/// The network detail page: the network, its subnets and its ports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetworkDetail {
    pub row: NetworkRow,
    pub description: Option<String>,
    pub project_id: String,
    pub mtu: Option<i64>,
    pub port_security: bool,
    pub created: Option<String>,
    pub subnets: Vec<SubnetRow>,
    pub ports: Vec<PortRow>,
}

/// The router detail page: gateway, interfaces and static routes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouterDetail {
    pub row: RouterRow,
    pub description: Option<String>,
    pub project_id: String,
    /// `(ip, subnet_id)` of each external fixed IP.
    pub gateway_ips: Vec<(String, String)>,
    pub interfaces: Vec<PortRow>,
    pub routes: Vec<(String, String)>,
}

/// A security group rule as shown (remote group resolved to its name).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuleRow {
    pub id: String,
    pub direction: String,
    pub ethertype: String,
    pub protocol: String,
    pub port_range: String,
    pub remote: String,
    pub description: Option<String>,
}

/// The security group detail page: the group and its rules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecurityGroupDetail {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub project_id: String,
    pub rules: Vec<RuleRow>,
}

/// The volume detail page: the volume, its snapshots and Cinder's messages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeDetail {
    pub volume: Volume,
    pub created: String,
    pub source_image: Option<String>,
    pub source_snapshot_id: Option<String>,
    pub multiattach: bool,
    pub metadata: Vec<(String, String)>,
    pub image_metadata: Vec<(String, String)>,
    pub snapshots: Vec<Snapshot>,
    /// Cinder's user messages about this volume (why it ended in `error`…), newest first.
    #[serde(default)]
    pub messages: Vec<VolumeMessage>,
}

/// A row of Volumes › Groups: a Cinder generic volume group (roadmap 5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeGroupRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub availability_zone: Option<String>,
    /// The group type; its name falls back to the ID when group types can't be listed.
    pub group_type: Ref,
    /// Volume types the group accepts, names resolved like `group_type`.
    pub volume_types: Vec<Ref>,
    /// Whether a group snapshot was taken of it (Horizon's *Has Snapshots*).
    pub has_snapshots: bool,
    pub created: String,
}

/// The volume group detail page: the group, its volumes and its snapshots.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeGroupDetail {
    pub row: VolumeGroupRow,
    pub volumes: Vec<Volume>,
    pub snapshots: Vec<GroupSnapshotRow>,
}

/// A row of Volumes › Group Snapshots (roadmap 5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupSnapshotRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    /// The group it was taken of; the name falls back to the ID (group deleted
    /// or unlisted).
    pub group: Ref,
    pub group_type: Ref,
    pub created: String,
}

/// The group snapshot detail page: the snapshot and the volume snapshots in it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupSnapshotDetail {
    pub row: GroupSnapshotRow,
    pub snapshots: Vec<Snapshot>,
}

/// One Cinder user message (`GET /messages`, volume API >= 3.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeMessage {
    pub id: String,
    pub level: String,
    pub message: String,
    /// RFC 3339
    pub created: String,
    pub request_id: Option<String>,
}

/// The image detail page, with the owner's project name and custom properties.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageDetail {
    pub image: Image,
    pub owner: Option<String>,
    pub checksum: Option<String>,
    pub container_format: Option<String>,
    #[serde(default)]
    pub virtual_size: Option<u64>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub properties: Vec<(String, String)>,
}

/// The key pair detail page (public key, type, fingerprint).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyPairDetail {
    pub name: String,
    pub key_type: Option<String>,
    pub fingerprint: String,
    pub public_key: String,
    pub created: Option<String>,
}

/// Who and where the session is, for the API Access page and its `openrc` /
/// `clouds.yaml` downloads. Never holds a secret: the password is asked for by
/// the shell script and left out of `clouds.yaml`, like Horizon does.
#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Credentials {
    pub user_name: String,
    pub user_id: String,
    pub user_domain: String,
    pub project_name: String,
    pub project_id: String,
    pub region: String,
    /// Keystone v3 URL (ends in `/v3`).
    pub auth_url: String,
    /// `[keystone] interface` (`public` by default).
    #[serde(default = "public")]
    pub interface: String,
    /// `[api_access] clouds_yaml_name` (`openstack` by default) and `clouds_yaml_profile`.
    #[serde(default = "openstack")]
    pub cloud_name: String,
    #[serde(default)]
    pub profile: Option<String>,
    /// The downloads, rendered on the server from `[api_access]` templates
    /// (`server::downloads`, roadmap 57).
    #[serde(default)]
    pub openrc: String,
    #[serde(default)]
    pub clouds_yaml: String,
    /// Signed in through WebSSO (roadmap 80): the user has no Keystone
    /// password, so `clouds.yaml` uses `v3websso` and the page says `openrc`
    /// won't work.
    #[serde(default)]
    pub federated: bool,
    /// That sign-in's identity provider (unknown without `[websso]
    /// idp_mapping`) and protocol, for the `v3websso` `clouds.yaml`.
    #[serde(default)]
    pub identity_provider: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
}

fn public() -> String {
    "public".into()
}
fn openstack() -> String {
    "openstack".into()
}

impl Credentials {
    /// Base file name for downloads, e.g. `demo-openrc.sh`.
    pub fn file_stem(&self) -> String {
        let stem: String = self
            .project_name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if stem.is_empty() { "openstack".into() } else { stem }
    }
}

#[cfg(test)]
mod topology_tests {
    use super::*;

    fn with(n: usize, max: usize) -> Topology {
        let net = |i: usize| TopoNetwork {
            id: i.to_string(),
            name: String::new(),
            external: false,
            shared: false,
            owned: true,
            status: "ACTIVE".into(),
            cidrs: vec![],
        };
        Topology {
            networks: (0..n).map(net).collect(),
            max_networks: max,
            ..Default::default()
        }
    }

    #[test]
    fn too_many_only_above_the_limit() {
        assert_eq!(with(50, 50).too_many(), None, "at the limit it still draws");
        assert_eq!(with(51, 50).too_many(), Some(51));
        assert_eq!(with(500, 0).too_many(), None, "0 = no limit");
        assert_eq!(with(0, 1).too_many(), None);
    }
}

#[cfg(test)]
mod credentials_tests {
    use super::*;

    fn sample() -> Credentials {
        Credentials {
            user_name: "ana".into(),
            user_id: "u1".into(),
            user_domain: "Default".into(),
            project_name: "it's \"demo\"".into(),
            project_id: "p1".into(),
            region: "es-central-1".into(),
            auth_url: "https://keystone.example/v3".into(),
            interface: "public".into(),
            cloud_name: "openstack".into(),
            profile: None,
            ..Default::default()
        }
    }

    #[test]
    fn file_stem_is_safe() {
        assert_eq!(sample().file_stem(), "it_s__demo_");
        assert_eq!(
            Credentials {
                project_name: String::new(),
                ..sample()
            }
            .file_stem(),
            "openstack"
        );
    }
}

/// One server's usage in the period (Nova `os-simple-tenant-usage`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerUsage {
    pub id: String,
    pub name: String,
    pub vcpus: i64,
    pub memory_mb: i64,
    pub local_gb: i64,
    /// Hours the server existed inside the period.
    pub hours: f64,
    /// Seconds since it started (or its lifetime, when terminated).
    pub uptime: i64,
    pub state: String,
}

impl ServerUsage {
    /// Still exists (Nova reports `terminated` for deleted ones in the period).
    pub fn active(&self) -> bool {
        !self.state.eq_ignore_ascii_case("terminated")
    }
}

/// Overview *Usage Summary* for a period (`start`/`end` are inclusive dates).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UsageSummary {
    pub start: String,
    pub end: String,
    pub servers: Vec<ServerUsage>,
}

impl UsageSummary {
    /// The table rows: like Horizon, only servers that still exist (the
    /// hour totals still count the ended ones).
    pub fn listed(&self) -> Vec<ServerUsage> {
        self.servers.iter().filter(|s| s.active()).cloned().collect()
    }
    /// Servers that still exist (Horizon's *Active Instances*).
    pub fn active_instances(&self) -> usize {
        self.servers.iter().filter(|s| s.active()).count()
    }
    /// RAM of the servers that still exist (Horizon's *Active RAM*).
    pub fn active_ram_mb(&self) -> i64 {
        self.servers.iter().filter(|s| s.active()).map(|s| s.memory_mb).sum()
    }
    /// Summed per server (Nova's own totals cover only one page when paginated).
    pub fn vcpu_hours(&self) -> f64 {
        self.servers.iter().map(|s| s.vcpus as f64 * s.hours).sum()
    }
    /// Disk GB-hours over the period, ended servers included.
    pub fn gb_hours(&self) -> f64 {
        self.servers.iter().map(|s| s.local_gb as f64 * s.hours).sum()
    }
    /// RAM MB-hours over the period, ended servers included.
    pub fn ram_mb_hours(&self) -> f64 {
        self.servers.iter().map(|s| s.memory_mb as f64 * s.hours).sum()
    }

    /// CSV like Horizon's *Download CSV Summary*.
    pub fn csv(&self) -> String {
        let n = |v: f64| fixed(v, 2);
        fn field(v: &str) -> String {
            if v.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", v.replace('"', "\"\""))
            } else {
                v.to_string()
            }
        }
        let mut out = format!("Usage Report For Period:,{},{}\n", self.start, self.end);
        out += &format!("Active Instances:,{}\n", self.active_instances());
        out += &format!("This Period's VCPU-Hours:,{}\n", n(self.vcpu_hours()));
        out += &format!("This Period's GB-Hours:,{}\n", n(self.gb_hours()));
        out += &format!("This Period's RAM-Hours:,{}\n\n", n(self.ram_mb_hours()));
        out += "Instance Name,VCPUs,RAM (MB),Disk (GB),Usage (Hours),Age (Seconds),State\n";
        for s in self.servers.iter().filter(|s| s.active()) {
            out += &format!(
                "{},{},{},{},{},{},{}\n",
                field(&s.name),
                s.vcpus,
                s.memory_mb,
                s.local_gb,
                n(s.hours),
                s.uptime,
                field(&s.state)
            );
        }
        out
    }
}

/// One project's line on Admin › Overview (roadmap 85): its servers in the
/// period, summed like Horizon's `NovaUsage` (sizes of the servers that still
/// exist, hours of all of them).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectUsage {
    pub project_id: String,
    /// The project's name, else its ID (a deleted project keeps its usage).
    pub project: String,
    pub servers: Vec<ServerUsage>,
    /// Keystone named other projects but not this one: Horizon's "(Deleted)"
    /// after the ID (roadmap 89). `false` when no names could be read.
    #[serde(default)]
    pub deleted: bool,
}

impl ProjectUsage {
    fn summary(&self) -> UsageSummary {
        UsageSummary {
            start: String::new(),
            end: String::new(),
            servers: self.servers.clone(),
        }
    }
    /// VCPUs of the servers that still exist.
    pub fn vcpus(&self) -> i64 {
        self.servers.iter().filter(|s| s.active()).map(|s| s.vcpus).sum()
    }
    /// Root disk GB of the servers that still exist.
    pub fn local_gb(&self) -> i64 {
        self.servers.iter().filter(|s| s.active()).map(|s| s.local_gb).sum()
    }
    /// RAM MB of the servers that still exist.
    pub fn memory_mb(&self) -> i64 {
        self.summary().active_ram_mb()
    }
    pub fn vcpu_hours(&self) -> f64 {
        self.summary().vcpu_hours()
    }
    pub fn gb_hours(&self) -> f64 {
        self.summary().gb_hours()
    }
    pub fn ram_mb_hours(&self) -> f64 {
        self.summary().ram_mb_hours()
    }
}

/// Admin › Overview: every project's usage over `[start, end]` (dates).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlobalUsage {
    pub start: String,
    pub end: String,
    /// Projects with servers in the period, by name.
    pub projects: Vec<ProjectUsage>,
}

impl GlobalUsage {
    /// Every server of every project, for the totals above the table.
    pub fn summary(&self) -> UsageSummary {
        UsageSummary {
            start: self.start.clone(),
            end: self.end.clone(),
            servers: self.projects.iter().flat_map(|p| p.servers.clone()).collect(),
        }
    }

    /// CSV like Horizon's Admin *Download CSV Summary* (`GlobalUsageCsvRenderer`):
    /// the totals, then one line per project; *Usage (Hours)* is VCPU-hours.
    pub fn csv(&self) -> String {
        let n = |v: f64| fixed(v, 2);
        let field = |v: &str| {
            if v.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", v.replace('"', "\"\""))
            } else {
                v.to_string()
            }
        };
        let all = self.summary();
        let mut out = format!("Usage Report For Period:,{},{}\n", self.start, self.end);
        out += &format!("Active Instances:,{}\n", all.active_instances());
        out += &format!("This Period's VCPU-Hours:,{}\n", n(all.vcpu_hours()));
        out += &format!("This Period's GB-Hours:,{}\n", n(all.gb_hours()));
        out += &format!("This Period's RAM-Hours:,{}\n\n", n(all.ram_mb_hours()));
        out += "Project Name,VCPUs,RAM (MB),Disk (GB),Usage (Hours)\n";
        for p in &self.projects {
            let name = if p.deleted {
                format!("{} (Deleted)", p.project)
            } else {
                p.project.clone()
            };
            out += &format!(
                "{},{},{},{},{}\n",
                field(&name),
                p.vcpus(),
                p.memory_mb(),
                p.local_gb(),
                n(p.vcpu_hours())
            );
        }
        out
    }
}

/// `v` with `places` decimals, never `-0.00`: summing an empty `f64` iterator
/// gives `-0.0`, and a tiny negative rounds to `-0.00` (roadmap 60: *Usage
/// Summary* with no instances in the period). Real negatives keep their sign.
pub fn fixed(v: f64, places: usize) -> String {
    let s = format!("{v:.places$}");
    match s.strip_prefix('-') {
        Some(rest) if rest.chars().all(|c| c == '0' || c == '.') => rest.to_string(),
        _ => s,
    }
}

/// Validates the usage period. Empty values default to the current month up to
/// `today`; the range is inclusive and at most a year.
pub fn usage_range(
    start: &str,
    end: &str,
    today: chrono::NaiveDate,
) -> Result<(chrono::NaiveDate, chrono::NaiveDate), String> {
    use chrono::{Datelike, NaiveDate};
    let parse = |v: &str, default: NaiveDate| -> Result<NaiveDate, String> {
        if v.trim().is_empty() {
            Ok(default)
        } else {
            NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d").map_err(|_| format!("invalid date: {v}"))
        }
    };
    let first = today.with_day(1).unwrap_or(today);
    let (from, to) = (parse(start, first)?, parse(end, today)?);
    if from > to {
        return Err("the start date is after the end date".into());
    }
    if (to - from).num_days() > 366 {
        return Err("the period can be at most one year".into());
    }
    Ok((from, to))
}

#[cfg(test)]
mod usage_tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn u(name: &str, vcpus: i64, hours: f64, state: &str) -> ServerUsage {
        ServerUsage {
            id: name.into(),
            name: name.into(),
            vcpus,
            memory_mb: 2048,
            local_gb: 10,
            hours,
            uptime: 3600,
            state: state.into(),
        }
    }

    #[test]
    fn range_defaults_to_this_month_and_validates() {
        let today = d("2026-10-24");
        assert_eq!(usage_range("", "", today), Ok((d("2026-10-01"), today)));
        assert_eq!(
            usage_range("2026-09-01", "2026-09-30", today),
            Ok((d("2026-09-01"), d("2026-09-30")))
        );
        assert!(usage_range("2026-10-05", "2026-10-01", today).is_err());
        assert!(usage_range("2024-01-01", "2026-01-01", today).is_err());
        assert!(usage_range("yesterday", "", today).is_err());
    }

    #[test]
    fn totals_count_only_active_servers_where_horizon_does() {
        let s = UsageSummary {
            start: "a".into(),
            end: "b".into(),
            servers: vec![u("x", 2, 10.0, "active"), u("y", 4, 1.5, "terminated")],
        };
        assert_eq!(s.active_instances(), 1);
        assert_eq!(s.active_ram_mb(), 2048);
        assert!((s.vcpu_hours() - 26.0).abs() < 1e-9);
        assert!((s.gb_hours() - 115.0).abs() < 1e-9);
        assert_eq!(
            s.listed().iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["x"],
            "ended servers are not listed"
        );
        assert!(!s.csv().contains("\ny,"), "nor in the CSV");
    }

    #[test]
    fn csv_quotes_names() {
        let s = UsageSummary {
            start: "2026-10-01".into(),
            end: "2026-10-24".into(),
            servers: vec![u("web, \"1\"", 1, 2.0, "active")],
        };
        let csv = s.csv();
        assert!(csv.starts_with("Usage Report For Period:,2026-10-01,2026-10-24\n"));
        assert!(csv.contains("\"web, \"\"1\"\"\",1,2048,10,2.00,3600,active\n"));
    }

    #[test]
    fn providers_form_a_tree_with_gpus_under_their_host() {
        let res = |used, total| {
            Some(ProviderResource {
                used,
                reserved: 0,
                total,
                allocation_ratio: 1.0,
            })
        };
        let rp = |uuid: &str, name: &str, parent: Option<&str>, vgpus| ResourceProviderRow {
            uuid: uuid.into(),
            name: name.into(),
            vcpus: None,
            memory_mb: None,
            disk_gb: None,
            parent: parent.map(Into::into),
            depth: 0,
            host: String::new(),
            descendants: 0,
            vgpus,
            others: vec![],
            custom_traits: vec![],
        };
        let rows = provider_tree(vec![
            rp("g2", "cmp-1_pci_0000_85_00_0", Some("h1"), res(1, 4)),
            rp("h2", "cmp-0", None, None),
            rp("g1", "cmp-1_pci_0000_84_00_0", Some("h1"), res(2, 4)),
            rp("h1", "cmp-1", None, None),
            rp("old", "cmp-2", None, res(3, 8)),
        ]);
        let shape: Vec<_> = rows.iter().map(|r| (r.name.as_str(), r.depth)).collect();
        assert_eq!(
            shape,
            [
                ("cmp-0", 0),
                ("cmp-1", 0),
                ("pci 0000:84:00.0", 1),
                ("pci 0000:85:00.0", 1),
                ("cmp-2", 0)
            ],
            "hosts by name, GPUs under theirs, short names"
        );
        assert_eq!(
            rows[1].vgpus.as_ref().map(|v| (v.used, v.total)),
            Some((3, 8)),
            "a host shows its GPUs' sum"
        );
        assert_eq!(
            rows.iter()
                .map(|r| (r.host.as_str(), r.descendants))
                .collect::<Vec<_>>(),
            [("h2", 0), ("h1", 2), ("h1", 0), ("h1", 0), ("old", 0)],
            "each row knows its host; a host counts what it can collapse"
        );
        let typed = |uuid: &str, parent: Option<&str>, tr: &[&str]| ResourceProviderRow {
            custom_traits: tr.iter().map(|t| t.to_string()).collect(),
            ..rp(uuid, uuid, parent, res(0, 1))
        };
        let host = provider_tree(vec![
            typed("h", None, &[]),
            typed("g1", Some("h"), &["CUSTOM_B"]),
            typed("g2", Some("h"), &["CUSTOM_A", "CUSTOM_B"]),
        ]);
        assert_eq!(
            host[0].custom_traits,
            ["CUSTOM_A", "CUSTOM_B"],
            "a host lists its devices' types once each"
        );
        assert_eq!(
            vgpu_summary(&rows),
            (6, 16, 3, 2),
            "cmp-1's two GPUs and cmp-2's own VGPU (older Nova), not cmp-1's sum twice"
        );
        assert_eq!(
            short_provider_name("cmp-1", "cmp-1_NUMA0"),
            "NUMA0",
            "other children lose only the host prefix"
        );
    }

    #[test]
    fn hypervisor_totals_sum_every_hypervisor() {
        let h = |used: i64, total: i64| HypervisorRow {
            id: "h".into(),
            hostname: "h".into(),
            kind: "QEMU".into(),
            vcpus_used: used,
            vcpus: total,
            ram_used_mb: used * 1024,
            ram_mb: total * 1024,
            disk_used_gb: used * 10,
            disk_gb: total * 10,
            instances: 0,
            up: true,
            enabled: true,
        };
        let hv = Hypervisors {
            hypervisors: vec![h(2, 8), h(3, 16)],
            hosts: Fallible {
                rows: vec![],
                error: None,
            },
            placement: false,
        };
        assert_eq!(hv.totals(), [(5, 24), (5120, 24576), (50, 240)]);
    }

    #[test]
    fn global_usage_totals_every_project_and_csv_has_a_line_each() {
        let p = |name: &str, servers| ProjectUsage {
            project_id: name.into(),
            project: name.into(),
            servers,
            deleted: false,
        };
        let g = GlobalUsage {
            start: "2026-10-01".into(),
            end: "2026-10-30".into(),
            projects: vec![
                p("a, b", vec![u("x", 2, 10.0, "active")]),
                p("c", vec![u("y", 4, 1.5, "terminated"), u("z", 1, 1.0, "active")]),
            ],
        };
        let all = g.summary();
        assert_eq!(
            (all.active_instances(), all.vcpu_hours()),
            (2, 27.0),
            "every project's servers"
        );
        let csv = g.csv();
        assert!(
            csv.starts_with("Usage Report For Period:,2026-10-01,2026-10-30\nActive Instances:,2\n"),
            "{csv}"
        );
        assert!(
            csv.contains(
                "Project Name,VCPUs,RAM (MB),Disk (GB),Usage (Hours)\n\"a, b\",2,2048,10,20.00\nc,1,2048,10,7.00\n"
            ),
            "{csv}"
        );
        let gone = GlobalUsage {
            projects: vec![ProjectUsage {
                deleted: true,
                ..p("dc87", vec![u("x", 2, 1.0, "active")])
            }],
            ..g
        };
        assert!(
            gone.csv().ends_with("dc87 (Deleted),2,2048,10,2.00\n"),
            "Horizon's label (roadmap 89)"
        );
    }

    #[test]
    fn an_empty_period_totals_zero_not_minus_zero() {
        let s = UsageSummary {
            start: "2026-10-01".into(),
            end: "2026-10-24".into(),
            servers: vec![],
        };
        assert!(
            s.vcpu_hours().is_sign_negative(),
            "an empty f64 sum is -0.0: the cause of roadmap 60"
        );
        let csv = s.csv();
        assert!(
            csv.contains("This Period's VCPU-Hours:,0.00\n") && !csv.contains("-0.00"),
            "{csv}"
        );
    }
}

/// Checks a static route: `destination` is a CIDR and `nexthop` an address of
/// the same IP version. Returns them trimmed. Neutron still checks that the
/// next hop is inside one of the router's subnets.
pub fn validate_route(destination: &str, nexthop: &str) -> Result<(String, String), String> {
    use std::net::IpAddr;
    let (dest, hop) = (destination.trim(), nexthop.trim());
    let (net, len) = dest.split_once('/').ok_or_else(|| format!("not a CIDR: {dest}"))?;
    let net: IpAddr = net.parse().map_err(|_| format!("not a CIDR: {dest}"))?;
    let len: u8 = len.parse().map_err(|_| format!("not a CIDR: {dest}"))?;
    if len > if net.is_ipv4() { 32 } else { 128 } {
        return Err(format!("not a CIDR: {dest}"));
    }
    let hop_ip: IpAddr = hop.parse().map_err(|_| format!("not an IP address: {hop}"))?;
    if hop_ip.is_ipv4() != net.is_ipv4() {
        return Err("destination and next hop must be the same IP version".into());
    }
    Ok((dest.to_string(), hop.to_string()))
}

/// What *Edit Image* changes; `None` / empty = unchanged. Custom properties
/// (description, architecture, kernel and ramdisk IDs) go in `set` / `remove`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ImageChanges {
    pub name: Option<String>,
    pub visibility: Option<String>,
    pub protected: Option<bool>,
    pub min_disk_gb: Option<u32>,
    pub min_ram_mb: Option<u32>,
    /// Only while the image is `queued` (Glance refuses it afterwards).
    pub disk_format: Option<String>,
    pub set: Vec<(String, String)>,
    pub remove: Vec<String>,
}

/// *Create Image* (roadmap 45): a Glance record plus, when `url` is set, the `web-download` import.
/// Description, architecture, kernel and ramdisk IDs and the metadata editor's pairs are all
/// custom image properties (`properties`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ImageSpec {
    pub name: String,
    /// Source `http(s)` URL the image service downloads from; `None` leaves a `queued`
    /// record without data (to be filled later, e.g. with `openstack image set`).
    pub url: Option<String>,
    pub disk_format: String,
    /// `None` = Horizon's choice: the disk format for AMI-style images, else `bare`.
    pub container_format: Option<String>,
    pub visibility: String,
    pub protected: bool,
    pub min_disk_gb: Option<u32>,
    pub min_ram_mb: Option<u32>,
    pub properties: Vec<(String, String)>,
    /// Size of the file the browser sends next (`[images] upload`), in bytes. Only the record is
    /// created here: the upload itself is not an [`OsAction`] (it is streamed, roadmap 45), so
    /// this is what the dry-run preview, the size limit and the mode checks go by. Excludes `url`.
    #[serde(default)]
    pub upload_bytes: Option<u64>,
    /// Admin › Images' *Create Image* (roadmap 89): the owning project; `None`
    /// = the session's, as Project's.
    #[serde(default)]
    pub owner: Option<String>,
}

impl OsAction {
    /// The size of the file that follows this action, for *Create Image* › *File*.
    pub fn image_upload_bytes(&self) -> Option<u64> {
        match self {
            OsAction::ImageCreate(spec) => spec.upload_bytes,
            _ => None,
        }
    }
}

/// Where the browser `PUT`s an image's data in `proxy` mode (axum syntax, roadmap 45).
pub const IMAGE_UPLOAD_ROUTE: &str = "/upload/image/{id}";

/// [`IMAGE_UPLOAD_ROUTE`] for image `id`.
pub fn image_upload_path(id: &str) -> String {
    IMAGE_UPLOAD_ROUTE.replace("{id}", id)
}

/// What the browser needs for one direct upload to Glance (`[images] upload = direct`, roadmap
/// 45): the URL and a scoped token for that request. Not `Debug`-printable, so it can't end up
/// in a log by accident.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct UploadTicket {
    pub url: String,
    pub token: String,
}

impl std::fmt::Debug for UploadTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UploadTicket {{ url: {:?}, token: <hidden> }}", self.url)
    }
}

/// Nova, Cinder and Glance accept keys and values of up to 255 characters.
pub const METADATA_MAX_LEN: usize = 255;

/// Checks edited metadata: trimmed keys, none empty or repeated, keys and
/// values within [`METADATA_MAX_LEN`]. Returns the pairs trimmed, sorted by key.
pub fn validate_metadata(pairs: &[(String, String)]) -> Result<Vec<(String, String)>, String> {
    let mut out: Vec<(String, String)> = Vec::with_capacity(pairs.len());
    for (k, v) in pairs {
        let (k, v) = (k.trim(), v.trim());
        if k.is_empty() {
            return Err("every entry needs a key".into());
        }
        if k.chars().count() > METADATA_MAX_LEN || v.chars().count() > METADATA_MAX_LEN {
            return Err(format!(
                "{k}: keys and values are limited to {METADATA_MAX_LEN} characters"
            ));
        }
        if out.iter().any(|(o, _)| o == k) {
            return Err(format!("{k}: repeated key"));
        }
        out.push((k.to_string(), v.to_string()));
    }
    out.sort();
    Ok(out)
}

/// What turns `old` into `new`: the pairs to add or change, and the keys to remove.
pub fn metadata_changes(old: &[(String, String)], new: &[(String, String)]) -> (Vec<(String, String)>, Vec<String>) {
    let set = new.iter().filter(|p| !old.contains(p)).cloned().collect();
    let remove = old
        .iter()
        .filter(|(k, _)| !new.iter().any(|(n, _)| n == k))
        .map(|(k, _)| k.clone())
        .collect();
    (set, remove)
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    fn kv(p: &[(&str, &str)]) -> Vec<(String, String)> {
        p.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn rbac_kinds_map_to_neutron() {
        assert_eq!(rbac_kind("external_network"), Ok(("network", "access_as_external")));
        assert_eq!(rbac_kind("shared_qos_policy"), Ok(("qos_policy", "access_as_shared")));
        assert!(rbac_kind("external_qos_policy").is_err(), "only Horizon's three");
        let del = OsAction::RbacPolicyDelete { id: "r1".into() };
        assert!(del.admin_only() && OsAction::Batch(vec![("r".into(), del)]).admin_only());
    }

    #[test]
    fn ipv6_modes_split_like_horizon() {
        assert_eq!(ipv6_modes("none/none"), Ok((None, None)));
        assert_eq!(
            ipv6_modes("none/slaac"),
            Ok((None, Some("slaac".into()))),
            "an external router announces"
        );
        assert_eq!(
            ipv6_modes("dhcpv6-stateful/dhcpv6-stateful"),
            Ok((Some("dhcpv6-stateful".into()), Some("dhcpv6-stateful".into())))
        );
        assert!(
            ipv6_modes("slaac/dhcpv6-stateful").is_err(),
            "only Horizon's five pairs"
        );
    }

    #[test]
    fn provider_networks_follow_horizons_rules() {
        let p = |ty: &str, phys: Option<&str>, seg: Option<u32>| ProviderNetwork {
            network_type: ty.into(),
            physical_network: phys.map(Into::into),
            segmentation_id: seg,
        };
        let builtin = crate::prefs::NetworkPrefs::default().provider_types();
        assert!(p("vlan", Some("physnet1"), Some(100)).validate(&builtin).is_ok());
        assert!(
            p("vlan", Some("physnet1"), None).validate(&builtin).is_ok(),
            "Neutron picks the VLAN"
        );
        assert!(
            p("vlan", None, Some(100)).validate(&builtin).is_err(),
            "vlan needs a physical network"
        );
        assert!(
            p("vlan", Some("physnet1"), Some(4095)).validate(&builtin).is_err(),
            "VLAN IDs stop at 4094"
        );
        assert!(
            p("flat", Some("physnet1"), Some(1)).validate(&builtin).is_err(),
            "flat has no segmentation ID"
        );
        assert!(
            p("vxlan", Some("physnet1"), None).validate(&builtin).is_err(),
            "vxlan has no physical network"
        );
        assert!(
            p("vxlan", None, Some(1 << 24)).validate(&builtin).is_err(),
            "VNIs are 24 bits"
        );
        assert!(p("geneve", None, Some(5000)).validate(&builtin).is_ok());
        assert!(p("local", None, None).validate(&builtin).is_ok());
        assert!(p("opflex", None, None).validate(&builtin).is_err(), "unknown type");
        let narrow = crate::prefs::NetworkPrefs {
            supported_provider_types: vec!["vlan".into(), "flat".into()],
            segmentation_id_range: [("vlan".to_string(), [100, 200])].into(),
            ..Default::default()
        }
        .provider_types();
        assert!(
            p("vlan", Some("physnet1"), Some(300)).validate(&narrow).is_err(),
            "[network] segmentation_id_range narrows VLANs"
        );
        assert!(p("vlan", Some("physnet1"), Some(150)).validate(&narrow).is_ok());
        assert!(
            p("geneve", None, None).validate(&narrow).is_err(),
            "not in supported_provider_types"
        );
    }

    #[test]
    fn admin_writes_are_marked_even_inside_a_batch() {
        let net = OsAction::AdminNetworkCreate(Box::new(AdminNetworkSpec {
            name: "n".into(),
            project_id: "p1".into(),
            provider: None,
            admin_up: true,
            shared: false,
            external: false,
            mtu: None,
            az_hints: vec![],
            subnet: None,
        }));
        assert!(net.admin_only());
        assert!(OsAction::Batch(vec![("n".into(), net)]).admin_only());
        assert!(!OsAction::NetworkDelete { id: "n1".into() }.admin_only());
    }

    #[test]
    fn metadata_is_trimmed_sorted_and_checked() {
        assert_eq!(
            validate_metadata(&kv(&[(" b ", " 2 "), ("a", "")])),
            Ok(kv(&[("a", ""), ("b", "2")]))
        );
        assert!(validate_metadata(&kv(&[("", "x")])).is_err(), "empty key");
        assert!(
            validate_metadata(&kv(&[("a", "1"), (" a", "2")])).is_err(),
            "repeated key"
        );
        assert!(validate_metadata(&kv(&[("k", &"x".repeat(256))])).is_err(), "too long");
        assert_eq!(validate_metadata(&[]), Ok(vec![]));
    }

    #[test]
    fn changes_list_what_was_added_changed_and_removed() {
        let old = kv(&[("a", "1"), ("b", "2"), ("c", "3")]);
        let new = kv(&[("a", "1"), ("b", "20"), ("d", "4")]);
        assert_eq!(
            metadata_changes(&old, &new),
            (kv(&[("b", "20"), ("d", "4")]), vec!["c".to_string()])
        );
        assert_eq!(metadata_changes(&old, &old), (vec![], vec![]));
    }
}

#[cfg(test)]
mod route_tests {
    use super::validate_route;

    #[test]
    fn routes_need_a_cidr_and_a_next_hop_of_the_same_version() {
        assert_eq!(
            validate_route(" 10.9.0.0/16 ", "10.0.0.1"),
            Ok(("10.9.0.0/16".into(), "10.0.0.1".into()))
        );
        assert!(validate_route("2001:db8::/48", "2001:db8::1").is_ok());
        assert!(validate_route("10.9.0.0", "10.0.0.1").is_err());
        assert!(validate_route("10.9.0.0/33", "10.0.0.1").is_err());
        assert!(validate_route("10.9.0.0/16", "router").is_err());
        assert!(validate_route("10.9.0.0/16", "2001:db8::1").is_err());
    }
}

#[cfg(test)]
mod secret_tests {
    use super::Secret;

    #[test]
    fn debug_never_prints_the_secret() {
        let s = Secret {
            file_name: "k.pem".into(),
            content: "-----BEGIN OPENSSH PRIVATE KEY-----".into(),
            kind: super::SecretKind::PrivateKey,
            fields: vec![("secret".into(), "app-cred-secret".into())],
            extra: vec![(
                "clouds.yaml".into(),
                "application_credential_secret: app-cred-secret".into(),
            )],
        };
        let dbg = format!("{s:?}");
        assert!(dbg.contains("k.pem") && dbg.contains("<redacted>"));
        assert!(
            !dbg.contains("PRIVATE") && !dbg.contains("app-cred-secret"),
            "fields and extra downloads hold it too"
        );
    }
}

#[cfg(test)]
mod launch_rules_tests {
    use super::*;

    fn image(gb_bytes: u64, min_disk: i64, min_ram: Option<i64>) -> Facts {
        Facts {
            size_bytes: Some(gb_bytes),
            min_disk_gb: Some(min_disk),
            min_ram_mb: min_ram,
            ..Default::default()
        }
    }
    fn flavor(ram: i64, disk: i64) -> Facts {
        Facts {
            ram_mb: Some(ram),
            disk_gb: Some(disk),
            vcpus: Some(1),
            ..Default::default()
        }
    }

    #[test]
    fn min_volume_matches_horizon() {
        // ADM-RHEL9.7-ES: 7.80 GB, min_disk 20 → 20 (seen in Horizon).
        assert_eq!(min_volume_gb(&image(7_800_000_000, 20, None)), 20);
        // Debian 12: 428.69 MB, no min_disk → 1 ("must be at least 1 GB").
        assert_eq!(min_volume_gb(&image(428_690_000, 0, None)), 1);
        assert_eq!(min_volume_gb(&image(2_100_000_000, 0, None)), 3);
        assert_eq!(min_volume_gb(&Facts::default()), 1);
    }

    #[test]
    fn flavor_issues_follow_ram_and_local_disk() {
        let img = image(7_800_000_000, 20, Some(4096));
        assert_eq!(
            flavor_issue(&flavor(2048, 50), Some(&img), true),
            Some(FlavorIssue::Ram(4096))
        );
        assert_eq!(
            flavor_issue(&flavor(8192, 10), Some(&img), false),
            Some(FlavorIssue::Disk(20))
        );
        assert_eq!(
            flavor_issue(&flavor(8192, 10), Some(&img), true),
            None,
            "volume boot ignores flavor disk"
        );
        assert_eq!(
            flavor_issue(&flavor(8192, 0), Some(&img), false),
            None,
            "disk 0 = sized from the image"
        );
        assert_eq!(flavor_issue(&flavor(512, 1), None, false), None);
    }

    #[cfg(test)]
    mod capability_tests {
        use super::*;

        fn caps() -> Capabilities {
            Capabilities {
                known: true,
                services: vec!["compute".into(), "network".into(), "volumev3".into(), "image".into()],
                network_extensions: vec!["router".into(), "security-group".into()],
                lb_mv: Some((2, 27)),
                compute_mv: Some((2, 79)),
                volume_mv: Some((3, 70)),
                plugins: vec![],
                roles: vec!["member".into()],
                multidomain: false,
                image_import: None,
            }
        }

        #[test]
        fn domains_need_the_setting_even_when_detection_failed() {
            let admin = Capabilities {
                roles: vec!["admin".into()],
                ..caps()
            };
            assert!(!needs::DOMAINS.met(&admin), "[keystone] multidomain off");
            assert!(needs::DOMAINS.met(&Capabilities {
                multidomain: true,
                ..admin.clone()
            }));
            assert!(
                !needs::DOMAINS.met(&Capabilities {
                    multidomain: true,
                    ..caps()
                }),
                "members don't see it"
            );
            assert!(
                !needs::DOMAINS.met(&Capabilities::default()),
                "configuration doesn't fail open"
            );
        }

        #[test]
        fn requirements_check_services_extensions_and_versions() {
            let c = caps();
            assert!(
                Requires::Service("block-storage").met(&c),
                "volumev3 counts as block-storage"
            );
            assert!(!Requires::Service("orchestration").met(&c));
            assert!(Requires::NetExt("router").met(&c));
            assert!(!Requires::NetExt("extraroute-atomic").met(&c));
            assert!(needs::UNSHELVE_TO_ZONE.met(&c));
            assert!(!needs::REBUILD.met(&c), "2.79 < 2.94");
            assert!(needs::VOLUME_MESSAGES.met(&c));
            assert!(!Requires::All(&[Requires::Service("network"), Requires::NetExt("trunk")]).met(&c));
        }

        #[test]
        fn snat_needs_the_extension_and_an_admin_role() {
            let ext = |roles: &[&str]| Capabilities {
                network_extensions: vec!["ext-gw-mode".into()],
                roles: roles.iter().map(|r| r.to_string()).collect(),
                ..caps()
            };
            assert!(!needs::SNAT.met(&caps()), "member, no extension");
            assert!(!needs::SNAT.met(&ext(&["member", "reader"])));
            assert!(needs::SNAT.met(&ext(&["member", "admin"])));
            assert!(!needs::SNAT.met(&Capabilities {
                network_extensions: vec![],
                ..ext(&["admin"])
            }));
            assert!(needs::SNAT.met(&Capabilities::default()), "unknown: the server decides");
        }

        #[test]
        fn unknown_capabilities_or_versions_fail_open() {
            assert!(Requires::Service("orchestration").met(&Capabilities::default()));
            let c = Capabilities {
                compute_mv: None,
                ..caps()
            };
            assert!(needs::REBUILD.met(&c), "no version range: don't hide");
        }

        #[test]
        fn reserved_image_keys_are_glances_and_the_operators() {
            let reserved = vec!["owner_specified.openstack.md5".to_string()];
            assert!(is_reserved_image_key("os_glance_failed_import", &[]));
            assert!(is_reserved_image_key("owner_specified.openstack.md5", &reserved));
            assert!(!is_reserved_image_key("owner_specified.openstack.md5", &[]));
            assert!(!is_reserved_image_key("hw_disk_bus", &reserved));
            let set = OsAction::ImageSetProperties {
                id: "i".into(),
                set: vec![("a".into(), "1".into())],
                remove: vec!["b".into()],
            };
            assert_eq!(set.image_property_keys(), ["a", "b"]);
            let batch = OsAction::Batch(vec![("x".into(), set)]);
            assert_eq!(batch.image_property_keys(), ["a", "b"], "inside a batch too");
        }

        #[test]
        fn plugins_are_opt_in_even_when_detection_failed() {
            let lb = Requires::All(&[
                Requires::Plugin(Plugin::LoadBalancer),
                Requires::Service("load-balancer"),
            ]);
            assert!(
                !lb.met(&Capabilities::default()),
                "not enabled: hidden although nothing is known"
            );
            let enabled = Capabilities {
                plugins: vec![Plugin::LoadBalancer],
                ..Default::default()
            };
            assert!(lb.met(&enabled), "enabled, detection failed: shown");
            assert!(
                !lb.met(&Capabilities {
                    plugins: vec![Plugin::LoadBalancer],
                    ..caps()
                }),
                "enabled but not in the catalog"
            );
            assert_eq!(Plugin::parse(" orchestration "), Some(Plugin::Orchestration));
            assert_eq!(Plugin::parse("octavia"), None);
            let batch = OsAction::Batch(vec![(
                "a".into(),
                OsAction::LoadBalancerDelete {
                    id: "a".into(),
                    cascade: true,
                },
            )]);
            assert_eq!(batch.plugins(), [Plugin::LoadBalancer]);
            assert!(OsAction::RouterDelete { id: "r".into() }.plugins().is_empty());
        }

        #[test]
        fn parses_microversions() {
            assert_eq!(parse_microversion("2.95"), Some((2, 95)));
            assert_eq!(parse_microversion(" 3.70 "), Some((3, 70)));
            assert_eq!(parse_microversion("v2"), None);
            assert!((2, 100) > (2, 95), "compared numerically, not as text");
        }
    }
}

#[cfg(test)]
mod port_forwarding_tests {
    use super::*;

    fn spec(internal: &str, external: &str, ranges: bool) -> PortForwardingSpec {
        PortForwardingSpec {
            floating_ip_id: "f".into(),
            protocol: "tcp".into(),
            internal_port_id: "p".into(),
            internal_ip: "10.0.0.5".into(),
            internal: internal.into(),
            external: external.into(),
            description: None,
            ranges,
        }
    }

    #[test]
    fn ports_and_ranges_are_validated() {
        assert_eq!(PortSpec::parse(" 22 ", false), Ok(PortSpec::One(22)));
        assert_eq!(PortSpec::parse("8080-8090", true), Ok(PortSpec::Range(8080, 8090)));
        assert_eq!(PortSpec::parse("80:80", true), Ok(PortSpec::One(80)));
        assert!(PortSpec::parse("80:90", false).unwrap_err().contains("ranges"));
        assert!(
            PortSpec::parse("90:80", true).is_err()
                && PortSpec::parse("0", true).is_err()
                && PortSpec::parse("70000", true).is_err()
        );
        assert_eq!(PortSpec::Range(8080, 8090).to_string(), "8080:8090");
        assert_eq!(
            spec("22", "2222", false).ports(),
            Ok((PortSpec::One(22), PortSpec::One(2222)))
        );
        assert!(
            spec("8080:8081", "80:82", true)
                .ports()
                .unwrap_err()
                .contains("same number")
        );
        assert!(spec("22", "2222", false).ports().is_ok());
        assert!(
            PortForwardingSpec {
                protocol: "icmp".into(),
                ..spec("22", "22", false)
            }
            .ports()
            .is_err()
        );
        assert!(
            PortForwardingSpec {
                internal_ip: "fd00::5".into(),
                ..spec("22", "22", false)
            }
            .ports()
            .is_err(),
            "IPv4 only"
        );
    }
}

#[cfg(test)]
mod qos_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rule_bodies_keep_the_fields_of_their_type() {
        let bw = QosRuleSpec {
            kind: "bandwidth_limit".into(),
            direction: Some("ingress".into()),
            max_kbps: Some(10_000),
            max_burst_kbps: Some(1000),
            min_kbps: Some(5),
            ..Default::default()
        };
        assert_eq!(
            bw.body().unwrap(),
            json!({"bandwidth_limit_rule": {"direction": "ingress", "max_kbps": 10000, "max_burst_kbps": 1000}})
        );
        let dscp = QosRuleSpec {
            kind: "dscp_marking".into(),
            dscp_mark: Some(26),
            ..Default::default()
        };
        assert_eq!(dscp.body().unwrap(), json!({"dscp_marking_rule": {"dscp_mark": 26}}));
        assert!(
            QosRuleSpec {
                dscp_mark: Some(27),
                ..dscp.clone()
            }
            .body()
            .is_err(),
            "not a DSCP mark"
        );
        assert!(
            QosRuleSpec {
                direction: Some("egress".into()),
                ..dscp
            }
            .body()
            .is_err(),
            "no direction for DSCP"
        );
        let pps = QosRuleSpec {
            kind: "minimum_packet_rate".into(),
            direction: Some("any".into()),
            min_kpps: Some(1000),
            ..Default::default()
        };
        assert_eq!(
            pps.body().unwrap(),
            json!({"minimum_packet_rate_rule": {"direction": "any", "min_kpps": 1000}})
        );
        assert!(
            QosRuleSpec {
                kind: "minimum_bandwidth".into(),
                direction: Some("any".into()),
                min_kbps: Some(1),
                ..Default::default()
            }
            .body()
            .is_err()
        );
        assert!(
            QosRuleSpec {
                kind: "bandwidth_limit".into(),
                ..Default::default()
            }
            .body()
            .is_err(),
            "max_kbps required"
        );
        assert!(
            QosRuleSpec {
                kind: "packet_loss".into(),
                ..Default::default()
            }
            .body()
            .is_err()
        );
    }
}

#[cfg(test)]
mod port_settings_tests {
    use super::*;

    fn before() -> PortSettings {
        PortSettings {
            name: "eth0".into(),
            admin_up: true,
            vnic_type: Some("normal".into()),
            port_security: true,
            security_group_ids: vec!["g1".into(), "g2".into()],
        }
    }

    #[test]
    fn port_changes_send_only_what_changed() {
        let b = before();
        assert_eq!(b.changes("p", &b), None, "nothing changed");
        let renamed = PortSettings {
            name: " web ".into(),
            ..b.clone()
        };
        assert_eq!(
            b.changes("p", &renamed),
            Some(OsAction::PortUpdate {
                id: "p".into(),
                name: Some("web".into()),
                admin_up: None,
                vnic_type: None,
                port_security: None,
                security_group_ids: None
            })
        );
        let reordered = PortSettings {
            security_group_ids: vec!["g2".into(), "g1".into()],
            ..b.clone()
        };
        assert_eq!(b.changes("p", &reordered), None, "the same groups in another order");
        let groups = PortSettings {
            security_group_ids: vec!["g3".into()],
            ..b.clone()
        };
        assert!(
            matches!(b.changes("p", &groups), Some(OsAction::PortUpdate { security_group_ids: Some(g), name: None, .. }) if g == ["g3"])
        );
    }

    #[test]
    fn port_security_off_clears_the_groups_in_the_same_request() {
        let b = before();
        let off = PortSettings {
            port_security: false,
            ..b.clone()
        };
        assert!(
            matches!(
                b.changes("p", &off),
                Some(OsAction::PortUpdate { port_security: Some(false), security_group_ids: Some(g), .. }) if g.is_empty()
            ),
            "Neutron refuses port_security_enabled=false while the port has groups"
        );
        let bare = PortSettings {
            security_group_ids: vec![],
            port_security: false,
            ..b.clone()
        };
        let rename = PortSettings {
            name: "x".into(),
            ..bare.clone()
        };
        assert!(matches!(
            bare.changes("p", &rename),
            Some(OsAction::PortUpdate {
                security_group_ids: None,
                port_security: None,
                ..
            })
        ));
    }
}

#[cfg(test)]
mod member_rows_tests {
    use super::*;

    fn row(address: &str, port: &str) -> MemberRow {
        MemberRow {
            address: address.into(),
            subnet_id: "sub-1".into(),
            port: port.into(),
            weight: "1".into(),
            admin_up: true,
            ..Default::default()
        }
    }

    #[test]
    fn rows_become_members_with_their_own_port_and_monitor() {
        let mut ext = row(" 192.0.2.10 ", "8443");
        ext.monitor_address = "192.0.2.11".into();
        ext.monitor_port = "9000".into();
        ext.backup = true;
        ext.weight = String::new();
        let json = serde_json::to_string(&[row("10.0.0.5", "80"), ext]).unwrap();
        let m = member_specs(&json).unwrap();
        assert_eq!(
            (m[0].port, m[0].weight, m[0].monitor_address.clone()),
            (80, Some(1), None)
        );
        assert_eq!(m[1].address, "192.0.2.10", "trimmed");
        assert_eq!(
            (m[1].port, m[1].weight, m[1].backup),
            (8443, None, true),
            "empty weight = Octavia's default"
        );
        assert_eq!(
            (m[1].monitor_address.as_deref(), m[1].monitor_port),
            (Some("192.0.2.11"), Some(9000))
        );
        assert!(member_specs("").unwrap().is_empty());
    }

    #[test]
    fn bad_rows_are_refused_with_the_address() {
        let bad = |r: MemberRow| member_specs(&serde_json::to_string(&[r]).unwrap()).unwrap_err();
        assert!(bad(row("10.0.0.5", "")).contains("10.0.0.5: the port is required"));
        assert!(
            bad(MemberRow {
                typed: true,
                subnet_id: String::new(),
                ..row("192.0.2.10", "80")
            })
            .contains("subnet"),
            "an external member needs its subnet"
        );
        assert!(bad(row("10.0.0.5", "70000")).contains("port must be 1-65535"));
        assert!(bad(row("web", "80")).contains("not an IP address"));
        assert!(
            bad(MemberRow {
                weight: "300".into(),
                ..row("10.0.0.5", "80")
            })
            .contains("weight")
        );
        assert!(
            bad(MemberRow {
                monitor_address: "x".into(),
                ..row("10.0.0.5", "80")
            })
            .contains("not an IP address: x")
        );
        let twice = serde_json::to_string(&[row("10.0.0.5", "80"), row("10.0.0.5", "80")]).unwrap();
        assert_eq!(member_specs(&twice).unwrap_err(), "10.0.0.5:80 is listed twice");
        let other_port = serde_json::to_string(&[row("10.0.0.5", "80"), row("10.0.0.5", "81")]).unwrap();
        assert_eq!(
            member_specs(&other_port).unwrap().len(),
            2,
            "same address, another port: allowed"
        );
    }

    fn member(id: &str, address: &str, port: i64) -> LbMember {
        LbMember {
            id: id.into(),
            name: id.into(),
            pool: Ref {
                id: "p".into(),
                name: "p".into(),
            },
            address: address.into(),
            port: Some(port),
            weight: Some(5),
            subnet: Some(Ref {
                id: "sub-1".into(),
                name: "s".into(),
            }),
            backup: false,
            operating_status: "ONLINE".into(),
            provisioning_status: "ACTIVE".into(),
            admin_up: true,
            monitor_address: Some("10.9.9.9".into()),
            monitor_port: Some(8080),
        }
    }

    #[test]
    fn existing_members_round_trip_and_removals_match_address_and_port() {
        let m = member("m1", "10.0.0.5", 80);
        let row = MemberRow::from_member(&m);
        assert!(
            !row.typed,
            "an existing member's address isn't editable: changing it would replace the member"
        );
        assert_eq!(row.subnet_name, "s", "shown by name");
        let spec = row.spec().unwrap();
        assert_eq!(spec.name, None, "a name that is only the ID fallback isn't sent");
        assert_eq!(
            (spec.weight, spec.monitor_address.as_deref(), spec.monitor_port),
            (Some(5), Some("10.9.9.9"), Some(8080)),
            "kept as they are"
        );
        let existing = [m, member("m2", "10.0.0.6", 80)];
        let keep = [spec.clone(), LbMemberSpec { port: 81, ..spec }];
        let gone: Vec<_> = removed_members(&existing, &keep)
            .iter()
            .map(|m| m.id.as_str())
            .collect();
        assert_eq!(gone, ["m2"]);
    }
}

/// Refuses a value that would change the URL it is pasted into (roadmap 48):
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

#[cfg(test)]
mod app_credential_tests {
    use super::*;

    #[test]
    fn expiry_follows_horizon() {
        // 20:20 UTC, the moment of the browser test (21:20 in Madrid).
        let now = chrono::NaiveDate::from_ymd_opt(2026, 10, 26)
            .unwrap()
            .and_hms_opt(20, 20, 0)
            .unwrap();
        assert_eq!(app_credential_expiry("", "", now), Ok(None), "never expires");
        assert_eq!(
            app_credential_expiry("2026-12-31", "", now).unwrap().as_deref(),
            Some("2026-12-31T00:00:00"),
            "a date alone: midnight"
        );
        assert_eq!(
            app_credential_expiry(" 2026-10-27 ", "03:15", now).unwrap().as_deref(),
            Some("2026-10-27T03:15:00"),
            "the credential created in the test"
        );
        assert_eq!(
            app_credential_expiry("", "23:30:15", now).unwrap().as_deref(),
            Some("2026-10-26T23:30:15"),
            "a time alone: today (UTC)"
        );
        assert!(
            app_credential_expiry("31/12/2026", "", now)
                .unwrap_err()
                .contains("YYYY-MM-DD")
        );
        assert!(
            app_credential_expiry("2026-12-31", "6pm", now)
                .unwrap_err()
                .contains("HH:MM")
        );
        // Horizon's pitfalls, refused before sending:
        let today_alone = app_credential_expiry("2026-10-26", "", now).unwrap_err();
        assert!(
            today_alone.contains("2026-10-26 00:00 UTC") && today_alone.contains("now (2026-10-26 20:20 UTC)"),
            "{today_alone}"
        );
        assert!(
            app_credential_expiry("", "19:00", now).is_err(),
            "a time alone that already passed today (UTC)"
        );
        assert!(
            app_credential_expiry("", "21:00", now).is_ok(),
            "21:00 UTC is later, though it is 21:20 in Madrid: the form is in UTC"
        );
    }

    #[test]
    fn secret_expiry_is_flagged_30_days_ahead() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 10, 27)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        assert_eq!(expiry(None, now), Expiry::Never);
        assert_eq!(
            expiry(Some("2026-10-27T11:59:00"), now),
            Expiry::Past,
            "Barbican's zoneless UTC"
        );
        assert_eq!(expiry(Some("2026-11-20T00:00:00+00:00"), now), Expiry::Soon);
        assert_eq!(expiry(Some("2027-01-01T00:00:00Z"), now), Expiry::Later);
        assert_eq!(expiry(Some("soon"), now), Expiry::Later, "unreadable: nothing to flag");
    }

    #[test]
    fn identity_actions_are_pointed_at_the_signed_in_user() {
        let spec = AppCredentialSpec {
            user_id: "someone-else".into(),
            name: "ci".into(),
            description: None,
            secret: None,
            expires_at: None,
            roles: vec![],
            unrestricted: false,
            access_rules: None,
        };
        let mut a = OsAction::Batch(vec![
            (
                "a".into(),
                OsAction::AppCredentialDelete {
                    user_id: "someone-else".into(),
                    id: "x".into(),
                },
            ),
            ("b".into(), OsAction::AppCredentialCreate(Box::new(spec))),
        ]);
        a.set_user("me");
        let OsAction::Batch(items) = a else { unreachable!() };
        assert!(matches!(&items[0].1, OsAction::AppCredentialDelete { user_id, .. } if user_id == "me"));
        assert!(
            matches!(&items[1].1, OsAction::AppCredentialCreate(s) if s.user_id == "me"),
            "whatever the browser sent"
        );
    }
}

#[cfg(test)]
mod identity_admin_tests {
    use super::*;

    #[test]
    fn nobody_locks_themselves_out() {
        let (me, here) = ("u-me", "p-here");
        let no = |a: OsAction| a.identity_safety(me, here).is_err();
        assert!(no(OsAction::UserDelete { id: me.into() }));
        assert!(no(OsAction::UserSetEnabled {
            id: me.into(),
            enabled: false
        }));
        assert!(
            !no(OsAction::UserSetEnabled {
                id: me.into(),
                enabled: true
            }),
            "enabling is fine"
        );
        assert!(!no(OsAction::UserDelete { id: "u-other".into() }));
        assert!(no(OsAction::ProjectDelete { id: here.into() }));
        assert!(no(OsAction::ProjectUpdate {
            id: here.into(),
            name: "x".into(),
            description: String::new(),
            enabled: false
        }));
        assert!(
            !no(OsAction::ProjectUpdate {
                id: here.into(),
                name: "renamed".into(),
                description: String::new(),
                enabled: true
            }),
            "renaming is fine"
        );
        let revoke_me = OsAction::ProjectRoleRevoke {
            project_id: here.into(),
            user_id: Some(me.into()),
            group_id: None,
            role_id: "r".into(),
        };
        assert!(no(revoke_me.clone()));
        assert!(
            !no(OsAction::ProjectRoleRevoke {
                project_id: "p-elsewhere".into(),
                user_id: Some(me.into()),
                group_id: None,
                role_id: "r".into()
            }),
            "another project"
        );
        assert!(
            no(OsAction::Batch(vec![
                ("ok".into(), OsAction::UserDelete { id: "u-other".into() }),
                ("bad".into(), revoke_me)
            ])),
            "inside a batch"
        );
    }

    #[test]
    fn creations_are_put_in_the_users_domain() {
        let spec = UserSpec {
            domain_id: "Default".into(),
            name: "ana".into(),
            description: None,
            email: None,
            password: "p".into(),
            project_id: None,
            role_id: None,
            enabled: true,
            lock_password: false,
        };
        let mut a = OsAction::Batch(vec![
            (
                "p".into(),
                OsAction::ProjectCreate {
                    name: "MyTest".into(),
                    description: None,
                    enabled: true,
                    domain_id: String::new(),
                },
            ),
            ("u".into(), OsAction::UserCreate(Box::new(spec))),
            (
                "g".into(),
                OsAction::GroupCreate {
                    name: "ops".into(),
                    description: None,
                    domain_id: String::new(),
                },
            ),
        ]);
        assert!(a.needs_domain());
        a.set_domain("d-spain");
        let OsAction::Batch(items) = a else { unreachable!() };
        assert!(matches!(&items[0].1, OsAction::ProjectCreate { domain_id, .. } if domain_id == "d-spain"));
        assert!(
            matches!(&items[1].1, OsAction::UserCreate(s) if s.domain_id == "d-spain"),
            "whatever the browser sent"
        );
        assert!(matches!(&items[2].1, OsAction::GroupCreate { domain_id, .. } if domain_id == "d-spain"));
        assert!(
            !OsAction::RoleCreate { name: "x".into() }.needs_domain(),
            "roles are global"
        );
    }

    #[test]
    fn member_changes_are_only_what_changed() {
        let row = |id: &str, roles: &[&str]| AssignmentRow {
            actor_id: id.into(),
            label: format!("user-{id}"),
            role_ids: roles.iter().map(|r| r.to_string()).collect(),
        };
        let before = [row("a", &["member"]), row("b", &["member", "reader"])];
        let after = [row("a", &["member", "admin"]), row("c", &["reader"])];
        let changes = assignment_changes("p", false, &before, &after);
        let kinds: Vec<_> = changes
            .iter()
            .map(|(_, a)| match a {
                OsAction::ProjectRoleGrant { user_id, role_id, .. } => {
                    format!("+{}:{role_id}", user_id.clone().unwrap())
                }
                OsAction::ProjectRoleRevoke { user_id, role_id, .. } => {
                    format!("-{}:{role_id}", user_id.clone().unwrap())
                }
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(
            kinds,
            ["+a:admin", "+c:reader", "-b:member", "-b:reader"],
            "a gains admin, c is added, b is removed"
        );
        assert!(assignment_changes("p", false, &before, &before).is_empty());
        let g = assignment_changes("p", true, &[], &[row("g", &["member"])]);
        assert!(
            matches!(&g[0].1, OsAction::ProjectRoleGrant { group_id: Some(gid), user_id: None, .. } if gid == "g"),
            "groups use the group path"
        );
    }
}

#[cfg(test)]
mod lb_option_tests {
    use super::*;

    /// Roadmap 54: what Octavia would refuse, refused before sending.
    #[test]
    fn listener_options_and_pool_tls_follow_octavia_rules() {
        let ok = LbListenerOptions {
            timeout_client_data: Some(1000),
            insert_headers: vec!["X-Forwarded-For".into()],
            allowed_cidrs: Some(vec!["10.0.0.0/8".into(), "2001:db8::/32".into()]),
            ..Default::default()
        };
        assert!(ok.check("HTTP").is_ok());
        assert!(ok.check("TCP").is_err(), "headers only on HTTP listeners");
        let bad = |o: LbListenerOptions| o.check("HTTP").is_err();
        assert!(bad(LbListenerOptions {
            timeout_member_data: Some(LB_TIMEOUT_MAX + 1),
            ..Default::default()
        }));
        assert!(bad(LbListenerOptions {
            insert_headers: vec!["X-Evil".into()],
            ..Default::default()
        }));
        assert!(
            bad(LbListenerOptions {
                allowed_cidrs: Some(vec!["10.0.0.0".into()]),
                ..Default::default()
            }),
            "a CIDR needs its length"
        );
        assert!(check_pool_tls("HTTP", true, Some("ECDHE-RSA-AES128-GCM-SHA256:AES256")).is_ok());
        assert!(check_pool_tls("UDP", true, None).is_err());
        assert!(
            check_pool_tls("HTTP", false, Some("AES256")).is_err(),
            "ciphers need TLS"
        );
        assert!(
            check_pool_tls("HTTP", true, Some("AES256 AES128")).is_err(),
            "colon-separated"
        );
        assert!(check_pool_tls("HTTP", false, Some(" ")).is_ok(), "blank = none");
    }

    #[test]
    fn listener_certificates_belong_to_terminated_https() {
        let r = "https://kms/v1/containers/c1".to_string();
        assert!(check_listener_tls(LB_TLS_PROTOCOL, Some(&r), &[]).is_ok());
        assert!(
            check_listener_tls(LB_TLS_PROTOCOL, Some(" "), &[]).is_err(),
            "it needs one"
        );
        assert!(
            check_listener_tls(LB_TLS_PROTOCOL, Some("c1"), &[]).is_err(),
            "a Barbican URL"
        );
        assert!(check_listener_tls("HTTP", None, std::slice::from_ref(&r)).is_err());
        assert!(check_listener_tls("HTTP", None, &[]).is_ok());
        let o = LbListenerOptions {
            tls_ciphers: Some("AES256".into()),
            ..Default::default()
        };
        assert!(
            o.check("HTTP").is_err() && o.check(LB_TLS_PROTOCOL).is_ok(),
            "listener ciphers only when TLS ends there"
        );
        assert!(
            lb_inserts_headers(LB_TLS_PROTOCOL) && !lb_inserts_headers("HTTPS"),
            "HTTPS passes through: no headers"
        );
    }

    #[test]
    fn octavia_version_gates_the_options() {
        let c = |v| Capabilities {
            known: true,
            lb_mv: v,
            ..Default::default()
        };
        assert!(needs::LB_TIMEOUTS.met(&c(Some((2, 1)))));
        assert!(!needs::LB_ALLOWED_CIDRS.met(&c(Some((2, 11)))));
        assert!(needs::LB_POOL_CIPHERS.met(&c(Some((2, 27)))));
        assert!(needs::LB_POOL_TLS.met(&c(None)), "no version found: fail open");
    }

    #[test]
    fn url_import_needs_glance_web_download() {
        let c = |m: Option<&[&str]>| Capabilities {
            known: true,
            image_import: m.map(|l| l.iter().map(|s| s.to_string()).collect()),
            ..Default::default()
        };
        assert!(needs::IMAGE_URL_IMPORT.met(&c(Some(&["glance-direct", "web-download"]))));
        assert!(
            !needs::IMAGE_URL_IMPORT.met(&c(Some(&["glance-direct"]))),
            "Glance would reject the import"
        );
        assert!(!needs::IMAGE_URL_IMPORT.met(&c(Some(&[]))), "import disabled");
        assert!(needs::IMAGE_URL_IMPORT.met(&c(None)), "list unreadable: fail open");
    }
}

#[cfg(test)]
mod lb_busy_tests {
    use super::*;

    /// Roadmap 54: the detail page polls while any part is PENDING_*.
    #[test]
    fn a_pending_part_makes_the_load_balancer_busy() {
        let d: LoadBalancerDetail = serde_json::from_value(serde_json::json!({
            "row": {"id": "lb1", "name": "lb", "description": null, "vip_address": null, "availability_zone": null,
                    "operating_status": "ONLINE", "provisioning_status": "ACTIVE", "admin_up": true, "provider": null},
            "project_id": "p", "flavor_id": null, "vip_port_id": null, "vip_network": null, "vip_subnet": null,
            "floating_ip": null, "created": null, "updated": null, "listeners": [], "pools": [], "members": [], "monitors": [],
            "l7policies": [{"id": "p1", "name": "p", "description": null, "listener": {"id": "l", "name": "l"}, "action": "REJECT",
                            "position": 1, "redirect_pool": null, "redirect_url": null, "operating_status": "ONLINE",
                            "provisioning_status": "ACTIVE", "admin_up": true,
                            "rules": [{"id": "r", "rule_type": "PATH", "compare_type": "EQUAL_TO", "key": null, "value": "/",
                                       "invert": false, "operating_status": "OFFLINE", "provisioning_status": "PENDING_CREATE", "admin_up": true}]}]
        }))
        .unwrap();
        assert!(d.busy(), "a rule being created");
        assert!(!lb_pending("ACTIVE") && lb_pending("PENDING_DELETE"));
    }
}

#[cfg(test)]
mod secret_acl_tests {
    use super::*;

    #[test]
    fn secret_acl_is_for_its_creator_and_admins() {
        let roles = |r: &[&str]| r.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(
            can_edit_secret_acl(Some("u1"), "u1", &roles(&["member"])),
            "the creator"
        );
        assert!(
            can_edit_secret_acl(Some("u2"), "u1", &roles(&["admin"])),
            "a project admin"
        );
        assert!(!can_edit_secret_acl(Some("u2"), "u1", &roles(&["member", "reader"])));
        assert!(
            !can_edit_secret_acl(None, "u1", &roles(&[])),
            "creator unknown: not the creator"
        );
    }

    #[test]
    fn typed_user_ids_split_and_dedupe() {
        assert_eq!(parse_user_ids(" a1, b2\nc3  a1,,"), ["a1", "b2", "c3"]);
        assert!(parse_user_ids(" , ").is_empty());
    }
}

#[cfg(test)]
mod request_tests {
    use super::*;

    #[test]
    fn requests_reach_the_browser_only_when_shown() {
        let req = RequestPreview {
            service: "compute".into(),
            method: "POST".into(),
            path: "/servers".into(),
            body: None,
        };
        let out = ActionOutcome {
            mode: WriteMode::DryRun,
            requests: vec![req.clone()],
            secret: None,
            results: vec![],
            created: None,
        };
        assert_eq!(out.clone().shown(true).requests.len(), 1);
        assert!(out.shown(false).requests.is_empty(), "dry-run too (roadmap 78)");
        let reply = ConsoleReply {
            mode: WriteMode::On,
            requests: vec![req],
            data: Some(1),
        };
        assert_eq!(
            reply.shown(false),
            ConsoleReply {
                mode: WriteMode::On,
                requests: vec![],
                data: Some(1)
            }
        );
    }
}

#[cfg(test)]
mod quota_tests {
    use super::*;

    fn row(key: &str, used: Option<i64>, limit: Option<i64>) -> QuotaRow {
        QuotaRow {
            key: key.into(),
            used,
            reserved: None,
            limit,
        }
    }

    #[test]
    fn quota_changes_keep_only_what_changed_and_refuse_below_usage() {
        let rows = [
            row("instances", Some(4), Some(10)),
            row("cores", Some(8), None),
            row("member", None, Some(50)),
        ];
        let w = |v: &[(&str, i64)]| v.iter().map(|(k, n)| (k.to_string(), *n)).collect::<Vec<_>>();
        assert!(
            quota_changes(&rows, &w(&[("instances", 10), ("cores", -1)]))
                .unwrap()
                .is_empty(),
            "unchanged, unlimited = -1"
        );
        let c = quota_changes(
            &rows,
            &w(&[("instances", 4), ("cores", 20), ("member", 0), ("unknown", 3)]),
        )
        .unwrap();
        assert_eq!(
            c.into_iter().collect::<Vec<_>>(),
            [
                ("cores".to_string(), 20),
                ("instances".to_string(), 4),
                ("member".to_string(), 0)
            ],
            "no usage reported: any value"
        );
        assert!(
            quota_changes(&rows, &w(&[("instances", 3)]))
                .unwrap_err()
                .contains("below the 4 in use")
        );
        assert!(
            quota_changes(&rows, &w(&[("instances", -1)])).is_ok(),
            "unlimited is never below usage"
        );
        assert!(quota_changes(&rows, &w(&[("instances", -2)])).is_err());
    }

    #[test]
    fn quota_units_and_load_balancer_plugin() {
        assert_eq!(QuotaService::Compute.unit("ram"), QuotaUnit::Mib);
        assert_eq!(QuotaService::Volume.unit("gigabytes_ceph"), QuotaUnit::Gib);
        assert_eq!(QuotaService::Network.unit("port"), QuotaUnit::Count);
        let lb = OsAction::QuotaUpdate {
            service: QuotaService::LoadBalancer,
            project_id: "p".into(),
            values: Default::default(),
        };
        assert_eq!(lb.plugins(), [Plugin::LoadBalancer]);
        let km = OsAction::QuotaUpdate {
            service: QuotaService::KeyManager,
            project_id: "p".into(),
            values: Default::default(),
        };
        assert_eq!(
            km.plugins(),
            [Plugin::KeyManager],
            "Barbican quotas need the key-manager plugin"
        );
    }

    #[test]
    fn another_projects_quotas_need_the_admin_role() {
        let q = |p: &str| OsAction::QuotaUpdate {
            service: QuotaService::Compute,
            project_id: p.into(),
            values: Default::default(),
        };
        assert!(
            !q("mine").other_projects_quotas("mine"),
            "the session's project: the cloud's policy decides"
        );
        assert!(q("other").other_projects_quotas("mine"));
        assert!(
            OsAction::Batch(vec![("a".into(), q("mine")), ("b".into(), q("other"))]).other_projects_quotas("mine"),
            "inside a batch too"
        );
        assert!(!OsAction::ServerStart { id: "x".into() }.other_projects_quotas("mine"));
    }
}
