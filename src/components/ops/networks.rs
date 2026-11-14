//! Networks, subnets, ports and RBAC policies.

use super::*;

/// Subnet fields, prefilled from `[network]` (DNS servers, IPv6 on or off).
fn subnet_fields(
    lang: Lang,
    when: Option<(&'static str, &'static str)>,
    net: &crate::prefs::NetworkPrefs,
) -> Vec<Field> {
    let tr = |k| t(lang, k).to_string();
    let versions: &[&str] = if net.enable_ipv6 { &["4", "6"] } else { &["4"] };
    let w = |f: Field| match when {
        Some((a, b)) => f.when(a, b),
        None => f,
    };
    vec![
        w(Field::text("subnet_name", tr(K::SubnetName))),
        w(Field::text("cidr", tr(K::NetworkAddress))
            .help("192.168.0.0/24")
            .required()),
        w(Field::select("ip_version", tr(K::IpVersion), options_raw(versions))
            .value("4")
            .required()),
        w(Field::text("gateway", tr(K::GatewayIp)).help(tr(K::GatewayAuto))),
        w(Field::check("no_gateway", tr(K::DisableGateway), false)),
        w(Field::check("dhcp", tr(K::EnableDhcp), true)),
        // Horizon shows the mode only for IPv6 subnets.
        w(Field::select("ipv6_modes", tr(K::Ipv6Modes), ipv6_mode_options(lang))
            .value(IPV6_MODES[0])
            .show_if(|v| v.get("ip_version").map(String::as_str) == Some("6"))),
        w(Field::area("pools", tr(K::AllocationPools)).help(tr(K::PoolsHelp))),
        w(Field::area("dns", tr(K::DnsNameServers))
            .help(tr(K::OnePerLine))
            .value(net.default_dns_nameservers.join("\n"))),
        w(Field::area("host_routes", tr(K::HostRoutes)).help(tr(K::HostRoutesHelp))),
    ]
}

/// Horizon's five *IPv6 Address Configuration Mode* choices, translated.
fn ipv6_mode_options(lang: Lang) -> Source {
    let keys = [
        K::Ipv6ModeNone,
        K::Ipv6ModeSlaacRouter,
        K::Ipv6ModeSlaacExternal,
        K::Ipv6ModeStateful,
        K::Ipv6ModeStateless,
    ];
    Source::Static(
        IPV6_MODES
            .iter()
            .zip(keys)
            .map(|(v, k)| (v.to_string(), t(lang, k).to_string()))
            .collect(),
    )
}

