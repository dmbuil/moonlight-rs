//! Neutron writes: networks, subnets, ports, RBAC, trunks, port forwarding, security groups, floating IPs.

use super::*;

/// `{"port_forwarding": {...}}`: single ports as integers, ranges as `"a:b"`
/// under the `*_range` keys (port-ranges extension).
fn port_forwarding_body(s: &PortForwardingSpec) -> R<Value> {
    let (internal, external) = s.ports().map_err(AppError::invalid)?;
    let mut pf = serde_json::Map::new();
    pf.insert("protocol".into(), json!(s.protocol));
    pf.insert(
        "internal_port_id".into(),
        json!(non_empty(&s.internal_port_id, "internal port")?),
    );
    pf.insert("internal_ip_address".into(), json!(s.internal_ip.trim()));
    for (key, p) in [("internal_port", internal), ("external_port", external)] {
        match p {
            PortSpec::One(n) => pf.insert(key.into(), json!(n)),
            PortSpec::Range(..) => pf.insert(format!("{key}_range"), json!(p.to_string())),
        };
    }
    if let Some(d) = opt(&s.description) {
        pf.insert("description".into(), json!(d));
    }
    Ok(json!({ "port_forwarding": pf }))
}

/// `OsAction::SubnetCreate` (and the subnet of a new network): `POST /v2.0/subnets`.
///
/// `gateway_ip`: `None` lets Neutron pick the first address, `Some("")` sends `null` (no
/// gateway). The IPv6 modes are sent only for IPv6 subnets, and host routes are validated with
/// `model::validate_route` first.
pub(super) async fn create_subnet(x: &mut Exec<'_>, network_id: &str, s: &SubnetSpec) -> R<()> {
    use openstack_sdk::api::network::v2::subnet::create as m;
    if s.ip_version != 4 && s.ip_version != 6 {
        return Err(AppError::invalid("ip version must be 4 or 6"));
    }
    let mut b = m::SubnetBuilder::default();
    b.network_id(network_id)
        .name(s.name.trim())
        .ip_version(s.ip_version as i32)
        .cidr(Some(Cow::from(non_empty(&s.cidr, "network address")?.to_string())))
        .enable_dhcp(s.enable_dhcp);
    match s.gateway_ip.as_deref().map(str::trim) {
        None => {} // Neutron picks the first address
        Some("") => {
            b.gateway_ip(None::<Cow<str>>); // explicitly no gateway
        }
        Some(gw) => {
            b.gateway_ip(Some(Cow::from(gw.to_string())));
        }
    }
    if !s.dns_nameservers.is_empty() {
        b.dns_nameservers(
            s.dns_nameservers
                .iter()
                .map(|d| Cow::from(d.trim().to_string()))
                .collect::<Vec<_>>(),
        );
    }
    if !s.allocation_pools.is_empty() {
        let pools = s
            .allocation_pools
            .iter()
            .map(|(a, z)| {
                m::AllocationPoolsBuilder::default()
                    .start(a.trim().to_string())
                    .end(z.trim().to_string())
                    .build()
                    .map_err(build_err)
            })
            .collect::<R<Vec<_>>>()?;
        b.allocation_pools(pools);
    }
    // Horizon's *IPv6 Address Configuration Mode*: IPv6 subnets only (Neutron
    // refuses the modes on IPv4).
    if s.ip_version == 6 {
        if let Some(ra) = &s.ipv6_ra_mode {
            b.ipv6_ra_mode(enum_of::<m::Ipv6RaMode>(ra, "IPv6 RA mode")?);
        }
        if let Some(addr) = &s.ipv6_address_mode {
            b.ipv6_address_mode(enum_of::<m::Ipv6AddressMode>(addr, "IPv6 address mode")?);
        }
    }
    if !s.host_routes.is_empty() {
        let routes = s
            .host_routes
            .iter()
            .map(|(dest, hop)| {
                let (dest, hop) = crate::model::validate_route(dest, hop).map_err(AppError::invalid)?;
                m::HostRoutesBuilder::default()
                    .destination(dest)
                    .nexthop(hop)
                    .build()
                    .map_err(build_err)
            })
            .collect::<R<Vec<_>>>()?;
        b.host_routes(routes);
    }
    let body = b.build().map_err(build_err)?;
    x.create(m::Request::builder().subnet(body).build().map_err(build_err)?, "subnet")
        .await?;
    Ok(())
}

/// `OsAction::RuleCreate`: `POST /v2.0/security-group-rules`.
///
/// A lone `port_min` is a single port (sent as both bounds); a remote CIDR and a remote group
/// are mutually exclusive, so both is an error rather than Neutron's.
pub(super) async fn create_rule(x: &mut Exec<'_>, s: &RuleSpec) -> R<()> {
    use openstack_sdk::api::network::v2::security_group_rule::create as m;
    let mut b = m::SecurityGroupRuleBuilder::default();
    b.security_group_id(s.security_group_id.as_str());
    b.direction(match s.direction.as_str() {
        "ingress" => m::Direction::Ingress,
        "egress" => m::Direction::Egress,
        _ => return Err(AppError::invalid("invalid direction")),
    });
    b.ethertype(match s.ethertype.as_str() {
        "IPv4" => m::Ethertype::Ipv4,
        "IPv6" => m::Ethertype::Ipv6,
        _ => return Err(AppError::invalid("invalid ethertype")),
    });
    if let Some(p) = opt(&s.protocol) {
        b.protocol(p.to_string());
    }
    if let (Some(a), Some(z)) = (s.port_min, s.port_max)
        && (a > z || a < 0 || z > 65535)
    {
        return Err(AppError::invalid("invalid port range"));
    }
    if let Some(a) = s.port_min {
        b.port_range_min(Some(a));
    }
    if let Some(z) = s.port_max.or(s.port_min) {
        b.port_range_max(Some(z));
    }
    match (opt(&s.remote_ip_prefix), opt(&s.remote_group_id)) {
        (Some(_), Some(_)) => return Err(AppError::invalid("use either a CIDR or a remote group, not both")),
        (Some(cidr), None) => {
            b.remote_ip_prefix(cidr.to_string());
        }
        (None, Some(g)) => {
            b.remote_group_id(g.to_string());
        }
        (None, None) => {}
    }
    if let Some(d) = opt(&s.description) {
        b.description(d.to_string());
    }
    let body = b.build().map_err(build_err)?;
    x.create(
        m::Request::builder()
            .security_group_rule(body)
            .build()
            .map_err(build_err)?,
        "rule",
    )
    .await?;
    Ok(())
}

