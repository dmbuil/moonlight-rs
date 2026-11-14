//! Deployment preferences from `moonlight.conf` that the browser needs too
//! (after Horizon's `local_settings.py`). The serde defaults here
//! are the only definition of each default: a missing section or key, in the
//! file or in the session info, always gets the same value.
//!
//! Where Moonlight already behaved a certain way, that is the default;
//! otherwise Horizon's default is used.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// Sent to the browser with the session info.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// `[ui] poll_interval_ms`: refetch interval of resources in transition.
    pub poll_interval_ms: u32,
    /// `[ui] bug_url`: *Report a Bug* in the user menu.
    pub bug_url: Option<String>,
    /// `[ui] background_batch_from`: a bulk action with this many
    /// items or more runs in the background, with a progress card, instead of
    /// in its dialog. `0` keeps every batch in the dialog.
    pub background_batch_from: u32,
    /// `[ui] page_size`: rows per page of the paged lists. `0` loads
    /// every row at once, as before paging.
    pub page_size: u32,
    /// `[ui] paged_lists`: which lists are paged.
    pub paged_lists: PagedLists,
    /// `[ui] quota_edit`: who sees *Edit Quotas* on Project › Quotas.
    pub quota_edit: QuotaEdit,
    pub launch: LaunchPrefs,
    pub network: NetworkPrefs,
    pub images: ImagePrefs,
    pub console: ConsolePrefs,
    pub admin: AdminPrefs,
}

/// `[admin]`: Horizon's `ENABLE_FLAVOR_EDIT` and `FILTER_DATA_FIRST`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminPrefs {
    /// Admin › Flavors' *Edit Flavor*: Nova can't change a flavor, so it is
    /// deleted and created again with the same ID, extra specs and access.
    pub flavor_edit: bool,
    /// Paged Admin lists that stay empty, without asking OpenStack, until a
    /// filter is given.
    pub filter_first: Vec<crate::model::PagedList>,
}

/// Which lists `[ui] page_size` applies to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PagedLists {
    /// The big Admin lists only (the default; Project panels were measured small).
    #[default]
    Admin,
    /// The Project panels as well.
    All,
}

/// Who may edit the session's own project quotas from Project › Quotas
///. Another project's quotas (Identity › Projects › *Modify
/// Quotas*) always need the `admin` role, whatever this says; Admin ›
/// *Defaults* is not affected either.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaEdit {
    /// Everyone sees *Edit Quotas*; the services' policy decides (the default,
    /// the behaviour before the setting).
    #[default]
    All,
    /// Only tokens with the `admin` role (`needs::ADMIN`).
    Admin,
    /// Nobody: Project › Quotas is read-only, as Horizon's project dashboard.
    Off,
}

impl QuotaEdit {
    /// Whether a user, an admin or not, may edit their own project's quotas.
    pub fn allows(self, is_admin: bool) -> bool {
        match self {
            QuotaEdit::All => true,
            QuotaEdit::Admin => is_admin,
            QuotaEdit::Off => false,
        }
    }
}

impl Prefs {
    /// Rows per page for a list, `None` = load them all: `page_size` is `0`, or
    /// the list is a Project panel and `paged_lists` is `admin`.
    pub fn page_size_for(&self, admin_list: bool) -> Option<u32> {
        let applies = admin_list || self.paged_lists == PagedLists::All;
        (self.page_size > 0 && applies).then_some(self.page_size)
    }
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            poll_interval_ms: 5000,
            bug_url: None,
            background_batch_from: 10,
            page_size: 100,
            paged_lists: PagedLists::Admin,
            quota_edit: QuotaEdit::All,
            launch: LaunchPrefs::default(),
            network: NetworkPrefs::default(),
            images: ImagePrefs::default(),
            console: ConsolePrefs::default(),
            admin: AdminPrefs::default(),
        }
    }
}

/// What the login page needs, before there is a session (`[login]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoginPrefs {
    pub default_domain: String,
    /// A dropdown instead of a text field when not empty.
    pub domain_choices: Vec<String>,
    pub domain_hidden: bool,
    pub password_autocomplete: bool,
    pub password_reveal: bool,
    /// Contents of `[login] message_file`.
    pub message: Option<String>,
    /// `[websso] choices` when `[websso] enabled`: Horizon's
    /// *Authenticate using* select. Empty = password sign-in only, no select.
    pub websso_choices: Vec<SsoChoice>,
    /// `[websso] initial_choice`: preselected in that select.
    pub websso_initial: String,
}

