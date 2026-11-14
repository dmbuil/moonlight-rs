//! Nova and Glance: instances, images, key pairs, server groups, image metadata.

use super::*;

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
    /// Nova server metadata.
    #[serde(default)]
    pub metadata: Vec<(String, String)>,
    /// Security group **names** of the server (Nova lists one entry per port, so
    /// deduplicated and sorted), for *Edit Security Groups*.
    #[serde(default)]
    pub security_groups: Vec<String>,
    /// Volumes still to read to name the image of a server booted from a volume:
    /// set only by a paged list, which shows the page first and fills
    /// the *Image* cell later (`api::get_boot_images`). Empty when the image
    /// is known or the server has no volume.
    #[serde(default)]
    pub boot_volume_ids: Vec<String>,
}

/// Volume IDs per `api::get_boot_images` call: a paged Instances page sends
/// its rows' `boot_volume_ids` in chunks this size, so each GET's query string stays
/// a few KiB and the *Image* cells fill in chunk by chunk.
pub const BOOT_IMAGE_CHUNK: usize = 50;

/// What `api::get_late_names` looks up: a paged page names the resources its rows
/// point at after the table shows, since neither Nova nor Cinder lists several
/// IDs in one request (probed on ttcp-dev).
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
    /// `None` when Nova did not say (a deleted flavor, not in the list). Swap 0 is `None`.
    pub vcpus: Option<i64>,
    pub ram_mb: Option<i64>,
    pub disk_gb: Option<i64>,
    pub ephemeral_gb: Option<i64>,
    pub swap_mb: Option<i64>,
}

/// Nova's `fault` on an instance in `ERROR`: the HTTP-like code and the message.
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
    /// Nova's `image.id`, else the boot volume's `volume_image_metadata.image_id`.
    pub image_id: Option<String>,
    pub security_groups: Vec<String>,
    /// Rules of each of `security_groups` the project can list.
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
    /// Bytes; `None` until data is uploaded.
    pub size: Option<u64>,
    pub owned_by_project: bool,
    pub owner_id: Option<String>,
    /// Project name of the owner, when Keystone lets us resolve it.
    pub owner_name: Option<String>,
    /// Custom properties: every key that is not a Glance core field.
    #[serde(default)]
    pub properties: Vec<(String, String)>,
    #[serde(default)]
    pub min_disk_gb: Option<i64>,
    #[serde(default)]
    pub min_ram_mb: Option<i64>,
    /// Why the last import failed (`os_glance_failed_import`, Glance sets it on
    /// `web-download` / `glance-direct` errors); `None` when it has not.
    #[serde(default)]
    pub import_failed: Option<String>,
}

impl Image {
    /// A custom property (`description`, `architecture`, `kernel_id`…).
    pub fn property(&self, k: &str) -> Option<&str> {
        self.properties.iter().find(|(p, _)| p == k).map(|(_, v)| v.as_str())
    }
}

/// A row of the Key Pairs list; `key_type` needs compute 2.2.
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

/// The server group detail page.
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

/// *Create Image*: a Glance record plus, when `url` is set, the `web-download` import.
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
    /// created here: the upload itself is not an [`OsAction`] (it is streamed), so
    /// this is what the dry-run preview, the size limit and the mode checks go by. Excludes `url`.
    #[serde(default)]
    pub upload_bytes: Option<u64>,
    /// Admin › Images' *Create Image*: the owning project; `None`
    /// = the session's, as Project's.
    #[serde(default)]
    pub owner: Option<String>,
}

/// Where the browser `PUT`s an image's data in `proxy` mode (axum syntax).
pub const IMAGE_UPLOAD_ROUTE: &str = "/upload/image/{id}";

/// [`IMAGE_UPLOAD_ROUTE`] for image `id`.
pub fn image_upload_path(id: &str) -> String {
    IMAGE_UPLOAD_ROUTE.replace("{id}", id)
}

/// What the browser needs for one direct upload to Glance (`[images] upload = direct`):
/// the URL and a scoped token for that request. Not `Debug`-printable, so it can't end up
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
