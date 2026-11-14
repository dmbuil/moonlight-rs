//! Octavia writes: load balancers, listeners, pools, members, health monitors.

use super::*;

/// Fills an SDK member builder (the load balancer create body, `member::create`
/// and the batch `member::replace` each have their own type) from a spec.
macro_rules! member_fields {
    ($b:expr, $m:expr) => {{
        let (b, m) = (&mut $b, $m);
        b.address(m.address.trim())
            .protocol_port(i32::from(m.port))
            .backup(m.backup)
            .admin_state_up(m.admin_up);
        if let Some(v) = opt(&m.name) {
            b.name(v);
        }
        if let Some(v) = opt(&m.subnet_id) {
            b.subnet_id(v);
        }
        if let Some(w) = m.weight {
            b.weight(i32::from(w));
        }
        if let Some(v) = opt(&m.monitor_address) {
            b.monitor_address(v);
        }
        if let Some(p) = m.monitor_port {
            b.monitor_port(i32::from(p));
        }
    }};
}

/// Fills an SDK listener builder (the load balancer create body and
/// `listener::create` have their own types) with the *Advanced* options
///: only what differs from Octavia's defaults is sent.
macro_rules! listener_options {
    ($b:expr, $o:expr) => {{
        let (b, o): (_, &LbListenerOptions) = (&mut $b, $o);
        if let Some(t) = o.timeout_client_data {
            b.timeout_client_data(t as i32);
        }
        if let Some(t) = o.timeout_member_connect {
            b.timeout_member_connect(t as i32);
        }
        if let Some(t) = o.timeout_member_data {
            b.timeout_member_data(t as i32);
        }
        if let Some(t) = o.timeout_tcp_inspect {
            b.timeout_tcp_inspect(t as i32);
        }
        if !o.insert_headers.is_empty() {
            b.insert_headers(o.insert_headers.iter().map(|h| (h.as_str(), "true")));
        }
        if let Some(c) = o.allowed_cidrs.as_ref().filter(|c| !c.is_empty()) {
            b.allowed_cidrs(c.iter().map(|x| Cow::from(x.trim().to_string())).collect::<Vec<_>>());
        }
        if let Some(c) = o.ciphers() {
            b.tls_ciphers(c);
        }
    }};
}

/// A `TERMINATED_HTTPS` listener's certificates and, when off, its admin state
///, on either SDK listener builder.
macro_rules! listener_certificates {
    ($b:expr, $l:expr) => {{
        let (b, l): (_, &LbListenerSpec) = (&mut $b, $l);
        if let Some(r) = opt(&l.default_tls_container_ref) {
            b.default_tls_container_ref(r);
        }
        if !l.admin_up {
            b.admin_state_up(false);
        }
        let sni: Vec<Cow<str>> = l
            .sni_container_refs
            .iter()
            .map(|r| r.trim())
            .filter(|r| !r.is_empty())
            .map(|r| Cow::from(r.to_string()))
            .collect();
        if !sni.is_empty() {
            b.sni_container_refs(sni);
        }
    }};
}

/// Pool re-encryption: `tls_enabled` only when on, so older
/// Octavias (< 2.8) never see the key; ciphers only with it. Also the admin
/// state, when off.
macro_rules! pool_tls {
    ($b:expr, $p:expr) => {{
        let (b, p): (_, &LbPoolSpec) = (&mut $b, $p);
        if !p.admin_up {
            b.admin_state_up(false);
        }
        if p.tls_enabled {
            b.tls_enabled(true);
            if let Some(c) = opt(&p.tls_ciphers) {
                b.tls_ciphers(c);
            }
        }
    }};
}

