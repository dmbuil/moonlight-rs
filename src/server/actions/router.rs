//! Neutron router writes: routers, gateways, interfaces, static routes.

use super::*;

/// `OsAction::RouterCreate`: `POST /v2.0/routers`, with an external gateway when a network is
/// given. A gateway QoS policy without an external network is refused (it lives in
/// `external_gateway_info`).
pub(super) async fn router_create(
    x: &mut Exec<'_>,
    name: &str,
    admin_up: &bool,
    external_network_id: &Option<String>,
    qos_policy_id: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::network::v2::router::create as m;
    let mut b = m::RouterBuilder::default();
    b.name(name.trim()).admin_state_up(*admin_up);
    if let Some(net) = opt(external_network_id) {
        let mut gw = m::ExternalGatewayInfoBuilder::default();
        gw.network_id(net.to_string());
        if let Some(q) = opt(qos_policy_id) {
            gw.qos_policy_id(Some(Cow::from(q.to_string())));
        }
        b.external_gateway_info(Some(gw.build().map_err(build_err)?));
    } else if opt(qos_policy_id).is_some() {
        return Err(AppError::invalid("a gateway QoS policy needs an external network"));
    }
    let body = b.build().map_err(build_err)?;
    x.create(m::Request::builder().router(body).build().map_err(build_err)?, "router")
        .await?;
    Ok(())
}

/// `OsAction::AdminRouterCreate` (Admin › Routers): `POST /v2.0/routers` for a project, with
/// Horizon's admin options (SNAT, AZ hints, distributed, HA). Options left `None` are not sent,
/// so Neutron's defaults apply; SNAT needs an external network.
pub(super) async fn admin_router_create(x: &mut Exec<'_>, spec: &AdminRouterSpec) -> R<()> {
    use openstack_sdk::api::network::v2::router::create as m;
    let mut b = m::RouterBuilder::default();
    b.name(spec.name.trim())
        .admin_state_up(spec.admin_up)
        .tenant_id(non_empty(&spec.project_id, "project")?);
    if let Some(net) = opt(&spec.external_network_id) {
        let mut gw = m::ExternalGatewayInfoBuilder::default();
        gw.network_id(net.to_string());
        if let Some(snat) = spec.enable_snat {
            gw.enable_snat(snat);
        }
        b.external_gateway_info(Some(gw.build().map_err(build_err)?));
    } else if spec.enable_snat.is_some() {
        return Err(AppError::invalid("SNAT needs an external network"));
    }
    if !spec.az_hints.is_empty() {
        b.availability_zone_hints(spec.az_hints.iter().map(|z| Cow::from(z.as_str())).collect::<Vec<_>>());
    }
    if let Some(d) = spec.distributed {
        b.distributed(Some(d));
    }
    if let Some(ha) = spec.ha {
        b.ha(Some(ha));
    }
    let body = b.build().map_err(build_err)?;
    x.create(m::Request::builder().router(body).build().map_err(build_err)?, "router")
        .await?;
    Ok(())
}

