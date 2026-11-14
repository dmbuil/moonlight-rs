//! Project › Network › Firewall (FWaaS v2): the tab bar the three
//! lists share, the cells they share, and the group, policy and rule detail
//! pages. The lists are in `lists.rs`. Another project's shared policies and
//! rules are shown read-only (no actions).

use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::components::A;

use super::details::{Card, DetailHeader, Kv, mono, param, yes_no};
use super::lists::{admin_state, link, moving_status};
use crate::api::*;
use crate::components::dialog::{CreateButton, RowActions, data_version};
use crate::components::ops;
use crate::components::table::{Col, DataTable, col};
use crate::components::widgets::{Copyable, Pill, Skeleton, Tone, dash, poll_while_busy, render};
use crate::i18n::{K, Lang, t, use_i18n};
use crate::model::*;

/// The panel's tabs, as neutron-fwaas-dashboard orders them; each is a URL so
/// a detail page's breadcrumb returns to the right one.
const TABS: [(K, &str); 3] = [
    (K::FirewallGroups, "/project/firewall"),
    (K::FirewallPolicies, "/project/firewall/policies"),
    (K::FirewallRules, "/project/firewall/rules"),
];

/// The tab bar above each list; `active` indexes [`TABS`].
#[component]
pub fn FirewallTabs(active: usize) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="tabs" role="tablist">
            {TABS.iter().enumerate().map(|(i, &(k, href))| view! {
                <A href=href attr:role="tab" attr:aria-selected=(i == active).to_string() attr:class=if i == active { "active" } else { "" }>
                    {move || i18n.t(k)}
                </A>
            }).collect_view()}
        </div>
    }
}

/// A firewall policy, linked; `-` for none.
pub fn policy_link(p: Option<&Ref>) -> AnyView {
    match p {
        Some(p) => link(format!("/project/firewall/policies/{}", p.id), p.name.clone()),
        None => "-".into_any(),
    }
}

/// Policies, linked, comma-separated; `-` for none.
pub fn policy_links(ps: &[Ref]) -> AnyView {
    joined(ps, "/project/firewall/policies")
}

/// A policy's rules in evaluation order, linked; `-` for none.
pub fn rule_links(rs: &[Ref]) -> AnyView {
    joined(rs, "/project/firewall/rules")
}

/// `refs` linked to `{base}/{id}` and comma-separated; `-` for none.
fn joined(refs: &[Ref], base: &'static str) -> AnyView {
    if refs.is_empty() {
        return "-".into_any();
    }
    let n = refs.len();
    refs.iter()
        .enumerate()
        .map(|(i, r)| view! { {link(format!("{base}/{}", r.id), r.name.clone())}{(i + 1 < n).then_some(", ")} })
        .collect_view()
        .into_any()
}

/// `TCP`, `UDP`, `ICMP`, or *Any*.
pub fn protocol(l: Lang, r: &FirewallRuleRow) -> String {
    r.protocol
        .as_deref()
        .map(str::to_uppercase)
        .unwrap_or_else(|| t(l, K::AnyValue).to_string())
}

/// An address or a port; *Any* when Neutron has `null`.
pub fn any(l: Lang, v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| t(l, K::AnyValue).to_string())
}

/// *Allow* green, *Deny* / *Reject* red.
pub fn action_pill(l: Lang, action: &str) -> AnyView {
    let (k, tone) = match action {
        "allow" => (K::FirewallAllow, Tone::Ok),
        "deny" => (K::FirewallDeny, Tone::Err),
        "reject" => (K::FirewallReject, Tone::Err),
        other => return other.to_string().into_any(),
    };
    view! { <Pill tone=tone label=t(l, k)/> }.into_any()
}

// --- Rule ---------------------------------------------------------------------------

/// Firewall rule detail: the rule's cards and its menu (*Edit Rule*, *Delete Rule*; none for
/// another project's shared rule).
#[component]
pub fn FirewallRuleDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_firewall_rule(id));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let r = td.data;
                let whole = r.clone();
                let items = Signal::derive(move || ops::firewall_rule_items(i18n.lang(), &whole));
                view! {
                    <DetailHeader group=K::NetworkGroup panel=K::Firewall href="/project/firewall/rules"
                        title_prefix=K::FirewallRule name=r.name.clone() timing=(td.millis, td.calls) items=items/>
                    <RuleCards r=r/>
                }
            })}
        </Transition>
    }
}

