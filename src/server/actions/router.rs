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
pub(super) async fn router_add_interface(x: &mut Exec<'_>, id: &str, subnet_id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::router::add_router_interface as m;
    x.send(
        m::Request::builder()
            .id(id)
            .subnet_id(subnet_id)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
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
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/routers/r/add_router_interface")
        );
        assert_eq!(body(&r[0]), json!({"subnet_id": "s"}));
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
