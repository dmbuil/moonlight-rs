//! Firewall (FWaaS v2, roadmap 93).

use super::*;

/// Ports apply to TCP and UDP rules only (Neutron refuses them otherwise).
fn firewall_ports_apply(v: &Vals) -> bool {
    matches!(v.get("protocol").map(String::as_str), Some("tcp" | "udp"))
}

/// *Add Rule* (`None`) or *Edit Rule*. No *Shared*: the Project panel never offers or
/// sends it (Neutron's default, not shared, applies); it belongs to the Admin panel
/// (roadmap 94), as for policies and groups.
pub fn firewall_rule_dialog(lang: Lang, rule: Option<&FirewallRuleRow>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let r = rule.cloned();
    let val = |f: fn(&FirewallRuleRow) -> Option<String>| r.as_ref().and_then(f).unwrap_or_default();
    let fields = vec![
        Field::text("name", tr(K::Name)).value(val(|r| Some(r.name.clone()).filter(|n| *n != r.id))),
        Field::text("description", tr(K::Description)).value(val(|r| r.description.clone())),
        // One line: protocol, IP version, action; then source and destination, each
        // an address and (TCP / UDP only) a port on its own line.
        Field::select(
            "protocol",
            tr(K::Protocol),
            Source::Static(vec![
                ("any".into(), tr(K::AnyValue)),
                ("tcp".into(), "TCP".into()),
                ("udp".into(), "UDP".into()),
                ("icmp".into(), "ICMP".into()),
            ]),
        )
        .value(
            r.as_ref()
                .map(|r| r.protocol.clone().unwrap_or_else(|| "any".into()))
                .unwrap_or_else(|| "tcp".into()),
        )
        .required()
        .short(),
        Field::select("ip_version", tr(K::IpVersion), options_raw(&["4", "6"]))
            .value(r.as_ref().map(|r| r.ip_version).unwrap_or(4).to_string())
            .required()
            .short(),
        Field::select(
            "action",
            tr(K::Action),
            options(
                &[
                    ("allow", K::FirewallAllow),
                    ("deny", K::FirewallDeny),
                    ("reject", K::FirewallReject),
                ],
                lang,
            ),
        )
        .value(r.as_ref().map(|r| r.action.clone()).unwrap_or_else(|| "allow".into()))
        .required()
        .short(),
        Field::text("source_ip", tr(K::SourceIp))
            .value(val(|r| r.source_ip.clone()))
            .help(tr(K::FirewallAddressHelp))
            .half()
            .new_line(),
        Field::text("source_port", tr(K::SourcePort))
            .value(val(|r| r.source_port.clone()))
            .help(tr(K::FirewallPortHelp))
            .show_if(firewall_ports_apply)
            .half(),
        Field::text("destination_ip", tr(K::DestinationIp))
            .value(val(|r| r.destination_ip.clone()))
            .help(tr(K::FirewallAddressHelp))
            .half()
            .new_line(),
        Field::text("destination_port", tr(K::DestinationPort))
            .value(val(|r| r.destination_port.clone()))
            .help(tr(K::FirewallPortHelp))
            .show_if(firewall_ports_apply)
            .half(),
        Field::check("enabled", tr(K::EnabledCol), r.as_ref().is_none_or(|r| r.enabled)),
    ];
    let (title, submit) = if r.is_some() {
        (K::EditRule, K::Save)
    } else {
        (K::AddRule, K::AddRule)
    };
    let id = r.map(|r| r.id);
    Dialog::form(tr(title), tr(submit), fields, move |v| {
        let spec = FirewallRuleSpec {
            name: v.get("name"),
            description: v.get("description"),
            protocol: v.get("protocol"),
            action: v.get("action"),
            ip_version: if v.get("ip_version") == "6" { 6 } else { 4 },
            source_ip: v.get("source_ip"),
            source_port: v.get("source_port"),
            destination_ip: v.get("destination_ip"),
            destination_port: v.get("destination_port"),
            enabled: v.flag("enabled"),
        };
        // The same check the server makes, so the dialog says what is wrong.
        spec.body()?;
        Ok(match &id {
            Some(id) => OsAction::FirewallRuleUpdate { id: id.clone(), spec },
            None => OsAction::FirewallRuleCreate(spec),
        })
    })
    .sized(DialogSize::Large)
}

