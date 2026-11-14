//! Floating IPs and port forwarding.

use super::*;

/// *Allocate IP To Project*: the external network (pool) and a description.
pub fn allocate_ip_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::AllocateIp),
        t(lang, K::AllocateIp),
        {
            let mut f = vec![
                Field::select("pool", t(lang, K::Pool), Source::Set(ChoiceSet::ExternalNetworks)).required(),
                Field::text("description", t(lang, K::Description)),
            ];
            // `qos` plugin and `qos-fip`.
            if needs::QOS_FIP.met(&caps()) {
                f.push(qos_field(lang));
            }
            f
        },
        move |v| {
            Ok(OsAction::FloatingIpAllocate {
                network_id: req(v, "pool", K::Pool, lang)?,
                description: v.opt("description"),
                port_id: None,
                qos_policy_id: v.opt("qos"),
            })
        },
    )
}

/// Admin › Floating IPs' *Allocate IP To Project* (roadmap 85), Horizon's
/// *Allocate Floating IP*: pool, project, optional address, description.
pub fn admin_allocate_ip_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::select("pool", tr(K::Pool), Source::Set(ChoiceSet::ExternalNetworks)).required(),
        Field::select("project_id", tr(K::Project), Source::Set(ChoiceSet::AllProjects))
            .required()
            .empty_label(tr(K::SelectProject)),
        Field::text("address", tr(K::FloatingIpAddressOptional)).help(tr(K::FloatingIpAddressHelp)),
        Field::text("description", tr(K::Description)),
    ];
    Dialog::form(tr(K::AllocateFloatingIp), tr(K::AllocateFloatingIp), fields, move |v| {
        Ok(OsAction::AdminFloatingIpAllocate {
            network_id: req(v, "pool", K::Pool, lang)?,
            project_id: req(v, "project_id", K::Project, lang)?,
            address: v.opt("address"),
            description: v.opt("description"),
        })
    })
}

/// Admin › Floating IPs' row menu, Horizon's: *Disassociate* (when mapped) and
/// *Release Floating IP*. No *Associate*: the ports to pick from would be the
/// session project's, not the IP's owner's.
pub fn admin_floating_ip_items(lang: Lang, f: &FloatingIpRow, project_id: Option<&str>) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if f.fixed_ip.is_some() {
        let (i, ip) = (f.id.clone(), f.ip.clone());
        items.push(MenuItem::danger(t(lang, K::Disassociate), move || {
            Dialog::confirm(
                lang,
                t(lang, K::Disassociate),
                ip.clone(),
                OsAction::FloatingIpDisassociate { id: i.clone() },
            )
        }));
    } else if let Some(owner) = project_id {
        // The owner's instance ports (Neutron needs the port and the floating
        // IP in one project), not the session project's, and only those the
        // IP's pool reaches through a router, as Horizon (roadmap 89).
        let (i, ip, owner) = (
            f.id.clone(),
            f.ip.clone(),
            crate::model::associable_port_key(owner, &f.pool_id),
        );
        items.push(MenuItem::dialog(t(lang, K::Associate), move || {
            let i = i.clone();
            let mut d = Dialog::form(
                t(lang, K::Associate),
                t(lang, K::Associate),
                vec![
                    Field::select(
                        "port",
                        t(lang, K::PortToAssociate),
                        Source::Of(ChoiceSet::AllAssociablePorts, owner.clone()),
                    )
                    .required()
                    // Search box however few ports there are (roadmap 110).
                    .searchable(),
                ],
                move |v| {
                    Ok(OsAction::FloatingIpAssociate {
                        id: i.clone(),
                        port_id: req(v, "port", K::PortToAssociate, lang)?,
                    })
                },
            );
            d.intro = vec![ip.clone()];
            d
        }));
    }
    let (i, ip) = (f.id.clone(), f.ip.clone());
    items.push(MenuItem::danger(t(lang, K::ReleaseIp), move || {
        Dialog::confirm(
            lang,
            t(lang, K::ReleaseIp),
            ip.clone(),
            OsAction::FloatingIpRelease { id: i.clone() },
        )
    }));
    items
}