/// `[network] allowed_private_subnet_cidr` / `enable_ipv6`, before submitting
/// (the server checks again). Skipped when the network dialog creates no subnet.
fn subnet_check(
    lang: Lang,
    net: crate::prefs::NetworkPrefs,
) -> impl Fn(&Vals, &Sets) -> Vec<(&'static str, String)> + Send + Sync + 'static {
    move |v, _| {
        let cidr = v.get("cidr").map(|c| c.trim()).unwrap_or_default();
        if v.get("with_subnet").map(String::as_str) == Some("false") || cidr.is_empty() {
            return vec![];
        }
        match net.check_subnet(cidr) {
            Ok(()) => vec![],
            Err(issue) => vec![("cidr", crate::i18n::subnet_issue(lang, &issue))],
        }
    }
}

/// The subnet of [`subnet_fields`] (*Create Network*, Admin *Create Network* and *Create
/// Subnet*): allocation pools and host routes one per line, `a,b`; a line that does not parse
/// names its field.
fn subnet_spec(v: &Values, lang: Lang) -> Result<SubnetSpec, String> {
    let pools = v
        .lines("pools")
        .into_iter()
        .map(|l| match l.split_once(',') {
            Some((a, z)) => Ok((a.trim().to_string(), z.trim().to_string())),
            None => Err(format!("{}: \"{l}\"", t(lang, K::AllocationPools))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let ip_version = if v.get("ip_version") == "6" { 6 } else { 4 };
    let (ipv6_ra_mode, ipv6_address_mode) = if ip_version == 6 {
        ipv6_modes(&v.opt("ipv6_modes").unwrap_or_else(|| IPV6_MODES[0].into()))?
    } else {
        (None, None)
    };
    let host_routes = v
        .lines("host_routes")
        .into_iter()
        .map(|l| match l.split_once(',') {
            Some((dest, hop)) => validate_route(dest, hop).map_err(|e| format!("{}: {e}", t(lang, K::HostRoutes))),
            None => Err(format!("{}: \"{l}\"", t(lang, K::HostRoutes))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SubnetSpec {
        name: v.get("subnet_name"),
        cidr: req(v, "cidr", K::NetworkAddress, lang)?,
        ip_version,
        gateway_ip: if v.flag("no_gateway") {
            Some(String::new())
        } else {
            v.opt("gateway")
        },
        enable_dhcp: v.flag("dhcp"),
        dns_nameservers: v.lines("dns"),
        allocation_pools: pools,
        ipv6_ra_mode,
        ipv6_address_mode,
        host_routes,
    })
}

/// *Create Network*: the network, then an optional subnet (wizard).
pub fn create_network_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let mut fields = vec![
        Field::text("name", tr(K::NetworkName)),
        Field::check("admin_up", tr(K::EnableAdminState), true),
        Field::check("with_subnet", tr(K::WithSubnet), true),
    ];
    let net = crate::components::dialog::prefs().network;
    fields.extend(subnet_fields(lang, Some(("with_subnet", "true")), &net));
    let with_subnet: fn(&Vals) -> bool = |v| v.get("with_subnet").map(String::as_str) == Some("true");
    Dialog::form(tr(K::CreateNetwork), tr(K::CreateNetwork), fields, move |v| {
        Ok(OsAction::NetworkCreate {
            name: v.get("name"),
            admin_up: v.flag("admin_up"),
            subnet: if v.flag("with_subnet") {
                Some(subnet_spec(v, lang)?)
            } else {
                None
            },
        })
    })
    // Horizon's three steps; the subnet ones switch off without a subnet.
    .wizard(vec![
        Step::new(tr(K::Network), &["name", "admin_up", "with_subnet"]).help(tr(K::NetworkStepHelp)),
        Step::new(
            tr(K::Subnet),
            &["subnet_name", "cidr", "ip_version", "gateway", "no_gateway"],
        )
        .help(tr(K::SubnetStepHelp))
        .enabled_if(with_subnet),
        Step::new(
            tr(K::StepSubnetDetails),
            &["dhcp", "ipv6_modes", "pools", "dns", "host_routes"],
        )
        .help(tr(K::SubnetDetailsHelp))
        .enabled_if(with_subnet),
    ])
    .check(subnet_check(lang, net))
}

/// Admin › Networks' *Create Network*: Horizon's admin form (the
/// owner project, the provider attributes, *Shared*, *External*), then the same
/// optional subnet steps as Project's.
pub fn admin_create_network_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    // `[network] supported_provider_types` / `segmentation_id_range` (Horizon's).
    let provider_types = crate::components::dialog::prefs().network.provider_types();
    let types: Vec<&str> = provider_types.iter().map(|(t, _)| t.as_str()).collect();
    // Horizon: a physical network for flat and VLAN, a segmentation ID for the
    // tunnelled types and VLAN.
    let physical: fn(&Vals) -> bool = |v| matches!(v.get("provider_type").map(String::as_str), Some("flat" | "vlan"));
    let segmented: fn(&Vals) -> bool = |v| {
        matches!(
            v.get("provider_type").map(String::as_str),
            Some("vlan" | "gre" | "vxlan" | "geneve")
        )
    };
    let mut fields = vec![
        Field::text("name", tr(K::NetworkName)),
        Field::select("project_id", tr(K::Project), Source::Set(ChoiceSet::AllProjects))
            .required()
            .empty_label(tr(K::SelectProject)),
        // Horizon preselects the first type, *Local*.
        Field::select("provider_type", tr(K::ProviderNetworkType), options_raw(&types))
            .value(types[0])
            .help(tr(K::ProviderNetworkTypeHelp))
            .required(),
        Field::text("physical_network", tr(K::PhysicalNetwork))
            .help("physnet1")
            .show_if(physical)
            .required(),
        Field::number("segmentation_id", tr(K::SegmentationId), 1)
            .help(tr(K::SegmentationIdHelp))
            .show_if(segmented),
        Field::check("admin_up", tr(K::EnableAdminState), true),
        Field::check("shared", tr(K::Shared), false),
        Field::check("external", tr(K::ExternalNetwork), false),
        Field::check("with_subnet", tr(K::WithSubnet), true),
    ];
    // Horizon's *Availability Zone Hints*, only where Neutron schedules by zone.
    let zones = needs::NETWORK_AZ.met(&crate::components::dialog::caps());
    if zones {
        fields.push(
            Field::multi("az_hints", tr(K::AzHints), Source::Set(ChoiceSet::NetworkZones))
                .help(tr(K::AzHintsHelp))
                .hide_when_empty(),
        );
    }
    fields.push(Field::number("mtu", tr(K::Mtu), 68).help(tr(K::MtuHelp)));
    let net = crate::components::dialog::prefs().network;
    fields.extend(subnet_fields(lang, Some(("with_subnet", "true")), &net));
    let with_subnet: fn(&Vals) -> bool = |v| v.get("with_subnet").map(String::as_str) == Some("true");
    Dialog::form(tr(K::CreateNetwork), tr(K::CreateNetwork), fields, move |v| {
        let mtu = num_of::<u32>(v, "mtu", K::Mtu, lang)?;
        let segmentation_id = num_of::<u32>(v, "segmentation_id", K::SegmentationId, lang)?;
        let provider = ProviderNetwork {
            network_type: req(v, "provider_type", K::ProviderNetworkType, lang)?,
            physical_network: v.opt("physical_network"),
            segmentation_id,
        };
        provider.validate(&provider_types)?;
        Ok(OsAction::AdminNetworkCreate(Box::new(AdminNetworkSpec {
            name: v.get("name"),
            project_id: req(v, "project_id", K::Project, lang)?,
            provider: Some(provider),
            admin_up: v.flag("admin_up"),
            shared: v.flag("shared"),
            external: v.flag("external"),
            mtu,
            az_hints: v.list("az_hints"),
            subnet: if v.flag("with_subnet") {
                Some(subnet_spec(v, lang)?)
            } else {
                None
            },
        })))
    })
    .wizard(vec![
        // Horizon's order: the subnet checkbox, then the zone hints and the MTU.
        Step::new(
            tr(K::Network),
            &[
                "name",
                "project_id",
                "provider_type",
                "physical_network",
                "segmentation_id",
                "admin_up",
                "shared",
                "external",
                "with_subnet",
                "az_hints",
                "mtu",
            ],
        )
        .help(tr(K::AdminNetworkStepHelp)),
        Step::new(
            tr(K::Subnet),
            &["subnet_name", "cidr", "ip_version", "gateway", "no_gateway"],
        )
        .help(tr(K::SubnetStepHelp))
        .enabled_if(with_subnet),
        Step::new(
            tr(K::StepSubnetDetails),
            &["dhcp", "ipv6_modes", "pools", "dns", "host_routes"],
        )
        .help(tr(K::SubnetDetailsHelp))
        .enabled_if(with_subnet),
    ])
    .check(subnet_check(lang, net))
}

/// Admin › RBAC Policies' *Create RBAC Policy*, Horizon's form:
/// *Target Project* (`*` = every project), *Action and Object Type*, then the
/// network or the QoS policy it shares. *Shared QoS Policy* needs Neutron's `qos`.
pub fn create_rbac_policy_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let mut kinds = vec![
        ("shared_network", K::SharedNetwork),
        ("external_network", K::ExternalNetwork),
    ];
    if Requires::NetExt("qos").met(&crate::components::dialog::caps()) {
        kinds.push(("shared_qos_policy", K::SharedQosPolicy));
    }
    let network: fn(&Vals) -> bool = |v| {
        matches!(
            v.get("kind").map(String::as_str),
            Some("shared_network" | "external_network")
        )
    };
    let fields = vec![
        Field::select(
            "target_project",
            tr(K::TargetProject),
            Source::Set(ChoiceSet::RbacTargets),
        )
        .help(tr(K::RbacTargetHelp))
        .required()
        .empty_label(tr(K::SelectChoice)),
        Field::select("kind", tr(K::ActionObjectType), options(&kinds, lang))
            .required()
            .empty_label(tr(K::SelectChoice)),
        Field::select("network_id", tr(K::Network), Source::Set(ChoiceSet::AllNetworks))
            .show_if(network)
            .required()
            .empty_label(tr(K::SelectChoice)),
        Field::select(
            "qos_policy_id",
            tr(K::QosPolicy),
            Source::Set(ChoiceSet::AllQosPolicies),
        )
        .when("kind", "shared_qos_policy")
        .required()
        .empty_label(tr(K::SelectChoice)),
    ];
    Dialog::form(tr(K::CreateRbacPolicy), tr(K::CreateRbacPolicy), fields, move |v| {
        let kind = req(v, "kind", K::ActionObjectType, lang)?;
        let (object_type, _) = rbac_kind(&kind)?;
        let object_id = if object_type == "qos_policy" {
            req(v, "qos_policy_id", K::QosPolicy, lang)?
        } else {
            req(v, "network_id", K::Network, lang)?
        };
        Ok(OsAction::RbacPolicyCreate {
            kind,
            object_id,
            target_project: req(v, "target_project", K::TargetProject, lang)?,
        })
    })
}

/// Horizon's *Update RBAC Policy*: only the target project changes (`*` = every
/// project), preselected with the current one.
pub fn edit_rbac_policy_dialog(lang: Lang, r: &RbacPolicyRow) -> Dialog {
    let id = r.id.clone();
    let target = Field::select(
        "target_project",
        t(lang, K::TargetProject),
        Source::Set(ChoiceSet::RbacTargets),
    )
    .value(r.target_id.clone())
    .help(t(lang, K::RbacTargetHelp))
    .required();
    Dialog::form(
        t(lang, K::UpdateRbacPolicy),
        t(lang, K::SaveChanges),
        vec![target],
        move |v| {
            Ok(OsAction::RbacPolicyUpdate {
                id: id.clone(),
                target_project: req(v, "target_project", K::TargetProject, lang)?,
            })
        },
    )
}

/// An RBAC policy's row menu, as Horizon's: *Edit Policy*, *Delete RBAC Policy*.
pub fn rbac_policy_items(lang: Lang, r: &RbacPolicyRow) -> Vec<MenuItem> {
    let row = r.clone();
    let (id, label) = (
        r.id.clone(),
        format!("{} {} → {}", r.object_type, r.object.name, r.target),
    );
    vec![
        MenuItem::dialog(t(lang, K::EditPolicy), move || edit_rbac_policy_dialog(lang, &row)),
        MenuItem::danger(t(lang, K::DeleteRbacPolicy), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeleteRbacPolicy),
                label.clone(),
                OsAction::RbacPolicyDelete { id: id.clone() },
            )
        }),
    ]
}

