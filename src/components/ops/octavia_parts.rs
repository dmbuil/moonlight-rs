//! Octavia listeners, pools, members, health monitors and L7 policies.

use super::*;

/// `(id, name)` pairs of `items`, for a `Source::Static` select.
fn refs_of<T>(items: &[T], f: impl Fn(&T) -> (String, String)) -> Vec<(String, String)> {
    items.iter().map(f).collect()
}

/// *Create Listener* / *Edit Listener*; `pools` of this load balancer for the default pool.
pub fn listener_dialog(lang: Lang, d: &LoadBalancerDetail, existing: Option<&LbListener>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let pools = refs_of(&d.pools, |p| (p.id.clone(), p.name.clone()));
    let lb = d.row.id.clone();
    let limit = existing
        .and_then(|l| l.connection_limit)
        .map(|n| n.to_string())
        .unwrap_or_default();
    let mut fields = vec![];
    if existing.is_none() {
        fields.push(
            Field::select("protocol", tr(K::Protocol), listener_protocols())
                .value("HTTP")
                .required(),
        );
        fields.push(Field::number("port", tr(K::Port), 1).value("80").required());
    }
    match existing {
        None => fields.extend(listener_certificate_fields(lang, None, Some(dialog_tls))),
        Some(l) if l.protocol == LB_TLS_PROTOCOL => fields.extend(listener_certificate_fields(lang, Some(l), None)),
        Some(_) => {}
    }
    fields.extend([
        Field::text("name", tr(K::Name)).value(existing.map(|l| l.name.clone()).unwrap_or_default()),
        Field::text("description", tr(K::Description))
            .value(existing.and_then(|l| l.description.clone()).unwrap_or_default()),
        Field::number("connection_limit", tr(K::ConnectionLimit), 1)
            .value(limit)
            .help(tr(K::UnlimitedWhenEmpty)),
        Field::select("default_pool", tr(K::DefaultPool), static_refs(&pools)).value(
            existing
                .and_then(|l| l.default_pool.as_ref().map(|p| p.id.clone()))
                .unwrap_or_default(),
        ),
    ]);
    // Also on a new listener.
    fields.push(Field::check(
        "admin_up",
        tr(K::EnableAdminState),
        existing.is_none_or(|l| l.admin_up),
    ));
    type Cond = Option<fn(&Vals) -> bool>;
    let always: fn(&Vals) -> bool = |_| true;
    let (headers_if, ciphers_if): (Cond, Cond) = match existing {
        None => (Some(dialog_headers), Some(dialog_tls)),
        Some(l) => (
            lb_inserts_headers(&l.protocol).then_some(always),
            (l.protocol == LB_TLS_PROTOCOL).then_some(always),
        ),
    };
    fields.extend(listener_option_fields(
        lang,
        existing.map(|l| &l.options),
        headers_if,
        ciphers_if,
        None,
    ));
    let id = existing.map(|l| (l.id.clone(), l.protocol.clone()));
    let title = tr(if existing.is_some() {
        K::EditListener
    } else {
        K::CreateListener
    });
    Dialog::form(title.clone(), title, fields, move |v| {
        let connection_limit = num_of::<u32>(v, "connection_limit", K::ConnectionLimit, lang)?;
        let options = listener_options_of(v, lang)?;
        Ok(match &id {
            Some((id, protocol)) => {
                if let Some(o) = &options {
                    o.check(protocol).map_err(|e| invalid(lang, e))?;
                }
                OsAction::ListenerUpdate {
                    id: id.clone(),
                    name: v.get("name"),
                    description: v.get("description"),
                    connection_limit,
                    default_pool_id: v.opt("default_pool"),
                    admin_up: v.flag("admin_up"),
                    protocol: protocol.clone(),
                    options,
                    tls: (protocol == LB_TLS_PROTOCOL).then(|| (v.get("certificate"), v.list("sni"))),
                }
            }
            None => {
                let spec = LbListenerSpec {
                    name: v.opt("name"),
                    description: v.opt("description"),
                    protocol: v.get("protocol"),
                    port: num_of::<u16>(v, "port", K::Port, lang)?.unwrap_or(0),
                    connection_limit,
                    options: options.unwrap_or_default(),
                    default_tls_container_ref: v.opt("certificate").filter(|_| v.get("protocol") == LB_TLS_PROTOCOL),
                    sni_container_refs: if v.get("protocol") == LB_TLS_PROTOCOL {
                        v.list("sni")
                    } else {
                        vec![]
                    },
                    admin_up: v.flag("admin_up"),
                };
                spec.check().map_err(|e| invalid(lang, e))?;
                OsAction::ListenerCreate {
                    loadbalancer_id: lb.clone(),
                    spec,
                    default_pool_id: v.opt("default_pool"),
                }
            }
        })
    })
    .on_change(|f, v, _| suggest_port(f, v, "protocol", "port"))
}

