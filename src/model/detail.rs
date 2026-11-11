//! Detail views of single resources, and the API access credentials.

use super::*;

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