/// The rule's *Information* and *Firewall Rule* (match and action) cards; Neutron's `null`
/// addresses and ports read *Any*.
#[component]
fn RuleCards(r: FirewallRuleRow) -> impl IntoView {
    let i18n = use_i18n();
    let r = StoredValue::new(r);
    let get = move |f: fn(&FirewallRuleRow, Lang) -> String| move || r.with_value(|r| f(r, i18n.lang()));
    view! {
        <div class="cards">
            <Card title=K::Information>
                <Kv label=K::Name><Copyable value=r.with_value(|r| r.name.clone())/></Kv>
                <Kv label=K::Id><Copyable value=r.with_value(|r| r.id.clone()) mono=true/></Kv>
                <Kv label=K::Description>{dash(r.with_value(|r| r.description.clone()))}</Kv>
                <Kv label=K::Action>{move || r.with_value(|r| action_pill(i18n.lang(), &r.action))}</Kv>
                <Kv label=K::EnabledCol>{get(|r, l| yes_no(l, r.enabled).into())}</Kv>
                <Kv label=K::Shared>{get(|r, l| yes_no(l, r.shared).into())}</Kv>
                <Kv label=K::Policies>{r.with_value(|r| policy_links(&r.policies))}</Kv>
            </Card>
            <Card title=K::FirewallRule>
                <Kv label=K::Protocol>{get(|r, l| protocol(l, r))}</Kv>
                <Kv label=K::IpVersion>{r.with_value(|r| format!("IPv{}", r.ip_version))}</Kv>
                <Kv label=K::SourceIp>{get(|r, l| any(l, &r.source_ip))}</Kv>
                <Kv label=K::SourcePort>{get(|r, l| any(l, &r.source_port))}</Kv>
                <Kv label=K::DestinationIp>{get(|r, l| any(l, &r.destination_ip))}</Kv>
                <Kv label=K::DestinationPort>{get(|r, l| any(l, &r.destination_port))}</Kv>
            </Card>
        </div>
    }
}

// --- Policy -------------------------------------------------------------------------

/// The policy page's rule table: evaluation order first, then the rule.
fn policy_rule_columns(policy: Option<FirewallPolicyRow>) -> Vec<Col<(usize, FirewallRuleRow)>> {
    type Row = (usize, FirewallRuleRow);
    let mut cols = vec![
        col(K::Position, |(i, _): &Row, _| (i + 1).to_string())
            .sort_num(|(i, _)| *i as i64)
            .class("num"),
        col(K::Name, |(_, r): &Row, _| {
            link(format!("/project/firewall/rules/{}", r.id), r.name.clone())
        }),
        col(K::Protocol, |(_, r): &Row, l| protocol(l, r)),
        col(K::SourceIp, |(_, r): &Row, l| any(l, &r.source_ip)),
        col(K::SourcePort, |(_, r): &Row, l| any(l, &r.source_port)),
        col(K::DestinationIp, |(_, r): &Row, l| any(l, &r.destination_ip)),
        col(K::DestinationPort, |(_, r): &Row, l| any(l, &r.destination_port)),
        col(K::Action, |(_, r): &Row, l| action_pill(l, &r.action)),
        col(K::EnabledCol, |(_, r): &Row, l| yes_no(l, r.enabled)),
    ];
    // Only the project's own policy can lose a rule.
    if let Some(p) = policy.filter(|p| p.owned) {
        let pid = p.id.clone();
        cols.push(
            col(K::Actions, move |(_, r): &Row, l| {
                let (pid, rule) = (
                    pid.clone(),
                    Ref {
                        id: r.id.clone(),
                        name: r.name.clone(),
                    },
                );
                let item = crate::components::dialog::MenuItem::danger(t(l, K::RemoveRule), move || {
                    ops::remove_firewall_rule_dialog(l, &pid, &rule)
                });
                view! { <RowActions items=vec![item]/> }
            })
            .class("actions"),
        );
    }
    cols
}

/// Firewall policy detail: its *Information* card and its rules in evaluation order, with
/// *Insert Rule* and each rule's *Remove Rule* when the project owns the policy.
#[component]
pub fn FirewallPolicyDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_firewall_policy(id));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let p = d.policy.clone();
                let whole = p.clone();
                let items = Signal::derive(move || ops::firewall_policy_items(i18n.lang(), &whole));
                let insert = p.owned.then(|| {
                    let pp = p.clone();
                    view! {
                        <div class="subtable-actions">
                            <CreateButton label=Signal::derive(move || i18n.t(K::InsertRule).to_string())
                                open=Arc::new(move || ops::insert_firewall_rule_dialog(i18n.lang(), &pp))/>
                        </div>
                    }
                });
                let rows: Vec<(usize, FirewallRuleRow)> = d.rules.into_iter().enumerate().collect();
                let empty = rows.is_empty();
                let columns = policy_rule_columns(Some(p.clone()));
                view! {
                    <DetailHeader group=K::NetworkGroup panel=K::Firewall href="/project/firewall/policies"
                        title_prefix=K::FirewallPolicy name=p.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=p.name.clone()/></Kv>
                            <Kv label=K::Id><Copyable value=p.id.clone() mono=true/></Kv>
                            <Kv label=K::Description>{dash(p.description.clone())}</Kv>
                            <Kv label=K::Audited>{move || yes_no(i18n.lang(), p.audited)}</Kv>
                            <Kv label=K::Shared>{move || yes_no(i18n.lang(), p.shared)}</Kv>
                        </Card>
                    </div>
                    <h2 class="section-title">{move || i18n.t(K::Rules)}</h2>
                    {insert}
                    {if empty {
                        view! { <p class="muted">{move || i18n.t(K::NoRulesInPolicy)}</p> }.into_any()
                    } else {
                        view! { <DataTable rows=rows columns=columns search=|(_, r): &(usize, FirewallRuleRow)| format!("{} {} {}", r.name, r.id, r.action)/> }.into_any()
                    }}
                }
            })}
        </Transition>
    }
}