/// *Create Subnet* on `network_id`.
pub fn create_subnet_dialog(lang: Lang, network_id: &str) -> Dialog {
    let n = network_id.to_string();
    let net = crate::components::dialog::prefs().network;
    Dialog::form(
        t(lang, K::CreateSubnet),
        t(lang, K::CreateSubnet),
        subnet_fields(lang, None, &net),
        move |v| {
            Ok(OsAction::SubnetCreate {
                network_id: n.clone(),
                spec: subnet_spec(v, lang)?,
            })
        },
    )
    .check(subnet_check(lang, net))
}

/// Nothing for other projects' (external, shared) networks, like Horizon.
pub fn network_items(lang: Lang, net: &NetworkRow) -> Vec<MenuItem> {
    if !net.owned {
        return Vec::new();
    }
    network_menu(lang, net, false)
}

/// Admin › Networks' row menu: every project's network, as
/// Horizon's admin table: *Edit Network*, *Create Subnet*, *Delete Network*.
pub fn admin_network_items(lang: Lang, net: &NetworkRow) -> Vec<MenuItem> {
    network_menu(lang, net, true)
}

/// Horizon's admin *Edit Network*: name, admin state, *Shared*, *External
/// Network*. The provider attributes are shown above the fields, read-only:
/// Horizon doesn't edit them either, and ML2 refuses to change them.
pub fn admin_edit_network_dialog(lang: Lang, net: &NetworkRow) -> Dialog {
    let id = net.id.clone();
    let mut d = Dialog::form(
        t(lang, K::EditNetwork),
        t(lang, K::Save),
        vec![
            Field::text("name", t(lang, K::NetworkName)).value(net.name.clone()),
            Field::check("admin_up", t(lang, K::EnableAdminState), net.admin_up),
            Field::check("shared", t(lang, K::Shared), net.shared),
            Field::check("external", t(lang, K::ExternalNetwork), net.external),
        ],
        move |v| {
            Ok(OsAction::AdminNetworkUpdate {
                id: id.clone(),
                name: v.get("name"),
                admin_up: v.flag("admin_up"),
                shared: v.flag("shared"),
                external: v.flag("external"),
            })
        },
    );
    if let Some(p) = &net.provider {
        d.intro = vec![crate::i18n::provider_summary(lang, p)];
    }
    d
}