/// *Create Pool* / *Edit Pool*. A new pool can become the default pool of a
/// listener that has none.
pub fn pool_dialog(lang: Lang, d: &LoadBalancerDetail, existing: Option<&LbPool>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let free: Vec<_> = d
        .listeners
        .iter()
        .filter(|l| l.default_pool.is_none())
        .map(|l| {
            (
                l.id.clone(),
                format!("{} ({} {})", l.name, l.protocol, l.port.unwrap_or(0)),
            )
        })
        .collect();
    let lb = d.row.id.clone();
    let mut fields = vec![Field::text("name", tr(K::Name)).value(existing.map(|p| p.name.clone()).unwrap_or_default())];
    fields.push(
        Field::text("description", tr(K::Description))
            .value(existing.and_then(|p| p.description.clone()).unwrap_or_default()),
    );
    if existing.is_none() {
        fields.push(Field::select("listener", tr(K::Listener), static_refs(&free)).help(tr(K::PoolListenerHelp)));
        fields.push(
            Field::select("protocol", tr(K::Protocol), options_raw(&LB_PROTOCOLS))
                .value("HTTP")
                .required(),
        );
    }
    fields.extend([
        Field::select("algorithm", tr(K::Algorithm), options_raw(&LB_ALGORITHMS))
            .value(existing.map_or("ROUND_ROBIN".into(), |p| p.algorithm.clone()))
            .required(),
        Field::select("persistence", tr(K::SessionPersistence), options_raw(&LB_PERSISTENCE))
            .value(existing.and_then(|p| p.persistence_type.clone()).unwrap_or_default()),
        Field::text("cookie_name", tr(K::CookieName))
            .value(existing.and_then(|p| p.cookie_name.clone()).unwrap_or_default())
            .when("persistence", "APP_COOKIE"),
    ]);
    fields.extend(pool_tls_fields(lang, existing, None));
    // Also on a new pool.
    fields.push(Field::check(
        "admin_up",
        tr(K::EnableAdminState),
        existing.is_none_or(|p| p.admin_up),
    ));
    let id = existing.map(|p| (p.id.clone(), p.protocol.clone()));
    let title = tr(if existing.is_some() { K::EditPool } else { K::CreatePool });
    Dialog::form(title.clone(), title, fields, move |v| {
        let (session_persistence, cookie_name) = persistence_of(v);
        let tls = pool_tls_of(v);
        Ok(match &id {
            Some((id, protocol)) => {
                let (on, ciphers) = tls.clone().unwrap_or_default();
                check_pool_tls(protocol, on, ciphers.as_deref()).map_err(|e| invalid(lang, e))?;
                OsAction::PoolUpdate {
                    id: id.clone(),
                    name: v.get("name"),
                    description: v.get("description"),
                    algorithm: v.get("algorithm"),
                    session_persistence,
                    cookie_name,
                    admin_up: v.flag("admin_up"),
                    protocol: protocol.clone(),
                    tls,
                }
            }
            None => {
                let (tls_enabled, tls_ciphers) = tls.unwrap_or_default();
                let spec = LbPoolSpec {
                    name: v.opt("name"),
                    description: v.opt("description"),
                    protocol: v.get("protocol"),
                    algorithm: v.get("algorithm"),
                    session_persistence,
                    cookie_name,
                    tls_enabled,
                    tls_ciphers,
                    admin_up: v.flag("admin_up"),
                };
                spec.check().map_err(|e| invalid(lang, e))?;
                OsAction::PoolCreate {
                    loadbalancer_id: lb.clone(),
                    listener_id: v.opt("listener"),
                    spec,
                }
            }
        })
    })
}

