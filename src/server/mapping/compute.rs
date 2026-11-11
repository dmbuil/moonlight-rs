//! Nova: instances (rows and detail), key pairs, server groups.

use super::*;

fn addresses(server: &Value) -> Vec<Address> {
    let Some(map) = server.get("addresses").and_then(Value::as_object) else {
        return vec![];
    };
    let mut nets: Vec<_> = map.iter().collect();
    nets.sort_by(|a, b| a.0.cmp(b.0));
    nets.into_iter()
        .flat_map(|(net, list)| {
            list.as_array().into_iter().flatten().filter_map(move |a| {
                Some(Address {
                    network: net.clone(),
                    addr: s(a, "addr")?,
                    floating: s(a, "OS-EXT-IPS:type").as_deref() == Some("floating"),
                    mac: s(a, "OS-EXT-IPS-MAC:mac_addr"),
                })
            })
        })
        .collect()
}

/// Image ID of a server, `None` when booted from volume (Nova sends `""`).
fn image_id(server: &Value) -> Option<String> {
    server.get("image").and_then(|img| s(img, "id"))
}

/// Integer field that some APIs return as `""` (e.g. flavor `swap`).
fn i_lenient(v: &Value, k: &str) -> Option<i64> {
    match v.get(k) {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

fn flavor_info(f: &Value, id: String, name: String) -> FlavorInfo {
    FlavorInfo {
        id,
        name,
        vcpus: i(f, "vcpus"),
        ram_mb: i(f, "ram"),
        disk_gb: i(f, "disk"),
        ephemeral_gb: i_lenient(f, "OS-FLV-EXT-DATA:ephemeral").or_else(|| i_lenient(f, "ephemeral")),
        swap_mb: i_lenient(f, "swap").filter(|s| *s > 0),
    }
}

fn flavor_of(server: &Value, flavors: &HashMap<String, &Value>) -> FlavorInfo {
    let f = server.get("flavor").cloned().unwrap_or(Value::Null);
    // Microversion >= 2.47 embeds the flavor (with `original_name`, no id): look the id up by name.
    if let Some(name) = s(&f, "original_name") {
        let id = flavors
            .iter()
            .find(|(_, fl)| s(fl, "name").as_deref() == Some(name.as_str()))
            .map(|(id, _)| id.clone())
            .unwrap_or_default();
        return flavor_info(&f, id, name);
    }
    let id = s(&f, "id").unwrap_or_default();
    match flavors.get(&id) {
        Some(fl) => flavor_info(fl, id.clone(), name_or_id(fl)),
        None => flavor_info(&Value::Null, id.clone(), id),
    }
}

const ROOT_DEVICES: &[&str] = &["/dev/vda", "/dev/sda", "/dev/xvda", "/dev/hda"];

/// Image a server was booted from. For boot-from-volume (`"image": ""`), the
/// image comes from the boot volume's `volume_image_metadata`.
/// Returns `(name, from_volume)`.
fn boot_image(
    server: &Value,
    images: &HashMap<String, String>,
    volumes: &HashMap<String, &Value>,
) -> Option<(String, bool)> {
    if let Some(id) = image_id(server) {
        return Some((images.get(&id).cloned().unwrap_or(id), false));
    }
    let md = boot_volume(server, volumes)?.get("volume_image_metadata")?;
    let name = s_ne(md, "image_name").or_else(|| s(md, "image_id").map(|id| images.get(&id).cloned().unwrap_or(id)))?;
    Some((name, true))
}

/// The image the server was built from: Nova's `image.id`, else (booted from
/// a volume) the boot volume's `volume_image_metadata.image_id`.
fn boot_image_id(server: &Value, volumes: &HashMap<String, &Value>) -> Option<String> {
    image_id(server).or_else(|| s(boot_volume(server, volumes)?.get("volume_image_metadata")?, "image_id"))
}

fn boot_volume<'v>(server: &Value, volumes: &HashMap<String, &'v Value>) -> Option<&'v Value> {
    let server_id = s(server, "id")?;
    let root = s(server, "OS-EXT-SRV-ATTR:root_device_name");
    let attached: Vec<&Value> = arr(server, "os-extended-volumes:volumes_attached")
        .iter()
        .filter_map(|a| s(a, "id"))
        .filter_map(|id| volumes.get(&id).copied())
        .collect();
    let device_of = |v: &Value| {
        arr(v, "attachments")
            .iter()
            .find(|a| s(a, "server_id").as_deref() == Some(server_id.as_str()))
            .and_then(|a| s(a, "device"))
    };
    // 1) the device Nova reports as root, 2) a conventional root device, 3) any bootable volume
    let boot = attached
        .iter()
        .find(|v| root.is_some() && device_of(v) == root)
        .or_else(|| {
            attached
                .iter()
                .find(|v| device_of(v).is_some_and(|d| ROOT_DEVICES.contains(&d.as_str())))
        })
        .or_else(|| attached.iter().find(|v| b(v, "bootable")))?;
    Some(*boot)
}