/// The page's *Add Rule* button.
pub fn create_firewall_rule_dialog(lang: Lang) -> Dialog {
    firewall_rule_dialog(lang, None)
}

/// A rule's menu; another project's shared rule has none.
pub fn firewall_rule_items(lang: Lang, r: &FirewallRuleRow) -> Vec<MenuItem> {
    if !r.owned {
        return vec![];
    }
    let rr = r.clone();
    let (i, n) = (r.id.clone(), r.name.clone());
    vec![
        MenuItem::dialog(t(lang, K::EditRule), move || firewall_rule_dialog(lang, Some(&rr))),
        MenuItem::danger(t(lang, K::DeleteRule), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeleteRule),
                n.clone(),
                OsAction::FirewallRuleDelete { id: i.clone() },
            )
        }),
    ]
}

/// *Add Policy*: its rules picked in evaluation order (*Allocated* order).
pub fn create_firewall_policy_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::text("name", tr(K::Name)).required(),
        Field::text("description", tr(K::Description)),
        Field::pick_many("rules", tr(K::Rules), Source::Set(ChoiceSet::FirewallRules)).help(tr(K::InsertRuleHelp)),
        Field::check("audited", tr(K::Audited), false),
    ];
    Dialog::form(tr(K::AddPolicy), tr(K::AddPolicy), fields, move |v| {
        Ok(OsAction::FirewallPolicyCreate {
            name: req(v, "name", K::Name, lang)?,
            description: v.get("description"),
            rule_ids: v.list("rules"),
            audited: v.flag("audited"),
        })
    })
    .sized(DialogSize::Large)
}

/// *Insert Rule*: a rule the policy doesn't have yet, at the top or after one
/// of its rules (Neutron's `insert_after`; *before* is the same as after the
/// previous one). Defaults to the end.
pub fn insert_firewall_rule_dialog(lang: Lang, p: &FirewallPolicyRow) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let mut places = vec![(String::new(), tr(K::AtTheTop))];
    places.extend(
        p.rules
            .iter()
            .map(|r| (r.id.clone(), format!("{} {}", t(lang, K::InsertAfter), r.name))),
    );
    let last = p.rules.last().map(|r| r.id.clone()).unwrap_or_default();
    let have: Vec<String> = p.rules.iter().map(|r| r.id.clone()).collect();
    let id = p.id.clone();
    let fields = vec![
        Field::select("rule", tr(K::Rule), Source::Without(ChoiceSet::FirewallRules, have)).required(),
        Field::select("after", tr(K::Position), Source::Static(places))
            .value(last)
            .help(tr(K::InsertRuleHelp)),
    ];
    Dialog::form(tr(K::InsertRule), tr(K::InsertRule), fields, move |v| {
        Ok(OsAction::FirewallPolicyInsertRule {
            id: id.clone(),
            rule_id: req(v, "rule", K::Rule, lang)?,
            before_id: None,
            after_id: v.opt("after"),
        })
    })
}

/// *Remove Rule* of one rule (from the policy page's rule table).
pub fn remove_firewall_rule_dialog(lang: Lang, policy_id: &str, rule: &Ref) -> Dialog {
    let action = OsAction::FirewallPolicyRemoveRule {
        id: policy_id.to_string(),
        rule_id: rule.id.clone(),
    };
    Dialog::confirm(lang, t(lang, K::RemoveRule), rule.name.clone(), action)
}