/// *Add Member* (an instance address, or any address on a subnet, into one of
/// this load balancer's pools) / *Edit Member*. Both set the monitor address
/// and port.
pub fn member_dialog(lang: Lang, d: &LoadBalancerDetail, existing: Option<&LbMember>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let pools = refs_of(&d.pools, |p| (p.id.clone(), p.name.clone()));
    let mut fields = vec![];
    if existing.is_none() {
        fields.push(
            Field::select("pool", tr(K::Pool), static_refs(&pools))
                .value(pools.first().map(|p| p.0.clone()).unwrap_or_default())
                .required(),
        );
        let sources = [("instance", K::Instance), ("external", K::ExternalAddress)];
        fields.push(
            Field::select("source", tr(K::MemberSource), options(&sources, lang))
                .value("instance")
                .required(),
        );
        fields.push(
            Field::select("target", tr(K::IpAddress), Source::Set(ChoiceSet::LbMemberTargets))
                .required()
                .when("source", "instance"),
        );
        fields.push(
            Field::text("address", tr(K::IpAddress))
                .required()
                .when("source", "external"),
        );
        fields.push(
            Field::select("subnet", tr(K::Subnet), Source::Set(ChoiceSet::LbMemberSubnets))
                .required()
                .when("source", "external"),
        );
        fields.push(Field::number("port", tr(K::MemberPort), 1).value("80").required());
    }
    fields.extend([
        Field::text("name", tr(K::Name)).value(
            existing
                .map(|m| if m.name == m.id { String::new() } else { m.name.clone() })
                .unwrap_or_default(),
        ),
        Field::number("weight", tr(K::Weight), 0)
            .value(existing.and_then(|m| m.weight).unwrap_or(1).to_string())
            .required(),
        Field::check("backup", tr(K::Backup), existing.is_some_and(|m| m.backup)),
        Field::text("monitor_address", tr(K::MonitorAddress))
            .value(existing.and_then(|m| m.monitor_address.clone()).unwrap_or_default())
            .help(tr(K::MonitorAddressHelp)),
        Field::number("monitor_port", tr(K::MonitorPort), 1).value(
            existing
                .and_then(|m| m.monitor_port)
                .map(|p| p.to_string())
                .unwrap_or_default(),
        ),
        Field::check("admin_up", tr(K::EnableAdminState), existing.is_none_or(|m| m.admin_up)),
    ]);
    let ids = existing.map(|m| (m.pool.id.clone(), m.id.clone()));
    let title = tr(if existing.is_some() {
        K::EditMember
    } else {
        K::AddMember
    });
    Dialog::form(title.clone(), title, fields, move |v| {
        let weight = num_of::<u16>(v, "weight", K::Weight, lang)?.unwrap_or(1);
        let monitor_port = num_of::<u16>(v, "monitor_port", K::MonitorPort, lang)?;
        Ok(match &ids {
            Some((pool, id)) => OsAction::MemberUpdate {
                pool_id: pool.clone(),
                id: id.clone(),
                name: v.get("name"),
                weight,
                backup: v.flag("backup"),
                admin_up: v.flag("admin_up"),
                monitor_address: v.opt("monitor_address"),
                monitor_port,
            },
            None => {
                let (address, subnet) = if v.get("source") == "external" {
                    (req(v, "address", K::IpAddress, lang)?, v.opt("subnet"))
                } else {
                    let target = req(v, "target", K::IpAddress, lang)?;
                    target
                        .split_once('@')
                        .map(|(a, s)| (a.to_string(), Some(s.to_string())))
                        .unwrap_or((target.clone(), None))
                };
                let spec = LbMemberSpec {
                    name: v.opt("name"),
                    address,
                    port: num_of::<u16>(v, "port", K::MemberPort, lang)?.unwrap_or(0),
                    subnet_id: subnet,
                    weight: Some(weight),
                    backup: v.flag("backup"),
                    monitor_address: v.opt("monitor_address"),
                    monitor_port,
                    admin_up: v.flag("admin_up"),
                };
                spec.check().map_err(|e| invalid(lang, e))?;
                OsAction::MemberCreate {
                    pool_id: req(v, "pool", K::Pool, lang)?,
                    spec,
                }
            }
        })
    })
}

/// *Edit Members* of a pool: the whole list at once, sent as Octavia's batch
/// update, which deletes the members left out; so it is destructive, and the
/// editor lists the members a save would delete.
pub fn pool_members_dialog(lang: Lang, d: &LoadBalancerDetail, pool: &LbPool) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let current: Vec<LbMember> = d.members.iter().filter(|m| m.pool.id == pool.id).cloned().collect();
    let rows: Vec<MemberRow> = current.iter().map(MemberRow::from_member).collect();
    let pool_id = pool.id.clone();
    let fields = vec![Field::member_rows("members", tr(K::Members), &rows, None, current).help(tr(K::MembersHelp))];
    let mut dialog = Dialog::form(
        format!("{}: {}", tr(K::EditMembers), pool.name),
        tr(K::EditMembers),
        fields,
        move |v| {
            let members = member_specs(&v.get("members")).map_err(|e| invalid(lang, e))?;
            Ok(OsAction::PoolSetMembers {
                pool_id: pool_id.clone(),
                members,
            })
        },
    )
    .sized(DialogSize::Large);
    dialog.intro = vec![tr(K::EditMembersIntro)];
    dialog.danger = true;
    dialog
}