/// `OsAction::NetworkCreate`: `POST /v2.0/networks`, then the optional subnet on the new
/// network's ID (a placeholder in dry-run), as Horizon's *Create Network* wizard does.
pub(super) async fn network_create(
    x: &mut Exec<'_>,
    name: &str,
    admin_up: &bool,
    subnet: &Option<SubnetSpec>,
) -> R<()> {
    use openstack_sdk::api::network::v2::network::create as m;
    let body = m::NetworkBuilder::default()
        .name(name.trim())
        .admin_state_up(*admin_up)
        .build()
        .map_err(build_err)?;
    let net_id = x
        .create(
            m::Request::builder().network(body).build().map_err(build_err)?,
            "network",
        )
        .await?;
    if let Some(spec) = subnet {
        create_subnet(x, &net_id, spec).await?;
    }
    Ok(())
}

/// `OsAction::AdminNetworkCreate` (Admin › Networks): `POST /v2.0/networks` with an owner
/// (`tenant_id`), `shared`, `router:external`, MTU, AZ hints and the `provider:*` attributes,
/// then the optional subnet.
///
/// The provider type is checked against this server's `[network]` settings, not the dialog's
/// choices.
pub(super) async fn admin_network_create(x: &mut Exec<'_>, spec: &AdminNetworkSpec) -> R<()> {
    use openstack_sdk::api::network::v2::network::create as m;
    let mut b = m::NetworkBuilder::default();
    b.name(spec.name.trim())
        .admin_state_up(spec.admin_up)
        .shared(spec.shared)
        .router_external(spec.external)
        // `tenant_id` is the SDK's name for the owner (Neutron takes both).
        .tenant_id(spec.project_id.as_str());
    if let Some(mtu) = spec.mtu {
        b.mtu(mtu);
    }
    if !spec.az_hints.is_empty() {
        b.availability_zone_hints(spec.az_hints.iter().map(|z| Cow::from(z.as_str())).collect::<Vec<_>>());
    }
    if let Some(p) = &spec.provider {
        // The server's own `[network]` settings, whatever the dialog sent
        // (the defaults in unit tests, which run without `config::init`).
        let net = crate::server::config::try_state()
            .map(|st| st.config.prefs.network.clone())
            .unwrap_or_default();
        p.validate(&net.provider_types()).map_err(AppError::invalid)?;
        b.provider_network_type(p.network_type.as_str());
        if let Some(phys) = &p.physical_network {
            b.provider_physical_network(phys.trim());
        }
        if let Some(seg) = p.segmentation_id {
            b.provider_segmentation_id(seg.to_string());
        }
    }
    let net_id = x
        .create(
            m::Request::builder()
                .network(b.build().map_err(build_err)?)
                .build()
                .map_err(build_err)?,
            "network",
        )
        .await?;
    if let Some(s) = &spec.subnet {
        create_subnet(x, &net_id, s).await?;
    }
    Ok(())
}