/// A policy's menu; another project's shared policy has none.
pub fn firewall_policy_items(lang: Lang, p: &FirewallPolicyRow) -> Vec<MenuItem> {
    if !p.owned {
        return vec![];
    }
    let tr = move |k| t(lang, k).to_string();
    let mut items = Vec::new();
    let pp = p.clone();
    items.push(MenuItem::dialog(tr(K::InsertRule), move || {
        insert_firewall_rule_dialog(lang, &pp)
    }));
    if !p.rules.is_empty() {
        let pp = p.clone();
        items.push(MenuItem::dialog(tr(K::RemoveRule), move || {
            let id = pp.id.clone();
            let rules = pp.rules.iter().map(|r| (r.id.clone(), r.name.clone())).collect();
            Dialog::form(
                tr(K::RemoveRule),
                tr(K::RemoveRule),
                vec![Field::select("rule", tr(K::Rule), Source::Static(rules)).required()],
                move |v| {
                    Ok(OsAction::FirewallPolicyRemoveRule {
                        id: id.clone(),
                        rule_id: req(v, "rule", K::Rule, lang)?,
                    })
                },
            )
        }));
    }
    if p.is_default() {
        // Neutron refuses to edit or (while the default group uses it) delete it.
        return items;
    }
    let pp = p.clone();
    items.push(MenuItem::dialog(tr(K::EditPolicy), move || {
        let id = pp.id.clone();
        let fields = vec![
            Field::text("name", tr(K::Name)).value(pp.name.clone()).required(),
            Field::text("description", tr(K::Description)).value(pp.description.clone().unwrap_or_default()),
            Field::check("audited", tr(K::Audited), pp.audited),
        ];
        Dialog::form(tr(K::EditPolicy), tr(K::Save), fields, move |v| {
            Ok(OsAction::FirewallPolicyUpdate {
                id: id.clone(),
                name: req(v, "name", K::Name, lang)?,
                description: v.get("description"),
                audited: v.flag("audited"),
            })
        })
    }));
    let (i, n) = (p.id.clone(), p.name.clone());
    items.push(MenuItem::danger(tr(K::DeletePolicy), move || {
        Dialog::confirm(
            lang,
            tr(K::DeletePolicy),
            n.clone(),
            OsAction::FirewallPolicyDelete { id: i.clone() },
        )
    }));
    items
}

/// The ingress / egress policy selects; empty: no filtering that way. The (i)
/// says which traffic each direction is: FWaaS sees it from the port's device,
/// so on a router interface *ingress* is the subnet's traffic going out.
fn firewall_policy_field(lang: Lang, name: &'static str, label: K, value: Option<&Ref>) -> Field {
    let tip = if label == K::IngressPolicy {
        K::TipIngressPolicy
    } else {
        K::TipEgressPolicy
    };
    Field::select(name, t(lang, label), Source::Set(ChoiceSet::FirewallPolicies))
        .empty_label(t(lang, K::NoPolicy))
        .value(value.map(|p| p.id.clone()).unwrap_or_default())
        .tip(t(lang, tip))
        .half()
}

/// *Create Firewall Group*: policies, the ports it protects (`FirewallPorts`
/// has only free ones: Neutron refuses a port another group has) and admin state.
pub fn create_firewall_group_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::text("name", tr(K::Name)),
        Field::text("description", tr(K::Description)),
        firewall_policy_field(lang, "ingress", K::IngressPolicy, None),
        firewall_policy_field(lang, "egress", K::EgressPolicy, None),
        Field::check("admin_up", tr(K::AdminStateUp), true),
        Field::pick_many("ports", tr(K::Ports), Source::Set(ChoiceSet::FirewallPorts)),
    ];
    let details = ["name", "description", "ingress", "egress", "admin_up"];
    // Two steps, the group first and then the ports it protects (optional: without
    // ports the group is INACTIVE until *Add Port*).
    let steps = vec![
        Step::new(tr(K::FirewallGroup), &details).required(),
        Step::new(tr(K::Ports), &["ports"]).help(tr(K::FirewallPortsHelp)),
    ];
    Dialog::form(
        tr(K::CreateFirewallGroup),
        tr(K::CreateFirewallGroup),
        fields,
        move |v| {
            Ok(OsAction::FirewallGroupCreate {
                name: v.get("name"),
                description: v.get("description"),
                ingress_policy_id: v.opt("ingress"),
                egress_policy_id: v.opt("egress"),
                port_ids: v.list("ports"),
                admin_up: v.flag("admin_up"),
            })
        },
    )
    .wizard(steps)
    .sized(DialogSize::Large)
}