/// *Create Health Monitor* (for a pool without one) / *Edit Health Monitor*.
pub fn monitor_dialog(lang: Lang, d: &LoadBalancerDetail, existing: Option<&LbMonitor>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let free: Vec<_> = d
        .pools
        .iter()
        .filter(|p| p.monitor.is_none())
        .map(|p| (p.id.clone(), p.name.clone()))
        .collect();
    let mut fields = vec![];
    if existing.is_none() {
        fields.push(
            Field::select("pool", tr(K::Pool), static_refs(&free))
                .value(free.first().map(|p| p.0.clone()).unwrap_or_default())
                .required(),
        );
    }
    fields.extend(monitor_fields(lang, None, existing));
    if let Some(m) = existing {
        fields.push(Field::check("admin_up", tr(K::EnableAdminState), m.admin_up));
    }
    let edit = existing.map(|m| (m.id.clone(), m.kind.clone()));
    let title = tr(if existing.is_some() {
        K::EditHealthMonitor
    } else {
        K::CreateHealthMonitor
    });
    Dialog::form(title.clone(), title, fields, move |v| {
        Ok(match &edit {
            Some((id, kind)) => {
                let spec = monitor_spec(v, lang, kind.clone())?;
                spec.check().map_err(|e| invalid(lang, e))?;
                OsAction::MonitorUpdate {
                    id: id.clone(),
                    spec,
                    admin_up: v.flag("admin_up"),
                }
            }
            None => {
                let spec = monitor_spec(v, lang, v.get("monitor_type"))?;
                spec.check().map_err(|e| invalid(lang, e))?;
                OsAction::MonitorCreate {
                    pool_id: req(v, "pool", K::Pool, lang)?,
                    spec,
                }
            }
        })
    })
}

/// A listener's menu (edit, delete).
pub fn listener_items(lang: Lang, d: &LoadBalancerDetail, l: &LbListener) -> Vec<MenuItem> {
    let (dd, ll) = (d.clone(), l.clone());
    let (i, n) = (l.id.clone(), l.name.clone());
    vec![
        MenuItem::dialog(t(lang, K::EditListener), move || listener_dialog(lang, &dd, Some(&ll))),
        MenuItem::danger(t(lang, K::DeleteListener), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeleteListener),
                n.clone(),
                OsAction::ListenerDelete { id: i.clone() },
            )
        }),
    ]
}

/// A pool's menu (edit, delete).
pub fn pool_items(lang: Lang, d: &LoadBalancerDetail, p: &LbPool) -> Vec<MenuItem> {
    let (dd, pp) = (d.clone(), p.clone());
    let (i, n) = (p.id.clone(), p.name.clone());
    vec![
        MenuItem::dialog(t(lang, K::EditPool), move || pool_dialog(lang, &dd, Some(&pp))),
        MenuItem::dialog(t(lang, K::EditMembers), {
            let (dd, pp) = (d.clone(), p.clone());
            move || pool_members_dialog(lang, &dd, &pp)
        }),
        MenuItem::danger(t(lang, K::DeletePool), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeletePool),
                n.clone(),
                OsAction::PoolDelete { id: i.clone() },
            )
        }),
    ]
}

/// A member's menu (edit, delete).
pub fn member_items(lang: Lang, d: &LoadBalancerDetail, m: &LbMember) -> Vec<MenuItem> {
    let (dd, mm) = (d.clone(), m.clone());
    let (pool, i) = (m.pool.id.clone(), m.id.clone());
    let what = format!("{}:{}", m.address, m.port.unwrap_or(0));
    vec![
        MenuItem::dialog(t(lang, K::EditMember), move || member_dialog(lang, &dd, Some(&mm))),
        MenuItem::danger(t(lang, K::DeleteMember), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeleteMember),
                what.clone(),
                OsAction::MemberDelete {
                    pool_id: pool.clone(),
                    id: i.clone(),
                },
            )
        }),
    ]
}

/// A health monitor's menu (edit, delete).
pub fn monitor_items(lang: Lang, d: &LoadBalancerDetail, m: &LbMonitor) -> Vec<MenuItem> {
    let (dd, mm) = (d.clone(), m.clone());
    let (i, n) = (m.id.clone(), m.name.clone());
    vec![
        MenuItem::dialog(t(lang, K::EditHealthMonitor), move || {
            monitor_dialog(lang, &dd, Some(&mm))
        }),
        MenuItem::danger(t(lang, K::DeleteHealthMonitor), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeleteHealthMonitor),
                n.clone(),
                OsAction::MonitorDelete { id: i.clone() },
            )
        }),
    ]
}

/// Listeners that can take L7 policies: Octavia applies them to HTTP and
/// `TERMINATED_HTTPS` listeners only.
pub fn l7_listeners(d: &LoadBalancerDetail) -> Vec<(String, String)> {
    d.listeners
        .iter()
        .filter(|l| lb_inserts_headers(&l.protocol))
        .map(|l| {
            (
                l.id.clone(),
                format!("{} ({} {})", l.name, l.protocol, l.port.unwrap_or(0)),
            )
        })
        .collect()
}