pub(super) fn instance(
    server: &Value,
    flavors: &HashMap<String, &Value>,
    images: &HashMap<String, String>,
    volumes: &HashMap<String, &Value>,
) -> Instance {
    let flavor = flavor_of(server, flavors);
    let image = boot_image(server, images, volumes);
    Instance {
        id: s(server, "id").unwrap_or_default(),
        name: name_or_id(server),
        image_from_volume: image.as_ref().is_some_and(|(_, v)| *v),
        image_name: image.map(|(n, _)| n),
        addresses: addresses(server),
        flavor_name: flavor.name.clone(),
        flavor,
        locked: b(server, "locked"),
        key_name: s_ne(server, "key_name"),
        status: s(server, "status").unwrap_or_default(),
        availability_zone: s_ne(server, "OS-EXT-AZ:availability_zone"),
        task_state: s_ne(server, "OS-EXT-STS:task_state"),
        power_state: i(server, "OS-EXT-STS:power_state").unwrap_or(0),
        created: s(server, "created").unwrap_or_default(),
        metadata: kv_pairs(server.get("metadata")),
        security_groups: server_security_groups(server),
        boot_volume_ids: Vec::new(),
    }
}

/// The server's security group names. Nova repeats a group once per port that
/// has it, so the list is deduplicated (and sorted for stable display).
fn server_security_groups(server: &Value) -> Vec<String> {
    let mut groups: Vec<String> = arr(server, "security_groups")
        .iter()
        .filter_map(|g| s(g, "name"))
        .collect();
    groups.sort();
    groups.dedup();
    groups
}

