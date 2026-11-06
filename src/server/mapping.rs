//! Pure functions: raw OpenStack JSON → UI models. No I/O, so every join is
//! unit-tested below with payloads shaped like the real APIs.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::model::*;

// --- small JSON helpers -----------------------------------------------------

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}
fn s_ne(v: &Value, k: &str) -> Option<String> {
    s(v, k).filter(|x| !x.is_empty())
}
fn b(v: &Value, k: &str) -> bool {
    match v.get(k) {
        Some(Value::Bool(x)) => *x,
        // Cinder returns "true"/"false" strings for `bootable`
        Some(Value::String(x)) => x.eq_ignore_ascii_case("true"),
        _ => false,
    }
}
fn i(v: &Value, k: &str) -> Option<i64> {
    v.get(k).and_then(Value::as_i64)
}
fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}
fn strs(v: &Value, k: &str) -> Vec<String> {
    arr(v, k).iter().filter_map(|x| x.as_str().map(str::to_string)).collect()
}
/// `name` if set, else the ID — what Horizon shows for unnamed resources.
fn name_or_id(v: &Value) -> String {
    s_ne(v, "name").or_else(|| s(v, "id")).unwrap_or_default()
}
fn index_by_id(items: &[Value]) -> HashMap<String, &Value> {
    items.iter().filter_map(|x| Some((s(x, "id")?, x))).collect()
}
/// These IDs sorted, each once, without blanks: the lookups a page asks for.
fn distinct(ids: impl Iterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = ids.filter(|id| !id.is_empty()).collect();
    out.sort();
    out.dedup();
    out
}

fn names_by_id(items: &[Value]) -> HashMap<String, String> {
    items.iter().filter_map(|x| Some((s(x, "id")?, name_or_id(x)))).collect()
}
/// Some list endpoints wrap each item (`{"keypair": {...}}`).
fn unwrap<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(v)
}

// --- Identity -----------------------------------------------------------------

/// Keystone `auth/projects` → the projects the user can switch to, sorted by name.
pub fn projects(raw: &[Value]) -> Vec<ProjectRef> {
    let mut out: Vec<_> = raw
        .iter()
        .filter(|p| p.get("enabled").and_then(Value::as_bool).unwrap_or(true))
        .filter_map(|p| Some(ProjectRef { id: s(p, "id")?, name: name_or_id(p) }))
        .collect();
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

// --- Compute ------------------------------------------------------------------

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
    let name = s_ne(md, "image_name")
        .or_else(|| s(md, "image_id").map(|id| images.get(&id).cloned().unwrap_or(id)))?;
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
        .or_else(|| attached.iter().find(|v| device_of(v).is_some_and(|d| ROOT_DEVICES.contains(&d.as_str()))))
        .or_else(|| attached.iter().find(|v| b(v, "bootable")))?;
    Some(*boot)
}

fn instance(
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
    let mut groups: Vec<String> = arr(server, "security_groups").iter().filter_map(|g| s(g, "name")).collect();
    groups.sort();
    groups.dedup();
    groups
}

/// Joins servers with flavors, images and (for boot-from-volume) volumes. Newest first.
pub fn instances(servers: &[Value], flavors: &[Value], images: &[Value], volumes: &[Value]) -> Vec<Instance> {
    let flavors = index_by_id(flavors);
    let images = names_by_id(images);
    let volumes = index_by_id(volumes);
    let mut out: Vec<_> = servers.iter().map(|sv| instance(sv, &flavors, &images, &volumes)).collect();
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
    let mut ids: Vec<String> = arr(server, "os-extended-volumes:volumes_attached").iter().filter_map(|a| s(a, "id")).collect();
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
    let mut servers: Vec<String> = volumes.iter().flat_map(|v| arr(v, "attachments").iter().filter_map(|a| s(a, "server_id"))).collect();
    servers.sort();
    servers.dedup();
    servers
        .into_iter()
        .filter_map(|id| {
            let attached: Vec<Value> = volumes
                .iter()
                .filter(|v| arr(v, "attachments").iter().any(|a| s(a, "server_id").as_deref() == Some(id.as_str())))
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
                .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())))
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

fn server_group(g: &Value) -> ServerGroup {
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
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))).collect())
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

// --- Identity (roadmap 57) -----------------------------------------------------

/// Keystone's `enabled` is optional and means true when absent.
fn enabled(v: &Value) -> bool {
    v.get("enabled").and_then(Value::as_bool).unwrap_or(true)
}

/// `domain_id` → name when `domains` has it, else the ID.
fn domain_of(v: &Value, domains: &HashMap<String, String>) -> String {
    let id = s(v, "domain_id").unwrap_or_default();
    domains.get(&id).cloned().unwrap_or(id)
}

/// Identity › Projects, by name. `mine`: the user's projects (they can switch to them).
pub fn identity_projects(raw: &[Value], domains: &HashMap<String, String>, current: &str, mine: &[String]) -> Vec<IdentityProject> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|p| {
            let id = s(p, "id")?;
            Some(IdentityProject {
                name: name_or_id(p),
                description: s_ne(p, "description"),
                domain: domain_of(p, domains),
                enabled: enabled(p),
                current: id == current,
                member: mine.contains(&id),
                id,
            })
        })
        .collect();
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

/// Identity › Users, by name.
pub fn identity_users(raw: &[Value], domains: &HashMap<String, String>) -> Vec<IdentityUser> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|u| unwrap(u, "user"))
        .filter_map(|u| {
            Some(IdentityUser {
                id: s(u, "id")?,
                name: name_or_id(u),
                description: s_ne(u, "description"),
                email: s_ne(u, "email"),
                enabled: enabled(u),
                domain: domain_of(u, domains),
            })
        })
        .collect();
    out.sort_by_key(|u| u.name.to_lowercase());
    out
}

/// A project's members from its role assignments (`include_names`): users and
/// groups with their roles, and each group's users (`group_users`, by group ID)
/// with the roles they get through it, like Horizon's *Users* tab.
pub fn project_members(assignments: &[Value], group_users: &[(String, Vec<Value>)]) -> (Vec<ProjectMember>, Vec<ProjectGroup>) {
    let mut users: Vec<ProjectMember> = Vec::new();
    let mut groups: Vec<ProjectGroup> = Vec::new();
    let role_of = |a: &Value| a.get("role").map(|r| s_ne(r, "name").or_else(|| s(r, "id")).unwrap_or_default()).unwrap_or_default();
    let role_id = |a: &Value| a.get("role").and_then(|r| s(r, "id")).unwrap_or_default();
    for a in assignments {
        let (role, rid) = (role_of(a), role_id(a));
        if let Some(u) = a.get("user") {
            let id = s(u, "id").unwrap_or_default();
            match users.iter_mut().find(|m| m.id == id) {
                Some(m) => {
                    m.roles.push(role);
                    m.role_ids.push(rid);
                }
                None => users.push(ProjectMember { name: name_or_id(u), id, roles: vec![role], role_ids: vec![rid], group_roles: vec![] }),
            }
        } else if let Some(g) = a.get("group") {
            let id = s(g, "id").unwrap_or_default();
            match groups.iter_mut().find(|x| x.id == id) {
                Some(x) => {
                    x.roles.push(role);
                    x.role_ids.push(rid);
                }
                None => groups.push(ProjectGroup { name: name_or_id(g), id, roles: vec![role], role_ids: vec![rid] }),
            }
        }
    }
    for (group_id, members) in group_users {
        let Some(g) = groups.iter().find(|g| &g.id == group_id).cloned() else { continue };
        for u in members {
            let id = s(u, "id").unwrap_or_default();
            let from_group = g.roles.iter().map(|r| (r.clone(), g.name.clone()));
            match users.iter_mut().find(|m| m.id == id) {
                Some(m) => m.group_roles.extend(from_group),
                None => users.push(ProjectMember { name: name_or_id(u), id, roles: vec![], role_ids: vec![], group_roles: from_group.collect() }),
            }
        }
    }
    for m in &mut users {
        m.roles.sort();
        m.roles.dedup();
        m.role_ids.sort();
        m.role_ids.dedup();
        m.group_roles.sort();
        m.group_roles.dedup();
    }
    for g in &mut groups {
        g.roles.sort();
        g.roles.dedup();
        g.role_ids.sort();
        g.role_ids.dedup();
    }
    users.sort_by_key(|m| m.name.to_lowercase());
    groups.sort_by_key(|g| g.name.to_lowercase());
    (users, groups)
}

/// A user's detail; `projects` names the primary project when known.
pub fn identity_user_detail(u: &Value, domains: &HashMap<String, String>, projects: &HashMap<String, String>) -> IdentityUserDetail {
    let u = unwrap(u, "user");
    // Keystone's own attributes; anything else is shown as extra, like Horizon.
    const KNOWN: [&str; 11] =
        ["id", "name", "description", "email", "enabled", "domain_id", "default_project_id", "password_expires_at", "options", "links", "federated"];
    let mut extra: Vec<(String, String)> = u
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, v)| !KNOWN.contains(&k.as_str()) && !v.is_null())
                .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())))
                .collect()
        })
        .unwrap_or_default();
    extra.sort();
    IdentityUserDetail {
        user: identity_users(std::slice::from_ref(u), domains).pop().unwrap_or(IdentityUser {
            id: String::new(),
            name: String::new(),
            description: None,
            email: None,
            enabled: true,
            domain: String::new(),
        }),
        domain_id: s_ne(u, "domain_id"),
        password_expires_at: s_ne(u, "password_expires_at"),
        lock_password: u.get("options").map(|o| b(o, "lock_password")).unwrap_or(false),
        primary_project: s_ne(u, "default_project_id").map(|id| named(id, projects)),
        primary_project_id: s_ne(u, "default_project_id"),
        extra,
        assignments: None,
        groups: None,
    }
}

/// Identity › Groups, by name.
pub fn identity_groups(raw: &[Value], domains: &HashMap<String, String>) -> Vec<IdentityGroup> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|g| Some(IdentityGroup { id: s(g, "id")?, name: name_or_id(g), description: s_ne(g, "description"), domain: domain_of(g, domains) }))
        .collect();
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

/// Identity › Roles, by name, with the roles each implies (`inferences`:
/// Keystone's `role_inferences`, `{prior_role, implies: [...]}`).
pub fn identity_roles(raw: &[Value], inferences: &[Value]) -> Vec<IdentityRole> {
    let implied = |id: &str| -> Vec<String> {
        let mut v: Vec<String> = inferences
            .iter()
            .filter(|i| i.get("prior_role").and_then(|p| s(p, "id")).as_deref() == Some(id))
            .flat_map(|i| arr(i, "implies").iter().map(name_or_id).collect::<Vec<_>>())
            .collect();
        v.sort();
        v
    };
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|r| {
            let id = s(r, "id")?;
            Some(IdentityRole { implies: implied(&id), name: name_or_id(r), id })
        })
        .collect();
    out.sort_by_key(|r| r.name.to_lowercase());
    out
}

/// A user's role assignments from `role_assignments?user.id&include_names`.
pub fn role_assignment_rows(raw: &[Value]) -> Vec<RoleAssignmentRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|a| {
            let role = a.get("role").map(|r| s_ne(r, "name").or_else(|| s(r, "id")).unwrap_or_default()).unwrap_or_default();
            let scope = a.get("scope");
            let inherited = scope.is_some_and(|sc| sc.get("OS-INHERIT:inherited_to").is_some());
            let (scope_kind, name, project_id) = match scope {
                Some(sc) if sc.get("project").is_some() => ("project", name_or_id(&sc["project"]), s(&sc["project"], "id")),
                Some(sc) if sc.get("domain").is_some() => ("domain", name_or_id(&sc["domain"]), None),
                Some(sc) if sc.get("system").is_some() => ("system", "all".to_string(), None),
                _ => ("", String::new(), None),
            };
            RoleAssignmentRow { role, scope_kind: scope_kind.into(), scope: name, project_id, inherited }
        })
        .collect();
    out.sort_by(|a, b| (a.scope_kind.as_str(), a.scope.to_lowercase(), a.role.as_str()).cmp(&(b.scope_kind.as_str(), b.scope.to_lowercase(), b.role.as_str())));
    out
}

/// A group's overview and its members.
pub fn identity_group_detail(g: &Value, users: &[Value], domains: &HashMap<String, String>) -> IdentityGroupDetail {
    let g = unwrap(g, "group");
    IdentityGroupDetail {
        group: identity_groups(std::slice::from_ref(g), domains).pop().unwrap_or(IdentityGroup { id: String::new(), name: String::new(), description: None, domain: String::new() }),
        domain_id: s_ne(g, "domain_id"),
        users: identity_users(users, domains),
    }
}

/// One application credential, with its access rules.
pub fn app_credential_detail(v: &Value) -> AppCredentialDetail {
    let v = unwrap(v, "application_credential");
    AppCredentialDetail {
        credential: app_credentials(std::slice::from_ref(v)).pop().unwrap_or(AppCredential {
            id: String::new(),
            name: String::new(),
            project_id: None,
            description: None,
            expires_at: None,
            roles: vec![],
            unrestricted: false,
        }),
        access_rules: arr(v, "access_rules")
            .iter()
            .map(|r| AccessRule { service: s(r, "service").unwrap_or_default(), method: s(r, "method").unwrap_or_default(), path: s(r, "path").unwrap_or_default() })
            .collect(),
    }
}