/// *Create L7 Policy* / *Edit L7 Policy*, as octavia-dashboard
/// has it: the redirect pool or URL shows for its action only.
pub fn l7_policy_dialog(lang: Lang, d: &LoadBalancerDetail, existing: Option<&LbL7Policy>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let pools = refs_of(&d.pools, |p| (p.id.clone(), p.name.clone()));
    let mut fields = vec![];
    if existing.is_none() {
        fields.push(Field::select("listener", tr(K::Listener), static_refs(&l7_listeners(d))).required());
    }
    fields.extend([
        Field::text("name", tr(K::Name)).value(existing.map(|p| p.name.clone()).unwrap_or_default()),
        Field::text("description", tr(K::Description))
            .value(existing.and_then(|p| p.description.clone()).unwrap_or_default()),
        Field::select("action", tr(K::Action), options_raw(&LB_L7_ACTIONS))
            .value(existing.map_or("REDIRECT_TO_POOL".into(), |p| p.action.clone()))
            .required(),
        Field::select("redirect_pool", tr(K::RedirectPool), static_refs(&pools))
            .value(
                existing
                    .and_then(|p| p.redirect_pool.as_ref().map(|r| r.id.clone()))
                    .unwrap_or_default(),
            )
            .required()
            .when("action", "REDIRECT_TO_POOL"),
        Field::text("redirect_url", tr(K::RedirectUrl))
            .value(existing.and_then(|p| p.redirect_url.clone()).unwrap_or_default())
            .required()
            .when("action", "REDIRECT_TO_URL"),
        Field::number("position", tr(K::Position), 1)
            .value(
                existing
                    .and_then(|p| p.position)
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
            )
            .help(tr(K::L7PositionHelp)),
        Field::check("admin_up", tr(K::EnableAdminState), existing.is_none_or(|p| p.admin_up)),
    ]);
    let id = existing.map(|p| p.id.clone());
    let title = tr(if existing.is_some() {
        K::EditL7Policy
    } else {
        K::CreateL7Policy
    });
    Dialog::form(title.clone(), title, fields, move |v| {
        let spec = LbL7PolicySpec {
            name: v.opt("name"),
            description: v.opt("description"),
            action: req(v, "action", K::Action, lang)?,
            position: num_of::<u32>(v, "position", K::Position, lang)?,
            redirect_pool_id: v.opt("redirect_pool"),
            redirect_url: v.opt("redirect_url"),
            admin_up: v.flag("admin_up"),
        };
        spec.check().map_err(|e| invalid(lang, e))?;
        Ok(match &id {
            Some(id) => OsAction::L7PolicyUpdate { id: id.clone(), spec },
            None => OsAction::L7PolicyCreate {
                listener_id: req(v, "listener", K::Listener, lang)?,
                spec,
            },
        })
    })
}

/// *Add L7 Rule* / *Edit L7 Rule*. `policy`: the policy it goes
/// to (from the policy's menu, or an edit); `None` offers a choice.
pub fn l7_rule_dialog(
    lang: Lang,
    d: &LoadBalancerDetail,
    policy: Option<&LbL7Policy>,
    existing: Option<&LbL7Rule>,
) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let mut fields = vec![];
    if policy.is_none() {
        let policies = refs_of(&d.l7policies, |p| {
            (p.id.clone(), format!("{} ({})", p.name, p.listener.name))
        });
        fields.push(Field::select("policy", tr(K::L7Policy), static_refs(&policies)).required());
    }
    let keyed: fn(&Vals) -> bool = |v| matches!(v.get("rule_type").map(String::as_str), Some("HEADER" | "COOKIE"));
    fields.extend([
        Field::select("rule_type", tr(K::RuleType), options_raw(&LB_L7_RULE_TYPES))
            .value(existing.map_or("PATH".into(), |r| r.rule_type.clone()))
            .required(),
        Field::select("compare_type", tr(K::CompareType), options_raw(&LB_L7_COMPARE_TYPES))
            .value(existing.map_or("STARTS_WITH".into(), |r| r.compare_type.clone()))
            .required(),
        Field::text("key", tr(K::L7Key))
            .value(existing.and_then(|r| r.key.clone()).unwrap_or_default())
            .help(tr(K::L7KeyHelp))
            .required()
            .show_if(keyed),
        Field::text("value", tr(K::Value))
            .value(existing.map(|r| r.value.clone()).unwrap_or_default())
            .required(),
        Field::check("invert", tr(K::Invert), existing.is_some_and(|r| r.invert)).help(tr(K::InvertHelp)),
        Field::check("admin_up", tr(K::EnableAdminState), existing.is_none_or(|r| r.admin_up)),
    ]);
    let (policy_id, id) = (policy.map(|p| p.id.clone()), existing.map(|r| r.id.clone()));
    let title = tr(if existing.is_some() {
        K::EditL7Rule
    } else {
        K::AddL7Rule
    });
    Dialog::form(title.clone(), title, fields, move |v| {
        let rule_type = req(v, "rule_type", K::RuleType, lang)?;
        let keyed = matches!(rule_type.as_str(), "HEADER" | "COOKIE");
        let spec = LbL7RuleSpec {
            compare_type: req(v, "compare_type", K::CompareType, lang)?,
            key: v.opt("key").filter(|_| keyed),
            value: req(v, "value", K::Value, lang)?,
            invert: v.flag("invert"),
            admin_up: v.flag("admin_up"),
            rule_type,
        };
        spec.check().map_err(|e| invalid(lang, e))?;
        let policy_id = match &policy_id {
            Some(p) => p.clone(),
            None => req(v, "policy", K::L7Policy, lang)?,
        };
        Ok(match &id {
            Some(id) => OsAction::L7RuleUpdate {
                policy_id,
                id: id.clone(),
                spec,
            },
            None => OsAction::L7RuleCreate { policy_id, spec },
        })
    })
}