/// A network's menu. `admin`: Horizon's admin edit and no *Create Port*
/// (Admin › Networks); else Project's edit and *Create Port*.
fn network_menu(lang: Lang, net: &NetworkRow, admin: bool) -> Vec<MenuItem> {
    let (id, name, admin_up) = (net.id.as_str(), net.name.as_str(), net.admin_up);
    let (i, n) = (id.to_string(), name.to_string());
    let row = net.clone();
    let mut items = vec![MenuItem::dialog(t(lang, K::EditNetwork), move || {
        if admin {
            return admin_edit_network_dialog(lang, &row);
        }
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditNetwork),
            t(lang, K::Save),
            vec![
                Field::text("name", t(lang, K::NetworkName)).value(n.clone()),
                Field::check("admin_up", t(lang, K::EnableAdminState), admin_up),
            ],
            move |v| {
                Ok(OsAction::NetworkUpdate {
                    id: i.clone(),
                    name: v.get("name"),
                    admin_up: v.flag("admin_up"),
                })
            },
        )
    })];
    let i = id.to_string();
    items.push(MenuItem::dialog(t(lang, K::CreateSubnet), move || {
        create_subnet_dialog(lang, &i)
    }));
    if !admin {
        let i = id.to_string();
        items.push(MenuItem::dialog(t(lang, K::CreatePort), move || {
            create_port_dialog(lang, &i)
        }));
    }
    let (i, n) = (id.to_string(), name.to_string());
    items.push(MenuItem::danger(t(lang, K::DeleteNetwork), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteNetwork),
            n.clone(),
            OsAction::NetworkDelete { id: i.clone() },
        )
    }));
    items
}

