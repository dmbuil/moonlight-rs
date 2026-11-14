//! Network QoS policies and rules.

use super::*;

/// *Create QoS Policy*: name, description and whether it is shared.
pub fn create_qos_policy_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::CreatePolicy),
        t(lang, K::CreatePolicy),
        vec![
            Field::text("name", t(lang, K::Name)).required(),
            Field::text("description", t(lang, K::Description)),
            Field::check("shared", t(lang, K::Shared), false),
        ],
        move |v| {
            Ok(OsAction::QosPolicyCreate {
                name: req(v, "name", K::Name, lang)?,
                description: v.opt("description"),
                shared: v.flag("shared"),
            })
        },
    )
}

/// Only the project's own policies can be changed; shared ones are read-only.
pub fn qos_policy_items(lang: Lang, p: &QosPolicyRow) -> Vec<MenuItem> {
    if !p.owned {
        return vec![];
    }
    let pp = p.clone();
    let (i, n) = (p.id.clone(), p.name.clone());
    vec![
        MenuItem::dialog(t(lang, K::AddRule), move || qos_rule_dialog(lang, &pp.id, None)),
        {
            let pp = p.clone();
            MenuItem::dialog(t(lang, K::EditPolicy), move || {
                let id = pp.id.clone();
                Dialog::form(
                    t(lang, K::EditPolicy),
                    t(lang, K::Save),
                    vec![
                        Field::text("name", t(lang, K::Name)).value(pp.name.clone()).required(),
                        Field::text("description", t(lang, K::Description))
                            .value(pp.description.clone().unwrap_or_default()),
                        Field::check("shared", t(lang, K::Shared), pp.shared),
                    ],
                    move |v| {
                        Ok(OsAction::QosPolicyUpdate {
                            id: id.clone(),
                            name: req(v, "name", K::Name, lang)?,
                            description: v.get("description"),
                            shared: v.flag("shared"),
                        })
                    },
                )
            })
        },
        MenuItem::danger(t(lang, K::DeletePolicy), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeletePolicy),
                n.clone(),
                OsAction::QosPolicyDelete { id: i.clone() },
            )
        }),
    ]
}

/// A QoS rule type (`bandwidth_limit`…) as the user reads it.
fn qos_kind_label(lang: Lang, kind: &str) -> String {
    t(
        lang,
        match kind {
            "bandwidth_limit" => K::BandwidthLimit,
            "dscp_marking" => K::DscpMarking,
            "minimum_bandwidth" => K::MinimumBandwidth,
            _ => K::MinimumPacketRate,
        },
    )
    .to_string()
}

/// *Add Rule* / *Edit Rule*: the fields follow the rule type (Neutron's
/// `bandwidth_limit`, `dscp_marking`, `minimum_bandwidth`, `minimum_packet_rate`).
pub fn qos_rule_dialog(lang: Lang, policy_id: &str, rule: Option<&QosRule>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let kinds = Source::Static(
        QOS_RULE_TYPES
            .iter()
            .map(|k| (k.to_string(), qos_kind_label(lang, k)))
            .collect(),
    );
    let num = |n: Option<i64>| n.map(|n| n.to_string()).unwrap_or_default();
    let kind = rule.map(|r| r.kind.clone()).unwrap_or_else(|| "bandwidth_limit".into());
    let mut fields = vec![];
    if rule.is_none() {
        fields.push(Field::select("kind", tr(K::Type), kinds).value(kind.clone()).required());
    }
    // On edit the type is fixed: show only its fields.
    let is = |k: &'static str| -> Box<dyn Fn(Field) -> Field> {
        match rule {
            Some(r) if r.kind == k => Box::new(|f| f),
            Some(_) => Box::new(|f: Field| f.show_if(|_| false)),
            None => Box::new(move |f: Field| f.when("kind", k)),
        }
    };
    let directional: fn(&Vals) -> bool = |v| v.get("kind").is_some_and(|k| !QosRuleSpec::directions(k).is_empty());
    let dir_options = |k: &str| {
        options_raw(if k.is_empty() {
            &["egress", "ingress", "any"]
        } else {
            QosRuleSpec::directions(k)
        })
    };
    let direction = Field::select(
        "direction",
        tr(K::Direction),
        dir_options(if rule.is_some() { &kind } else { "" }),
    )
    .value(
        rule.and_then(|r| r.direction.clone())
            .unwrap_or_else(|| "egress".into()),
    );
    fields.push(match rule {
        Some(r) if QosRuleSpec::directions(&r.kind).is_empty() => direction.show_if(|_| false),
        Some(_) => direction,
        None => direction.show_if(directional).help(tr(K::AnyOnlyPacketRate)),
    });
    fields.extend([
        is("bandwidth_limit")(
            Field::number("max_kbps", tr(K::MaxKbps), 1)
                .value(num(rule.and_then(|r| r.max_kbps)))
                .required(),
        ),
        is("bandwidth_limit")(
            Field::number("max_burst_kbps", tr(K::MaxBurstKbps), 0).value(num(rule.and_then(|r| r.max_burst_kbps))),
        ),
        is("dscp_marking")(
            Field::select(
                "dscp_mark",
                tr(K::DscpMark),
                options_raw(
                    &DSCP_MARKS
                        .map(|m| m.to_string())
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                ),
            )
            .value(num(rule.and_then(|r| r.dscp_mark)))
            .required(),
        ),
        is("minimum_bandwidth")(
            Field::number("min_kbps", tr(K::MinKbps), 1)
                .value(num(rule.and_then(|r| r.min_kbps)))
                .required(),
        ),
        is("minimum_packet_rate")(
            Field::number("min_kpps", tr(K::MinKpps), 1)
                .value(num(rule.and_then(|r| r.min_kpps)))
                .required(),
        ),
    ]);
    let (pid, id) = (policy_id.to_string(), rule.map(|r| r.id.clone()));
    let fixed = rule.map(|r| r.kind.clone());
    let title = tr(if rule.is_some() { K::EditRule } else { K::AddRule });
    Dialog::form(title.clone(), title, fields, move |v| {
        let kind = fixed.clone().unwrap_or_else(|| v.get("kind"));
        let n = |k: &str, label| num_of::<u32>(v, k, label, lang);
        let spec = QosRuleSpec {
            direction: v
                .opt("direction")
                .filter(|_| !QosRuleSpec::directions(&kind).is_empty()),
            max_kbps: n("max_kbps", K::MaxKbps)?.filter(|_| kind == "bandwidth_limit"),
            max_burst_kbps: n("max_burst_kbps", K::MaxBurstKbps)?.filter(|_| kind == "bandwidth_limit"),
            dscp_mark: n("dscp_mark", K::DscpMark)?
                .and_then(|m| u8::try_from(m).ok())
                .filter(|_| kind == "dscp_marking"),
            min_kbps: n("min_kbps", K::MinKbps)?.filter(|_| kind == "minimum_bandwidth"),
            min_kpps: n("min_kpps", K::MinKpps)?.filter(|_| kind == "minimum_packet_rate"),
            kind,
        };
        spec.body().map_err(|e| invalid(lang, e))?;
        Ok(match &id {
            Some(id) => OsAction::QosRuleUpdate {
                policy_id: pid.clone(),
                id: id.clone(),
                spec,
            },
            None => OsAction::QosRuleCreate {
                policy_id: pid.clone(),
                spec,
            },
        })
    })
}