/// *Add Port*: free router interface and instance ports, appended to the
/// group's list (Neutron takes the whole list).
pub fn add_firewall_port_dialog(lang: Lang, g: &FirewallGroupRow) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let (id, have): (String, Vec<String>) = (g.id.clone(), g.ports.iter().map(|p| p.port.id.clone()).collect());
    let fields = vec![
        Field::pick_many("ports", tr(K::Ports), Source::Set(ChoiceSet::FirewallPorts))
            .required()
            .help(tr(K::FirewallPortsHelp)),
    ];
    Dialog::form(tr(K::AddPort), tr(K::AddPort), fields, move |v| {
        let added = v.list("ports");
        if added.is_empty() {
            return Err(format!("{}: {}", t(lang, K::Ports), t(lang, K::RequiredField)));
        }
        Ok(OsAction::FirewallGroupSetPorts {
            id: id.clone(),
            port_ids: have
                .iter()
                .cloned()
                .chain(added.into_iter().filter(|p| !have.contains(p)))
                .collect(),
        })
    })
}

/// *Remove Port* of one port: the group's list without it.
pub fn remove_firewall_port_dialog(lang: Lang, g: &FirewallGroupRow, port: &FirewallPort) -> Dialog {
    let rest = g
        .ports
        .iter()
        .filter(|p| p.port.id != port.port.id)
        .map(|p| p.port.id.clone())
        .collect();
    let what = firewall_port_label(port);
    Dialog::confirm(
        lang,
        t(lang, K::RemovePort),
        what,
        OsAction::FirewallGroupSetPorts {
            id: g.id.clone(),
            port_ids: rest,
        },
    )
}

/// `device: IPs`, else the port's name.
pub fn firewall_port_label(p: &FirewallPort) -> String {
    let ips = if p.ips.is_empty() {
        String::new()
    } else {
        format!(" ({})", p.ips.join(", "))
    };
    match &p.device {
        Some(d) => format!("{}{ips}", d.name),
        None => format!("{}{ips}", p.port.name),
    }
}