// --- Group --------------------------------------------------------------------------

/// The group page's port table: port, fixed IPs, whether it is a router's or an instance's
/// port, the device linked, and *Remove Port* when the project owns the group.
fn group_port_columns(group: FirewallGroupRow) -> Vec<Col<FirewallPort>> {
    let mut cols = vec![
        col(K::Port, |p: &FirewallPort, _| {
            link(format!("/project/networks/ports/{}", p.port.id), p.port.name.clone())
        })
        .sort_text(|p| p.port.name.clone()),
        col(K::FixedIps, |p: &FirewallPort, _| {
            mono(if p.ips.is_empty() {
                "-".to_string()
            } else {
                p.ips.join(", ")
            })
        }),
        col(K::Type, |p: &FirewallPort, l| match p.kind.as_deref() {
            Some("router") => t(l, K::Router).into_any(),
            Some("instance") => t(l, K::Instance).into_any(),
            _ => "-".into_any(),
        }),
        col(K::Device, |p: &FirewallPort, _| match (&p.device, p.device_href()) {
            (Some(d), Some(href)) => link(href, d.name.clone()),
            (Some(d), None) => mono(d.name.clone()),
            _ => "-".into_any(),
        }),
    ];
    if group.owned {
        let g = StoredValue::new(group);
        cols.push(
            col(K::Actions, move |p: &FirewallPort, l| {
                let p = p.clone();
                let item = crate::components::dialog::MenuItem::danger(t(l, K::RemovePort), move || {
                    g.with_value(|g| ops::remove_firewall_port_dialog(l, g, &p))
                });
                view! { <RowActions items=vec![item]/> }
            })
            .class("actions"),
        );
    }
    cols
}

/// Firewall group detail: its *Information* card (ingress and egress policies, status),
/// notes for an inactive group and for `default`, and its ports with *Add Port* / *Remove
/// Port*. It polls while Neutron moves the group through `PENDING_*`.
#[component]
pub fn FirewallGroupDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_firewall_group(id));
    // Neutron moves the group through `PENDING_*` on its own.
    let polling = poll_while_busy(data, move || data.with(|r| matches!(r, Some(Ok(td)) if td.data.busy())));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let g = td.data;
                let whole = g.clone();
                let items = Signal::derive(move || ops::firewall_group_items(i18n.lang(), &whole));
                let status = g.status.clone();
                let busy = g.busy();
                // Why it does nothing, and why `default` has fewer actions.
                let notes = [
                    (g.status.eq_ignore_ascii_case("inactive")).then_some(K::FirewallInactiveHelp),
                    (g.owned && g.is_default()).then_some(K::FirewallDefaultGroupHelp),
                ];
                let add = g.owned.then(|| {
                    let gg = g.clone();
                    view! {
                        <div class="subtable-actions">
                            <CreateButton label=Signal::derive(move || i18n.t(K::AddPort).to_string())
                                open=Arc::new(move || ops::add_firewall_port_dialog(i18n.lang(), &gg))/>
                        </div>
                    }
                });
                let empty = g.ports.is_empty();
                let columns = group_port_columns(g.clone());
                let (ingress, egress) = (g.ingress.clone(), g.egress.clone());
                view! {
                    <DetailHeader group=K::NetworkGroup panel=K::Firewall href="/project/firewall"
                        title_prefix=K::FirewallGroup name=g.name.clone() timing=(td.millis, td.calls) items=items/>
                    {move || polling.get().then(|| view! { <p class="updating">{i18n.t(K::Updating)}</p> })}
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=g.name.clone()/></Kv>
                            <Kv label=K::Id><Copyable value=g.id.clone() mono=true/></Kv>
                            <Kv label=K::Description>{dash(g.description.clone())}</Kv>
                            <Kv label=K::IngressPolicy>{policy_link(ingress.as_ref())}</Kv>
                            <Kv label=K::EgressPolicy>{policy_link(egress.as_ref())}</Kv>
                            <Kv label=K::Status>{move || moving_status(&status, i18n.lang(), busy)}</Kv>
                            <Kv label=K::AdminState>{move || admin_state(i18n.lang(), g.admin_up)}</Kv>
                            <Kv label=K::Shared>{move || yes_no(i18n.lang(), g.shared)}</Kv>
                        </Card>
                    </div>
                    {notes.into_iter().flatten().map(|k| view! { <p class="muted">{move || i18n.t(k)}</p> }).collect_view()}
                    <h2 class="section-title">{move || i18n.t(K::Ports)}</h2>
                    {add}
                    {if empty {
                        view! { <p class="muted">{move || i18n.t(K::NoPortsInGroup)}</p> }.into_any()
                    } else {
                        view! { <DataTable rows=g.ports.clone() columns=columns search=|p: &FirewallPort| format!("{} {} {}", p.port.name, p.ips.join(" "), p.device.as_ref().map(|d| d.name.as_str()).unwrap_or(""))/> }.into_any()
                    }}
                }
            })}
        </Transition>
    }
}