/// A subnet's menu (edit, delete).
pub fn subnet_items(lang: Lang, s: &SubnetRow) -> Vec<MenuItem> {
    let (i, n, dhcp, dns) = (
        s.id.clone(),
        s.name.clone(),
        s.enable_dhcp,
        s.dns_nameservers.join("\n"),
    );
    let mut items = vec![MenuItem::dialog(t(lang, K::EditSubnet), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditSubnet),
            t(lang, K::Save),
            vec![
                Field::text("name", t(lang, K::SubnetName)).value(n.clone()),
                Field::check("dhcp", t(lang, K::EnableDhcp), dhcp),
                Field::area("dns", t(lang, K::DnsNameServers))
                    .value(dns.clone())
                    .help(t(lang, K::OnePerLine)),
            ],
            move |v| {
                Ok(OsAction::SubnetUpdate {
                    id: i.clone(),
                    name: v.get("name"),
                    enable_dhcp: v.flag("dhcp"),
                    dns_nameservers: v.lines("dns"),
                })
            },
        )
    })];
    let (i, n) = (s.id.clone(), s.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteSubnet), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteSubnet),
            n.clone(),
            OsAction::SubnetDelete { id: i.clone() },
        )
    }));
    items
}

/// *Create Port* on `network_id`.
pub fn create_port_dialog(lang: Lang, network_id: &str) -> Dialog {
    let n = network_id.to_string();
    Dialog::form(
        t(lang, K::CreatePort),
        t(lang, K::CreatePort),
        vec![
            Field::text("name", t(lang, K::Name)),
            Field::check("admin_up", t(lang, K::EnableAdminState), true),
            Field::text("fixed_ip", t(lang, K::FixedIpOptional)),
        ],
        move |v| {
            Ok(OsAction::PortCreate {
                network_id: n.clone(),
                name: v.get("name"),
                admin_up: v.flag("admin_up"),
                fixed_ip: v.opt("fixed_ip"),
            })
        },
    )
}

