//! Write plumbing: the write mode, request previews, action outcomes, consoles and the specs the write dialogs send.

use super::*;

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

/// What an instance *Log* / *Console* call returned: `data` is
/// `None` unless the request was sent (`[writes] mode = on`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConsoleReply<T> {
    pub mode: WriteMode,
    /// The requests, sent or previewed; cleared unless `[ui] show_requests`.
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
    /// The URL for the *Console* tab's frame: noVNC asked to scale
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
    /// exists): *Create Image* › *File* uploads to it.
    #[serde(default)]
    pub created: Option<String>,
}

impl ActionOutcome {
    /// Without the REST requests unless `[ui] show_requests`: the
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
    /// `openrc` and `clouds.yaml`).
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
    /// An application credential's ID and secret.
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
    /// Instances to create in one request (`min_count` = `max_count`), 1 to 100.
    pub count: u32,
    pub source: BootSource,
    /// The image, volume or volume snapshot ID, per `source`.
    pub source_id: String,
    /// Image source only: create a new volume from the image (boot from volume).
    pub create_volume: bool,
    /// GiB of the new boot volume: required with `create_volume`; from a volume snapshot,
    /// `None` (or 0) keeps the snapshot's size. Ignored for an existing volume.
    pub volume_size_gb: Option<u32>,
    pub delete_volume_on_terminate: bool,
    pub flavor_id: String,
    pub network_ids: Vec<String>,
    /// Existing ports (Horizon's *Network Ports* step), after the networks.
    #[serde(default)]
    pub port_ids: Vec<String>,
    /// Security group **names** (Nova's `security_groups` takes names).
    pub security_groups: Vec<String>,
    pub key_name: Option<String>,
    /// Sent as the `group` scheduler hint.
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

/// Admin › Volumes' *Manage Volume*, Horizon's form: a volume
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

/// The provider network types Neutron's ML2 plugin knows, with the range of their segmentation
/// ID (VLAN tag, GRE key, VNI); `None` for the types that have none. Horizon's
/// `SEGMENTATION_ID_RANGE` defaults; `[network] segmentation_id_range` may narrow them.
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

/// Admin › Flavors' *Create Flavor*, Horizon's form. A flavor
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

/// Admin › Routers' *Create Router*: Project's fields plus the
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
    /// `None` = any protocol.
    pub protocol: Option<String>,
    /// `None` = any port; alone (no `port_max`) it is a single port, sent as both bounds.
    pub port_min: Option<i32>,
    pub port_max: Option<i32>,
    /// The remote CIDR or the remote group, never both; neither = any source.
    pub remote_ip_prefix: Option<String>,
    pub remote_group_id: Option<String>,
    pub description: Option<String>,
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
        assert!(out.shown(false).requests.is_empty(), "dry-run too");
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