impl Default for LoginPrefs {
    fn default() -> Self {
        LoginPrefs {
            default_domain: "Default".into(),
            domain_choices: vec![],
            domain_hidden: false,
            password_autocomplete: false,
            password_reveal: true,
            message: None,
            websso_choices: vec![],
            websso_initial: String::new(),
        }
    }
}

/// The server route that sends the browser to Keystone for a WebSSO choice:
/// `?choice=<id>` (`server::websso::start`).
pub const WEBSSO_START: &str = "/auth/websso/start";

/// One way to sign in (`WEBSSO_CHOICES`): [`SsoChoice::CREDENTIALS`] is the
/// password form, any other ID is a Keystone federation protocol, or a key of
/// `[websso] idp_mapping`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SsoChoice {
    pub id: String,
    /// Shown in the select; empty = the translated *Keystone Credentials* for
    /// `credentials`, else the ID.
    #[serde(default)]
    pub label: String,
}

impl SsoChoice {
    /// The ID of the password form, as in Horizon.
    pub const CREDENTIALS: &str = "credentials";
}

/// Polling stops after this long, whatever the interval.
pub const POLL_FOR_MS: u32 = 10 * 60 * 1000;

impl Prefs {
    /// Polling rounds that fit in [`POLL_FOR_MS`].
    pub fn poll_rounds(&self) -> u32 {
        POLL_FOR_MS / self.poll_interval_ms.max(1)
    }
}

/// `[launch]`: Horizon's `LAUNCH_INSTANCE_DEFAULTS` and the hypervisor features.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LaunchPrefs {
    /// *Create New Volume* preselected for image sources (the production cloud boots from volume).
    pub create_volume: bool,
    pub config_drive: bool,
    /// Boot sources offered in the wizard.
    pub disable_image: bool,
    pub disable_instance_snapshot: bool,
    pub disable_volume: bool,
    pub disable_volume_snapshot: bool,
    /// A key pair must be chosen (`OPENSTACK_HYPERVISOR_FEATURES["requires_keypair"]`).
    pub requires_keypair: bool,
    /// Offer an admin password field (`["can_set_password"]`).
    pub can_set_password: bool,
    /// Flavor order in the wizard (`CREATE_INSTANCE_FLAVOR_SORT`).
    pub flavor_sort: FlavorSort,
    /// Sort `flavor_sort` descending.
    pub flavor_sort_reverse: bool,
}

impl Default for LaunchPrefs {
    fn default() -> Self {
        LaunchPrefs {
            create_volume: true,
            config_drive: false,
            disable_image: false,
            disable_instance_snapshot: false,
            disable_volume: false,
            disable_volume_snapshot: false,
            requires_keypair: false,
            can_set_password: false,
            flavor_sort: FlavorSort::Ram,
            flavor_sort_reverse: false,
        }
    }
}

impl LaunchPrefs {
    /// Boot sources offered, in Horizon's order. Never empty: if every source
    /// is disabled the image source stays (validated when loading).
    pub fn sources(&self) -> Vec<&'static str> {
        [
            ("image", self.disable_image),
            ("instance_snapshot", self.disable_instance_snapshot),
            ("volume", self.disable_volume),
            ("volume_snapshot", self.disable_volume_snapshot),
        ]
        .into_iter()
        .filter(|(_, off)| !off)
        .map(|(s, _)| s)
        .collect()
    }
}

/// `[launch] flavor_sort`: the column *Launch Instance* sorts flavors by (Horizon's
/// `CREATE_INSTANCE_FLAVOR_SORT`, RAM by default). Written in lower case in the file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlavorSort {
    Id,
    Name,
    #[default]
    Ram,
    Disk,
    Vcpus,
}

/// `[network]`: Horizon's `OPENSTACK_NEUTRON_NETWORK` and `ALLOWED_PRIVATE_SUBNET_CIDR`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkPrefs {
    /// Prefilled in *Create Subnet* (one per line).
    pub default_dns_nameservers: Vec<String>,
    /// Subnet CIDRs users may create; empty = anything.
    pub allowed_private_subnet_cidr: AllowedCidrs,
    /// Offer IPv6 when creating subnets.
    pub enable_ipv6: bool,
    /// *Associate Floating IP* only offers ports on networks that a router with
    /// an external gateway reaches (Horizon's `enable_fip_topology_check`).
    pub fip_topology_check: bool,
    /// *Network Topology* draws its *Topology* and *Graph* tabs only up to this
    /// many networks (own, shared and external); above it they show a note and
    /// the *List* view stays available. `0`: no limit.
    pub topology_max_networks: usize,
    /// Admin *Create Network*'s provider types, in this order (Horizon's
    /// `supported_provider_types`); `["*"]`: every type of
    /// [`crate::model::PROVIDER_NETWORK_TYPES`].
    pub supported_provider_types: Vec<String>,
    /// Per type, `[min, max]` segmentation IDs (Horizon's
    /// `segmentation_id_range`), within the type's own range; a type left out
    /// keeps its full range.
    pub segmentation_id_range: std::collections::BTreeMap<String, [u32; 2]>,
}

