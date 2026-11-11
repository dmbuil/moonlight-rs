//! The Admin dashboard (roadmap 85): system info, hypervisors, providers, flavors, volume types, RBAC.

use super::*;

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
pub(super) fn short_provider_name(host: &str, name: &str) -> String {
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
