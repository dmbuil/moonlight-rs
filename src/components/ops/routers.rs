//! Routers, their interfaces and static routes.

use super::*;

/// Optional *QoS Policy* select (the project's and shared policies).
pub(super) fn qos_field(lang: Lang) -> Field {
    Field::select("qos", t(lang, K::QosPolicy), Source::Set(ChoiceSet::QosPolicies))
}

/// *Create Router*: name, external network, SNAT.
pub fn create_router_dialog(lang: Lang) -> Dialog {
    let mut fields = vec![
        Field::text("name", t(lang, K::Name)),
        Field::check("admin_up", t(lang, K::EnableAdminState), true),
        Field::select(
            "external",
            t(lang, K::ExternalNetwork),
            Source::Set(ChoiceSet::ExternalNetworks),
        ),
    ];
    // The gateway's policy (`qos` plugin and `qos-gateway-ip`); only with a gateway.
    let gateway: fn(&Vals) -> bool = |v| v.get("external").is_some_and(|e| !e.trim().is_empty());
    if needs::QOS_GATEWAY.met(&caps()) {
        fields.push(qos_field(lang).help(t(lang, K::GatewayQosHelp)).show_if(gateway));
    }
    Dialog::form(t(lang, K::CreateRouter), t(lang, K::CreateRouter), fields, move |v| {
        let external = v.opt("external");
        Ok(OsAction::RouterCreate {
            name: v.get("name"),
            admin_up: v.flag("admin_up"),
            qos_policy_id: v.opt("qos").filter(|_| external.is_some()),
            external_network_id: external,
        })
    })
}

/// *Add Interface* to a router: a subnet, with an optional IP address.
pub fn add_interface_dialog(lang: Lang, router_id: &str) -> Dialog {
    let r = router_id.to_string();
    Dialog::form(
        t(lang, K::AddInterface),
        t(lang, K::AddInterface),
        vec![Field::select("subnet", t(lang, K::Subnet), Source::Set(ChoiceSet::Subnets)).required()],
        move |v| {
            Ok(OsAction::RouterAddInterface {
                id: r.clone(),
                subnet_id: req(v, "subnet", K::Subnet, lang)?,
            })
        },
    )
}

/// A router's menu: set or clear the gateway, edit, add interface, delete.
pub fn router_items(lang: Lang, r: &RouterRow) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if r.external_network.is_some() {
        let (i, n) = (r.id.clone(), r.name.clone());
        items.push(MenuItem::danger(t(lang, K::ClearGateway), move || {
            Dialog::confirm(
                lang,
                t(lang, K::ClearGateway),
                n.clone(),
                OsAction::RouterClearGateway { id: i.clone() },
            )
        }));
    } else {
        let i = r.id.clone();
        items.push(MenuItem::dialog(t(lang, K::SetGateway), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::SetGateway),
                t(lang, K::SetGateway),
                vec![
                    Field::select(
                        "external",
                        t(lang, K::ExternalNetwork),
                        Source::Set(ChoiceSet::ExternalNetworks),
                    )
                    .required(),
                ],
                move |v| {
                    Ok(OsAction::RouterSetGateway {
                        id: i.clone(),
                        network_id: req(v, "external", K::ExternalNetwork, lang)?,
                    })
                },
            )
        }));
    }
    items.push(edit_router_item(lang, r, false));
    let i = r.id.clone();
    items.push(MenuItem::dialog(t(lang, K::AddInterface), move || {
        add_interface_dialog(lang, &i)
    }));
    items.push(delete_router_item(lang, r));
    items
}