impl Default for NetworkPrefs {
    fn default() -> Self {
        NetworkPrefs {
            default_dns_nameservers: vec![],
            allowed_private_subnet_cidr: AllowedCidrs::default(),
            enable_ipv6: true,
            fip_topology_check: true,
            topology_max_networks: 50,
            supported_provider_types: vec!["*".into()],
            segmentation_id_range: Default::default(),
        }
    }
}

/// A provider network type and its segmentation ID range (`None`: it has
/// none, as `flat`).
pub type ProviderType = (String, Option<(u32, u32)>);

impl NetworkPrefs {
    /// The provider types admin *Create Network* offers and the server
    /// accepts: the supported ones, with their configured ranges.
    pub fn provider_types(&self) -> Vec<ProviderType> {
        let known = crate::model::PROVIDER_NETWORK_TYPES;
        let all = self.supported_provider_types.iter().any(|t| t.trim() == "*");
        let names: Vec<&str> = if all {
            known.iter().map(|(t, _)| *t).collect()
        } else {
            self.supported_provider_types.iter().map(|t| t.trim()).collect()
        };
        names
            .into_iter()
            .filter_map(|n| known.iter().find(|(t, _)| *t == n))
            .map(|(t, range)| {
                (
                    t.to_string(),
                    range.map(|r| self.segmentation_id_range.get(*t).map_or(r, |[lo, hi]| (*lo, *hi))),
                )
            })
            .collect()
    }

    /// Start-up checks: known types, ranges only for types that have one,
    /// inside it and in order.
    pub fn check_provider_types(&self) -> Result<(), String> {
        let known = crate::model::PROVIDER_NETWORK_TYPES;
        for t in &self.supported_provider_types {
            let t = t.trim();
            if t != "*" && !known.iter().any(|(k, _)| *k == t) {
                return Err(format!("[network] supported_provider_types: unknown type {t:?}"));
            }
        }
        for (t, [lo, hi]) in &self.segmentation_id_range {
            let Some((_, range)) = known.iter().find(|(k, _)| k == t) else {
                return Err(format!("[network] segmentation_id_range: unknown type {t:?}"));
            };
            let Some((min, max)) = range else {
                return Err(format!("[network] segmentation_id_range: {t} has no segmentation ID"));
            };
            if lo > hi || lo < min || hi > max {
                return Err(format!(
                    "[network] segmentation_id_range.{t}: [{lo}, {hi}] must be ordered and within {min}–{max}"
                ));
            }
        }
        if self.provider_types().is_empty() {
            return Err("[network] supported_provider_types cannot be empty".into());
        }
        Ok(())
    }
}

/// `[network.allowed_private_subnet_cidr]`: the ranges a new subnet must fall in, per IP version
/// (Horizon's `ALLOWED_PRIVATE_SUBNET_CIDR`). An empty list allows any subnet of that version;
/// each entry is validated as a CIDR when the config loads.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AllowedCidrs {
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
}

/// Why a new subnet is refused (the UI localises it).
#[derive(Clone, Debug, PartialEq)]
pub enum SubnetIssue {
    NotACidr,
    Ipv6Disabled,
    /// Outside every allowed range (listed).
    Outside(Vec<String>),
}

impl std::fmt::Display for SubnetIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubnetIssue::NotACidr => f.write_str("not a CIDR"),
            SubnetIssue::Ipv6Disabled => f.write_str("IPv6 subnets are disabled on this Moonlight"),
            SubnetIssue::Outside(r) => write!(f, "outside the allowed ranges ({})", r.join(", ")),
        }
    }
}

impl NetworkPrefs {
    /// Checks a new subnet against `allowed_private_subnet_cidr` and `enable_ipv6`.
    pub fn check_subnet(&self, cidr: &str) -> Result<(), SubnetIssue> {
        let net = Cidr::parse(cidr).map_err(|_| SubnetIssue::NotACidr)?;
        if net.addr.is_ipv6() && !self.enable_ipv6 {
            return Err(SubnetIssue::Ipv6Disabled);
        }
        let allowed = if net.addr.is_ipv4() {
            &self.allowed_private_subnet_cidr.ipv4
        } else {
            &self.allowed_private_subnet_cidr.ipv6
        };
        if allowed.is_empty()
            || allowed
                .iter()
                .filter_map(|a| Cidr::parse(a).ok())
                .any(|a| a.contains(&net))
        {
            Ok(())
        } else {
            Err(SubnetIssue::Outside(allowed.clone()))
        }
    }
}