/// `OsAction::AdminNetworkUpdate`: `PUT /v2.0/networks/{id}` with name, admin state, `shared`
/// and `router:external`.
pub(super) async fn admin_network_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    admin_up: &bool,
    shared: &bool,
    external: &bool,
) -> R<()> {
    use openstack_sdk::api::network::v2::network::set as m;
    let body = m::NetworkBuilder::default()
        .name(name.trim())
        .admin_state_up(*admin_up)
        .shared(*shared)
        .router_external(*external)
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).network(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `OsAction::RbacPolicyCreate`: `POST /v2.0/rbac-policies`; `kind` maps to the object type
/// and action with `model::rbac_kind` (e.g. a network shared, or usable as external).
pub(super) async fn rbac_policy_create(x: &mut Exec<'_>, kind: &str, object_id: &str, target_project: &str) -> R<()> {
    use openstack_sdk::api::network::v2::rbac_policy::create as m;
    let (object_type, action) = crate::model::rbac_kind(kind).map_err(AppError::invalid)?;
    let body = m::RbacPolicyBuilder::default()
        .object_type(object_type)
        .action(action)
        .object_id(non_empty(object_id, "object")?)
        .target_tenant(non_empty(target_project, "target project")?)
        .build()
        .map_err(build_err)?;
    x.create(
        m::Request::builder().rbac_policy(body).build().map_err(build_err)?,
        "RBAC policy",
    )
    .await?;
    Ok(())
}

/// `OsAction::RbacPolicyUpdate`: `PUT /v2.0/rbac-policies/{id}`; only the target project can
/// change.
pub(super) async fn rbac_policy_update(x: &mut Exec<'_>, id: &str, target_project: &str) -> R<()> {
    use openstack_sdk::api::network::v2::rbac_policy::set as m;
    let body = m::RbacPolicyBuilder::default()
        .target_tenant(non_empty(target_project, "target project")?)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .rbac_policy(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `OsAction::RbacPolicyDelete`: `DELETE /v2.0/rbac-policies/{id}`.
pub(super) async fn rbac_policy_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::rbac_policy::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `OsAction::NetworkUpdate`: `PUT /v2.0/networks/{id}` with name and admin state.
pub(super) async fn network_update(x: &mut Exec<'_>, id: &str, name: &str, admin_up: &bool) -> R<()> {
    use openstack_sdk::api::network::v2::network::set as m;
    let body = m::NetworkBuilder::default()
        .name(name.trim())
        .admin_state_up(*admin_up)
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).network(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `OsAction::NetworkDelete`: `DELETE /v2.0/networks/{id}`.
pub(super) async fn network_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::network::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `OsAction::SubnetUpdate`: `PUT /v2.0/subnets/{id}`. The DNS servers are always sent, so an
/// empty list clears them.
pub(super) async fn subnet_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    enable_dhcp: &bool,
    dns_nameservers: &[String],
) -> R<()> {
    use openstack_sdk::api::network::v2::subnet::set as m;
    let dns: Vec<Cow<str>> = dns_nameservers
        .iter()
        .map(|d| Cow::from(d.trim().to_string()))
        .collect();
    let body = m::SubnetBuilder::default()
        .name(name.trim())
        .enable_dhcp(*enable_dhcp)
        .dns_nameservers(dns)
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).subnet(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `OsAction::SubnetDelete`: `DELETE /v2.0/subnets/{id}`.
pub(super) async fn subnet_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::subnet::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `OsAction::PortCreate`: `POST /v2.0/ports`, with one fixed IP when given (Neutron picks
/// the subnet).
pub(super) async fn port_create(
    x: &mut Exec<'_>,
    network_id: &str,
    name: &str,
    admin_up: &bool,
    fixed_ip: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::network::v2::port::create as m;
    let mut b = m::PortBuilder::default();
    b.network_id(network_id).name(name.trim()).admin_state_up(*admin_up);
    if let Some(ip) = opt(fixed_ip) {
        let f = m::FixedIpsBuilder::default()
            .ip_address(ip.to_string())
            .build()
            .map_err(build_err)?;
        b.fixed_ips(vec![f]);
    }
    let body = b.build().map_err(build_err)?;
    x.create(m::Request::builder().port(body).build().map_err(build_err)?, "port")
        .await?;
    Ok(())
}

/// `OsAction::PortUpdate`: `PUT /v2.0/ports/{id}` with only the fields that are `Some`.
///
/// The VNIC type goes through the SDK enum's serde names, so an unknown one fails here, not
/// at Neutron.
pub(super) async fn port_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &Option<String>,
    admin_up: &Option<bool>,
    vnic_type: &Option<String>,
    port_security: &Option<bool>,
    security_group_ids: &Option<Vec<String>>,
) -> R<()> {
    use openstack_sdk::api::network::v2::port::set as m;
    let mut b = m::PortBuilder::default();
    if let Some(n) = name {
        b.name(n.trim());
    }
    if let Some(up) = admin_up {
        b.admin_state_up(*up);
    }
    if let Some(v) = vnic_type {
        // The SDK's enum has the API's spellings as serde names.
        let v: m::BindingVnicType = serde_json::from_value(Value::String(v.clone()))
            .map_err(|_| AppError::invalid(format!("unknown VNIC type: {v}")))?;
        b.binding_vnic_type(v);
    }
    if let Some(on) = port_security {
        b.port_security_enabled(*on);
    }
    if let Some(groups) = security_group_ids {
        b.security_groups(groups.iter().map(|g| Cow::from(g.as_str())).collect::<Vec<_>>());
    }
    let body = b.build().map_err(build_err)?;
    x.send(m::Request::builder().id(id).port(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `OsAction::PortSetSecurityGroups`: `PUT /v2.0/ports/{id}` replacing the port's security
/// groups with exactly `security_group_ids` (empty removes them all).
pub(super) async fn port_set_security_groups(x: &mut Exec<'_>, id: &str, security_group_ids: &[String]) -> R<()> {
    use openstack_sdk::api::network::v2::port::set as m;
    let groups: Vec<Cow<str>> = security_group_ids.iter().map(|g| Cow::from(g.as_str())).collect();
    let body = m::PortBuilder::default()
        .security_groups(groups)
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).port(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `OsAction::PortDelete`: `DELETE /v2.0/ports/{id}`.
pub(super) async fn port_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::port::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `OsAction::TrunkCreate`: `POST /v2.0/trunks` on the parent port, with any sub-ports.
///
/// The SDK has no trunk endpoints (only their tags), so this and the other trunk writes are
/// raw JSON requests.
pub(super) async fn trunk_create(
    x: &mut Exec<'_>,
    name: &str,
    description: &Option<String>,
    port_id: &str,
    admin_up: &bool,
    subports: &[SubportSpec],
) -> R<()> {
    let sub_ports = subports
        .iter()
        .map(SubportSpec::body)
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::invalid)?;
    let mut t = serde_json::Map::new();
    t.insert("name".into(), json!(name.trim()));
    if let Some(d) = opt(description) {
        t.insert("description".into(), json!(d));
    }
    t.insert("port_id".into(), json!(non_empty(port_id, "parent port")?));
    t.insert("admin_state_up".into(), json!(admin_up));
    if !sub_ports.is_empty() {
        t.insert("sub_ports".into(), json!(sub_ports));
    }
    x.create(
        JsonRequest::new(
            http::Method::POST,
            openstack_sdk::types::ServiceType::Network,
            "trunks",
            Some(json!({ "trunk": t })),
        ),
        "trunk",
    )
    .await?;
    Ok(())
}

/// `OsAction::TrunkUpdate`: `PUT /v2.0/trunks/{id}` (raw JSON: the SDK has no trunk
/// endpoints).
pub(super) async fn trunk_update(x: &mut Exec<'_>, id: &str, name: &str, description: &str, admin_up: &bool) -> R<()> {
    let body = json!({"trunk": {"name": name.trim(), "description": description.trim(), "admin_state_up": admin_up}});
    x.send(JsonRequest::new(
        http::Method::PUT,
        openstack_sdk::types::ServiceType::Network,
        format!("trunks/{}", non_empty(id, "trunk")?),
        Some(body),
    ))
    .await?;
    Ok(())
}

/// `OsAction::TrunkDelete`: `DELETE /v2.0/trunks/{id}` (raw JSON request).
pub(super) async fn trunk_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    x.send(JsonRequest::new(
        http::Method::DELETE,
        openstack_sdk::types::ServiceType::Network,
        format!("trunks/{}", non_empty(id, "trunk")?),
        None,
    ))
    .await?;
    Ok(())
}

/// `OsAction::TrunkAddSubports`: `PUT /v2.0/trunks/{id}/add_subports` (raw JSON). An empty
/// list is refused here rather than sent.
pub(super) async fn trunk_add_subports(x: &mut Exec<'_>, id: &str, subports: &[SubportSpec]) -> R<()> {
    if subports.is_empty() {
        return Err(AppError::invalid("no sub-ports to add"));
    }
    let sub_ports = subports
        .iter()
        .map(SubportSpec::body)
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::invalid)?;
    let path = format!("trunks/{}/add_subports", non_empty(id, "trunk")?);
    x.send(JsonRequest::new(
        http::Method::PUT,
        openstack_sdk::types::ServiceType::Network,
        path,
        Some(json!({ "sub_ports": sub_ports })),
    ))
    .await?;
    Ok(())
}

/// `OsAction::TrunkRemoveSubports`: `PUT /v2.0/trunks/{id}/remove_subports` with
/// `[{"port_id": …}]` (raw JSON). An empty list is refused.
pub(super) async fn trunk_remove_subports(x: &mut Exec<'_>, id: &str, port_ids: &[String]) -> R<()> {
    let sub_ports: Vec<Value> = port_ids
        .iter()
        .map(|p| non_empty(p, "sub-port").map(|p| json!({ "port_id": p })))
        .collect::<R<_>>()?;
    if sub_ports.is_empty() {
        return Err(AppError::invalid("no sub-ports to remove"));
    }
    let path = format!("trunks/{}/remove_subports", non_empty(id, "trunk")?);
    x.send(JsonRequest::new(
        http::Method::PUT,
        openstack_sdk::types::ServiceType::Network,
        path,
        Some(json!({ "sub_ports": sub_ports })),
    ))
    .await?;
    Ok(())
}

/// `OsAction::PortForwardingCreate`: `POST /v2.0/floatingips/{fip}/port_forwardings`, as raw
/// JSON: the SDK types the `*_range` keys as numbers, Neutron wants `"a:b"` (see
/// `port_forwarding_body`).
pub(super) async fn port_forwarding_create(x: &mut Exec<'_>, spec: &PortForwardingSpec) -> R<()> {
    let path = format!(
        "floatingips/{}/port_forwardings",
        non_empty(&spec.floating_ip_id, "floating IP")?
    );
    let body = port_forwarding_body(spec)?;
    x.create(
        JsonRequest::new(
            http::Method::POST,
            openstack_sdk::types::ServiceType::Network,
            path,
            Some(body),
        ),
        "port forwarding",
    )
    .await?;
    Ok(())
}

/// `OsAction::PortForwardingUpdate`: `PUT /v2.0/floatingips/{fip}/port_forwardings/{id}`
/// (raw JSON, as for create).
pub(super) async fn port_forwarding_update(x: &mut Exec<'_>, id: &str, spec: &PortForwardingSpec) -> R<()> {
    let path = format!(
        "floatingips/{}/port_forwardings/{}",
        non_empty(&spec.floating_ip_id, "floating IP")?,
        non_empty(id, "rule")?
    );
    let body = port_forwarding_body(spec)?;
    x.send(JsonRequest::new(
        http::Method::PUT,
        openstack_sdk::types::ServiceType::Network,
        path,
        Some(body),
    ))
    .await?;
    Ok(())
}

/// `OsAction::PortForwardingDelete`: `DELETE
/// /v2.0/floatingips/{floating_ip_id}/port_forwardings/{id}`.
pub(super) async fn port_forwarding_delete(x: &mut Exec<'_>, floating_ip_id: &str, id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::floatingip::port_forwarding::delete as m;
    x.send(
        m::Request::builder()
            .floatingip_id(floating_ip_id)
            .id(id)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `OsAction::SecurityGroupCreate`: `POST /v2.0/security-groups`.
pub(super) async fn security_group_create(x: &mut Exec<'_>, name: &str, description: &Option<String>) -> R<()> {
    use openstack_sdk::api::network::v2::security_group::create as m;
    let mut b = m::SecurityGroupBuilder::default();
    b.name(non_empty(name, "name")?);
    if let Some(d) = opt(description) {
        b.description(d.to_string());
    }
    let body = b.build().map_err(build_err)?;
    x.create(
        m::Request::builder().security_group(body).build().map_err(build_err)?,
        "security group",
    )
    .await?;
    Ok(())
}

/// `OsAction::SecurityGroupUpdate`: `PUT /v2.0/security-groups/{id}`. The description is
/// always sent, so an empty one clears it.
pub(super) async fn security_group_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    description: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::network::v2::security_group::set as m;
    let body = m::SecurityGroupBuilder::default()
        .name(non_empty(name, "name")?)
        .description(opt(description).unwrap_or("").to_string())
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .security_group(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `OsAction::SecurityGroupDelete`: `DELETE /v2.0/security-groups/{id}`.
pub(super) async fn security_group_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::security_group::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `OsAction::RuleDelete`: `DELETE /v2.0/security-group-rules/{id}`.
pub(super) async fn rule_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::security_group_rule::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `OsAction::FloatingIpAllocate`: `POST /v2.0/floatingips` from the pool `network_id`,
/// optionally associated with a port and given a QoS policy at once.
pub(super) async fn floating_ip_allocate(
    x: &mut Exec<'_>,
    network_id: &str,
    description: &Option<String>,
    port_id: &Option<String>,
    qos_policy_id: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::network::v2::floatingip::create as m;
    let mut b = m::FloatingipBuilder::default();
    b.floating_network_id(network_id);
    if let Some(q) = opt(qos_policy_id) {
        b.qos_policy_id(Some(Cow::from(q.to_string())));
    }
    if let Some(d) = opt(description) {
        b.description(d.to_string());
    }
    if let Some(p) = opt(port_id) {
        b.port_id(Some(Cow::from(p.to_string())));
    }
    let body = b.build().map_err(build_err)?;
    x.create(
        m::Request::builder().floatingip(body).build().map_err(build_err)?,
        "floating ip",
    )
    .await?;
    Ok(())
}

/// `OsAction::AdminFloatingIpAllocate` (Admin › Floating IPs): `POST /v2.0/floatingips` for
/// another project, optionally with a specific address (checked to be an IP here so the error
/// names the field).
pub(super) async fn admin_floating_ip_allocate(
    x: &mut Exec<'_>,
    network_id: &str,
    project_id: &str,
    address: &Option<String>,
    description: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::network::v2::floatingip::create as m;
    let mut b = m::FloatingipBuilder::default();
    b.floating_network_id(non_empty(network_id, "pool")?)
        .tenant_id(non_empty(project_id, "project")?);
    if let Some(a) = opt(address) {
        // Neutron would say "invalid input"; name the field instead.
        a.parse::<std::net::IpAddr>()
            .map_err(|_| AppError::invalid(format!("not an IP address: {a}")))?;
        b.floating_ip_address(Some(Cow::from(a.to_string())));
    }
    if let Some(d) = opt(description) {
        b.description(d.to_string());
    }
    let body = b.build().map_err(build_err)?;
    x.create(
        m::Request::builder().floatingip(body).build().map_err(build_err)?,
        "floating ip",
    )
    .await?;
    Ok(())
}

/// `OsAction::FloatingIpAssociate`: `PUT /v2.0/floatingips/{id}` with `port_id`.
pub(super) async fn floating_ip_associate(x: &mut Exec<'_>, id: &str, port_id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::floatingip::set as m;
    let body = m::FloatingipBuilder::default()
        .port_id(Some(Cow::from(port_id.to_string())))
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .floatingip(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `OsAction::FloatingIpDisassociate`: `PUT /v2.0/floatingips/{id}` with `"port_id": null`.
pub(super) async fn floating_ip_disassociate(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::floatingip::set as m;
    let body = m::FloatingipBuilder::default()
        .port_id(None::<Cow<str>>)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .floatingip(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `OsAction::FloatingIpRelease`: `DELETE /v2.0/floatingips/{id}`.
pub(super) async fn floating_ip_release(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::floatingip::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[tokio::test]
    async fn network_with_subnet_chains_the_new_id() {
        let spec = SubnetSpec {
            name: "sub".into(),
            cidr: "10.0.0.0/24".into(),
            ip_version: 4,
            gateway_ip: Some("".into()),
            enable_dhcp: true,
            dns_nameservers: vec!["8.8.8.8".into()],
            allocation_pools: vec![("10.0.0.10".into(), "10.0.0.99".into())],
            ipv6_ra_mode: None,
            ipv6_address_mode: None,
            host_routes: vec![],
        };
        let r = dry(OsAction::NetworkCreate {
            name: id("net"),
            admin_up: true,
            subnet: Some(spec),
        })
        .await;
        assert_eq!(r.len(), 2);
        assert_eq!(body(&r[0]), json!({"network": {"name": "net", "admin_state_up": true}}));
        let s = &body(&r[1])["subnet"];
        assert_eq!(s["network_id"], "<new network id>");
        assert_eq!(s["gateway_ip"], Value::Null, "empty gateway disables it");
        assert_eq!(
            s["allocation_pools"],
            json!([{"start": "10.0.0.10", "end": "10.0.0.99"}])
        );
        assert_eq!(r[1].path, "/subnets");
    }

    #[tokio::test]
    async fn admin_network_sends_owner_provider_and_flags() {
        let spec = |provider| AdminNetworkSpec {
            name: " ext ".into(),
            project_id: "p1".into(),
            provider,
            admin_up: true,
            shared: true,
            external: true,
            mtu: Some(1450),
            az_hints: vec!["az1".into()],
            subnet: None,
        };
        let vlan = ProviderNetwork {
            network_type: "vlan".into(),
            physical_network: Some("physnet1".into()),
            segmentation_id: Some(120),
        };
        let r = dry(OsAction::AdminNetworkCreate(Box::new(spec(Some(vlan))))).await;
        assert_eq!(
            (r.len(), r[0].method.as_str(), r[0].path.as_str()),
            (1, "POST", "/networks")
        );
        assert_eq!(
            body(&r[0]),
            json!({"network": {
                "name": "ext", "admin_state_up": true, "shared": true, "router:external": true, "tenant_id": "p1",
                "mtu": 1450, "availability_zone_hints": ["az1"],
                "provider:network_type": "vlan", "provider:physical_network": "physnet1", "provider:segmentation_id": "120"
            }})
        );
        let r = dry(OsAction::AdminNetworkCreate(Box::new(spec(None)))).await;
        assert!(
            body(&r[0])["network"].get("provider:network_type").is_none(),
            "no provider: Neutron's default type"
        );
    }

    #[tokio::test]
    async fn admin_allocates_a_floating_ip_to_another_project() {
        let alloc = |address: Option<&str>| OsAction::AdminFloatingIpAllocate {
            network_id: id("ext"),
            project_id: id("p2"),
            address: address.map(Into::into),
            description: Some(id("vpn")),
        };
        let r = dry(alloc(Some("203.0.113.10"))).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/floatingips"));
        assert_eq!(
            body(&r[0]),
            json!({"floatingip": {"floating_network_id": "ext", "tenant_id": "p2", "floating_ip_address": "203.0.113.10", "description": "vpn"}})
        );
        assert!(
            body(&dry(alloc(None)).await[0])["floatingip"]
                .get("floating_ip_address")
                .is_none(),
            "no address: Neutron picks one"
        );
        assert!(dry_err(alloc(Some("203.0.113"))).await, "not an IP address");
    }

    #[tokio::test]
    async fn rbac_policies_are_created_and_deleted_like_horizon() {
        let create = |kind: &str, target: &str| OsAction::RbacPolicyCreate {
            kind: kind.into(),
            object_id: id("n1"),
            target_project: target.into(),
        };
        let r = dry(create("external_network", "*")).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/rbac-policies"));
        assert_eq!(
            body(&r[0]),
            json!({"rbac_policy": {"object_type": "network", "action": "access_as_external", "object_id": "n1", "target_tenant": "*"}})
        );
        let r = dry(create("shared_qos_policy", "p2")).await;
        assert_eq!(body(&r[0])["rbac_policy"]["object_type"], "qos_policy");
        assert!(
            dry_err(create("external_qos_policy", "p2")).await,
            "only Horizon's three kinds"
        );
        assert!(
            dry_err(create("shared_network", "")).await,
            "a target project is required"
        );
        let r = dry(OsAction::RbacPolicyUpdate {
            id: id("r1"),
            target_project: id("*"),
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/rbac-policies/r1"));
        assert_eq!(
            body(&r[0]),
            json!({"rbac_policy": {"target_tenant": "*"}}),
            "Horizon's edit changes only the target"
        );
        assert!(
            dry_err(OsAction::RbacPolicyUpdate {
                id: id("r1"),
                target_project: id(" ")
            })
            .await,
            "a target is required"
        );
        let r = dry(OsAction::RbacPolicyDelete { id: id("r1") }).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/rbac-policies/r1")
        );
    }

    #[tokio::test]
    async fn ipv6_modes_and_host_routes_reach_the_subnet() {
        let spec = |ip_version: u8, cidr: &str, routes: Vec<(String, String)>| SubnetSpec {
            name: String::new(),
            cidr: cidr.into(),
            ip_version,
            gateway_ip: None,
            enable_dhcp: true,
            dns_nameservers: vec![],
            allocation_pools: vec![],
            ipv6_ra_mode: Some("slaac".into()),
            ipv6_address_mode: Some("slaac".into()),
            host_routes: routes,
        };
        let v6 = spec(
            6,
            "2001:db8::/64",
            vec![("2001:db8:1::/64".into(), "2001:db8::1".into())],
        );
        let r = dry(OsAction::SubnetCreate {
            network_id: id("n1"),
            spec: v6,
        })
        .await;
        let s = &body(&r[0])["subnet"];
        assert_eq!(
            (&s["ipv6_ra_mode"], &s["ipv6_address_mode"]),
            (&json!("slaac"), &json!("slaac"))
        );
        assert_eq!(
            s["host_routes"],
            json!([{"destination": "2001:db8:1::/64", "nexthop": "2001:db8::1"}])
        );
        let r = dry(OsAction::SubnetCreate {
            network_id: id("n1"),
            spec: spec(4, "10.0.0.0/24", vec![]),
        })
        .await;
        assert!(
            body(&r[0])["subnet"].get("ipv6_ra_mode").is_none(),
            "IPv4: the modes are not sent (Neutron refuses them)"
        );
        let bad = spec(4, "10.0.0.0/24", vec![("10.1.0.0/24".into(), "2001:db8::1".into())]);
        assert!(
            dry_err(OsAction::SubnetCreate {
                network_id: id("n1"),
                spec: bad
            })
            .await,
            "a route's next hop must match its IP version"
        );
    }

    /// Edits and creates no other test dry-ran (found by
    /// `every_action_has_a_dry_run_test`). An empty description clears it.
    #[tokio::test]
    async fn renames_subnets_gateways_and_security_groups() {
        let one = |r: Vec<RequestPreview>| {
            assert_eq!(r.len(), 1);
            (r[0].method.clone(), r[0].path.clone(), body(&r[0]))
        };
        let r = dry(OsAction::VolumeUpdate {
            id: id("v1"),
            name: id(" data "),
            description: None,
        })
        .await;
        assert_eq!(
            one(r),
            (
                id("PUT"),
                id("/volumes/v1"),
                json!({"volume": {"name": "data", "description": ""}})
            )
        );
        let r = dry(OsAction::SnapshotUpdate {
            id: id("s1"),
            name: id("snap"),
            description: Some(id("daily")),
        })
        .await;
        assert_eq!(
            one(r),
            (
                id("PUT"),
                id("/snapshots/s1"),
                json!({"snapshot": {"name": "snap", "description": "daily"}})
            )
        );
        let r = dry(OsAction::NetworkUpdate {
            id: id("n1"),
            name: id("net"),
            admin_up: false,
        })
        .await;
        assert_eq!(
            one(r),
            (
                id("PUT"),
                id("/networks/n1"),
                json!({"network": {"name": "net", "admin_state_up": false}})
            )
        );
        let r = dry(OsAction::AdminNetworkUpdate {
            id: id("n1"),
            name: id(" ext "),
            admin_up: true,
            shared: true,
            external: false,
        })
        .await;
        assert_eq!(
            one(r),
            (
                id("PUT"),
                id("/networks/n1"),
                json!({"network": {"name": "ext", "admin_state_up": true, "shared": true, "router:external": false}})
            ),
            "Horizon's admin edit sends shared and external too, never provider attributes"
        );
        let spec = SubnetSpec {
            name: "sub".into(),
            cidr: "10.1.0.0/24".into(),
            ip_version: 4,
            gateway_ip: Some("10.1.0.1".into()),
            enable_dhcp: true,
            dns_nameservers: vec![],
            allocation_pools: vec![],
            ipv6_ra_mode: None,
            ipv6_address_mode: None,
            host_routes: vec![],
        };
        let (method, path, b) = one(dry(OsAction::SubnetCreate {
            network_id: id("n1"),
            spec,
        })
        .await);
        assert_eq!((method.as_str(), path.as_str()), ("POST", "/subnets"));
        assert_eq!(
            b,
            json!({"subnet": {"network_id": "n1", "name": "sub", "ip_version": 4, "cidr": "10.1.0.0/24", "gateway_ip": "10.1.0.1", "enable_dhcp": true}})
        );
        let r = dry(OsAction::SubnetUpdate {
            id: id("s1"),
            name: id("sub"),
            enable_dhcp: false,
            dns_nameservers: vec![id("1.1.1.1")],
        })
        .await;
        assert_eq!(
            one(r),
            (
                id("PUT"),
                id("/subnets/s1"),
                json!({"subnet": {"name": "sub", "enable_dhcp": false, "dns_nameservers": ["1.1.1.1"]}})
            )
        );
        let r = dry(OsAction::RouterSetGateway {
            id: id("r1"),
            network_id: id("ext"),
        })
        .await;
        assert_eq!(
            one(r),
            (
                id("PUT"),
                id("/routers/r1"),
                json!({"router": {"external_gateway_info": {"network_id": "ext"}}})
            )
        );
        let (method, path, b) = one(dry(OsAction::SecurityGroupCreate {
            name: id("web"),
            description: Some(id("HTTP in")),
        })
        .await);
        assert_eq!((method.as_str(), path.as_str()), ("POST", "/security-groups"));
        assert_eq!(b, json!({"security_group": {"name": "web", "description": "HTTP in"}}));
        let r = dry(OsAction::SecurityGroupUpdate {
            id: id("g1"),
            name: id("web"),
            description: None,
        })
        .await;
        assert_eq!(
            one(r),
            (
                id("PUT"),
                id("/security-groups/g1"),
                json!({"security_group": {"name": "web", "description": ""}})
            )
        );
        let empty = OsAction::SecurityGroupCreate {
            name: id(" "),
            description: None,
        };
        assert!(
            apply(&mut Exec::new(WriteMode::DryRun, None), &empty).await.is_err(),
            "a security group needs a name"
        );
    }

    #[tokio::test]
    async fn security_group_rules() {
        let rule = RuleSpec {
            security_group_id: "sg".into(),
            direction: "ingress".into(),
            ethertype: "IPv4".into(),
            protocol: Some("tcp".into()),
            port_min: Some(22),
            port_max: None,
            remote_ip_prefix: Some("10.0.0.0/8".into()),
            remote_group_id: None,
            description: None,
        };
        let r = dry(OsAction::RuleCreate(rule.clone())).await;
        let b = &body(&r[0])["security_group_rule"];
        assert_eq!(b["port_range_min"], 22);
        assert_eq!(b["port_range_max"], 22, "single port fills both ends");
        assert_eq!(b["direction"], "ingress");
        assert_eq!(b["ethertype"], "IPv4");

        let mut x = Exec::new(WriteMode::DryRun, None);
        let mut both = rule.clone();
        both.remote_group_id = Some("other".into());
        assert!(apply(&mut x, &OsAction::RuleCreate(both)).await.is_err());
        let mut bad = rule;
        bad.port_min = Some(100);
        bad.port_max = Some(10);
        assert!(apply(&mut x, &OsAction::RuleCreate(bad)).await.is_err());
    }

    #[tokio::test]
    async fn floating_ips() {
        let r = dry(OsAction::FloatingIpAllocate {
            network_id: id("ext"),
            description: None,
            port_id: None,
            qos_policy_id: None,
        })
        .await;
        assert_eq!(body(&r[0]), json!({"floatingip": {"floating_network_id": "ext"}}));
        let r = dry(OsAction::FloatingIpAssociate {
            id: id("f"),
            port_id: id("p"),
        })
        .await;
        assert_eq!(body(&r[0]), json!({"floatingip": {"port_id": "p"}}));
        let r = dry(OsAction::FloatingIpDisassociate { id: id("f") }).await;
        assert_eq!(body(&r[0]), json!({"floatingip": {"port_id": null}}));
        let r = dry(OsAction::FloatingIpRelease { id: id("f") }).await;
        assert_eq!((r[0].method.as_str(), r[0].service.as_str()), ("DELETE", "network"));
    }

    #[tokio::test]
    async fn every_delete_is_a_delete() {
        let deletes = [
            OsAction::ImageDelete { id: id("x") },
            OsAction::ServerGroupDelete { id: id("x") },
            OsAction::VolumeDelete { id: id("x") },
            OsAction::NetworkDelete { id: id("x") },
            OsAction::SubnetDelete { id: id("x") },
            OsAction::PortDelete { id: id("x") },
            OsAction::RouterDelete { id: id("x") },
            OsAction::SecurityGroupDelete { id: id("x") },
            OsAction::RuleDelete { id: id("x") },
            OsAction::LoadBalancerDelete {
                id: id("x"),
                cascade: false,
            },
        ];
        for a in deletes {
            let r = dry(a.clone()).await;
            assert_eq!(r.len(), 1);
            assert_eq!(r[0].method, "DELETE", "{a:?}");
            assert!(r[0].path.ends_with("/x"), "{a:?} → {}", r[0].path);
            assert!(r[0].body.is_none());
        }
    }

    fn forwarding(internal: &str, external: &str, ranges: bool) -> PortForwardingSpec {
        PortForwardingSpec {
            floating_ip_id: id("fip"),
            protocol: id("tcp"),
            internal_port_id: id("port"),
            internal_ip: id("10.0.0.5"),
            internal: id(internal),
            external: id(external),
            description: Some(id("ssh")),
            ranges,
        }
    }

    #[tokio::test]
    async fn port_forwarding_create_sends_integer_ports() {
        let r = dry(OsAction::PortForwardingCreate(forwarding("22", "2222", false))).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
            ("POST", "/floatingips/fip/port_forwardings", "network")
        );
        assert_eq!(
            body(&r[0]),
            json!({"port_forwarding": {"protocol": "tcp", "internal_port_id": "port", "internal_ip_address": "10.0.0.5",
                   "internal_port": 22, "external_port": 2222, "description": "ssh"}})
        );
    }

    #[tokio::test]
    async fn port_forwarding_ranges_update_and_delete() {
        let r = dry(OsAction::PortForwardingUpdate {
            id: id("pf1"),
            spec: forwarding("8080-8090", "80:90", true),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/floatingips/fip/port_forwardings/pf1")
        );
        let pf = &body(&r[0])["port_forwarding"];
        assert_eq!(
            (pf["internal_port_range"].clone(), pf["external_port_range"].clone()),
            (json!("8080:8090"), json!("80:90"))
        );
        assert!(pf.get("internal_port").is_none());
        let r = dry(OsAction::PortForwardingDelete {
            floating_ip_id: id("fip"),
            id: id("pf1"),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/floatingips/fip/port_forwardings/pf1")
        );
        assert!(
            dry_err(OsAction::PortForwardingCreate(forwarding("80:90", "80:91", true))).await,
            "sizes differ"
        );
        assert!(
            dry_err(OsAction::PortForwardingCreate(forwarding("80:90", "80:90", false))).await,
            "no ranges extension"
        );
    }

    #[tokio::test]
    async fn trunks_and_their_subports() {
        let vlan = |p: &str, n: u16| SubportSpec {
            port_id: id(p),
            segmentation_type: id("vlan"),
            segmentation_id: Some(n),
        };
        let r = dry(OsAction::TrunkCreate {
            name: id("t1"),
            description: None,
            port_id: id("parent"),
            admin_up: true,
            subports: vec![vlan("sp1", 101)],
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
            ("POST", "/trunks", "network")
        );
        assert_eq!(
            body(&r[0]),
            json!({"trunk": {"name": "t1", "port_id": "parent", "admin_state_up": true,
                             "sub_ports": [{"port_id": "sp1", "segmentation_type": "vlan", "segmentation_id": 101}]}})
        );
        let inherit = SubportSpec {
            port_id: id("sp2"),
            segmentation_type: id("inherit"),
            segmentation_id: None,
        };
        let r = dry(OsAction::TrunkAddSubports {
            id: id("t1"),
            subports: vec![inherit],
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/trunks/t1/add_subports")
        );
        assert_eq!(
            body(&r[0]),
            json!({"sub_ports": [{"port_id": "sp2", "segmentation_type": "inherit"}]})
        );
        let r = dry(OsAction::TrunkRemoveSubports {
            id: id("t1"),
            port_ids: vec![id("sp1")],
        })
        .await;
        assert_eq!(
            (r[0].path.as_str(), body(&r[0])),
            ("/trunks/t1/remove_subports", json!({"sub_ports": [{"port_id": "sp1"}]}))
        );
        let r = dry(OsAction::TrunkUpdate {
            id: id("t1"),
            name: id("t1"),
            description: id("x"),
            admin_up: false,
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/trunks/t1"));
        let r = dry(OsAction::TrunkDelete { id: id("t1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/trunks/t1"));
        assert!(
            dry_err(OsAction::TrunkAddSubports {
                id: id("t1"),
                subports: vec![vlan("sp", 0)]
            })
            .await,
            "VLAN 0"
        );
        assert!(
            dry_err(OsAction::TrunkAddSubports {
                id: id("t1"),
                subports: vec![vlan("sp", 4095)]
            })
            .await,
            "VLAN 4095"
        );
        assert!(
            dry_err(OsAction::TrunkRemoveSubports {
                id: id("t1"),
                port_ids: vec![]
            })
            .await
        );
    }

    #[tokio::test]
    async fn port_security_groups_and_allocate_with_association() {
        let r = dry(OsAction::PortSetSecurityGroups {
            id: id("p"),
            security_group_ids: vec![id("g1"), id("g2")],
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/ports/p"));
        assert_eq!(body(&r[0]), json!({"port": {"security_groups": ["g1", "g2"]}}));
        let r = dry(OsAction::PortSetSecurityGroups {
            id: id("p"),
            security_group_ids: vec![],
        })
        .await;
        assert_eq!(body(&r[0]), json!({"port": {"security_groups": []}}));
        let r = dry(OsAction::FloatingIpAllocate {
            network_id: id("ext"),
            description: None,
            port_id: Some(id("p")),
            qos_policy_id: None,
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"floatingip": {"floating_network_id": "ext", "port_id": "p"}})
        );
    }

    #[tokio::test]
    async fn port_update_sends_only_the_changed_fields() {
        let upd = |name: Option<&str>, port_security, groups: Option<Vec<String>>| OsAction::PortUpdate {
            id: id("p"),
            name: name.map(id),
            admin_up: None,
            vnic_type: None,
            port_security,
            security_group_ids: groups,
        };
        let r = dry(upd(Some(" web "), None, None)).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/ports/p"));
        assert_eq!(body(&r[0]), json!({"port": {"name": "web"}}), "rename only");
        let r = dry(upd(None, Some(false), Some(vec![]))).await;
        assert_eq!(
            body(&r[0]),
            json!({"port": {"port_security_enabled": false, "security_groups": []}}),
            "port security off"
        );
        let r = dry(upd(None, None, Some(vec![id("g1"), id("g2")]))).await;
        assert_eq!(
            body(&r[0]),
            json!({"port": {"security_groups": ["g1", "g2"]}}),
            "groups only"
        );
        let r = dry(OsAction::PortUpdate {
            id: id("p"),
            name: None,
            admin_up: Some(false),
            vnic_type: Some(id("direct")),
            port_security: None,
            security_group_ids: None,
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"port": {"admin_state_up": false, "binding:vnic_type": "direct"}})
        );
        let bad = OsAction::PortUpdate {
            id: id("p"),
            name: None,
            admin_up: None,
            vnic_type: Some(id("warp")),
            port_security: None,
            security_group_ids: None,
        };
        assert!(dry_err(bad).await, "unknown VNIC type is refused before sending");
    }
}
