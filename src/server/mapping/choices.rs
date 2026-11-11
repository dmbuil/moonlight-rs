//! Dialog choices: the launch wizard tables (one cell per column, raw booleans) and the other pickers.

use super::*;

fn yes(v: bool) -> String {
    v.to_string()
}

fn gb(n: Option<i64>) -> String {
    format!("{} GB", n.unwrap_or(0))
}

fn mib_text(n: Option<i64>) -> String {
    crate::i18n::bytes((n.unwrap_or(0).max(0) as u64) * 1024 * 1024)
}

pub(super) fn when(v: &Value, k: &str) -> String {
    s(v, k)
        .map(|t| crate::components::widgets::short_time(&t))
        .unwrap_or_else(|| "-".into())
}

pub(super) fn sorted(mut out: Vec<Choice>) -> Vec<Choice> {
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
                    yes(f
                        .get("os-flavor-access:is_public")
                        .and_then(Value::as_bool)
                        .unwrap_or(true)),
                ],
                facts: Facts {
                    vcpus: i(f, "vcpus"),
                    ram_mb: i(f, "ram"),
                    disk_gb: disk,
                    ..Default::default()
                },
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
                    facts: Facts {
                        min_disk_gb: i(v, "size"),
                        ..Default::default()
                    },
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
                    facts: Facts {
                        min_disk_gb: i(v, "size"),
                        ..Default::default()
                    },
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

/// Networks a router with an external gateway is attached to: the only ones
/// whose ports a floating IP can reach (Horizon's `enable_fip_topology_check`).
/// `ports` must include the routers' interface ports.
pub fn externally_reachable_networks(ports: &[Value], routers: &[Value]) -> std::collections::HashSet<String> {
    let gateways: std::collections::HashSet<String> = routers
        .iter()
        .filter(|r| {
            r.get("external_gateway_info")
                .and_then(|g| s_ne(g, "network_id"))
                .is_some()
        })
        .filter_map(|r| s(r, "id"))
        .collect();
    ports
        .iter()
        .filter(|p| is_router_interface(p))
        .filter(|p| s(p, "device_id").is_some_and(|d| gateways.contains(&d)))
        .filter_map(|p| s(p, "network_id"))
        .collect()
}

/// A router's interface port: plain, DVR (`…_distributed`) or an HA router's
/// (`network:ha_router_replicated_interface`), as Horizon's
/// `ROUTER_INTERFACE_OWNERS`.
fn is_router_interface(port: &Value) -> bool {
    s(port, "device_owner")
        .is_some_and(|o| o.starts_with("network:router_interface") || o == "network:ha_router_replicated_interface")
}

/// For each external network (floating IP pool), the subnets that a router
/// with its gateway there has an interface on: a floating IP of that pool
/// reaches only ports on them (Neutron refuses the others; Horizon's
/// `FloatingIpManager._get_reachable_subnets`). `ports` must include the
/// routers' interface ports.
pub fn reachable_subnets_by_pool(
    ports: &[Value],
    routers: &[Value],
) -> HashMap<String, std::collections::HashSet<String>> {
    let gateway: HashMap<String, String> = routers
        .iter()
        .filter_map(|r| Some((s(r, "id")?, s_ne(r.get("external_gateway_info")?, "network_id")?)))
        .collect();
    let mut out: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    for p in ports.iter().filter(|p| is_router_interface(p)) {
        let Some(pool) = s(p, "device_id").and_then(|d| gateway.get(&d)) else {
            continue;
        };
        out.entry(pool.clone())
            .or_default()
            .extend(arr(p, "fixed_ips").iter().filter_map(|f| s(f, "subnet_id")));
    }
    out
}

/// Instance ports (`compute:*`), labelled "server: ip". The first cell is the
/// instance ID (for `Source::Of`), the second the port's security group IDs,
/// comma-separated (to prefill the editor). `without_fip` drops ports that
/// already have a floating IP (association); `reachable` (when set) keeps only
/// those on those networks.
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
                    cells: vec![
                        s(p, "device_id").unwrap_or_default(),
                        strs(p, "security_groups").join(","),
                    ],
                    ..Default::default()
                })
            })
            .collect(),
    )
}