/// A QoS rule's menu (edit, delete).
pub fn qos_rule_items(lang: Lang, policy_id: &str, r: &QosRule) -> Vec<MenuItem> {
    let (p, rule) = (policy_id.to_string(), r.clone());
    let edit = MenuItem::dialog(t(lang, K::EditRule), move || qos_rule_dialog(lang, &p, Some(&rule)));
    let (p, kind, id) = (policy_id.to_string(), r.kind.clone(), r.id.clone());
    let what = qos_kind_label(lang, &r.kind);
    let delete = MenuItem::danger(t(lang, K::DeleteRule), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteRule),
            what.clone(),
            OsAction::QosRuleDelete {
                policy_id: p.clone(),
                kind: kind.clone(),
                id: id.clone(),
            },
        )
    });
    vec![edit, delete]
}

/// The localised name of a QoS rule type (`bandwidth_limit` → *Bandwidth Limit*).
pub fn qos_kind(lang: Lang, kind: &str) -> String {
    qos_kind_label(lang, kind)
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[test]
    fn qos_rule_dialog_sends_only_the_fields_of_its_type() {
        let d = qos_rule_dialog(Lang::EnUs, "q1", None);
        let a = (d.build)(&vals(&[
            ("kind", "dscp_marking"),
            ("dscp_mark", "26"),
            ("direction", "egress"),
            ("max_kbps", "5"),
        ]))
        .unwrap();
        let OsAction::QosRuleCreate { policy_id, spec } = a else {
            panic!("not a create")
        };
        assert_eq!(
            (
                policy_id.as_str(),
                spec.dscp_mark,
                spec.direction.clone(),
                spec.max_kbps
            ),
            ("q1", Some(26), None, None)
        );
        assert!(
            (d.build)(&vals(&[("kind", "bandwidth_limit"), ("direction", "egress")])).is_err(),
            "max_kbps is required"
        );
        let rule = QosRule {
            id: "r1".into(),
            kind: "minimum_bandwidth".into(),
            direction: Some("ingress".into()),
            max_kbps: None,
            max_burst_kbps: None,
            dscp_mark: None,
            min_kbps: Some(100),
            min_kpps: None,
        };
        let e = qos_rule_dialog(Lang::EnUs, "q1", Some(&rule));
        assert!(e.fields.iter().all(|f| f.name != "kind"), "the type is fixed on edit");
        let a = (e.build)(&vals(&[("direction", "egress"), ("min_kbps", "200")])).unwrap();
        assert!(
            matches!(a, OsAction::QosRuleUpdate { ref id, ref spec, .. } if id == "r1" && spec.min_kbps == Some(200) && spec.kind == "minimum_bandwidth")
        );
        // No `qos` plugin in a test context: the router and floating IP dialogs have no QoS field.
        assert!(create_router_dialog(Lang::EnUs).fields.iter().all(|f| f.name != "qos"));
        assert!(allocate_ip_dialog(Lang::EnUs).fields.iter().all(|f| f.name != "qos"));
    }
}