/// `OsAction::RouterUpdate`: `PUT /v2.0/routers/{id}`. `snat` carries the current gateway
/// network because `enable_snat` lives in `external_gateway_info`, which is replaced whole.
/// `distributed` and `ha` are admin-only and sent only when `Some`.
pub(super) async fn router_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    admin_up: &bool,
    snat: &Option<(String, bool)>,
    distributed: &Option<bool>,
    ha: &Option<bool>,
) -> R<()> {
    use openstack_sdk::api::network::v2::router::set as m;
    let mut b = m::RouterBuilder::default();
    b.name(name.trim()).admin_state_up(*admin_up);
    if let Some(d) = distributed {
        b.distributed(Some(*d));
    }
    if let Some(h) = ha {
        b.ha(Some(*h));
    }
    if let Some((network_id, on)) = snat {
        let gw = m::ExternalGatewayInfoBuilder::default()
            .network_id(network_id.as_str())
            .enable_snat(*on)
            .build()
            .map_err(build_err)?;
        b.external_gateway_info(Some(gw));
    }
    let body = b.build().map_err(build_err)?;
    x.send(m::Request::builder().id(id).router(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `OsAction::RouterSetGateway`: `PUT /v2.0/routers/{id}` with
/// `external_gateway_info.network_id`.
pub(super) async fn router_set_gateway(x: &mut Exec<'_>, id: &str, network_id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::router::set as m;
    let gw = m::ExternalGatewayInfoBuilder::default()
        .network_id(network_id)
        .build()
        .map_err(build_err)?;
    let body = m::RouterBuilder::default()
        .external_gateway_info(Some(gw))
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).router(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `OsAction::RouterClearGateway`: `PUT /v2.0/routers/{id}` with
/// `"external_gateway_info": null`.
pub(super) async fn router_clear_gateway(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::router::set as m;
    let body = m::RouterBuilder::default()
        .external_gateway_info(None::<m::ExternalGatewayInfo>)
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).router(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `OsAction::RouterAddInterface`: `PUT /v2.0/routers/{id}/add_router_interface` on a
/// subnet (Neutron creates the port with the subnet's gateway IP).
///
/// With an `ip_address`, as Horizon: `POST /v2.0/ports` on the subnet's network with that fixed
/// IP, then the same `PUT` with the new `port_id`. Adding by subnet alone would take the
/// gateway IP, which another router on the subnet already holds (409). A port left behind by a
/// failed `PUT` is deleted again.
pub(super) async fn router_add_interface(
    x: &mut Exec<'_>,
    id: &str,
    subnet_id: &str,
    ip_address: &Option<String>,
    network_id: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::network::v2::router::add_router_interface as m;
    let Some(ip) = opt(ip_address) else {
        x.send(
            m::Request::builder()
                .id(id)
                .subnet_id(subnet_id)
                .build()
                .map_err(build_err)?,
        )
        .await
        .map_err(|e| {
            in_use(
                e,
                "the subnet's gateway IP is already used by another router: enter an IP Address",
            )
        })?;
        return Ok(());
    };
    ip.parse::<std::net::IpAddr>()
        .map_err(|_| AppError::invalid(format!("not an IP address: {ip}")))?;
    let net = opt(network_id).ok_or_else(|| AppError::invalid("the subnet's network is needed to pick an address"))?;
    let port = {
        use openstack_sdk::api::network::v2::port::create as p;
        let fixed = p::FixedIpsBuilder::default()
            .subnet_id(subnet_id)
            .ip_address(ip.to_string())
            .build()
            .map_err(build_err)?;
        let body = p::PortBuilder::default()
            .network_id(net)
            .fixed_ips(vec![fixed])
            .build()
            .map_err(build_err)?;
        x.create(p::Request::builder().port(body).build().map_err(build_err)?, "port")
            .await?
    };
    let added = x
        .send(
            m::Request::builder()
                .id(id)
                .port_id(port.as_str())
                .build()
                .map_err(build_err)?,
        )
        .await;
    if added.is_err() {
        // The port would stay behind, holding the address; its own failure adds nothing to report.
        let _ = super::network::port_delete(x, &port).await;
    }
    added.map_err(|e| in_use(e, "the IP address is already in use in this subnet"))
}

/// Neutron answers 409 (`IpAddressInUse`) when the address an interface needs is taken; the
/// generic "OpenStack rejected the request" hides that, so name the cause. Other errors pass.
fn in_use(e: AppError, why: &str) -> AppError {
    if e.kind == ErrorKind::OpenStack(409) {
        AppError {
            detail: format!("Add Interface: {why}"),
            ..e
        }
    } else {
        e
    }
}

/// `OsAction::RouterRemoveInterface`: `PUT /v2.0/routers/{id}/remove_router_interface` by
/// port ID.
pub(super) async fn router_remove_interface(x: &mut Exec<'_>, id: &str, port_id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::router::remove_router_interface as m;
    x.send(
        m::Request::builder()
            .id(id)
            .port_id(port_id)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `OsAction::RouterAddRoute`: `PUT /v2.0/routers/{id}/add_extraroutes` (extension
/// `extraroute-atomic`), after `validate_route`. Without that extension the UI sends
/// `RouterSetRoutes` instead.
pub(super) async fn router_add_route(x: &mut Exec<'_>, id: &str, destination: &str, nexthop: &str) -> R<()> {
    use openstack_sdk::api::network::v2::router::add_extraroutes as m;
    let (d, h) = validate_route(destination, nexthop).map_err(AppError::invalid)?;
    let route = m::RoutesBuilder::default()
        .destination(d)
        .nexthop(h)
        .build()
        .map_err(build_err)?;
    let body = m::RouterBuilder::default()
        .routes(vec![route])
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).router(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `OsAction::RouterRemoveRoute`: `PUT /v2.0/routers/{id}/remove_extraroutes`
/// (`extraroute-atomic`). The route is sent as listed, unvalidated, so an odd existing one can
/// still be removed.
pub(super) async fn router_remove_route(x: &mut Exec<'_>, id: &str, destination: &str, nexthop: &str) -> R<()> {
    use openstack_sdk::api::network::v2::router::remove_extraroutes as m;
    let route = m::RoutesBuilder::default()
        .destination(destination)
        .nexthop(nexthop)
        .build()
        .map_err(build_err)?;
    let body = m::RouterBuilder::default()
        .routes(vec![route])
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).router(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `OsAction::RouterSetRoutes`: `PUT /v2.0/routers/{id}` replacing the whole `routes` list:
/// the fallback for clouds with plain `extraroute` only (it races with concurrent edits).
pub(super) async fn router_set_routes(x: &mut Exec<'_>, id: &str, routes: &[(String, String)]) -> R<()> {
    use openstack_sdk::api::network::v2::router::set as m;
    let list = routes
        .iter()
        .map(|(d, n)| {
            m::RoutesBuilder::default()
                .destination(d.as_str())
                .nexthop(n.as_str())
                .build()
                .map_err(build_err)
        })
        .collect::<R<Vec<_>>>()?;
    let body = m::RouterBuilder::default().routes(list).build().map_err(build_err)?;
    x.send(m::Request::builder().id(id).router(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `OsAction::RouterDelete`: `DELETE /v2.0/routers/{id}`.
pub(super) async fn router_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::router::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[tokio::test]
    async fn admin_router_sends_owner_gateway_and_horizon_options() {
        let spec = |gw: Option<&str>, snat: Option<bool>| AdminRouterSpec {
            name: id(" edge "),
            project_id: id("p2"),
            admin_up: true,
            external_network_id: gw.map(Into::into),
            enable_snat: snat,
            az_hints: vec![id("az1")],
            distributed: Some(false),
            ha: Some(true),
        };
        let r = dry(OsAction::AdminRouterCreate(Box::new(spec(Some("ext"), Some(false))))).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/routers"));
        assert_eq!(
            body(&r[0]),
            json!({"router": {"name": "edge", "admin_state_up": true, "tenant_id": "p2",
                "external_gateway_info": {"network_id": "ext", "enable_snat": false},
                "availability_zone_hints": ["az1"], "distributed": false, "ha": true}})
        );
        let none = AdminRouterSpec {
            az_hints: vec![],
            distributed: None,
            ha: None,
            ..spec(None, None)
        };
        assert_eq!(
            body(&dry(OsAction::AdminRouterCreate(Box::new(none))).await[0]),
            json!({"router": {"name": "edge", "admin_state_up": true, "tenant_id": "p2"}}),
            "server defaults: nothing sent"
        );
        assert!(
            dry_err(OsAction::AdminRouterCreate(Box::new(spec(None, Some(true))))).await,
            "SNAT without a gateway"
        );
        assert!(
            dry_err(OsAction::AdminRouterCreate(Box::new(AdminRouterSpec {
                project_id: id(""),
                ..spec(None, None)
            })))
            .await,
            "an owner is required"
        );
    }

    #[tokio::test]
    async fn ports_routers_and_interfaces() {
        let r = dry(OsAction::PortCreate {
            network_id: id("n"),
            name: id("p"),
            admin_up: true,
            fixed_ip: Some(id("10.0.0.5")),
        })
        .await;
        assert_eq!(body(&r[0])["port"]["fixed_ips"], json!([{"ip_address": "10.0.0.5"}]));
        let r = dry(OsAction::RouterCreate {
            name: id("r"),
            admin_up: true,
            external_network_id: Some(id("ext")),
            qos_policy_id: None,
        })
        .await;
        assert_eq!(body(&r[0])["router"]["external_gateway_info"]["network_id"], "ext");
        let r = dry(OsAction::RouterUpdate {
            id: id("r"),
            name: id(" gw "),
            admin_up: true,
            snat: None,
            distributed: None,
            ha: None,
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/routers/r"));
        assert_eq!(
            body(&r[0]),
            json!({"router": {"name": "gw", "admin_state_up": true}}),
            "rename keeps the gateway"
        );
        let r = dry(OsAction::RouterUpdate {
            id: id("r"),
            name: id("gw"),
            admin_up: false,
            snat: Some((id("ext"), false)),
            distributed: None,
            ha: None,
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"router": {"name": "gw", "admin_state_up": false,
                "external_gateway_info": {"network_id": "ext", "enable_snat": false}}})
        );
        // Admin Edit Router: type and HA mode only when they change.
        let r = dry(OsAction::RouterUpdate {
            id: id("r"),
            name: id("gw"),
            admin_up: false,
            snat: None,
            distributed: Some(true),
            ha: Some(false),
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"router": {"name": "gw", "admin_state_up": false, "distributed": true, "ha": false}})
        );
        let r = dry(OsAction::RouterClearGateway { id: id("r") }).await;
        assert_eq!(body(&r[0]), json!({"router": {"external_gateway_info": null}}));
        let r = dry(OsAction::RouterAddInterface {
            id: id("r"),
            subnet_id: id("s"),
            ip_address: None,
            network_id: None,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/routers/r/add_router_interface")
        );
        assert_eq!(body(&r[0]), json!({"subnet_id": "s"}));
        assert_eq!(r.len(), 1, "without an IP there is no port to create");
        // With an IP (a second router on the subnet): the port first, then it is added by ID.
        let with_ip = |ip: &str, network: Option<&str>| OsAction::RouterAddInterface {
            id: id("r"),
            subnet_id: id("s"),
            ip_address: Some(id(ip)),
            network_id: network.map(id),
        };
        let r = dry(with_ip(" 10.0.0.9 ", Some("n"))).await;
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/ports"));
        assert_eq!(
            body(&r[0]),
            json!({"port": {"network_id": "n", "fixed_ips": [{"subnet_id": "s", "ip_address": "10.0.0.9"}]}})
        );
        assert_eq!(r[1].path, "/routers/r/add_router_interface");
        assert_eq!(body(&r[1]), json!({"port_id": "<new port id>"}));
        assert!(
            dry_err(with_ip("not-an-ip", Some("n"))).await,
            "an address that is not one"
        );
        assert!(
            dry_err(with_ip("10.0.0.9", None)).await,
            "the network is needed for the port"
        );
        let r = dry(OsAction::RouterRemoveInterface {
            id: id("r"),
            port_id: id("p"),
        })
        .await;
        assert_eq!(body(&r[0]), json!({"port_id": "p"}));
    }

    #[tokio::test]
    async fn static_routes_are_added_and_removed_one_at_a_time() {
        let r = dry(OsAction::RouterAddRoute {
            id: id("r"),
            destination: id(" 10.9.0.0/16"),
            nexthop: id("10.0.0.1"),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/routers/r/add_extraroutes")
        );
        assert_eq!(
            body(&r[0]),
            json!({"router": {"routes": [{"destination": "10.9.0.0/16", "nexthop": "10.0.0.1"}]}})
        );
        let r = dry(OsAction::RouterRemoveRoute {
            id: id("r"),
            destination: id("10.9.0.0/16"),
            nexthop: id("10.0.0.1"),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/routers/r/remove_extraroutes")
        );
        assert_eq!(
            body(&r[0]),
            json!({"router": {"routes": [{"destination": "10.9.0.0/16", "nexthop": "10.0.0.1"}]}})
        );
    }

    #[tokio::test]
    async fn router_gateway_and_floating_ip_take_a_qos_policy() {
        let r = dry(OsAction::RouterCreate {
            name: id("r"),
            admin_up: true,
            external_network_id: Some(id("ext")),
            qos_policy_id: Some(id("q1")),
        })
        .await;
        assert_eq!(
            body(&r[0])["router"]["external_gateway_info"],
            json!({"network_id": "ext", "qos_policy_id": "q1"})
        );
        assert!(
            dry_err(OsAction::RouterCreate {
                name: id("r"),
                admin_up: true,
                external_network_id: None,
                qos_policy_id: Some(id("q1"))
            })
            .await
        );
        let r = dry(OsAction::FloatingIpAllocate {
            network_id: id("ext"),
            description: None,
            port_id: None,
            qos_policy_id: Some(id("q1")),
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"floatingip": {"floating_network_id": "ext", "qos_policy_id": "q1"}})
        );
        let r = dry(OsAction::FloatingIpAllocate {
            network_id: id("ext"),
            description: None,
            port_id: None,
            qos_policy_id: None,
        })
        .await;
        assert!(
            body(&r[0])["floatingip"].get("qos_policy_id").is_none(),
            "not sent unless chosen"
        );
    }

    #[tokio::test]
    async fn router_routes_as_a_whole_list() {
        let r = dry(OsAction::RouterSetRoutes {
            id: id("r"),
            routes: vec![(id("10.9.0.0/16"), id("10.0.0.5"))],
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/routers/r"));
        assert_eq!(
            body(&r[0]),
            json!({"router": {"routes": [{"destination": "10.9.0.0/16", "nexthop": "10.0.0.5"}]}})
        );
        let r = dry(OsAction::RouterSetRoutes {
            id: id("r"),
            routes: vec![],
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"router": {"routes": []}}),
            "removing the last route sends an empty list"
        );
    }
}

#[cfg(test)]
mod in_use_tests {
    use super::*;

    #[test]
    fn a_409_names_the_cause_and_other_errors_pass() {
        let conflict = AppError::new(ErrorKind::OpenStack(409), "OpenStack rejected the request");
        let e = in_use(conflict, "the IP address is already in use in this subnet");
        assert_eq!(e.kind, ErrorKind::OpenStack(409));
        assert!(e.detail.contains("already in use"), "{}", e.detail);

        let other = AppError::new(ErrorKind::OpenStack(500), "boom");
        assert_eq!(in_use(other.clone(), "x"), other, "only a 409 is reworded");
    }
}