/// Which pools reach a port, for [`owned_port_choices`].
pub enum PoolReach<'a> {
    /// `[network] fip_topology_check`: the pools whose gateway routers have an
    /// interface on one of the port's subnets ([`reachable_subnets_by_pool`]).
    Subnets(&'a HashMap<String, std::collections::HashSet<String>>),
    /// The check is off: every one of these pools (the external networks).
    Every(&'a [String]),
}

/// [`instance_port_choices`] (without a floating IP) for every project, once
/// per pool that reaches the port, keyed first by `model::associable_port_key`
/// (`AllAssociablePorts`).
pub fn owned_port_choices(
    ports: &[Value],
    fips: &[Value],
    servers: &HashMap<String, String>,
    reach: PoolReach,
) -> Vec<Choice> {
    let by_id: HashMap<String, &Value> = ports.iter().filter_map(|p| Some((s(p, "id")?, p))).collect();
    let mut out = Vec::new();
    for c in instance_port_choices(ports, fips, servers, true, None) {
        let Some(p) = by_id.get(&c.value) else { continue };
        let owner = s_ne(p, "project_id")
            .or_else(|| s_ne(p, "tenant_id"))
            .unwrap_or_default();
        let pools: Vec<&String> = match &reach {
            PoolReach::Every(pools) => pools.iter().collect(),
            PoolReach::Subnets(by_pool) => {
                let subnets: Vec<String> = arr(p, "fixed_ips").iter().filter_map(|f| s(f, "subnet_id")).collect();
                let mut v: Vec<&String> = by_pool
                    .iter()
                    .filter(|(_, reached)| subnets.iter().any(|sn| reached.contains(sn)))
                    .map(|(pool, _)| pool)
                    .collect();
                v.sort();
                v
            }
        };
        for pool in pools {
            let mut c = c.clone();
            c.cells.insert(0, crate::model::associable_port_key(&owner, pool));
            out.push(c);
        }
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
                let dev = s(p, "device_id")
                    .and_then(|d| servers.get(&d).cloned())
                    .unwrap_or_else(|| name_or_id(p));
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
pub fn floating_ip_detail(
    f: &Value,
    rules: &[Value],
    networks: &[Value],
    servers: &[Value],
    ports: &[Value],
) -> FloatingIpDetail {
    let row = floating_ips(std::slice::from_ref(f), networks, servers).remove(0);
    let server_names = names_by_id(servers);
    let owner: HashMap<String, String> = ports
        .iter()
        .filter_map(|p| Some((s(p, "id")?, s(p, "device_id")?)))
        .collect();
    // Ranges come as `*_range`; single ports as numbers.
    let ports_of = |r: &Value, key: &str| {
        s_ne(r, &format!("{key}_range"))
            .or_else(|| i(r, key).map(|n| n.to_string()))
            .unwrap_or_else(|| "-".into())
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
                instance: owner.get(&port).map(|d| Ref {
                    id: d.clone(),
                    name: server_names.get(d).cloned().unwrap_or_else(|| d.clone()),
                }),
                internal_port_id: port,
                description: s_ne(r, "description"),
            }
        })
        .collect();
    forwardings
        .sort_by(|a, b| (a.protocol.as_str(), a.external.as_str()).cmp(&(b.protocol.as_str(), b.external.as_str())));
    FloatingIpDetail {
        row,
        port_id: s_ne(f, "port_id"),
        router_id: s_ne(f, "router_id"),
        created: s_ne(f, "created_at"),
        forwardings,
    }
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
                let dev = s(p, "device_id")
                    .and_then(|d| servers.get(&d).cloned())
                    .unwrap_or_else(|| name_or_id(p));
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
            .filter_map(|x| {
                Some(Choice::new(
                    s(x, "id")?,
                    s_ne(x, "name").or_else(|| s(x, "cidr")).or_else(|| s(x, "id"))?,
                ))
            })
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
                let policy = s(g, "policy")
                    .or_else(|| strs(g, "policies").into_iter().next())
                    .unwrap_or_default();
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
    let net = s(sn, "network_id")
        .and_then(|id| net_names.get(&id).cloned())
        .unwrap_or_default();
    format!("{}: {} ({})", net, s(sn, "cidr").unwrap_or_default(), name_or_id(sn))
}

/// A port choice label: the server it belongs to (or the port's name) and its
/// first address.
pub fn label_port(p: &Value, server_names: &HashMap<String, String>) -> String {
    let ip = arr(p, "fixed_ips")
        .first()
        .and_then(|f| s(f, "ip_address"))
        .unwrap_or_default();
    let dev = s(p, "device_id")
        .and_then(|d| server_names.get(&d).cloned())
        .unwrap_or_else(|| name_or_id(p));
    format!("{dev}: {ip}")
}

/// IDs of networks with a subnet (in their `subnets` list) that `subnets`
/// lacks: the external networks of another project, whose subnets neither the
/// `tenant_id` nor the `shared` filter returns (roadmap 64).
pub fn networks_without_subnets(nets: &[Value], subnets: &[Value]) -> Vec<String> {
    let have: std::collections::HashSet<String> = subnets.iter().filter_map(|x| s(x, "id")).collect();
    nets.iter()
        .filter(|n| strs(n, "subnets").iter().any(|id| !have.contains(id)))
        .filter_map(|n| s(n, "id"))
        .collect()
}

/// Network ID → name.
pub fn network_names(nets: &[Value]) -> HashMap<String, String> {
    names_by_id(nets)
}

/// Server ID → name.
pub fn server_names(servers: &[Value]) -> HashMap<String, String> {
    names_by_id(servers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lb_zones_are_the_enabled_ones_by_name() {
        let z = lb_az_choices(&[
            json!({"name": "az-b", "description": "Madrid", "enabled": true}),
            json!({"name": "az-a", "enabled": true}),
            json!({"name": "off", "enabled": false}),
        ]);
        assert_eq!(
            z.iter()
                .map(|c| (c.value.as_str(), c.label.as_str()))
                .collect::<Vec<_>>(),
            [("az-a", "az-a"), ("az-b", "az-b (Madrid)")]
        );
    }

    #[test]
    fn flavors_follow_the_configured_order() {
        let f = |id: &str, name: &str, ram: i64, vcpus: i64| json!({"id": id, "name": name, "ram": ram, "vcpus": vcpus, "disk": 10});
        let raw = vec![
            f("3", "m1.large", 8192, 4),
            f("1", "m1.small", 2048, 1),
            f("2", "c1.big", 4096, 8),
        ];
        let names = |sort, rev| {
            flavor_choices(&raw, sort, rev)
                .into_iter()
                .map(|c| c.cells[0].clone())
                .collect::<Vec<_>>()
        };
        use crate::prefs::FlavorSort::*;
        assert_eq!(names(Ram, false), ["m1.small", "c1.big", "m1.large"], "default: by RAM");
        assert_eq!(names(Ram, true), ["m1.large", "c1.big", "m1.small"]);
        assert_eq!(names(Vcpus, false), ["m1.small", "m1.large", "c1.big"]);
        assert_eq!(names(Id, false), ["m1.small", "c1.big", "m1.large"]);
    }

    #[test]
    fn choices_are_sorted_with_readable_labels() {
        let c = choices(
            &[json!({"id": "2", "name": "b"}), json!({"id": "1", "name": "A"})],
            "id",
            label_name,
        );
        assert_eq!(c[0].label, "A");
        let f = label_flavor(&json!({"name": "m1", "vcpus": 2, "ram": 4096, "disk": 20}));
        assert_eq!(f, "m1 · 2 vCPU · 4 GB · 20 GB");
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
        assert_eq!(
            imgs[0].cells,
            vec!["ADM-RHEL9.7-ES", "2026-09-25 14:09", "7.3 GB", "QCOW2", "shared"]
        );
        assert_eq!(
            (imgs[0].facts.min_disk_gb, imgs[0].facts.size_bytes),
            (Some(20), Some(7_800_000_000))
        );
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
        let nets = vec![
            json!({"id": "n1", "name": "sadasd", "subnets": ["sn1", "gone"], "shared": false,
            "admin_state_up": true, "status": "ACTIVE"}),
        ];
        let subs = vec![json!({"id": "sn1", "name": "test-geneve", "cidr": "10.0.0.0/24"})];
        assert_eq!(
            network_choices(&nets, &subs)[0].cells[1],
            "test-geneve 10.0.0.0/24, gone"
        );
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
        let raw = vec![
            json!({"id": "g1", "name": "a", "policy": "anti-affinity"}),
            json!({"id": "g2", "name": "b", "policies": ["affinity"]}),
        ];
        let c = server_group_choices(&raw);
        assert_eq!(
            (c[0].cells[1].as_str(), c[1].cells[1].as_str()),
            ("anti-affinity", "affinity")
        );
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
        assert_eq!(
            (ips.len(), ips[0].value.as_str(), ips[0].label.as_str()),
            (1, "f2", "1.2.3.5")
        );
    }

    #[test]
    fn lb_choices_carry_subnets_and_skip_disabled_flavors() {
        let ports = vec![json!({"id": "p1", "device_owner": "compute:nova", "device_id": "s1",
                                "fixed_ips": [{"ip_address": "10.0.0.5", "subnet_id": "sub-1"}, {"ip_address": "fd00::5", "subnet_id": "sub-6"}]})];
        let subnets = [json!({"id": "sub-1", "name": "web-subnet"})];
        let c = lb_member_choices(
            &ports,
            &HashMap::from([("s1".to_string(), "web".to_string())]),
            &subnets,
        );
        let v: Vec<_> = c.iter().map(|c| (c.value.as_str(), c.label.as_str())).collect();
        assert_eq!(
            v,
            [("10.0.0.5@sub-1", "web: 10.0.0.5"), ("fd00::5@sub-6", "web: fd00::5")]
        );
        assert_eq!(
            c[0].cells,
            ["web", "10.0.0.5", "web-subnet"],
            "instance, IP and subnet name for the tables"
        );
        assert_eq!(c[1].cells[2], "sub-6", "an unknown subnet keeps its ID");
        let nets = [json!({"id": "n-own"}), json!({"id": "n-shared"})];
        let all = [
            json!({"id": "s1", "name": "own-subnet", "network_id": "n-own"}),
            json!({"id": "s2", "name": "", "cidr": "203.0.113.0/24", "network_id": "n-shared"}),
            json!({"id": "s3", "name": "other-project", "network_id": "n-other"}),
        ];
        let sub: Vec<_> = lb_member_subnet_choices(&nets, &all)
            .into_iter()
            .map(|c| (c.value, c.label))
            .collect();
        assert_eq!(
            sub,
            [
                ("s2".to_string(), "203.0.113.0/24".to_string()),
                ("s1".into(), "own-subnet".into())
            ],
            "by name, else CIDR; other projects' left out"
        );
        let f = lb_flavor_choices(&[
            json!({"id": "f1", "name": "small", "description": "1 amphora", "enabled": true}),
            json!({"id": "f2", "name": "off", "enabled": false}),
        ]);
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
        assert_eq!(
            instance_port_choices(&ports, &[], &names, true, None).len(),
            3,
            "check off"
        );
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
        let pools = ["public".to_string()];
        let got: Vec<(String, String)> = owned_port_choices(&ports, &fips, &HashMap::new(), PoolReach::Every(&pools))
            .into_iter()
            .map(|c| (c.value, c.cells[0].clone()))
            .collect();
        assert_eq!(got.len(), 2, "no DHCP port, no port that has a floating IP: {got:?}");
        assert!(
            got.contains(&("pa".into(), "p1 public".into())) && got.contains(&("pb".into(), "p2 public".into())),
            "older Neutron says tenant_id: {got:?}"
        );
    }
}