/// Identity › Application Credentials, by name; role names (else IDs).
pub fn app_credentials(raw: &[Value]) -> Vec<AppCredential> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|a| {
            Some(AppCredential {
                id: s(a, "id")?,
                name: name_or_id(a),
                project_id: s_ne(a, "project_id"),
                description: s_ne(a, "description"),
                expires_at: s_ne(a, "expires_at"),
                roles: arr(a, "roles").iter().filter_map(|r| s_ne(r, "name").or_else(|| s(r, "id"))).collect(),
                unrestricted: a.get("unrestricted").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Identity › Domains, by name; `context` marks the session's domain context.
pub fn identity_domains(raw: &[Value], context: Option<&str>) -> Vec<IdentityDomain> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|d| {
            let id = s(d, "id")?;
            Some(IdentityDomain { name: name_or_id(d), description: s_ne(d, "description"), enabled: enabled(d), context: context == Some(id.as_str()), id })
        })
        .collect();
    out.sort_by_key(|d| d.name.to_lowercase());
    out
}

/// Domain ID → name, from a domain list (admins) plus the user's own domain.
pub fn domain_names(raw: &[Value], own: Option<(String, String)>) -> HashMap<String, String> {
    let mut m = names_by_id(raw);
    if let Some((id, name)) = own {
        m.entry(id).or_insert(name);
    }
    m
}

/// The group (`(id, name)`) whose members include `server_id`, if any.
pub fn group_of(groups: &[Value], server_id: &str) -> Option<(String, String)> {
    groups
        .iter()
        .find(|g| strs(g, "members").iter().any(|m| m == server_id))
        .map(|g| (s(g, "id").unwrap_or_default(), name_or_id(g)))
}

// --- Image ----------------------------------------------------------------------

/// Images with visibility and owner (`project_names` for owners other than
/// `project_id`): the project's own first, then by name.
pub fn images(raw: &[Value], project_id: &str, project_names: &HashMap<String, String>) -> Vec<Image> {
    let mut out = images_page(raw, project_id, project_names);
    // Project images first, then alphabetical (Horizon groups them by tab)
    out.sort_by(|a, b| {
        b.owned_by_project
            .cmp(&a.owned_by_project)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

/// [`images`] in Glance's order, for a paged list (roadmap 88): the next page's
/// marker is the last row.
pub fn images_page(raw: &[Value], project_id: &str, project_names: &HashMap<String, String>) -> Vec<Image> {
    raw
        .iter()
        .map(|im| Image {
            id: s(im, "id").unwrap_or_default(),
            name: name_or_id(im),
            is_snapshot: s(im, "image_type").as_deref() == Some("snapshot"),
            status: s(im, "status").unwrap_or_default(),
            visibility: s(im, "visibility").unwrap_or_default(),
            protected: b(im, "protected"),
            disk_format: s_ne(im, "disk_format"),
            size: im.get("size").and_then(Value::as_u64),
            owned_by_project: s(im, "owner").as_deref() == Some(project_id),
            owner_name: s(im, "owner").and_then(|o| project_names.get(&o).cloned()),
            owner_id: s_ne(im, "owner"),
            properties: image_properties(im),
            min_disk_gb: i(im, "min_disk"),
            min_ram_mb: i(im, "min_ram"),
            import_failed: s_ne(im, "os_glance_failed_import"),
        })
        .collect()
}

// --- Block storage --------------------------------------------------------------

/// Volumes with their attachments' server names.
pub fn volumes(raw: &[Value], servers: &[Value]) -> Vec<Volume> {
    let mut out = volumes_page(raw, servers);
    out.sort_by_key(|v| v.name.to_lowercase());
    out
}

/// [`volumes`] in Cinder's order, for a paged list (roadmap 88): the next page's
/// marker is the last row. `servers` may be empty: the attachments then carry only
/// the server IDs, named later (`api::get_late_names`).
pub fn volumes_page(raw: &[Value], servers: &[Value]) -> Vec<Volume> {
    let server_names = names_by_id(servers);
    raw
        .iter()
        .map(|v| Volume {
            id: s(v, "id").unwrap_or_default(),
            name: name_or_id(v),
            description: s_ne(v, "description"),
            size_gb: i(v, "size").unwrap_or(0),
            status: s(v, "status").unwrap_or_default(),
            volume_type: s_ne(v, "volume_type"),
            attachments: arr(v, "attachments")
                .iter()
                .filter_map(|a| {
                    let server_id = s(a, "server_id")?;
                    Some(VolumeAttachment {
                        server_name: server_names.get(&server_id).cloned(),
                        server_id,
                        device: s_ne(a, "device"),
                    })
                })
                .collect(),
            availability_zone: s_ne(v, "availability_zone"),
            bootable: b(v, "bootable"),
            encrypted: b(v, "encrypted"),
            metadata: kv_pairs(v.get("metadata")),
        })
        .collect()
}

/// Volume snapshots with their volume's name.
pub fn snapshots(raw: &[Value], volumes: &[Value]) -> Vec<Snapshot> {
    let mut out = snapshots_page(raw, volumes);
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

/// [`snapshots`] in Cinder's order, for a paged list (roadmap 88). `volumes` may be
/// empty: the volume is then named later (`api::get_late_names`).
pub fn snapshots_page(raw: &[Value], volumes: &[Value]) -> Vec<Snapshot> {
    let vol_names: HashMap<String, String> = volumes
        .iter()
        .filter_map(|v| Some((s(v, "id")?, s_ne(v, "name")?)))
        .collect();
    raw
        .iter()
        .map(|sn| {
            let volume_id = s(sn, "volume_id").unwrap_or_default();
            Snapshot {
                id: s(sn, "id").unwrap_or_default(),
                name: name_or_id(sn),
                description: s_ne(sn, "description"),
                size_gb: i(sn, "size").unwrap_or(0),
                status: s(sn, "status").unwrap_or_default(),
                volume_name: vol_names.get(&volume_id).cloned(),
                volume_id,
                created: s(sn, "created_at").unwrap_or_default(),
                metadata: kv_pairs(sn.get("metadata")),
            }
        })
        .collect()
}

/// Volumes › Backups (roadmap 5), newest first, with the names of their volumes
/// (`volumes` may be empty: the ID is shown then). `is_incremental` and
/// `fail_reason` are absent on older Cinders; a backup of a deleted volume
/// keeps its `volume_id`, so the link may point nowhere.
pub fn backups(raw: &[Value], volumes: &[Value]) -> Vec<BackupRow> {
    let names = names_by_id(volumes);
    let mut out: Vec<_> = raw
        .iter()
        .map(|bk| BackupRow {
            id: s(bk, "id").unwrap_or_default(),
            name: name_or_id(bk),
            description: s_ne(bk, "description"),
            status: s(bk, "status").unwrap_or_default(),
            size_gb: i(bk, "size").unwrap_or(0),
            volume: named_ref(s(bk, "volume_id").unwrap_or_default(), &names),
            incremental: b(bk, "is_incremental"),
            container: s_ne(bk, "container"),
            snapshot_id: s_ne(bk, "snapshot_id"),
            availability_zone: s_ne(bk, "availability_zone"),
            fail_reason: s_ne(bk, "fail_reason"),
            created: s(bk, "created_at").unwrap_or_default(),
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

/// A `Ref` to `id`, named from `names` (the ID when unknown, like Horizon).
fn named_ref(id: String, names: &HashMap<String, String>) -> Ref {
    Ref { name: names.get(&id).cloned().unwrap_or_else(|| id.clone()), id }
}

/// One generic volume group (roadmap 5). `group_snapshots` only tells whether
/// it has any (Horizon's *Has Snapshots*).
fn volume_group(g: &Value, group_types: &HashMap<String, String>, volume_types: &HashMap<String, String>, group_snapshots: &[Value]) -> VolumeGroupRow {
    let id = s(g, "id").unwrap_or_default();
    VolumeGroupRow {
        name: name_or_id(g),
        description: s_ne(g, "description"),
        status: s(g, "status").unwrap_or_default(),
        availability_zone: s_ne(g, "availability_zone"),
        group_type: named_ref(s(g, "group_type").unwrap_or_default(), group_types),
        volume_types: strs(g, "volume_types").into_iter().map(|t| named_ref(t, volume_types)).collect(),
        has_snapshots: group_snapshots.iter().any(|x| s(x, "group_id").as_deref() == Some(id.as_str())),
        created: s(g, "created_at").unwrap_or_default(),
        id,
    }
}

/// Volumes › Groups, sorted by name. Type names come from the group type and
/// volume type lists; either may be empty (IDs shown instead).
pub fn volume_groups(raw: &[Value], group_types: &[Value], volume_types: &[Value], group_snapshots: &[Value]) -> Vec<VolumeGroupRow> {
    let mut out = volume_groups_page(raw, group_types, volume_types, group_snapshots);
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

/// [`volume_groups`] in Cinder's order, for a paged list (roadmap 88).
pub fn volume_groups_page(raw: &[Value], group_types: &[Value], volume_types: &[Value], group_snapshots: &[Value]) -> Vec<VolumeGroupRow> {
    let (gt, vt) = (names_by_id(group_types), names_by_id(volume_types));
    raw.iter().map(|g| volume_group(g, &gt, &vt, group_snapshots)).collect()
}

/// The volume group detail: `volumes` and `group_snapshots` may hold the whole
/// project's; only those with this `group_id` are kept. Volumes carry
/// `group_id` from volume API 3.13, which `fetch` asks for.
pub fn volume_group_detail(g: &Value, group_types: &[Value], volume_types: &[Value], group_snapshots: &[Value], volumes_raw: &[Value]) -> VolumeGroupDetail {
    let row = volume_group(g, &names_by_id(group_types), &names_by_id(volume_types), group_snapshots);
    let in_group = |x: &&Value| s(x, "group_id").as_deref() == Some(row.id.as_str());
    let mine: Vec<Value> = volumes_raw.iter().filter(in_group).cloned().collect();
    let snaps: Vec<Value> = group_snapshots.iter().filter(in_group).cloned().collect();
    let groups = [g.clone()];
    VolumeGroupDetail { volumes: volumes(&mine, &[]), snapshots: group_snapshots_rows(&snaps, &groups, group_types), row }
}

/// Volumes › Group Snapshots, newest first, with their group's name.
pub fn group_snapshots_rows(raw: &[Value], groups: &[Value], group_types: &[Value]) -> Vec<GroupSnapshotRow> {
    let mut out = group_snapshots_page(raw, groups, group_types);
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

/// [`group_snapshots_rows`] in Cinder's order, for a paged list (roadmap 88);
/// `groups` need only be the page's ([`group_ids`]).
pub fn group_snapshots_page(raw: &[Value], groups: &[Value], group_types: &[Value]) -> Vec<GroupSnapshotRow> {
    let (gn, gt) = (names_by_id(groups), names_by_id(group_types));
    raw
        .iter()
        .map(|x| GroupSnapshotRow {
            id: s(x, "id").unwrap_or_default(),
            name: name_or_id(x),
            description: s_ne(x, "description"),
            status: s(x, "status").unwrap_or_default(),
            group: named_ref(s(x, "group_id").unwrap_or_default(), &gn),
            group_type: named_ref(s(x, "group_type_id").unwrap_or_default(), &gt),
            created: s(x, "created_at").unwrap_or_default(),
        })
        .collect()
}

/// The groups a page of group snapshots belongs to, each once (roadmap 88).
pub fn group_ids(raw: &[Value]) -> Vec<String> {
    distinct(raw.iter().filter_map(|x| s(x, "group_id")))
}

/// *Manage Volumes* choices: per group, its own volumes and those Cinder lets
/// it take (like Horizon's form: no group yet, one of the group's volume types,
/// `available` or `in-use`), each with the group's ID as first cell. Groups list
/// volume types by ID while volumes name theirs (`volume_type_id` only from
/// 3.61), so both are matched. Volumes need `group_id` (volume 3.13).
pub fn group_volume_choices(groups: &[Value], volumes_raw: &[Value], volume_types: &[Value]) -> Vec<Choice> {
    let type_names = names_by_id(volume_types);
    let mut out = Vec::new();
    for g in groups {
        let Some(gid) = s(g, "id") else { continue };
        let ids = strs(g, "volume_types");
        let accepted: Vec<&str> = ids.iter().map(String::as_str).chain(ids.iter().filter_map(|t| type_names.get(t).map(String::as_str))).collect();
        let fits = |v: &Value| {
            s(v, "group_id").is_none()
                && matches!(s(v, "status").as_deref(), Some("available" | "in-use"))
                && [s(v, "volume_type"), s(v, "volume_type_id")].iter().flatten().any(|t| accepted.contains(&t.as_str()))
        };
        let mine = volumes_raw.iter().filter(|v| s(v, "group_id").as_deref() == Some(gid.as_str()) || fits(v));
        out.extend(sorted(
            mine.filter_map(|v| Some(Choice { value: s(v, "id")?, label: label_volume(v), cells: vec![gid.clone()], ..Default::default() })).collect(),
        ));
    }
    out
}

/// The group snapshot detail: the volume snapshots taken with it (those with
/// its `group_snapshot_id`, volume API 3.14), named after their volumes.
pub fn group_snapshot_detail(x: &Value, groups: &[Value], group_types: &[Value], snapshots_raw: &[Value], volumes_raw: &[Value]) -> GroupSnapshotDetail {
    let row = group_snapshots_rows(std::slice::from_ref(x), groups, group_types).remove(0);
    let mine: Vec<Value> = snapshots_raw.iter().filter(|sn| s(sn, "group_snapshot_id").as_deref() == Some(row.id.as_str())).cloned().collect();
    GroupSnapshotDetail { snapshots: snapshots(&mine, volumes_raw), row }
}

// --- Network --------------------------------------------------------------------

/// Network rows with their subnets (name and CIDR) and whether the project owns them,
/// sorted by name.
pub fn networks(raw: &[Value], subnets: &[Value], project_id: &str) -> Vec<NetworkRow> {
    let mut out = networks_page(raw, subnets, project_id);
    out.sort_by_key(|n| n.name.to_lowercase());
    out
}

/// [`networks`] in Neutron's order, for a paged list (roadmap 88); `subnets`
/// need only be the page's (`subnets?network_id=…`).
pub fn networks_page(raw: &[Value], subnets: &[Value], project_id: &str) -> Vec<NetworkRow> {
    let subnet_label: HashMap<String, String> = subnets
        .iter()
        .filter_map(|sn| {
            let cidr = s(sn, "cidr").unwrap_or_default();
            let label = match s_ne(sn, "name") {
                Some(n) => format!("{n} {cidr}"),
                None => cidr,
            };
            Some((s(sn, "id")?, label))
        })
        .collect();
    raw
        .iter()
        .map(|n| NetworkRow {
            id: s(n, "id").unwrap_or_default(),
            name: name_or_id(n),
            subnets: strs(n, "subnets")
                .into_iter()
                .map(|id| subnet_label.get(&id).cloned().unwrap_or(id))
                .collect(),
            shared: b(n, "shared"),
            external: b(n, "router:external"),
            status: s(n, "status").unwrap_or_default(),
            admin_up: b(n, "admin_state_up"),
            availability_zones: strs(n, "availability_zones"),
            owned: s(n, "project_id").or_else(|| s(n, "tenant_id")).as_deref() == Some(project_id),
            dhcp_agents: None,
            provider: provider_network(n),
        })
        .collect()
}

/// The IDs of a page's rows, each once: the networks whose subnets a page reads.
pub fn row_ids(raw: &[Value]) -> Vec<String> {
    distinct(raw.iter().filter_map(|x| s(x, "id")))
}

/// A network's `provider:*` attributes (admins see them); `None` without a
/// type. The segmentation ID comes as a number, or `null` for flat/local.
fn provider_network(n: &Value) -> Option<ProviderNetwork> {
    Some(ProviderNetwork {
        network_type: s_ne(n, "provider:network_type")?,
        physical_network: s_ne(n, "provider:physical_network"),
        segmentation_id: n.get("provider:segmentation_id").and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok())).and_then(|v| u32::try_from(v).ok()),
    })
}

/// Every project's Neutron objects (networks, QoS policies) as choices labelled
/// `name (project)`, by label: an admin sees the same names in many projects.
pub fn owned_choices(raw: &[Value], names: &HashMap<String, String>) -> Vec<Choice> {
    let mut out: Vec<Choice> = raw
        .iter()
        .filter_map(|v| {
            let id = s(v, "id")?;
            let owner = s_ne(v, "project_id").or_else(|| s_ne(v, "tenant_id")).map(|p| names.get(&p).cloned().unwrap_or(p));
            let name = name_or_id(v);
            Some(Choice::new(id, match owner {
                Some(p) => format!("{name} ({p})"),
                None => name,
            }))
        })
        .collect();
    out.sort_by_key(|c| c.label.to_lowercase());
    out
}

/// Neutron `availability_zones` (one entry per zone and resource) → the
/// names of `resource`'s available zones (`network`, `router`), once each,
/// sorted. Filters here too, as not every Neutron accepts the query's filters.
pub fn zone_choices(raw: &[Value], resource: &str) -> Vec<Choice> {
    let mut names: Vec<String> = raw
        .iter()
        .filter(|z| s(z, "resource").is_none_or(|r| r == resource))
        .filter(|z| s(z, "state").is_none_or(|st| st == "available"))
        .filter_map(|z| s_ne(z, "name"))
        .collect();
    names.sort();
    names.dedup();
    names.into_iter().map(|n| Choice::new(&n, &n)).collect()
}

/// Glance `metadefs/resource_types` (`{"resource_types": [{"name"}]}`) → the
/// names, sorted (*Update Associations*).
pub fn metadef_resource_type_choices(v: &Value) -> Vec<Choice> {
    let mut names: Vec<String> = arr(v, "resource_types").iter().filter_map(|t| s_ne(t, "name")).collect();
    names.sort();
    names.into_iter().map(|n| Choice::new(&n, &n)).collect()
}

/// Cinder `scheduler-stats/get_pools` → the pool names (`host@backend#pool`),
/// sorted: *Migrate Volume*'s destinations (Horizon lists them the same way).
pub fn pool_choices(raw: &[Value]) -> Vec<Choice> {
    let mut names: Vec<String> = raw.iter().filter_map(|p| s_ne(p, "name")).collect();
    names.sort();
    names.dedup();
    names.into_iter().map(|n| Choice::new(&n, &n)).collect()
}

/// `nova-compute` services → their hosts, once each, sorted. `usable`: only
/// enabled and up ones, the hosts an admin can migrate to (Horizon's *New
/// Host*); else all (*Manage Hosts* of an aggregate).
pub fn compute_host_choices(raw: &[Value], usable: bool) -> Vec<Choice> {
    let mut hosts: Vec<String> =
        host_services(raw).into_iter().filter(|s| !s.host.is_empty() && (!usable || (s.enabled && s.up))).map(|s| s.host).collect();
    hosts.sort();
    hosts.dedup();
    hosts.into_iter().map(|h| Choice::new(&h, &h)).collect()
}

/// DHCP agents per network ID, from each DHCP agent's `dhcp-networks`
/// (`{"networks": [...]}` per agent): Horizon's *DHCP Agents* column, without
/// its one call per network.
pub fn dhcp_agent_counts(per_agent: &[Value]) -> HashMap<String, usize> {
    let mut out = HashMap::new();
    for agent in per_agent {
        for n in arr(agent, "networks") {
            if let Some(id) = s(n, "id") {
                *out.entry(id).or_insert(0) += 1;
            }
        }
    }
    out
}

/// Router rows with their external network's name.
pub fn routers(raw: &[Value], networks: &[Value]) -> Vec<RouterRow> {
    let mut out = routers_page(raw, networks);
    out.sort_by_key(|r| r.name.to_lowercase());
    out
}

/// The external networks of a page of routers, each once (roadmap 88).
pub fn gateway_ids(raw: &[Value]) -> Vec<String> {
    distinct(raw.iter().filter_map(|r| r.get("external_gateway_info").filter(|g| !g.is_null()).and_then(|g| s(g, "network_id"))))
}

/// [`routers`] in Neutron's order, for a paged list (roadmap 88); `networks`
/// need only be the page's gateways ([`gateway_ids`]).
pub fn routers_page(raw: &[Value], networks: &[Value]) -> Vec<RouterRow> {
    let net_names = names_by_id(networks);
    raw
        .iter()
        .map(|r| {
            let gw = r.get("external_gateway_info").filter(|g| !g.is_null());
            let ext_id = gw.and_then(|g| s(g, "network_id"));
            RouterRow {
                id: s(r, "id").unwrap_or_default(),
                name: name_or_id(r),
                status: s(r, "status").unwrap_or_default(),
                external_network: ext_id.as_ref().map(|id| net_names.get(id).cloned().unwrap_or_else(|| id.clone())),
                external_network_id: ext_id,
                snat: gw.and_then(|g| g.get("enable_snat")).and_then(Value::as_bool),
                admin_up: b(r, "admin_state_up"),
                availability_zones: strs(r, "availability_zones"),
            }
        })
        .collect()
}

/// Everything `topology` needs, fetched in parallel by `fetch::topology`.
pub struct TopologyInputs<'a> {
    /// The session's project, to tell its own networks from shared ones.
    pub project_id: &'a str,
    pub networks: &'a [Value],
    pub subnets: &'a [Value],
    pub routers: &'a [Value],
    pub ports: &'a [Value],
    pub servers: &'a [Value],
    pub floating_ips: &'a [Value],
}

fn fixed_ips(v: &Value, key: &str) -> Vec<String> {
    arr(v, key).iter().filter_map(|ip| s(ip, "ip_address")).collect()
}

/// Networks, routers and instances, joined by their ports. Router gateways come
/// from `external_gateway_info`: their port belongs to no project, so a list
/// filtered by `tenant_id` does not return it. Ports of other devices (DHCP,
/// load balancers) and ports with no device are left out.
pub fn topology(d: TopologyInputs<'_>) -> Topology {
    let mut networks: Vec<TopoNetwork> = d
        .networks
        .iter()
        .filter_map(|n| {
            let id = s(n, "id")?;
            let cidrs = d
                .subnets
                .iter()
                .filter(|sn| s(sn, "network_id").as_deref() == Some(id.as_str()))
                .filter_map(|sn| s(sn, "cidr"))
                .collect();
            Some(TopoNetwork {
                name: name_or_id(n),
                external: b(n, "router:external"),
                shared: b(n, "shared"),
                owned: s(n, "project_id").or_else(|| s(n, "tenant_id")).as_deref() == Some(d.project_id),
                status: s(n, "status").unwrap_or_default(),
                cidrs,
                id,
            })
        })
        .collect();
    networks.sort_by(|a, b| b.external.cmp(&a.external).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));

    let device = |v: &Value, kind| {
        Some(TopoDevice { id: s(v, "id")?, name: name_or_id(v), kind, status: s(v, "status").unwrap_or_default() })
    };
    let mut devices: Vec<TopoDevice> = d.routers.iter().filter_map(|r| device(r, TopoKind::Router)).collect();
    let mut servers: Vec<TopoDevice> = d.servers.iter().filter_map(|v| device(v, TopoKind::Server)).collect();
    devices.sort_by_key(|x| x.name.to_lowercase());
    servers.sort_by_key(|x| x.name.to_lowercase());
    devices.extend(servers);

    let known_net: std::collections::HashSet<&str> = networks.iter().map(|n| n.id.as_str()).collect();
    let known_dev: HashMap<&str, TopoKind> = devices.iter().map(|x| (x.id.as_str(), x.kind)).collect();
    let floating: HashMap<String, Vec<String>> = d.floating_ips.iter().fold(HashMap::new(), |mut m, f| {
        if let (Some(port), Some(ip)) = (s_ne(f, "port_id"), s(f, "floating_ip_address")) {
            m.entry(port).or_default().push(ip);
        }
        m
    });

    let mut links: Vec<TopoLink> = d
        .routers
        .iter()
        .filter_map(|r| {
            let g = r.get("external_gateway_info").filter(|g| g.is_object())?;
            Some(TopoLink {
                device_id: s(r, "id")?,
                network_id: s(g, "network_id")?,
                port_id: None,
                ips: fixed_ips(g, "external_fixed_ips"),
                floating: vec![],
                gateway: true,
            })
        })
        .collect();
    for p in d.ports {
        let owner = s(p, "device_owner").unwrap_or_default();
        let (Some(dev), Some(net), Some(id)) = (s_ne(p, "device_id"), s(p, "network_id"), s(p, "id")) else {
            continue;
        };
        let wanted = match known_dev.get(dev.as_str()) {
            Some(TopoKind::Router) => owner.starts_with("network:router_interface")
                || owner == "network:ha_router_replicated_interface"
                || owner == "network:router_gateway",
            Some(TopoKind::Server) => owner.starts_with("compute:"),
            None => false,
        };
        if !wanted {
            continue;
        }
        let gateway = owner == "network:router_gateway";
        // The gateway seen through the router already: keep one, with its port.
        if let Some(l) = links.iter_mut().find(|l| l.gateway && gateway && l.device_id == dev && l.network_id == net) {
            l.port_id = Some(id);
            continue;
        }
        links.push(TopoLink {
            floating: floating.get(&id).cloned().unwrap_or_default(),
            device_id: dev,
            network_id: net,
            port_id: Some(id),
            ips: fixed_ips(p, "fixed_ips"),
            gateway,
        });
    }
    // No bar to draw a link to a network the project cannot list.
    links.retain(|l| known_net.contains(l.network_id.as_str()));
    links.sort_by(|a, b| (&a.device_id, &a.network_id, &a.ips).cmp(&(&b.device_id, &b.network_id, &b.ips)));
    Topology { networks, devices, links }
}

/// The rules of each named group, remote groups shown by name. Groups the
/// project can't list (or ambiguous names) get no rules.
fn group_rules(names: &[String], groups: &[Value]) -> Vec<(String, Vec<String>)> {
    let by_id = names_by_id(groups);
    names
        .iter()
        .filter_map(|n| {
            let mut same = groups.iter().filter(|g| s(g, "name").as_deref() == Some(n.as_str()));
            let g = same.next().filter(|_| same.next().is_none())?;
            Some((n.clone(), arr(g, "security_group_rules").iter().map(|r| describe_rule_with(r, &by_id)).collect()))
        })
        .collect()
}

/// One-line, Horizon-like summary of a security group rule.
pub fn describe_rule(r: &Value) -> String {
    describe_rule_with(r, &HashMap::new())
}

/// [`describe_rule`], naming remote groups found in `group_names` (by ID).
fn describe_rule_with(r: &Value, group_names: &HashMap<String, String>) -> String {
    let dir = s(r, "direction").unwrap_or_default();
    let eth = s(r, "ethertype").unwrap_or_default();
    let proto = s(r, "protocol").unwrap_or_else(|| "any".into());
    let ports = match (i(r, "port_range_min"), i(r, "port_range_max")) {
        (Some(a), Some(b)) if a == b => format!(" {a}"),
        (Some(a), Some(b)) => format!(" {a}-{b}"),
        _ => String::new(),
    };
    let peer = s(r, "remote_ip_prefix")
        .or_else(|| {
            s(r, "remote_group_id").map(|g| match group_names.get(&g) {
                Some(n) => format!("group {n}"),
                None => format!("group {}", &g[..g.len().min(8)]),
            })
        })
        .unwrap_or_else(|| "any".into());
    let arrow = if dir == "egress" { "→" } else { "←" };
    format!("{dir} {eth} {proto}{ports} {arrow} {peer}")
}

/// Security group rows, sorted by name.
pub fn security_groups(raw: &[Value]) -> Vec<SecurityGroupRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|g| SecurityGroupRow {
            id: s(g, "id").unwrap_or_default(),
            name: name_or_id(g),
            description: s_ne(g, "description"),
            rules: arr(g, "security_group_rules").iter().map(describe_rule).collect(),
            shared: b(g, "shared"),
        })
        .collect();
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

/// Floating IP rows with their pool (network) and associated server names.
pub fn floating_ips(raw: &[Value], networks: &[Value], servers: &[Value]) -> Vec<FloatingIpRow> {
    let mut out = floating_ips_page(raw, networks, servers);
    out.sort_by(|a, b| a.ip.cmp(&b.ip));
    out
}

/// [`floating_ips`] in Neutron's order, for a paged list (roadmap 88). `servers`
/// may be empty: instances are then named later (`api::get_late_names`).
pub fn floating_ips_page(raw: &[Value], networks: &[Value], servers: &[Value]) -> Vec<FloatingIpRow> {
    let net_names = names_by_id(networks);
    let server_names = names_by_id(servers);
    raw
        .iter()
        .map(|f| {
            let details = f.get("port_details").cloned().unwrap_or(Value::Null);
            let instance_id = s(&details, "device_id")
                .filter(|_| s(&details, "device_owner").is_some_and(|o| o.starts_with("compute:")));
            let pool_id = s(f, "floating_network_id").unwrap_or_default();
            FloatingIpRow {
                id: s(f, "id").unwrap_or_default(),
                ip: s(f, "floating_ip_address").unwrap_or_default(),
                fixed_ip: s_ne(f, "fixed_ip_address"),
                instance_name: instance_id.as_ref().and_then(|id| server_names.get(id).cloned()),
                instance_id,
                pool: net_names.get(&pool_id).cloned().unwrap_or(pool_id),
                status: s(f, "status").unwrap_or_default(),
                description: s_ne(f, "description"),
                dns_name: s_ne(f, "dns_name"),
                dns_domain: s_ne(f, "dns_domain"),
            }
        })
        .collect()
}

/// The pools (external networks) a page of floating IPs names, each once: one
/// `networks?id=…&id=…` request (Neutron takes a repeated filter as *any of*).
pub fn pool_ids(raw: &[Value]) -> Vec<String> {
    distinct(raw.iter().filter_map(|f| s(f, "floating_network_id")))
}

/// A server's or volume's name for `api::get_late_names` (roadmap 88), from its
/// `GET servers/{id}` or `volumes/{id}` body (wrapped or not): unnamed → its ID;
/// a volume also gives its host.
pub fn late_name(v: &Value) -> Option<LateName> {
    let v = v.get("server").or_else(|| v.get("volume")).unwrap_or(v);
    Some(LateName { id: s(v, "id")?, name: name_or_id(v), host: s_ne(v, "os-vol-host-attr:host") })
}


// --- Detail views -------------------------------------------------------------

fn kv_pairs(v: Option<&Value>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = v
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// One subnet, with its network's name.
pub fn subnet(sn: &Value, net_names: &HashMap<String, String>) -> SubnetRow {
    let network_id = s(sn, "network_id").unwrap_or_default();
    SubnetRow {
        id: s(sn, "id").unwrap_or_default(),
        name: name_or_id(sn),
        network_name: net_names.get(&network_id).cloned(),
        network_id,
        cidr: s(sn, "cidr").unwrap_or_default(),
        ip_version: i(sn, "ip_version").unwrap_or(4),
        gateway_ip: s_ne(sn, "gateway_ip"),
        enable_dhcp: b(sn, "enable_dhcp"),
        allocation_pools: arr(sn, "allocation_pools")
            .iter()
            .map(|p| format!("{} – {}", s(p, "start").unwrap_or_default(), s(p, "end").unwrap_or_default()))
            .collect(),
        dns_nameservers: strs(sn, "dns_nameservers"),
        host_routes: arr(sn, "host_routes")
            .iter()
            .map(|r| format!("{} → {}", s(r, "destination").unwrap_or_default(), s(r, "nexthop").unwrap_or_default()))
            .collect(),
        description: s_ne(sn, "description"),
    }
}

/// Subnets of `networks`, sorted by name.
pub fn subnets(raw: &[Value], networks: &[Value]) -> Vec<SubnetRow> {
    let names = names_by_id(networks);
    let mut out: Vec<_> = raw.iter().map(|sn| subnet(sn, &names)).collect();
    out.sort_by_key(|x| x.name.to_lowercase());
    out
}

/// What *Edit Port* edits. `port_security_enabled` is missing without the
/// `port-security` extension: the port then filters traffic, as if `true`.
pub fn port_settings(p: &Value) -> PortSettings {
    PortSettings {
        name: s(p, "name").unwrap_or_default(),
        admin_up: b(p, "admin_state_up"),
        vnic_type: s_ne(p, "binding:vnic_type"),
        port_security: p.get("port_security_enabled").and_then(Value::as_bool).unwrap_or(true),
        security_group_ids: strs(p, "security_groups"),
    }
}

/// One port with its names resolved (network, subnets, server, groups).
pub fn port(p: &Value, net_names: &HashMap<String, String>, subnet_names: &HashMap<String, String>, server_names: &HashMap<String, String>, sg_names: &HashMap<String, String>) -> PortRow {
    let network_id = s(p, "network_id").unwrap_or_default();
    let device_id = s_ne(p, "device_id");
    let device_owner = s_ne(p, "device_owner");
    let fixed = arr(p, "fixed_ips");
    PortRow {
        id: s(p, "id").unwrap_or_default(),
        name: s(p, "name").unwrap_or_default(),
        network_name: net_names.get(&network_id).cloned(),
        network_id,
        fixed_ips: fixed
            .iter()
            .map(|f| {
                let ip = s(f, "ip_address").unwrap_or_default();
                match s(f, "subnet_id").and_then(|id| subnet_names.get(&id).cloned()) {
                    Some(n) => format!("{ip} ({n})"),
                    None => ip,
                }
            })
            .collect(),
        subnet_ids: fixed.iter().filter_map(|f| s(f, "subnet_id")).collect(),
        mac: s(p, "mac_address").unwrap_or_default(),
        status: s(p, "status").unwrap_or_default(),
        admin_up: b(p, "admin_state_up"),
        device_name: device_id
            .as_ref()
            .filter(|_| device_owner.as_deref().is_some_and(|o| o.starts_with("compute:")))
            .and_then(|id| server_names.get(id).cloned()),
        device_owner,
        device_id,
        security_groups: strs(p, "security_groups")
            .into_iter()
            .map(|id| sg_names.get(&id).cloned().unwrap_or(id))
            .collect(),
        security_group_ids: strs(p, "security_groups"),
        port_security: p.get("port_security_enabled").and_then(Value::as_bool).unwrap_or(true),
        vnic_type: s_ne(p, "binding:vnic_type"),
        description: s_ne(p, "description"),
    }
}

/// ID → name lookups shared by every port of a page.
pub struct PortLookups {
    pub networks: HashMap<String, String>,
    pub subnets: HashMap<String, String>,
    pub servers: HashMap<String, String>,
    pub security_groups: HashMap<String, String>,
}

impl PortLookups {
    /// Builds the lookups from the raw lists.
    pub fn new(networks: &[Value], subnets: &[Value], servers: &[Value], sgs: &[Value]) -> Self {
        Self {
            networks: names_by_id(networks),
            subnets: names_by_id(subnets),
            servers: names_by_id(servers),
            security_groups: names_by_id(sgs),
        }
    }
    /// One port, names resolved.
    pub fn port(&self, p: &Value) -> PortRow {
        port(p, &self.networks, &self.subnets, &self.servers, &self.security_groups)
    }
    /// Ports, names resolved, sorted by address.
    pub fn ports(&self, raw: &[Value]) -> Vec<PortRow> {
        let mut out: Vec<_> = raw.iter().map(|p| self.port(p)).collect();
        out.sort_by(|a, b| a.fixed_ips.cmp(&b.fixed_ips));
        out
    }
}

/// The network detail page; `ports` already mapped.
pub fn network_detail(net: &Value, subnets_raw: &[Value], ports: Vec<PortRow>, project_id: &str) -> NetworkDetail {
    let row = networks(std::slice::from_ref(net), subnets_raw, project_id).remove(0);
    let names = HashMap::from([(row.id.clone(), row.name.clone())]);
    NetworkDetail {
        description: s_ne(net, "description"),
        project_id: s(net, "project_id").or_else(|| s(net, "tenant_id")).unwrap_or_default(),
        mtu: i(net, "mtu"),
        port_security: b(net, "port_security_enabled"),
        created: s(net, "created_at"),
        subnets: subnets_raw
            .iter()
            .filter(|sn| s(sn, "network_id").as_deref() == Some(row.id.as_str()))
            .map(|sn| subnet(sn, &names))
            .collect(),
        ports,
        row,
    }
}

/// The router detail page; `interfaces` are its ports, already mapped.
pub fn router_detail(r: &Value, networks_raw: &[Value], interfaces: Vec<PortRow>) -> RouterDetail {
    let row = routers(std::slice::from_ref(r), networks_raw).remove(0);
    let gw = r.get("external_gateway_info").cloned().unwrap_or(Value::Null);
    RouterDetail {
        description: s_ne(r, "description"),
        project_id: s(r, "project_id").or_else(|| s(r, "tenant_id")).unwrap_or_default(),
        gateway_ips: arr(&gw, "external_fixed_ips")
            .iter()
            .filter_map(|f| Some((s(f, "ip_address")?, s(f, "subnet_id").unwrap_or_default())))
            .collect(),
        interfaces,
        routes: arr(r, "routes")
            .iter()
            .map(|x| (s(x, "destination").unwrap_or_default(), s(x, "nexthop").unwrap_or_default()))
            .collect(),
        row,
    }
}

/// One security group rule; remote groups shown by name.
pub fn rule_row(r: &Value, sg_names: &HashMap<String, String>) -> RuleRow {
    let port_range = match (i(r, "port_range_min"), i(r, "port_range_max")) {
        (Some(a), Some(z)) if a == z => a.to_string(),
        (Some(a), Some(z)) => format!("{a} - {z}"),
        _ => "Any".into(),
    };
    let remote = s(r, "remote_ip_prefix")
        .or_else(|| s(r, "remote_group_id").map(|g| sg_names.get(&g).cloned().unwrap_or(g)))
        .unwrap_or_else(|| "-".into());
    RuleRow {
        id: s(r, "id").unwrap_or_default(),
        direction: s(r, "direction").unwrap_or_default(),
        ethertype: s(r, "ethertype").unwrap_or_default(),
        protocol: s(r, "protocol").unwrap_or_else(|| "Any".into()),
        port_range,
        remote,
        description: s_ne(r, "description"),
    }
}

/// The security group detail page; `all_groups` names remote groups.
pub fn security_group_detail(g: &Value, all_groups: &[Value]) -> SecurityGroupDetail {
    let names = names_by_id(all_groups);
    let mut rules: Vec<RuleRow> = arr(g, "security_group_rules").iter().map(|r| rule_row(r, &names)).collect();
    rules.sort_by(|a, b| (&a.direction, &a.ethertype, &a.protocol).cmp(&(&b.direction, &b.ethertype, &b.protocol)));
    SecurityGroupDetail {
        id: s(g, "id").unwrap_or_default(),
        name: name_or_id(g),
        description: s_ne(g, "description"),
        project_id: s(g, "project_id").or_else(|| s(g, "tenant_id")).unwrap_or_default(),
        rules,
    }
}

/// Cinder user messages about one volume, newest first.
pub fn volume_messages(msgs: &[Value], volume_id: &str) -> Vec<VolumeMessage> {
    let mut out: Vec<VolumeMessage> = msgs
        .iter()
        .filter(|m| s(m, "resource_uuid").as_deref() == Some(volume_id))
        .filter_map(|m| {
            Some(VolumeMessage {
                id: s(m, "id")?,
                level: s(m, "message_level").unwrap_or_default(),
                message: s_ne(m, "user_message").unwrap_or_else(|| s(m, "event_id").unwrap_or_default()),
                created: s(m, "created_at").unwrap_or_default(),
                request_id: s_ne(m, "request_id"),
            })
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

/// The volume detail page, with attached servers' names and its snapshots.
pub fn volume_detail(v: &Value, servers: &[Value], snaps: &[Value]) -> VolumeDetail {
    let volume = volumes(std::slice::from_ref(v), servers).remove(0);
    let image_md = v.get("volume_image_metadata");
    let snapshots = snapshots(
        &snaps.iter().filter(|x| s(x, "volume_id").as_deref() == Some(volume.id.as_str())).cloned().collect::<Vec<_>>(),
        std::slice::from_ref(v),
    );
    VolumeDetail {
        created: s(v, "created_at").unwrap_or_default(),
        source_image: image_md.and_then(|m| s(m, "image_name").or_else(|| s(m, "image_id"))),
        source_snapshot_id: s_ne(v, "snapshot_id"),
        multiattach: b(v, "multiattach"),
        metadata: kv_pairs(v.get("metadata")),
        image_metadata: kv_pairs(image_md),
        snapshots,
        volume,
        messages: Vec::new(),
    }
}

const IMAGE_CORE_KEYS: &[&str] = &[
    "id", "name", "status", "visibility", "protected", "disk_format", "container_format", "size",
    "virtual_size", "checksum", "owner", "min_disk", "min_ram", "created_at", "updated_at", "tags",
    "self", "file", "schema", "locations", "direct_url", "os_hash_algo", "os_hash_value", "os_hidden",
    "stores",
];

/// Whether Glance owns this key (not a custom property).
pub fn is_image_core_key(k: &str) -> bool {
    IMAGE_CORE_KEYS.contains(&k)
}

fn image_properties(im: &Value) -> Vec<(String, String)> {
    let mut properties: Vec<(String, String)> = im
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| !is_image_core_key(k))
                .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())))
                .collect()
        })
        .unwrap_or_default();
    properties.sort();
    properties
}

/// The image detail page, with the owner's project name.
pub fn image_detail(im: &Value, project_id: &str, project_names: &HashMap<String, String>) -> ImageDetail {
    let image = images(std::slice::from_ref(im), project_id, project_names).remove(0);
    let properties = image.properties.clone();
    ImageDetail {
        owner: s_ne(im, "owner"),
        checksum: s_ne(im, "checksum"),
        container_format: s_ne(im, "container_format"),
        virtual_size: im.get("virtual_size").and_then(Value::as_u64),
        tags: strs(im, "tags"),
        created: s(im, "created_at"),
        updated: s(im, "updated_at"),
        properties,
        image,
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

/// `(value, label)` list for select boxes, sorted by label.
pub fn choices(items: &[Value], value_key: &str, label: impl Fn(&Value) -> String) -> Vec<Choice> {
    let mut out: Vec<Choice> = items
        .iter()
        .filter_map(|x| Some(Choice::new(s(x, value_key)?, label(x))))
        .collect();
    out.sort_by_key(|c| c.label.to_lowercase());
    out
}

// --- launch wizard tables: one cell per column, raw booleans ------------------

fn yes(v: bool) -> String {
    v.to_string()
}
fn gb(n: Option<i64>) -> String {
    format!("{} GB", n.unwrap_or(0))
}
fn mib_text(n: Option<i64>) -> String {
    crate::i18n::bytes((n.unwrap_or(0).max(0) as u64) * 1024 * 1024)
}
fn when(v: &Value, k: &str) -> String {
    s(v, k).map(|t| crate::components::widgets::short_time(&t)).unwrap_or_else(|| "-".into())
}
fn sorted(mut out: Vec<Choice>) -> Vec<Choice> {
    out.sort_by_key(|c| c.label.to_lowercase());
    out
}

/// Active images for the launch *Source* step: Name, Updated, Size, Format,
/// Visibility. `snapshots` picks Horizon's *Instance Snapshot* list instead.
pub fn image_choices(raw: &[Value], snapshots: bool) -> Vec<Choice> {
    sorted(
        raw.iter()
            .filter(|im| s(im, "status").as_deref() == Some("active"))
            .filter(|im| (s(im, "image_type").as_deref() == Some("snapshot")) == snapshots)
            .filter_map(|im| {
                let size = im.get("size").and_then(Value::as_u64);
                Some(Choice {
                    value: s(im, "id")?,
                    label: name_or_id(im),
                    cells: vec![
                        name_or_id(im),
                        when(im, "updated_at"),
                        size.map(crate::i18n::bytes).unwrap_or_else(|| "-".into()),
                        s(im, "disk_format").unwrap_or_default().to_uppercase(),
                        s(im, "visibility").unwrap_or_default(),
                    ],
                    facts: Facts {
                        size_bytes: size,
                        min_disk_gb: i(im, "min_disk"),
                        min_ram_mb: i(im, "min_ram"),
                        ..Default::default()
                    },
                })
            })
            .collect(),
    )
}

/// Name, VCPUS, RAM, Total Disk, Root Disk, Ephemeral Disk, Public.
/// Flavors in `[launch] flavor_sort` order (Horizon's `CREATE_INSTANCE_FLAVOR_SORT`), ties by name.
pub fn flavor_choices(raw: &[Value], sort: crate::prefs::FlavorSort, reverse: bool) -> Vec<Choice> {
    use crate::prefs::FlavorSort;
    let mut out: Vec<Choice> = raw
        .iter()
            .filter_map(|f| {
                let (disk, eph) = (i(f, "disk"), i(f, "OS-FLV-EXT-DATA:ephemeral"));
                Some(Choice {
                    value: s(f, "id")?,
                    label: label_flavor(f),
                    cells: vec![
                        name_or_id(f),
                        i(f, "vcpus").unwrap_or(0).to_string(),
                        mib_text(i(f, "ram")),
                        gb(Some(disk.unwrap_or(0) + eph.unwrap_or(0))),
                        gb(disk),
                        gb(eph),
                        yes(f.get("os-flavor-access:is_public").and_then(Value::as_bool).unwrap_or(true)),
                    ],
                    facts: Facts { vcpus: i(f, "vcpus"), ram_mb: i(f, "ram"), disk_gb: disk, ..Default::default() },
                })
            })
            .collect();
    let num = |c: &Choice| match sort {
        FlavorSort::Ram => c.facts.ram_mb,
        FlavorSort::Disk => c.facts.disk_gb,
        FlavorSort::Vcpus => c.facts.vcpus,
        FlavorSort::Id | FlavorSort::Name => None,
    };
    out.sort_by(|a, b| {
        let key = match sort {
            FlavorSort::Id => a.value.cmp(&b.value),
            FlavorSort::Name => a.label.to_lowercase().cmp(&b.label.to_lowercase()),
            _ => num(a).cmp(&num(b)),
        };
        let key = if reverse { key.reverse() } else { key };
        key.then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    out
}

/// Available volumes: Name, Description, Size, Type, Availability Zone.
pub fn volume_choices(raw: &[Value]) -> Vec<Choice> {
    sorted(
        raw.iter()
            .filter(|v| s(v, "status").as_deref() == Some("available"))
            .filter_map(|v| {
                Some(Choice {
                    value: s(v, "id")?,
                    label: label_volume(v),
                    cells: vec![
                        name_or_id(v),
                        s_ne(v, "description").unwrap_or_else(|| "-".into()),
                        gb(i(v, "size")),
                        s_ne(v, "volume_type").unwrap_or_else(|| "-".into()),
                        s_ne(v, "availability_zone").unwrap_or_else(|| "-".into()),
                    ],
                    facts: Facts { min_disk_gb: i(v, "size"), ..Default::default() },
                })
            })
            .collect(),
    )
}

/// Volume snapshots: Name, Description, Size, Created, Status.
pub fn snapshot_choices(raw: &[Value]) -> Vec<Choice> {
    sorted(
        raw.iter()
            .filter_map(|v| {
                Some(Choice {
                    value: s(v, "id")?,
                    label: label_volume(v),
                    cells: vec![
                        name_or_id(v),
                        s_ne(v, "description").unwrap_or_else(|| "-".into()),
                        gb(i(v, "size")),
                        when(v, "created_at"),
                        s(v, "status").unwrap_or_default(),
                    ],
                    facts: Facts { min_disk_gb: i(v, "size"), ..Default::default() },
                })
            })
            .collect(),
    )
}

/// Network, Subnets Associated, Shared, Admin State, Status.
pub fn network_choices(nets: &[Value], subnets: &[Value]) -> Vec<Choice> {
    let by_id = index_by_id(subnets);
    sorted(
        nets.iter()
            .filter_map(|n| {
                let subs: Vec<String> = strs(n, "subnets")
                    .iter()
                    .map(|id| match by_id.get(id) {
                        Some(sn) => format!("{} {}", name_or_id(sn), s(sn, "cidr").unwrap_or_default()),
                        None => id.clone(),
                    })
                    .collect();
                Some(Choice {
                    value: s(n, "id")?,
                    label: name_or_id(n),
                    cells: vec![
                        name_or_id(n),
                        if subs.is_empty() { "-".into() } else { subs.join(", ") },
                        yes(b(n, "shared")),
                        yes(n.get("admin_state_up").and_then(Value::as_bool).unwrap_or(true)),
                        s(n, "status").unwrap_or_default(),
                    ],
                    ..Default::default()
                })
            })
            .collect(),
    )
}

/// Instance ports (`compute:*`), labelled "server: ip". The first cell is the
/// instance ID (for `Source::Of`), the second the port's security group IDs,
/// comma-separated (to prefill the editor). `without_fip` drops ports that
/// already have a floating IP (association).
/// Networks a router with an external gateway is attached to: the only ones
/// whose ports a floating IP can reach (Horizon's `enable_fip_topology_check`).
/// `ports` must include the routers' interface ports.
pub fn externally_reachable_networks(ports: &[Value], routers: &[Value]) -> std::collections::HashSet<String> {
    let gateways: std::collections::HashSet<String> = routers
        .iter()
        .filter(|r| r.get("external_gateway_info").and_then(|g| s_ne(g, "network_id")).is_some())
        .filter_map(|r| s(r, "id"))
        .collect();
    ports
        .iter()
        .filter(|p| s(p, "device_owner").is_some_and(|o| o.starts_with("network:router_interface")))
        .filter(|p| s(p, "device_id").is_some_and(|d| gateways.contains(&d)))
        .filter_map(|p| s(p, "network_id"))
        .collect()
}

/// Instance ports; `reachable` (when set) keeps only those on those networks.
pub fn instance_port_choices(
    ports: &[Value],
    fips: &[Value],
    servers: &HashMap<String, String>,
    without_fip: bool,
    reachable: Option<&std::collections::HashSet<String>>,
) -> Vec<Choice> {
    let taken: std::collections::HashSet<String> = fips.iter().filter_map(|f| s_ne(f, "port_id")).collect();
    sorted(
        ports
            .iter()
            .filter(|p| s(p, "device_owner").is_some_and(|o| o.starts_with("compute:")))
            .filter(|p| reachable.is_none_or(|r| s(p, "network_id").is_some_and(|n| r.contains(&n))))
            .filter_map(|p| {
                let id = s(p, "id")?;
                if without_fip && taken.contains(&id) {
                    return None;
                }
                Some(Choice {
                    value: id,
                    label: label_port(p, servers),
                    cells: vec![s(p, "device_id").unwrap_or_default(), strs(p, "security_groups").join(",")],
                    ..Default::default()
                })
            })
            .collect(),
    )
}

/// [`instance_port_choices`] (without a floating IP) for every project, each
/// choice's port project put first in its cells (`AllAssociablePorts`).
pub fn owned_port_choices(ports: &[Value], fips: &[Value], servers: &HashMap<String, String>) -> Vec<Choice> {
    let owner: HashMap<String, String> = ports.iter().filter_map(|p| Some((s(p, "id")?, s_ne(p, "project_id").or_else(|| s_ne(p, "tenant_id"))?))).collect();
    let mut out = instance_port_choices(ports, fips, servers, true, None);
    for c in &mut out {
        c.cells.insert(0, owner.get(&c.value).cloned().unwrap_or_default());
    }
    out
}

/// Each fixed IPv4 address of an instance port, for port forwarding: value
/// `port_id/ip`, label `instance: ip`.
pub fn forwardable_port_choices(ports: &[Value], servers: &HashMap<String, String>) -> Vec<Choice> {
    sorted(
        ports
            .iter()
            .filter(|p| s(p, "device_owner").is_some_and(|o| o.starts_with("compute:")))
            .flat_map(|p| {
                let id = s(p, "id").unwrap_or_default();
                let dev = s(p, "device_id").and_then(|d| servers.get(&d).cloned()).unwrap_or_else(|| name_or_id(p));
                arr(p, "fixed_ips")
                    .iter()
                    .filter_map(|f| s(f, "ip_address"))
                    .filter(|ip| ip.parse::<std::net::Ipv4Addr>().is_ok())
                    .map(move |ip| Choice::new(format!("{id}/{ip}"), format!("{dev}: {ip}")))
                    .collect::<Vec<_>>()
            })
            .collect(),
    )
}

/// A floating IP with its port forwarding rules; rule ports are joined to
/// their instance through the project's ports.
pub fn floating_ip_detail(f: &Value, rules: &[Value], networks: &[Value], servers: &[Value], ports: &[Value]) -> FloatingIpDetail {
    let row = floating_ips(std::slice::from_ref(f), networks, servers).remove(0);
    let server_names = names_by_id(servers);
    let owner: HashMap<String, String> = ports.iter().filter_map(|p| Some((s(p, "id")?, s(p, "device_id")?))).collect();
    // Ranges come as `*_range`; single ports as numbers.
    let ports_of = |r: &Value, key: &str| {
        s_ne(r, &format!("{key}_range")).or_else(|| i(r, key).map(|n| n.to_string())).unwrap_or_else(|| "-".into())
    };
    let mut forwardings: Vec<_> = rules
        .iter()
        .map(|r| {
            let port = s(r, "internal_port_id").unwrap_or_default();
            PortForwardingRow {
                id: s(r, "id").unwrap_or_default(),
                protocol: s(r, "protocol").unwrap_or_default(),
                external: ports_of(r, "external_port"),
                internal_ip: s(r, "internal_ip_address").unwrap_or_default(),
                internal: ports_of(r, "internal_port"),
                instance: owner.get(&port).map(|d| Ref { id: d.clone(), name: server_names.get(d).cloned().unwrap_or_else(|| d.clone()) }),
                internal_port_id: port,
                description: s_ne(r, "description"),
            }
        })
        .collect();
    forwardings.sort_by(|a, b| (a.protocol.as_str(), a.external.as_str()).cmp(&(b.protocol.as_str(), b.external.as_str())));
    FloatingIpDetail { row, port_id: s_ne(f, "port_id"), router_id: s_ne(f, "router_id"), created: s_ne(f, "created_at"), forwardings }
}

/// Load balancer member candidates: every fixed IP of an instance port, value
/// `ip@subnet_id` (the member needs both), label `instance: ip`, cells
/// `[instance, ip, subnet name]` for the members step's tables (roadmap 54).
pub fn lb_member_choices(ports: &[Value], servers: &HashMap<String, String>, subnets: &[Value]) -> Vec<Choice> {
    let subnet_names = names_by_id(subnets);
    sorted(
        ports
            .iter()
            .filter(|p| s(p, "device_owner").is_some_and(|o| o.starts_with("compute:")))
            .flat_map(|p| {
                let dev = s(p, "device_id").and_then(|d| servers.get(&d).cloned()).unwrap_or_else(|| name_or_id(p));
                let subnet_names = &subnet_names;
                arr(p, "fixed_ips")
                    .iter()
                    .filter_map(|f| Some((s(f, "ip_address")?, s(f, "subnet_id")?)))
                    .map(move |(ip, sub)| {
                        let mut c = Choice::new(format!("{ip}@{sub}"), format!("{dev}: {ip}"));
                        c.cells = vec![dev.clone(), ip, subnet_names.get(&sub).cloned().unwrap_or(sub)];
                        c
                    })
                    .collect::<Vec<_>>()
            })
            .collect(),
    )
}

/// Subnets for an external member, labelled by name (else CIDR, else ID): those
/// on `networks` (the project's own, shared and external ones), like Horizon.
pub fn lb_member_subnet_choices(networks: &[Value], subnets: &[Value]) -> Vec<Choice> {
    let nets: std::collections::HashSet<String> = networks.iter().filter_map(|n| s(n, "id")).collect();
    sorted(
        subnets
            .iter()
            .filter(|x| s(x, "network_id").is_some_and(|n| nets.contains(&n)))
            .filter_map(|x| Some(Choice::new(s(x, "id")?, s_ne(x, "name").or_else(|| s(x, "cidr")).or_else(|| s(x, "id"))?)))
            .collect(),
    )
}

/// Enabled Octavia flavors, labelled `name (description)`.
/// Octavia availability zones that are enabled, by name (the value Octavia
/// takes), with the description (roadmap 54).
pub fn lb_az_choices(raw: &[Value]) -> Vec<Choice> {
    sorted(
        raw.iter()
            .filter(|z| z.get("enabled").and_then(Value::as_bool).unwrap_or(true))
            .filter_map(|z| {
                let name = s_ne(z, "name")?;
                let label = match s_ne(z, "description") {
                    Some(d) => format!("{name} ({d})"),
                    None => name.clone(),
                };
                Some(Choice::new(name, label))
            })
            .collect(),
    )
}

pub fn lb_flavor_choices(raw: &[Value]) -> Vec<Choice> {
    sorted(
        raw.iter()
            .filter(|f| f.get("enabled").and_then(Value::as_bool).unwrap_or(true))
            .filter_map(|f| {
                let label = match s_ne(f, "description") {
                    Some(d) => format!("{} ({d})", name_or_id(f)),
                    None => name_or_id(f),
                };
                Some(Choice::new(s(f, "id")?, label))
            })
            .collect(),
    )
}

/// Floating IPs not associated with a port, labelled by address.
pub fn free_floating_ip_choices(fips: &[Value]) -> Vec<Choice> {
    sorted(
        fips.iter()
            .filter(|f| s_ne(f, "port_id").is_none())
            .filter_map(|f| Some(Choice::new(s(f, "id")?, s(f, "floating_ip_address")?)))
            .collect(),
    )
}

/// Ports not attached to anything: Name, IP, Admin State, Status.
pub fn free_port_choices(ports: &[Value]) -> Vec<Choice> {
    sorted(
        ports
            .iter()
            .filter(|p| s_ne(p, "device_id").is_none() && s_ne(p, "device_owner").is_none())
            .filter_map(|p| {
                let ips: Vec<String> = arr(p, "fixed_ips").iter().filter_map(|f| s(f, "ip_address")).collect();
                Some(Choice {
                    value: s(p, "id")?,
                    label: name_or_id(p),
                    cells: vec![
                        name_or_id(p),
                        if ips.is_empty() { "-".into() } else { ips.join(", ") },
                        yes(p.get("admin_state_up").and_then(Value::as_bool).unwrap_or(true)),
                        s(p, "status").unwrap_or_default(),
                    ],
                    ..Default::default()
                })
            })
            .collect(),
    )
}

/// Security groups (Name, Description); `value_key` is `name` for Nova launch.
pub fn security_group_choices(raw: &[Value], value_key: &str) -> Vec<Choice> {
    sorted(
        raw.iter()
            .filter_map(|g| {
                Some(Choice {
                    value: s(g, value_key)?,
                    label: name_or_id(g),
                    cells: vec![name_or_id(g), s_ne(g, "description").unwrap_or_else(|| "-".into())],
                    ..Default::default()
                })
            })
            .collect(),
    )
}

/// Key pairs: Name, Type, Fingerprint.
pub fn keypair_choices(raw: &[Value]) -> Vec<Choice> {
    keypairs(raw)
        .into_iter()
        .map(|k| Choice {
            value: k.name.clone(),
            label: k.name.clone(),
            cells: vec![k.name, k.key_type.unwrap_or_else(|| "ssh".into()), k.fingerprint],
            ..Default::default()
        })
        .collect()
}

/// Server groups: Name, Policy (`policy` from 2.64, else `policies[0]`).
pub fn server_group_choices(raw: &[Value]) -> Vec<Choice> {
    sorted(
        raw.iter()
            .filter_map(|g| {
                let policy = s(g, "policy").or_else(|| strs(g, "policies").into_iter().next()).unwrap_or_default();
                Some(Choice {
                    value: s(g, "id")?,
                    label: name_or_id(g),
                    cells: vec![name_or_id(g), policy],
                    ..Default::default()
                })
            })
            .collect(),
    )
}

/// A choice label: the name, or the ID when unnamed.
pub fn label_name(x: &Value) -> String {
    name_or_id(x)
}

/// A flavor choice label with its size (vCPUs, RAM, disk).
pub fn label_flavor(f: &Value) -> String {
    format!(
        "{} · {} vCPU · {} · {} GB",
        name_or_id(f),
        i(f, "vcpus").unwrap_or(0),
        crate::i18n::bytes((i(f, "ram").unwrap_or(0).max(0) as u64) * 1024 * 1024),
        i(f, "disk").unwrap_or(0)
    )
}

/// A volume choice label with its size.
pub fn label_volume(v: &Value) -> String {
    format!("{} ({} GiB)", name_or_id(v), i(v, "size").unwrap_or(0))
}

/// A subnet choice label: `network: CIDR (subnet)`.
pub fn label_subnet(sn: &Value, net_names: &HashMap<String, String>) -> String {
    let net = s(sn, "network_id").and_then(|id| net_names.get(&id).cloned()).unwrap_or_default();
    format!("{}: {} ({})", net, s(sn, "cidr").unwrap_or_default(), name_or_id(sn))
}

/// A port choice label: the server it belongs to (or the port's name) and its
/// first address.
pub fn label_port(p: &Value, server_names: &HashMap<String, String>) -> String {
    let ip = arr(p, "fixed_ips").first().and_then(|f| s(f, "ip_address")).unwrap_or_default();
    let dev = s(p, "device_id").and_then(|d| server_names.get(&d).cloned()).unwrap_or_else(|| name_or_id(p));
    format!("{dev}: {ip}")
}

/// IDs of networks with a subnet (in their `subnets` list) that `subnets`
/// lacks: the external networks of another project, whose subnets neither the
/// `tenant_id` nor the `shared` filter returns (roadmap 64).
pub fn networks_without_subnets(nets: &[Value], subnets: &[Value]) -> Vec<String> {
    let have: std::collections::HashSet<String> = subnets.iter().filter_map(|x| s(x, "id")).collect();
    nets.iter().filter(|n| strs(n, "subnets").iter().any(|id| !have.contains(id))).filter_map(|n| s(n, "id")).collect()
}

/// Network ID → name.
pub fn network_names(nets: &[Value]) -> HashMap<String, String> {
    names_by_id(nets)
}
/// Server ID → name.
pub fn server_names(servers: &[Value]) -> HashMap<String, String> {
    names_by_id(servers)
}

// --- Quotas ---------------------------------------------------------------------

fn limit(v: Option<i64>) -> Option<i64> {
    v.filter(|x| *x >= 0)
}

/// Nova `GET /limits` → `{"absolute": {...}}`
pub fn compute_quotas(limits: &Value) -> Vec<Quota> {
    let a = limits.get("absolute").unwrap_or(limits);
    let q = |key, used: &str, max: &str, unit| Quota {
        key,
        used: i(a, used).unwrap_or(0),
        limit: limit(i(a, max)),
        unit,
    };
    vec![
        q(QuotaKey::Instances, "totalInstancesUsed", "maxTotalInstances", QuotaUnit::Count),
        q(QuotaKey::Vcpus, "totalCoresUsed", "maxTotalCores", QuotaUnit::Count),
        q(QuotaKey::Ram, "totalRAMUsed", "maxTotalRAMSize", QuotaUnit::Mib),
    ]
}

/// Cinder `GET /limits` → `{"absolute": {...}}`
pub fn volume_quotas(limits: &Value) -> Vec<Quota> {
    let a = limits.get("absolute").unwrap_or(limits);
    let q = |key, used: &str, max: &str, unit| Quota {
        key,
        used: i(a, used).unwrap_or(0),
        limit: limit(i(a, max)),
        unit,
    };
    vec![
        q(QuotaKey::Volumes, "totalVolumesUsed", "maxTotalVolumes", QuotaUnit::Count),
        q(QuotaKey::VolumeSnapshots, "totalSnapshotsUsed", "maxTotalSnapshots", QuotaUnit::Count),
        q(QuotaKey::VolumeStorage, "totalGigabytesUsed", "maxTotalVolumeGigabytes", QuotaUnit::Gib),
    ]
}

/// Neutron `GET /quotas/{project}/details` → `{"network": {"used", "limit"}, ...}`
pub fn network_quotas(details: &Value) -> Vec<Quota> {
    let q = |key, res: &str| {
        let r = details.get(res).cloned().unwrap_or(Value::Null);
        Quota { key, used: i(&r, "used").unwrap_or(0), limit: limit(i(&r, "limit")), unit: QuotaUnit::Count }
    };
    vec![
        q(QuotaKey::FloatingIps, "floatingip"),
        q(QuotaKey::SecurityGroups, "security_group"),
        q(QuotaKey::SecurityGroupRules, "security_group_rule"),
        q(QuotaKey::Networks, "network"),
        q(QuotaKey::Ports, "port"),
        q(QuotaKey::Routers, "router"),
    ]
}

/// Keys a quota set reports that are no quota of this service: Nova's
/// network proxies (gone from 2.36 on, Neutron's own quotas apply) and
/// Octavia's deprecated aliases of `load_balancer` and `health_monitor`.
const NOT_QUOTAS: &[&str] = &[
    "id",
    "project_id",
    "tenant_id",
    "floating_ips",
    "fixed_ips",
    "security_groups",
    "security_group_rules",
    "networks",
    "loadbalancer",
    "healthmonitor",
];

/// Project › Quotas (roadmap 82): every quota of one service's quota set, with
/// the response key stripped. Values are either `{limit, in_use|used, reserved}`
/// (Nova `os-quota-sets/{id}/detail`, Cinder `?usage=true`, Neutron
/// `quotas/{id}/details`) or a bare limit (Octavia), where `null` means the
/// cloud's default, taken from `defaults` (Octavia `lbaas/quotas/defaults`).
/// The editable keys come first, in `service.keys()` order, then the rest by name.
pub fn quota_rows(service: QuotaService, set: &Value, defaults: &Value) -> Vec<QuotaRow> {
    let Some(m) = set.as_object() else { return vec![] };
    let mut rows: Vec<QuotaRow> = m
        .iter()
        .filter(|(k, _)| !NOT_QUOTAS.contains(&k.as_str()))
        .filter_map(|(k, v)| {
            let row = match v {
                Value::Object(_) => QuotaRow {
                    key: k.clone(),
                    used: i(v, "in_use").or_else(|| i(v, "used")),
                    reserved: i(v, "reserved"),
                    limit: limit(i(v, "limit")),
                },
                Value::Number(n) => QuotaRow { key: k.clone(), used: None, reserved: None, limit: limit(n.as_i64()) },
                Value::Null => QuotaRow { key: k.clone(), used: None, reserved: None, limit: limit(i(defaults, k)) },
                _ => return None,
            };
            Some(row)
        })
        .collect();
    let keys = service.keys();
    // Cinder's per-type keys are grouped by volume type: volumes, snapshots,
    // size of `ceph`, then of the next type.
    let group = |k: &str| -> (String, usize) {
        let per_type = ["volumes_", "snapshots_", "gigabytes_"];
        per_type
            .iter()
            .enumerate()
            .find_map(|(i, p)| k.strip_prefix(p).filter(|_| service == QuotaService::Volume).map(|ty| (ty.to_string(), i)))
            .unwrap_or_else(|| (k.to_string(), 0))
    };
    rows.sort_by(|a, b| {
        let pos = |r: &QuotaRow| keys.iter().position(|k| *k == r.key).unwrap_or(usize::MAX);
        pos(a).cmp(&pos(b)).then_with(|| group(&a.key).cmp(&group(&b.key)))
    });
    rows
}

/// Barbican's quotas (roadmap 82): `GET v1/quotas` (`{"quotas": {"secrets": 10…}}`,
/// the token's project, defaults applied, `-1` unlimited) with *Used* read
/// from the `total` of one-item list pages (`secrets`, `containers`, `orders`),
/// as Barbican reports no usage. A list that failed (`Null`) leaves `-`.
pub fn key_manager_quota_rows(quotas: &Value, totals: &[(&str, &Value)]) -> Vec<QuotaRow> {
    let set = quotas.get("quotas").unwrap_or(quotas);
    let mut rows = quota_rows(QuotaService::KeyManager, set, &Value::Null);
    for r in &mut rows {
        r.used = totals.iter().find(|(k, _)| *k == r.key).and_then(|(_, page)| i(page, "total"));
    }
    rows
}

/// Barbican's quotas of **another** project (roadmap 82, Identity › *Modify
/// Quotas*): `GET v1/project-quotas/{p}` (`target`, `{"project_quotas": {…}}`,
/// `None` on 404: nothing set) holds only the values set for it; the others
/// follow the deployment's defaults, which Barbican doesn't publish. `v1/quotas`
/// (`effective`) is the session's project with defaults applied, so it gives the
/// defaults only when that project has no own values (`mine` is `Some(None)`, a
/// 404, or all `null`); `mine` is `None` when it couldn't be read. Otherwise
/// keys without their own value are left out rather than guessed.
/// Barbican counts nothing per project: *Used* stays `-`.
pub fn key_manager_project_rows(target: Option<&Value>, mine: Option<Option<&Value>>, effective: &Value) -> Vec<QuotaRow> {
    let own = |v: Option<&Value>| v.map(|v| v.get("project_quotas").unwrap_or(v).clone()).unwrap_or(Value::Null);
    let t = own(target);
    let mine_empty = mine.is_some_and(|m| own(m).as_object().is_none_or(|o| o.values().all(Value::is_null)));
    let defaults = if mine_empty { effective.get("quotas").unwrap_or(effective).clone() } else { Value::Null };
    let set: serde_json::Map<String, Value> =
        QuotaService::KeyManager.keys().iter().map(|k| (k.to_string(), t.get(*k).cloned().unwrap_or(Value::Null))).collect();
    let known = |k: &str| t.get(k).is_some_and(|v| !v.is_null()) || defaults.get(k).is_some_and(|v| !v.is_null());
    let mut rows = quota_rows(QuotaService::KeyManager, &Value::Object(set), &defaults);
    rows.retain(|r| known(&r.key));
    rows
}

/// `os-simple-tenant-usage/{project}` (response key already stripped).
pub fn server_usages(v: &Value) -> Vec<ServerUsage> {
    arr(v, "server_usages")
        .iter()
        .map(|u| {
            let id = s(u, "instance_id").unwrap_or_default();
            ServerUsage {
                name: s_ne(u, "name").unwrap_or_else(|| id.clone()),
                id,
                vcpus: i(u, "vcpus").unwrap_or(0),
                memory_mb: i(u, "memory_mb").unwrap_or(0),
                local_gb: i(u, "local_gb").unwrap_or(0),
                hours: u.get("hours").and_then(Value::as_f64).unwrap_or(0.0),
                uptime: i(u, "uptime").unwrap_or(0),
                state: s(u, "state").unwrap_or_default(),
            }
        })
        .collect()
}

/// Admin › Overview: Nova's detailed `tenant_usages`, from every page, one line
/// per project sorted by name. Paging is by instance, so a project can come back
/// on two pages: merged by `tenant_id`. Projects Keystone doesn't name keep their ID.
pub fn tenant_usages(raw: &[Value], names: &HashMap<String, String>) -> Vec<ProjectUsage> {
    let mut by_id: Vec<ProjectUsage> = Vec::new();
    for t in raw {
        let id = s(t, "tenant_id").unwrap_or_default();
        let servers = server_usages(t);
        match by_id.iter_mut().find(|p| p.project_id == id) {
            Some(p) => p.servers.extend(servers),
            None => by_id.push(ProjectUsage { project: names.get(&id).cloned().unwrap_or_else(|| id.clone()), project_id: id, servers }),
        }
    }
    by_id.sort_by_key(|a| a.project.to_lowercase());
    by_id
}

// --- Trunks -------------------------------------------------------------------

/// Trunks with their parent port and sub-ports named from the project's ports.
pub fn trunks(raw: &[Value], ports: &[Value]) -> Vec<TrunkRow> {
    let mut out = trunks_page(raw, ports);
    out.sort_by_key(|t| t.name.to_lowercase());
    out
}

/// The ports a page of trunks names (parents and sub-ports), each once (roadmap 88).
pub fn trunk_port_ids(raw: &[Value]) -> Vec<String> {
    distinct(raw.iter().flat_map(|t| s(t, "port_id").into_iter().chain(arr(t, "sub_ports").iter().filter_map(|sp| s(sp, "port_id")))))
}

/// [`trunks`] in Neutron's order, for a paged list (roadmap 88); `ports` need
/// only be the page's ([`trunk_port_ids`]).
pub fn trunks_page(raw: &[Value], ports: &[Value]) -> Vec<TrunkRow> {
    let port_names = names_by_id(ports);
    let named = |id: String| Ref { name: port_names.get(&id).cloned().unwrap_or_else(|| id.clone()), id };
    raw
        .iter()
        .map(|t| {
            let mut subports: Vec<_> = arr(t, "sub_ports")
                .iter()
                .map(|sp| {
                    let r = named(s(sp, "port_id").unwrap_or_default());
                    Subport { port_id: r.id, port_name: r.name, segmentation_type: s(sp, "segmentation_type").unwrap_or_default(), segmentation_id: i(sp, "segmentation_id") }
                })
                .collect();
            subports.sort_by_key(|s| s.segmentation_id);
            TrunkRow {
                id: s(t, "id").unwrap_or_default(),
                name: name_or_id(t),
                description: s_ne(t, "description"),
                parent: named(s(t, "port_id").unwrap_or_default()),
                status: s(t, "status").unwrap_or_default(),
                admin_up: b(t, "admin_state_up"),
                subports,
            }
        })
        .collect()
}

// --- Firewall (FWaaS v2, roadmap 93) --------------------------------------------

/// Whether a FWaaS row is the project's (`project_id`, or `tenant_id` on older Neutron).
fn owned_by(v: &Value, project_id: &str) -> bool {
    s(v, "project_id").or_else(|| s(v, "tenant_id")).as_deref() == Some(project_id)
}

/// A rule's `firewall_policy_id`: a list in FWaaS v2 (a rule can be in several
/// policies), a single ID or `null` on some releases.
fn rule_policy_ids(rule: &Value) -> Vec<String> {
    match rule.get("firewall_policy_id") {
        Some(Value::String(id)) if !id.is_empty() => vec![id.clone()],
        Some(Value::Array(ids)) => ids.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => vec![],
    }
}

/// Firewall rules, sorted by name, with the policies that use them: the rule's
/// `firewall_policy_id` and every visible policy that lists it.
pub fn firewall_rules(rules: &[Value], policies: &[Value], project_id: &str) -> Vec<FirewallRuleRow> {
    let policy_names = names_by_id(policies);
    let mut out: Vec<_> = rules
        .iter()
        .map(|r| {
            let id = s(r, "id").unwrap_or_default();
            let listed = policies.iter().filter(|p| strs(p, "firewall_rules").contains(&id)).filter_map(|p| s(p, "id"));
            let ids = distinct(rule_policy_ids(r).into_iter().chain(listed));
            FirewallRuleRow {
                name: name_or_id(r),
                description: s_ne(r, "description"),
                protocol: s_ne(r, "protocol"),
                ip_version: i(r, "ip_version").and_then(|v| u8::try_from(v).ok()).unwrap_or(4),
                source_ip: s_ne(r, "source_ip_address"),
                source_port: s_ne(r, "source_port"),
                destination_ip: s_ne(r, "destination_ip_address"),
                destination_port: s_ne(r, "destination_port"),
                action: s(r, "action").unwrap_or_default(),
                // Neutron's default is enabled.
                enabled: r.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                shared: b(r, "shared"),
                policies: ids.into_iter().map(|p| named_ref(p, &policy_names)).collect(),
                owned: owned_by(r, project_id),
                id,
            }
        })
        .collect();
    out.sort_by_key(|r| r.name.to_lowercase());
    out
}

/// Firewall policies, sorted by name, their rules in evaluation order.
pub fn firewall_policies(policies: &[Value], rules: &[Value], project_id: &str) -> Vec<FirewallPolicyRow> {
    let rule_names = names_by_id(rules);
    let mut out: Vec<_> = policies
        .iter()
        .map(|p| FirewallPolicyRow {
            id: s(p, "id").unwrap_or_default(),
            name: name_or_id(p),
            description: s_ne(p, "description"),
            rules: strs(p, "firewall_rules").into_iter().map(|r| named_ref(r, &rule_names)).collect(),
            audited: b(p, "audited"),
            shared: b(p, "shared"),
            owned: owned_by(p, project_id),
        })
        .collect();
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

/// A policy's page: [`firewall_policies`] of one, and its rules in its order.
pub fn firewall_policy_detail(policy: &Value, rules: &[Value], all_policies: &[Value], project_id: &str) -> FirewallPolicyDetail {
    let p = firewall_policies(std::slice::from_ref(policy), rules, project_id).remove(0);
    let rows = firewall_rules(rules, all_policies, project_id);
    let by_id: HashMap<&str, &FirewallRuleRow> = rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let ordered = p.rules.iter().filter_map(|r| by_id.get(r.id.as_str()).map(|x| (*x).clone())).collect();
    FirewallPolicyDetail { policy: p, rules: ordered }
}

/// What a port belongs to, from its `device_owner`: `router` for router
/// interfaces (FWaaS v2's L3 ports), `instance` for `compute:*`.
fn port_kind(port: &Value) -> Option<&'static str> {
    let owner = s(port, "device_owner").unwrap_or_default();
    if owner.starts_with("network:router_interface") || owner == "network:ha_router_replicated_interface" {
        Some("router")
    } else if owner.starts_with("compute:") {
        Some("instance")
    } else {
        None
    }
}

/// A port with its IPs and device; `routers` and `servers` name the device
/// (both may be empty: then the device is its ID).
fn firewall_port(id: String, ports: &HashMap<String, &Value>, routers: &HashMap<String, String>, servers: &HashMap<String, String>) -> FirewallPort {
    let Some(p) = ports.get(&id) else {
        return FirewallPort { port: Ref { name: id.clone(), id }, ips: vec![], kind: None, device: None };
    };
    let kind = port_kind(p);
    let device = s_ne(p, "device_id").map(|d| match kind {
        Some("router") => named_ref(d, routers),
        Some("instance") => named_ref(d, servers),
        _ => Ref { name: d.clone(), id: d },
    });
    FirewallPort {
        port: Ref { name: name_or_id(p), id },
        ips: arr(p, "fixed_ips").iter().filter_map(|f| s(f, "ip_address")).collect(),
        kind: kind.map(str::to_string),
        device,
    }
}

/// Firewall groups, sorted by name, policies named and ports resolved from
/// `ports` (and their devices from `routers` / `servers`, which may be empty).
pub fn firewall_groups(groups: &[Value], policies: &[Value], ports: &[Value], routers: &[Value], servers: &[Value], project_id: &str) -> Vec<FirewallGroupRow> {
    let (policy_names, port_index, router_names, server_names) = (names_by_id(policies), index_by_id(ports), names_by_id(routers), names_by_id(servers));
    let policy = |g: &Value, k: &str| s_ne(g, k).map(|id| named_ref(id, &policy_names));
    let mut out: Vec<_> = groups
        .iter()
        .map(|g| FirewallGroupRow {
            id: s(g, "id").unwrap_or_default(),
            name: name_or_id(g),
            description: s_ne(g, "description"),
            ingress: policy(g, "ingress_firewall_policy_id"),
            egress: policy(g, "egress_firewall_policy_id"),
            ports: strs(g, "ports").into_iter().map(|p| firewall_port(p, &port_index, &router_names, &server_names)).collect(),
            status: s(g, "status").unwrap_or_default(),
            admin_up: g.get("admin_state_up").and_then(Value::as_bool).unwrap_or(true),
            shared: b(g, "shared"),
            owned: owned_by(g, project_id),
        })
        .collect();
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

/// *Insert Rule*, *Add Policy*: the rules the project can use; cells `[action, protocol]`.
/// Rule pickers: labelled by name; cells `[name, action, protocol]` (empty: any),
/// which `pick::columns` translates in the browser.
pub fn firewall_rule_choices(rules: &[Value]) -> Vec<Choice> {
    sorted(
        rules
            .iter()
            .filter_map(|r| {
                let name = name_or_id(r);
                let cells = vec![name.clone(), s(r, "action").unwrap_or_default(), s_ne(r, "protocol").unwrap_or_default()];
                Some(Choice { value: s(r, "id")?, label: name, cells, ..Default::default() })
            })
            .collect(),
    )
}

/// Ingress / egress policy pickers.
pub fn firewall_policy_choices(policies: &[Value]) -> Vec<Choice> {
    sorted(policies.iter().filter_map(|p| Some(Choice::new(s(p, "id")?, name_or_id(p)))).collect())
}

/// Ports a firewall group can protect: the project's router interfaces and
/// instance ports that no group has yet (Neutron refuses a port another group
/// protects). Ports are seldom named, so the row says what the port belongs
/// to: cells `[device, kind, IPs, port]` (`pick::columns`), the port by name
/// or, unnamed, the start of its ID; labelled `device: IPs`.
pub fn firewall_port_choices(ports: &[Value], groups: &[Value], routers: &[Value], servers: &[Value]) -> Vec<Choice> {
    let taken: HashSet<String> = groups.iter().flat_map(|g| strs(g, "ports")).collect();
    let (index, router_names, server_names) = (index_by_id(ports), names_by_id(routers), names_by_id(servers));
    sorted(
        ports
            .iter()
            .filter(|p| port_kind(p).is_some())
            .filter_map(|p| {
                let id = s(p, "id")?;
                if taken.contains(&id) {
                    return None;
                }
                let fp = firewall_port(id.clone(), &index, &router_names, &server_names);
                let device = fp.device.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| "-".into());
                let ips = if fp.ips.is_empty() { "-".to_string() } else { fp.ips.join(", ") };
                let port = s_ne(p, "name").unwrap_or_else(|| id.chars().take(8).collect());
                let label = format!("{device}: {ips}");
                Some(Choice { cells: vec![device, fp.kind.unwrap_or_default(), ips, port], value: id, label, ..Default::default() })
            })
            .collect(),
    )
}

// --- Key Manager (Barbican, roadmap 55) ---------------------------------------

/// The ID in a Barbican `secret_ref` / `container_ref` URL: its last path
/// segment (Barbican returns refs, not IDs).
pub fn ref_id(r: &str) -> String {
    r.trim_end_matches('/').rsplit('/').next().unwrap_or_default().to_string()
}

/// Barbican pages with `limit`/`offset` and reports `total` (not `marker`): the
/// offset of the next page after one of `got` items at `offset`, if any.
/// Without `total`, the `next` link says whether there is more.
pub fn next_offset(page: &Value, offset: u64, got: usize) -> Option<u64> {
    let next = offset + got as u64;
    if got == 0 {
        return None;
    }
    match page.get("total").and_then(Value::as_u64) {
        Some(total) => (next < total).then_some(next),
        None => page.get("next").filter(|n| !n.is_null()).map(|_| next),
    }
}

/// A Barbican timestamp, `None` when empty or `null`.
fn km_when(v: &Value, k: &str) -> Option<String> {
    s_ne(v, k)
}

/// Secrets' metadata, sorted by name like barbican-ui.
pub fn secrets(raw: &[Value]) -> Vec<SecretRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|x| {
            let secret_ref = s(x, "secret_ref").unwrap_or_default();
            let id = ref_id(&secret_ref);
            SecretRow {
                name: s_ne(x, "name").unwrap_or_else(|| id.clone()),
                id,
                secret_type: s(x, "secret_type").unwrap_or_else(|| "opaque".into()),
                algorithm: s_ne(x, "algorithm"),
                bit_length: i(x, "bit_length"),
                mode: s_ne(x, "mode"),
                content_type: x.get("content_types").and_then(|c| s_ne(c, "default")),
                status: s(x, "status").unwrap_or_default(),
                expiration: km_when(x, "expiration"),
                created: s(x, "created").unwrap_or_default(),
                updated: km_when(x, "updated"),
                creator_id: s_ne(x, "creator_id"),
                secret_ref,
            }
        })
        .collect();
    out.sort_by_key(|x| x.name.to_lowercase());
    out
}

/// Secrets to put in a container: value `secret_ref` (Barbican takes refs),
/// label the name, one cell with the type.
pub fn secret_choices(raw: &[Value]) -> Vec<Choice> {
    secrets(raw).into_iter().map(|x| Choice { cells: vec![x.secret_type.clone()], ..Choice::new(x.secret_ref, format!("{} ({})", x.name, x.secret_type)) }).collect()
}

/// Certificates a `TERMINATED_HTTPS` listener can use (roadmap 54), as
/// octavia-dashboard lists them: `certificate` containers holding both a
/// `certificate` and a `private_key`, then opaque secrets (PKCS#12 bundles) that
/// aren't part of one. The value is the Barbican URL Octavia takes; the label the
/// name and the certificate's expiry (`secrets`, the project's, give it); the
/// first cell that expiry, `""` when it never expires.
pub fn lb_certificate_choices(containers: &[Value], secrets: &[Value]) -> Vec<Choice> {
    let expiry: HashMap<String, Option<String>> = secrets.iter().filter_map(|x| Some((s(x, "secret_ref")?, km_when(x, "expiration")))).collect();
    let date = |e: &Option<String>| e.as_deref().map(|d| d.get(..10).unwrap_or(d).to_string()).unwrap_or_default();
    let mut used = std::collections::HashSet::new();
    let mut out: Vec<Choice> = containers
        .iter()
        .filter(|c| s(c, "type").as_deref() == Some("certificate"))
        .filter_map(|c| {
            let refs: HashMap<String, String> = arr(c, "secret_refs").iter().filter_map(|r| Some((s(r, "name")?, s(r, "secret_ref")?))).collect();
            let cert = refs.get("certificate")?;
            refs.get("private_key")?;
            used.extend(refs.values().cloned());
            let container_ref = s(c, "container_ref")?;
            let name = s_ne(c, "name").unwrap_or_else(|| ref_id(&container_ref));
            let when = date(expiry.get(cert).unwrap_or(&None));
            Some((name, when, container_ref))
        })
        .map(|(name, when, r)| Choice { cells: vec![when.clone()], ..Choice::new(r, if when.is_empty() { name } else { format!("{name} ({when})") }) })
        .collect();
    let mut bundles: Vec<Choice> = secrets
        .iter()
        .filter(|x| s(x, "secret_type").as_deref() == Some("opaque"))
        .filter_map(|x| {
            let r = s(x, "secret_ref")?;
            if used.contains(&r) {
                return None;
            }
            let name = s_ne(x, "name").unwrap_or_else(|| ref_id(&r));
            let when = date(&km_when(x, "expiration"));
            Some(Choice { cells: vec![when.clone()], ..Choice::new(r, if when.is_empty() { name } else { format!("{name} ({when})") }) })
        })
        .collect();
    out.sort_by_key(|c| c.label.to_lowercase());
    bundles.sort_by_key(|c| c.label.to_lowercase());
    out.extend(bundles);
    out
}

/// A secret's `read` ACL (`GET /v1/secrets/{id}/acl`): the users granted
/// access, and `project-access` (Barbican's default is `true`).
pub fn secret_acl(acl: &Value) -> (Vec<String>, Option<bool>) {
    let read = acl.get("read").unwrap_or(&Value::Null);
    let project = read.get("project-access").and_then(Value::as_bool).or(acl.is_object().then_some(true));
    (strs(read, "users"), project)
}

/// What uses a secret (roadmap 75): its consumers (`service resource_type
/// resource_id`), then each container holding it that has consumers
/// (`container NAME: consumer, …`). Octavia registers on the container, not on
/// its secrets, so the second part is what finds a load balancer.
pub fn secret_in_use(id: &str, consumers: &[SecretConsumer], containers: &[ContainerRow]) -> Vec<String> {
    let own = consumers.iter().map(|c| format!("{} {} {}", c.service, c.resource_type, c.resource_id));
    let holders = containers
        .iter()
        .filter(|c| !c.consumers.is_empty() && c.secrets.iter().any(|x| x.id == id))
        .map(|c| format!("container {}: {}", c.name, c.consumers.iter().map(consumer_text).collect::<Vec<_>>().join(", ")));
    own.chain(holders).collect()
}

/// The Barbican refs Octavia's listeners and pools point at, as `(ID, use)`:
/// listeners' `default_tls_container_ref`, `sni_container_refs`,
/// `client_ca_tls_container_ref`, `client_crl_container_ref`; pools'
/// `tls_container_ref`, `ca_tls_container_ref`, `crl_container_ref`. Load
/// balancers are named from `load_balancers` (the project's), else by ID.
/// Octavia registers no Barbican consumer, so this is how a load balancer using a
/// certificate is found (roadmap 75).
pub fn octavia_cert_refs(listeners: &[Value], pools: &[Value], load_balancers: &[Value]) -> Vec<(String, OctaviaUse)> {
    let names = names_by_id(load_balancers);
    let mut out = vec![];
    let mut add = |x: &Value, kind: &str, keys: &[&str], list: Option<&str>| {
        let lb_id = arr(x, "loadbalancers").first().and_then(|l| s(l, "id"));
        let lb_name = lb_id.as_ref().map(|id| names.get(id).cloned().unwrap_or_else(|| id.clone())).unwrap_or_else(|| "-".into());
        let one = keys.iter().filter_map(|k| x.get(*k)?.as_str());
        let many = list.and_then(|k| x.get(k)?.as_array()).into_iter().flatten().filter_map(Value::as_str);
        for r in one.chain(many).filter(|r| !r.is_empty()) {
            let used = OctaviaUse { lb_id: lb_id.clone(), lb_name: lb_name.clone(), kind: kind.into(), name: name_or_id(x), via: None };
            out.push((ref_id(r), used));
        }
    };
    for l in listeners {
        add(l, "listener", &["default_tls_container_ref", "client_ca_tls_container_ref", "client_crl_container_ref"], Some("sni_container_refs"));
    }
    for p in pools {
        add(p, "pool", &["tls_container_ref", "ca_tls_container_ref", "crl_container_ref"], None);
    }
    out
}

/// The uses of `id` among [`octavia_cert_refs`], each once; for a secret also
/// those through the `holders` (containers holding it), marked with `via`.
pub fn octavia_uses(refs: &[(String, OctaviaUse)], id: &str, holders: &[ContainerRow]) -> Vec<OctaviaUse> {
    let direct = refs.iter().filter(|(r, _)| r == id).map(|(_, u)| u.clone());
    let through = holders.iter().filter(|c| c.secrets.iter().any(|x| x.id == id)).flat_map(|c| {
        refs.iter().filter(|(r, _)| *r == c.id).map(|(_, u)| OctaviaUse { via: Some((c.id.clone(), c.name.clone())), ..u.clone() })
    });
    let mut out: Vec<OctaviaUse> = vec![];
    for u in direct.chain(through) {
        if !out.contains(&u) {
            out.push(u);
        }
    }
    out
}

/// What uses a container: its consumers.
pub fn container_in_use(c: &ContainerRow) -> Vec<String> {
    c.consumers.iter().map(consumer_text).collect()
}

/// A container consumer as `name url` (Octavia: `octavia lbaas://region/loadbalancer/ID`).
fn consumer_text(c: &ContainerConsumer) -> String {
    format!("{} {}", c.name, c.url).trim().to_string()
}

/// `GET /v1/secrets/{id}/consumers`.
pub fn secret_consumers(raw: &[Value]) -> Vec<SecretConsumer> {
    raw.iter()
        .map(|c| SecretConsumer {
            service: s(c, "service").unwrap_or_default(),
            resource_type: s(c, "resource_type").unwrap_or_default(),
            resource_id: s(c, "resource_id").unwrap_or_default(),
        })
        .collect()
}

/// Containers with their secrets named from `secrets` (the project's, from
/// `GET /v1/secrets`; an unknown secret keeps its ID).
pub fn containers(raw: &[Value], secrets: &[Value]) -> Vec<ContainerRow> {
    let names: HashMap<String, String> = self::secrets(secrets).into_iter().map(|x| (x.id, x.name)).collect();
    let mut out: Vec<_> = raw
        .iter()
        .map(|c| {
            let container_ref = s(c, "container_ref").unwrap_or_default();
            let id = ref_id(&container_ref);
            let mut members: Vec<_> = arr(c, "secret_refs")
                .iter()
                .map(|r| {
                    let id = ref_id(&s(r, "secret_ref").unwrap_or_default());
                    ContainerSecret { role: s_ne(r, "name").unwrap_or_else(|| "-".into()), name: names.get(&id).cloned().unwrap_or_else(|| id.clone()), id }
                })
                .collect();
            members.sort_by(|a, b| (&a.role, &a.name).cmp(&(&b.role, &b.name)));
            ContainerRow {
                name: s_ne(c, "name").unwrap_or_else(|| id.clone()),
                id,
                container_type: s(c, "type").unwrap_or_else(|| "generic".into()),
                status: s(c, "status").unwrap_or_default(),
                created: s(c, "created").unwrap_or_default(),
                updated: km_when(c, "updated"),
                creator_id: s_ne(c, "creator_id"),
                container_ref,
                secrets: members,
                consumers: arr(c, "consumers").iter().map(|k| ContainerConsumer { name: s(k, "name").unwrap_or_default(), url: s(k, "URL").unwrap_or_default() }).collect(),
            }
        })
        .collect();
    out.sort_by_key(|c| c.name.to_lowercase());
    out
}

// --- Orchestration (Heat, roadmap 62) -----------------------------------------

/// Heat stacks, newest first like Horizon.
pub fn stacks(raw: &[Value]) -> Vec<StackRow> {
    let mut out: Vec<_> = raw.iter().map(stack_row).collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

fn stack_row(x: &Value) -> StackRow {
    let id = s(x, "id").unwrap_or_default();
    StackRow {
        name: s_ne(x, "stack_name").unwrap_or_else(|| id.clone()),
        id,
        description: s_ne(x, "description"),
        status: s(x, "stack_status").unwrap_or_default(),
        status_reason: s_ne(x, "stack_status_reason"),
        created: s(x, "creation_time").unwrap_or_default(),
        updated: s_ne(x, "updated_time"),
    }
}

/// The value of an output or parameter as text: strings as they are, other
/// JSON compact, `-` for `null`.
fn heat_value(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "-".into(),
        Some(Value::String(x)) => x.clone(),
        Some(other) => other.to_string(),
    }
}

/// The stack detail from `GET stacks/{name}/{id}` (`stack` unwrapped or not),
/// its resources, events and template.
pub fn stack_detail(stack: &Value, resources: &[Value], events: &[Value], template: &Value) -> StackDetail {
    let x = unwrap(stack, "stack");
    let mut parameters: Vec<(String, String)> = x
        .get("parameters")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        // Heat adds `OS::stack_id`, `OS::stack_name` and `OS::project_id` to every stack.
        .filter(|(k, _)| !k.starts_with("OS::"))
        .map(|(k, v)| (k.clone(), heat_value(Some(v))))
        .collect();
    parameters.sort();
    let mut outputs: Vec<StackOutput> = arr(x, "outputs")
        .iter()
        .map(|o| StackOutput {
            key: s(o, "output_key").unwrap_or_default(),
            value: heat_value(o.get("output_value")),
            description: s_ne(o, "description"),
            error: s_ne(o, "output_error"),
        })
        .collect();
    outputs.sort_by(|a, b| a.key.cmp(&b.key));
    StackDetail {
        stack: stack_row(x),
        parameters,
        outputs,
        timeout_mins: i(x, "timeout_mins"),
        disable_rollback: x.get("disable_rollback").and_then(Value::as_bool).unwrap_or(true),
        parent: s_ne(x, "parent"),
        tags: strs(x, "tags"),
        resources: stack_resources(resources),
        events: stack_events(events),
        template: serde_json::to_string_pretty(template).unwrap_or_default(),
    }
}

/// A stack's resources by name, with `depends_on` filled from the other
/// resources' `required_by` and a link to Moonlight's page for each.
pub fn stack_resources(raw: &[Value]) -> Vec<StackResource> {
    let mut out: Vec<StackResource> = raw
        .iter()
        .map(|r| {
            let resource_type = s(r, "resource_type").unwrap_or_default();
            let physical_id = s_ne(r, "physical_resource_id");
            StackResource {
                name: s(r, "resource_name").unwrap_or_default(),
                href: physical_id.as_deref().and_then(|id| resource_href(&resource_type, id)),
                physical_id,
                resource_type,
                status: s(r, "resource_status").unwrap_or_default(),
                status_reason: s_ne(r, "resource_status_reason"),
                updated: s(r, "updated_time").or_else(|| s(r, "creation_time")).unwrap_or_default(),
                required_by: strs(r, "required_by"),
                depends_on: Vec::new(),
            }
        })
        .collect();
    let edges: Vec<(String, String)> = out.iter().flat_map(|r| r.required_by.iter().map(|by| (by.clone(), r.name.clone()))).collect();
    for r in &mut out {
        r.depends_on = edges.iter().filter(|(by, _)| *by == r.name).map(|(_, on)| on.clone()).collect();
        r.depends_on.sort();
        r.required_by.sort();
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Moonlight's page for a Heat resource, by its type. Nested stacks
/// (`OS::Heat::Stack`, groups, provider templates) link to their stack page.
pub fn resource_href(resource_type: &str, physical_id: &str) -> Option<String> {
    let path = match resource_type {
        "OS::Nova::Server" => "instances",
        "OS::Nova::KeyPair" => "key_pairs",
        "OS::Nova::ServerGroup" => "server_groups",
        "OS::Glance::WebImage" | "OS::Glance::Image" => "images",
        "OS::Cinder::Volume" => "volumes",
        "OS::Neutron::Net" | "OS::Neutron::ProviderNet" => "networks",
        "OS::Neutron::Subnet" => "networks/subnets",
        "OS::Neutron::Port" => "networks/ports",
        "OS::Neutron::Router" => "routers",
        "OS::Neutron::FloatingIP" => "floating_ips",
        "OS::Neutron::SecurityGroup" => "security_groups",
        "OS::Neutron::Trunk" => "trunks",
        "OS::Neutron::QoSPolicy" => "qos",
        "OS::Octavia::LoadBalancer" => "load_balancers",
        // Barbican's physical IDs are refs (URLs).
        "OS::Barbican::Secret" => return Some(format!("/project/key_manager/secrets/{}", ref_id(physical_id))),
        "OS::Barbican::GenericContainer" | "OS::Barbican::CertificateContainer" | "OS::Barbican::RSAContainer" => {
            return Some(format!("/project/key_manager/containers/{}", ref_id(physical_id)));
        }
        "OS::Heat::Stack" | "OS::Heat::ResourceGroup" | "OS::Heat::AutoScalingGroup" => "stacks",
        t if t.ends_with(".yaml") || t.ends_with(".template") || t.starts_with("file://") || t.starts_with("http") => "stacks",
        _ => return None,
    };
    Some(format!("/project/{path}/{physical_id}"))
}

/// Stack events, newest first (whatever order Heat sent them in).
pub fn stack_events(raw: &[Value]) -> Vec<StackEvent> {
    let mut out: Vec<StackEvent> = raw
        .iter()
        .map(|e| StackEvent {
            id: s(e, "id").unwrap_or_default(),
            time: s(e, "event_time").unwrap_or_default(),
            resource_name: s(e, "resource_name").or_else(|| s(e, "logical_resource_id")).unwrap_or_default(),
            physical_id: s_ne(e, "physical_resource_id"),
            status: s(e, "resource_status").unwrap_or_default(),
            status_reason: s_ne(e, "resource_status_reason"),
        })
        .collect();
    out.sort_by(|a, b| b.time.cmp(&a.time));
    out
}

/// Resource types by name. Heat lists `{resource_type, description}` with
/// `with_description=true` (Pike) and bare names before it.
pub fn resource_types(raw: &[Value]) -> Vec<ResourceTypeRow> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|x| match x {
            Value::String(n) => Some(ResourceTypeRow { name: n.clone(), description: None }),
            _ => Some(ResourceTypeRow { name: s_ne(x, "resource_type")?, description: s_ne(x, "description") }),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// A constraint without its `description`, compact: `allowed_values: ["a","b"]`.
fn heat_constraint(c: &Value) -> Option<String> {
    let m = c.as_object()?;
    let parts: Vec<String> = m.iter().filter(|(k, _)| *k != "description").map(|(k, v)| format!("{k}: {}", heat_value(Some(v)))).collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// A resource type's schema (`GET resource_types/{type}`), properties and
/// attributes sorted by name.
pub fn resource_type_detail(v: &Value) -> ResourceTypeDetail {
    let support = v.get("support_status").and_then(|st| {
        let status = s_ne(st, "status")?;
        Some(match s_ne(st, "version") {
            Some(ver) => format!("{status} ({ver})"),
            None => status,
        })
    });
    let entries = |k: &str| {
        let mut e: Vec<(String, Value)> = v.get(k).and_then(Value::as_object).map(|m| m.iter().map(|(k, x)| (k.clone(), x.clone())).collect()).unwrap_or_default();
        e.sort_by(|a, b| a.0.cmp(&b.0));
        e
    };
    let properties = entries("properties")
        .into_iter()
        .map(|(name, p)| TypeProperty {
            kind: s(&p, "type").unwrap_or_default(),
            required: b(&p, "required"),
            update_allowed: b(&p, "update_allowed"),
            default: p.get("default").filter(|d| !d.is_null()).map(|d| heat_value(Some(d))),
            description: s_ne(&p, "description"),
            constraints: arr(&p, "constraints").iter().filter_map(heat_constraint).collect(),
            name,
        })
        .collect();
    let attributes = entries("attributes")
        .into_iter()
        .map(|(name, a)| TypeAttribute { kind: s_ne(&a, "type"), description: s_ne(&a, "description"), name })
        .collect();
    ResourceTypeDetail {
        name: s(v, "resource_type").unwrap_or_default(),
        description: s_ne(v, "description"),
        support,
        properties,
        attributes,
        raw: serde_json::to_string_pretty(v).unwrap_or_default(),
    }
}

/// Template versions, newest first within each kind (HOT, then CFN), as their
/// names sort by date.
pub fn template_versions(raw: &[Value]) -> Vec<TemplateVersion> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|x| Some(TemplateVersion { version: s_ne(x, "version")?, kind: s(x, "type").unwrap_or_default(), aliases: strs(x, "aliases") }))
        .collect();
    out.sort_by(|a, b| a.kind.cmp(&b.kind).reverse().then_with(|| b.version.cmp(&a.version)));
    out
}

/// A template version's functions by name (Heat calls the name `functions`).
pub fn template_functions(raw: &[Value]) -> Vec<TemplateFunction> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|x| Some(TemplateFunction { name: s_ne(x, "functions")?, description: s_ne(x, "description") }))
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// --- Network QoS --------------------------------------------------------------

/// QoS policies the project sees: its own and shared ones (`owned` tells them
/// apart: only the own ones can be changed). Rules come embedded in the policy.
pub fn qos_policies(raw: &[Value], project_id: &str) -> Vec<QosPolicyRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|p| QosPolicyRow {
            id: s(p, "id").unwrap_or_default(),
            name: name_or_id(p),
            description: s_ne(p, "description"),
            shared: b(p, "shared"),
            is_default: b(p, "is_default"),
            owned: s(p, "project_id").or_else(|| s(p, "tenant_id")).as_deref() == Some(project_id),
            rules: arr(p, "rules")
                .iter()
                .map(|r| QosRule {
                    id: s(r, "id").unwrap_or_default(),
                    kind: s(r, "type").unwrap_or_default(),
                    direction: s_ne(r, "direction"),
                    max_kbps: i(r, "max_kbps"),
                    max_burst_kbps: i(r, "max_burst_kbps"),
                    dscp_mark: i(r, "dscp_mark"),
                    min_kbps: i(r, "min_kbps"),
                    min_kpps: i(r, "min_kpps"),
                })
                .collect(),
        })
        .collect();
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

/// QoS policies as choices, by name.
pub fn qos_policy_choices(raw: &[Value]) -> Vec<Choice> {
    sorted(raw.iter().filter_map(|p| Some(Choice::new(s(p, "id")?, name_or_id(p)))).collect())
}

// --- Load balancers (Octavia) -------------------------------------------------

/// A listener's *Advanced* options (roadmap 54). `insert_headers` values are
/// strings (`"true"`/`"false"`); only the true ones count. Missing keys (older
/// Octavia) read as defaults.
fn listener_options(v: &Value) -> LbListenerOptions {
    let ms = |k: &str| i(v, k).and_then(|n| u32::try_from(n).ok());
    let on = |x: &Value| x.as_bool() == Some(true) || x.as_str().is_some_and(|s| s.eq_ignore_ascii_case("true"));
    let mut insert_headers: Vec<String> =
        v.get("insert_headers").and_then(Value::as_object).map(|m| m.iter().filter(|(_, x)| on(x)).map(|(k, _)| k.clone()).collect()).unwrap_or_default();
    insert_headers.sort();
    LbListenerOptions {
        timeout_client_data: ms("timeout_client_data"),
        timeout_member_connect: ms("timeout_member_connect"),
        timeout_member_data: ms("timeout_member_data"),
        timeout_tcp_inspect: ms("timeout_tcp_inspect"),
        insert_headers,
        // Present (even `null`) from Octavia 2.12: offered in the edit dialog then.
        allowed_cidrs: v.get("allowed_cidrs").map(|_| strs(v, "allowed_cidrs")),
        tls_ciphers: s_ne(v, "tls_ciphers"),
    }
}

fn ids(v: &Value, k: &str) -> Vec<String> {
    arr(v, k).iter().filter_map(|x| s(x, "id")).collect()
}
fn named(id: String, names: &HashMap<String, String>) -> Ref {
    Ref { name: names.get(&id).cloned().unwrap_or_else(|| id.clone()), id }
}
fn status(v: &Value, k: &str) -> String {
    s(v, k).unwrap_or_else(|| "UNKNOWN".into())
}

/// Load balancer rows (Octavia), with VIP and statuses.
pub fn load_balancers(raw: &[Value]) -> Vec<LoadBalancerRow> {
    let mut out: Vec<_> = raw.iter().map(load_balancer_row).collect();
    out.sort_by_key(|l| l.name.to_lowercase());
    out
}

fn load_balancer_row(l: &Value) -> LoadBalancerRow {
    LoadBalancerRow {
        id: s(l, "id").unwrap_or_default(),
        name: name_or_id(l),
        description: s_ne(l, "description"),
        vip_address: s_ne(l, "vip_address"),
        availability_zone: s_ne(l, "availability_zone"),
        operating_status: status(l, "operating_status"),
        provisioning_status: status(l, "provisioning_status"),
        admin_up: b(l, "admin_state_up"),
        provider: s_ne(l, "provider"),
    }
}

/// What the detail page needs, beyond the load balancer itself.
pub struct LbInputs<'a> {
    pub listeners: &'a [Value],
    pub pools: &'a [Value],
    /// Every pool's members, each tagged with its pool by the caller.
    pub members: &'a [(String, Vec<Value>)],
    pub monitors: &'a [Value],
    pub networks: &'a [Value],
    pub subnets: &'a [Value],
    pub floating_ips: &'a [Value],
    /// The project's L7 policies; those of other load balancers are dropped.
    pub l7policies: &'a [Value],
    /// Each policy's rules, tagged with the policy by the caller.
    pub l7rules: &'a [(String, Vec<Value>)],
}

/// The load balancer detail page from the load balancer and `x`, the rest.
pub fn load_balancer_detail(l: &Value, x: LbInputs) -> LoadBalancerDetail {
    let pool_names = names_by_id(x.pools);
    let listener_names = names_by_id(x.listeners);
    let monitor_names = names_by_id(x.monitors);
    let subnet_names = names_by_id(x.subnets);
    let net_names = names_by_id(x.networks);
    let pool_ids: std::collections::HashSet<String> = x.pools.iter().filter_map(|p| s(p, "id")).collect();
    let by_name = |a: &String, b: &String| a.to_lowercase().cmp(&b.to_lowercase());

    let mut listeners: Vec<_> = x
        .listeners
        .iter()
        .map(|v| LbListener {
            id: s(v, "id").unwrap_or_default(),
            name: name_or_id(v),
            description: s_ne(v, "description"),
            protocol: s(v, "protocol").unwrap_or_default(),
            port: i(v, "protocol_port"),
            default_pool: s_ne(v, "default_pool_id").map(|id| named(id, &pool_names)),
            connection_limit: i(v, "connection_limit").filter(|&n| n >= 0),
            operating_status: status(v, "operating_status"),
            provisioning_status: status(v, "provisioning_status"),
            admin_up: b(v, "admin_state_up"),
            options: listener_options(v),
            default_tls_container_ref: s_ne(v, "default_tls_container_ref"),
            sni_container_refs: strs(v, "sni_container_refs"),
        })
        .collect();
    listeners.sort_by(|a, b| a.port.cmp(&b.port).then_with(|| by_name(&a.name, &b.name)));

    let mut pools: Vec<_> = x
        .pools
        .iter()
        .map(|v| {
            let sp = v.get("session_persistence").filter(|p| p.is_object());
            let persistence = sp.and_then(|p| {
                let kind = s_ne(p, "type")?;
                Some(match s_ne(p, "cookie_name") {
                    Some(c) => format!("{kind} ({c})"),
                    None => kind,
                })
            });
            LbPool {
                id: s(v, "id").unwrap_or_default(),
                name: name_or_id(v),
                protocol: s(v, "protocol").unwrap_or_default(),
                algorithm: s(v, "lb_algorithm").unwrap_or_default(),
                session_persistence: persistence,
                persistence_type: sp.and_then(|p| s_ne(p, "type")),
                cookie_name: sp.and_then(|p| s_ne(p, "cookie_name")),
                description: s_ne(v, "description"),
                listeners: ids(v, "listeners").into_iter().map(|id| named(id, &listener_names)).collect(),
                members: arr(v, "members").len(),
                monitor: s_ne(v, "healthmonitor_id").map(|id| named(id, &monitor_names)),
                operating_status: status(v, "operating_status"),
                provisioning_status: status(v, "provisioning_status"),
                admin_up: b(v, "admin_state_up"),
                tls_enabled: b(v, "tls_enabled"),
                tls_ciphers: s_ne(v, "tls_ciphers"),
            }
        })
        .collect();
    pools.sort_by(|a, b| by_name(&a.name, &b.name));

    let mut members: Vec<_> = x
        .members
        .iter()
        .flat_map(|(pool_id, list)| list.iter().map(move |v| (pool_id, v)))
        .map(|(pool_id, v)| LbMember {
            id: s(v, "id").unwrap_or_default(),
            name: name_or_id(v),
            pool: named(pool_id.clone(), &pool_names),
            address: s(v, "address").unwrap_or_default(),
            port: i(v, "protocol_port"),
            weight: i(v, "weight"),
            subnet: s_ne(v, "subnet_id").map(|id| named(id, &subnet_names)),
            backup: b(v, "backup"),
            operating_status: status(v, "operating_status"),
            provisioning_status: status(v, "provisioning_status"),
            admin_up: b(v, "admin_state_up"),
            monitor_address: s_ne(v, "monitor_address"),
            monitor_port: i(v, "monitor_port"),
        })
        .collect();
    members.sort_by(|a, b| by_name(&a.pool.name, &b.pool.name).then_with(|| a.address.cmp(&b.address)).then(a.port.cmp(&b.port)));

    // Monitors are listed per project: keep those of this load balancer's pools.
    let mut monitors: Vec<_> = x
        .monitors
        .iter()
        .filter(|v| ids(v, "pools").iter().any(|p| pool_ids.contains(p)))
        .map(|v| {
            let kind = s(v, "type").unwrap_or_default();
            let http = kind.starts_with("HTTP");
            let http_check = kind.starts_with("HTTP").then(|| {
                let method = s_ne(v, "http_method").unwrap_or_else(|| "GET".into());
                let path = s_ne(v, "url_path").unwrap_or_else(|| "/".into());
                let codes = s_ne(v, "expected_codes").unwrap_or_else(|| "200".into());
                format!("{method} {path} → {codes}")
            });
            LbMonitor {
                id: s(v, "id").unwrap_or_default(),
                name: name_or_id(v),
                kind,
                delay: i(v, "delay"),
                timeout: i(v, "timeout"),
                max_retries: i(v, "max_retries"),
                max_retries_down: i(v, "max_retries_down"),
                http_check,
                http_method: s_ne(v, "http_method").filter(|_| http),
                url_path: s_ne(v, "url_path").filter(|_| http),
                expected_codes: s_ne(v, "expected_codes").filter(|_| http),
                pools: ids(v, "pools").into_iter().map(|id| named(id, &pool_names)).collect(),
                operating_status: status(v, "operating_status"),
                provisioning_status: status(v, "provisioning_status"),
                admin_up: b(v, "admin_state_up"),
            }
        })
        .collect();
    monitors.sort_by(|a, b| by_name(&a.name, &b.name));

    let l7policies = l7_policies(x.l7policies, x.l7rules, &listener_names, &pool_names);
    let vip_port_id = s_ne(l, "vip_port_id");
    let fip = vip_port_id.as_ref().and_then(|port| x.floating_ips.iter().find(|f| s(f, "port_id").as_ref() == Some(port)));
    LoadBalancerDetail {
        row: load_balancer_row(l),
        project_id: s(l, "project_id").unwrap_or_default(),
        flavor_id: s_ne(l, "flavor_id"),
        vip_network: s_ne(l, "vip_network_id").map(|id| named(id, &net_names)),
        vip_subnet: s_ne(l, "vip_subnet_id").map(|id| named(id, &subnet_names)),
        floating_ip: fip.and_then(|f| s(f, "floating_ip_address")),
        floating_ip_id: fip.and_then(|f| s(f, "id")),
        vip_port_id,
        created: s_ne(l, "created_at"),
        updated: s_ne(l, "updated_at"),
        listeners,
        pools,
        members,
        monitors,
        l7policies,
    }
}

/// The L7 policies of this load balancer's listeners (`listener_names` holds
/// exactly those), with their rules, by listener and then position.
fn l7_policies(policies: &[Value], rules: &[(String, Vec<Value>)], listener_names: &HashMap<String, String>, pool_names: &HashMap<String, String>) -> Vec<LbL7Policy> {
    let mut out: Vec<LbL7Policy> = policies
        .iter()
        .filter(|p| s(p, "listener_id").is_some_and(|l| listener_names.contains_key(&l)))
        .map(|p| {
            let id = s(p, "id").unwrap_or_default();
            let mut policy_rules: Vec<LbL7Rule> = rules
                .iter()
                .filter(|(pid, _)| *pid == id)
                .flat_map(|(_, r)| r.iter())
                .map(|r| LbL7Rule {
                    id: s(r, "id").unwrap_or_default(),
                    rule_type: s(r, "type").unwrap_or_default(),
                    compare_type: s(r, "compare_type").unwrap_or_default(),
                    key: s_ne(r, "key"),
                    value: s(r, "value").unwrap_or_default(),
                    invert: b(r, "invert"),
                    operating_status: status(r, "operating_status"),
                    provisioning_status: status(r, "provisioning_status"),
                    admin_up: b(r, "admin_state_up"),
                })
                .collect();
            policy_rules.sort_by(|a, b| (&a.rule_type, &a.value).cmp(&(&b.rule_type, &b.value)));
            LbL7Policy {
                name: name_or_id(p),
                description: s_ne(p, "description"),
                listener: named(s(p, "listener_id").unwrap_or_default(), listener_names),
                action: s(p, "action").unwrap_or_default(),
                position: i(p, "position"),
                redirect_pool: s_ne(p, "redirect_pool_id").map(|id| named(id, pool_names)),
                redirect_url: s_ne(p, "redirect_url"),
                operating_status: status(p, "operating_status"),
                provisioning_status: status(p, "provisioning_status"),
                admin_up: b(p, "admin_state_up"),
                rules: policy_rules,
                id,
            }
        })
        .collect();
    out.sort_by(|a, b| (&a.listener.name, a.position).cmp(&(&b.listener.name, b.position)));
    out
}

// --- Alarming (Aodh, roadmap 79) -------------------------------------------------

/// Aodh `GET v2/alarms` → the *Alarms* list, by name.
pub fn alarms(raw: &[Value]) -> Vec<AlarmRow> {
    let mut out: Vec<_> = raw.iter().map(|x| alarm_row(x, &alarm_rule(x))).collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

fn alarm_row(x: &Value, rule: &AlarmRule) -> AlarmRow {
    let id = s(x, "alarm_id").unwrap_or_default();
    AlarmRow {
        name: s_ne(x, "name").unwrap_or_else(|| id.clone()),
        id,
        description: s_ne(x, "description"),
        alarm_type: s(x, "type").unwrap_or_default(),
        state: s(x, "state").unwrap_or_default(),
        state_reason: s_ne(x, "state_reason"),
        state_since: s_ne(x, "state_timestamp"),
        severity: s_ne(x, "severity"),
        // Aodh defaults to enabled; a missing field is not "disabled".
        enabled: x.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        condition: condition(rule),
        updated: s_ne(x, "timestamp"),
        spec: alarm_spec(x),
    }
}

/// The alarm as *Edit Alarm* starts from: Aodh's PUT replaces the whole alarm,
/// so everything the form doesn't show (time constraints) is carried over.
fn alarm_spec(x: &Value) -> AlarmSpec {
    let kind = s(x, "type").unwrap_or_default();
    let rule = x.get(format!("{kind}_rule")).unwrap_or(&Value::Null);
    let text = |k| s(rule, k).unwrap_or_default();
    AlarmSpec {
        name: s(x, "name").unwrap_or_default(),
        description: s(x, "description").unwrap_or_default(),
        severity: s_ne(x, "severity").unwrap_or_else(|| "low".into()),
        enabled: x.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        repeat_actions: b(x, "repeat_actions"),
        alarm_actions: strs(x, "alarm_actions"),
        ok_actions: strs(x, "ok_actions"),
        insufficient_data_actions: strs(x, "insufficient_data_actions"),
        time_constraints: time_constraints(x),
        query: text("query"),
        comparison: text("comparison_operator"),
        threshold: num(rule, "threshold"),
        metric: text("metric"),
        metrics: strs(rule, "metrics"),
        resource_type: text("resource_type"),
        resource_id: text("resource_id"),
        aggregation: text("aggregation_method"),
        granularity: i(rule, "granularity"),
        evaluation_periods: i(rule, "evaluation_periods"),
        event_type: text("event_type"),
        event_query: match alarm_rule(x) {
            AlarmRule::Event { query, .. } => query,
            _ => vec![],
        },
        alarm_type: kind,
    }
}

fn time_constraints(x: &Value) -> Vec<AlarmTimeConstraint> {
    arr(x, "time_constraints")
        .iter()
        .map(|t| AlarmTimeConstraint {
            name: s(t, "name").unwrap_or_default(),
            start: s(t, "start").unwrap_or_default(),
            duration: i(t, "duration"),
            timezone: s_ne(t, "timezone"),
            description: s_ne(t, "description"),
        })
        .collect()
}

/// A number Aodh may send as JSON number or string (`threshold` is a float).
fn num(v: &Value, k: &str) -> Option<f64> {
    match v.get(k)? {
        Value::Number(n) => n.as_f64(),
        Value::String(x) => x.parse().ok(),
        _ => None,
    }
}

/// The alarm's rule, from `<type>_rule`. Gnocchi's three threshold types share
/// one shape; types Moonlight doesn't know are kept as JSON.
fn alarm_rule(x: &Value) -> AlarmRule {
    let kind = s(x, "type").unwrap_or_default();
    let rule = x.get(format!("{kind}_rule")).unwrap_or(&Value::Null);
    match kind.as_str() {
        "gnocchi_resources_threshold" | "gnocchi_aggregation_by_metrics_threshold" | "gnocchi_aggregation_by_resources_threshold" => {
            let metrics = match s_ne(rule, "metric") {
                Some(m) => vec![m],
                None => strs(rule, "metrics"),
            };
            let resource_type = s_ne(rule, "resource_type");
            let resource_id = s_ne(rule, "resource_id");
            // Gnocchi's `instance` resources are Nova servers, with the server's ID.
            let resource_href = match (resource_type.as_deref(), resource_id.as_deref()) {
                (Some("instance"), Some(id)) => Some(format!("/project/instances/{id}")),
                _ => None,
            };
            AlarmRule::Threshold {
                metrics,
                resource_type,
                resource_id,
                resource_href,
                query: s_ne(rule, "query"),
                aggregation: s_ne(rule, "aggregation_method"),
                comparison: s_ne(rule, "comparison_operator"),
                threshold: num(rule, "threshold"),
                granularity: i(rule, "granularity"),
                evaluation_periods: i(rule, "evaluation_periods"),
            }
        }
        "prometheus" => AlarmRule::Prometheus {
            query: s(rule, "query").unwrap_or_default(),
            comparison: s_ne(rule, "comparison_operator"),
            threshold: num(rule, "threshold"),
        },
        "event" => AlarmRule::Event {
            event_type: s(rule, "event_type").unwrap_or_else(|| "*".into()),
            query: arr(rule, "query")
                .iter()
                .map(|q| {
                    let value = q.get("value").map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).unwrap_or_default();
                    format!("{} {} {value}", s(q, "field").unwrap_or_default(), comparison_symbol(&s(q, "op").unwrap_or_else(|| "eq".into())))
                })
                .collect(),
        },
        _ if rule.is_null() => AlarmRule::Raw(String::new()),
        _ => AlarmRule::Raw(serde_json::to_string_pretty(rule).unwrap_or_default()),
    }
}

/// "`threshold` compared": ` > 80`, or nothing without an operator.
fn compared(comparison: &Option<String>, threshold: Option<f64>) -> String {
    match (comparison, threshold) {
        (Some(c), Some(t)) => format!(" {} {t}", comparison_symbol(c)),
        _ => String::new(),
    }
}

/// The list's one-line *Condition*: `mean(cpu) > 80 · instance <id>`, the
/// PromQL query and its threshold, or the event type.
fn condition(rule: &AlarmRule) -> String {
    match rule {
        AlarmRule::Threshold { metrics, resource_type, resource_id, aggregation, comparison, threshold, .. } => {
            let what = metrics.join(", ");
            let what = match aggregation {
                Some(a) => format!("{a}({what})"),
                None => what,
            };
            let on = match (resource_type, resource_id) {
                (Some(t), Some(id)) => format!(" · {t} {id}"),
                (Some(t), None) => format!(" · {t}"),
                _ => String::new(),
            };
            format!("{what}{}{on}", compared(comparison, *threshold))
        }
        AlarmRule::Prometheus { query, comparison, threshold } => format!("{query}{}", compared(comparison, *threshold)),
        AlarmRule::Event { event_type, .. } => event_type.clone(),
        AlarmRule::Raw(_) => "-".into(),
    }
}

/// `GET v2/alarms/{id}` and its history → the alarm detail page.
pub fn alarm_detail(x: &Value, history: &[Value]) -> AlarmDetail {
    let rule = alarm_rule(x);
    let mut history: Vec<_> = history.iter().map(alarm_change).collect();
    history.sort_by(|a, b| b.time.cmp(&a.time));
    AlarmDetail {
        alarm: alarm_row(x, &rule),
        rule,
        alarm_actions: strs(x, "alarm_actions"),
        ok_actions: strs(x, "ok_actions"),
        insufficient_data_actions: strs(x, "insufficient_data_actions"),
        repeat_actions: b(x, "repeat_actions"),
        time_constraints: time_constraints(x),
        project_id: s_ne(x, "project_id"),
        user_id: s_ne(x, "user_id"),
        history,
    }
}

/// One history entry. Aodh's `detail` is JSON *text*: `{"state", "transition_reason"}`
/// for a state transition, the changed fields for a rule change, the whole
/// alarm for a creation (left out: the overview shows it).
fn alarm_change(h: &Value) -> AlarmChange {
    let kind = s(h, "type").unwrap_or_default();
    let detail = s(h, "detail").unwrap_or_default();
    let parsed = serde_json::from_str::<Value>(&detail).ok();
    let (state, detail) = match (kind.as_str(), &parsed) {
        ("state transition", Some(d)) => (s(d, "state"), s_ne(d, "transition_reason")),
        ("rule change", Some(d)) => (None, Some(d.to_string())),
        ("rule change", None) => (None, Some(detail).filter(|x| !x.is_empty())),
        _ => (None, None),
    };
    AlarmChange { time: s(h, "timestamp").unwrap_or_default(), kind, state, detail, user_id: s_ne(h, "user_id") }
}

// --- Admin (roadmap 85) -------------------------------------------------------

/// OpenStack timestamps as RFC 3339 UTC, for the UI's ages. Nova and Cinder
/// send naive UTC with microseconds (`2026-10-30T10:00:00.000000`), Neutron's
/// agents a space instead of `T` (`2026-10-30 10:00:00`); RFC 3339 passes.
pub fn utc_rfc3339(v: &str) -> Option<String> {
    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(v) {
        return Some(t.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    }
    ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"]
        .iter()
        .find_map(|f| chrono::NaiveDateTime::parse_from_str(v, f).ok())
        .map(|n| n.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// Nova's or Cinder's `os-services` → *Compute* / *Block Storage Services*,
/// sorted by binary then host, as Horizon lists them grouped.
pub fn host_services(raw: &[Value]) -> Vec<HostService> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| HostService {
            // A UUID from compute 2.53; older microversions send a number.
            id: s(v, "id").unwrap_or_default(),
            binary: s(v, "binary").unwrap_or_default(),
            host: s(v, "host").unwrap_or_default(),
            zone: s_ne(v, "zone"),
            enabled: s(v, "status").is_some_and(|x| x.eq_ignore_ascii_case("enabled")),
            up: s(v, "state").is_some_and(|x| x.eq_ignore_ascii_case("up")),
            updated_at: s(v, "updated_at").as_deref().and_then(utc_rfc3339),
            disabled_reason: s_ne(v, "disabled_reason"),
        })
        .collect();
    out.sort_by(|a, b| (&a.binary, &a.host).cmp(&(&b.binary, &b.host)));
    out
}

/// Glance `metadefs/namespaces` → *Metadata Definitions*, sorted by name like
/// Horizon's table.
pub fn metadef_namespaces(raw: &[Value]) -> Vec<MetadefNamespace> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|v| {
            let namespace = s(v, "namespace")?;
            Some(MetadefNamespace {
                name: s_ne(v, "display_name").unwrap_or_else(|| namespace.clone()),
                description: s_ne(v, "description"),
                resource_types: arr(v, "resource_type_associations").iter().filter_map(|a| s(a, "name")).collect(),
                public: s(v, "visibility").as_deref() == Some("public"),
                protected: b(v, "protected"),
                namespace,
            })
        })
        .collect();
    out.sort_by_key(|n| n.name.to_lowercase());
    out
}

/// The properties one metadef namespace offers for a resource type (roadmap 25): the body of
/// `GET metadefs/namespaces/{ns}?resource_type=…`. Glance has already put the association's
/// `prefix` on the names in that response (`hw:cpu_policy`), also on an object's properties;
/// applying it again gave `CIM_PASD_CIM_PASD_…` on ttcp-dev. The namespace's own `properties`
/// come first, then each of its `objects`' (titled `Object › Property`, as a namespace groups
/// them). A `readonly` property is skipped, an array or object type is offered as free text, and
/// a key an earlier entry already offers is not repeated. Sorted by title.
pub fn metadef_options(ns: &Value) -> Vec<MetadataOption> {
    let namespace = s_ne(ns, "display_name").or_else(|| s(ns, "namespace")).unwrap_or_default();
    let text = |v: &Value| match v {
        Value::String(x) => Some(x.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    };
    let of = |props: Option<&Value>, group: Option<&str>| -> Vec<MetadataOption> {
        props
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter(|(name, p)| !name.is_empty() && !b(p, "readonly"))
            .map(|(name, p)| {
                let title = s_ne(p, "title").unwrap_or_else(|| name.clone());
                MetadataOption {
                    key: name.clone(),
                    title: group.map_or(title.clone(), |g| format!("{g} › {title}")),
                    description: s_ne(p, "description"),
                    kind: match s(p, "type").as_deref() {
                        Some("integer") => MetadataKind::Integer,
                        Some("number") => MetadataKind::Number,
                        Some("boolean") => MetadataKind::Boolean,
                        _ => MetadataKind::Text,
                    },
                    choices: arr(p, "enum").iter().filter_map(text).collect(),
                    default: p.get("default").and_then(text),
                    namespace: namespace.clone(),
                }
            })
            .collect()
    };
    let mut out = of(ns.get("properties"), None);
    for o in arr(ns, "objects") {
        let name = s_ne(o, "name").unwrap_or_default();
        let group = (!name.is_empty()).then_some(name.as_str());
        out.extend(of(o.get("properties"), group));
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|o| seen.insert(o.key.clone()));
    out.sort_by_key(|o| o.title.to_lowercase());
    out
}

/// Adds Horizon's *Project* and *Host* columns to a Project panel's rows:
/// `raw` is the API list they were mapped from (joined by `id`), `project_key`
/// and `host_key` where that API keeps them, `names` project ID → name.
pub fn owned<T>(rows: Vec<T>, id_of: impl Fn(&T) -> &str, raw: &[Value], project_key: &str, host_key: Option<&str>, names: &HashMap<String, String>) -> Vec<Owned<T>> {
    let by_id = index_by_id(raw);
    rows.into_iter()
        .map(|row| {
            let v = by_id.get(id_of(&row)).copied();
            let project_id = v.and_then(|v| s_ne(v, project_key));
            Owned {
                project: project_id.clone().map(|p| names.get(&p).cloned().unwrap_or(p)).unwrap_or_else(|| "-".into()),
                host: host_key.and_then(|k| v.and_then(|v| s_ne(v, k))),
                project_id,
                row,
            }
        })
        .collect()
}

/// Sorted `(key, value)` pairs of a JSON object; non-string values as JSON.
fn pairs(v: &Value, k: &str) -> Vec<(String, String)> {
    let mut out: Vec<_> = v
        .get(k)
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// Nova `os-hypervisors/detail` (2.53) → *Hypervisor* tab, by hostname.
pub fn hypervisors(raw: &[Value]) -> Vec<HypervisorRow> {
    let n = |v: &Value, k: &str| i(v, k).unwrap_or(0);
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| HypervisorRow {
            // An integer before 2.53, a UUID from it.
            id: s(v, "id").or_else(|| i(v, "id").map(|x| x.to_string())).unwrap_or_default(),
            hostname: s(v, "hypervisor_hostname").unwrap_or_default(),
            kind: s(v, "hypervisor_type").unwrap_or_default(),
            vcpus_used: n(v, "vcpus_used"),
            vcpus: n(v, "vcpus"),
            ram_used_mb: n(v, "memory_mb_used"),
            ram_mb: n(v, "memory_mb"),
            disk_used_gb: n(v, "local_gb_used"),
            disk_gb: n(v, "local_gb"),
            instances: n(v, "running_vms"),
            up: s(v, "state").as_deref() == Some("up"),
            enabled: s(v, "status").as_deref() == Some("enabled"),
        })
        .collect();
    out.sort_by(|a, b| a.hostname.cmp(&b.hostname));
    out
}

/// A Placement provider (an entry of `resource_providers`) with its
/// `inventories` and `usages` responses: VCPU, MEMORY_MB and DISK_GB. A class
/// missing from the inventory is `None`; a missing usage is 0.
pub fn resource_provider(p: &Value, inventories: &Value, usages: &Value) -> ResourceProviderRow {
    let class = |name: &str| {
        let inv = inventories.get("inventories")?.get(name)?;
        Some(ProviderResource {
            used: usages.get("usages").and_then(|u| u.get(name)).and_then(Value::as_i64).unwrap_or(0),
            reserved: i(inv, "reserved").unwrap_or(0),
            total: i(inv, "total").unwrap_or(0),
            allocation_ratio: inv.get("allocation_ratio").and_then(Value::as_f64).unwrap_or(1.0),
        })
    };
    const SHOWN: [&str; 4] = ["VCPU", "MEMORY_MB", "DISK_GB", "VGPU"];
    let mut others: Vec<(String, ProviderResource)> = inventories
        .get("inventories")
        .and_then(Value::as_object)
        .map(|m| m.keys().filter(|k| !SHOWN.contains(&k.as_str())).filter_map(|k| Some((k.clone(), class(k)?))).collect())
        .unwrap_or_default();
    others.sort_by(|a, b| a.0.cmp(&b.0));
    let uuid = s(p, "uuid").unwrap_or_default();
    ResourceProviderRow {
        name: s_ne(p, "name").unwrap_or_else(|| uuid.clone()),
        uuid,
        vcpus: class("VCPU"),
        memory_mb: class("MEMORY_MB"),
        disk_gb: class("DISK_GB"),
        // Placement 1.14; `null` for a compute node.
        parent: s_ne(p, "parent_provider_uuid"),
        depth: 0,
        host: String::new(),
        descendants: 0,
        vgpus: class("VGPU"),
        others,
        custom_traits: vec![],
    }
}

/// A provider's `traits` response → its `CUSTOM_*` traits, sorted (the standard
/// ones, `COMPUTE_*`, `HW_*`…, say nothing about a vGPU's type).
pub fn custom_traits(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = strs(v, "traits").into_iter().filter(|t| t.starts_with("CUSTOM_")).collect();
    out.sort();
    out
}

/// Nova `os-aggregates` → *Host Aggregates*, by name.
pub fn aggregates(raw: &[Value]) -> Vec<AggregateRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| {
            let mut hosts = strs(v, "hosts");
            hosts.sort();
            AggregateRow {
                id: s(v, "id").or_else(|| i(v, "id").map(|x| x.to_string())).unwrap_or_default(),
                name: s(v, "name").unwrap_or_default(),
                zone: s_ne(v, "availability_zone"),
                hosts,
                metadata: pairs(v, "metadata"),
            }
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Nova `os-availability-zone/detail` → Horizon's *Availability Zones* table:
/// each zone's hosts (the keys of `hosts`), by name.
pub fn zones(raw: &[Value]) -> Vec<ZoneRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| {
            let mut hosts: Vec<String> = v.get("hosts").and_then(Value::as_object).map(|m| m.keys().cloned().collect()).unwrap_or_default();
            hosts.sort();
            ZoneRow {
                name: s(v, "zoneName").unwrap_or_default(),
                hosts,
                available: v.get("zoneState").is_some_and(|z| b(z, "available")),
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Nova `flavors/detail?is_public=None` → *Flavors*, by name. Extra specs
/// come with the flavor from compute 2.61.
pub fn flavor_rows(raw: &[Value]) -> Vec<FlavorRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| FlavorRow {
            id: s(v, "id").unwrap_or_default(),
            name: name_or_id(v),
            vcpus: i(v, "vcpus").unwrap_or(0),
            ram_mb: i(v, "ram").unwrap_or(0),
            disk_gb: i(v, "disk").unwrap_or(0),
            ephemeral_gb: i(v, "OS-FLV-EXT-DATA:ephemeral").unwrap_or(0),
            // `""` when there is no swap.
            swap_mb: i(v, "swap").unwrap_or(0),
            rxtx_factor: v.get("rxtx_factor").and_then(Value::as_f64),
            public: v.get("os-flavor-access:is_public").and_then(Value::as_bool).unwrap_or(true),
            extra_specs: pairs(v, "extra_specs"),
            access: vec![],
        })
        .collect();
    out.sort_by_key(|f| f.name.to_lowercase());
    out
}

/// A flavor's `flavor_access` entries (`{"flavor_id", "tenant_id"}`) as
/// project names, sorted; unnamed projects keep their ID.
pub fn flavor_access(raw: &[Value], names: &HashMap<String, String>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = raw
        .iter()
        .filter_map(|a| s(a, "tenant_id"))
        .map(|id| (id.clone(), names.get(&id).cloned().unwrap_or(id)))
        .collect();
    out.sort_by_key(|(_, n)| n.to_lowercase());
    out
}

/// Cinder `types/{id}/encryption` → its spec; `None` for `{}` (not encrypted)
/// or a body without a provider. `key_size` may come as a number or a string.
pub fn encryption_spec(v: &Value) -> Option<EncryptionSpec> {
    let provider = s_ne(v, "provider")?;
    let key_size = v.get("key_size").and_then(|k| k.as_u64().or_else(|| k.as_str().and_then(|s| s.parse().ok()))).map(|k| k as u32);
    Some(EncryptionSpec { provider, control_location: s(v, "control_location").unwrap_or_else(|| "front-end".into()), cipher: s_ne(v, "cipher"), key_size })
}

/// Cinder `types` → *Volume Types*: `encryption` holds each encrypted type's
/// ID and spec, `qos` the QoS specs (for names).
pub fn volume_type_rows(raw: &[Value], encryption: &[(String, EncryptionSpec)], qos: &[Value]) -> Vec<VolumeTypeRow> {
    let qos_names = names_by_id(qos);
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| {
            let id = s(v, "id").unwrap_or_default();
            VolumeTypeRow {
                name: name_or_id(v),
                description: s_ne(v, "description"),
                qos: s_ne(v, "qos_specs_id").map(|q| Ref { name: qos_names.get(&q).cloned().unwrap_or_else(|| q.clone()), id: q }),
                encryption: encryption.iter().find(|(t, _)| *t == id).map(|(_, e)| e.clone()),
                encrypted: encryption.iter().any(|(t, _)| *t == id),
                public: v.get("os-volume-type-access:is_public").or_else(|| v.get("is_public")).and_then(Value::as_bool).unwrap_or(true),
                extra_specs: pairs(v, "extra_specs"),
                id,
            }
        })
        .collect();
    out.sort_by_key(|t| t.name.to_lowercase());
    out
}

/// Cinder `qos-specs` → Horizon's *QoS Specs* table.
pub fn qos_specs(raw: &[Value]) -> Vec<QosSpecRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| QosSpecRow { id: s(v, "id").unwrap_or_default(), name: name_or_id(v), consumer: s(v, "consumer").unwrap_or_default(), specs: pairs(v, "specs") })
        .collect();
    out.sort_by_key(|q| q.name.to_lowercase());
    out
}

/// Cinder `group_types` → *Group Types*.
pub fn group_type_rows(raw: &[Value]) -> Vec<GroupTypeRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| GroupTypeRow {
            id: s(v, "id").unwrap_or_default(),
            name: name_or_id(v),
            description: s_ne(v, "description"),
            public: v.get("is_public").and_then(Value::as_bool).unwrap_or(true),
            specs: pairs(v, "group_specs"),
        })
        .collect();
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

/// Neutron `rbac-policies` → *RBAC Policies*, projects and networks named.
/// `objects` holds, per RBAC `object_type` (`network`, `qos_policy`,
/// `security_group`), the list that names its objects; an object of another
/// type, or one the list lacks, keeps its ID.
pub fn rbac_policies(raw: &[Value], objects: &[(&str, &[Value])], names: &HashMap<String, String>) -> Vec<RbacPolicyRow> {
    let by_type: HashMap<&str, HashMap<String, String>> = objects.iter().map(|(ty, list)| (*ty, names_by_id(list))).collect();
    let project = |id: Option<String>| id.map(|p| names.get(&p).cloned().unwrap_or(p)).unwrap_or_else(|| "-".into());
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| {
            let object_type = s(v, "object_type").unwrap_or_default();
            let object_id = s(v, "object_id").unwrap_or_default();
            let object_name = by_type.get(object_type.as_str()).and_then(|m| m.get(&object_id)).cloned().unwrap_or_else(|| object_id.clone());
            let target = s(v, "target_tenant").or_else(|| s(v, "target_project"));
            RbacPolicyRow {
                id: s(v, "id").unwrap_or_default(),
                project: project(s_ne(v, "project_id").or_else(|| s_ne(v, "tenant_id"))),
                object: Ref { id: object_id, name: object_name },
                target_id: target.clone().unwrap_or_default(),
                target: match target.as_deref() {
                    Some("*") => "*".into(),
                    _ => project(target),
                },
                action: s(v, "action").unwrap_or_default(),
                object_type,
            }
        })
        .collect();
    out.sort_by(|a, b| (&a.project, &a.object_type, &a.object.name).cmp(&(&b.project, &b.object_type, &b.object.name)));
    out
}