/// *Add Rule* / *Edit Rule* for a floating IP's port forwarding. `ranges`: the
/// cloud accepts `8080:8090` (port-ranges extension).
pub fn port_forwarding_dialog(lang: Lang, floating_ip_id: &str, rule: Option<&PortForwardingRow>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let ranges = needs::PORT_FORWARDING_RANGES.met(&caps());
    let port_help = tr(if ranges { K::PortOrRangeHelp } else { K::PortHelp });
    let fields = vec![
        Field::select("protocol", tr(K::Protocol), options_raw(&["tcp", "udp"]))
            .value(rule.map_or("tcp".into(), |r| r.protocol.clone()))
            .required(),
        Field::select(
            "internal",
            tr(K::InternalIpAddress),
            Source::Set(ChoiceSet::ForwardablePorts),
        )
        .value(
            rule.map(|r| format!("{}/{}", r.internal_port_id, r.internal_ip))
                .unwrap_or_default(),
        )
        .required()
        // Search box however few ports there are, as Associate (roadmap 110).
        .searchable(),
        Field::text("internal_port", tr(K::InternalPort))
            .value(rule.map(|r| r.internal.clone()).unwrap_or_default())
            .help(port_help.clone())
            .required(),
        Field::text("external_port", tr(K::ExternalPort))
            .value(rule.map(|r| r.external.clone()).unwrap_or_default())
            .help(port_help)
            .required(),
        Field::text("description", tr(K::Description))
            .value(rule.and_then(|r| r.description.clone()).unwrap_or_default()),
    ];
    let (fip, id) = (floating_ip_id.to_string(), rule.map(|r| r.id.clone()));
    let title = tr(if rule.is_some() { K::EditRule } else { K::AddRule });
    Dialog::form(title.clone(), title, fields, move |v| {
        let internal = req(v, "internal", K::InternalIpAddress, lang)?;
        let (port_id, ip) = internal
            .split_once('/')
            .ok_or_else(|| t(lang, K::InternalIpAddress).to_string())?;
        let spec = PortForwardingSpec {
            floating_ip_id: fip.clone(),
            protocol: req(v, "protocol", K::Protocol, lang)?,
            internal_port_id: port_id.to_string(),
            internal_ip: ip.to_string(),
            internal: req(v, "internal_port", K::InternalPort, lang)?,
            external: req(v, "external_port", K::ExternalPort, lang)?,
            description: v.opt("description"),
            ranges,
        };
        spec.ports()
            .map_err(|_| t(lang, if ranges { K::PortOrRangeHelp } else { K::PortHelp }).to_string())?;
        Ok(match &id {
            Some(id) => OsAction::PortForwardingUpdate { id: id.clone(), spec },
            None => OsAction::PortForwardingCreate(spec),
        })
    })
}

/// A port forwarding's menu (edit, delete).
pub fn port_forwarding_items(lang: Lang, floating_ip_id: &str, r: &PortForwardingRow) -> Vec<MenuItem> {
    let (f, rule) = (floating_ip_id.to_string(), r.clone());
    let edit = MenuItem::dialog(t(lang, K::EditRule), move || {
        port_forwarding_dialog(lang, &f, Some(&rule))
    });
    let (f, id) = (floating_ip_id.to_string(), r.id.clone());
    let what = format!(
        "{} {} → {}:{}",
        r.protocol.to_uppercase(),
        r.external,
        r.internal_ip,
        r.internal
    );
    let delete = MenuItem::danger(t(lang, K::DeleteRule), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteRule),
            what.clone(),
            OsAction::PortForwardingDelete {
                floating_ip_id: f.clone(),
                id: id.clone(),
            },
        )
    });
    vec![edit, delete]
}