/// `OsAction::LoadBalancerCreate`: one `POST /v2/lbaas/loadbalancers` carrying the wizard's
/// whole tree (listener, pool, members, health monitor) as Octavia's fully populated create,
/// so it is built at once or not at all. Checked by `LbCreateSpec::check` first.
pub(super) async fn load_balancer_create(x: &mut Exec<'_>, spec: &LbCreateSpec) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::loadbalancer::create as m;
    spec.check().map_err(AppError::invalid)?;
    let mut lb = m::LoadbalancerBuilder::default();
    lb.vip_subnet_id(spec.vip_subnet_id.trim());
    if let Some(v) = opt(&spec.name) {
        lb.name(v);
    }
    if let Some(v) = opt(&spec.description) {
        lb.description(v);
    }
    if let Some(v) = opt(&spec.vip_address) {
        lb.vip_address(v);
    }
    if let Some(v) = opt(&spec.flavor_id) {
        lb.flavor_id(v);
    }
    if let Some(v) = opt(&spec.availability_zone) {
        lb.availability_zone(v);
    }
    // Only "down" is sent: the bodies stay as before for the default.
    if !spec.admin_up {
        lb.admin_state_up(false);
    }
    if let Some(l) = &spec.listener {
        let mut lb_l = m::ListenersBuilder::default();
        lb_l.protocol(enum_of::<m::ListenersProtocol>(&l.protocol, "listener protocol")?)
            .protocol_port(i32::from(l.port));
        if let Some(v) = opt(&l.name) {
            lb_l.name(v);
        }
        if let Some(v) = opt(&l.description) {
            lb_l.description(v);
        }
        lb_l.connection_limit(l.connection_limit.map_or(-1, |n| n as i32));
        listener_options!(lb_l, &l.options);
        listener_certificates!(lb_l, l);
        if let Some(p) = &spec.pool {
            let mut pool = m::DefaultPoolBuilder::default();
            pool.protocol(enum_of::<m::DefaultPoolProtocol>(&p.protocol, "pool protocol")?)
                .lb_algorithm(enum_of::<m::LbAlgorithm>(&p.algorithm, "algorithm")?);
            if let Some(v) = opt(&p.name) {
                pool.name(v);
            }
            if let Some(v) = opt(&p.description) {
                pool.description(v);
            }
            if let Some(kind) = p.session_persistence.as_deref() {
                let mut sp = m::SessionPersistenceBuilder::default();
                sp._type(enum_of::<m::SessionPersistenceType>(kind, "session persistence")?);
                if let Some(c) = opt(&p.cookie_name).filter(|_| kind == "APP_COOKIE") {
                    sp.cookie_name(c);
                }
                pool.session_persistence(sp.build().map_err(build_err)?);
            }
            pool_tls!(pool, p);
            let members = spec
                .members
                .iter()
                .map(|mb| {
                    let mut b = m::MembersBuilder::default();
                    member_fields!(b, mb);
                    b.build().map_err(build_err)
                })
                .collect::<R<Vec<_>>>()?;
            if !members.is_empty() {
                pool.members(members);
            }
            if let Some(hm) = &spec.monitor {
                let mut b = m::HealthmonitorBuilder::default();
                b._type(enum_of::<m::HealthmonitorType>(&hm.kind, "monitor type")?)
                    .delay(hm.delay as i32)
                    .timeout(hm.timeout as i32)
                    .max_retries(i32::from(hm.max_retries));
                if let Some(n) = hm.max_retries_down {
                    b.max_retries_down(i32::from(n));
                }
                if let Some(v) = opt(&hm.name) {
                    b.name(v);
                }
                if let Some(v) = hm.http_method.as_deref() {
                    b.http_method(enum_of::<m::HttpMethod>(v, "HTTP method")?);
                }
                if let Some(v) = opt(&hm.url_path) {
                    b.url_path(v);
                }
                if let Some(v) = opt(&hm.expected_codes) {
                    b.expected_codes(v);
                }
                if !hm.admin_up {
                    b.admin_state_up(false);
                }
                pool.healthmonitor(b.build().map_err(build_err)?);
            }
            lb_l.default_pool(pool.build().map_err(build_err)?);
        }
        lb.listeners(vec![lb_l.build().map_err(build_err)?]);
    }
    let body = lb.build().map_err(build_err)?;
    x.create(
        m::Request::builder().loadbalancer(body).build().map_err(build_err)?,
        "load balancer",
    )
    .await?;
    Ok(())
}