/// *Edit Router*: name, admin state and, with a gateway, SNAT. Admin's
/// (`admin`) adds Horizon's *Router Type* and *High Availability*
/// where Neutron has `dvr` / `l3-ha` and shows the router's current value.
fn edit_router_item(lang: Lang, r: &RouterRow, admin: bool) -> MenuItem {
    let (i, n, up) = (r.id.clone(), r.name.clone(), r.admin_up);
    // SNAT is part of the gateway: only with one, and when the token's roles
    // may change it.
    let gw = r.external_network_id.clone().filter(|_| needs::SNAT.met(&caps()));
    let snat = r.snat.unwrap_or(true);
    let dvr = r.distributed.filter(|_| admin && needs::DVR.met(&caps()));
    let ha = r.ha.filter(|_| admin && needs::L3_HA.met(&caps()));
    MenuItem::dialog(t(lang, K::EditRouter), move || {
        let (i, gw) = (i.clone(), gw.clone());
        let mut fields = vec![
            Field::text("name", t(lang, K::Name)).value(n.clone()),
            Field::check("admin_up", t(lang, K::EnableAdminState), up),
        ];
        if gw.is_some() {
            fields.push(Field::check("snat", t(lang, K::EnableSnat), snat));
        }
        let down_help = t(lang, K::RouterChangeWhileDown);
        if let Some(d) = dvr {
            let kinds = options(
                &[("distributed", K::Distributed), ("centralized", K::Centralized)],
                lang,
            );
            fields.push(
                Field::select("mode", t(lang, K::RouterType), kinds)
                    .value(if d { "distributed" } else { "centralized" })
                    .help(down_help),
            );
        }
        if let Some(h) = ha {
            fields.push(Field::check("ha", t(lang, K::HighAvailability), h).help(down_help));
        }
        // What differs from the router: `None` = unchanged (or not offered).
        let changes = move |v: &Vals| {
            let is = |k: &str, want: &str| v.get(k).is_some_and(|x| x.trim() == want);
            (
                dvr.map(|_| is("mode", "distributed")).filter(|now| Some(*now) != dvr),
                ha.map(|_| is("ha", "true")).filter(|now| Some(*now) != ha),
            )
        };
        Dialog::form(t(lang, K::EditRouter), t(lang, K::Save), fields, move |v| {
            // Only a change goes in the request, so a rename leaves the gateway alone.
            let snat = gw
                .clone()
                .map(|net| (net, v.flag("snat")))
                .filter(|(_, on)| *on != snat);
            let (distributed, ha) = changes(&v.0);
            Ok(OsAction::RouterUpdate {
                id: i.clone(),
                name: v.get("name"),
                admin_up: v.flag("admin_up"),
                snat,
                distributed,
                ha,
            })
        })
        // Neutron refuses them on a router that is up ("set router admin_state_up
        // to False prior to upgrade"), whatever this request sets.
        .check(move |v, _| {
            if !up {
                return vec![];
            }
            let (d, h) = changes(v);
            let msg = t(lang, K::RouterSetDownFirst).to_string();
            let mut out = vec![];
            if d.is_some() {
                out.push(("mode", msg.clone()));
            }
            if h.is_some() {
                out.push(("ha", msg));
            }
            out
        })
    })
}

/// *Delete Router* (`OsAction::RouterDelete`).
fn delete_router_item(lang: Lang, r: &RouterRow) -> MenuItem {
    let (i, n) = (r.id.clone(), r.name.clone());
    MenuItem::danger(t(lang, K::DeleteRouter), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteRouter),
            n.clone(),
            OsAction::RouterDelete { id: i.clone() },
        )
    })
}

/// Admin › Routers' row menu, Horizon's: *Edit Router* and *Delete Router*. No
/// gateway or interface items: their subnets would be the session project's.
pub fn admin_router_items(lang: Lang, r: &RouterRow) -> Vec<MenuItem> {
    vec![edit_router_item(lang, r, true), delete_router_item(lang, r)]
}

/// Admin › Routers' *Create Router*, Horizon's admin form: the
/// owner project, then Project's fields, SNAT with a gateway, and *Router Type*,
/// *High Availability Mode* and *Availability Zone Hints* where Neutron has
/// them. *Use Server Default* leaves a field out of the request.
pub fn admin_create_router_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let caps = caps();
    let gateway: fn(&Vals) -> bool = |v| v.get("external").is_some_and(|e| !e.trim().is_empty());
    let mut fields = vec![
        Field::text("name", tr(K::Name)),
        Field::select("project_id", tr(K::Project), Source::Set(ChoiceSet::AllProjects))
            .required()
            .empty_label(tr(K::SelectProject)),
        Field::check("admin_up", tr(K::EnableAdminState), true),
        Field::select(
            "external",
            tr(K::ExternalNetwork),
            Source::Set(ChoiceSet::ExternalNetworks),
        ),
    ];
    if needs::SNAT.met(&caps) {
        // Horizon's default: SNAT on.
        fields.push(Field::check("snat", tr(K::EnableSnat), true).show_if(gateway));
    }
    let tri = |on: K, off: K| options(&[("default", K::UseServerDefault), ("on", on), ("off", off)], lang);
    if needs::DVR.met(&caps) {
        fields.push(Field::select("mode", tr(K::RouterType), tri(K::Distributed, K::Centralized)).value("default"));
    }
    if needs::L3_HA.met(&caps) {
        fields.push(Field::select("ha", tr(K::HaMode), tri(K::HaEnable, K::HaDisable)).value("default"));
    }
    if needs::ROUTER_AZ.met(&caps) {
        fields.push(
            Field::multi("az_hints", tr(K::AzHints), Source::Set(ChoiceSet::RouterZones))
                .help(tr(K::RouterAzHintsHelp))
                .hide_when_empty(),
        );
    }
    // `default` (or a hidden field) sends nothing.
    let tri_value = |v: &Values, k: &str| match v.get(k).as_str() {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    };
    Dialog::form(tr(K::CreateRouter), tr(K::CreateRouter), fields, move |v| {
        let external = v.opt("external");
        Ok(OsAction::AdminRouterCreate(Box::new(AdminRouterSpec {
            name: v.get("name"),
            project_id: req(v, "project_id", K::Project, lang)?,
            admin_up: v.flag("admin_up"),
            // A hidden *Enable SNAT* (no gateway, or no `ext-gw-mode`) is empty: not sent.
            enable_snat: v.opt("snat").filter(|_| external.is_some()).map(|s| s == "true"),
            external_network_id: external,
            az_hints: v.list("az_hints"),
            distributed: tri_value(v, "mode"),
            ha: tri_value(v, "ha"),
        })))
    })
}