/// A firewall group's menu; another project's shared group has none, and the
/// `default` group only *Add Port* / *Remove Port* (Neutron refuses the rest).
pub fn firewall_group_items(lang: Lang, g: &FirewallGroupRow) -> Vec<MenuItem> {
    if !g.owned {
        return vec![];
    }
    let tr = move |k| t(lang, k).to_string();
    let mut items = Vec::new();
    let gg = g.clone();
    let edit = MenuItem::dialog(tr(K::EditFirewallGroup), move || {
        let id = gg.id.clone();
        let fields = vec![
            Field::text("name", tr(K::Name)).value(gg.name.clone()),
            Field::text("description", tr(K::Description)).value(gg.description.clone().unwrap_or_default()),
            firewall_policy_field(lang, "ingress", K::IngressPolicy, gg.ingress.as_ref()),
            firewall_policy_field(lang, "egress", K::EgressPolicy, gg.egress.as_ref()),
            Field::check("admin_up", tr(K::AdminStateUp), gg.admin_up),
        ];
        Dialog::form(tr(K::EditFirewallGroup), tr(K::Save), fields, move |v| {
            Ok(OsAction::FirewallGroupUpdate {
                id: id.clone(),
                name: v.get("name"),
                description: v.get("description"),
                ingress_policy_id: v.opt("ingress"),
                egress_policy_id: v.opt("egress"),
                admin_up: v.flag("admin_up"),
            })
        })
    });
    if !g.is_default() {
        items.push(edit);
    }
    let gg = g.clone();
    items.push(MenuItem::dialog(tr(K::AddPort), move || {
        add_firewall_port_dialog(lang, &gg)
    }));
    if !g.ports.is_empty() {
        let gg = g.clone();
        items.push(MenuItem::dialog(tr(K::RemovePort), move || {
            let id = gg.id.clone();
            let ports: Vec<(String, String)> = gg
                .ports
                .iter()
                .map(|p| (p.port.id.clone(), firewall_port_label(p)))
                .collect();
            let all: Vec<String> = ports.iter().map(|(i, _)| i.clone()).collect();
            Dialog::form(
                tr(K::RemovePort),
                tr(K::RemovePort),
                vec![Field::select("port", tr(K::Port), Source::Static(ports)).required()],
                move |v| {
                    let gone = req(v, "port", K::Port, lang)?;
                    Ok(OsAction::FirewallGroupSetPorts {
                        id: id.clone(),
                        port_ids: all.iter().filter(|p| **p != gone).cloned().collect(),
                    })
                },
            )
        }));
    }
    if g.deletable() {
        let (i, n) = (g.id.clone(), g.name.clone());
        items.push(MenuItem::danger(tr(K::DeleteFirewallGroup), move || {
            Dialog::confirm(
                lang,
                tr(K::DeleteFirewallGroup),
                n.clone(),
                OsAction::FirewallGroupDelete { id: i.clone() },
            )
        }));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::components::dialog::Kind;

    #[test]
    fn firewall_dialogs() {
        let l = Lang::EnUs;
        let d = firewall_rule_dialog(l, None);
        assert!(!d.fields.iter().any(|f| f.name == "shared"), "never offered");
        let a = (d.build)(&vals(&[
            ("protocol", "tcp"),
            ("action", "deny"),
            ("ip_version", "6"),
            ("destination_port", "443"),
            ("enabled", "true"),
        ]))
        .unwrap();
        let OsAction::FirewallRuleCreate(spec) = a else {
            panic!("not a create")
        };
        assert_eq!((spec.ip_version, spec.destination_port.as_str()), (6, "443"));
        assert!(
            (d.build)(&vals(&[
                ("protocol", "icmp"),
                ("action", "allow"),
                ("source_port", "22")
            ]))
            .is_err(),
            "the server's check runs in the dialog"
        );
        let rule = FirewallRuleRow {
            id: "r1".into(),
            name: "r1".into(),
            description: None,
            protocol: None,
            ip_version: 4,
            source_ip: None,
            source_port: None,
            destination_ip: None,
            destination_port: None,
            action: "allow".into(),
            enabled: false,
            shared: true,
            policies: vec![],
            owned: true,
        };
        let e = firewall_rule_dialog(l, Some(&rule));
        let field = |n: &str| e.fields.iter().find(|f| f.name == n).unwrap().default.clone();
        assert_eq!(
            (field("name"), field("protocol"), field("enabled")),
            ("".into(), "any".into(), "false".into()),
            "unnamed: empty, not the ID"
        );
        assert!(
            !e.fields.iter().any(|f| f.name == "shared"),
            "not even for a shared rule"
        );
        assert!(
            firewall_rule_items(l, &FirewallRuleRow { owned: false, ..rule }).is_empty(),
            "another project's shared rule"
        );

        let r = |id: &str| Ref {
            id: id.into(),
            name: id.into(),
        };
        let p = FirewallPolicyRow {
            id: "fp1".into(),
            name: "web".into(),
            description: None,
            rules: vec![r("a"), r("b")],
            audited: false,
            shared: false,
            owned: true,
        };
        let ins = insert_firewall_rule_dialog(l, &p);
        assert!(
            matches!(&ins.fields[0].kind, Kind::Select(Source::Without(ChoiceSet::FirewallRules, have)) if have == &["a", "b"]),
            "rules it has are not offered"
        );
        assert_eq!(ins.fields[1].default, "b", "at the end by default");
        let a = (ins.build)(&vals(&[("rule", "c"), ("after", "")])).unwrap();
        assert_eq!(
            a,
            OsAction::FirewallPolicyInsertRule {
                id: "fp1".into(),
                rule_id: "c".into(),
                before_id: None,
                after_id: None
            },
            "at the top"
        );
        let labels: Vec<_> = firewall_policy_items(l, &p).into_iter().map(|i| i.label).collect();
        assert_eq!(labels, ["Insert Rule", "Remove Rule", "Edit Policy", "Delete Policy"]);

        let port = |id: &str| FirewallPort {
            port: r(id),
            ips: vec!["10.0.0.1".into()],
            kind: Some("router".into()),
            device: Some(r("router1")),
        };
        let g = FirewallGroupRow {
            id: "g1".into(),
            name: "gw".into(),
            description: None,
            ingress: None,
            egress: None,
            ports: vec![port("p1"), port("p2")],
            status: "ACTIVE".into(),
            admin_up: true,
            shared: false,
            owned: true,
        };
        let add = add_firewall_port_dialog(l, &g);
        assert_eq!(
            (add.build)(&vals(&[("ports", "p2,p3")])).unwrap(),
            OsAction::FirewallGroupSetPorts {
                id: "g1".into(),
                port_ids: vec!["p1".into(), "p2".into(), "p3".into()]
            },
            "the whole list, each once"
        );
        assert!(matches!(
            &add.fields[0].kind,
            Kind::Pick {
                src: Source::Set(ChoiceSet::FirewallPorts),
                ..
            }
        ));
        let rm = remove_firewall_port_dialog(l, &g, &g.ports[0]);
        assert_eq!(
            (rm.build)(&vals(&[])).unwrap(),
            OsAction::FirewallGroupSetPorts {
                id: "g1".into(),
                port_ids: vec!["p2".into()]
            }
        );
        assert_eq!(firewall_port_label(&g.ports[0]), "router1 (10.0.0.1)");
        let tips: Vec<_> = create_firewall_group_dialog(l)
            .fields
            .iter()
            .filter(|f| f.tip.is_some())
            .map(|f| f.name)
            .collect();
        assert_eq!(tips, ["ingress", "egress"], "the direction of each policy is explained");
        let d = create_firewall_group_dialog(l);
        let steps: Vec<_> = d.steps.iter().map(|s| s.fields.clone()).collect();
        assert_eq!(steps[1], ["ports"], "the port picker is its own step");
        assert!(steps[0].starts_with(&["name", "description", "ingress", "egress", "admin_up"]));

        // Neutron's `default` group and policies (neutron-fwaas matches them by name).
        let labels = |g: &FirewallGroupRow| {
            firewall_group_items(l, g)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(&g),
            [
                "Edit Firewall Group",
                "Add Port",
                "Remove Port",
                "Delete Firewall Group"
            ]
        );
        let def = FirewallGroupRow {
            name: "default".into(),
            ..g.clone()
        };
        assert_eq!(labels(&def), ["Add Port", "Remove Port"], "only its ports can change");
        assert!(!def.deletable() && g.deletable() && !FirewallGroupRow { owned: false, ..g }.deletable());
        let dp = FirewallPolicyRow {
            name: "default ingress".into(),
            ..p.clone()
        };
        let labels: Vec<_> = firewall_policy_items(l, &dp).into_iter().map(|i| i.label).collect();
        assert_eq!(
            labels,
            ["Insert Rule", "Remove Rule"],
            "its rules can change, the policy can't"
        );
        assert!(!dp.deletable() && p.deletable());
    }
}
