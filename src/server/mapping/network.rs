//! Neutron: networks, subnets, ports, routers, topology, security groups, floating IPs, trunks, QoS.

use super::*;

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
    raw.iter()
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
        segmentation_id: n
            .get("provider:segmentation_id")
            .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
            .and_then(|v| u32::try_from(v).ok()),
    })
}

/// Every project's Neutron objects (networks, QoS policies) as choices labelled
/// `name (project)`, by label: an admin sees the same names in many projects.
pub fn owned_choices(raw: &[Value], names: &HashMap<String, String>) -> Vec<Choice> {
    let mut out: Vec<Choice> = raw
        .iter()
        .filter_map(|v| {
            let id = s(v, "id")?;
            let owner = s_ne(v, "project_id")
                .or_else(|| s_ne(v, "tenant_id"))
                .map(|p| names.get(&p).cloned().unwrap_or(p));
            let name = name_or_id(v);
            Some(Choice::new(
                id,
                match owner {
                    Some(p) => format!("{name} ({p})"),
                    None => name,
                },
            ))
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
    let mut names: Vec<String> = arr(v, "resource_types")
        .iter()
        .filter_map(|t| s_ne(t, "name"))
        .collect();
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
    let mut hosts: Vec<String> = host_services(raw)
        .into_iter()
        .filter(|s| !s.host.is_empty() && (!usable || (s.enabled && s.up)))
        .map(|s| s.host)
        .collect();
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
    distinct(raw.iter().filter_map(|r| {
        r.get("external_gateway_info")
            .filter(|g| !g.is_null())
            .and_then(|g| s(g, "network_id"))
    }))
}

/// [`routers`] in Neutron's order, for a paged list (roadmap 88); `networks`
/// need only be the page's gateways ([`gateway_ids`]).
pub fn routers_page(raw: &[Value], networks: &[Value]) -> Vec<RouterRow> {
    let net_names = names_by_id(networks);
    raw.iter()
        .map(|r| {
            let gw = r.get("external_gateway_info").filter(|g| !g.is_null());
            let ext_id = gw.and_then(|g| s(g, "network_id"));
            RouterRow {
                id: s(r, "id").unwrap_or_default(),
                name: name_or_id(r),
                status: s(r, "status").unwrap_or_default(),
                external_network: ext_id
                    .as_ref()
                    .map(|id| net_names.get(id).cloned().unwrap_or_else(|| id.clone())),
                external_network_id: ext_id,
                snat: gw.and_then(|g| g.get("enable_snat")).and_then(Value::as_bool),
                admin_up: b(r, "admin_state_up"),
                availability_zones: strs(r, "availability_zones"),
                distributed: r.get("distributed").and_then(Value::as_bool),
                ha: r.get("ha").and_then(Value::as_bool),
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

pub(super) fn fixed_ips(v: &Value, key: &str) -> Vec<String> {
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
    networks.sort_by(|a, b| {
        b.external
            .cmp(&a.external)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    let device = |v: &Value, kind| {
        Some(TopoDevice {
            id: s(v, "id")?,
            name: name_or_id(v),
            kind,
            status: s(v, "status").unwrap_or_default(),
        })
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
            Some(TopoKind::Router) => {
                owner.starts_with("network:router_interface")
                    || owner == "network:ha_router_replicated_interface"
                    || owner == "network:router_gateway"
            }
            Some(TopoKind::Server) => owner.starts_with("compute:"),
            None => false,
        };
        if !wanted {
            continue;
        }
        let gateway = owner == "network:router_gateway";
        // The gateway seen through the router already: keep one, with its port.
        if let Some(l) = links
            .iter_mut()
            .find(|l| l.gateway && gateway && l.device_id == dev && l.network_id == net)
        {
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
    Topology {
        networks,
        devices,
        links,
        max_networks: 0,
    }
}

/// The rules of each named group, remote groups shown by name. Groups the
/// project can't list (or ambiguous names) get no rules.
pub(super) fn group_rules(names: &[String], groups: &[Value]) -> Vec<(String, Vec<String>)> {
    let by_id = names_by_id(groups);
    names
        .iter()
        .filter_map(|n| {
            let mut same = groups.iter().filter(|g| s(g, "name").as_deref() == Some(n.as_str()));
            let g = same.next().filter(|_| same.next().is_none())?;
            Some((
                n.clone(),
                arr(g, "security_group_rules")
                    .iter()
                    .map(|r| describe_rule_with(r, &by_id))
                    .collect(),
            ))
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
    raw.iter()
        .map(|f| {
            let details = f.get("port_details").cloned().unwrap_or(Value::Null);
            let instance_id = s(&details, "device_id")
                .filter(|_| s(&details, "device_owner").is_some_and(|o| o.starts_with("compute:")));
            let pool_id = s(f, "floating_network_id").unwrap_or_default();
            FloatingIpRow {
                pool_id: pool_id.clone(),
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
    Some(LateName {
        id: s(v, "id")?,
        name: name_or_id(v),
        host: s_ne(v, "os-vol-host-attr:host"),
    })
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
            .map(|p| {
                format!(
                    "{} – {}",
                    s(p, "start").unwrap_or_default(),
                    s(p, "end").unwrap_or_default()
                )
            })
            .collect(),
        dns_nameservers: strs(sn, "dns_nameservers"),
        host_routes: arr(sn, "host_routes")
            .iter()
            .map(|r| {
                format!(
                    "{} → {}",
                    s(r, "destination").unwrap_or_default(),
                    s(r, "nexthop").unwrap_or_default()
                )
            })
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
pub fn port(
    p: &Value,
    net_names: &HashMap<String, String>,
    subnet_names: &HashMap<String, String>,
    server_names: &HashMap<String, String>,
    sg_names: &HashMap<String, String>,
) -> PortRow {
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
            .map(|x| {
                (
                    s(x, "destination").unwrap_or_default(),
                    s(x, "nexthop").unwrap_or_default(),
                )
            })
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
    let mut rules: Vec<RuleRow> = arr(g, "security_group_rules")
        .iter()
        .map(|r| rule_row(r, &names))
        .collect();
    rules.sort_by(|a, b| (&a.direction, &a.ethertype, &a.protocol).cmp(&(&b.direction, &b.ethertype, &b.protocol)));
    SecurityGroupDetail {
        id: s(g, "id").unwrap_or_default(),
        name: name_or_id(g),
        description: s_ne(g, "description"),
        project_id: s(g, "project_id").or_else(|| s(g, "tenant_id")).unwrap_or_default(),
        rules,
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

/// Trunks with their parent port and sub-ports named from the project's ports.
pub fn trunks(raw: &[Value], ports: &[Value]) -> Vec<TrunkRow> {
    let mut out = trunks_page(raw, ports);
    out.sort_by_key(|t| t.name.to_lowercase());
    out
}

/// The ports a page of trunks names (parents and sub-ports), each once (roadmap 88).
pub fn trunk_port_ids(raw: &[Value]) -> Vec<String> {
    distinct(raw.iter().flat_map(|t| {
        s(t, "port_id")
            .into_iter()
            .chain(arr(t, "sub_ports").iter().filter_map(|sp| s(sp, "port_id")))
    }))
}

/// [`trunks`] in Neutron's order, for a paged list (roadmap 88); `ports` need
/// only be the page's ([`trunk_port_ids`]).
pub fn trunks_page(raw: &[Value], ports: &[Value]) -> Vec<TrunkRow> {
    let port_names = names_by_id(ports);
    let named = |id: String| Ref {
        name: port_names.get(&id).cloned().unwrap_or_else(|| id.clone()),
        id,
    };
    raw.iter()
        .map(|t| {
            let mut subports: Vec<_> = arr(t, "sub_ports")
                .iter()
                .map(|sp| {
                    let r = named(s(sp, "port_id").unwrap_or_default());
                    Subport {
                        port_id: r.id,
                        port_name: r.name,
                        segmentation_type: s(sp, "segmentation_type").unwrap_or_default(),
                        segmentation_id: i(sp, "segmentation_id"),
                    }
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
    sorted(
        raw.iter()
            .filter_map(|p| Some(Choice::new(s(p, "id")?, name_or_id(p))))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::*;
    use super::*;
    use serde_json::json;

    #[test]
    fn trunks_name_their_parent_and_subports() {
        let raw = vec![
            json!({"id": "t1", "name": "", "port_id": "p0", "status": "ACTIVE", "admin_state_up": true,
                              "sub_ports": [{"port_id": "p2", "segmentation_type": "vlan", "segmentation_id": 200},
                                            {"port_id": "p1", "segmentation_type": "vlan", "segmentation_id": 100}]}),
        ];
        let ports = vec![
            json!({"id": "p0", "name": "parent"}),
            json!({"id": "p1", "name": "vlan100"}),
        ];
        let t = &trunks(&raw, &ports)[0];
        assert_eq!(
            (t.name.as_str(), t.parent.name.as_str(), t.status.as_str()),
            ("t1", "parent", "ACTIVE"),
            "unnamed: the ID"
        );
        let sp: Vec<_> = t
            .subports
            .iter()
            .map(|s| (s.port_name.as_str(), s.segmentation_id))
            .collect();
        assert_eq!(
            sp,
            [("vlan100", Some(100)), ("p2", Some(200))],
            "by VLAN; unknown port: its ID"
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
        assert_eq!(
            (
                p[0].name.as_str(),
                p[0].owned,
                p[0].is_default,
                p[0].description.clone()
            ),
            ("gold", true, true, None)
        );
        assert_eq!((p[1].owned, p[1].shared), (false, true));
        assert_eq!(
            (
                p[0].rules[0].kind.as_str(),
                p[0].rules[0].max_kbps,
                p[0].rules[1].dscp_mark
            ),
            ("bandwidth_limit", Some(10000), Some(26))
        );
        assert_eq!(
            qos_policy_choices(&raw)
                .iter()
                .map(|c| c.label.as_str())
                .collect::<Vec<_>>(),
            ["gold", "shared-silver"]
        );
    }

    #[test]
    fn load_balancer_detail_joins_listeners_pools_members_and_monitors() {
        let (lb, listeners, pools, members, monitors) = octavia();
        let networks = vec![json!({"id": "net-1", "name": "web-net"})];
        let subnets = vec![json!({"id": "sub-1", "name": "web-subnet"})];
        let fips = vec![
            json!({"floating_ip_address": "203.0.113.7", "port_id": "port-vip"}),
            json!({"floating_ip_address": "203.0.113.8", "port_id": "other"}),
        ];
        // L7 (roadmap 54): the project's policies, one on another load balancer's listener.
        let (ls_id, pool_id) = (
            listeners[0]["id"].as_str().unwrap().to_string(),
            pools[0]["id"].as_str().unwrap().to_string(),
        );
        let l7policies = vec![
            json!({"id": "pol-1", "name": "api", "listener_id": ls_id, "action": "REDIRECT_TO_POOL", "position": 1,
                   "redirect_pool_id": pool_id, "redirect_url": null, "admin_state_up": true,
                   "operating_status": "ONLINE", "provisioning_status": "ACTIVE"}),
            json!({"id": "pol-x", "name": "elsewhere", "listener_id": "other-listener", "action": "REJECT"}),
        ];
        let l7rules = vec![(
            "pol-1".to_string(),
            vec![
                json!({"id": "r1", "type": "PATH", "compare_type": "STARTS_WITH", "key": null, "value": "/api", "invert": false, "admin_state_up": true}),
                json!({"id": "r2", "type": "HEADER", "compare_type": "EQUAL_TO", "key": "X-Env", "value": "test", "invert": true, "admin_state_up": true}),
            ],
        )];
        let d = load_balancer_detail(
            &lb,
            LbInputs {
                listeners: &listeners,
                pools: &pools,
                members: &members,
                monitors: &monitors,
                networks: &networks,
                subnets: &subnets,
                floating_ips: &fips,
                l7policies: &l7policies,
                l7rules: &l7rules,
            },
        );
        assert_eq!(d.l7policies.len(), 1, "only this load balancer's listeners");
        let pol = &d.l7policies[0];
        assert_eq!(
            (
                pol.redirect_pool.as_ref().map(|p| p.name.as_str()),
                pol.position,
                pol.rules.len()
            ),
            (Some("web"), Some(1), 2)
        );
        assert_eq!(
            (
                pol.rules[0].rule_type.as_str(),
                pol.rules[0].key.as_deref(),
                pol.rules[0].invert
            ),
            ("HEADER", Some("X-Env"), true)
        );
        assert_eq!(d.floating_ip.as_deref(), Some("203.0.113.7"));
        assert_eq!(d.vip_network.as_ref().map(|r| r.name.as_str()), Some("web-net"));
        assert_eq!(d.vip_subnet.as_ref().map(|r| r.name.as_str()), Some("web-subnet"));
        assert_eq!((d.flavor_id.clone(), d.updated.clone()), (None, None));

        let ports: Vec<_> = d.listeners.iter().map(|l| (l.port, l.name.as_str())).collect();
        assert_eq!(
            ports,
            [(Some(80), "ls-80"), (Some(443), "https")],
            "by port; unnamed → ID"
        );
        assert_eq!(
            d.listeners[1].default_pool.as_ref().map(|p| p.name.as_str()),
            Some("web")
        );
        assert_eq!(
            (d.listeners[1].connection_limit, d.listeners[0].connection_limit),
            (None, Some(5000)),
            "-1 = unlimited"
        );

        let p = &d.pools[0];
        assert_eq!(p.session_persistence.as_deref(), Some("APP_COOKIE (JSESSIONID)"));
        assert_eq!((p.members, p.listeners[0].name.as_str()), (2, "https"));
        assert_eq!(p.monitor.as_ref().map(|m| m.name.as_str()), Some("hm-1"));

        let addrs: Vec<_> = d.members.iter().map(|m| m.address.as_str()).collect();
        assert_eq!(addrs, ["10.0.0.11", "10.0.0.12"]);
        assert_eq!(
            (
                d.members[0].pool.name.as_str(),
                d.members[0].backup,
                d.members[0].subnet.clone()
            ),
            ("web", true, None)
        );
        assert_eq!(
            d.members[1].subnet.as_ref().map(|s| s.name.as_str()),
            Some("web-subnet")
        );
        assert_eq!(
            (d.members[1].monitor_address.as_deref(), d.members[1].monitor_port),
            (Some("10.0.9.12"), Some(9000))
        );
        assert_eq!(
            (d.members[0].monitor_address.clone(), d.members[0].monitor_port),
            (None, None),
            "absent: the member itself"
        );

        assert_eq!(d.monitors.len(), 1, "monitors of other load balancers are dropped");
        assert_eq!(d.monitors[0].http_check.as_deref(), Some("GET /healthz → 200-204"));
        assert_eq!(d.monitors[0].pools[0].name, "web");
    }

    #[test]
    fn detail_joins_volumes_interfaces_actions_and_fault() {
        let mut sv = server_fixture();
        sv["fault"] = json!({"code": 500, "message": "No valid host was found."});
        let volumes = vec![json!({"id": "vol-1", "name": "data", "size": 100,
            "attachments": [{"server_id": "3f2a9c1e-0000-0000-0000-000000000c91", "device": "/dev/vdb"}]})];
        let networks = vec![json!({"id": "net-1", "name": "ecommerce-net"})];
        let interfaces = vec![
            json!({"port_id": "port-1", "net_id": "net-1", "mac_addr": "fa:16:3e:4a:12:21",
            "port_state": "ACTIVE", "fixed_ips": [{"ip_address": "10.0.12.21", "subnet_id": "s"}]}),
        ];
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
            ports: &[
                json!({"id": "port-1", "name": "eth0", "admin_state_up": true, "binding:vnic_type": "normal",
                "security_groups": ["g-def"]}),
            ],
            actions: &actions,
            security_groups: &[json!({"id": "g-def", "name": "default", "security_group_rules": [
                    {"direction": "ingress", "ethertype": "IPv4", "remote_group_id": "g-def"},
                    {"direction": "egress", "ethertype": "IPv6", "remote_ip_prefix": "::/0"}]})],
            server_groups: &[
                json!({"id": "sg-other", "name": "db", "members": ["x"]}),
                json!({"id": "sg-web", "name": "web-spread", "members": ["3f2a9c1e-0000-0000-0000-000000000c91"]}),
            ],
        });
        assert_eq!(
            d.server_group,
            Some(("sg-web".into(), "web-spread".into())),
            "found by membership (no 2.71 field)"
        );
        assert_eq!(d.security_groups, vec!["default", "web"], "sorted and deduplicated");
        assert_eq!(
            d.security_group_rules,
            vec![(
                "default".into(),
                vec![
                    "ingress IPv4 any ← group default".into(),
                    "egress IPv6 any → ::/0".into()
                ]
            )],
            "rules by group, remote groups by name; `web` isn't listed by the project"
        );
        assert_eq!(d.flavor.vcpus, Some(4));
        assert_eq!(d.volumes[0].name.as_deref(), Some("data"));
        assert_eq!(d.volumes[0].device.as_deref(), Some("/dev/vdb"));
        assert_eq!(d.volumes[1].name, None, "unknown volume keeps its id only");
        assert_eq!(d.interfaces[0].network_name.as_deref(), Some("ecommerce-net"));
        assert_eq!(d.interfaces[0].fixed_ips, vec!["10.0.12.21"]);
        let port = d.interfaces[0].port.as_ref().expect("merged from the port list");
        assert_eq!(
            (port.name.as_str(), port.admin_up, port.vnic_type.as_deref()),
            ("eth0", true, Some("normal"))
        );
        assert!(
            port.port_security,
            "missing port_security_enabled means on (no extension)"
        );
        assert_eq!(port.security_group_ids, vec!["g-def"]);
        assert_eq!(d.actions[0].action, "stop", "newest action first");
        assert_eq!(d.fault.unwrap().code, 500);
        assert_eq!(d.metadata[0], ("env".into(), "prod".into()));
        assert_eq!(d.host.as_deref(), Some("compute-03"));
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
        assert_eq!(
            networks_without_subnets(&nets, &subnets),
            ["public"],
            "one missing subnet is enough; none listed: nothing to fetch"
        );
    }

    #[test]
    fn networks_and_routers_resolve_names() {
        let nets = vec![
            json!({"id": "n1", "name": "public", "router:external": true, "shared": false,
            "status": "ACTIVE", "admin_state_up": true, "subnets": ["sn1", "sn-unknown"]}),
        ];
        let subnets = vec![json!({"id": "sn1", "name": "pub-sub", "cidr": "203.0.113.0/24"})];
        let n = &networks(&nets, &subnets, "p1")[0];
        assert!(n.external);
        assert!(!n.owned, "no project_id: not ours");
        let own = json!({"id": "n2", "name": "mine", "project_id": "p1"});
        assert!(networks(std::slice::from_ref(&own), &[], "p1")[0].owned);
        assert!(!networks(&[own], &[], "p2")[0].owned);
        assert_eq!(n.subnets, vec!["pub-sub 203.0.113.0/24", "sn-unknown"]);
        let r = routers(
            &[json!({"id": "r1", "name": "gw", "external_gateway_info": {"network_id": "n1"}})],
            &nets,
        );
        assert_eq!(r[0].external_network.as_deref(), Some("public"));
        assert_eq!(
            (r[0].external_network_id.as_deref(), r[0].snat),
            (Some("n1"), None),
            "no enable_snat key"
        );
        let r = routers(
            &[
                json!({"id": "r1", "external_gateway_info": {"network_id": "gone", "enable_snat": false}}),
                json!({"id": "r2", "external_gateway_info": null}),
            ],
            &nets,
        );
        assert_eq!(
            r[0].external_network.as_deref(),
            Some("gone"),
            "unknown network: its ID"
        );
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
        let out = floating_ips(
            &raw,
            &[json!({"id": "n1", "name": "public"})],
            &[json!({"id": "s1", "name": "web"})],
        );
        assert_eq!(out[0].ip, "203.0.113.2");
        assert_eq!(out[0].instance_id, None);
        assert_eq!(out[1].instance_name.as_deref(), Some("web"));
        assert_eq!(out[1].pool, "public");
        assert_eq!(out[1].description, None);
    }

    #[test]
    fn floating_ip_description_and_dns_treat_empty_as_missing() {
        let raw = vec![
            json!({"id": "f1", "floating_ip_address": "203.0.113.9", "floating_network_id": "n1",
            "status": "DOWN", "description": "bastion", "dns_name": "", "dns_domain": "example.org."}),
        ];
        let out = floating_ips(&raw, &[], &[]);
        assert_eq!(out[0].description.as_deref(), Some("bastion"));
        assert_eq!(out[0].dns_name, None);
        assert_eq!(out[0].dns_domain.as_deref(), Some("example.org."));
    }

    #[test]
    fn ports_resolve_devices_subnets_and_groups() {
        let l = PortLookups::new(
            &[json!({"id": "n1", "name": "net"})],
            &[json!({"id": "sn1", "name": "sub"})],
            &[json!({"id": "s1", "name": "web"})],
            &[json!({"id": "g1", "name": "default"})],
        );
        let p = l.port(
            &json!({"id": "p1", "network_id": "n1", "mac_address": "fa:16", "status": "ACTIVE",
            "admin_state_up": true, "device_owner": "compute:alh1", "device_id": "s1", "security_groups": ["g1"],
            "fixed_ips": [{"ip_address": "10.0.0.5", "subnet_id": "sn1"}], "binding:vnic_type": "normal"}),
        );
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
        assert_eq!(
            (
                d.rules[0].protocol.as_str(),
                d.rules[0].port_range.as_str(),
                d.rules[0].remote.as_str()
            ),
            ("Any", "Any", "-")
        );
        let r1 = d.rules.iter().find(|r| r.id == "r1").unwrap();
        assert_eq!(r1.port_range, "80 - 443");
        let r3 = d.rules.iter().find(|r| r.id == "r3").unwrap();
        assert_eq!(r3.remote, "web");
    }

    #[test]
    fn topology_joins_routers_and_servers_to_networks_through_ports() {
        let nets = vec![
            json!({"id": "ext", "name": "public", "router:external": true, "status": "ACTIVE"}),
            json!({"id": "n-b", "name": "beta", "project_id": "p1", "status": "ACTIVE"}),
            json!({"id": "n-a", "name": "", "shared": true, "tenant_id": "p2", "status": "DOWN"}),
        ];
        let subnets = vec![json!({"id": "s1", "network_id": "n-b", "cidr": "10.0.1.0/24"})];
        let routers = vec![
            json!({"id": "r1", "name": "edge", "status": "ACTIVE",
            "external_gateway_info": {"network_id": "ext",
                "external_fixed_ips": [{"subnet_id": "x", "ip_address": "203.0.113.2"}]}}),
            json!({"id": "r2", "name": "lonely", "status": "ACTIVE", "external_gateway_info": null}),
        ];
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
        let fips = vec![
            json!({"floating_ip_address": "203.0.113.9", "port_id": "p-vm-b"}),
            json!({"floating_ip_address": "203.0.113.10", "port_id": null}),
        ];
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
        assert_eq!(
            names,
            vec!["public", "beta", "n-a"],
            "external first, unnamed falls back to id"
        );
        assert_eq!(t.networks[1].cidrs, vec!["10.0.1.0/24"]);
        let owned: Vec<_> = t.networks.iter().map(|n| n.owned).collect();
        assert_eq!(owned, [false, true, false], "owned from project_id, else tenant_id");
        let devs: Vec<_> = t.devices.iter().map(|d| (d.name.as_str(), d.kind)).collect();
        assert_eq!(
            devs,
            vec![
                ("edge", TopoKind::Router),
                ("lonely", TopoKind::Router),
                ("web", TopoKind::Server)
            ]
        );

        let links: Vec<_> = t
            .links
            .iter()
            .map(|l| (l.device_id.as_str(), l.network_id.as_str()))
            .collect();
        assert_eq!(
            links,
            vec![("r1", "ext"), ("r1", "n-b"), ("vm1", "n-a"), ("vm1", "n-b")]
        );
        let gw = &t.links[0];
        assert!(gw.gateway, "gateway from external_gateway_info");
        assert_eq!(
            (gw.port_id.as_deref(), gw.ips.clone()),
            (Some("p-gw"), vec!["203.0.113.2".to_string()]),
            "the gateway port is merged, not duplicated"
        );
        let vm_b = t.links.iter().find(|l| l.port_id.as_deref() == Some("p-vm-b")).unwrap();
        assert_eq!(
            (vm_b.ips.clone(), vm_b.floating.clone()),
            (vec!["10.0.1.5".into()], vec!["203.0.113.9".into()])
        );
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
    fn provider_attributes_are_read_when_admins_get_them() {
        let net = |extra: Value| {
            let mut n = json!({"id": "n1", "name": "ext", "status": "ACTIVE", "admin_state_up": true, "subnets": []});
            n.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            networks(&[n], &[], "p")[0].provider.clone()
        };
        let vlan = net(
            json!({"provider:network_type": "vlan", "provider:physical_network": "physnet1", "provider:segmentation_id": 120}),
        );
        assert_eq!(
            vlan,
            Some(ProviderNetwork {
                network_type: "vlan".into(),
                physical_network: Some("physnet1".into()),
                segmentation_id: Some(120)
            })
        );
        let flat = net(
            json!({"provider:network_type": "flat", "provider:physical_network": "public", "provider:segmentation_id": null}),
        );
        assert_eq!(flat.and_then(|p| p.segmentation_id), None, "flat: null segmentation ID");
        assert_eq!(net(json!({})), None, "members don't see provider attributes");
    }

    #[test]
    fn owned_choices_name_the_project() {
        let raw = vec![
            json!({"id": "n2", "name": "priv", "project_id": "p2"}),
            json!({"id": "n1", "name": "priv", "project_id": "p1"}),
            json!({"id": "n3", "name": ""}),
        ];
        let names = HashMap::from([("p1".to_string(), "alpha".to_string())]);
        let got: Vec<_> = owned_choices(&raw, &names)
            .into_iter()
            .map(|c| (c.value, c.label))
            .collect();
        assert_eq!(
            got,
            [
                ("n3".into(), "n3".into()),
                ("n1".into(), "priv (alpha)".into()),
                ("n2".into(), "priv (p2)".into())
            ],
            "same names told apart by project; unnamed → ID"
        );
    }

    #[test]
    fn metadef_resource_types_are_sorted_names() {
        let v = json!({"resource_types": [{"name": "OS::Nova::Flavor", "created_at": "x"}, {"name": "OS::Cinder::Volume"}, {}]});
        let got: Vec<String> = metadef_resource_type_choices(&v).into_iter().map(|c| c.value).collect();
        assert_eq!(got, ["OS::Cinder::Volume", "OS::Nova::Flavor"]);
    }

    #[test]
    fn admin_associate_offers_the_ports_the_pool_reaches() {
        let ports = vec![
            json!({"id": "a", "project_id": "p1", "device_owner": "compute:nova", "device_id": "s1", "fixed_ips": [{"subnet_id": "routed"}]}),
            json!({"id": "b", "project_id": "p1", "device_owner": "compute:nova", "device_id": "s1", "fixed_ips": [{"subnet_id": "isolated"}]}),
            json!({"id": "c", "project_id": "p1", "device_owner": "compute:nova", "device_id": "s1", "fixed_ips": [{"subnet_id": "both"}]}),
            json!({"id": "ri1", "device_owner": "network:router_interface", "device_id": "r1", "fixed_ips": [{"subnet_id": "routed"}, {"subnet_id": "both"}]}),
            json!({"id": "ri2", "device_owner": "network:ha_router_replicated_interface", "device_id": "r2", "fixed_ips": [{"subnet_id": "both"}]}),
            json!({"id": "ri3", "device_owner": "network:router_interface", "device_id": "r3", "fixed_ips": [{"subnet_id": "isolated"}]}),
        ];
        let routers = vec![
            json!({"id": "r1", "external_gateway_info": {"network_id": "public"}}),
            json!({"id": "r2", "external_gateway_info": {"network_id": "lab"}}),
            json!({"id": "r3", "external_gateway_info": null}),
        ];
        let reach = reachable_subnets_by_pool(&ports, &routers);
        assert_eq!(reach.len(), 2, "a router without a gateway reaches nothing: {reach:?}");
        assert!(
            reach["lab"].contains("both"),
            "an HA router's interface counts, as in Horizon"
        );
        let mut got: Vec<(String, String)> =
            owned_port_choices(&ports, &[], &HashMap::new(), PoolReach::Subnets(&reach))
                .into_iter()
                .map(|c| (c.cells[0].clone(), c.value))
                .collect();
        got.sort();
        let want =
            [("p1 lab", "c"), ("p1 public", "a"), ("p1 public", "c")].map(|(k, v)| (k.to_string(), v.to_string()));
        assert_eq!(
            got, want,
            "an isolated port is offered for no pool; one behind two routers for both"
        );
    }

    #[test]
    fn cinder_pools_are_sorted_names() {
        let raw = vec![
            json!({"name": "c2@ceph#slow"}),
            json!({"name": "c1@lvm#lvm"}),
            json!({"name": ""}),
            json!({}),
        ];
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
        let hosts = |usable| {
            compute_host_choices(&raw, usable)
                .into_iter()
                .map(|c| c.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            hosts(true),
            ["c1", "c2"],
            "disabled or down hosts can't take an instance"
        );
        assert_eq!(hosts(false), ["c1", "c2", "c3", "c4"], "but they can join an aggregate");
        assert_eq!(
            host_services(&raw)[0].id,
            "u2",
            "the UUID (2.53) is kept for Enable/Disable Service"
        );
    }

    #[test]
    fn dhcp_agents_are_counted_per_network() {
        let per_agent = vec![
            json!({"networks": [{"id": "n1"}, {"id": "n2"}]}),
            json!({"networks": [{"id": "n1"}]}),
            json!({}),
        ];
        let c = dhcp_agent_counts(&per_agent);
        assert_eq!(
            (c.get("n1"), c.get("n2"), c.get("n3")),
            (Some(&2), Some(&1), None),
            "an agent without networks counts nothing"
        );
    }

    #[test]
    fn slice_c_pages_keep_the_apis_order_and_name_from_the_pages_lookups() {
        let ids = |v: Vec<String>| v;
        let nets = vec![
            json!({"id": "n2", "name": "zeta", "subnets": ["s1"]}),
            json!({"id": "n1", "name": "alpha", "subnets": ["gone"]}),
        ];
        let page = networks_page(&nets, &[json!({"id": "s1", "name": "a", "cidr": "10.0.0.0/24"})], "");
        assert_eq!(
            page.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            ["n2", "n1"],
            "Neutron's order: the marker is the last row"
        );
        assert_eq!(
            (page[0].subnets[0].as_str(), page[1].subnets[0].as_str()),
            ("a 10.0.0.0/24", "gone"),
            "a subnet not read stays an ID"
        );
        assert_eq!(row_ids(&nets), ["n1", "n2"]);

        let routers = vec![
            json!({"id": "r2", "name": "z", "external_gateway_info": {"network_id": "ext"}}),
            json!({"id": "r1", "name": "a", "external_gateway_info": null}),
            json!({"id": "r3", "name": "b", "external_gateway_info": {"network_id": "ext"}}),
        ];
        assert_eq!(
            gateway_ids(&routers),
            ["ext"],
            "one network lookup, null gateways skipped"
        );
        let page = routers_page(&routers, &[json!({"id": "ext", "name": "public"})]);
        assert_eq!(
            (
                page[0].id.as_str(),
                page[0].external_network.as_deref(),
                page[1].external_network.as_deref()
            ),
            ("r2", Some("public"), None)
        );

        let trunks = vec![
            json!({"id": "t2", "name": "z", "port_id": "p1", "sub_ports": [{"port_id": "p2", "segmentation_type": "vlan", "segmentation_id": 10}, {"port_id": "p1"}]}),
            json!({"id": "t1", "name": "a", "port_id": "p3"}),
        ];
        assert_eq!(
            ids(trunk_port_ids(&trunks)),
            ["p1", "p2", "p3"],
            "parents and sub-ports, each once"
        );
        let page = trunks_page(&trunks, &[json!({"id": "p1", "name": "parent"})]);
        assert_eq!(
            (
                page[0].id.as_str(),
                page[0].parent.name.as_str(),
                page[1].parent.name.as_str()
            ),
            ("t2", "parent", "p3"),
            "a port not read stays an ID"
        );

        let images = vec![
            json!({"id": "i2", "name": "zeta", "owner": "p"}),
            json!({"id": "i1", "name": "alpha"}),
        ];
        let names = HashMap::from([("p".to_string(), "proj".to_string())]);
        let page = images_page(&images, "", &names);
        assert_eq!(
            (page[0].id.as_str(), page[0].owner_name.as_deref()),
            ("i2", Some("proj")),
            "Glance's order, not owned-first by name"
        );

        let groups = vec![json!({"id": "g2", "name": "z"}), json!({"id": "g1", "name": "a"})];
        let page = volume_groups_page(&groups, &[], &[], &[json!({"id": "x", "group_id": "g1"})]);
        assert_eq!(
            (page[0].id.as_str(), page[0].has_snapshots, page[1].has_snapshots),
            ("g2", false, true)
        );
        let gs = vec![
            json!({"id": "a", "created_at": "2020", "group_id": "g1"}),
            json!({"id": "b", "created_at": "2026", "group_id": "g1"}),
            json!({"id": "c", "group_id": "g2"}),
        ];
        assert_eq!(group_ids(&gs), ["g1", "g2"]);
        let page = group_snapshots_page(&gs, &[json!({"id": "g1", "name": "db"})], &[]);
        assert_eq!(
            page.iter()
                .map(|x| (x.id.as_str(), x.group.name.as_str()))
                .collect::<Vec<_>>(),
            [("a", "db"), ("b", "db"), ("c", "g2")],
            "Cinder's order, not newest first"
        );
    }

    #[test]
    fn late_names_come_from_wrapped_or_bare_bodies() {
        let srv = late_name(&json!({"server": {"id": "s1", "name": "web"}})).unwrap();
        assert_eq!((srv.name.as_str(), srv.host), ("web", None));
        let vol =
            late_name(&json!({"volume": {"id": "v1", "name": "", "os-vol-host-attr:host": "c1@ceph#rbd"}})).unwrap();
        assert_eq!(
            (vol.name.as_str(), vol.host.as_deref()),
            ("v1", Some("c1@ceph#rbd")),
            "unnamed: the ID"
        );
        assert!(late_name(&json!({})).is_none());
    }
}