/// Neutron `agents` → *Network Agents*, sorted by type then host.
pub fn network_agents(raw: &[Value]) -> Vec<NetworkAgent> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| NetworkAgent {
            agent_type: s(v, "agent_type").unwrap_or_default(),
            binary: s(v, "binary").unwrap_or_default(),
            host: s(v, "host").unwrap_or_default(),
            zone: s_ne(v, "availability_zone"),
            admin_up: b(v, "admin_state_up"),
            alive: b(v, "alive"),
            heartbeat: s(v, "heartbeat_timestamp").as_deref().and_then(utc_rfc3339),
        })
        .collect();
    out.sort_by(|a, b| (&a.agent_type, &a.host).cmp(&(&b.agent_type, &b.host)));
    out
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;


    // --- Orchestration (roadmap 62) ---

    #[test]
    fn stacks_newest_first_with_name_fallback() {
        let raw = vec![
            json!({"id": "s1", "stack_name": "old", "stack_status": "CREATE_COMPLETE", "stack_status_reason": "Stack CREATE completed successfully",
                   "creation_time": "2026-01-01T10:00:00Z", "updated_time": null, "description": "No description"}),
            json!({"id": "s2", "stack_name": "", "stack_status": "UPDATE_IN_PROGRESS", "creation_time": "2026-02-01T10:00:00Z",
                   "updated_time": "2026-02-02T10:00:00Z", "description": ""}),
        ];
        let out = stacks(&raw);
        assert_eq!(out.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["s2", "old"], "newest first, unnamed → ID");
        assert_eq!(out[0].updated.as_deref(), Some("2026-02-02T10:00:00Z"));
        assert_eq!(out[1].updated, None, "null updated_time");
        assert_eq!(out[0].description, None, "empty description");
        assert!(stack_busy(&out[0].status) && !stack_busy(&out[1].status));
    }

    #[test]
    fn stack_detail_hides_heat_parameters_and_keeps_output_errors() {
        let stack = json!({"stack": {
            "id": "s1", "stack_name": "web", "stack_status": "CREATE_FAILED", "creation_time": "2026-01-01T10:00:00Z",
            "parameters": {"OS::stack_id": "s1", "OS::stack_name": "web", "OS::project_id": "p1", "flavor": "m1.small", "password": "******", "count": 2},
            "outputs": [
                {"output_key": "url", "output_value": "http://10.0.0.5", "description": "Site"},
                {"output_key": "ips", "output_value": ["10.0.0.5", "10.0.0.6"], "description": "No description given"},
                {"output_key": "broken", "output_value": null, "output_error": "Resource server failed"}
            ],
            "timeout_mins": 60, "disable_rollback": false, "parent": null, "tags": ["a", "b"]
        }});
        let template = json!({"heat_template_version": "2018-08-31", "resources": {}});
        let d = stack_detail(&stack, &[], &[], &template);
        assert_eq!(d.parameters, [("count".into(), "2".into()), ("flavor".into(), "m1.small".into()), ("password".into(), "******".into())], "OS::* hidden, sorted, numbers as text");
        assert_eq!(d.outputs[0].key, "broken");
        assert_eq!((d.outputs[0].value.as_str(), d.outputs[0].error.as_deref()), ("-", Some("Resource server failed")));
        assert_eq!(d.outputs[1].value, r#"["10.0.0.5","10.0.0.6"]"#, "non-strings as JSON");
        assert_eq!(d.outputs[2].value, "http://10.0.0.5");
        assert_eq!((d.timeout_mins, d.disable_rollback, d.parent.clone()), (Some(60), false, None));
        assert_eq!(d.tags, ["a", "b"]);
        assert!(d.template.contains("\"heat_template_version\": \"2018-08-31\""), "pretty JSON");
        assert_eq!(stack_detail(&stack["stack"], &[], &[], &Value::Null).stack.name, "web", "unwrapped too");
    }

    #[test]
    fn stack_resources_link_and_invert_dependencies() {
        let raw = vec![
            json!({"resource_name": "server", "physical_resource_id": "vm1", "resource_type": "OS::Nova::Server", "resource_status": "CREATE_COMPLETE",
                   "updated_time": "2026-01-01T10:01:00Z", "required_by": []}),
            json!({"resource_name": "port", "physical_resource_id": "p1", "resource_type": "OS::Neutron::Port", "resource_status": "CREATE_COMPLETE",
                   "creation_time": "2026-01-01T10:00:00Z", "required_by": ["server", "fip"]}),
            json!({"resource_name": "fip", "physical_resource_id": "", "resource_type": "OS::Neutron::FloatingIP", "resource_status": "INIT_COMPLETE",
                   "required_by": []}),
            json!({"resource_name": "cert", "physical_resource_id": "https://kms:9311/v1/secrets/abc", "resource_type": "OS::Barbican::Secret", "required_by": []}),
            json!({"resource_name": "group", "physical_resource_id": "nested1", "resource_type": "OS::Heat::ResourceGroup", "required_by": []}),
            json!({"resource_name": "custom", "physical_resource_id": "x1", "resource_type": "OS::Zaqar::Queue", "required_by": []}),
        ];
        let out = stack_resources(&raw);
        let by = |n: &str| out.iter().find(|r| r.name == n).unwrap();
        assert_eq!(by("server").depends_on, ["port"]);
        assert_eq!(by("fip").depends_on, ["port"]);
        assert_eq!(by("port").required_by, ["fip", "server"], "sorted");
        assert_eq!(by("port").updated, "2026-01-01T10:00:00Z", "creation_time when never updated");
        assert_eq!(by("server").href.as_deref(), Some("/project/instances/vm1"));
        assert_eq!(by("port").href.as_deref(), Some("/project/networks/ports/p1"));
        assert_eq!((by("fip").physical_id.clone(), by("fip").href.clone()), (None, None), "not created yet");
        assert_eq!(by("cert").href.as_deref(), Some("/project/key_manager/secrets/abc"), "Barbican refs");
        assert_eq!(by("group").href.as_deref(), Some("/project/stacks/nested1"), "nested stack");
        assert_eq!(by("custom").href, None, "no Moonlight page");
        assert_eq!(resource_href("templates/web.yaml", "n2").as_deref(), Some("/project/stacks/n2"), "provider template");
    }

    #[test]
    fn stack_events_newest_first() {
        let raw = vec![
            json!({"id": "e1", "event_time": "2026-01-01T10:00:00Z", "resource_name": "web", "resource_status": "CREATE_IN_PROGRESS",
                   "resource_status_reason": "Stack CREATE started", "physical_resource_id": "s1"}),
            json!({"id": "e2", "event_time": "2026-01-01T10:02:00Z", "logical_resource_id": "server", "resource_status": "CREATE_FAILED",
                   "resource_status_reason": "Quota exceeded", "physical_resource_id": ""}),
        ];
        let out = stack_events(&raw);
        assert_eq!(out.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["e2", "e1"]);
        assert_eq!(out[0].resource_name, "server", "logical_resource_id when resource_name is missing");
        assert_eq!(out[0].physical_id, None);
    }

    #[test]
    fn resource_types_with_or_without_descriptions() {
        let out = resource_types(&[
            json!({"resource_type": "OS::Nova::Server", "description": "A server."}),
            json!("AWS::EC2::Instance"),
            json!({"resource_type": "OS::Heat::None", "description": ""}),
        ]);
        let pairs: Vec<_> = out.iter().map(|r| (r.name.as_str(), r.description.as_deref())).collect();
        assert_eq!(pairs, [("AWS::EC2::Instance", None), ("OS::Heat::None", None), ("OS::Nova::Server", Some("A server."))]);
    }

    #[test]
    fn resource_type_schema_tables() {
        let d = resource_type_detail(&json!({
            "resource_type": "OS::Nova::Server",
            "description": "A server.",
            "support_status": {"status": "SUPPORTED", "version": "2014.1", "message": null},
            "properties": {
                "name": {"type": "string", "required": false, "update_allowed": true, "description": "Name."},
                "flavor": {"type": "string", "required": true, "update_allowed": true,
                           "constraints": [{"custom_constraint": "nova.flavor"}]},
                "count": {"type": "integer", "default": 1, "required": false, "update_allowed": false,
                          "constraints": [{"range": {"min": 0}, "description": "At least zero."}, {"allowed_values": [1, 2]}]}
            },
            "attributes": {"networks": {"type": "map", "description": "Addresses."}, "show": {"description": "All."}}
        }));
        assert_eq!(d.support.as_deref(), Some("SUPPORTED (2014.1)"));
        assert_eq!(d.properties.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["count", "flavor", "name"]);
        let count = &d.properties[0];
        assert_eq!((count.kind.as_str(), count.required, count.update_allowed, count.default.as_deref()), ("integer", false, false, Some("1")));
        assert_eq!(count.constraints, [r#"range: {"min":0}"#, "allowed_values: [1,2]"], "descriptions left out");
        assert_eq!(d.properties[1].constraints, ["custom_constraint: nova.flavor"]);
        assert_eq!(d.properties[1].default, None);
        assert_eq!(d.attributes[1].kind, None, "old Heat reports no attribute types");
        assert!(d.raw.contains("\"OS::Nova::Server\""));
    }

    #[test]
    fn template_versions_hot_first_newest_first() {
        let out = template_versions(&[
            json!({"version": "AWSTemplateFormatVersion.2010-09-09", "type": "cfn", "aliases": []}),
            json!({"version": "heat_template_version.2013-05-23", "type": "hot", "aliases": []}),
            json!({"version": "heat_template_version.2018-08-31", "type": "hot", "aliases": ["heat_template_version.rocky"]}),
        ]);
        assert_eq!(out.iter().map(|v| v.version.as_str()).collect::<Vec<_>>(),
            ["heat_template_version.2018-08-31", "heat_template_version.2013-05-23", "AWSTemplateFormatVersion.2010-09-09"]);
        assert_eq!(out[0].aliases, ["heat_template_version.rocky"]);
        let f = template_functions(&[json!({"functions": "get_param", "description": "Get a parameter."}), json!({"functions": "get_attr", "description": ""})]);
        assert_eq!(f.iter().map(|x| (x.name.as_str(), x.description.is_some())).collect::<Vec<_>>(), [("get_attr", false), ("get_param", true)]);
    }

    #[test]
    fn lb_zones_are_the_enabled_ones_by_name() {
        let z = lb_az_choices(&[
            json!({"name": "az-b", "description": "Madrid", "enabled": true}),
            json!({"name": "az-a", "enabled": true}),
            json!({"name": "off", "enabled": false}),
        ]);
        assert_eq!(z.iter().map(|c| (c.value.as_str(), c.label.as_str())).collect::<Vec<_>>(), [("az-a", "az-a"), ("az-b", "az-b (Madrid)")]);
    }

    /// Roadmap 54, slice 3: like octavia-dashboard, certificate containers with
    /// a private key, then opaque secrets not in one, with the certificate's expiry.
    #[test]
    fn lb_certificates_are_complete_containers_and_loose_bundles() {
        let b = "https://kms/v1";
        let containers = [
            json!({"container_ref": format!("{b}/containers/c1"), "name": "web", "type": "certificate",
                   "secret_refs": [{"name": "certificate", "secret_ref": format!("{b}/secrets/crt")}, {"name": "private_key", "secret_ref": format!("{b}/secrets/key")}]}),
            json!({"container_ref": format!("{b}/containers/c2"), "name": "no key", "type": "certificate",
                   "secret_refs": [{"name": "certificate", "secret_ref": format!("{b}/secrets/crt2")}]}),
            json!({"container_ref": format!("{b}/containers/c3"), "name": "generic", "type": "generic", "secret_refs": []}),
        ];
        let secrets = [
            json!({"secret_ref": format!("{b}/secrets/crt"), "name": "crt", "secret_type": "certificate", "expiration": "2027-03-01T00:00:00"}),
            json!({"secret_ref": format!("{b}/secrets/key"), "name": "key", "secret_type": "opaque"}),
            json!({"secret_ref": format!("{b}/secrets/p12"), "name": "bundle", "secret_type": "opaque", "expiration": null}),
            json!({"secret_ref": format!("{b}/secrets/sym"), "name": "aes", "secret_type": "symmetric"}),
        ];
        let c = lb_certificate_choices(&containers, &secrets);
        let got: Vec<(&str, &str, &str)> = c.iter().map(|x| (x.value.as_str(), x.label.as_str(), x.cells[0].as_str())).collect();
        assert_eq!(got, vec![(format!("{b}/containers/c1").as_str(), "web (2027-03-01)", "2027-03-01"), (format!("{b}/secrets/p12").as_str(), "bundle", "")]);
    }

    /// Roadmap 54: Octavia sends `insert_headers` values as strings; only the
    /// `"true"` ones are set. `allowed_cidrs` is absent before Octavia 2.12.
    #[test]
    fn listener_options_read_headers_timeouts_and_cidrs() {
        let v = serde_json::json!({
            "timeout_client_data": 50000, "timeout_tcp_inspect": 0,
            "insert_headers": {"X-Forwarded-Proto": "true", "X-Forwarded-For": "True", "X-Forwarded-Port": "false"},
            "allowed_cidrs": null
        });
        let o = listener_options(&v);
        assert_eq!((o.timeout_client_data, o.timeout_tcp_inspect, o.timeout_member_data), (Some(50_000), Some(0), None));
        assert_eq!(o.insert_headers, vec!["X-Forwarded-For", "X-Forwarded-Proto"]);
        assert_eq!(o.allowed_cidrs, Some(vec![]), "null: offered, none set");
        assert_eq!(listener_options(&serde_json::json!({})).allowed_cidrs, None, "missing: Octavia < 2.12");
    }

    /// (load balancer, listeners, pools, members per pool, monitors)
    type Octavia = (Value, Vec<Value>, Vec<Value>, Vec<(String, Vec<Value>)>, Vec<Value>);

    fn octavia() -> Octavia {
        let lb = json!({
            "id": "lb-1", "name": "web-lb", "description": "", "project_id": "p1",
            "provisioning_status": "ACTIVE", "operating_status": "DEGRADED", "admin_state_up": true,
            "vip_address": "10.0.0.50", "vip_port_id": "port-vip", "vip_subnet_id": "sub-1",
            "vip_network_id": "net-1", "provider": "amphora", "flavor_id": "", "availability_zone": null,
            "listeners": [{"id": "ls-443"}, {"id": "ls-80"}], "pools": [{"id": "pool-1"}],
            "created_at": "2026-10-01T10:00:00", "updated_at": null
        });
        let listeners = vec![
            json!({"id": "ls-443", "name": "https", "protocol": "TERMINATED_HTTPS", "protocol_port": 443,
                   "default_pool_id": "pool-1", "connection_limit": -1, "provisioning_status": "ACTIVE",
                   "operating_status": "ONLINE", "admin_state_up": true}),
            json!({"id": "ls-80", "name": "", "protocol": "HTTP", "protocol_port": 80, "default_pool_id": null,
                   "connection_limit": 5000, "provisioning_status": "ACTIVE", "operating_status": "ONLINE",
                   "admin_state_up": false}),
        ];
        let pools = vec![json!({
            "id": "pool-1", "name": "web", "protocol": "HTTP", "lb_algorithm": "ROUND_ROBIN",
            "session_persistence": {"type": "APP_COOKIE", "cookie_name": "JSESSIONID"},
            "listeners": [{"id": "ls-443"}], "members": [{"id": "m-1"}, {"id": "m-2"}],
            "healthmonitor_id": "hm-1", "provisioning_status": "ACTIVE", "operating_status": "DEGRADED",
            "admin_state_up": true
        })];
        let members = vec![("pool-1".to_string(), vec![
            json!({"id": "m-2", "name": "", "address": "10.0.0.12", "protocol_port": 8080, "weight": 1,
                   "subnet_id": "sub-1", "backup": false, "provisioning_status": "ACTIVE",
                   "operating_status": "ERROR", "admin_state_up": true,
                   "monitor_address": "10.0.9.12", "monitor_port": 9000}),
            json!({"id": "m-1", "name": "web-1", "address": "10.0.0.11", "protocol_port": 8080, "weight": 10,
                   "subnet_id": null, "backup": true, "provisioning_status": "ACTIVE",
                   "operating_status": "ONLINE", "admin_state_up": true}),
        ])];
        let monitors = vec![
            json!({"id": "hm-1", "name": "", "type": "HTTP", "delay": 5, "timeout": 3, "max_retries": 3,
                   "max_retries_down": 3, "http_method": "GET", "url_path": "/healthz", "expected_codes": "200-204",
                   "pools": [{"id": "pool-1"}], "provisioning_status": "ACTIVE", "operating_status": "ONLINE",
                   "admin_state_up": true}),
            json!({"id": "hm-other", "name": "other-lb", "type": "TCP", "pools": [{"id": "pool-x"}]}),
        ];
        (lb, listeners, pools, members, monitors)
    }

    #[test]
    fn flavors_follow_the_configured_order() {
        let f = |id: &str, name: &str, ram: i64, vcpus: i64| json!({"id": id, "name": name, "ram": ram, "vcpus": vcpus, "disk": 10});
        let raw = vec![f("3", "m1.large", 8192, 4), f("1", "m1.small", 2048, 1), f("2", "c1.big", 4096, 8)];
        let names = |sort, rev| flavor_choices(&raw, sort, rev).into_iter().map(|c| c.cells[0].clone()).collect::<Vec<_>>();
        use crate::prefs::FlavorSort::*;
        assert_eq!(names(Ram, false), ["m1.small", "c1.big", "m1.large"], "default: by RAM");
        assert_eq!(names(Ram, true), ["m1.large", "c1.big", "m1.small"]);
        assert_eq!(names(Vcpus, false), ["m1.small", "m1.large", "c1.big"]);
        assert_eq!(names(Id, false), ["m1.small", "c1.big", "m1.large"]);
    }

    #[test]
    fn trunks_name_their_parent_and_subports() {
        let raw = vec![json!({"id": "t1", "name": "", "port_id": "p0", "status": "ACTIVE", "admin_state_up": true,
                              "sub_ports": [{"port_id": "p2", "segmentation_type": "vlan", "segmentation_id": 200},
                                            {"port_id": "p1", "segmentation_type": "vlan", "segmentation_id": 100}]})];
        let ports = vec![json!({"id": "p0", "name": "parent"}), json!({"id": "p1", "name": "vlan100"})];
        let t = &trunks(&raw, &ports)[0];
        assert_eq!((t.name.as_str(), t.parent.name.as_str(), t.status.as_str()), ("t1", "parent", "ACTIVE"), "unnamed: the ID");
        let sp: Vec<_> = t.subports.iter().map(|s| (s.port_name.as_str(), s.segmentation_id)).collect();
        assert_eq!(sp, [("vlan100", Some(100)), ("p2", Some(200))], "by VLAN; unknown port: its ID");
    }

    #[test]
    fn firewall_rules_read_null_as_any_and_name_their_policies() {
        let rules = vec![
            json!({"id": "r1", "name": "ssh", "protocol": "tcp", "ip_version": 4, "source_ip_address": null, "destination_ip_address": "10.0.0.0/24",
                   "source_port": null, "destination_port": "22", "action": "allow", "enabled": true, "shared": false, "project_id": "p1",
                   "firewall_policy_id": ["fp1"]}),
            json!({"id": "r2", "name": "", "protocol": null, "ip_version": 6, "action": "deny", "enabled": false, "shared": true, "tenant_id": "other",
                   "firewall_policy_id": null}),
        ];
        let policies = vec![json!({"id": "fp1", "name": "web", "firewall_rules": ["r1"]}), json!({"id": "fp2", "name": "", "firewall_rules": ["r2", "r1"]})];
        let r = firewall_rules(&rules, &policies, "p1");
        assert_eq!((r[0].name.as_str(), r[0].protocol.as_deref(), r[0].source_ip.as_deref(), r[0].destination_port.as_deref()), ("r2", None, None, None), "unnamed: the ID; null: any");
        assert_eq!((r[0].owned, r[0].shared, r[0].enabled, r[0].ip_version), (false, true, false, 6), "tenant_id on older Neutron");
        assert_eq!(r[0].policies, [Ref { id: "fp2".into(), name: "fp2".into() }], "listed by a policy, though its own field is null");
        assert_eq!((r[1].destination_ip.as_deref(), r[1].destination_port.as_deref(), r[1].owned), (Some("10.0.0.0/24"), Some("22"), true));
        let names: Vec<_> = r[1].policies.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["web", "fp2"], "its own field and every policy that lists it, each once");
        let one = firewall_rules(&[json!({"id": "r3", "firewall_policy_id": "fp1"})], &[], "p1");
        assert_eq!(one[0].policies[0].id, "fp1", "a single ID on some releases");
    }

    #[test]
    fn firewall_policies_keep_rule_order() {
        let p = firewall_policies(
            &[json!({"id": "fp1", "name": "web", "firewall_rules": ["r2", "r1", "gone"], "audited": true, "shared": false, "project_id": "p1"})],
            &[json!({"id": "r1", "name": "ssh"}), json!({"id": "r2", "name": "http"})],
            "p1",
        );
        let rules: Vec<_> = p[0].rules.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(rules, ["http", "ssh", "gone"], "evaluation order, not sorted; unknown rule: its ID");
        assert!(p[0].audited && p[0].owned && !p[0].shared);
    }

    #[test]
    fn firewall_rule_choices_leave_action_and_protocol_to_the_columns() {
        let c = firewall_rule_choices(&[
            json!({"id": "r1", "name": "ssh", "action": "allow", "protocol": "tcp"}),
            json!({"id": "r2", "name": "", "action": "deny", "protocol": null}),
        ]);
        let ssh = c.iter().find(|c| c.value == "r1").unwrap();
        assert_eq!((ssh.label.as_str(), ssh.cells.clone()), ("ssh", vec!["ssh".to_string(), "allow".into(), "tcp".into()]), "no English in the label");
        let any = c.iter().find(|c| c.value == "r2").unwrap();
        assert_eq!(any.cells, ["r2", "deny", ""], "unnamed: its ID; any protocol: empty");
    }

    #[test]
    fn firewall_groups_resolve_ports_to_their_router_or_instance() {
        let groups = vec![json!({"id": "g1", "name": "gw", "ingress_firewall_policy_id": "fp1", "egress_firewall_policy_id": null,
                                 "ports": ["prt", "pvm", "pgone"], "status": "PENDING_UPDATE", "admin_state_up": true, "project_id": "p1"})];
        let ports = vec![
            json!({"id": "prt", "name": "", "device_owner": "network:router_interface", "device_id": "r1", "fixed_ips": [{"ip_address": "10.0.0.1"}]}),
            json!({"id": "pvm", "name": "vm-port", "device_owner": "compute:nova", "device_id": "s1", "fixed_ips": [{"ip_address": "10.0.0.5"}]}),
        ];
        let g = &firewall_groups(&groups, &[json!({"id": "fp1", "name": "web"})], &ports, &[json!({"id": "r1", "name": "router1"})], &[json!({"id": "s1", "name": "vm1"})], "p1")[0];
        assert_eq!((g.ingress.as_ref().map(|p| p.name.as_str()), g.egress.as_ref()), (Some("web"), None));
        assert!(g.busy() && g.owned);
        let p: Vec<_> = g.ports.iter().map(|p| (p.port.name.as_str(), p.kind.as_deref(), p.device.as_ref().map(|d| d.name.as_str()), p.device_href())).collect();
        assert_eq!(
            p,
            [
                ("prt", Some("router"), Some("router1"), Some("/project/routers/r1".into())),
                ("vm-port", Some("instance"), Some("vm1"), Some("/project/instances/s1".into())),
                ("pgone", None, None, None),
            ]
        );
    }

    #[test]
    fn firewall_port_choices_are_router_and_instance_ports_with_their_group() {
        let mut ports = vec![
            json!({"id": "prt", "device_owner": "network:router_interface_distributed", "device_id": "r1", "fixed_ips": [{"ip_address": "10.0.0.1"}]}),
            json!({"id": "pvm", "device_owner": "compute:az1", "device_id": "s1", "fixed_ips": []}),
            json!({"id": "pdhcp", "device_owner": "network:dhcp", "device_id": "d"}),
            json!({"id": "pgw", "device_owner": "network:router_gateway", "device_id": "r1"}),
        ];
        ports.push(json!({"id": "pvm2", "name": "web-nic", "device_owner": "compute:nova", "device_id": "s2", "fixed_ips": [{"ip_address": "10.0.0.7"}, {"ip_address": "fd00::7"}]}));
        let c = firewall_port_choices(&ports, &[json!({"id": "g1", "ports": ["pvm"]})], &[json!({"id": "r1", "name": "router1"})], &[json!({"id": "s2", "name": "vm2"})]);
        let got: Vec<_> = c.iter().map(|c| (c.value.as_str(), c.label.as_str(), c.cells.clone())).collect();
        assert_eq!(
            got,
            [
                ("prt", "router1: 10.0.0.1", vec!["router1".to_string(), "router".into(), "10.0.0.1".into(), "prt".into()]),
                ("pvm2", "vm2: 10.0.0.7, fd00::7", vec!["vm2".to_string(), "instance".into(), "10.0.0.7, fd00::7".into(), "web-nic".into()]),
            ],
            "no DHCP or gateway ports, none another group has; the device named, an unnamed port by its ID's start"
        );
    }

    #[test]
    fn qos_policies_own_and_shared_with_rules() {
        let raw = vec![
            json!({"id": "q2", "name": "shared-silver", "project_id": "other", "shared": true, "is_default": false, "rules": []}),
            json!({"id": "q1", "name": "gold", "project_id": "p1", "shared": false, "is_default": true, "description": "",
                   "rules": [{"id": "r1", "type": "bandwidth_limit", "max_kbps": 10000, "max_burst_kbps": 0, "direction": "egress"},
                             {"id": "r2", "type": "dscp_marking", "dscp_mark": 26}]}),
        ];
        let p = qos_policies(&raw, "p1");
        assert_eq!((p[0].name.as_str(), p[0].owned, p[0].is_default, p[0].description.clone()), ("gold", true, true, None));
        assert_eq!((p[1].owned, p[1].shared), (false, true));
        assert_eq!((p[0].rules[0].kind.as_str(), p[0].rules[0].max_kbps, p[0].rules[1].dscp_mark), ("bandwidth_limit", Some(10000), Some(26)));
        assert_eq!(qos_policy_choices(&raw).iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["gold", "shared-silver"]);
    }

    #[test]
    fn load_balancer_list_rows() {
        let (lb, ..) = octavia();
        let unnamed = json!({"id": "lb-0", "name": "", "admin_state_up": false});
        let rows = load_balancers(&[lb, unnamed]);
        assert_eq!(rows[0].name, "lb-0", "unnamed: falls back to the ID");
        assert_eq!(rows[0].operating_status, "UNKNOWN");
        let r = &rows[1];
        assert_eq!((r.vip_address.as_deref(), r.availability_zone.as_deref()), (Some("10.0.0.50"), None));
        assert_eq!((r.operating_status.as_str(), r.provisioning_status.as_str()), ("DEGRADED", "ACTIVE"));
        assert!(r.admin_up && r.description.is_none());
    }

    #[test]
    fn load_balancer_detail_joins_listeners_pools_members_and_monitors() {
        let (lb, listeners, pools, members, monitors) = octavia();
        let networks = vec![json!({"id": "net-1", "name": "web-net"})];
        let subnets = vec![json!({"id": "sub-1", "name": "web-subnet"})];
        let fips = vec![json!({"floating_ip_address": "203.0.113.7", "port_id": "port-vip"}),
                        json!({"floating_ip_address": "203.0.113.8", "port_id": "other"})];
        // L7 (roadmap 54): the project's policies, one on another load balancer's listener.
        let (ls_id, pool_id) = (listeners[0]["id"].as_str().unwrap().to_string(), pools[0]["id"].as_str().unwrap().to_string());
        let l7policies = vec![
            json!({"id": "pol-1", "name": "api", "listener_id": ls_id, "action": "REDIRECT_TO_POOL", "position": 1,
                   "redirect_pool_id": pool_id, "redirect_url": null, "admin_state_up": true,
                   "operating_status": "ONLINE", "provisioning_status": "ACTIVE"}),
            json!({"id": "pol-x", "name": "elsewhere", "listener_id": "other-listener", "action": "REJECT"}),
        ];
        let l7rules = vec![("pol-1".to_string(), vec![
            json!({"id": "r1", "type": "PATH", "compare_type": "STARTS_WITH", "key": null, "value": "/api", "invert": false, "admin_state_up": true}),
            json!({"id": "r2", "type": "HEADER", "compare_type": "EQUAL_TO", "key": "X-Env", "value": "test", "invert": true, "admin_state_up": true}),
        ])];
        let d = load_balancer_detail(&lb, LbInputs {
            listeners: &listeners, pools: &pools, members: &members, monitors: &monitors,
            networks: &networks, subnets: &subnets, floating_ips: &fips,
            l7policies: &l7policies, l7rules: &l7rules,
        });
        assert_eq!(d.l7policies.len(), 1, "only this load balancer's listeners");
        let pol = &d.l7policies[0];
        assert_eq!((pol.redirect_pool.as_ref().map(|p| p.name.as_str()), pol.position, pol.rules.len()), (Some("web"), Some(1), 2));
        assert_eq!((pol.rules[0].rule_type.as_str(), pol.rules[0].key.as_deref(), pol.rules[0].invert), ("HEADER", Some("X-Env"), true));
        assert_eq!(d.floating_ip.as_deref(), Some("203.0.113.7"));
        assert_eq!(d.vip_network.as_ref().map(|r| r.name.as_str()), Some("web-net"));
        assert_eq!(d.vip_subnet.as_ref().map(|r| r.name.as_str()), Some("web-subnet"));
        assert_eq!((d.flavor_id.clone(), d.updated.clone()), (None, None));

        let ports: Vec<_> = d.listeners.iter().map(|l| (l.port, l.name.as_str())).collect();
        assert_eq!(ports, [(Some(80), "ls-80"), (Some(443), "https")], "by port; unnamed → ID");
        assert_eq!(d.listeners[1].default_pool.as_ref().map(|p| p.name.as_str()), Some("web"));
        assert_eq!((d.listeners[1].connection_limit, d.listeners[0].connection_limit), (None, Some(5000)), "-1 = unlimited");

        let p = &d.pools[0];
        assert_eq!(p.session_persistence.as_deref(), Some("APP_COOKIE (JSESSIONID)"));
        assert_eq!((p.members, p.listeners[0].name.as_str()), (2, "https"));
        assert_eq!(p.monitor.as_ref().map(|m| m.name.as_str()), Some("hm-1"));

        let addrs: Vec<_> = d.members.iter().map(|m| m.address.as_str()).collect();
        assert_eq!(addrs, ["10.0.0.11", "10.0.0.12"]);
        assert_eq!((d.members[0].pool.name.as_str(), d.members[0].backup, d.members[0].subnet.clone()), ("web", true, None));
        assert_eq!(d.members[1].subnet.as_ref().map(|s| s.name.as_str()), Some("web-subnet"));
        assert_eq!((d.members[1].monitor_address.as_deref(), d.members[1].monitor_port), (Some("10.0.9.12"), Some(9000)));
        assert_eq!((d.members[0].monitor_address.clone(), d.members[0].monitor_port), (None, None), "absent: the member itself");

        assert_eq!(d.monitors.len(), 1, "monitors of other load balancers are dropped");
        assert_eq!(d.monitors[0].http_check.as_deref(), Some("GET /healthz → 200-204"));
        assert_eq!(d.monitors[0].pools[0].name, "web");
    }

    fn server_fixture() -> Value {
        json!({
            "id": "3f2a9c1e-0000-0000-0000-000000000c91",
            "name": "web-prod-01",
            "status": "ACTIVE",
            "tenant_id": "p1",
            "created": "2026-09-14T10:22:00Z",
            "updated": "2026-10-01T08:00:00Z",
            "key_name": "ops-2026",
            "locked": false,
            "image": {"id": "img-rhel"},
            "flavor": {"id": "4"},
            "OS-EXT-AZ:availability_zone": "nova",
            "OS-EXT-STS:power_state": 1,
            "OS-EXT-STS:task_state": null,
            "OS-EXT-SRV-ATTR:host": "compute-03",
            "addresses": {
                "zeta-net": [{"addr": "10.9.0.5", "OS-EXT-IPS:type": "fixed"}],
                "ecommerce-net": [
                    {"addr": "10.0.12.21", "version": 4, "OS-EXT-IPS:type": "fixed", "OS-EXT-IPS-MAC:mac_addr": "fa:16:3e:4a:12:21"},
                    {"addr": "203.0.113.41", "version": 4, "OS-EXT-IPS:type": "floating"}
                ]
            },
            "security_groups": [{"name": "web"}, {"name": "default"}, {"name": "web"}],
            "os-extended-volumes:volumes_attached": [{"id": "vol-1"}, {"id": "vol-gone"}],
            "metadata": {"role": "frontend", "env": "prod"}
        })
    }

    fn flavors() -> Vec<Value> {
        vec![json!({"id": "4", "name": "m1.large", "vcpus": 4, "ram": 8192, "disk": 40})]
    }
    fn images_fixture() -> Vec<Value> {
        vec![json!({"id": "img-rhel", "name": "rhel-9.4"})]
    }

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
        assert_eq!(ips, vec![("10.0.12.21", false), ("203.0.113.41", true), ("10.9.0.5", false)]);
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
        assert_eq!(vm.name, "3f2a9c1e-0000-0000-0000-000000000c91", "unnamed falls back to id");

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
    fn detail_joins_volumes_interfaces_actions_and_fault() {
        let mut sv = server_fixture();
        sv["fault"] = json!({"code": 500, "message": "No valid host was found."});
        let volumes = vec![json!({"id": "vol-1", "name": "data", "size": 100,
            "attachments": [{"server_id": "3f2a9c1e-0000-0000-0000-000000000c91", "device": "/dev/vdb"}]})];
        let networks = vec![json!({"id": "net-1", "name": "ecommerce-net"})];
        let interfaces = vec![json!({"port_id": "port-1", "net_id": "net-1", "mac_addr": "fa:16:3e:4a:12:21",
            "port_state": "ACTIVE", "fixed_ips": [{"ip_address": "10.0.12.21", "subnet_id": "s"}]})];
        let actions = vec![
            json!({"action": "create", "request_id": "req-1", "start_time": "2026-09-14T10:22:00Z", "user_id": "u1"}),
            json!({"action": "stop", "request_id": "req-2", "start_time": "2026-10-01T08:00:00Z", "user_id": "u1", "message": null}),
        ];
        let d = instance_detail(DetailInputs {
            server: &sv,
            flavors: &flavors(),
            images: &images_fixture(),
            volumes: &volumes,
            networks: &networks,
            interfaces: &interfaces,
            ports: &[json!({"id": "port-1", "name": "eth0", "admin_state_up": true, "binding:vnic_type": "normal",
                "security_groups": ["g-def"]})],
            actions: &actions,
            security_groups: &[
                json!({"id": "g-def", "name": "default", "security_group_rules": [
                    {"direction": "ingress", "ethertype": "IPv4", "remote_group_id": "g-def"},
                    {"direction": "egress", "ethertype": "IPv6", "remote_ip_prefix": "::/0"}]}),
            ],
            server_groups: &[
                json!({"id": "sg-other", "name": "db", "members": ["x"]}),
                json!({"id": "sg-web", "name": "web-spread", "members": ["3f2a9c1e-0000-0000-0000-000000000c91"]}),
            ],
        });
        assert_eq!(d.server_group, Some(("sg-web".into(), "web-spread".into())), "found by membership (no 2.71 field)");
        assert_eq!(d.security_groups, vec!["default", "web"], "sorted and deduplicated");
        assert_eq!(
            d.security_group_rules,
            vec![("default".into(), vec!["ingress IPv4 any ← group default".into(), "egress IPv6 any → ::/0".into()])],
            "rules by group, remote groups by name; `web` isn't listed by the project"
        );
        assert_eq!(d.flavor.vcpus, Some(4));
        assert_eq!(d.volumes[0].name.as_deref(), Some("data"));
        assert_eq!(d.volumes[0].device.as_deref(), Some("/dev/vdb"));
        assert_eq!(d.volumes[1].name, None, "unknown volume keeps its id only");
        assert_eq!(d.interfaces[0].network_name.as_deref(), Some("ecommerce-net"));
        assert_eq!(d.interfaces[0].fixed_ips, vec!["10.0.12.21"]);
        let port = d.interfaces[0].port.as_ref().expect("merged from the port list");
        assert_eq!((port.name.as_str(), port.admin_up, port.vnic_type.as_deref()), ("eth0", true, Some("normal")));
        assert!(port.port_security, "missing port_security_enabled means on (no extension)");
        assert_eq!(port.security_group_ids, vec!["g-def"]);
        assert_eq!(d.actions[0].action, "stop", "newest action first");
        assert_eq!(d.fault.unwrap().code, 500);
        assert_eq!(d.metadata[0], ("env".into(), "prod".into()));
        assert_eq!(d.host.as_deref(), Some("compute-03"));
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
        let raw = vec![json!({"keypair": {"name": "cert", "fingerprint": "ee", "type": "x509"}}), json!({"keypair": {"name": "old", "fingerprint": "ff"}})];
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
    fn domains_mark_the_context() {
        let d = identity_domains(&[json!({"id": "d2", "name": "Zeta", "enabled": false}), json!({"id": "d1", "name": "ttcp-spain"})], Some("d2"));
        assert_eq!((d[0].name.as_str(), d[0].enabled, d[0].context), ("ttcp-spain", true, false));
        assert_eq!((d[1].name.as_str(), d[1].enabled, d[1].context), ("Zeta", false, true));
    }

    #[test]
    fn role_assignments_name_their_scope() {
        let rows = role_assignment_rows(&[
            json!({"role": {"id": "r1", "name": "member"}, "user": {"id": "u"}, "scope": {"project": {"id": "p1", "name": "pokemon"}}}),
            json!({"role": {"id": "r2", "name": "admin"}, "user": {"id": "u"}, "scope": {"domain": {"id": "d1", "name": "ttcp-spain"}, "OS-INHERIT:inherited_to": "projects"}}),
            json!({"role": {"id": "r3", "name": "reader"}, "user": {"id": "u"}, "scope": {"system": {"all": true}}}),
        ]);
        assert_eq!((rows[0].scope_kind.as_str(), rows[0].scope.as_str(), rows[0].inherited), ("domain", "ttcp-spain", true));
        assert_eq!((rows[1].scope.as_str(), rows[1].project_id.as_deref()), ("pokemon", Some("p1")));
        assert_eq!((rows[2].scope_kind.as_str(), rows[2].scope.as_str()), ("system", "all"));
        let a = app_credential_detail(&json!({"application_credential": {"id": "a", "name": "ci", "roles": [], "unrestricted": true,
            "access_rules": [{"id": "x", "service": "compute", "method": "GET", "path": "/v2.1/servers"}]}}));
        assert_eq!((a.credential.unrestricted, a.access_rules[0].path.as_str()), (true, "/v2.1/servers"));
        let g = identity_group_detail(&json!({"group": {"id": "g", "name": "ops", "domain_id": "d1"}}), &[json!({"id": "u1", "name": "ana"})], &HashMap::new());
        assert_eq!((g.group.name.as_str(), g.domain_id.as_deref(), g.users[0].name.as_str()), ("ops", Some("d1"), "ana"));
    }

    #[test]
    fn project_members_join_direct_and_group_roles() {
        let assignments = [
            json!({"role": {"id": "r1", "name": "member"}, "user": {"id": "u1", "name": "ana"}, "scope": {"project": {"id": "p"}}}),
            json!({"role": {"id": "r2", "name": "reader"}, "user": {"id": "u1", "name": "ana"}, "scope": {"project": {"id": "p"}}}),
            json!({"role": {"id": "r3", "name": "admin"}, "group": {"id": "g1", "name": "ops"}, "scope": {"project": {"id": "p"}}}),
            json!({"role": {"id": "r4"}, "group": {"id": "g1", "name": "ops"}, "scope": {"project": {"id": "p"}}}),
        ];
        let (users, groups) = project_members(&assignments, &[("g1".into(), vec![json!({"id": "u1", "name": "ana"}), json!({"id": "u2", "name": "bo"})])]);
        assert_eq!(groups, [ProjectGroup { id: "g1".into(), name: "ops".into(), roles: vec!["admin".into(), "r4".into()], role_ids: vec!["r3".into(), "r4".into()] }], "a nameless role keeps its ID");
        assert_eq!(users[0].role_ids, ["r1", "r2"], "IDs kept for Manage Members");
        assert_eq!((users[0].name.as_str(), users[0].roles.clone()), ("ana", vec!["member".to_string(), "reader".into()]));
        assert_eq!(users[0].group_roles, [("admin".to_string(), "ops".to_string()), ("r4".into(), "ops".into())], "also through the group");
        assert_eq!((users[1].name.as_str(), users[1].roles.len(), users[1].group_roles.len()), ("bo", 0, 2), "only through the group");
    }

    #[test]
    fn user_detail_reads_options_primary_project_and_extras() {
        let domains = domain_names(&[], Some(("d1".into(), "ttcp-spain".into())));
        let projects = HashMap::from([("p1".to_string(), "admin2".to_string())]);
        let d = identity_user_detail(
            &json!({"user": {"id": "u1", "name": "admin2", "domain_id": "d1", "enabled": true, "default_project_id": "p1",
                "password_expires_at": null, "options": {"lock_password": true}, "links": {"self": "x"}, "department": "ops", "phone": null}}),
            &domains,
            &projects,
        );
        assert_eq!((d.user.name.as_str(), d.user.domain.as_str(), d.domain_id.as_deref()), ("admin2", "ttcp-spain", Some("d1")));
        assert_eq!((d.password_expires_at.clone(), d.lock_password), (None, true), "null = never expires");
        assert_eq!(d.primary_project.map(|p| p.name), Some("admin2".into()));
        assert_eq!(d.extra, [("department".to_string(), "ops".to_string())], "unknown attributes shown; nulls and links not");
        let other = identity_user_detail(&json!({"id": "u2", "name": "x", "default_project_id": "p9"}), &domains, &projects);
        assert_eq!((other.primary_project.map(|p| p.name), other.lock_password), (Some("p9".into()), false), "an unknown project keeps its ID");
    }

    #[test]
    fn identity_lists_follow_keystone_quirks() {
        let domains = domain_names(&[], Some(("d1".into(), "ttcp-spain".into())));
        let p = identity_projects(
            &[
                json!({"id": "p2", "name": "pokemon", "description": "", "domain_id": "d1", "enabled": true}),
                json!({"id": "p1", "name": "admin2", "description": "admin Project", "domain_id": "d9"}),
                json!({"id": "p3", "name": "Zeta", "domain_id": "d1", "enabled": false}),
            ],
            &domains,
            "p2",
            &["p1".into(), "p2".into()],
        );
        assert_eq!(p.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["admin2", "pokemon", "Zeta"]);
        assert_eq!((p[0].enabled, p[0].domain.as_str(), p[0].member, p[0].current), (true, "d9", true, false), "`enabled` absent = true; unknown domain keeps its ID");
        assert_eq!((p[1].description.clone(), p[1].domain.as_str(), p[1].current), (None, "ttcp-spain", true), "empty description = none");
        assert!(!p[2].enabled && !p[2].member, "not the user's: no *Set as Active Project*");

        let u = identity_users(&[json!({"user": {"id": "u1", "name": "zc01001", "domain_id": "d1", "password_expires_at": null}}), json!({"id": "u2", "name": "", "email": "a@example.com", "enabled": false})], &domains);
        assert_eq!((u[0].name.as_str(), u[0].email.clone()), ("u2", Some("a@example.com".into())), "a nameless user falls back to its ID");
        assert_eq!((u[1].name.as_str(), u[1].email.clone(), u[1].domain.as_str(), u[1].enabled), ("zc01001", None, "ttcp-spain", true), "`user::get` comes wrapped");

        let a = app_credentials(&[json!({"id": "a1", "name": "poke", "project_id": "p2", "expires_at": null,
            "roles": [{"id": "r1", "name": "power_user"}, {"id": "r2"}], "unrestricted": false})]);
        assert_eq!((a[0].expires_at.clone(), a[0].roles.clone()), (None, vec!["power_user".to_string(), "r2".into()]), "never expires; a role without a name keeps its ID");
        let roles = identity_roles(
            &[json!({"id": "r", "name": "reader"}), json!({"id": "a", "name": "admin"}), json!({"id": "m", "name": "member"})],
            &[json!({"prior_role": {"id": "a", "name": "admin"}, "implies": [{"id": "m", "name": "member"}]}),
              json!({"prior_role": {"id": "m", "name": "member"}, "implies": [{"id": "r", "name": "reader"}]})],
        );
        assert_eq!((roles[0].name.as_str(), roles[0].implies.clone()), ("admin", vec!["member".to_string()]), "Keystone's default chain");
        assert!(roles[2].implies.is_empty(), "reader implies nothing");
        assert_eq!(identity_groups(&[json!({"id": "g", "name": "ops", "domain_id": "d1"})], &domains)[0].domain, "ttcp-spain");
    }

    #[test]
    fn server_group_detail_names_members_and_reads_rules() {
        let servers = vec![json!({"id": "a", "name": "web-1", "status": "ACTIVE"}), json!({"id": "b", "name": "", "status": "SHUTOFF"})];
        // compute 2.64: one `policy` and its `rules` (numbers), wrapped as the GET returns it.
        let d = server_group_detail(
            &json!({"server_group": {"id": "g1", "name": "spread", "policy": "anti-affinity",
                "rules": {"max_server_per_host": 2}, "members": ["a", "b", "gone"],
                "project_id": "p1", "user_id": "u1"}}),
            &servers,
        );
        assert_eq!(d.group.policies, vec!["anti-affinity"]);
        assert_eq!(d.rules, vec![("max_server_per_host".into(), "2".into())], "numbers shown as text");
        assert_eq!((d.project_id.as_deref(), d.user_id.as_deref()), (Some("p1"), Some("u1")));
        assert_eq!(d.members[0], GroupMember { id: "a".into(), name: Some("web-1".into()), status: Some("ACTIVE".into()) });
        assert_eq!(d.members[1].name, None, "an empty name falls back to the ID in the page");
        assert_eq!((d.members[2].name.clone(), d.members[2].status.clone()), (None, None), "a member missing from the list keeps only its ID");
        // Before 2.64: `policies`, no `rules`; empty `project_id` means unknown.
        let d = server_group_detail(&json!({"id": "g2", "name": "", "policies": ["affinity"], "members": [], "project_id": ""}), &servers);
        assert_eq!((d.group.name.as_str(), d.group.policies.clone()), ("g2", vec!["affinity".to_string()]));
        assert!(d.rules.is_empty() && d.members.is_empty() && d.project_id.is_none());
    }

    #[test]
    fn failed_import_reason_comes_from_os_glance_failed_import() {
        let ok = json!({"id": "a", "status": "queued", "os_glance_failed_import": ""});
        let bad = json!({"id": "b", "status": "queued", "os_glance_failed_import": "Unable to download: 404"});
        let out = images(&[ok, bad], "p", &HashMap::new());
        let by = |id: &str| out.iter().find(|i| i.id == id).unwrap().import_failed.clone();
        assert_eq!(by("a"), None, "an empty value is not a failure");
        assert_eq!(by("b").as_deref(), Some("Unable to download: 404"));
    }

    #[test]
    fn images_put_project_ones_first_and_detect_snapshots() {
        let raw = vec![
            json!({"id": "1", "name": "ubuntu", "owner": "other", "visibility": "public", "size": 1024}),
            json!({"id": "2", "name": "backup", "owner": "p1", "visibility": "private", "image_type": "snapshot", "protected": true}),
        ];
        let out = images(&raw, "p1", &HashMap::new());
        assert_eq!(out[0].name, "backup");
        assert!(out[0].is_snapshot && out[0].protected && out[0].owned_by_project);
        assert_eq!(out[1].size, Some(1024));
    }

    #[test]
    fn volumes_resolve_server_names_and_string_booleans() {
        let raw = vec![json!({"id": "v1", "name": "", "size": 10, "status": "in-use", "bootable": "true",
            "encrypted": false, "attachments": [{"server_id": "s1", "device": "/dev/vda"}]})];
        let servers = vec![json!({"id": "s1", "name": "web"})];
        let v = &volumes(&raw, &servers)[0];
        assert_eq!(v.name, "v1");
        assert!(v.bootable && !v.encrypted);
        assert_eq!(v.attachments[0].server_name.as_deref(), Some("web"));
    }

    #[test]
    fn backups_name_their_volume_and_survive_missing_fields() {
        let raw = [
            json!({"id": "b1", "name": "", "status": "available", "size": 5, "volume_id": "v1", "is_incremental": true, "container": "volumebackups", "created_at": "2026-01-01T00:00:00", "description": ""}),
            json!({"id": "b2", "name": "new", "status": "error", "size": 1, "volume_id": "gone", "fail_reason": "no space", "snapshot_id": null, "created_at": "2026-02-01T00:00:00"}),
        ];
        let rows = backups(&raw, &[json!({"id": "v1", "name": "data"})]);
        assert_eq!(rows[0].id, "b2", "newest first");
        assert_eq!((rows[0].volume.name.as_str(), rows[0].incremental, rows[0].fail_reason.as_deref()), ("gone", false, Some("no space")), "deleted volume: ID shown");
        assert_eq!((rows[1].name.as_str(), rows[1].volume.name.as_str(), rows[1].description.clone()), ("b1", "data", None), "unnamed: ID; \"\" is no description");
        assert!(rows[1].incremental);
        assert_eq!(backups(&raw, &[])[1].volume.name, "v1");
    }

    #[test]
    fn volume_groups_resolve_type_names_and_snapshots() {
        let groups = [
            json!({"id": "g1", "name": "db", "description": "", "status": "available", "availability_zone": "nova",
                   "group_type": "gt1", "volume_types": ["vt1", "vt-gone"], "created_at": "2026-10-01T10:00:00.000000"}),
            json!({"id": "g2", "name": null, "status": "creating", "group_type": "gt-gone", "volume_types": []}),
        ];
        let types = [json!({"id": "gt1", "name": "default"})];
        let vtypes = [json!({"id": "vt1", "name": "ceph"})];
        let snaps = [json!({"id": "gs1", "name": "nightly", "group_id": "g1", "group_type_id": "gt1", "status": "available", "created_at": "2026-10-02T10:00:00.000000"})];
        let rows = volume_groups(&groups, &types, &vtypes, &snaps);
        assert_eq!(rows.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["db", "g2"], "unnamed: the ID, sorted by name");
        let db = &rows[0];
        assert_eq!((db.description.as_deref(), db.group_type.name.as_str()), (None, "default"), "\"\" description is none");
        assert_eq!(db.volume_types.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["ceph", "vt-gone"], "unknown type: its ID");
        assert!(db.has_snapshots && !rows[1].has_snapshots);
        assert_eq!(rows[1].group_type.name, "gt-gone");
        // Without the type lists (a user Cinder refuses them to) every name is an ID.
        assert_eq!(volume_groups(&groups, &[], &[], &[])[0].group_type.name, "gt1");
    }

    #[test]
    fn volume_group_detail_keeps_only_its_volumes_and_snapshots() {
        let g = json!({"id": "g1", "name": "db", "status": "available", "group_type": "gt1", "volume_types": ["vt1"]});
        let vols = [
            json!({"id": "v1", "name": "data", "size": 10, "status": "in-use", "group_id": "g1", "bootable": "false"}),
            json!({"id": "v2", "name": "other", "size": 1, "status": "available", "group_id": null}),
        ];
        let snaps = [
            json!({"id": "gs1", "group_id": "g1", "group_type_id": "gt1", "status": "available"}),
            json!({"id": "gs2", "group_id": "g9", "status": "available"}),
        ];
        let d = volume_group_detail(&g, &[], &[], &snaps, &vols);
        assert_eq!(d.volumes.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["v1"]);
        assert_eq!(d.snapshots.iter().map(|s| (s.id.as_str(), s.group.name.as_str())).collect::<Vec<_>>(), [("gs1", "db")]);
        assert!(d.row.has_snapshots);
    }

    #[test]
    fn group_snapshots_name_their_group_and_list_their_volume_snapshots() {
        let gs = json!({"id": "gs1", "name": "", "description": "before upgrade", "status": "available", "group_id": "g1",
                        "group_type_id": "gt1", "created_at": "2026-10-02T10:00:00.000000"});
        let groups = [json!({"id": "g1", "name": "db"})];
        let rows = group_snapshots_rows(&[gs.clone(), json!({"id": "gs2", "group_id": "g-gone", "status": "error"})], &groups, &[]);
        assert_eq!(rows.iter().map(|r| r.group.name.as_str()).collect::<Vec<_>>(), ["db", "g-gone"], "newest first; a deleted group: its ID");
        assert_eq!((rows[0].name.as_str(), rows[0].group_type.name.as_str()), ("gs1", "gt1"), "\"\" name: the ID");
        let snaps = [
            json!({"id": "s1", "name": "data-snap", "volume_id": "v1", "size": 10, "status": "available", "group_snapshot_id": "gs1"}),
            json!({"id": "s2", "volume_id": "v1", "size": 10, "status": "available"}),
        ];
        let d = group_snapshot_detail(&gs, &groups, &[], &snaps, &[json!({"id": "v1", "name": "data"})]);
        assert_eq!(d.snapshots.iter().map(|s| (s.id.as_str(), s.volume_name.as_deref())).collect::<Vec<_>>(), [("s1", Some("data"))]);
    }

    #[test]
    fn networks_without_subnets_lists_those_whose_subnets_were_not_fetched() {
        let nets = [
            json!({"id": "own", "subnets": ["s1"]}),
            json!({"id": "public", "router:external": true, "subnets": ["ext1", "ext2"]}),
            json!({"id": "empty", "subnets": []}),
            json!({"id": "no-key"}),
        ];
        let subnets = [json!({"id": "s1"}), json!({"id": "ext1"})];
        assert_eq!(networks_without_subnets(&nets, &subnets), ["public"], "one missing subnet is enough; none listed: nothing to fetch");
    }

    #[test]
    fn group_volume_choices_offer_members_and_volumes_the_group_can_take() {
        let groups = [json!({"id": "g1", "volume_types": ["vt-ssd"]}), json!({"id": "g2", "volume_types": ["vt-hdd"]})];
        let types = [json!({"id": "vt-ssd", "name": "ssd"}), json!({"id": "vt-hdd", "name": "hdd"})];
        let vols = [
            json!({"id": "v1", "name": "in-g1", "size": 10, "status": "in-use", "volume_type": "hdd", "group_id": "g1"}),
            json!({"id": "v2", "name": "free-ssd", "size": 5, "status": "available", "volume_type": "ssd", "group_id": null}),
            json!({"id": "v3", "name": "busy-ssd", "size": 5, "status": "creating", "volume_type": "ssd"}),
            json!({"id": "v4", "name": "in-g2", "size": 5, "status": "available", "volume_type": "ssd", "group_id": "g2"}),
            // From 3.61 volumes also carry the type's ID.
            json!({"id": "v5", "name": "by-id", "size": 1, "status": "available", "volume_type": "x", "volume_type_id": "vt-hdd"}),
        ];
        let c = group_volume_choices(&groups, &vols, &types);
        let of = |g: &str| c.iter().filter(|x| x.cells == [g]).map(|x| x.value.as_str()).collect::<Vec<_>>();
        assert_eq!(of("g1"), ["v2", "v1"], "a member stays even with another type; sorted by label");
        assert_eq!(of("g2"), ["v5", "v4"], "type matched by ID; not v3 (creating) nor g1's volumes");
        assert_eq!(c.iter().find(|x| x.value == "v2").unwrap().label, "free-ssd (5 GiB)");
    }

    #[test]
    fn snapshots_resolve_volume_names() {
        let s = snapshots(
            &[json!({"id": "s1", "name": "snap", "size": 5, "volume_id": "v1", "created_at": "2026-01-01"})],
            &[json!({"id": "v1", "name": "data"})],
        );
        assert_eq!(s[0].volume_name.as_deref(), Some("data"));
    }

    #[test]
    fn networks_and_routers_resolve_names() {
        let nets = vec![json!({"id": "n1", "name": "public", "router:external": true, "shared": false,
            "status": "ACTIVE", "admin_state_up": true, "subnets": ["sn1", "sn-unknown"]})];
        let subnets = vec![json!({"id": "sn1", "name": "pub-sub", "cidr": "203.0.113.0/24"})];
        let n = &networks(&nets, &subnets, "p1")[0];
        assert!(n.external);
        assert!(!n.owned, "no project_id: not ours");
        let own = json!({"id": "n2", "name": "mine", "project_id": "p1"});
        assert!(networks(std::slice::from_ref(&own), &[], "p1")[0].owned);
        assert!(!networks(&[own], &[], "p2")[0].owned);
        assert_eq!(n.subnets, vec!["pub-sub 203.0.113.0/24", "sn-unknown"]);
        let r = routers(&[json!({"id": "r1", "name": "gw", "external_gateway_info": {"network_id": "n1"}})], &nets);
        assert_eq!(r[0].external_network.as_deref(), Some("public"));
        assert_eq!((r[0].external_network_id.as_deref(), r[0].snat), (Some("n1"), None), "no enable_snat key");
        let r = routers(
            &[
                json!({"id": "r1", "external_gateway_info": {"network_id": "gone", "enable_snat": false}}),
                json!({"id": "r2", "external_gateway_info": null}),
            ],
            &nets,
        );
        assert_eq!(r[0].external_network.as_deref(), Some("gone"), "unknown network: its ID");
        assert_eq!(r[0].snat, Some(false));
        assert_eq!((r[1].external_network.as_deref(), r[1].snat), (None, None));
    }

    #[test]
    fn security_group_rules_are_summarised() {
        let rule = json!({"direction": "ingress", "ethertype": "IPv4", "protocol": "tcp",
            "port_range_min": 22, "port_range_max": 22, "remote_ip_prefix": "0.0.0.0/0"});
        assert_eq!(describe_rule(&rule), "ingress IPv4 tcp 22 ← 0.0.0.0/0");
        let egress = json!({"direction": "egress", "ethertype": "IPv6"});
        assert_eq!(describe_rule(&egress), "egress IPv6 any → any");
        let range = json!({"direction": "ingress", "ethertype": "IPv4", "protocol": "udp",
            "port_range_min": 1000, "port_range_max": 2000, "remote_group_id": "abcdef0123456789"});
        assert_eq!(describe_rule(&range), "ingress IPv4 udp 1000-2000 ← group abcdef01");
    }

    #[test]
    fn floating_ips_map_to_instances_only_for_compute_ports() {
        let raw = vec![
            json!({"id": "f1", "floating_ip_address": "203.0.113.41", "fixed_ip_address": "10.0.0.5",
                "floating_network_id": "n1", "status": "ACTIVE",
                "port_details": {"device_id": "s1", "device_owner": "compute:nova"}}),
            json!({"id": "f2", "floating_ip_address": "203.0.113.2", "floating_network_id": "n1",
                "status": "DOWN", "port_details": {"device_id": "r1", "device_owner": "network:router_gateway"}}),
        ];
        let out = floating_ips(&raw, &[json!({"id": "n1", "name": "public"})], &[json!({"id": "s1", "name": "web"})]);
        assert_eq!(out[0].ip, "203.0.113.2");
        assert_eq!(out[0].instance_id, None);
        assert_eq!(out[1].instance_name.as_deref(), Some("web"));
        assert_eq!(out[1].pool, "public");
        assert_eq!(out[1].description, None);
    }

    #[test]
    fn floating_ip_description_and_dns_treat_empty_as_missing() {
        let raw = vec![json!({"id": "f1", "floating_ip_address": "203.0.113.9", "floating_network_id": "n1",
            "status": "DOWN", "description": "bastion", "dns_name": "", "dns_domain": "example.org."})];
        let out = floating_ips(&raw, &[], &[]);
        assert_eq!(out[0].description.as_deref(), Some("bastion"));
        assert_eq!(out[0].dns_name, None);
        assert_eq!(out[0].dns_domain.as_deref(), Some("example.org."));
    }

    #[test]
    fn quota_rows_of_every_service_shape() {
        let keys = |r: &[QuotaRow]| r.iter().map(|x| x.key.clone()).collect::<Vec<_>>();
        // Nova detail: the network proxies (pre-2.36) and `id` are dropped.
        let nova = json!({"id": "p1", "cores": {"in_use": 8, "limit": 20, "reserved": 1},
            "instances": {"in_use": 3, "limit": -1, "reserved": 0}, "floating_ips": {"in_use": 0, "limit": 10, "reserved": 0},
            "ram": {"in_use": 16384, "limit": 51200, "reserved": 0}});
        let r = quota_rows(QuotaService::Compute, &nova, &Value::Null);
        assert_eq!(keys(&r), ["instances", "cores", "ram"], "editable order, proxies gone");
        assert_eq!((r[0].used, r[0].limit, r[1].reserved), (Some(3), None, Some(1)));
        // Cinder with usage: per-type keys follow the known ones, by type.
        let cinder = json!({"id": "p1", "volumes_ceph": {"limit": -1, "in_use": 1, "reserved": 0, "allocated": 0},
            "gigabytes": {"limit": 1000, "in_use": 250, "reserved": 0, "allocated": 0},
            "gigabytes_ceph": {"limit": 500, "in_use": 20, "reserved": 0, "allocated": 0}, "volumes": {"limit": 10, "in_use": 2, "reserved": 0}});
        let r = quota_rows(QuotaService::Volume, &cinder, &Value::Null);
        assert_eq!(keys(&r), ["volumes", "gigabytes", "volumes_ceph", "gigabytes_ceph"], "grouped by type");
        assert_eq!((r[1].used, r[1].limit), (Some(250), Some(1000)));
        // Neutron details say `used`.
        let r = quota_rows(QuotaService::Network, &json!({"port": {"used": 4, "limit": 500, "reserved": 0}}), &Value::Null);
        assert_eq!((r[0].used, r[0].limit), (Some(4), Some(500)));
        // Octavia: bare limits, `null` = the default, deprecated aliases dropped, no usage.
        let lb = json!({"load_balancer": 5, "loadbalancer": 5, "listener": null, "member": -1, "health_monitor": 3, "healthmonitor": 3});
        let r = quota_rows(QuotaService::LoadBalancer, &lb, &json!({"listener": 20}));
        assert_eq!(keys(&r), ["load_balancer", "listener", "member", "health_monitor"], "current names, as ttcp-dev's Octavia returns");
        assert_eq!((r[0].used, r[0].limit, r[1].limit, r[2].limit), (None, Some(5), Some(20), None));
        assert!(quota_rows(QuotaService::Network, &Value::Null, &Value::Null).is_empty());
    }

    #[test]
    fn key_manager_quotas_take_usage_from_list_totals() {
        let q = json!({"quotas": {"secrets": 10, "orders": -1, "containers": 5, "consumers": -1, "cas": 2}});
        let secrets = json!({"secrets": [{}], "total": 3});
        let r = key_manager_quota_rows(&q, &[("secrets", &secrets), ("containers", &Value::Null), ("orders", &json!({"orders": [], "total": 0}))]);
        let got: Vec<_> = r.iter().map(|x| (x.key.as_str(), x.used, x.limit)).collect();
        assert_eq!(
            got,
            [("secrets", Some(3), Some(10)), ("containers", None, Some(5)), ("orders", Some(0), None), ("consumers", None, None), ("cas", None, Some(2))],
            "editable order; a failed list leaves usage unknown"
        );
    }

    #[test]
    fn quotas_treat_minus_one_as_unlimited() {
        let nova = json!({"absolute": {"maxTotalInstances": -1, "totalInstancesUsed": 3,
            "maxTotalCores": 20, "totalCoresUsed": 8, "maxTotalRAMSize": 51200, "totalRAMUsed": 16384}});
        let q = compute_quotas(&nova);
        assert_eq!((q[0].used, q[0].limit), (3, None));
        assert_eq!((q[1].used, q[1].limit), (8, Some(20)));
        assert_eq!(q[2].unit, QuotaUnit::Mib);

        let neutron = json!({"port": {"used": 0, "limit": 500, "reserved": 0}, "security_group": {"used": 1, "limit": -1}});
        let q = network_quotas(&neutron);
        let port = q.iter().find(|x| x.key == QuotaKey::Ports).unwrap();
        assert_eq!(port.limit, Some(500));
        let sg = q.iter().find(|x| x.key == QuotaKey::SecurityGroups).unwrap();
        assert_eq!((sg.used, sg.limit), (1, None));
        // a missing resource is reported as 0 used, unlimited
        let routers = q.iter().find(|x| x.key == QuotaKey::Routers).unwrap();
        assert_eq!((routers.used, routers.limit), (0, None));

        let cinder = json!({"absolute": {"maxTotalVolumeGigabytes": 1000, "totalGigabytesUsed": 250}});
        let v = volume_quotas(&cinder);
        assert_eq!(v[2].unit, QuotaUnit::Gib);
        assert_eq!((v[2].used, v[2].limit), (250, Some(1000)));
    }


    #[test]
    fn ports_resolve_devices_subnets_and_groups() {
        let l = PortLookups::new(
            &[json!({"id": "n1", "name": "net"})],
            &[json!({"id": "sn1", "name": "sub"})],
            &[json!({"id": "s1", "name": "web"})],
            &[json!({"id": "g1", "name": "default"})],
        );
        let p = l.port(&json!({"id": "p1", "network_id": "n1", "mac_address": "fa:16", "status": "ACTIVE",
            "admin_state_up": true, "device_owner": "compute:alh1", "device_id": "s1", "security_groups": ["g1"],
            "fixed_ips": [{"ip_address": "10.0.0.5", "subnet_id": "sn1"}], "binding:vnic_type": "normal"}));
        assert_eq!(p.network_name.as_deref(), Some("net"));
        assert_eq!(p.fixed_ips, vec!["10.0.0.5 (sub)"]);
        assert_eq!(p.device_name.as_deref(), Some("web"));
        assert_eq!(p.security_groups, vec!["default"]);
        let router_port = l.port(&json!({"id": "p2", "device_owner": "network:router_interface", "device_id": "s1"}));
        assert_eq!(router_port.device_name, None, "only compute ports map to instances");
    }

    #[test]
    fn network_detail_keeps_only_its_subnets() {
        let net = json!({"id": "n1", "name": "net", "mtu": 1450, "project_id": "p", "subnets": ["a"]});
        let subs = vec![
            json!({"id": "a", "network_id": "n1", "cidr": "10.0.0.0/24", "allocation_pools": [{"start": "10.0.0.2", "end": "10.0.0.254"}], "enable_dhcp": true}),
            json!({"id": "b", "network_id": "other", "cidr": "192.168.0.0/24"}),
        ];
        let d = network_detail(&net, &subs, vec![], "p1");
        assert_eq!(d.subnets.len(), 1);
        assert_eq!(d.subnets[0].allocation_pools, vec!["10.0.0.2 – 10.0.0.254"]);
        assert_eq!(d.subnets[0].network_name.as_deref(), Some("net"));
        assert_eq!(d.mtu, Some(1450));
    }

    #[test]
    fn router_detail_reads_gateway_and_routes() {
        let r = json!({"id": "r", "name": "gw", "external_gateway_info": {"network_id": "ext", "enable_snat": true,
            "external_fixed_ips": [{"ip_address": "203.0.113.9", "subnet_id": "x"}]},
            "routes": [{"destination": "10.9.0.0/16", "nexthop": "10.0.0.1"}]});
        let d = router_detail(&r, &[json!({"id": "ext", "name": "public"})], vec![]);
        assert_eq!(d.row.external_network.as_deref(), Some("public"));
        assert_eq!(d.gateway_ips, vec![("203.0.113.9".into(), "x".into())]);
        assert_eq!(d.row.external_network_id.as_deref(), Some("ext"));
        assert_eq!(d.row.snat, Some(true));
        assert_eq!(d.routes, vec![("10.9.0.0/16".into(), "10.0.0.1".into())]);
    }

    #[test]
    fn security_group_rules_resolve_remote_groups() {
        let g = json!({"id": "g1", "name": "web", "security_group_rules": [
            {"id": "r1", "direction": "ingress", "ethertype": "IPv4", "protocol": "tcp", "port_range_min": 80, "port_range_max": 443, "remote_ip_prefix": "0.0.0.0/0"},
            {"id": "r2", "direction": "egress", "ethertype": "IPv4"},
            {"id": "r3", "direction": "ingress", "ethertype": "IPv4", "remote_group_id": "g1"}
        ]});
        let d = security_group_detail(&g, std::slice::from_ref(&g));
        assert_eq!(d.rules[0].direction, "egress");
        assert_eq!((d.rules[0].protocol.as_str(), d.rules[0].port_range.as_str(), d.rules[0].remote.as_str()), ("Any", "Any", "-"));
        let r1 = d.rules.iter().find(|r| r.id == "r1").unwrap();
        assert_eq!(r1.port_range, "80 - 443");
        let r3 = d.rules.iter().find(|r| r.id == "r3").unwrap();
        assert_eq!(r3.remote, "web");
    }

    #[test]
    fn image_detail_separates_custom_properties() {
        let im = json!({"id": "i", "name": "rhel", "owner": "p", "min_disk": 10, "hw_disk_bus": "scsi", "os_distro": "rhel", "size": 5});
        let d = image_detail(&im, "p", &HashMap::new());
        assert_eq!(d.properties, vec![("hw_disk_bus".into(), "scsi".into()), ("os_distro".into(), "rhel".into())]);
        assert_eq!((d.image.min_disk_gb, d.image.min_ram_mb), (Some(10), None));
        assert_eq!(d.image.property("os_distro"), Some("rhel"));
    }

    #[test]
    fn volume_detail_lists_its_snapshots_and_source() {
        let v = json!({"id": "v", "name": "data", "size": 10, "bootable": "true", "created_at": "2026-01-01T00:00:00",
            "volume_image_metadata": {"image_name": "rhel-9.4", "image_id": "img"}});
        let snaps = vec![json!({"id": "s1", "volume_id": "v", "name": "a"}), json!({"id": "s2", "volume_id": "other"})];
        let d = volume_detail(&v, &[], &snaps);
        assert_eq!(d.source_image.as_deref(), Some("rhel-9.4"));
        assert_eq!(d.snapshots.len(), 1);
        assert_eq!(d.snapshots[0].volume_name.as_deref(), Some("data"));
    }

    #[test]
    fn choices_are_sorted_with_readable_labels() {
        let c = choices(&[json!({"id": "2", "name": "b"}), json!({"id": "1", "name": "A"})], "id", label_name);
        assert_eq!(c[0].label, "A");
        let f = label_flavor(&json!({"name": "m1", "vcpus": 2, "ram": 4096, "disk": 20}));
        assert_eq!(f, "m1 · 2 vCPU · 4 GB · 20 GB");
    }

    #[test]
    fn boot_from_volume_takes_the_image_from_the_boot_volume() {
        let mut sv = server_fixture();
        sv["image"] = json!("");
        sv["os-extended-volumes:volumes_attached"] = json!([{"id": "data"}, {"id": "root"}]);
        let id = sv["id"].clone();
        let volumes = vec![
            json!({"id": "data", "bootable": "false", "attachments": [{"server_id": id, "device": "/dev/vdb"}]}),
            json!({"id": "root", "bootable": "true", "attachments": [{"server_id": id, "device": "/dev/vda"}],
                   "volume_image_metadata": {"image_name": "Ubuntu-24.04LTS", "image_id": "img-u"}}),
        ];
        let vm = &instances(&[sv.clone()], &flavors(), &[], &volumes)[0];
        assert_eq!(vm.image_name.as_deref(), Some("Ubuntu-24.04LTS"));
        assert!(vm.image_from_volume);

        // root_device_name (admin field) wins over conventions
        sv["OS-EXT-SRV-ATTR:root_device_name"] = json!("/dev/vdb");
        let volumes2 = vec![
            json!({"id": "data", "attachments": [{"server_id": id, "device": "/dev/vdb"}],
                   "volume_image_metadata": {"image_id": "img-r"}}),
            volumes[1].clone(),
        ];
        let vm = &instances(&[sv.clone()], &flavors(), &[json!({"id": "img-r", "name": "rhel"})], &volumes2)[0];
        assert_eq!(vm.image_name.as_deref(), Some("rhel"), "image id resolved through Glance names");

        // no volume metadata at all → still None, not a crash
        let vm = &instances(&[sv], &flavors(), &[], &[])[0];
        assert_eq!(vm.image_name, None);
        assert!(!vm.image_from_volume);
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
        let listed = vec![json!({"id": "4", "name": "m1.large", "vcpus": 4, "ram": 8192, "disk": 40, "swap": "", "OS-FLV-EXT-DATA:ephemeral": 0})];
        let vm = &instances(&[server_fixture()], &listed, &[], &[])[0];
        assert_eq!((vm.flavor.swap_mb, vm.flavor.ephemeral_gb, vm.flavor.vcpus), (None, Some(0), Some(4)));
        assert!(!vm.locked);
    }

    #[test]
    fn image_owner_resolves_to_project_name() {
        let names = HashMap::from([("p1".to_string(), "Proyecto A".to_string())]);
        let out = images(&[json!({"id": "1", "name": "a", "owner": "p1"}), json!({"id": "2", "name": "b", "owner": "zz"})], "p1", &names);
        assert_eq!(out[0].owner_name.as_deref(), Some("Proyecto A"));
        assert_eq!((out[1].owner_name.clone(), out[1].owner_id.as_deref()), (None, Some("zz")));
    }

    #[test]
    fn projects_skip_disabled_and_sort() {
        let p = projects(&[
            json!({"id": "2", "name": "beta", "enabled": true}),
            json!({"id": "3", "name": "gone", "enabled": false}),
            json!({"id": "1", "name": "Alpha"}),
        ]);
        assert_eq!(p.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["Alpha", "beta"]);
    }

    #[test]
    fn server_usages_fall_back_to_the_id_and_tolerate_missing_fields() {
        let v = json!({"server_usages": [
            {"instance_id": "i1", "name": "web", "vcpus": 2, "memory_mb": 4096, "local_gb": 20,
             "hours": 12.5, "uptime": 45000, "state": "active", "ended_at": null},
            {"instance_id": "i2", "name": "", "hours": 1, "state": "terminated"}
        ], "total_hours": 13.5});
        let out = server_usages(&v);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].hours, 12.5);
        assert_eq!(out[1].name, "i2");
        assert_eq!(out[1].vcpus, 0);
        assert!(!out[1].active());
        assert!(server_usages(&json!({})).is_empty());
    }

    #[test]
    fn launch_images_split_snapshots_and_carry_sizing_facts() {
        let raw = vec![
            json!({"id": "i1", "name": "ADM-RHEL9.7-ES", "status": "active", "size": 7_800_000_000u64,
                "min_disk": 20, "min_ram": 0, "disk_format": "qcow2", "visibility": "shared",
                "updated_at": "2026-09-25T14:09:00Z"}),
            json!({"id": "s1", "name": "Instalacion", "status": "active", "image_type": "snapshot", "size": 0}),
            json!({"id": "q", "name": "queued", "status": "queued"}),
        ];
        let imgs = image_choices(&raw, false);
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].cells, vec!["ADM-RHEL9.7-ES", "2026-09-25 14:09", "7.3 GB", "QCOW2", "shared"]);
        assert_eq!((imgs[0].facts.min_disk_gb, imgs[0].facts.size_bytes), (Some(20), Some(7_800_000_000)));
        let snaps = image_choices(&raw, true);
        assert_eq!(snaps.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(), vec!["s1"]);
    }

    #[test]
    fn flavor_choices_compute_total_disk_and_public() {
        let f = json!({"id": "f1", "name": "C3.1-2", "vcpus": 1, "ram": 2048, "disk": 50,
            "OS-FLV-EXT-DATA:ephemeral": 10, "os-flavor-access:is_public": false});
        let c = &flavor_choices(&[f], crate::prefs::FlavorSort::Ram, false)[0];
        assert_eq!(c.cells, vec!["C3.1-2", "1", "2 GB", "60 GB", "50 GB", "10 GB", "false"]);
        assert_eq!((c.facts.ram_mb, c.facts.disk_gb), (Some(2048), Some(50)));
    }

    #[test]
    fn networks_list_subnets_and_free_ports_skip_attached_ones() {
        let nets = vec![json!({"id": "n1", "name": "sadasd", "subnets": ["sn1", "gone"], "shared": false,
            "admin_state_up": true, "status": "ACTIVE"})];
        let subs = vec![json!({"id": "sn1", "name": "test-geneve", "cidr": "10.0.0.0/24"})];
        assert_eq!(network_choices(&nets, &subs)[0].cells[1], "test-geneve 10.0.0.0/24, gone");
        let ports = vec![
            json!({"id": "p1", "name": "", "device_id": "", "device_owner": "", "status": "DOWN",
                "fixed_ips": [{"ip_address": "10.0.0.7"}]}),
            json!({"id": "p2", "device_id": "srv", "device_owner": "compute:nova"}),
        ];
        let free = free_port_choices(&ports);
        assert_eq!(free.len(), 1);
        assert_eq!(free[0].cells, vec!["p1", "10.0.0.7", "true", "DOWN"]);
    }

    #[test]
    fn server_groups_read_policy_from_either_microversion() {
        let raw = vec![json!({"id": "g1", "name": "a", "policy": "anti-affinity"}), json!({"id": "g2", "name": "b", "policies": ["affinity"]})];
        let c = server_group_choices(&raw);
        assert_eq!((c[0].cells[1].as_str(), c[1].cells[1].as_str()), ("anti-affinity", "affinity"));
    }

    #[test]
    fn topology_joins_routers_and_servers_to_networks_through_ports() {
        let nets = vec![
            json!({"id": "ext", "name": "public", "router:external": true, "status": "ACTIVE"}),
            json!({"id": "n-b", "name": "beta", "project_id": "p1", "status": "ACTIVE"}),
            json!({"id": "n-a", "name": "", "shared": true, "tenant_id": "p2", "status": "DOWN"}),
        ];
        let subnets = vec![json!({"id": "s1", "network_id": "n-b", "cidr": "10.0.1.0/24"})];
        let routers = vec![json!({"id": "r1", "name": "edge", "status": "ACTIVE",
            "external_gateway_info": {"network_id": "ext",
                "external_fixed_ips": [{"subnet_id": "x", "ip_address": "203.0.113.2"}]}}),
            json!({"id": "r2", "name": "lonely", "status": "ACTIVE", "external_gateway_info": null})];
        let servers = vec![json!({"id": "vm1", "name": "web", "status": "ACTIVE"})];
        let ports = vec![
            json!({"id": "p-if", "network_id": "n-b", "device_id": "r1",
                "device_owner": "network:router_interface", "fixed_ips": [{"ip_address": "10.0.1.1"}]}),
            json!({"id": "p-gw", "network_id": "ext", "device_id": "r1", "device_owner": "network:router_gateway",
                "fixed_ips": [{"ip_address": "203.0.113.2"}]}),
            json!({"id": "p-vm-b", "network_id": "n-b", "device_id": "vm1", "device_owner": "compute:nova",
                "fixed_ips": [{"ip_address": "10.0.1.5"}]}),
            json!({"id": "p-vm-a", "network_id": "n-a", "device_id": "vm1", "device_owner": "compute:mad1",
                "fixed_ips": []}),
            json!({"id": "p-dhcp", "network_id": "n-b", "device_id": "dhcp-1", "device_owner": "network:dhcp"}),
            json!({"id": "p-free", "network_id": "n-b", "device_id": "", "device_owner": ""}),
            json!({"id": "p-other", "network_id": "hidden", "device_id": "vm1", "device_owner": "compute:nova"}),
        ];
        let fips = vec![json!({"floating_ip_address": "203.0.113.9", "port_id": "p-vm-b"}),
            json!({"floating_ip_address": "203.0.113.10", "port_id": null})];
        let t = topology(TopologyInputs {
            project_id: "p1",
            networks: &nets,
            subnets: &subnets,
            routers: &routers,
            ports: &ports,
            servers: &servers,
            floating_ips: &fips,
        });
        let names: Vec<_> = t.networks.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["public", "beta", "n-a"], "external first, unnamed falls back to id");
        assert_eq!(t.networks[1].cidrs, vec!["10.0.1.0/24"]);
        let owned: Vec<_> = t.networks.iter().map(|n| n.owned).collect();
        assert_eq!(owned, [false, true, false], "owned from project_id, else tenant_id");
        let devs: Vec<_> = t.devices.iter().map(|d| (d.name.as_str(), d.kind)).collect();
        assert_eq!(devs, vec![("edge", TopoKind::Router), ("lonely", TopoKind::Router), ("web", TopoKind::Server)]);

        let links: Vec<_> = t.links.iter().map(|l| (l.device_id.as_str(), l.network_id.as_str())).collect();
        assert_eq!(links, vec![("r1", "ext"), ("r1", "n-b"), ("vm1", "n-a"), ("vm1", "n-b")]);
        let gw = &t.links[0];
        assert!(gw.gateway, "gateway from external_gateway_info");
        assert_eq!((gw.port_id.as_deref(), gw.ips.clone()), (Some("p-gw"), vec!["203.0.113.2".to_string()]),
            "the gateway port is merged, not duplicated");
        let vm_b = t.links.iter().find(|l| l.port_id.as_deref() == Some("p-vm-b")).unwrap();
        assert_eq!((vm_b.ips.clone(), vm_b.floating.clone()), (vec!["10.0.1.5".into()], vec!["203.0.113.9".into()]));
        assert_eq!(t.links_of("vm1").count(), 2);
        assert_eq!(t.links_of("n-b").count(), 2);
    }

    #[test]
    fn topology_keeps_a_gateway_whose_port_is_not_listed() {
        let nets = vec![json!({"id": "ext", "name": "public", "router:external": true})];
        let routers = vec![json!({"id": "r1", "external_gateway_info": {"network_id": "ext"}})];
        let t = topology(TopologyInputs {
            project_id: "p1",
            networks: &nets,
            subnets: &[],
            routers: &routers,
            ports: &[],
            servers: &[],
            floating_ips: &[],
        });
        assert_eq!(t.links.len(), 1);
        assert_eq!((t.links[0].port_id.clone(), t.links[0].ips.len()), (None, 0));
        assert_eq!(t.devices[0].name, "r1");
    }

    #[test]
    fn instance_ports_carry_their_owner_and_groups() {
        let ports = vec![
            json!({"id": "p1", "device_owner": "compute:nova", "device_id": "s1", "security_groups": ["g1", "g2"],
                   "fixed_ips": [{"ip_address": "10.0.0.5"}]}),
            json!({"id": "p2", "device_owner": "compute:az1", "device_id": "s1", "security_groups": [],
                   "fixed_ips": [{"ip_address": "10.0.0.6"}]}),
            json!({"id": "r1", "device_owner": "network:router_interface", "device_id": "r"}),
        ];
        let fips = vec![
            json!({"id": "f1", "floating_ip_address": "1.2.3.4", "port_id": "p2"}),
            json!({"id": "f2", "floating_ip_address": "1.2.3.5", "port_id": null}),
        ];
        let names = HashMap::from([("s1".to_string(), "web".to_string())]);
        let all = instance_port_choices(&ports, &fips, &names, false, None);
        assert_eq!(all.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(), ["p1", "p2"]);
        assert_eq!(all[0].label, "web: 10.0.0.5");
        assert_eq!(all[0].cells, ["s1", "g1,g2"]);
        let free = instance_port_choices(&ports, &fips, &names, true, None);
        assert_eq!(free.len(), 1, "p2 already has a floating IP");
        let ips = free_floating_ip_choices(&fips);
        assert_eq!((ips.len(), ips[0].value.as_str(), ips[0].label.as_str()), (1, "f2", "1.2.3.5"));
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
        let d = floating_ip_detail(&fip, &rules, &[json!({"id": "pub", "name": "public"})], &[json!({"id": "s1", "name": "web"})], &ports);
        assert_eq!((d.row.ip.as_str(), d.row.pool.as_str(), d.port_id.clone(), d.router_id.as_deref()), ("203.0.113.7", "public", None, Some("r1")));
        let r = &d.forwardings;
        assert_eq!((r[0].protocol.as_str(), r[0].external.as_str(), r[0].internal.as_str()), ("tcp", "2222", "22"));
        assert_eq!(r[0].instance.as_ref().map(|i| i.name.as_str()), Some("web"));
        assert_eq!((r[1].external.as_str(), r[1].internal.as_str(), r[1].instance.clone(), r[1].description.clone()), ("6000:6010", "5000:5010", None, None));
    }

    #[test]
    fn lb_choices_carry_subnets_and_skip_disabled_flavors() {
        let ports = vec![json!({"id": "p1", "device_owner": "compute:nova", "device_id": "s1",
                                "fixed_ips": [{"ip_address": "10.0.0.5", "subnet_id": "sub-1"}, {"ip_address": "fd00::5", "subnet_id": "sub-6"}]})];
        let subnets = [json!({"id": "sub-1", "name": "web-subnet"})];
        let c = lb_member_choices(&ports, &HashMap::from([("s1".to_string(), "web".to_string())]), &subnets);
        let v: Vec<_> = c.iter().map(|c| (c.value.as_str(), c.label.as_str())).collect();
        assert_eq!(v, [("10.0.0.5@sub-1", "web: 10.0.0.5"), ("fd00::5@sub-6", "web: fd00::5")]);
        assert_eq!(c[0].cells, ["web", "10.0.0.5", "web-subnet"], "instance, IP and subnet name for the tables");
        assert_eq!(c[1].cells[2], "sub-6", "an unknown subnet keeps its ID");
        let nets = [json!({"id": "n-own"}), json!({"id": "n-shared"})];
        let all = [
            json!({"id": "s1", "name": "own-subnet", "network_id": "n-own"}),
            json!({"id": "s2", "name": "", "cidr": "203.0.113.0/24", "network_id": "n-shared"}),
            json!({"id": "s3", "name": "other-project", "network_id": "n-other"}),
        ];
        let sub: Vec<_> = lb_member_subnet_choices(&nets, &all).into_iter().map(|c| (c.value, c.label)).collect();
        assert_eq!(sub, [("s2".to_string(), "203.0.113.0/24".to_string()), ("s1".into(), "own-subnet".into())], "by name, else CIDR; other projects' left out");
        let f = lb_flavor_choices(&[json!({"id": "f1", "name": "small", "description": "1 amphora", "enabled": true}), json!({"id": "f2", "name": "off", "enabled": false})]);
        assert_eq!((f.len(), f[0].label.as_str()), (1, "small (1 amphora)"));
    }

    #[test]
    fn forwardable_ports_are_each_fixed_ipv4_of_an_instance_port() {
        let ports = vec![
            json!({"id": "p1", "device_owner": "compute:nova", "device_id": "s1",
                   "fixed_ips": [{"ip_address": "10.0.0.5"}, {"ip_address": "fd00::5"}, {"ip_address": "10.0.1.5"}]}),
            json!({"id": "r1", "device_owner": "network:router_interface", "fixed_ips": [{"ip_address": "10.0.0.1"}]}),
        ];
        let c = forwardable_port_choices(&ports, &HashMap::from([("s1".to_string(), "web".to_string())]));
        let v: Vec<_> = c.iter().map(|c| (c.value.as_str(), c.label.as_str())).collect();
        assert_eq!(v, [("p1/10.0.0.5", "web: 10.0.0.5"), ("p1/10.0.1.5", "web: 10.0.1.5")]);
    }

    #[test]
    fn floating_ips_only_reach_networks_behind_a_gateway_router() {
        let ports = vec![
            json!({"id": "a", "device_owner": "compute:nova", "device_id": "s1", "network_id": "routed"}),
            json!({"id": "b", "device_owner": "compute:nova", "device_id": "s1", "network_id": "isolated"}),
            json!({"id": "c", "device_owner": "compute:nova", "device_id": "s1", "network_id": "no-gateway"}),
            json!({"id": "ri1", "device_owner": "network:router_interface", "device_id": "gw", "network_id": "routed"}),
            json!({"id": "ri2", "device_owner": "network:router_interface_distributed", "device_id": "internal", "network_id": "no-gateway"}),
        ];
        let routers = vec![
            json!({"id": "gw", "external_gateway_info": {"network_id": "public"}}),
            json!({"id": "internal", "external_gateway_info": null}),
        ];
        let reach = externally_reachable_networks(&ports, &routers);
        assert_eq!(reach, ["routed".to_string()].into());
        let names = HashMap::new();
        let ok = instance_port_choices(&ports, &[], &names, true, Some(&reach));
        assert_eq!(ok.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(), ["a"]);
        assert_eq!(instance_port_choices(&ports, &[], &names, true, None).len(), 3, "check off");
    }

    #[test]
    fn volume_messages_keep_this_volume_newest_first() {
        let msgs = vec![
            json!({"id": "m1", "resource_uuid": "v1", "message_level": "ERROR", "created_at": "2026-10-01T10:00:00",
                   "user_message": "create volume: Could not find any available weighted backend.", "request_id": "req-1"}),
            json!({"id": "m2", "resource_uuid": "v2", "message_level": "ERROR", "created_at": "2026-10-02T10:00:00", "user_message": "x"}),
            json!({"id": "m3", "resource_uuid": "v1", "message_level": "ERROR", "created_at": "2026-10-03T10:00:00",
                   "user_message": "", "event_id": "VOLUME_000002"}),
        ];
        let out = volume_messages(&msgs, "v1");
        assert_eq!(out.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["m3", "m1"]);
        assert_eq!(out[0].message, "VOLUME_000002", "empty user message falls back to the event");
        assert_eq!(out[1].request_id.as_deref(), Some("req-1"));
    }

    // --- Key Manager (roadmap 55)

    #[test]
    fn barbican_refs_end_in_the_id() {
        assert_eq!(ref_id("https://kms:9311/v1/secrets/5e2a"), "5e2a");
        assert_eq!(ref_id("https://kms:9311/v1/containers/c-1/"), "c-1", "trailing slash");
        assert_eq!(ref_id(""), "");
    }

    #[test]
    fn barbican_pages_by_offset_until_total() {
        // Default page size is 10: 23 secrets take three pages.
        let page = json!({"secrets": [], "total": 23, "next": "https://kms/v1/secrets?limit=10&offset=10"});
        assert_eq!(next_offset(&page, 0, 10), Some(10));
        assert_eq!(next_offset(&page, 10, 10), Some(20));
        assert_eq!(next_offset(&page, 20, 3), None, "last page");
        assert_eq!(next_offset(&json!({"total": 50}), 0, 0), None, "an empty page ends the loop");
        assert_eq!(next_offset(&json!({"next": "x"}), 0, 10), Some(10), "no total: follow `next`");
        assert_eq!(next_offset(&json!({"next": null}), 0, 10), None);
    }

    #[test]
    fn secrets_map_barbican_metadata() {
        let raw = [
            json!({"secret_ref": "https://kms/v1/secrets/b2", "name": "web-cert", "secret_type": "certificate", "algorithm": null,
                   "bit_length": null, "mode": null, "content_types": {"default": "text/plain"}, "status": "ACTIVE",
                   "expiration": "2026-12-31T00:00:00", "created": "2026-10-01T10:00:00", "updated": "2026-10-01T10:00:00", "creator_id": "u1"}),
            json!({"secret_ref": "https://kms/v1/secrets/a1", "name": null, "secret_type": "symmetric", "algorithm": "aes",
                   "bit_length": 256, "mode": "cbc", "status": "ACTIVE", "expiration": null, "created": "2026-09-01T10:00:00"}),
        ];
        let out = secrets(&raw);
        assert_eq!(out[0].name, "a1", "unnamed: the ID from the ref, sorted first");
        assert_eq!((out[0].algorithm.as_deref(), out[0].bit_length, out[0].mode.as_deref()), (Some("aes"), Some(256), Some("cbc")));
        assert_eq!(out[0].content_type, None, "no payload yet: no content types");
        assert_eq!(out[0].expiration, None);
        assert_eq!(out[1].id, "b2");
        assert_eq!(out[1].content_type.as_deref(), Some("text/plain"));
        assert_eq!(out[1].expiration.as_deref(), Some("2026-12-31T00:00:00"));
        assert_eq!(out[1].secret_ref, "https://kms/v1/secrets/b2");
        let c = secret_choices(&raw);
        assert_eq!((c[1].value.as_str(), c[1].label.as_str()), ("https://kms/v1/secrets/b2", "web-cert (certificate)"), "containers take refs");
    }

    #[test]
    fn in_use_finds_consumers_and_the_containers_holding_a_secret() {
        let container = |name: &str, secret: &str, consumers: &[(&str, &str)]| ContainerRow {
            id: name.into(),
            name: name.into(),
            container_type: "certificate".into(),
            status: "ACTIVE".into(),
            created: String::new(),
            updated: None,
            creator_id: None,
            container_ref: String::new(),
            secrets: vec![ContainerSecret { role: "certificate".into(), id: secret.into(), name: secret.into() }],
            consumers: consumers.iter().map(|(n, u)| ContainerConsumer { name: n.to_string(), url: u.to_string() }).collect(),
        };
        let cs = [
            container("web", "s1", &[("octavia", "lbaas://es/loadbalancer/lb1")]),
            container("idle", "s1", &[]),
            container("other", "s2", &[("octavia", "lbaas://es/loadbalancer/lb2")]),
        ];
        let own = [SecretConsumer { service: "image".into(), resource_type: "image".into(), resource_id: "i1".into() }];
        assert_eq!(secret_in_use("s1", &own, &cs), ["image image i1", "container web: octavia lbaas://es/loadbalancer/lb1"]);
        assert!(secret_in_use("s3", &[], &cs).is_empty());
        assert_eq!(container_in_use(&cs[0]), ["octavia lbaas://es/loadbalancer/lb1"]);
    }

    #[test]
    fn octavia_listeners_and_pools_name_the_refs_they_use() {
        let kms = |kind: &str, id: &str| format!("https://kms/v1/{kind}/{id}");
        let listeners = [
            json!({"id": "l1", "name": "https", "default_tls_container_ref": kms("containers", "c1"), "sni_container_refs": [kms("secrets", "p12")],
                   "client_ca_tls_container_ref": null}),
            json!({"id": "l2", "name": "", "default_tls_container_ref": null, "sni_container_refs": []}),
        ];
        let pools = [json!({"id": "p1", "name": "backend", "tls_container_ref": kms("containers", "c2"), "ca_tls_container_ref": "",
                            "loadbalancers": [{"id": "lb2"}]})];
        let lbs = [json!({"id": "lb2", "name": "web"})];
        let refs = octavia_cert_refs(&listeners, &pools, &lbs);
        let pairs: Vec<_> = refs.iter().map(|(i, u)| (i.as_str(), u.kind.as_str(), u.name.as_str(), u.lb_name.as_str())).collect();
        assert_eq!(pairs, [("c1", "listener", "https", "-"), ("p12", "listener", "https", "-"), ("c2", "pool", "backend", "web")]);
        let l = [json!({"id": "l3", "name": "", "default_tls_container_ref": kms("containers", "c3"), "loadbalancers": [{"id": "lb9"}]})];
        let r = octavia_cert_refs(&l, &[], &[]);
        assert_eq!((r[0].1.lb_id.as_deref(), r[0].1.lb_name.as_str(), r[0].1.name.as_str()), (Some("lb9"), "lb9", "l3"), "unnamed: IDs");
        // A secret is used directly (a PKCS12 SNI ref) and through container c2.
        let holder = ContainerRow {
            id: "c2".into(),
            name: "web-tls".into(),
            container_type: "certificate".into(),
            status: "ACTIVE".into(),
            created: String::new(),
            updated: None,
            creator_id: None,
            container_ref: String::new(),
            secrets: vec![ContainerSecret { role: "certificate".into(), id: "p12".into(), name: "cert".into() }],
            consumers: vec![],
        };
        let uses = octavia_uses(&refs, "p12", std::slice::from_ref(&holder));
        assert_eq!(uses.len(), 2);
        assert_eq!((uses[0].via.clone(), uses[1].via.clone()), (None, Some(("c2".to_string(), "web-tls".to_string()))));
        assert!(octavia_uses(&refs, "nothing", &[holder]).is_empty());
    }

    #[test]
    fn secret_acl_defaults_to_project_access() {
        assert_eq!(secret_acl(&json!({"read": {"project-access": false, "users": ["u1", "u2"]}})), (vec!["u1".to_string(), "u2".to_string()], Some(false)));
        assert_eq!(secret_acl(&json!({"read": {}})), (vec![], Some(true)), "no ACL set: Barbican's default");
        assert_eq!(secret_acl(&Value::Null), (vec![], None), "ACL unreadable");
    }

    #[test]
    fn containers_name_their_secrets_by_role() {
        let secrets_raw = [json!({"secret_ref": "https://kms/v1/secrets/s1", "name": "web-cert"})];
        let raw = [json!({
            "container_ref": "https://kms/v1/containers/c1", "name": "", "type": "certificate", "status": "ACTIVE",
            "created": "2026-10-01T10:00:00", "updated": null,
            "secret_refs": [
                {"name": "private_key", "secret_ref": "https://kms/v1/secrets/s2"},
                {"name": "certificate", "secret_ref": "https://kms/v1/secrets/s1"}
            ],
            "consumers": [{"name": "octavia", "URL": "https://lb/v2/lbaas/listeners/l1"}]
        })];
        let c = &containers(&raw, &secrets_raw)[0];
        assert_eq!((c.id.as_str(), c.name.as_str(), c.container_type.as_str()), ("c1", "c1", "certificate"), "empty name: the ID");
        assert_eq!(c.updated, None);
        let roles: Vec<_> = c.secrets.iter().map(|x| (x.role.as_str(), x.name.as_str())).collect();
        assert_eq!(roles, [("certificate", "web-cert"), ("private_key", "s2")], "by role; an unknown secret keeps its ID");
        assert_eq!(c.consumers, [ContainerConsumer { name: "octavia".into(), url: "https://lb/v2/lbaas/listeners/l1".into() }]);
    }

    // --- Alarming (roadmap 79) ---

    #[test]
    fn alarms_map_every_rule_type() {
        let raw = vec![
            json!({"alarm_id": "a1", "name": "cpu-high", "type": "gnocchi_resources_threshold", "state": "alarm",
                   "severity": "critical", "enabled": true, "timestamp": "2026-10-28T10:00:00.123456",
                   "state_timestamp": "2026-10-28T11:00:00", "state_reason": "Transition to alarm due to 3 samples outside threshold",
                   "gnocchi_resources_threshold_rule": {"metric": "cpu", "resource_type": "instance", "resource_id": "srv1",
                       "aggregation_method": "mean", "comparison_operator": "gt", "threshold": 80.0, "granularity": 300, "evaluation_periods": 3}}),
            json!({"alarm_id": "a2", "name": "", "type": "prometheus", "state": "insufficient data", "severity": "",
                   "enabled": false, "prometheus_rule": {"query": "rate(cpu[5m])", "comparison_operator": "ge", "threshold": "0.5"}}),
            json!({"alarm_id": "a3", "name": "Deleted", "type": "event", "state": "ok",
                   "event_rule": {"event_type": "compute.instance.delete.end", "query": [{"field": "traits.instance_id", "op": "eq", "type": "string", "value": "srv1"}]}}),
            json!({"alarm_id": "a4", "name": "both", "type": "composite", "state": "ok",
                   "composite_rule": {"or": [{"type": "prometheus", "threshold": 1}]}}),
        ];
        let rows = alarms(&raw);
        let names: Vec<_> = rows.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["a2", "both", "cpu-high", "Deleted"], "by name, case-insensitive; empty name: the ID");
        let cpu = &rows[2];
        assert_eq!(cpu.condition, "mean(cpu) > 80 · instance srv1");
        assert_eq!((cpu.severity.as_deref(), cpu.enabled, cpu.state.as_str()), (Some("critical"), true, "alarm"));
        assert_eq!(rows[0].condition, "rate(cpu[5m]) ≥ 0.5", "a string threshold is read too");
        assert_eq!((rows[0].severity.clone(), rows[0].enabled), (None, false), "\"\" severity is unset");
        assert_eq!(rows[3].condition, "compute.instance.delete.end");
        assert_eq!(rows[1].condition, "-", "composite: no one-line summary");
        assert!(alarms(&[json!({"alarm_id": "a5", "name": "x", "type": "prometheus", "state": "ok"})])[0].enabled, "missing `enabled`: Aodh's default");
    }

    #[test]
    fn alarm_detail_parses_rule_actions_and_history() {
        let alarm = json!({"alarm_id": "a1", "name": "cpu-high", "type": "gnocchi_resources_threshold", "state": "alarm",
            "gnocchi_resources_threshold_rule": {"metric": "cpu", "resource_type": "instance", "resource_id": "srv1",
                "aggregation_method": "mean", "comparison_operator": "gt", "threshold": 80, "granularity": 300, "evaluation_periods": 3},
            "alarm_actions": ["https://hook/alarm"], "ok_actions": [], "insufficient_data_actions": ["log://"],
            "repeat_actions": false, "project_id": "p1", "user_id": "u1",
            "time_constraints": [{"name": "office", "start": "0 9 * * 1-5", "duration": 32400, "timezone": "Europe/Madrid", "description": ""}]});
        let history = vec![
            json!({"event_id": "e1", "type": "creation", "timestamp": "2026-10-28T09:00:00", "user_id": "u1", "detail": "{\"name\": \"cpu-high\"}"}),
            json!({"event_id": "e3", "type": "state transition", "timestamp": "2026-10-28T11:00:00", "user_id": null,
                   "detail": "{\"state\": \"alarm\", \"transition_reason\": \"3 datapoints are unknown\"}"}),
            json!({"event_id": "e2", "type": "rule change", "timestamp": "2026-10-28T10:00:00", "user_id": "u1", "detail": "{\"severity\": \"critical\"}"}),
        ];
        let d = alarm_detail(&alarm, &history);
        let AlarmRule::Threshold { resource_href, threshold, granularity, .. } = &d.rule else { panic!("threshold rule: {:?}", d.rule) };
        assert_eq!(resource_href.as_deref(), Some("/project/instances/srv1"), "Gnocchi `instance` resources are servers");
        assert_eq!((*threshold, *granularity), (Some(80.0), Some(300)));
        assert_eq!((d.alarm_actions.len(), d.ok_actions.len(), d.insufficient_data_actions.len()), (1, 0, 1));
        let tc = &d.time_constraints[0];
        assert_eq!((tc.start.as_str(), tc.duration, tc.timezone.as_deref(), tc.description.clone()), ("0 9 * * 1-5", Some(32400), Some("Europe/Madrid"), None));
        let kinds: Vec<_> = d.history.iter().map(|h| h.kind.as_str()).collect();
        assert_eq!(kinds, ["state transition", "rule change", "creation"], "newest first");
        assert_eq!((d.history[0].state.as_deref(), d.history[0].detail.as_deref(), d.history[0].user_id.clone()), (Some("alarm"), Some("3 datapoints are unknown"), None));
        assert_eq!(d.history[1].detail.as_deref(), Some(r#"{"severity":"critical"}"#));
        assert_eq!(d.history[2].detail, None, "a creation repeats the whole alarm: left out");
        let spec = &d.alarm.spec;
        assert_eq!((spec.alarm_type.as_str(), spec.metric.as_str(), spec.resource_id.as_str(), spec.threshold), ("gnocchi_resources_threshold", "cpu", "srv1", Some(80.0)));
        assert_eq!((spec.severity.as_str(), spec.time_constraints.len()), ("low", 1), "no severity: Aodh's default; constraints kept for the PUT");
        let body = spec.body().unwrap();
        assert_eq!(body["gnocchi_resources_threshold_rule"]["granularity"], json!(300), "an unchanged alarm rebuilds its rule");
        assert_eq!(body["time_constraints"][0]["timezone"], json!("Europe/Madrid"));
        let unknown = alarm_detail(&json!({"alarm_id": "a9", "name": "lb", "type": "loadbalancer_member_health", "loadbalancer_member_health_rule": {"pool_id": "p"}}), &[]);
        assert!(matches!(&unknown.rule, AlarmRule::Raw(j) if j.contains("\"pool_id\"")), "unknown types keep their JSON");
    }

    // --- Admin (roadmap 85) ---

    #[test]
    fn timestamps_of_nova_cinder_and_neutron_become_rfc3339_utc() {
        assert_eq!(utc_rfc3339("2026-10-30T10:00:00.000000").as_deref(), Some("2026-10-30T10:00:00Z"), "Nova, Cinder: naive UTC");
        assert_eq!(utc_rfc3339("2026-10-30 10:00:00").as_deref(), Some("2026-10-30T10:00:00Z"), "Neutron agents: a space");
        assert_eq!(utc_rfc3339("2026-10-30T12:00:00+02:00").as_deref(), Some("2026-10-30T10:00:00Z"));
        assert_eq!(utc_rfc3339(""), None);
    }

    #[test]
    fn os_services_map_status_state_and_zone() {
        let raw = vec![
            json!({"id": 3, "binary": "nova-scheduler", "host": "s-0", "zone": "internal", "status": "enabled", "state": "up", "updated_at": "2026-10-30T10:00:00.000000", "disabled_reason": null}),
            json!({"id": 1, "binary": "nova-compute", "host": "c-1", "zone": "nova", "status": "disabled", "state": "down", "updated_at": null, "disabled_reason": "maintenance"}),
            json!({"id": 2, "binary": "nova-compute", "host": "c-0", "zone": "", "status": "enabled", "state": "up"}),
        ];
        let out = host_services(&raw);
        assert_eq!(out.iter().map(|x| x.host.as_str()).collect::<Vec<_>>(), ["c-0", "c-1", "s-0"], "by binary, then host");
        assert_eq!((out[1].enabled, out[1].up, out[1].disabled_reason.as_deref()), (false, false, Some("maintenance")));
        assert_eq!(out[0].zone, None, "empty zone");
        assert_eq!(out[1].updated_at, None, "never reported");
        assert_eq!(out[2].updated_at.as_deref(), Some("2026-10-30T10:00:00Z"));
    }

    #[test]
    fn network_agents_map_liveness_and_heartbeat() {
        let raw = vec![
            json!({"id": "a2", "agent_type": "OVN Metadata agent", "binary": "neutron-ovn-metadata-agent", "host": "h1", "availability_zone": null,
                   "admin_state_up": true, "alive": true, "heartbeat_timestamp": "2026-10-19 08:00:00"}),
            json!({"id": "a1", "agent_type": "OVN Controller Gateway agent", "binary": "ovn-controller", "host": "h3", "availability_zone": "",
                   "admin_state_up": false, "alive": false, "heartbeat_timestamp": "2026-10-30 10:00:00"}),
        ];
        let out = network_agents(&raw);
        assert_eq!(out[0].agent_type, "OVN Controller Gateway agent", "by type");
        assert_eq!((out[0].admin_up, out[0].alive, out[0].zone.clone()), (false, false, None));
        assert_eq!(out[1].heartbeat.as_deref(), Some("2026-10-19T08:00:00Z"));
    }

    #[test]
    fn metadef_namespaces_show_display_names_and_resource_types() {
        let raw = vec![
            json!({"namespace": "OS::Compute::Watchdog", "display_name": "Watchdog Behavior", "description": "Compute watchdog",
                   "visibility": "public", "protected": true,
                   "resource_type_associations": [{"name": "OS::Glance::Image"}, {"name": "OS::Nova::Flavor"}]}),
            json!({"namespace": "CIM::StorageAllocationSettingData", "display_name": "", "visibility": "private", "protected": false}),
            json!({"display_name": "no namespace"}),
        ];
        let out = metadef_namespaces(&raw);
        assert_eq!(out.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["CIM::StorageAllocationSettingData", "Watchdog Behavior"], "empty display name → namespace; no namespace → skipped");
        assert_eq!(out[1].resource_types, ["OS::Glance::Image", "OS::Nova::Flavor"]);
        assert_eq!((out[1].public, out[1].protected, out[0].public, out[0].description.clone()), (true, true, false, None));
    }

    #[test]
    fn metadef_options_type_the_values_and_keep_the_prefixed_names() {
        // Shaped like Glance's answer for `?resource_type=OS::Glance::Image`: names already prefixed.
        let ns = json!({"namespace": "OS::Compute::CPUPinning", "display_name": "CPU Pinning",
            "resource_type_associations": [{"name": "OS::Glance::Image", "prefix": "hw_"}],
            "properties": {
                "hw_cpu_policy": {"title": "CPU Pinning policy", "type": "string", "enum": ["shared", "dedicated"], "default": "shared", "description": "Pin"},
                "hw_numa_nodes": {"type": "integer", "default": 2},
                "hw_hidden": {"type": "boolean", "readonly": true},
                "hw_flags": {"title": "Flags", "type": "array"}
            }});
        let out = metadef_options(&ns);
        assert_eq!(out.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(), ["hw_cpu_policy", "hw_flags", "hw_numa_nodes"], "prefix not applied twice; readonly skipped; sorted by title (no title → name)");
        assert_eq!((out[0].kind, out[0].choices.clone(), out[0].default.clone()), (MetadataKind::Text, vec!["shared".to_string(), "dedicated".to_string()], Some("shared".to_string())));
        assert_eq!((out[2].kind, out[2].default.clone(), out[2].title.as_str()), (MetadataKind::Integer, Some("2".to_string()), "hw_numa_nodes"));
        assert_eq!(out[1].kind, MetadataKind::Text, "arrays are free text");
        assert_eq!(out[0].namespace, "CPU Pinning");
    }

    #[test]
    fn metadef_options_include_the_properties_of_objects_once() {
        let ns = json!({"namespace": "OS::Compute::Libvirt", "display_name": "Libvirt Driver Options",
            "properties": {"hw_serial": {"title": "Serial", "type": "string"}},
            "objects": [
                {"name": "Quota", "properties": {"quota:cpu_shares": {"title": "CPU shares", "type": "integer"}, "hw_serial": {"title": "Dup"}}},
                {"name": "", "properties": {"quota:vif": {"type": "string"}}},
                {"name": "Empty"}
            ]});
        let out = metadef_options(&ns);
        let got: Vec<_> = out.iter().map(|o| (o.title.as_str(), o.key.as_str())).collect();
        assert_eq!(got, [("Quota › CPU shares", "quota:cpu_shares"), ("quota:vif", "quota:vif"), ("Serial", "hw_serial")], "object properties get their object's name; a repeated key and nameless objects are handled");
        assert_eq!(out[0].kind, MetadataKind::Integer);
    }

    #[test]
    fn default_quota_classes_map_like_project_quotas() {
        // Nova's `os-quota-class-sets/default` at 2.1: plain numbers, its `id`
        // and the pre-2.36 network proxies, which Horizon's Defaults don't show.
        let nova = json!({"id": "default", "instances": 10, "cores": 20, "ram": 51200, "floating_ips": 10, "fixed_ips": -1,
                          "security_groups": 10, "security_group_rules": 20, "key_pairs": 100, "server_groups": 10,
                          "server_group_members": 10, "metadata_items": 128, "injected_files": 5,
                          "injected_file_content_bytes": 10240, "injected_file_path_bytes": 255});
        let rows = quota_rows(QuotaService::Compute, &nova, &Value::Null);
        assert_eq!(rows.len(), 10, "Horizon's ten compute defaults");
        assert_eq!((rows[0].key.as_str(), rows[0].limit, rows[0].used), ("instances", Some(10), None));
        let cinder = json!({"id": "default", "volumes": -1, "gigabytes": 1000, "volumes_ceph": -1});
        let rows = quota_rows(QuotaService::Volume, &cinder, &Value::Null);
        assert_eq!(rows.iter().map(|r| (r.key.as_str(), r.limit)).collect::<Vec<_>>(), [("volumes", None), ("gigabytes", Some(1000)), ("volumes_ceph", None)], "-1 = unlimited");
    }

    #[test]
    fn resource_providers_read_inventory_and_usage_per_class() {
        let p = json!({"uuid": "u1", "name": "cmp-0", "generation": 7});
        let inv = json!({"inventories": {
            "VCPU": {"total": 48, "reserved": 2, "allocation_ratio": 16.0, "min_unit": 1},
            "MEMORY_MB": {"total": 262144, "reserved": 4096, "allocation_ratio": 1.0},
            "DISK_GB": {"total": 900, "reserved": 0}
        }});
        let r = resource_provider(&p, &inv, &json!({"usages": {"VCPU": 12, "MEMORY_MB": 65536}}));
        assert_eq!(r.vcpus, Some(ProviderResource { used: 12, reserved: 2, total: 48, allocation_ratio: 16.0 }));
        assert_eq!(r.disk_gb.map(|d| (d.used, d.allocation_ratio)), Some((0, 1.0)), "no usage → 0; no ratio → 1.0");
        let nested = resource_provider(&json!({"uuid": "u2", "name": ""}), &json!({"inventories": {}}), &json!({}));
        assert_eq!((nested.name.as_str(), nested.vcpus), ("u2", None), "nameless → UUID; no inventory → None");
        let gpu = resource_provider(
            &json!({"uuid": "g1", "name": "cmp-0_pci_0000_84_00_0", "parent_provider_uuid": "u1", "root_provider_uuid": "u1"}),
            &json!({"inventories": {"VGPU": {"total": 8, "reserved": 0, "allocation_ratio": 1.0}, "CUSTOM_PCI_10DE_2236": {"total": 1}}}),
            &json!({"usages": {"VGPU": 3}}),
        );
        assert_eq!((gpu.parent.as_deref(), gpu.vgpus.map(|v| (v.used, v.total))), (Some("u1"), Some((3, 8))));
        assert_eq!(gpu.others.iter().map(|(k, r)| (k.as_str(), r.total)).collect::<Vec<_>>(), [("CUSTOM_PCI_10DE_2236", 1)], "other classes are kept");
        assert_eq!(
            custom_traits(&json!({"traits": ["COMPUTE_VGPU", "CUSTOM_NVIDIA_468", "HW_GPU_API_VULKAN", "CUSTOM_A"]})),
            ["CUSTOM_A", "CUSTOM_NVIDIA_468"],
            "only CUSTOM_*, sorted"
        );
    }

    #[test]
    fn provider_attributes_are_read_when_admins_get_them() {
        let net = |extra: Value| {
            let mut n = json!({"id": "n1", "name": "ext", "status": "ACTIVE", "admin_state_up": true, "subnets": []});
            n.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            networks(&[n], &[], "p")[0].provider.clone()
        };
        let vlan = net(json!({"provider:network_type": "vlan", "provider:physical_network": "physnet1", "provider:segmentation_id": 120}));
        assert_eq!(vlan, Some(ProviderNetwork { network_type: "vlan".into(), physical_network: Some("physnet1".into()), segmentation_id: Some(120) }));
        let flat = net(json!({"provider:network_type": "flat", "provider:physical_network": "public", "provider:segmentation_id": null}));
        assert_eq!(flat.and_then(|p| p.segmentation_id), None, "flat: null segmentation ID");
        assert_eq!(net(json!({})), None, "members don't see provider attributes");
    }

    #[test]
    fn owned_choices_name_the_project() {
        let raw = vec![json!({"id": "n2", "name": "priv", "project_id": "p2"}), json!({"id": "n1", "name": "priv", "project_id": "p1"}), json!({"id": "n3", "name": ""})];
        let names = HashMap::from([("p1".to_string(), "alpha".to_string())]);
        let got: Vec<_> = owned_choices(&raw, &names).into_iter().map(|c| (c.value, c.label)).collect();
        assert_eq!(got, [("n3".into(), "n3".into()), ("n1".into(), "priv (alpha)".into()), ("n2".into(), "priv (p2)".into())], "same names told apart by project; unnamed → ID");
    }

    #[test]
    fn metadef_resource_types_are_sorted_names() {
        let v = json!({"resource_types": [{"name": "OS::Nova::Flavor", "created_at": "x"}, {"name": "OS::Cinder::Volume"}, {}]});
        let got: Vec<String> = metadef_resource_type_choices(&v).into_iter().map(|c| c.value).collect();
        assert_eq!(got, ["OS::Cinder::Volume", "OS::Nova::Flavor"]);
    }

    #[test]
    fn owned_ports_lead_with_their_project_and_skip_taken_ones() {
        let ports = vec![
            json!({"id": "pa", "project_id": "p1", "device_owner": "compute:nova", "device_id": "s1", "fixed_ips": [{"ip_address": "10.0.0.5"}]}),
            json!({"id": "pb", "tenant_id": "p2", "device_owner": "compute:az1", "device_id": "s2", "fixed_ips": [{"ip_address": "10.0.0.6"}]}),
            json!({"id": "pc", "project_id": "p1", "device_owner": "network:dhcp", "fixed_ips": []}),
            json!({"id": "pd", "project_id": "p1", "device_owner": "compute:nova", "device_id": "s3", "fixed_ips": [{"ip_address": "10.0.0.7"}]}),
        ];
        let fips = vec![json!({"id": "f", "port_id": "pd"})];
        let got: Vec<(String, String)> = owned_port_choices(&ports, &fips, &HashMap::new()).into_iter().map(|c| (c.value, c.cells[0].clone())).collect();
        assert_eq!(got.len(), 2, "no DHCP port, no port that has a floating IP: {got:?}");
        assert!(got.contains(&("pa".into(), "p1".into())) && got.contains(&("pb".into(), "p2".into())), "older Neutron says tenant_id: {got:?}");
    }

    #[test]
    fn cinder_pools_are_sorted_names() {
        let raw = vec![json!({"name": "c2@ceph#slow"}), json!({"name": "c1@lvm#lvm"}), json!({"name": ""}), json!({})];
        let got: Vec<String> = pool_choices(&raw).into_iter().map(|c| c.value).collect();
        assert_eq!(got, ["c1@lvm#lvm", "c2@ceph#slow"], "unnamed pools dropped");
    }

    #[test]
    fn zones_are_named_once_and_only_for_their_resource() {
        let raw = vec![
            json!({"name": "az2", "resource": "network", "state": "available"}),
            json!({"name": "az1", "resource": "router", "state": "available"}),
            json!({"name": "az1", "resource": "network", "state": "available"}),
            json!({"name": "az2", "resource": "network", "state": "available"}),
            json!({"name": "az3", "resource": "router", "state": "unavailable"}),
        ];
        let names = |r| zone_choices(&raw, r).into_iter().map(|c| c.label).collect::<Vec<_>>();
        assert_eq!(names("network"), ["az1", "az2"]);
        assert_eq!(names("router"), ["az1"], "unavailable zones are left out");
    }

    #[test]
    fn migration_targets_are_enabled_up_compute_hosts() {
        let raw = vec![
            json!({"id": "u1", "binary": "nova-compute", "host": "c2", "status": "enabled", "state": "up"}),
            json!({"id": "u2", "binary": "nova-compute", "host": "c1", "status": "enabled", "state": "up"}),
            json!({"id": "u3", "binary": "nova-compute", "host": "c3", "status": "disabled", "state": "up"}),
            json!({"id": "u4", "binary": "nova-compute", "host": "c4", "status": "enabled", "state": "down"}),
        ];
        let hosts = |usable| compute_host_choices(&raw, usable).into_iter().map(|c| c.label).collect::<Vec<_>>();
        assert_eq!(hosts(true), ["c1", "c2"], "disabled or down hosts can't take an instance");
        assert_eq!(hosts(false), ["c1", "c2", "c3", "c4"], "but they can join an aggregate");
        assert_eq!(host_services(&raw)[0].id, "u2", "the UUID (2.53) is kept for Enable/Disable Service");
    }

    #[test]
    fn dhcp_agents_are_counted_per_network() {
        let per_agent = vec![json!({"networks": [{"id": "n1"}, {"id": "n2"}]}), json!({"networks": [{"id": "n1"}]}), json!({})];
        let c = dhcp_agent_counts(&per_agent);
        assert_eq!((c.get("n1"), c.get("n2"), c.get("n3")), (Some(&2), Some(&1), None), "an agent without networks counts nothing");
    }

    #[test]
    fn tenant_usages_merge_pages_and_name_projects() {
        let u = |id: &str, vcpus: i64, state: &str| json!({"instance_id": id, "name": id, "vcpus": vcpus, "memory_mb": 1024, "local_gb": 10, "hours": 2.0, "uptime": 7200, "state": state});
        // Page 1 ends inside p1; page 2 carries p1's last server and p2.
        let raw = vec![
            json!({"tenant_id": "p1", "server_usages": [u("a", 2, "active")]}),
            json!({"tenant_id": "p1", "server_usages": [u("b", 4, "terminated")]}),
            json!({"tenant_id": "p2", "server_usages": [u("c", 1, "active")]}),
        ];
        let names = HashMap::from([("p1".to_string(), "zeta".to_string())]);
        let out = tenant_usages(&raw, &names);
        assert_eq!(out.iter().map(|p| p.project.as_str()).collect::<Vec<_>>(), ["p2", "zeta"], "one line per project, by name; unnamed → ID");
        let zeta = &out[1];
        assert_eq!(zeta.servers.len(), 2, "the split project is merged");
        assert_eq!((zeta.vcpus(), zeta.memory_mb(), zeta.local_gb()), (2, 1024, 10), "sizes count only servers that still exist");
        assert_eq!(zeta.vcpu_hours(), 12.0, "hours count the ended one too: 2×2 + 4×2");
    }

    #[test]
    fn owned_rows_get_project_names_and_hosts() {
        let raw = vec![json!({"id": "s1", "tenant_id": "p1", "OS-EXT-SRV-ATTR:host": "c-0"}), json!({"id": "s2", "tenant_id": "p9"})];
        let names = HashMap::from([("p1".to_string(), "demo".to_string())]);
        let out = owned(vec!["s1", "s2", "s3"], |x| x, &raw, "tenant_id", Some("OS-EXT-SRV-ATTR:host"), &names);
        let got: Vec<_> = out.iter().map(|o| (o.project.as_str(), o.host.as_deref())).collect();
        assert_eq!(got, [("demo", Some("c-0")), ("p9", None), ("-", None)], "named; unknown project → ID; missing → `-`");
    }

    #[test]
    fn hypervisors_aggregates_and_zones() {
        let h = hypervisors(&[json!({"id": "6f6b…", "hypervisor_hostname": "b", "hypervisor_type": "QEMU", "vcpus": 64, "vcpus_used": 8,
            "memory_mb": 515000, "memory_mb_used": 16384, "local_gb": 900, "local_gb_used": 40, "running_vms": 3, "state": "up", "status": "enabled"}),
            json!({"id": 7, "hypervisor_hostname": "a", "state": "down", "status": "disabled"})]);
        assert_eq!(h.iter().map(|x| (x.hostname.as_str(), x.id.as_str())).collect::<Vec<_>>(), [("a", "7"), ("b", "6f6b…")], "integer IDs before 2.53");
        assert_eq!((h[1].ram_used_mb, h[1].instances, h[1].up, h[0].up, h[0].vcpus), (16384, 3, true, false, 0));
        let a = aggregates(&[json!({"id": 3, "name": "ssd", "availability_zone": "alh1", "hosts": ["h2", "h1"], "metadata": {"ssd": "true", "availability_zone": "alh1"}})]);
        assert_eq!((a[0].id.as_str(), a[0].hosts.clone(), a[0].metadata[0].0.as_str()), ("3", vec!["h1".to_string(), "h2".to_string()], "availability_zone"));
        let z = zones(&[json!({"zoneName": "nova", "zoneState": {"available": true}, "hosts": {"c-1": {}, "c-0": {}}}), json!({"zoneName": "internal", "zoneState": {"available": false}, "hosts": null})]);
        assert_eq!(z.iter().map(|z| (z.name.as_str(), z.hosts.len(), z.available)).collect::<Vec<_>>(), [("internal", 0, false), ("nova", 2, true)]);
    }

    #[test]
    fn flavor_rows_read_swap_ephemeral_and_access() {
        let f = flavor_rows(&[json!({"id": "f1", "name": "m1.small", "vcpus": 1, "ram": 2048, "disk": 20, "OS-FLV-EXT-DATA:ephemeral": 0, "swap": "",
            "rxtx_factor": 1.0, "os-flavor-access:is_public": false, "extra_specs": {"hw:cpu_policy": "dedicated"}})]);
        assert_eq!((f[0].swap_mb, f[0].public, f[0].rxtx_factor, f[0].extra_specs.len()), (0, false, Some(1.0), 1), "`\"\"` swap is none; private");
    }

    #[test]
    fn flavor_access_names_projects_and_keeps_unknown_ids() {
        let raw = vec![json!({"flavor_id": "f1", "tenant_id": "p2"}), json!({"flavor_id": "f1", "tenant_id": "p1"})];
        let names = HashMap::from([("p1".to_string(), "Zeta".to_string())]);
        let names: Vec<String> = flavor_access(&raw, &names).into_iter().map(|(_, n)| n).collect();
        assert_eq!(names, ["p2", "Zeta"], "named, sorted case-insensitively; unknown → ID");
    }

    #[test]
    fn volume_types_qos_and_group_types() {
        let qos = vec![json!({"id": "q1", "name": "gold", "consumer": "back-end", "specs": {"total_iops_sec": "1000"}})];
        let t = volume_type_rows(&[json!({"id": "t1", "name": "ssd", "qos_specs_id": "q1", "os-volume-type-access:is_public": true}),
            json!({"id": "t2", "name": "enc", "description": "", "qos_specs_id": null, "is_public": false})], &[("t2".into(), luks())], &qos);
        assert_eq!(t.iter().map(|t| (t.name.as_str(), t.qos.as_ref().map(|q| q.name.as_str()), t.encrypted, t.public)).collect::<Vec<_>>(),
            [("enc", None, true, false), ("ssd", Some("gold"), false, true)]);
        assert_eq!(qos_specs(&qos)[0].specs, [("total_iops_sec".to_string(), "1000".to_string())]);
        assert_eq!(t[0].encryption, Some(luks()), "the spec prefills Update Encryption");
        let g = group_type_rows(&[json!({"id": "g1", "name": "", "is_public": true, "group_specs": {"consistent_group_snapshot_enabled": "<is> True"}})]);
        assert_eq!((g[0].name.as_str(), g[0].specs.len()), ("g1", 1), "unnamed → ID; specs read");
    }

    fn luks() -> EncryptionSpec {
        EncryptionSpec { provider: "luks".into(), control_location: "front-end".into(), cipher: Some("aes-xts-plain64".into()), key_size: Some(256) }
    }

    #[test]
    fn encryption_specs_read_numbers_or_strings_and_empty_is_none() {
        let e = encryption_spec(&json!({"provider": "luks", "control_location": "front-end", "cipher": "aes-xts-plain64", "key_size": 256, "encryption_id": "e1"}));
        assert_eq!(e, Some(luks()));
        assert_eq!(encryption_spec(&json!({"provider": "luks", "control_location": "back-end", "key_size": "512", "cipher": null})).and_then(|e| e.key_size), Some(512));
        assert_eq!(encryption_spec(&json!({})), None, "Cinder answers {{}} for a type without encryption");
    }

    #[test]
    fn rbac_policies_name_projects_networks_and_the_wildcard() {
        let names = HashMap::from([("p1".to_string(), "admin".to_string()), ("p2".to_string(), "demo".to_string())]);
        let nets = [json!({"id": "n1", "name": "public"})];
        let qos = [json!({"id": "q1", "name": "gold"})];
        let out = rbac_policies(&[
            json!({"id": "r1", "project_id": "p1", "object_type": "network", "object_id": "n1", "target_tenant": "*", "action": "access_as_external"}),
            json!({"id": "r2", "project_id": "p1", "object_type": "qos_policy", "object_id": "q1", "target_tenant": "p2", "action": "access_as_shared"}),
            json!({"id": "r3", "project_id": "p1", "object_type": "qos_policy", "object_id": "q9", "target_tenant": "p2", "action": "access_as_shared"}),
            json!({"id": "r4", "project_id": "p1", "object_type": "address_scope", "object_id": "n1", "target_tenant": "p2", "action": "access_as_shared"}),
        ], &[("network", &nets[..]), ("qos_policy", &qos[..])], &names);
        assert_eq!(
            out.iter().map(|r| (r.object.name.as_str(), r.target.as_str())).collect::<Vec<_>>(),
            [("n1", "demo"), ("public", "*"), ("gold", "demo"), ("q9", "demo")],
            "QoS policies named like networks; an unknown one, or another type's object with the same ID, keeps its ID"
        );
        assert_eq!(out.iter().map(|r| r.target_id.as_str()).collect::<Vec<_>>(), ["p2", "*", "p2", "p2"], "the target's ID, for Edit Policy");
    }

    #[test]
    fn another_projects_barbican_quotas_use_known_defaults_only() {
        // Roadmap 82: Barbican has no usage nor defaults per project; `v1/quotas`
        // of the session's project are the defaults only when it sets nothing.
        let target = json!({"project_quotas": {"secrets": 50, "orders": null}});
        let effective = json!({"quotas": {"secrets": -1, "containers": 20, "orders": 10, "consumers": -1, "cas": 5}});
        let rows = key_manager_project_rows(Some(&target), Some(None), &effective);
        let got: Vec<_> = rows.iter().map(|r| (r.key.as_str(), r.limit, r.used)).collect();
        assert_eq!(got, [("secrets", Some(50), None), ("containers", Some(20), None), ("orders", Some(10), None), ("consumers", None, None), ("cas", Some(5), None)]);
        let own = json!({"project_quotas": {"secrets": 3}});
        let rows = key_manager_project_rows(Some(&target), Some(Some(&own)), &effective);
        assert_eq!(rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), ["secrets"], "defaults unknown: only the target's own values");
        assert_eq!(key_manager_project_rows(None, None, &effective).len(), 0, "nothing set and defaults unknown: no rows");
        assert_eq!(key_manager_project_rows(None, Some(None), &effective).len(), 5, "nothing set: every default");
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
        assert_eq!(image_ids(&servers), ["img1"], "each image once; boot-from-volume servers have none");
        let rows = instances_page(&servers, &[]);
        let pending: Vec<(&str, Vec<String>)> = rows.iter().map(|i| (i.id.as_str(), i.boot_volume_ids.clone())).collect();
        assert_eq!(
            pending,
            [("a", vec![]), ("b", vec![]), ("c", vec!["vol1".to_string(), "vol2".to_string()]), ("d", vec!["vol1".to_string()]), ("e", vec![])],
            "only boot-from-volume servers wait for their volumes; a data volume of an image-booted server is not read"
        );
        assert!(rows[2].image_name.is_none() && rows[3].image_name.is_none(), "no volume is read for the page itself: the image comes later");
        assert!(image_ids(&[]).is_empty());
    }

    #[test]
    fn boot_images_are_found_from_the_volumes_alone() {
        let volumes = vec![
            // c: a data volume first, then the root one.
            json!({"id": "data", "bootable": "false", "attachments": [{"server_id": "c", "device": "/dev/vdb"}]}),
            json!({"id": "root", "bootable": "true", "attachments": [{"server_id": "c", "device": "/dev/vda"}],
                   "volume_image_metadata": {"image_name": "rhel-9.4", "image_id": "img-r"}}),
            // d: no conventional device, the bootable volume decides; only an image ID.
            json!({"id": "v2", "bootable": "true", "attachments": [{"server_id": "d", "device": "/dev/xyz"}],
                   "volume_image_metadata": {"image_id": "img-u"}}),
            // e: a volume without image metadata names nothing.
            json!({"id": "v3", "bootable": "true", "attachments": [{"server_id": "e", "device": "/dev/vda"}]}),
        ];
        assert_eq!(boot_images(&volumes), [("c".to_string(), "rhel-9.4".to_string()), ("d".to_string(), "img-u".to_string())]);
        assert!(boot_images(&[]).is_empty());
    }

    #[test]
    fn volume_snapshot_and_floating_ip_pages_keep_the_apis_order() {
        let vols = vec![json!({"id": "v2", "name": "zeta", "attachments": [{"server_id": "s1", "device": "/dev/vdb"}]}), json!({"id": "v1", "name": "alpha"})];
        let page = volumes_page(&vols, &[]);
        assert_eq!(page.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["v2", "v1"], "Cinder's order: the marker is the last row");
        assert_eq!((page[0].attachments[0].server_id.as_str(), page[0].attachments[0].server_name.as_deref()), ("s1", None), "named later");
        let snaps = vec![json!({"id": "a", "created_at": "2020-01-01", "volume_id": "v1"}), json!({"id": "b", "created_at": "2026-01-01", "volume_id": "v1"})];
        assert_eq!(snapshots_page(&snaps, &[]).iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        let fips = vec![
            json!({"id": "f2", "floating_ip_address": "10.0.0.9", "floating_network_id": "ext", "port_details": {"device_id": "s1", "device_owner": "compute:nova"}}),
            json!({"id": "f1", "floating_ip_address": "10.0.0.1", "floating_network_id": "ext"}),
        ];
        let page = floating_ips_page(&fips, &[json!({"id": "ext", "name": "public"})], &[]);
        assert_eq!(page.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), ["f2", "f1"]);
        assert_eq!((page[0].pool.as_str(), page[0].instance_id.as_deref(), page[0].instance_name.as_deref()), ("public", Some("s1"), None));
        assert_eq!(pool_ids(&fips), ["ext"], "one network lookup for the whole page");
    }

    #[test]
    fn slice_c_pages_keep_the_apis_order_and_name_from_the_pages_lookups() {
        let ids = |v: Vec<String>| v;
        let nets = vec![json!({"id": "n2", "name": "zeta", "subnets": ["s1"]}), json!({"id": "n1", "name": "alpha", "subnets": ["gone"]})];
        let page = networks_page(&nets, &[json!({"id": "s1", "name": "a", "cidr": "10.0.0.0/24"})], "");
        assert_eq!(page.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), ["n2", "n1"], "Neutron's order: the marker is the last row");
        assert_eq!((page[0].subnets[0].as_str(), page[1].subnets[0].as_str()), ("a 10.0.0.0/24", "gone"), "a subnet not read stays an ID");
        assert_eq!(row_ids(&nets), ["n1", "n2"]);

        let routers = vec![
            json!({"id": "r2", "name": "z", "external_gateway_info": {"network_id": "ext"}}),
            json!({"id": "r1", "name": "a", "external_gateway_info": null}),
            json!({"id": "r3", "name": "b", "external_gateway_info": {"network_id": "ext"}}),
        ];
        assert_eq!(gateway_ids(&routers), ["ext"], "one network lookup, null gateways skipped");
        let page = routers_page(&routers, &[json!({"id": "ext", "name": "public"})]);
        assert_eq!((page[0].id.as_str(), page[0].external_network.as_deref(), page[1].external_network.as_deref()), ("r2", Some("public"), None));

        let trunks = vec![json!({"id": "t2", "name": "z", "port_id": "p1", "sub_ports": [{"port_id": "p2", "segmentation_type": "vlan", "segmentation_id": 10}, {"port_id": "p1"}]}), json!({"id": "t1", "name": "a", "port_id": "p3"})];
        assert_eq!(ids(trunk_port_ids(&trunks)), ["p1", "p2", "p3"], "parents and sub-ports, each once");
        let page = trunks_page(&trunks, &[json!({"id": "p1", "name": "parent"})]);
        assert_eq!((page[0].id.as_str(), page[0].parent.name.as_str(), page[1].parent.name.as_str()), ("t2", "parent", "p3"), "a port not read stays an ID");

        let images = vec![json!({"id": "i2", "name": "zeta", "owner": "p"}), json!({"id": "i1", "name": "alpha"})];
        let names = HashMap::from([("p".to_string(), "proj".to_string())]);
        let page = images_page(&images, "", &names);
        assert_eq!((page[0].id.as_str(), page[0].owner_name.as_deref()), ("i2", Some("proj")), "Glance's order, not owned-first by name");

        let groups = vec![json!({"id": "g2", "name": "z"}), json!({"id": "g1", "name": "a"})];
        let page = volume_groups_page(&groups, &[], &[], &[json!({"id": "x", "group_id": "g1"})]);
        assert_eq!((page[0].id.as_str(), page[0].has_snapshots, page[1].has_snapshots), ("g2", false, true));
        let gs = vec![json!({"id": "a", "created_at": "2020", "group_id": "g1"}), json!({"id": "b", "created_at": "2026", "group_id": "g1"}), json!({"id": "c", "group_id": "g2"})];
        assert_eq!(group_ids(&gs), ["g1", "g2"]);
        let page = group_snapshots_page(&gs, &[json!({"id": "g1", "name": "db"})], &[]);
        assert_eq!(page.iter().map(|x| (x.id.as_str(), x.group.name.as_str())).collect::<Vec<_>>(), [("a", "db"), ("b", "db"), ("c", "g2")], "Cinder's order, not newest first");
    }

    #[test]
    fn late_names_come_from_wrapped_or_bare_bodies() {
        let srv = late_name(&json!({"server": {"id": "s1", "name": "web"}})).unwrap();
        assert_eq!((srv.name.as_str(), srv.host), ("web", None));
        let vol = late_name(&json!({"volume": {"id": "v1", "name": "", "os-vol-host-attr:host": "c1@ceph#rbd"}})).unwrap();
        assert_eq!((vol.name.as_str(), vol.host.as_deref()), ("v1", Some("c1@ceph#rbd")), "unnamed: the ID");
        assert!(late_name(&json!({})).is_none());
    }

    #[test]
    fn instances_page_keeps_the_apis_order() {
        let servers = vec![
            json!({"id": "old", "name": "z", "created": "2020-01-01T00:00:00Z", "status": "ACTIVE"}),
            json!({"id": "new", "name": "a", "created": "2026-01-01T00:00:00Z", "status": "ACTIVE"}),
        ];
        let ids: Vec<_> = instances_page(&servers, &[]).into_iter().map(|i| i.id).collect();
        assert_eq!(ids, ["old", "new"], "the next-page marker is the last row, so the order is the API's, not newest-first");
    }
}
