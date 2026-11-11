//! Security groups and their rules.

use super::*;

/// *Create Security Group*: name and description.
pub fn create_security_group_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::CreateSecurityGroup),
        t(lang, K::CreateSecurityGroup),
        vec![
            Field::text("name", t(lang, K::Name)).required(),
            Field::area("description", t(lang, K::Description)),
        ],
        move |v| {
            Ok(OsAction::SecurityGroupCreate {
                name: req(v, "name", K::Name, lang)?,
                description: v.opt("description"),
            })
        },
    )
}

/// `default` only gets *Manage Rules*: Neutron won't delete it, and renaming it
/// is pointless (Horizon offers neither).
pub fn security_group_items(
    lang: Lang,
    id: &str,
    name: &str,
    description: Option<&str>,
    with_manage: bool,
) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if with_manage {
        items.push(MenuItem::link(
            t(lang, K::ManageRules),
            format!("/project/security_groups/{id}"),
        ));
    }
    if name == "default" {
        return items;
    }
    let (i, n, d) = (id.to_string(), name.to_string(), description.unwrap_or("").to_string());
    items.push(MenuItem::dialog(t(lang, K::EditSecurityGroup), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditSecurityGroup),
            t(lang, K::Save),
            vec![
                Field::text("name", t(lang, K::Name)).value(n.clone()).required(),
                Field::area("description", t(lang, K::Description)).value(d.clone()),
            ],
            move |v| {
                Ok(OsAction::SecurityGroupUpdate {
                    id: i.clone(),
                    name: req(v, "name", K::Name, lang)?,
                    description: v.opt("description"),
                })
            },
        )
    }));
    let (i, n) = (id.to_string(), name.to_string());
    items.push(MenuItem::danger(t(lang, K::DeleteSecurityGroup), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteSecurityGroup),
            n.clone(),
            OsAction::SecurityGroupDelete { id: i.clone() },
        )
    }));
    items
}

/// *Add Rule* to a security group: Neutron's rule fields, each with a help text
/// under it (roadmap 61) instead of Horizon's side panel. The ports stay visible
/// whatever the protocol; their help says when they don't apply.
pub fn add_rule_dialog(lang: Lang, group_id: &str) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let g = group_id.to_string();
    Dialog::form(
        tr(K::AddRule),
        tr(K::AddRule),
        vec![
            Field::select(
                "direction",
                tr(K::Direction),
                options(&[("ingress", K::Ingress), ("egress", K::Egress)], lang),
            )
            .value("ingress")
            .required()
            .help(tr(K::DirectionHelp)),
            Field::select("ethertype", tr(K::EtherType), options_raw(&["IPv4", "IPv6"]))
                .value("IPv4")
                .required()
                .help(tr(K::EtherTypeHelp)),
            Field::select("protocol", tr(K::IpProtocol), options_raw(&["tcp", "udp", "icmp"]))
                .help(tr(K::IpProtocolHelp)),
            Field::number("port_min", tr(K::PortFrom), 0).help(tr(K::PortFromHelp)),
            Field::number("port_max", tr(K::PortTo), 0).help(tr(K::PortToHelp)),
            Field::text("cidr", tr(K::RemoteCidr))
                .value("0.0.0.0/0")
                .help(tr(K::RemoteCidrHelp)),
            Field::select(
                "remote_group",
                tr(K::RemoteGroup),
                Source::Set(ChoiceSet::SecurityGroupIds),
            )
            .help(tr(K::RemoteGroupHelp)),
            Field::text("description", tr(K::Description)).help(tr(K::RuleDescriptionHelp)),
        ],
        move |v| {
            let to_i32 = |k: &str, label: K| -> Result<Option<i32>, String> {
                v.num(k)
                    .map(|o| o.map(|n| n as i32))
                    .map_err(|_| t(lang, label).to_string())
            };
            Ok(OsAction::RuleCreate(RuleSpec {
                security_group_id: g.clone(),
                direction: req(v, "direction", K::Direction, lang)?,
                ethertype: req(v, "ethertype", K::EtherType, lang)?,
                protocol: v.opt("protocol"),
                port_min: to_i32("port_min", K::PortFrom)?,
                port_max: to_i32("port_max", K::PortTo)?,
                // A remote group wins over the default CIDR.
                remote_ip_prefix: if v.opt("remote_group").is_some() {
                    None
                } else {
                    v.opt("cidr")
                },
                remote_group_id: v.opt("remote_group"),
                description: v.opt("description"),
            }))
        },
    )
}

/// A security group rule's menu (*Delete Rule*).
pub fn rule_items(lang: Lang, r: &RuleRow) -> Vec<MenuItem> {
    let (i, label) = (
        r.id.clone(),
        format!("{} {} {} {}", r.direction, r.ethertype, r.protocol, r.port_range),
    );
    vec![MenuItem::danger(t(lang, K::DeleteRule), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteRule),
            label.clone(),
            OsAction::RuleDelete { id: i.clone() },
        )
    })]
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[test]
    fn rule_form_prefers_remote_group_over_cidr() {
        let d = add_rule_dialog(Lang::EnUs, "sg");
        let a = (d.build)(&vals(&[
            ("direction", "ingress"),
            ("ethertype", "IPv4"),
            ("protocol", "tcp"),
            ("port_min", "22"),
            ("cidr", "0.0.0.0/0"),
        ]))
        .unwrap();
        let OsAction::RuleCreate(r) = a else { panic!() };
        assert_eq!(
            (r.port_min, r.remote_ip_prefix.as_deref()),
            (Some(22), Some("0.0.0.0/0"))
        );
        let a = (d.build)(&vals(&[
            ("direction", "egress"),
            ("ethertype", "IPv6"),
            ("remote_group", "other"),
            ("cidr", "0.0.0.0/0"),
        ]))
        .unwrap();
        let OsAction::RuleCreate(r) = a else { panic!() };
        assert_eq!(r.remote_ip_prefix, None);
        assert_eq!(r.remote_group_id.as_deref(), Some("other"));
    }

    #[test]
    fn every_rule_field_has_help_and_the_to_port_label_has_no_hint() {
        let d = add_rule_dialog(Lang::EnUs, "sg");
        let missing: Vec<_> = d.fields.iter().filter(|f| f.help.is_none()).map(|f| f.name).collect();
        assert!(missing.is_empty(), "fields without help (roadmap 61): {missing:?}");
        let to = d.fields.iter().find(|f| f.name == "port_max").unwrap();
        assert_eq!(to.label, "To Port", "the \"empty = same\" hint moved to the help text");
    }
}