/// `current`: the router's routes when only plain `extraroute` exists, so the
/// new route is sent with the whole list; `None` uses the atomic call.
pub fn add_route_dialog(lang: Lang, router_id: &str, current: Option<Vec<(String, String)>>) -> Dialog {
    let r = router_id.to_string();
    Dialog::form(
        t(lang, K::AddStaticRoute),
        t(lang, K::AddStaticRoute),
        vec![
            Field::text("destination", t(lang, K::Destination))
                .required()
                .help("10.9.0.0/16"),
            Field::text("nexthop", t(lang, K::NextHop))
                .required()
                .help(t(lang, K::NextHopHelp)),
        ],
        move |v| {
            let (destination, nexthop) = validate_route(
                &req(v, "destination", K::Destination, lang)?,
                &req(v, "nexthop", K::NextHop, lang)?,
            )?;
            Ok(match &current {
                None => OsAction::RouterAddRoute {
                    id: r.clone(),
                    destination,
                    nexthop,
                },
                Some(list) => {
                    let mut routes = list.clone();
                    if !routes.contains(&(destination.clone(), nexthop.clone())) {
                        routes.push((destination, nexthop));
                    }
                    OsAction::RouterSetRoutes { id: r.clone(), routes }
                }
            })
        },
    )
}

/// `current` as in [`add_route_dialog`].
pub fn route_items(
    lang: Lang,
    router_id: &str,
    route: &(String, String),
    current: Option<&[(String, String)]>,
) -> Vec<MenuItem> {
    let (r, (dest, hop)) = (router_id.to_string(), route.clone());
    let action = match current {
        None => OsAction::RouterRemoveRoute {
            id: r,
            destination: dest.clone(),
            nexthop: hop.clone(),
        },
        Some(list) => OsAction::RouterSetRoutes {
            id: r,
            routes: list.iter().filter(|x| **x != route.clone()).cloned().collect(),
        },
    };
    vec![MenuItem::danger(t(lang, K::DeleteStaticRoute), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteStaticRoute),
            format!("{dest} → {hop}"),
            action.clone(),
        )
    })]
}