/// Joins servers with flavors, images and (for boot-from-volume) volumes. Newest first.
pub fn instances(servers: &[Value], flavors: &[Value], images: &[Value], volumes: &[Value]) -> Vec<Instance> {
    let flavors = index_by_id(flavors);
    let images = names_by_id(images);
    let volumes = index_by_id(volumes);
    let mut out: Vec<_> = servers
        .iter()
        .map(|sv| instance(sv, &flavors, &images, &volumes))
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

/// [`instances`] for one page of a paged list (roadmap 88), in the order Nova
/// answered: the marker of the next page is the last row, so re-sorting here would
/// break the paging. Nova embeds the flavor (2.47), so no flavor list is needed.
/// Boot-from-volume rows get no image here but their attached volumes in
/// `boot_volume_ids`, read afterwards ([`boot_images`]): on a cloud that boots from
/// volume, reading them first cost one GET per row and ≈10 s a page on ttcp-dev.
pub fn instances_page(servers: &[Value], images: &[Value]) -> Vec<Instance> {
    let images = names_by_id(images);
    servers
        .iter()
        .map(|sv| {
            let mut row = instance(sv, &HashMap::new(), &images, &HashMap::new());
            if image_id(sv).is_none() {
                row.boot_volume_ids = attached_volume_ids(sv);
            }
            row
        })
        .collect()
}

/// Image IDs the servers were built from, each once: what a page asks Glance for
/// (`images?id=in:…`) instead of every image of the cloud (roadmap 88).
pub fn image_ids(servers: &[Value]) -> Vec<String> {
    let mut ids: Vec<String> = servers.iter().filter_map(image_id).collect();
    ids.sort();
    ids.dedup();
    ids
}

/// IDs of the volumes attached to a server (Nova's `volumes_attached`), sorted.
fn attached_volume_ids(server: &Value) -> Vec<String> {
    let mut ids: Vec<String> = arr(server, "os-extended-volumes:volumes_attached")
        .iter()
        .filter_map(|a| s(a, "id"))
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// The image of each server booted from these volumes, as `(server ID, image
/// name)`, from the volumes alone (a paged list's second call, roadmap 88). Each
/// server is rebuilt from the volumes' `attachments`, so [`boot_volume`] picks the
/// same volume as for a full list, except that the server's own root device name is
/// unknown here: a conventional root device or a bootable volume decides. A volume
/// without image metadata names nothing; the image ID stands for a missing name.
pub fn boot_images(volumes: &[Value]) -> Vec<(String, String)> {
    let by_id = index_by_id(volumes);
    let mut servers: Vec<String> = volumes
        .iter()
        .flat_map(|v| arr(v, "attachments").iter().filter_map(|a| s(a, "server_id")))
        .collect();
    servers.sort();
    servers.dedup();
    servers
        .into_iter()
        .filter_map(|id| {
            let attached: Vec<Value> = volumes
                .iter()
                .filter(|v| {
                    arr(v, "attachments")
                        .iter()
                        .any(|a| s(a, "server_id").as_deref() == Some(id.as_str()))
                })
                .filter_map(|v| Some(serde_json::json!({ "id": s(v, "id")? })))
                .collect();
            let server = serde_json::json!({ "id": id, "os-extended-volumes:volumes_attached": attached });
            let (name, _) = boot_image(&server, &HashMap::new(), &by_id)?;
            Some((id, name))
        })
        .collect()
}

/// Everything `instance_detail` needs, fetched in parallel by `fetch::instance`.
pub struct DetailInputs<'a> {
    pub server: &'a Value,
    pub flavors: &'a [Value],
    pub images: &'a [Value],
    pub volumes: &'a [Value],
    pub networks: &'a [Value],
    pub interfaces: &'a [Value],
    /// The server's Neutron ports (`ports?device_id=`), for name, admin state,
    /// VNIC type, port security and groups of each interface.
    pub ports: &'a [Value],
    pub actions: &'a [Value],
    /// The project's security groups, for the rules of the server's ones.
    pub security_groups: &'a [Value],
    /// The project's server groups, to find the one holding the server.
    pub server_groups: &'a [Value],
}

/// The instance detail page from the server and its lookups.
pub fn instance_detail(d: DetailInputs<'_>) -> InstanceDetail {
    let flavors = index_by_id(d.flavors);
    let images = names_by_id(d.images);
    let volumes = index_by_id(d.volumes);
    let networks = names_by_id(d.networks);
    let sv = d.server;

    let security_groups = server_security_groups(sv);

    let attached = arr(sv, "os-extended-volumes:volumes_attached")
        .iter()
        .filter_map(|a| s(a, "id"))
        .map(|id| {
            let vol = volumes.get(&id);
            let device = vol.and_then(|v| {
                arr(v, "attachments")
                    .iter()
                    .find(|a| s(a, "server_id") == s(sv, "id"))
                    .and_then(|a| s(a, "device"))
            });
            AttachedVolume {
                name: vol.and_then(|v| s_ne(v, "name")),
                size_gb: vol.and_then(|v| i(v, "size")),
                device,
                id,
            }
        })
        .collect();

    let mut metadata: Vec<(String, String)> = sv
        .get("metadata")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    metadata.sort();

    let fault = sv.get("fault").map(|f| Fault {
        code: i(f, "code").unwrap_or(0),
        message: s(f, "message").unwrap_or_default(),
    });

    let ports = index_by_id(d.ports);
    let interfaces = d
        .interfaces
        .iter()
        .map(|it| {
            let network_id = s(it, "net_id").unwrap_or_default();
            let port_id = s(it, "port_id").unwrap_or_default();
            Interface {
                port: ports.get(&port_id).map(|p| port_settings(p)),
                port_id,
                network_name: networks.get(&network_id).cloned(),
                network_id,
                fixed_ips: arr(it, "fixed_ips").iter().filter_map(|f| s(f, "ip_address")).collect(),
                mac: s(it, "mac_addr").unwrap_or_default(),
                state: s(it, "port_state").unwrap_or_default(),
            }
        })
        .collect();

    let mut actions: Vec<InstanceAction> = d
        .actions
        .iter()
        .map(|a| InstanceAction {
            request_id: s(a, "request_id").unwrap_or_default(),
            action: s(a, "action").unwrap_or_default(),
            start_time: s(a, "start_time").unwrap_or_default(),
            user_id: s_ne(a, "user_id"),
            message: s_ne(a, "message"),
        })
        .collect();
    actions.sort_by(|a, b| b.start_time.cmp(&a.start_time));

    InstanceDetail {
        base: instance(sv, &flavors, &images, &volumes),
        description: s_ne(sv, "description"),
        project_id: s(sv, "tenant_id").unwrap_or_default(),
        host: s_ne(sv, "OS-EXT-SRV-ATTR:host"),
        locked: b(sv, "locked"),
        updated: s(sv, "updated"),
        flavor: flavor_of(sv, &flavors),
        image_id: boot_image_id(sv, &volumes),
        security_group_rules: group_rules(&security_groups, d.security_groups),
        security_groups,
        volumes: attached,
        metadata,
        fault,
        interfaces,
        actions,
        server_group: s(sv, "id").and_then(|id| group_of(d.server_groups, &id)),
    }
}

/// Key pairs, unwrapped from Nova's `{"keypair": {…}}` and sorted by name.
pub fn keypairs(raw: &[Value]) -> Vec<KeyPair> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|k| unwrap(k, "keypair"))
        .filter_map(|k| {
            Some(KeyPair {
                name: s(k, "name")?,
                fingerprint: s(k, "fingerprint").unwrap_or_default(),
                key_type: s_ne(k, "type"),
            })
        })
        .collect();
    out.sort_by_key(|k| k.name.to_lowercase());
    out
}