/// `[images]`: Horizon's `OPENSTACK_IMAGE_BACKEND` and `CREATE_IMAGE_DEFAULTS`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImagePrefs {
    /// Disk formats offered (Upload to Image, Edit Image).
    pub disk_formats: Vec<String>,
    /// Preselected visibility where one is chosen.
    pub default_visibility: String,
    /// Offer *Create Image* › *URL* (Glance `web-download` import; Horizon's
    /// `IMAGES_ALLOW_LOCATION`). Off by default: the cloud fetches the URL.
    pub allow_location: bool,
    /// How *Create Image* › *File* reaches Glance; `off` hides the source.
    pub upload: ImageUpload,
    /// Largest file *File* accepts, in GiB (1024³ bytes); checked in the dialog and again by
    /// the server, for both modes.
    pub max_upload_gb: u32,
    /// Image property keys the metadata editors hide and the server never sends (Horizon's
    /// `IMAGE_RESERVED_CUSTOM_PROPERTIES`). `os_glance_*` is always reserved.
    pub reserved_properties: Vec<String>,
}

/// `[images] upload`: Horizon's `HORIZON_IMAGES_UPLOAD_MODE` (`off`, `legacy`, `direct`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageUpload {
    /// No *File* source: nothing changes until an operator enables it.
    #[default]
    Off,
    /// The browser sends the file to Moonlight, which streams it to Glance (nothing is stored).
    Proxy,
    /// The browser sends the file straight to Glance's endpoint with a one-off token; needs
    /// CORS on Glance and hands that token to the page (CONFIGURATION.md).
    Direct,
}

impl ImagePrefs {
    /// `max_upload_gb` in bytes.
    pub fn max_upload_bytes(&self) -> u64 {
        u64::from(self.max_upload_gb) << 30
    }
}

impl Default for ImagePrefs {
    fn default() -> Self {
        ImagePrefs {
            disk_formats: [
                "raw", "qcow2", "vdi", "vhd", "vmdk", "iso", "aki", "ami", "ari", "docker", "ploop",
            ]
            .map(String::from)
            .to_vec(),
            default_visibility: "private".into(),
            allow_location: false,
            upload: ImageUpload::Off,
            max_upload_gb: 20,
            reserved_properties: Vec::new(),
        }
    }
}

/// `[console]`: the instance *Log* and *Console* tabs. Both are
/// Nova POSTs that read without changing the server, so they are sent only
/// when this is enabled **and** `[writes] mode = on`; dry-run shows the request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConsolePrefs {
    pub enabled: bool,
    /// Horizon's `CONSOLE_TYPE`.
    #[serde(rename = "type")]
    pub kind: ConsoleType,
    /// Lines shown by the *Log* tab (`INSTANCE_LOG_LENGTH`).
    pub log_length: u32,
}

impl Default for ConsolePrefs {
    fn default() -> Self {
        ConsolePrefs {
            enabled: false,
            kind: ConsoleType::Auto,
            log_length: 35,
        }
    }
}

/// `[console] type`: which remote console *Console* asks Nova for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleType {
    /// VNC, then SPICE if the cloud refuses VNC.
    #[default]
    Auto,
    Vnc,
    Spice,
}

/// Glance image visibilities, in Horizon's order: the choices for `[images] default_visibility`
/// (checked at load) and the visibility select of a volume's *Upload to Image*.
pub const VISIBILITIES: [&str; 4] = ["private", "shared", "community", "public"];

/// An IP network (`10.0.0.0/8`), for the subnet allow-list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cidr {
    pub addr: IpAddr,
    pub len: u8,
}

impl Cidr {
    /// Parses `addr/len`, rejecting a prefix longer than the address (33 for IPv4). The address
    /// need not be the network address: `10.0.0.1/8` is accepted as written.
    pub fn parse(s: &str) -> Result<Cidr, String> {
        let s = s.trim();
        let bad = || format!("not a CIDR: {s}");
        let (a, l) = s.split_once('/').ok_or_else(bad)?;
        let addr: IpAddr = a.parse().map_err(|_| bad())?;
        let len: u8 = l.parse().map_err(|_| bad())?;
        if len > if addr.is_ipv4() { 32 } else { 128 } {
            return Err(bad());
        }
        Ok(Cidr { addr, len })
    }