/// `OsAction::LoadBalancerUpdate`: `PUT /v2/lbaas/loadbalancers/{id}` with name, description
/// and admin state.
pub(super) async fn load_balancer_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    description: &str,
    admin_up: &bool,
) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::loadbalancer::set as m;
    let body = m::LoadbalancerBuilder::default()
        .name(name.trim())
        .description(description.trim())
        .admin_state_up(*admin_up)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .loadbalancer(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `OsAction::ListenerCreate`: `POST /v2/lbaas/listeners` on a load balancer, with its options,
/// TLS certificates (Barbican refs) and optional default pool.
pub(super) async fn listener_create(
    x: &mut Exec<'_>,
    loadbalancer_id: &str,
    spec: &LbListenerSpec,
    default_pool_id: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::listener::create as m;
    spec.check().map_err(AppError::invalid)?;
    let mut b = m::ListenerBuilder::default();
    b.loadbalancer_id(loadbalancer_id)
        .protocol(enum_of::<m::ListenerProtocol>(&spec.protocol, "listener protocol")?)
        .protocol_port(i32::from(spec.port))
        .connection_limit(spec.connection_limit.map_or(-1, |n| n as i32));
    if let Some(v) = opt(&spec.name) {
        b.name(v);
    }
    if let Some(v) = opt(&spec.description) {
        b.description(v);
    }
    if let Some(v) = opt(default_pool_id) {
        b.default_pool_id(v);
    }
    listener_options!(b, &spec.options);
    listener_certificates!(b, spec);
    x.create(
        m::Request::builder()
            .listener(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "listener",
    )
    .await?;
    Ok(())
}

/// `OsAction::ListenerUpdate`: `PUT /v2/lbaas/listeners/{id}`.
///
/// JSON: a cleared default pool must be sent as `null`, which the SDK cannot express.
/// Cleared options likewise: `null` timeouts, `{}` headers and `[]` CIDRs are
/// Octavia's "back to the default".
#[allow(clippy::too_many_arguments)] // one parameter per field of the variant
pub(super) async fn listener_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    description: &str,
    connection_limit: &Option<u32>,
    default_pool_id: &Option<String>,
    admin_up: &bool,
    protocol: &str,
    options: &Option<LbListenerOptions>,
    tls: &Option<(String, Vec<String>)>,
) -> R<()> {
    let mut body = json!({"listener": {
        "name": name.trim(),
        "description": description.trim(),
        "connection_limit": connection_limit.map_or(-1, i64::from),
        "default_pool_id": opt(default_pool_id),
        "admin_state_up": admin_up,
    }});
    if let Some(o) = options {
        o.check(protocol).map_err(AppError::invalid)?;
        let l = &mut body["listener"];
        l["timeout_client_data"] = json!(o.timeout_client_data);
        l["timeout_member_connect"] = json!(o.timeout_member_connect);
        l["timeout_member_data"] = json!(o.timeout_member_data);
        l["timeout_tcp_inspect"] = json!(o.timeout_tcp_inspect);
        // Octavia refuses `insert_headers` on other listeners, even empty.
        if lb_inserts_headers(protocol) {
            l["insert_headers"] = Value::Object(o.headers_map());
        }
        if let Some(c) = &o.allowed_cidrs {
            l["allowed_cidrs"] = json!(c.iter().map(|x| x.trim()).collect::<Vec<_>>());
        }
        // Set or changed only: `null` would need Octavia 2.15 (like pools).
        if let Some(c) = o.ciphers() {
            l["tls_ciphers"] = json!(c);
        }
    }
    if let Some((default_ref, sni)) = tls {
        check_listener_tls(protocol, Some(default_ref), sni).map_err(AppError::invalid)?;
        body["listener"]["default_tls_container_ref"] = json!(default_ref.trim());
        body["listener"]["sni_container_refs"] = json!(sni.iter().map(|r| r.trim()).collect::<Vec<_>>());
    }
    x.send(octavia(
        http::Method::PUT,
        format!("lbaas/listeners/{}", non_empty(id, "listener")?),
        body,
    ))
    .await?;
    Ok(())
}

/// `OsAction::ListenerDelete`: `DELETE /v2/lbaas/listeners/{id}`.
pub(super) async fn listener_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::listener::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `OsAction::PoolCreate`: `POST /v2/lbaas/pools`, under a listener (which implies its load
/// balancer) or directly under the load balancer.
pub(super) async fn pool_create(
    x: &mut Exec<'_>,
    loadbalancer_id: &str,
    listener_id: &Option<String>,
    spec: &LbPoolSpec,
) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::pool::create as m;
    spec.check().map_err(AppError::invalid)?;
    let mut b = m::PoolBuilder::default();
    b.protocol(enum_of::<m::Protocol>(&spec.protocol, "pool protocol")?)
        .lb_algorithm(enum_of::<m::LbAlgorithm>(&spec.algorithm, "algorithm")?);
    // Octavia takes one of the two: the listener implies its load balancer.
    match opt(listener_id) {
        Some(l) => b.listener_id(l),
        None => b.loadbalancer_id(non_empty(loadbalancer_id, "load balancer")?),
    };
    if let Some(v) = opt(&spec.name) {
        b.name(v);
    }
    if let Some(v) = opt(&spec.description) {
        b.description(v);
    }
    if let Some(kind) = spec.session_persistence.as_deref() {
        let mut sp = m::SessionPersistenceBuilder::default();
        sp._type(enum_of::<m::SessionPersistenceType>(kind, "session persistence")?);
        if let Some(c) = opt(&spec.cookie_name).filter(|_| kind == "APP_COOKIE") {
            sp.cookie_name(c);
        }
        b.session_persistence(sp.build().map_err(build_err)?);
    }
    pool_tls!(b, spec);
    x.create(
        m::Request::builder()
            .pool(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "pool",
    )
    .await?;
    Ok(())
}

/// `OsAction::PoolUpdate`: `PUT /v2/lbaas/pools/{id}`, validated through an `LbPoolSpec`.
///
/// JSON: removing persistence means `"session_persistence": null`.
#[allow(clippy::too_many_arguments)] // one parameter per field of the variant
pub(super) async fn pool_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    description: &str,
    algorithm: &str,
    session_persistence: &Option<String>,
    cookie_name: &Option<String>,
    admin_up: &bool,
    protocol: &str,
    tls: &Option<(bool, Option<String>)>,
) -> R<()> {
    let (tls_enabled, tls_ciphers) = tls.clone().unwrap_or_default();
    LbPoolSpec {
        name: None,
        description: None,
        protocol: if protocol.is_empty() {
            "HTTP".into()
        } else {
            protocol.to_string()
        },
        algorithm: algorithm.to_string(),
        session_persistence: session_persistence.clone(),
        cookie_name: cookie_name.clone(),
        tls_enabled,
        tls_ciphers: tls_ciphers.clone(),
        admin_up: *admin_up,
    }
    .check()
    .map_err(AppError::invalid)?;
    let persistence =
        session_persistence
            .as_deref()
            .map(|kind| match opt(cookie_name).filter(|_| kind == "APP_COOKIE") {
                Some(c) => json!({"type": kind, "cookie_name": c}),
                None => json!({"type": kind}),
            });
    let mut body = json!({"pool": {
        "name": name.trim(),
        "description": description.trim(),
        "lb_algorithm": algorithm,
        "session_persistence": persistence,
        "admin_state_up": admin_up,
    }});
    // Ciphers can be changed but not cleared here: `null` would need
    // Octavia 2.15 even when the dialog didn't offer them.
    if let Some((enabled, ciphers)) = tls {
        body["pool"]["tls_enabled"] = json!(enabled);
        if let Some(c) = opt(ciphers).filter(|_| *enabled) {
            body["pool"]["tls_ciphers"] = json!(c);
        }
    }
    x.send(octavia(
        http::Method::PUT,
        format!("lbaas/pools/{}", non_empty(id, "pool")?),
        body,
    ))
    .await?;
    Ok(())
}

/// `OsAction::PoolDelete`: `DELETE /v2/lbaas/pools/{id}`.
pub(super) async fn pool_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::pool::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `OsAction::MemberCreate`: `POST /v2/lbaas/pools/{pool_id}/members`.
pub(super) async fn member_create(x: &mut Exec<'_>, pool_id: &str, spec: &LbMemberSpec) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::pool::member::create as m;
    spec.check().map_err(AppError::invalid)?;
    let mut b = m::MemberBuilder::default();
    member_fields!(b, spec);
    let ep = m::Request::builder()
        .pool_id(pool_id)
        .member(b.build().map_err(build_err)?)
        .build()
        .map_err(build_err)?;
    x.create(ep, "member").await?;
    Ok(())
}

/// `OsAction::MemberUpdate`: `PUT /v2/lbaas/pools/{pool_id}/members/{id}`, as JSON so a
/// cleared monitor address or port goes as `null`. Weight (0-256), monitor address (an IP)
/// and port are checked here.
#[allow(clippy::too_many_arguments)] // one parameter per field of the variant
pub(super) async fn member_update(
    x: &mut Exec<'_>,
    pool_id: &str,
    id: &str,
    name: &str,
    weight: &u16,
    backup: &bool,
    admin_up: &bool,
    monitor_address: &Option<String>,
    monitor_port: &Option<u16>,
) -> R<()> {
    if *weight > 256 {
        return Err(AppError::invalid("the weight must be 0-256"));
    }
    let monitor_address = opt(monitor_address);
    if let Some(a) = monitor_address {
        a.parse::<std::net::IpAddr>()
            .map_err(|_| AppError::invalid(format!("not an IP address: {a}")))?;
    }
    if *monitor_port == Some(0) {
        return Err(AppError::invalid("the monitor port must be 1-65535"));
    }
    // JSON, not `member::set`: clearing the monitor address/port needs an
    // explicit `null`, which the SDK builder can't send.
    let body = json!({"member": {
        "name": name.trim(),
        "weight": weight,
        "backup": backup,
        "admin_state_up": admin_up,
        "monitor_address": monitor_address,
        "monitor_port": monitor_port,
    }});
    let path = format!(
        "lbaas/pools/{}/members/{}",
        non_empty(pool_id, "pool")?,
        non_empty(id, "member")?
    );
    x.send(octavia(http::Method::PUT, path, body)).await?;
    Ok(())
}

/// `OsAction::PoolSetMembers`: `PUT /v2/lbaas/pools/{pool_id}/members`, Octavia's batch
/// update: the list replaces the pool's members, adding, changing and removing in one call.
pub(super) async fn pool_set_members(x: &mut Exec<'_>, pool_id: &str, members: &[LbMemberSpec]) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::pool::member::replace as m;
    members
        .iter()
        .try_for_each(LbMemberSpec::check)
        .map_err(AppError::invalid)?;
    let list = members
        .iter()
        .map(|mb| {
            let mut b = m::MembersBuilder::default();
            member_fields!(b, mb);
            b.build().map_err(build_err)
        })
        .collect::<R<Vec<_>>>()?;
    x.send(
        m::Request::builder()
            .pool_id(non_empty(pool_id, "pool")?)
            .members(list)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `OsAction::MemberDelete`: `DELETE /v2/lbaas/pools/{pool_id}/members/{id}`.
pub(super) async fn member_delete(x: &mut Exec<'_>, pool_id: &str, id: &str) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::pool::member::delete as m;
    x.send(
        m::Request::builder()
            .pool_id(pool_id)
            .id(id)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `OsAction::MonitorCreate`: `POST /v2/lbaas/healthmonitors` on a pool (one monitor per
/// pool). Checked by `LbMonitorSpec::check`.
pub(super) async fn monitor_create(x: &mut Exec<'_>, pool_id: &str, spec: &LbMonitorSpec) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::healthmonitor::create as m;
    spec.check().map_err(AppError::invalid)?;
    let mut b = m::HealthmonitorBuilder::default();
    b.pool_id(pool_id)
        ._type(enum_of::<m::Type>(&spec.kind, "monitor type")?)
        .delay(spec.delay as i32)
        .timeout(spec.timeout as i32)
        .max_retries(i32::from(spec.max_retries));
    if let Some(n) = spec.max_retries_down {
        b.max_retries_down(i32::from(n));
    }
    if let Some(v) = opt(&spec.name) {
        b.name(v);
    }
    if let Some(v) = spec.http_method.as_deref() {
        b.http_method(enum_of::<m::HttpMethod>(v, "HTTP method")?);
    }
    if let Some(v) = opt(&spec.url_path) {
        b.url_path(v);
    }
    if let Some(v) = opt(&spec.expected_codes) {
        b.expected_codes(v);
    }
    if !spec.admin_up {
        b.admin_state_up(false);
    }
    x.create(
        m::Request::builder()
            .healthmonitor(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "health monitor",
    )
    .await?;
    Ok(())
}

/// `OsAction::MonitorUpdate`: `PUT /v2/lbaas/healthmonitors/{id}`. The type can't change, so
/// it isn't sent; unset optional fields are left as they are.
pub(super) async fn monitor_update(x: &mut Exec<'_>, id: &str, spec: &LbMonitorSpec, admin_up: &bool) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::healthmonitor::set as m;
    spec.check().map_err(AppError::invalid)?;
    let mut b = m::HealthmonitorBuilder::default();
    b.name(spec.name.as_deref().map(str::trim).unwrap_or(""))
        .delay(spec.delay as i32)
        .timeout(spec.timeout as i32)
        .max_retries(i32::from(spec.max_retries))
        .admin_state_up(*admin_up);
    if let Some(n) = spec.max_retries_down {
        b.max_retries_down(i32::from(n));
    }
    if let Some(v) = spec.http_method.as_deref() {
        b.http_method(enum_of::<m::HttpMethod>(v, "HTTP method")?);
    }
    if let Some(v) = opt(&spec.url_path) {
        b.url_path(v);
    }
    if let Some(v) = opt(&spec.expected_codes) {
        b.expected_codes(v);
    }
    x.send(
        m::Request::builder()
            .id(id)
            .healthmonitor(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `OsAction::MonitorDelete`: `DELETE /v2/lbaas/healthmonitors/{id}`.
pub(super) async fn monitor_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::healthmonitor::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `OsAction::LoadBalancerDelete`: `DELETE /v2/lbaas/loadbalancers/{id}`, with `?cascade=true`
/// when asked: Octavia otherwise refuses a load balancer that still has listeners or pools.
pub(super) async fn load_balancer_delete(x: &mut Exec<'_>, id: &str, cascade: &bool) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::loadbalancer::delete as m;
    let mut b = m::Request::builder();
    b.id(id);
    if *cascade {
        b.cascade(true);
    }
    x.send(b.build().map_err(build_err)?).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    /// Listener timeouts, headers and CIDRs, and pool
    /// re-encryption, in the wizard body, the single creates and the edits.
    #[tokio::test]
    async fn listener_and_pool_options() {
        let options = LbListenerOptions {
            timeout_client_data: Some(60_000),
            timeout_tcp_inspect: Some(0),
            insert_headers: vec![id("X-Forwarded-For"), id("X-Forwarded-Proto")],
            allowed_cidrs: Some(vec![id(" 192.0.2.0/24 ")]),
            ..Default::default()
        };
        let listener = LbListenerSpec {
            name: None,
            description: None,
            protocol: id("HTTP"),
            port: 80,
            connection_limit: None,
            options: options.clone(),
            default_tls_container_ref: None,
            sni_container_refs: vec![],
            admin_up: true,
        };
        let pool = LbPoolSpec {
            name: Some(id("web")),
            description: None,
            protocol: id("HTTP"),
            algorithm: id("ROUND_ROBIN"),
            session_persistence: None,
            cookie_name: None,
            tls_enabled: true,
            tls_ciphers: Some(id("ECDHE-RSA-AES256-GCM-SHA384")),
            admin_up: true,
        };
        let wizard = LbCreateSpec {
            name: None,
            description: None,
            vip_subnet_id: id("sub-1"),
            vip_address: None,
            flavor_id: None,
            availability_zone: None,
            listener: Some(listener.clone()),
            pool: Some(pool.clone()),
            members: vec![],
            monitor: None,
            admin_up: true,
        };
        let r = dry(OsAction::LoadBalancerCreate(Box::new(wizard))).await;
        let l = &body(&r[0])["loadbalancer"]["listeners"][0];
        let want_listener = json!({
            "timeout_client_data": 60000,
            "timeout_tcp_inspect": 0,
            "insert_headers": {"X-Forwarded-For": "true", "X-Forwarded-Proto": "true"},
            "allowed_cidrs": ["192.0.2.0/24"],
        });
        for (k, v) in want_listener.as_object().unwrap() {
            assert_eq!(&l[k], v, "wizard listener {k}");
        }
        assert!(
            l.get("timeout_member_data").is_none(),
            "unset timeouts keep Octavia's defaults"
        );
        assert_eq!(
            (
                l["default_pool"]["tls_enabled"].clone(),
                l["default_pool"]["tls_ciphers"].clone()
            ),
            (json!(true), json!("ECDHE-RSA-AES256-GCM-SHA384"))
        );

        let r = dry(OsAction::ListenerCreate {
            loadbalancer_id: id("lb1"),
            spec: listener.clone(),
            default_pool_id: None,
        })
        .await;
        assert_eq!(
            body(&r[0])["listener"]["insert_headers"],
            want_listener["insert_headers"]
        );
        let r = dry(OsAction::PoolCreate {
            loadbalancer_id: id("lb1"),
            listener_id: None,
            spec: pool.clone(),
        })
        .await;
        assert_eq!(body(&r[0])["pool"]["tls_enabled"], true);

        // Edits send cleared values as Octavia's "back to the default".
        let cleared = LbListenerOptions {
            allowed_cidrs: Some(vec![]),
            ..Default::default()
        };
        let upd = |protocol: &str, options| OsAction::ListenerUpdate {
            id: id("ls1"),
            name: id("web"),
            description: String::new(),
            connection_limit: None,
            default_pool_id: None,
            admin_up: true,
            protocol: id(protocol),
            options,
            tls: None,
        };
        let l = body(&dry(upd("HTTP", Some(cleared.clone()))).await[0])["listener"].clone();
        assert_eq!(
            (
                l["timeout_client_data"].clone(),
                l["insert_headers"].clone(),
                l["allowed_cidrs"].clone()
            ),
            (Value::Null, json!({}), json!([]))
        );
        let l = body(&dry(upd("TCP", Some(LbListenerOptions::default()))).await[0])["listener"].clone();
        assert!(
            l.get("insert_headers").is_none() && l.get("allowed_cidrs").is_none(),
            "TCP: no headers; CIDRs not offered: not sent"
        );
        let headers_on_tcp = upd(
            "TCP",
            Some(LbListenerOptions {
                insert_headers: vec![id("X-Forwarded-For")],
                ..Default::default()
            }),
        );
        assert!(
            apply(&mut Exec::new(WriteMode::DryRun, None), &headers_on_tcp)
                .await
                .is_err()
        );

        let pool_upd = |tls| OsAction::PoolUpdate {
            id: id("pool1"),
            name: id("web"),
            description: String::new(),
            algorithm: id("ROUND_ROBIN"),
            session_persistence: None,
            cookie_name: None,
            admin_up: true,
            protocol: id("HTTP"),
            tls,
        };
        let p = body(&dry(pool_upd(Some((false, None)))).await[0])["pool"].clone();
        assert_eq!((p["tls_enabled"].clone(), p.get("tls_ciphers")), (json!(false), None));
        let ciphers_without_tls = pool_upd(Some((false, Some(id("AES256")))));
        assert!(
            apply(&mut Exec::new(WriteMode::DryRun, None), &ciphers_without_tls)
                .await
                .is_err(),
            "ciphers need TLS"
        );
    }

    /// `TERMINATED_HTTPS` with a Barbican certificate, SNI
    /// certificates and ciphers; the pool behind it is HTTP.
    #[tokio::test]
    async fn tls_terminated_listeners_carry_their_certificates() {
        let cert = "https://barbican.example/v1/containers/c1";
        let sni = "https://barbican.example/v1/secrets/s2";
        let listener = LbListenerSpec {
            name: Some(id("https")),
            description: None,
            protocol: id("TERMINATED_HTTPS"),
            port: 443,
            connection_limit: None,
            options: LbListenerOptions {
                tls_ciphers: Some(id("ECDHE-RSA-AES256-GCM-SHA384")),
                insert_headers: vec![id("X-Forwarded-Proto")],
                ..Default::default()
            },
            default_tls_container_ref: Some(id(cert)),
            sni_container_refs: vec![id(sni)],
            admin_up: true,
        };
        let r = dry(OsAction::ListenerCreate {
            loadbalancer_id: id("lb1"),
            spec: listener.clone(),
            default_pool_id: None,
        })
        .await;
        let l = &body(&r[0])["listener"];
        assert_eq!(
            (
                l["protocol"].clone(),
                l["default_tls_container_ref"].clone(),
                l["sni_container_refs"].clone(),
                l["tls_ciphers"].clone(),
                l["insert_headers"].clone()
            ),
            (
                json!("TERMINATED_HTTPS"),
                json!(cert),
                json!([sni]),
                json!("ECDHE-RSA-AES256-GCM-SHA384"),
                json!({"X-Forwarded-Proto": "true"})
            )
        );
        let wizard = LbCreateSpec {
            name: None,
            description: None,
            vip_subnet_id: id("sub-1"),
            vip_address: None,
            flavor_id: None,
            availability_zone: None,
            listener: Some(listener.clone()),
            pool: None,
            members: vec![],
            monitor: None,
            admin_up: true,
        };
        let r = dry(OsAction::LoadBalancerCreate(Box::new(wizard.clone()))).await;
        assert_eq!(
            body(&r[0])["loadbalancer"]["listeners"][0]["default_tls_container_ref"],
            cert
        );
        assert!(
            OsAction::LoadBalancerCreate(Box::new(wizard))
                .plugins()
                .contains(&Plugin::KeyManager),
            "Barbican certificates need the key-manager plugin"
        );

        let no_cert = LbListenerSpec {
            default_tls_container_ref: None,
            ..listener.clone()
        };
        assert!(
            apply(
                &mut Exec::new(WriteMode::DryRun, None),
                &OsAction::ListenerCreate {
                    loadbalancer_id: id("lb1"),
                    spec: no_cert,
                    default_pool_id: None
                }
            )
            .await
            .is_err()
        );
        let http_with_cert = LbListenerSpec {
            protocol: id("HTTP"),
            options: Default::default(),
            ..listener
        };
        assert!(http_with_cert.check().is_err(), "certificates only on TERMINATED_HTTPS");

        let upd = OsAction::ListenerUpdate {
            id: id("ls1"),
            name: id("https"),
            description: String::new(),
            connection_limit: None,
            default_pool_id: None,
            admin_up: true,
            protocol: id("TERMINATED_HTTPS"),
            options: None,
            tls: Some((id(cert), vec![])),
        };
        assert!(upd.plugins().contains(&Plugin::KeyManager));
        let l = body(&dry(upd).await[0])["listener"].clone();
        assert_eq!(
            (l["default_tls_container_ref"].clone(), l["sni_container_refs"].clone()),
            (json!(cert), json!([])),
            "an empty list removes the SNI certificates"
        );
    }

    /// The availability zone, and *Admin State Up* off at
    /// every level, sent only when off (default bodies unchanged).
    #[tokio::test]
    async fn wizard_sends_the_zone_and_admin_state_down() {
        let mut w = lb_wizard();
        w.availability_zone = Some(id("az-1"));
        w.admin_up = false;
        w.listener.as_mut().unwrap().admin_up = false;
        w.pool.as_mut().unwrap().admin_up = false;
        w.monitor.as_mut().unwrap().admin_up = false;
        let b = body(&dry(OsAction::LoadBalancerCreate(Box::new(w))).await[0])["loadbalancer"].clone();
        let l = &b["listeners"][0];
        assert_eq!(
            (
                b["availability_zone"].clone(),
                b["admin_state_up"].clone(),
                l["admin_state_up"].clone(),
                l["default_pool"]["admin_state_up"].clone(),
                l["default_pool"]["healthmonitor"]["admin_state_up"].clone()
            ),
            (json!("az-1"), json!(false), json!(false), json!(false), json!(false))
        );
        let b = body(&dry(OsAction::LoadBalancerCreate(Box::new(lb_wizard()))).await[0])["loadbalancer"].clone();
        assert!(
            b.get("admin_state_up").is_none() && b["listeners"][0].get("admin_state_up").is_none(),
            "up: not sent"
        );
    }

    fn lb_wizard() -> LbCreateSpec {
        LbCreateSpec {
            name: Some(id("web-lb")),
            description: None,
            vip_subnet_id: id("sub-1"),
            vip_address: None,
            flavor_id: None,
            availability_zone: None,
            listener: Some(LbListenerSpec {
                name: Some(id("http")),
                description: None,
                protocol: id("HTTP"),
                port: 80,
                connection_limit: None,
                options: Default::default(),
                default_tls_container_ref: None,
                sni_container_refs: vec![],
                admin_up: true,
            }),
            pool: Some(LbPoolSpec {
                name: Some(id("web")),
                description: None,
                protocol: id("HTTP"),
                algorithm: id("ROUND_ROBIN"),
                session_persistence: Some(id("APP_COOKIE")),
                cookie_name: Some(id("JSESSIONID")),
                tls_enabled: false,
                tls_ciphers: None,
                admin_up: true,
            }),
            members: vec![LbMemberSpec {
                name: None,
                address: id("10.0.0.11"),
                port: 8080,
                subnet_id: Some(id("sub-1")),
                weight: Some(10),
                backup: false,
                monitor_address: None,
                monitor_port: None,
                admin_up: true,
            }],
            monitor: Some(LbMonitorSpec {
                name: None,
                kind: id("HTTP"),
                delay: 5,
                timeout: 3,
                max_retries: 3,
                max_retries_down: None,
                http_method: Some(id("GET")),
                url_path: Some(id("/healthz")),
                expected_codes: Some(id("200")),
                admin_up: true,
            }),
            admin_up: true,
        }
    }

    #[tokio::test]
    async fn pool_members_are_replaced_in_one_batch_request() {
        let web = lb_wizard().members[0].clone();
        let external = LbMemberSpec {
            name: Some(id("legacy")),
            address: id("192.0.2.10"),
            port: 8443,
            subnet_id: Some(id("sub-ext")),
            weight: None,
            backup: true,
            monitor_address: Some(id("192.0.2.11")),
            monitor_port: Some(9000),
            admin_up: false,
        };
        let r = dry(OsAction::PoolSetMembers {
            pool_id: id("pool1"),
            members: vec![web.clone(), external.clone()],
        })
        .await;
        assert_eq!(r.len(), 1);
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/lbaas/pools/pool1/members")
        );
        assert_eq!(
            body(&r[0]),
            json!({"members": [
                {"address": "10.0.0.11", "protocol_port": 8080, "subnet_id": "sub-1", "weight": 10, "backup": false, "admin_state_up": true},
                {"name": "legacy", "address": "192.0.2.10", "protocol_port": 8443, "subnet_id": "sub-ext", "backup": true,
                 "admin_state_up": false, "monitor_address": "192.0.2.11", "monitor_port": 9000}
            ]}),
            "each member with its own port; the external one with its monitor"
        );
        let r = dry(OsAction::PoolSetMembers {
            pool_id: id("pool1"),
            members: vec![],
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"members": []}),
            "an empty list removes every member"
        );
        assert!(
            dry_err(OsAction::PoolSetMembers {
                pool_id: id("pool1"),
                members: vec![LbMemberSpec { port: 0, ..web.clone() }]
            })
            .await
        );
        assert!(
            dry_err(OsAction::PoolSetMembers {
                pool_id: id(" "),
                members: vec![web]
            })
            .await,
            "the pool is required"
        );
    }

    #[tokio::test]
    async fn load_balancer_wizard_is_one_fully_populated_request() {
        let r = dry(OsAction::LoadBalancerCreate(Box::new(lb_wizard()))).await;
        assert_eq!(r.len(), 1);
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
            ("POST", "/lbaas/loadbalancers", "load-balancer")
        );
        assert_eq!(
            body(&r[0]),
            json!({"loadbalancer": {"name": "web-lb", "vip_subnet_id": "sub-1", "listeners": [{
                "name": "http", "protocol": "HTTP", "protocol_port": 80, "connection_limit": -1,
                "default_pool": {
                    "name": "web", "protocol": "HTTP", "lb_algorithm": "ROUND_ROBIN",
                    "session_persistence": {"type": "APP_COOKIE", "cookie_name": "JSESSIONID"},
                    "members": [{"address": "10.0.0.11", "protocol_port": 8080, "subnet_id": "sub-1", "weight": 10, "backup": false,
                                 "admin_state_up": true}],
                    "healthmonitor": {"type": "HTTP", "delay": 5, "timeout": 3, "max_retries": 3,
                                      "http_method": "GET", "url_path": "/healthz", "expected_codes": "200"}
                }
            }]}})
        );
        let bare = LbCreateSpec {
            listener: None,
            pool: None,
            members: vec![],
            monitor: None,
            ..lb_wizard()
        };
        assert_eq!(
            body(&dry(OsAction::LoadBalancerCreate(Box::new(bare))).await[0]),
            json!({"loadbalancer": {"name": "web-lb", "vip_subnet_id": "sub-1"}})
        );
        assert!(
            dry_err(OsAction::LoadBalancerCreate(Box::new(LbCreateSpec {
                listener: None,
                ..lb_wizard()
            })))
            .await,
            "a pool needs a listener"
        );
        for name in [None, Some(id(" "))] {
            let mut unnamed = lb_wizard();
            unnamed.pool.as_mut().unwrap().name = name;
            assert!(
                dry_err(OsAction::LoadBalancerCreate(Box::new(unnamed))).await,
                "Octavia rejects an unnamed default pool"
            );
        }
        let slow = LbMonitorSpec {
            timeout: 10,
            ..lb_wizard().monitor.unwrap()
        };
        assert!(
            dry_err(OsAction::LoadBalancerCreate(Box::new(LbCreateSpec {
                monitor: Some(slow),
                ..lb_wizard()
            })))
            .await,
            "timeout > delay"
        );
    }

    #[tokio::test]
    async fn load_balancer_parts_are_created_updated_and_deleted() {
        let w = lb_wizard();
        let r = dry(OsAction::ListenerCreate {
            loadbalancer_id: id("lb1"),
            spec: w.listener.clone().unwrap(),
            default_pool_id: None,
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/lbaas/listeners"));
        assert_eq!(
            body(&r[0])["listener"],
            json!({"loadbalancer_id": "lb1", "name": "http", "protocol": "HTTP", "protocol_port": 80, "connection_limit": -1})
        );

        let r = dry(OsAction::ListenerUpdate {
            id: id("ls1"),
            name: id("web"),
            description: String::new(),
            connection_limit: Some(500),
            default_pool_id: None,
            admin_up: true,
            protocol: id("HTTP"),
            options: None,
            tls: None,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/lbaas/listeners/ls1")
        );
        assert!(
            body(&r[0])["listener"].get("timeout_client_data").is_none(),
            "no options: left alone"
        );
        assert_eq!(
            body(&r[0])["listener"]["default_pool_id"],
            Value::Null,
            "an explicit null clears it"
        );
        assert_eq!(body(&r[0])["listener"]["connection_limit"], 500);

        let r = dry(OsAction::PoolCreate {
            loadbalancer_id: id("lb1"),
            listener_id: Some(id("ls1")),
            spec: w.pool.clone().unwrap(),
        })
        .await;
        let p = &body(&r[0])["pool"];
        assert_eq!(
            (p["listener_id"].clone(), p.get("loadbalancer_id").cloned()),
            (json!("ls1"), None),
            "the listener implies the load balancer"
        );
        let r = dry(OsAction::PoolCreate {
            loadbalancer_id: id("lb1"),
            listener_id: None,
            spec: w.pool.clone().unwrap(),
        })
        .await;
        assert_eq!(body(&r[0])["pool"]["loadbalancer_id"], "lb1");

        let r = dry(OsAction::PoolUpdate {
            id: id("pool1"),
            name: id("web"),
            description: String::new(),
            algorithm: id("LEAST_CONNECTIONS"),
            session_persistence: None,
            cookie_name: None,
            admin_up: true,
            protocol: id("HTTP"),
            tls: None,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/lbaas/pools/pool1")
        );
        assert!(
            body(&r[0])["pool"].get("tls_enabled").is_none(),
            "an old Octavia never sees the key"
        );
        assert_eq!(
            body(&r[0])["pool"]["session_persistence"],
            Value::Null,
            "removing persistence"
        );

        let r = dry(OsAction::MemberCreate {
            pool_id: id("pool1"),
            spec: w.members[0].clone(),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("POST", "/lbaas/pools/pool1/members")
        );
        let upd = |weight, monitor_address: Option<&str>, monitor_port| OsAction::MemberUpdate {
            pool_id: id("pool1"),
            id: id("m1"),
            name: String::new(),
            weight,
            backup: true,
            admin_up: false,
            monitor_address: monitor_address.map(id),
            monitor_port,
        };
        let r = dry(upd(0, None, None)).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/lbaas/pools/pool1/members/m1")
        );
        assert_eq!(
            body(&r[0]),
            json!({"member": {"name": "", "weight": 0, "backup": true, "admin_state_up": false, "monitor_address": null, "monitor_port": null}}),
            "explicit nulls: health-check the member itself"
        );
        let r = dry(upd(1, Some(" 10.0.9.1 "), Some(9000))).await;
        assert_eq!(
            (
                body(&r[0])["member"]["monitor_address"].clone(),
                body(&r[0])["member"]["monitor_port"].clone()
            ),
            (json!("10.0.9.1"), json!(9000))
        );
        assert!(dry_err(upd(300, None, None)).await, "weight 0-256");
        assert!(
            dry_err(upd(1, Some("web"), None)).await,
            "monitor address must be an IP"
        );
        assert!(dry_err(upd(1, None, Some(0))).await, "monitor port 1-65535");

        let r = dry(OsAction::MonitorCreate {
            pool_id: id("pool1"),
            spec: w.monitor.clone().unwrap(),
        })
        .await;
        assert_eq!(
            (r[0].path.as_str(), body(&r[0])["healthmonitor"]["pool_id"].clone()),
            ("/lbaas/healthmonitors", json!("pool1"))
        );
        let tcp = LbMonitorSpec {
            kind: id("TCP"),
            http_method: None,
            url_path: None,
            expected_codes: None,
            ..w.monitor.clone().unwrap()
        };
        let r = dry(OsAction::MonitorUpdate {
            id: id("hm1"),
            spec: tcp,
            admin_up: true,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/lbaas/healthmonitors/hm1")
        );
        assert!(
            body(&r[0])["healthmonitor"].get("type").is_none(),
            "the type cannot change"
        );

        for (a, path) in [
            (OsAction::ListenerDelete { id: id("x") }, "/lbaas/listeners/x"),
            (OsAction::PoolDelete { id: id("x") }, "/lbaas/pools/x"),
            (
                OsAction::MemberDelete {
                    pool_id: id("p"),
                    id: id("x"),
                },
                "/lbaas/pools/p/members/x",
            ),
            (OsAction::MonitorDelete { id: id("x") }, "/lbaas/healthmonitors/x"),
        ] {
            let r = dry(a).await;
            assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", path));
        }
        let r = dry(OsAction::LoadBalancerUpdate {
            id: id("lb1"),
            name: id("web"),
            description: id("front"),
            admin_up: false,
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"loadbalancer": {"name": "web", "description": "front", "admin_state_up": false}})
        );
    }

    #[tokio::test]
    async fn load_balancer_delete_cascades_only_when_asked() {
        let r = dry(OsAction::LoadBalancerDelete {
            id: id("lb1"),
            cascade: true,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/lbaas/loadbalancers/lb1?cascade=true")
        );
        assert_eq!(r[0].service, "load-balancer");
        let r = dry(OsAction::LoadBalancerDelete {
            id: id("lb1"),
            cascade: false,
        })
        .await;
        assert_eq!(r[0].path, "/lbaas/loadbalancers/lb1");
    }
}