/// A router interface's menu (*Delete Interface*).
pub fn interface_items(lang: Lang, router_id: &str, p: &PortRow) -> Vec<MenuItem> {
    let (r, pid, label) = (router_id.to_string(), p.id.clone(), p.fixed_ips.join(", "));
    let is_gw = p.device_owner.as_deref() == Some("network:router_gateway");
    if is_gw {
        return vec![];
    }
    vec![MenuItem::danger(t(lang, K::DeleteInterface), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteInterface),
            label.clone(),
            OsAction::RouterRemoveInterface {
                id: r.clone(),
                port_id: pid.clone(),
            },
        )
    })]
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[test]
    fn admin_create_router_sends_server_defaults_as_nothing() {
        let d = admin_create_router_dialog(Lang::EnUs);
        let names: Vec<&str> = d.fields.iter().map(|f| f.name).collect();
        assert_eq!(
            names,
            [
                "name",
                "project_id",
                "admin_up",
                "external",
                "snat",
                "mode",
                "ha",
                "az_hints"
            ],
            "unknown capabilities show every field"
        );
        let a = (d.build)(&vals(&[
            ("name", "edge"),
            ("project_id", "p2"),
            ("admin_up", "true"),
            ("external", "ext"),
            ("snat", "false"),
            ("mode", "off"),
            ("ha", "default"),
            ("az_hints", "az1,az2"),
        ]))
        .unwrap();
        let OsAction::AdminRouterCreate(spec) = a else { panic!() };
        assert_eq!(
            *spec,
            AdminRouterSpec {
                name: "edge".into(),
                project_id: "p2".into(),
                admin_up: true,
                external_network_id: Some("ext".into()),
                enable_snat: Some(false),
                az_hints: vec!["az1".into(), "az2".into()],
                distributed: Some(false),
                ha: None,
            }
        );
        // No gateway: *Enable SNAT* is hidden (empty) and not sent.
        let OsAction::AdminRouterCreate(spec) = (d.build)(&vals(&[("project_id", "p2"), ("mode", "on")])).unwrap()
        else {
            panic!()
        };
        assert_eq!(
            (spec.enable_snat, spec.external_network_id, spec.distributed),
            (None, None, Some(true))
        );
        assert!(
            (d.build)(&vals(&[("name", "edge")])).is_err(),
            "the project is required"
        );
    }

    #[test]
    fn admin_router_menu_is_horizons_edit_and_delete() {
        let r = RouterRow {
            id: "r1".into(),
            name: "edge".into(),
            status: "ACTIVE".into(),
            external_network: Some("public".into()),
            external_network_id: Some("ext".into()),
            snat: Some(true),
            admin_up: true,
            availability_zones: vec![],
            distributed: None,
            ha: None,
        };
        let labels = |items: Vec<MenuItem>| items.into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(
            labels(admin_router_items(Lang::EnUs, &r)),
            ["Edit Router", "Delete Router"],
            "no gateway or interface items: their choices are the session project's"
        );
        assert_eq!(
            labels(router_items(Lang::EnUs, &r)),
            ["Clear Gateway", "Edit Router", "Add Interface", "Delete Router"],
            "Project's menu unchanged"
        );
    }

    #[test]
    fn admin_edit_router_changes_type_and_ha_only_while_down() {
        let r = |up: bool| RouterRow {
            id: "r1".into(),
            name: "edge".into(),
            status: "ACTIVE".into(),
            external_network: None,
            external_network_id: None,
            snat: None,
            admin_up: up,
            availability_zones: vec![],
            distributed: Some(false),
            ha: Some(true),
        };
        let names = |d: &Dialog| d.fields.iter().map(|f| f.name).collect::<Vec<_>>();
        assert_eq!(
            names(&dialog_of(&router_items(Lang::EnUs, &r(true)), "Edit Router")),
            ["name", "admin_up"],
            "Project's has neither"
        );
        let d = dialog_of(&admin_router_items(Lang::EnUs, &r(true)), "Edit Router");
        assert_eq!(names(&d), ["name", "admin_up", "mode", "ha"]);
        let v = |mode: &str, ha: &str| vals(&[("name", "edge"), ("admin_up", "false"), ("mode", mode), ("ha", ha)]).0;
        let check = d.check.clone().unwrap();
        assert!(
            check(&v("centralized", "true"), &Sets::default()).is_empty(),
            "nothing changed: fine while up"
        );
        let errs = check(&v("distributed", "false"), &Sets::default());
        assert_eq!(
            errs.iter().map(|e| e.0).collect::<Vec<_>>(),
            ["mode", "ha"],
            "the router is up, whatever the form says"
        );
        let down = dialog_of(&admin_router_items(Lang::EnUs, &r(false)), "Edit Router");
        assert!(down.check.clone().unwrap()(&v("distributed", "false"), &Sets::default()).is_empty());
        let a = (down.build)(&Values(v("distributed", "true"))).unwrap();
        assert!(
            matches!(
                a,
                OsAction::RouterUpdate {
                    distributed: Some(true),
                    ha: None,
                    ..
                }
            ),
            "only what changed: {a:?}"
        );
    }

    #[test]
    fn routes_use_the_atomic_call_or_the_whole_list() {
        let cur = vec![("10.1.0.0/16".to_string(), "10.0.0.1".to_string())];
        let new = vals(&[("destination", "10.2.0.0/16"), ("nexthop", "10.0.0.2")]);
        let a = (add_route_dialog(Lang::EnUs, "r", None).build)(&new).unwrap();
        assert!(matches!(a, OsAction::RouterAddRoute { .. }));
        let OsAction::RouterSetRoutes { routes, .. } =
            (add_route_dialog(Lang::EnUs, "r", Some(cur.clone())).build)(&new).unwrap()
        else {
            panic!()
        };
        assert_eq!(routes.len(), 2);
        let items = route_items(Lang::EnUs, "r", &cur[0], Some(&cur));
        let crate::components::dialog::ItemKind::Dialog(open) = &items[0].kind else {
            panic!()
        };
        assert_eq!(
            (open().build)(&vals(&[])).unwrap(),
            OsAction::RouterSetRoutes {
                id: "r".into(),
                routes: vec![]
            }
        );
    }
}