/// Server groups, sorted by name; the 2.64 `policy` joins the older `policies`.
pub fn server_groups(raw: &[Value]) -> Vec<ServerGroup> {
    let mut out: Vec<_> = raw.iter().map(server_group).collect();
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

pub(super) fn server_group(g: &Value) -> ServerGroup {
    let mut policies = strs(g, "policies");
    // microversion >= 2.64 uses a single `policy`
    if let Some(p) = s(g, "policy") {
        policies.push(p);
    }
    ServerGroup {
        id: s(g, "id").unwrap_or_default(),
        name: name_or_id(g),
        policies,
        members: strs(g, "members"),
    }
}

/// The server group detail page: the group (2.64 `policy` + `rules`, or the
/// older `policies`), its members named and with status from `servers`.
pub fn server_group_detail(g: &Value, servers: &[Value]) -> ServerGroupDetail {
    let g = unwrap(g, "server_group");
    let by_id = index_by_id(servers);
    let mut rules: Vec<(String, String)> = g
        .get("rules")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    rules.sort();
    let group = server_group(g);
    let members = group
        .members
        .iter()
        .map(|id| {
            let sv = by_id.get(id);
            GroupMember {
                id: id.clone(),
                name: sv.and_then(|v| s_ne(v, "name")),
                status: sv.and_then(|v| s_ne(v, "status")),
            }
        })
        .collect();
    ServerGroupDetail {
        rules,
        project_id: s_ne(g, "project_id"),
        user_id: s_ne(g, "user_id"),
        members,
        group,
    }
}

/// The key pair detail page, unwrapped from `{"keypair": {…}}`.
pub fn keypair_detail(k: &Value) -> KeyPairDetail {
    let k = unwrap(k, "keypair");
    KeyPairDetail {
        name: s(k, "name").unwrap_or_default(),
        key_type: s_ne(k, "type"),
        fingerprint: s(k, "fingerprint").unwrap_or_default(),
        public_key: s(k, "public_key").unwrap_or_default(),
        created: s(k, "created_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::*;
    use super::*;
    use serde_json::json;

    #[test]
    fn joins_instance_with_flavor_image_and_addresses() {
        let out = instances(&[server_fixture()], &flavors(), &images_fixture(), &[]);
        let vm = &out[0];
        assert_eq!(vm.name, "web-prod-01");
        assert_eq!(vm.flavor_name, "m1.large");
        assert_eq!(vm.image_name.as_deref(), Some("rhel-9.4"));
        assert_eq!(vm.power_state, 1);
        assert_eq!(vm.task_state, None);
        // networks sorted by name, floating flag kept
        let ips: Vec<_> = vm.addresses.iter().map(|a| (a.addr.as_str(), a.floating)).collect();
        assert_eq!(
            ips,
            vec![("10.0.12.21", false), ("203.0.113.41", true), ("10.9.0.5", false)]
        );
    }

    #[test]
    fn boot_from_volume_and_unknown_references_degrade_gracefully() {
        let mut sv = server_fixture();
        sv["image"] = json!("");
        sv["flavor"] = json!({"id": "deleted-flavor"});
        sv["name"] = json!("");
        let vm = &instances(&[sv], &flavors(), &images_fixture(), &[])[0];
        assert_eq!(vm.image_name, None);
        assert_eq!(vm.flavor_name, "deleted-flavor");
        assert_eq!(
            vm.name, "3f2a9c1e-0000-0000-0000-000000000c91",
            "unnamed falls back to id"
        );

        let mut sv = server_fixture();
        sv["image"] = json!({"id": "private-img"});
        let vm = &instances(&[sv], &[], &[], &[])[0];
        assert_eq!(vm.image_name.as_deref(), Some("private-img"));
    }

    #[test]
    fn embedded_flavor_from_newer_microversions_is_used() {
        let mut sv = server_fixture();
        sv["flavor"] = json!({"original_name": "r1.2xlarge", "vcpus": 8, "ram": 65536, "disk": 80});
        let vm = &instances(&[sv], &[], &[], &[])[0];
        assert_eq!(vm.flavor_name, "r1.2xlarge");
    }

    #[test]
    fn instances_are_sorted_newest_first() {
        let mut old = server_fixture();
        old["created"] = json!("2025-01-01T00:00:00Z");
        old["id"] = json!("old");
        let out = instances(&[old, server_fixture()], &flavors(), &[], &[]);
        assert_eq!(out[1].id, "old");
    }

    #[test]
    fn keypairs_are_unwrapped() {
        let raw = vec![
            json!({"keypair": {"name": "b-key", "fingerprint": "aa:bb", "type": "ssh"}}),
            json!({"name": "A-key", "fingerprint": "cc:dd"}),
        ];
        let k = keypairs(&raw);
        assert_eq!(k[0].name, "A-key");
        assert_eq!(k[1].key_type.as_deref(), Some("ssh"));
        // compute 2.2 lists: the launch dialog shows the real type, `ssh` only when absent.
        let raw = vec![
            json!({"keypair": {"name": "cert", "fingerprint": "ee", "type": "x509"}}),
            json!({"keypair": {"name": "old", "fingerprint": "ff"}}),
        ];
        let types: Vec<_> = keypair_choices(&raw).into_iter().map(|c| c.cells[1].clone()).collect();
        assert_eq!(types, ["x509", "ssh"]);
    }

    #[test]
    fn server_group_policy_formats() {
        let g = server_groups(&[
            json!({"id": "1", "name": "old", "policies": ["anti-affinity"], "members": ["a", "b"]}),
            json!({"id": "2", "name": "new", "policy": "soft-affinity", "members": []}),
        ]);
        assert_eq!(g[0].name, "new");
        assert_eq!(g[0].policies, vec!["soft-affinity"]);
        assert_eq!(g[1].members.len(), 2);
    }

    #[test]
    fn server_group_detail_names_members_and_reads_rules() {
        let servers = vec![
            json!({"id": "a", "name": "web-1", "status": "ACTIVE"}),
            json!({"id": "b", "name": "", "status": "SHUTOFF"}),
        ];
        // compute 2.64: one `policy` and its `rules` (numbers), wrapped as the GET returns it.
        let d = server_group_detail(
            &json!({"server_group": {"id": "g1", "name": "spread", "policy": "anti-affinity",
                "rules": {"max_server_per_host": 2}, "members": ["a", "b", "gone"],
                "project_id": "p1", "user_id": "u1"}}),
            &servers,
        );
        assert_eq!(d.group.policies, vec!["anti-affinity"]);
        assert_eq!(
            d.rules,
            vec![("max_server_per_host".into(), "2".into())],
            "numbers shown as text"
        );
        assert_eq!(
            (d.project_id.as_deref(), d.user_id.as_deref()),
            (Some("p1"), Some("u1"))
        );
        assert_eq!(
            d.members[0],
            GroupMember {
                id: "a".into(),
                name: Some("web-1".into()),
                status: Some("ACTIVE".into())
            }
        );
        assert_eq!(
            d.members[1].name, None,
            "an empty name falls back to the ID in the page"
        );
        assert_eq!(
            (d.members[2].name.clone(), d.members[2].status.clone()),
            (None, None),
            "a member missing from the list keeps only its ID"
        );
        // Before 2.64: `policies`, no `rules`; empty `project_id` means unknown.
        let d = server_group_detail(
            &json!({"id": "g2", "name": "", "policies": ["affinity"], "members": [], "project_id": ""}),
            &servers,
        );
        assert_eq!(
            (d.group.name.as_str(), d.group.policies.clone()),
            ("g2", vec!["affinity".to_string()])
        );
        assert!(d.rules.is_empty() && d.members.is_empty() && d.project_id.is_none());
    }

    #[test]
    fn embedded_flavor_keeps_details_and_finds_the_id_by_name() {
        let mut sv = server_fixture();
        sv["locked"] = json!(true);
        sv["flavor"] = json!({"original_name": "m1.large", "vcpus": 4, "ram": 8192, "disk": 40,
            "ephemeral": 10, "swap": 0, "extra_specs": {"hw:cpu_policy": "dedicated"}});
        let vm = &instances(&[sv], &flavors(), &[], &[])[0];
        assert!(vm.locked);
        assert_eq!(vm.flavor.id, "4");
        assert_eq!((vm.flavor.ephemeral_gb, vm.flavor.swap_mb), (Some(10), None));
        // flavor list entries carry swap as "" and ephemeral under an extension key
        let listed = vec![
            json!({"id": "4", "name": "m1.large", "vcpus": 4, "ram": 8192, "disk": 40, "swap": "", "OS-FLV-EXT-DATA:ephemeral": 0}),
        ];
        let vm = &instances(&[server_fixture()], &listed, &[], &[])[0];
        assert_eq!(
            (vm.flavor.swap_mb, vm.flavor.ephemeral_gb, vm.flavor.vcpus),
            (None, Some(0), Some(4))
        );
        assert!(!vm.locked);
    }

    #[test]
    fn floating_ip_detail_lists_forwarding_rules_with_their_instance() {
        let fip = json!({"id": "f1", "floating_ip_address": "203.0.113.7", "floating_network_id": "pub", "status": "DOWN",
                         "port_id": null, "router_id": "r1", "created_at": "2026-10-01T10:00:00Z"});
        let rules = vec![
            json!({"id": "pf2", "protocol": "udp", "internal_ip_address": "10.0.0.6", "internal_port_id": "p2",
                   "internal_port_range": "5000:5010", "external_port_range": "6000:6010", "description": ""}),
            json!({"id": "pf1", "protocol": "tcp", "internal_ip_address": "10.0.0.5", "internal_port_id": "p1",
                   "internal_port": 22, "external_port": 2222, "description": "ssh"}),
        ];
        let ports = vec![json!({"id": "p1", "device_id": "s1"})];
        let d = floating_ip_detail(
            &fip,
            &rules,
            &[json!({"id": "pub", "name": "public"})],
            &[json!({"id": "s1", "name": "web"})],
            &ports,
        );
        assert_eq!(
            (
                d.row.ip.as_str(),
                d.row.pool.as_str(),
                d.port_id.clone(),
                d.router_id.as_deref()
            ),
            ("203.0.113.7", "public", None, Some("r1"))
        );
        let r = &d.forwardings;
        assert_eq!(
            (r[0].protocol.as_str(), r[0].external.as_str(), r[0].internal.as_str()),
            ("tcp", "2222", "22")
        );
        assert_eq!(r[0].instance.as_ref().map(|i| i.name.as_str()), Some("web"));
        assert_eq!(
            (
                r[1].external.as_str(),
                r[1].internal.as_str(),
                r[1].instance.clone(),
                r[1].description.clone()
            ),
            ("6000:6010", "5000:5010", None, None)
        );
    }

    #[test]
    fn a_page_asks_only_for_the_images_and_boot_volumes_it_shows() {
        let servers = vec![
            json!({"id": "a", "image": {"id": "img1"}, "os-extended-volumes:volumes_attached": [{"id": "data1"}]}),
            json!({"id": "b", "image": {"id": "img1"}}),
            json!({"id": "c", "image": "", "os-extended-volumes:volumes_attached": [{"id": "vol2"}, {"id": "vol1"}]}),
            json!({"id": "d", "image": "", "os-extended-volumes:volumes_attached": [{"id": "vol1"}]}),
            json!({"id": "e"}),
        ];
        assert_eq!(
            image_ids(&servers),
            ["img1"],
            "each image once; boot-from-volume servers have none"
        );
        let rows = instances_page(&servers, &[]);
        let pending: Vec<(&str, Vec<String>)> = rows
            .iter()
            .map(|i| (i.id.as_str(), i.boot_volume_ids.clone()))
            .collect();
        assert_eq!(
            pending,
            [
                ("a", vec![]),
                ("b", vec![]),
                ("c", vec!["vol1".to_string(), "vol2".to_string()]),
                ("d", vec!["vol1".to_string()]),
                ("e", vec![])
            ],
            "only boot-from-volume servers wait for their volumes; a data volume of an image-booted server is not read"
        );
        assert!(
            rows[2].image_name.is_none() && rows[3].image_name.is_none(),
            "no volume is read for the page itself: the image comes later"
        );
        assert!(image_ids(&[]).is_empty());
    }
}