/// An L7 policy's menu: edit, add a rule, delete (its rules go with it).
pub fn l7_policy_items(lang: Lang, d: &LoadBalancerDetail, p: &LbL7Policy) -> Vec<MenuItem> {
    let (d1, p1, d2, p2) = (d.clone(), p.clone(), d.clone(), p.clone());
    let (i, n) = (p.id.clone(), p.name.clone());
    vec![
        MenuItem::dialog(t(lang, K::EditL7Policy), move || l7_policy_dialog(lang, &d1, Some(&p1))),
        MenuItem::dialog(t(lang, K::AddL7Rule), move || {
            l7_rule_dialog(lang, &d2, Some(&p2), None)
        }),
        MenuItem::danger(t(lang, K::DeleteL7Policy), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeleteL7Policy),
                n.clone(),
                OsAction::L7PolicyDelete { id: i.clone() },
            )
        }),
    ]
}

/// An L7 rule's menu (`p`: its policy).
pub fn l7_rule_items(lang: Lang, d: &LoadBalancerDetail, p: &LbL7Policy, r: &LbL7Rule) -> Vec<MenuItem> {
    let (dd, pp, rr) = (d.clone(), p.clone(), r.clone());
    let (pid, id, label) = (
        p.id.clone(),
        r.id.clone(),
        format!("{} {} {}", r.rule_type, r.compare_type, r.value),
    );
    vec![
        MenuItem::dialog(t(lang, K::EditL7Rule), move || {
            l7_rule_dialog(lang, &dd, Some(&pp), Some(&rr))
        }),
        MenuItem::danger(t(lang, K::DeleteL7Rule), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeleteL7Rule),
                label.clone(),
                OsAction::L7RuleDelete {
                    policy_id: pid.clone(),
                    id: id.clone(),
                },
            )
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::components::dialog::Kind;

    fn lb_with_members() -> LoadBalancerDetail {
        let m = |id: &str, address: &str| {
            serde_json::from_value::<LbMember>(serde_json::json!({
                "id": id, "name": id, "pool": {"id": "pool-1", "name": "web"}, "address": address, "port": 80, "weight": 3,
                "subnet": {"id": "sub-1", "name": "s"}, "backup": false, "operating_status": "ONLINE",
                "provisioning_status": "ACTIVE", "admin_up": true, "monitor_address": "10.0.9.9", "monitor_port": 9000
            }))
            .unwrap()
        };
        serde_json::from_value(serde_json::json!({
            "row": {"id": "lb1", "name": "lb", "description": null, "vip_address": null, "availability_zone": null,
                    "operating_status": "ONLINE", "provisioning_status": "ACTIVE", "admin_up": true, "provider": null},
            "project_id": "p", "flavor_id": null, "vip_port_id": null, "vip_network": null, "vip_subnet": null,
            "floating_ip": null, "created": null, "updated": null, "listeners": [],
            "pools": [{"id": "pool-1", "name": "web", "protocol": "HTTP", "algorithm": "ROUND_ROBIN", "session_persistence": null,
                       "listeners": [], "members": 2, "monitor": null, "operating_status": "ONLINE",
                       "provisioning_status": "ACTIVE", "admin_up": true}],
            "members": [m("m1", "10.0.0.5"), m("m2", "10.0.0.6"), serde_json::to_value(LbMember { pool: Ref { id: "other".into(), name: "o".into() }, ..m("m3", "10.0.0.7") }).unwrap()],
            "monitors": []
        }))
        .unwrap()
    }

    #[test]
    fn edit_members_starts_from_the_pool_and_sends_one_batch() {
        let d = lb_with_members();
        let dlg = pool_members_dialog(Lang::EnUs, &d, &d.pools[0]);
        assert!(dlg.danger, "the batch deletes members left out");
        let rows: Vec<MemberRow> = serde_json::from_str(&dlg.fields[0].default).unwrap();
        assert_eq!(
            rows.iter().map(|r| r.address.as_str()).collect::<Vec<_>>(),
            ["10.0.0.5", "10.0.0.6"],
            "only this pool's members"
        );
        assert_eq!(
            (rows[0].weight.as_str(), rows[0].monitor_port.as_str()),
            ("3", "9000"),
            "kept, so saving doesn't reset them"
        );
        let Kind::MemberRows { existing, .. } = &dlg.fields[0].kind else {
            panic!("not a members editor")
        };
        assert_eq!(existing.len(), 2);
        let keep_one = serde_json::to_string(&rows[..1]).unwrap();
        let OsAction::PoolSetMembers { pool_id, members } = (dlg.build)(&vals(&[("members", &keep_one)])).unwrap()
        else {
            panic!("not a batch")
        };
        assert_eq!(
            (pool_id.as_str(), members.len(), members[0].monitor_address.as_deref()),
            ("pool-1", 1, Some("10.0.9.9"))
        );
    }

    /// The *Advanced* box opens on a listener that has
    /// options, *Insert Headers* only for HTTP, and unticked leaves them alone.
    #[test]
    fn listener_and_pool_dialogs_carry_the_advanced_options() {
        let mut d = lb_with_members();
        let listener = |protocol: &str, options: serde_json::Value| -> LbListener {
            serde_json::from_value(serde_json::json!({
                "id": "ls1", "name": "web", "protocol": protocol, "port": 80, "default_pool": null, "connection_limit": null,
                "operating_status": "ONLINE", "provisioning_status": "ACTIVE", "admin_up": true, "options": options
            }))
            .unwrap()
        };
        let http = listener(
            "HTTP",
            serde_json::json!({"timeout_client_data": 60000, "insert_headers": ["X-Forwarded-For"], "allowed_cidrs": []}),
        );
        d.listeners = vec![http.clone()];
        let dlg = listener_dialog(Lang::EnUs, &d, Some(&http));
        let field = |dlg: &Dialog, n: &str| dlg.fields.iter().find(|f| f.name == n).map(|f| f.default.clone());
        assert_eq!(field(&dlg, "lb_advanced").as_deref(), Some("true"), "options set: open");
        assert_eq!(
            (
                field(&dlg, "timeout_client_data").as_deref(),
                field(&dlg, "insert_headers").as_deref()
            ),
            (Some("60000"), Some("X-Forwarded-For"))
        );
        let base = [
            ("name", "web"),
            ("description", ""),
            ("connection_limit", ""),
            ("default_pool", ""),
            ("admin_up", "true"),
        ];
        let with = |extra: &[(&str, &str)]| vals(&base.iter().chain(extra).copied().collect::<Vec<_>>());
        let OsAction::ListenerUpdate { options, protocol, .. } = (dlg.build)(&with(&[
            ("lb_advanced", "true"),
            ("timeout_client_data", ""),
            ("insert_headers", "X-Forwarded-Proto"),
            ("allowed_cidrs", "10.0.0.0/8\n"),
        ]))
        .unwrap() else {
            panic!("not an update")
        };
        let o = options.expect("ticked: sent");
        assert_eq!(
            (
                protocol.as_str(),
                o.timeout_client_data,
                o.insert_headers,
                o.allowed_cidrs
            ),
            (
                "HTTP",
                None,
                vec!["X-Forwarded-Proto".to_string()],
                Some(vec!["10.0.0.0/8".to_string()])
            )
        );
        let OsAction::ListenerUpdate { options, .. } = (dlg.build)(&with(&[("lb_advanced", "false")])).unwrap() else {
            panic!()
        };
        assert_eq!(options, None, "unticked: left alone");

        let tcp = listener("TCP", serde_json::json!({}));
        let dlg = listener_dialog(Lang::EnUs, &d, Some(&tcp));
        assert_eq!(field(&dlg, "insert_headers"), None, "TCP listeners take no headers");
        assert_eq!(
            field(&dlg, "allowed_cidrs"),
            None,
            "not reported by the listener: Octavia < 2.12"
        );
        assert_eq!(field(&dlg, "lb_advanced").as_deref(), Some("false"));

        let pool = d.pools[0].clone();
        let dlg = pool_dialog(Lang::EnUs, &d, Some(&pool));
        let pool_vals = |extra: &[(&str, &str)]| {
            vals(
                &[
                    ("name", "web"),
                    ("description", ""),
                    ("algorithm", "ROUND_ROBIN"),
                    ("persistence", ""),
                    ("admin_up", "true"),
                ]
                .iter()
                .chain(extra)
                .copied()
                .collect::<Vec<_>>(),
            )
        };
        let OsAction::PoolUpdate { tls, .. } =
            (dlg.build)(&pool_vals(&[("tls_enabled", "true"), ("tls_ciphers", "AES256")])).unwrap()
        else {
            panic!()
        };
        assert_eq!(tls, Some((true, Some("AES256".to_string()))));
        assert!(
            (dlg.build)(&pool_vals(&[("tls_enabled", "false"), ("tls_ciphers", "AES256")])).is_ok(),
            "hidden ciphers are dropped"
        );
    }

    /// Policies only on HTTP listeners; the policy's own
    /// menu presets it on a new rule; the key is dropped for keyless types.
    #[test]
    fn l7_dialogs_build_policies_and_rules() {
        let mut d = lb_with_members();
        let listener = |id: &str, protocol: &str| -> LbListener {
            serde_json::from_value(serde_json::json!({
                "id": id, "name": id, "protocol": protocol, "port": 80, "default_pool": null, "connection_limit": null,
                "operating_status": "ONLINE", "provisioning_status": "ACTIVE", "admin_up": true
            }))
            .unwrap()
        };
        d.listeners = vec![
            listener("http", "HTTP"),
            listener("tcp", "TCP"),
            listener("tls", "TERMINATED_HTTPS"),
        ];
        assert_eq!(
            l7_listeners(&d).iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            ["http", "tls"]
        );
        let dlg = l7_policy_dialog(Lang::EnUs, &d, None);
        let OsAction::L7PolicyCreate { listener_id, spec } = (dlg.build)(&vals(&[
            ("listener", "http"),
            ("name", "api"),
            ("description", ""),
            ("action", "REDIRECT_TO_POOL"),
            ("redirect_pool", "pool-1"),
            ("redirect_url", ""),
            ("position", "2"),
            ("admin_up", "true"),
        ]))
        .unwrap() else {
            panic!("not a create")
        };
        assert_eq!(
            (listener_id.as_str(), spec.position, spec.redirect_pool_id.as_deref()),
            ("http", Some(2), Some("pool-1"))
        );

        let policy: LbL7Policy = serde_json::from_value(serde_json::json!({
            "id": "pol-1", "name": "api", "description": null, "listener": {"id": "http", "name": "http"}, "action": "REJECT",
            "position": 1, "redirect_pool": null, "redirect_url": null, "operating_status": "ONLINE",
            "provisioning_status": "ACTIVE", "admin_up": true, "rules": []
        }))
        .unwrap();
        let dlg = l7_rule_dialog(Lang::EnUs, &d, Some(&policy), None);
        assert!(
            dlg.fields.iter().all(|f| f.name != "policy"),
            "preset from the policy's menu"
        );
        let OsAction::L7RuleCreate { policy_id, spec } = (dlg.build)(&vals(&[
            ("rule_type", "PATH"),
            ("compare_type", "STARTS_WITH"),
            ("key", "stale"),
            ("value", "/api"),
            ("invert", "false"),
            ("admin_up", "true"),
        ]))
        .unwrap() else {
            panic!("not a create")
        };
        assert_eq!(
            (policy_id.as_str(), spec.key),
            ("pol-1", None),
            "a hidden key isn't sent"
        );
    }

    #[test]
    fn add_member_takes_an_external_address() {
        let d = lb_with_members();
        let dlg = member_dialog(Lang::EnUs, &d, None);
        let ext = (dlg.build)(&vals(&[
            ("pool", "pool-1"),
            ("source", "external"),
            ("address", "192.0.2.10"),
            ("subnet", "sub-ext"),
            ("port", "8080"),
            ("weight", "1"),
            ("monitor_address", ""),
            ("monitor_port", "9000"),
            ("admin_up", "true"),
        ]))
        .unwrap();
        let OsAction::MemberCreate { spec, .. } = ext else {
            panic!("not a create")
        };
        assert_eq!(
            (
                spec.address.as_str(),
                spec.subnet_id.as_deref(),
                spec.monitor_port,
                spec.monitor_address
            ),
            ("192.0.2.10", Some("sub-ext"), Some(9000), None)
        );
        let edit = member_dialog(Lang::EnUs, &d, Some(&d.members[0]));
        let field = |n: &str| {
            edit.fields
                .iter()
                .find(|f| f.name == n)
                .map(|f| f.default.clone())
                .unwrap()
        };
        assert_eq!(
            (field("name"), field("monitor_address")),
            (String::new(), "10.0.9.9".to_string()),
            "the ID fallback isn't offered as a name"
        );
        let OsAction::MemberUpdate {
            monitor_address,
            monitor_port,
            ..
        } = (edit.build)(&vals(&[
            ("name", ""),
            ("weight", "3"),
            ("monitor_address", ""),
            ("monitor_port", ""),
            ("admin_up", "true"),
        ]))
        .unwrap()
        else {
            panic!("not an update")
        };
        assert_eq!(
            (monitor_address, monitor_port),
            (None, None),
            "emptied: back to monitoring the member itself"
        );
    }
}