/// A floating IP's menu: associate or disassociate, port forwarding rules, release.
pub fn floating_ip_items(lang: Lang, f: &FloatingIpRow) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if f.fixed_ip.is_some() {
        let (i, ip) = (f.id.clone(), f.ip.clone());
        items.push(MenuItem::danger(t(lang, K::Disassociate), move || {
            Dialog::confirm(
                lang,
                t(lang, K::Disassociate),
                ip.clone(),
                OsAction::FloatingIpDisassociate { id: i.clone() },
            )
        }));
    } else {
        let i = f.id.clone();
        items.push(MenuItem::dialog(t(lang, K::Associate), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::Associate),
                t(lang, K::Associate),
                vec![
                    Field::select(
                        "port",
                        t(lang, K::PortToAssociate),
                        Source::Set(ChoiceSet::AssociablePorts),
                    )
                    .required()
                    // Search box however few ports there are (roadmap 110).
                    .searchable(),
                ],
                move |v| {
                    Ok(OsAction::FloatingIpAssociate {
                        id: i.clone(),
                        port_id: req(v, "port", K::PortToAssociate, lang)?,
                    })
                },
            )
        }));
    }
    // In the dropdown: Associate / Disassociate stays the main button.
    if needs::PORT_FORWARDING.met(&caps()) {
        items.push(MenuItem::link(
            t(lang, K::PortForwardingRules),
            format!("/project/floating_ips/{}?tab=forwarding", f.id),
        ));
    }
    let (i, ip) = (f.id.clone(), f.ip.clone());
    items.push(MenuItem::danger(t(lang, K::ReleaseIp), move || {
        Dialog::confirm(
            lang,
            t(lang, K::ReleaseIp),
            ip.clone(),
            OsAction::FloatingIpRelease { id: i.clone() },
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
    fn floating_ip_menu_keeps_associate_first_and_rules_in_the_dropdown() {
        let row = FloatingIpRow {
            id: "f1".into(),
            ip: "203.0.113.7".into(),
            fixed_ip: None,
            instance_id: None,
            instance_name: None,
            pool: "public".into(),
            pool_id: "net-public".into(),
            status: "DOWN".into(),
            description: None,
            dns_name: None,
            dns_domain: None,
        };
        let labels = |r: &FloatingIpRow| {
            floating_ip_items(Lang::EnUs, r)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(&row),
            ["Associate", "Port Forwarding Rules", "Release Floating IP"]
        );
        let associated = FloatingIpRow {
            fixed_ip: Some("10.0.0.5".into()),
            ..row.clone()
        };
        assert_eq!(labels(&associated)[0], "Disassociate");
        let admin = |r: &FloatingIpRow| {
            admin_floating_ip_items(Lang::EnUs, r, Some("p9"))
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            admin(&row),
            ["Associate", "Release Floating IP"],
            "Admin: Associate among the owner's ports"
        );
        let items = admin_floating_ip_items(Lang::EnUs, &row, Some("p9"));
        let d = dialog_of(&items, "Associate");
        assert!(
            matches!(&d.fields[0].kind, Kind::Select(Source::Of(ChoiceSet::AllAssociablePorts, p)) if p == "p9 net-public"),
            "the owner's ports its pool reaches, not the session's"
        );
        assert!(
            d.fields[0].searchable,
            "port picker always has a search box (roadmap 110)"
        );
        let d = dialog_of(&floating_ip_items(Lang::EnUs, &row), "Associate");
        assert!(d.fields[0].searchable, "project port picker too (roadmap 110)");
        assert_eq!(
            admin_floating_ip_items(Lang::EnUs, &row, None).len(),
            1,
            "no owner known: no Associate"
        );
        assert_eq!(
            admin(&associated),
            ["Disassociate", "Release Floating IP"],
            "Horizon's admin menu"
        );
    }

    #[test]
    fn port_forwarding_port_picker_has_a_search_box() {
        // As the Associate pickers (roadmap 110), even with a single instance port.
        let d = port_forwarding_dialog(Lang::EnUs, "f1", None);
        let f = d.fields.iter().find(|f| f.name == "internal").unwrap();
        assert!(f.searchable);
    }

    #[test]
    fn admin_allocate_names_the_project_and_optional_address() {
        let d = admin_allocate_ip_dialog(Lang::EnUs);
        let a = (d.build)(&vals(&[
            ("pool", "ext"),
            ("project_id", "p2"),
            ("address", ""),
            ("description", "vpn"),
        ]))
        .unwrap();
        assert_eq!(
            a,
            OsAction::AdminFloatingIpAllocate {
                network_id: "ext".into(),
                project_id: "p2".into(),
                address: None,
                description: Some("vpn".into())
            }
        );
        assert!((d.build)(&vals(&[("pool", "ext")])).is_err(), "the project is required");
    }
}