/// A port's menu (edit, delete).
pub fn port_items(lang: Lang, p: &PortRow) -> Vec<MenuItem> {
    let (i, settings) = (p.id.clone(), p.settings());
    let mut items = vec![MenuItem::dialog(t(lang, K::EditPort), move || {
        edit_port_dialog(lang, &i, &settings, false)
    })];
    let (i, label) = (
        p.id.clone(),
        if p.name.is_empty() {
            p.id.clone()
        } else {
            p.name.clone()
        },
    );
    items.push(MenuItem::danger(t(lang, K::DeletePort), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeletePort),
            label.clone(),
            OsAction::PortDelete { id: i.clone() },
        )
    }));
    items
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::components::dialog::Kind;

    #[test]
    fn subnet_dialogs_follow_the_network_settings() {
        use crate::prefs::{AllowedCidrs, NetworkPrefs};
        let field = |fs: &[Field], n: &str| fs.iter().find(|f| f.name == n).cloned().unwrap();
        let def = subnet_fields(Lang::EnUs, None, &NetworkPrefs::default());
        assert_eq!(field(&def, "dns").default, "", "no DNS servers by default");
        assert!(matches!(field(&def, "ip_version").kind, Kind::Select(Source::Static(o)) if o.len() == 2));
        let net = NetworkPrefs {
            default_dns_nameservers: vec!["10.0.0.2".into(), "10.0.0.3".into()],
            enable_ipv6: false,
            allowed_private_subnet_cidr: AllowedCidrs {
                ipv4: vec!["10.0.0.0/8".into()],
                ipv6: vec![],
            },
            ..Default::default()
        };
        let fs = subnet_fields(Lang::EnUs, None, &net);
        assert_eq!(field(&fs, "dns").default, "10.0.0.2\n10.0.0.3");
        assert!(matches!(field(&fs, "ip_version").kind, Kind::Select(Source::Static(o)) if o.len() == 1));
        let check = subnet_check(Lang::EnUs, net);
        let vals = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vals>()
        };
        let sets = Sets::default();
        assert!(check(&vals(&[("cidr", "10.1.0.0/24")]), &sets).is_empty());
        assert_eq!(check(&vals(&[("cidr", "172.16.0.0/24")]), &sets)[0].0, "cidr");
        assert!(
            check(&vals(&[("with_subnet", "false"), ("cidr", "172.16.0.0/24")]), &sets).is_empty(),
            "no subnet: no check"
        );
        let a = OsAction::Batch(vec![(
            "s".into(),
            OsAction::SubnetCreate {
                network_id: "n".into(),
                spec: SubnetSpec {
                    cidr: "10.0.0.0/24".into(),
                    ..sample_subnet()
                },
            },
        )]);
        assert_eq!(a.new_subnet_cidrs(), ["10.0.0.0/24"]);
    }

    fn sample_subnet() -> SubnetSpec {
        SubnetSpec {
            name: String::new(),
            cidr: String::new(),
            ip_version: 4,
            gateway_ip: None,
            enable_dhcp: true,
            dns_nameservers: vec![],
            allocation_pools: vec![],
            ipv6_ra_mode: None,
            ipv6_address_mode: None,
            host_routes: vec![],
        }
    }

    #[test]
    fn admin_edit_network_sends_shared_and_external_and_shows_the_provider() {
        let net = NetworkRow {
            id: "n1".into(),
            name: "ext".into(),
            subnets: vec![],
            shared: false,
            external: true,
            status: "ACTIVE".into(),
            admin_up: true,
            availability_zones: vec![],
            owned: false,
            dhcp_agents: None,
            provider: Some(ProviderNetwork {
                network_type: "vlan".into(),
                physical_network: Some("physnet1".into()),
                segmentation_id: Some(120),
            }),
        };
        let d = admin_edit_network_dialog(Lang::EnUs, &net);
        assert_eq!(
            d.fields.iter().map(|f| f.name).collect::<Vec<_>>(),
            ["name", "admin_up", "shared", "external"],
            "Horizon's four fields"
        );
        assert!(
            d.intro[0].contains("vlan") && d.intro[0].contains("physnet1") && d.intro[0].contains("120"),
            "{:?}",
            d.intro
        );
        let a = (d.build)(&vals(&[
            ("name", "ext"),
            ("admin_up", "true"),
            ("shared", "true"),
            ("external", "false"),
        ]))
        .unwrap();
        assert_eq!(
            a,
            OsAction::AdminNetworkUpdate {
                id: "n1".into(),
                name: "ext".into(),
                admin_up: true,
                shared: true,
                external: false
            }
        );
    }

    #[test]
    fn admin_network_form_builds_provider_and_owner() {
        let d = admin_create_network_dialog(Lang::EnUs);
        let base = [
            ("name", "ext"),
            ("project_id", "p1"),
            ("admin_up", "true"),
            ("shared", "true"),
            ("external", "true"),
            ("with_subnet", "false"),
        ];
        let with = |extra: &[(&'static str, &'static str)]| vals(&[&base[..], extra].concat());
        let a = (d.build)(&with(&[
            ("provider_type", "vlan"),
            ("physical_network", "physnet1"),
            ("segmentation_id", "120"),
        ]))
        .unwrap();
        let OsAction::AdminNetworkCreate(spec) = a else {
            panic!()
        };
        assert_eq!(
            (
                spec.project_id.as_str(),
                spec.shared,
                spec.external,
                spec.subnet.is_none()
            ),
            ("p1", true, true, true)
        );
        assert_eq!(
            spec.provider,
            Some(ProviderNetwork {
                network_type: "vlan".into(),
                physical_network: Some("physnet1".into()),
                segmentation_id: Some(120)
            })
        );
        assert!(
            (d.build)(&with(&[("provider_type", "vlan"), ("segmentation_id", "120")])).is_err(),
            "vlan without a physical network"
        );
        assert!(
            (d.build)(&with(&[("provider_type", "vxlan"), ("segmentation_id", "99999999")])).is_err(),
            "VNI out of range"
        );
        assert!(
            (d.build)(&vals(&[("provider_type", "vxlan")])).is_err(),
            "the project is required"
        );
        let a = (d.build)(&with(&[
            ("provider_type", "local"),
            ("mtu", "1450"),
            ("az_hints", "az1,az2"),
        ]))
        .unwrap();
        let OsAction::AdminNetworkCreate(spec) = a else {
            panic!()
        };
        assert_eq!(
            (spec.mtu, spec.az_hints.clone()),
            (Some(1450), vec!["az1".to_string(), "az2".to_string()])
        );
        assert_eq!(
            d.fields
                .iter()
                .find(|f| f.name == "provider_type")
                .map(|f| f.default.as_str()),
            Some("local"),
            "Horizon preselects Local"
        );
    }

    #[test]
    fn rbac_edit_changes_only_the_target_and_starts_from_the_current_one() {
        let r = RbacPolicyRow {
            id: "r1".into(),
            project: "admin".into(),
            object_type: "network".into(),
            object: Ref {
                id: "n1".into(),
                name: "public".into(),
            },
            target: "demo".into(),
            target_id: "p2".into(),
            action: "access_as_shared".into(),
        };
        let labels: Vec<String> = rbac_policy_items(Lang::EnUs, &r).into_iter().map(|m| m.label).collect();
        assert_eq!(
            labels,
            ["Edit Policy", "Delete RBAC Policy"],
            "Horizon's row menu, delete last"
        );
        let d = edit_rbac_policy_dialog(Lang::EnUs, &r);
        assert_eq!(
            (d.fields.len(), d.fields[0].default.as_str()),
            (1, "p2"),
            "one field, preselected with the target's ID"
        );
        let a = (d.build)(&vals(&[("target_project", "*")])).unwrap();
        assert_eq!(
            a,
            OsAction::RbacPolicyUpdate {
                id: "r1".into(),
                target_project: "*".into()
            }
        );
    }

    #[test]
    fn rbac_form_picks_the_object_by_kind() {
        let d = create_rbac_policy_dialog(Lang::EnUs);
        let a = (d.build)(&vals(&[
            ("target_project", "*"),
            ("kind", "external_network"),
            ("network_id", "n1"),
            ("qos_policy_id", "q1"),
        ]))
        .unwrap();
        assert_eq!(
            a,
            OsAction::RbacPolicyCreate {
                kind: "external_network".into(),
                object_id: "n1".into(),
                target_project: "*".into()
            }
        );
        let a = (d.build)(&vals(&[
            ("target_project", "p2"),
            ("kind", "shared_qos_policy"),
            ("qos_policy_id", "q1"),
        ]))
        .unwrap();
        assert_eq!(
            a,
            OsAction::RbacPolicyCreate {
                kind: "shared_qos_policy".into(),
                object_id: "q1".into(),
                target_project: "p2".into()
            }
        );
        assert!(
            (d.build)(&vals(&[("target_project", "p2"), ("kind", "shared_network")])).is_err(),
            "the network is required"
        );
    }

    #[test]
    fn subnet_steps_read_ipv6_modes_and_host_routes() {
        let d = create_network_dialog(Lang::EnUs);
        let base = [("with_subnet", "true"), ("dhcp", "true")];
        let build = |extra: &[(&'static str, &'static str)]| (d.build)(&vals(&[&base[..], extra].concat()));
        let subnet = |a: OsAction| match a {
            OsAction::NetworkCreate { subnet: Some(s), .. } => s,
            other => panic!("{other:?}"),
        };
        let s = subnet(
            build(&[
                ("cidr", "2001:db8::/64"),
                ("ip_version", "6"),
                ("ipv6_modes", "none/slaac"),
                ("host_routes", "2001:db8:1::/64,2001:db8::1"),
            ])
            .unwrap(),
        );
        assert_eq!((s.ipv6_ra_mode, s.ipv6_address_mode.as_deref()), (None, Some("slaac")));
        assert_eq!(
            s.host_routes,
            vec![("2001:db8:1::/64".to_string(), "2001:db8::1".to_string())]
        );
        let s = subnet(
            build(&[
                ("cidr", "10.0.0.0/24"),
                ("ip_version", "4"),
                ("ipv6_modes", "slaac/slaac"),
            ])
            .unwrap(),
        );
        assert_eq!(s.ipv6_address_mode, None, "IPv4 ignores a leftover IPv6 mode");
        assert!(
            build(&[
                ("cidr", "10.0.0.0/24"),
                ("ip_version", "4"),
                ("host_routes", "10.1.0.0/24")
            ])
            .is_err(),
            "a route needs its next hop"
        );
    }

    #[test]
    fn network_form_with_and_without_subnet() {
        let d = create_network_dialog(Lang::EnUs);
        let a = (d.build)(&vals(&[("name", "n"), ("admin_up", "true"), ("with_subnet", "false")])).unwrap();
        assert!(matches!(a, OsAction::NetworkCreate { subnet: None, .. }));
        let a = (d.build)(&vals(&[
            ("with_subnet", "true"),
            ("cidr", "10.0.0.0/24"),
            ("ip_version", "4"),
            ("no_gateway", "true"),
            ("dhcp", "true"),
            ("pools", "10.0.0.10,10.0.0.20"),
            ("dns", "1.1.1.1\n8.8.8.8"),
        ]))
        .unwrap();
        let OsAction::NetworkCreate { subnet: Some(s), .. } = a else {
            panic!()
        };
        assert_eq!(s.gateway_ip.as_deref(), Some(""));
        assert_eq!(s.allocation_pools, vec![("10.0.0.10".into(), "10.0.0.20".into())]);
        assert_eq!(s.dns_nameservers.len(), 2);
        assert!((d.build)(&vals(&[("with_subnet", "true"), ("cidr", "x"), ("pools", "nocomma")])).is_err());
    }

    #[test]
    fn foreign_networks_and_the_default_group_are_read_only() {
        let net = |owned| NetworkRow {
            id: "n".into(),
            name: "public".into(),
            subnets: vec![],
            shared: false,
            external: true,
            status: "ACTIVE".into(),
            admin_up: true,
            availability_zones: vec![],
            owned,
            dhcp_agents: None,
            provider: None,
        };
        assert!(network_items(Lang::EnUs, &net(false)).is_empty());
        let labels = |items: Vec<MenuItem>| items.into_iter().map(|m| m.label).collect::<Vec<_>>();
        assert_eq!(
            labels(network_items(Lang::EnUs, &net(true))),
            ["Edit Network", "Create Subnet", "Create Port", "Delete Network"]
        );
        assert_eq!(
            labels(admin_network_items(Lang::EnUs, &net(false))),
            ["Edit Network", "Create Subnet", "Delete Network"],
            "Admin: any project's network, Horizon's admin menu"
        );
        assert_eq!(
            labels(security_group_items(Lang::EnUs, "g", "default", None, true)),
            ["Manage Rules"]
        );
        assert!(security_group_items(Lang::EnUs, "g", "default", None, false).is_empty());
        assert_eq!(security_group_items(Lang::EnUs, "g", "web", None, true).len(), 3);
    }
}