    /// The address as 128 bits, IPv4 shifted to the top so one mask works for both versions.
    fn bits(addr: IpAddr) -> u128 {
        match addr {
            IpAddr::V4(a) => u128::from(u32::from(a)) << 96,
            IpAddr::V6(a) => u128::from(a),
        }
    }

    /// `other` lies entirely inside `self` (same IP version).
    pub fn contains(&self, other: &Cidr) -> bool {
        if self.addr.is_ipv4() != other.addr.is_ipv4() || other.len < self.len {
            return false;
        }
        // Both versions sit at the top of 128 bits, so the prefix is the top `len` bits.
        let mask = if self.len == 0 {
            0
        } else {
            u128::MAX << (128 - u32::from(self.len))
        };
        Self::bits(self.addr) & mask == Self::bits(other.addr) & mask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_edit_allows_by_setting_and_role() {
        assert!(QuotaEdit::All.allows(false) && QuotaEdit::All.allows(true));
        assert!(!QuotaEdit::Admin.allows(false), "a member, with `admin`");
        assert!(QuotaEdit::Admin.allows(true));
        assert!(!QuotaEdit::Off.allows(true), "`off` hides it from admins too");
    }

    #[test]
    fn missing_values_get_the_defaults() {
        let p: Prefs = serde_json::from_str("{}").unwrap();
        assert_eq!(p, Prefs::default());
        assert_eq!(p.poll_interval_ms, 5000);
        assert!(p.launch.create_volume && !p.launch.config_drive && !p.launch.requires_keypair);
        assert_eq!(p.launch.flavor_sort, FlavorSort::Ram);
        assert!(p.network.enable_ipv6 && p.network.fip_topology_check);
        assert_eq!(p.network.topology_max_networks, 50);
        assert_eq!(p.images.default_visibility, "private");
        assert!(p.images.disk_formats.contains(&"qcow2".to_string()));
        let partial: LaunchPrefs = serde_json::from_str(r#"{"config_drive": true}"#).unwrap();
        assert_eq!(
            partial,
            LaunchPrefs {
                config_drive: true,
                ..Default::default()
            },
            "one key: the rest default"
        );
        assert_eq!(p.poll_rounds(), 120, "10 minutes at 5 s");
    }

    #[test]
    fn launch_sources_follow_the_switches() {
        let l = LaunchPrefs {
            disable_volume: true,
            disable_instance_snapshot: true,
            ..Default::default()
        };
        assert_eq!(l.sources(), ["image", "volume_snapshot"]);
        assert_eq!(LaunchPrefs::default().sources().len(), 4);
    }

    #[test]
    fn cidrs_contain_their_subnets() {
        let c = |s| Cidr::parse(s).unwrap();
        assert!(c("10.0.0.0/8").contains(&c("10.20.0.0/24")));
        assert!(!c("10.0.0.0/8").contains(&c("11.0.0.0/24")));
        assert!(!c("10.0.0.0/24").contains(&c("10.0.0.0/16")), "larger than the range");
        assert!(c("0.0.0.0/0").contains(&c("192.168.1.0/24")));
        assert!(c("fd00::/8").contains(&c("fd12:3456::/64")));
        assert!(!c("fd00::/8").contains(&c("10.0.0.0/24")), "other IP version");
        assert!(Cidr::parse("10.0.0.0/33").is_err() && Cidr::parse("nope").is_err());
    }

    #[test]
    fn subnets_are_checked_against_the_allow_list() {
        let open = NetworkPrefs::default();
        assert!(open.check_subnet("203.0.113.0/24").is_ok(), "empty list: anything");
        let strict = NetworkPrefs {
            allowed_private_subnet_cidr: AllowedCidrs {
                ipv4: vec!["10.0.0.0/8".into(), "192.168.0.0/16".into()],
                ipv6: vec![],
            },
            enable_ipv6: false,
            ..Default::default()
        };
        assert!(strict.check_subnet("192.168.10.0/24").is_ok());
        assert_eq!(
            strict.check_subnet("172.16.0.0/24"),
            Err(SubnetIssue::Outside(vec!["10.0.0.0/8".into(), "192.168.0.0/16".into()]))
        );
        assert_eq!(strict.check_subnet("fd00::/64"), Err(SubnetIssue::Ipv6Disabled));
        assert_eq!(strict.check_subnet("x"), Err(SubnetIssue::NotACidr));
    }
}
