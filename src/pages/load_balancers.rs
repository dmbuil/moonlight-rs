//! Octavia load balancer detail: Horizon's (octavia-dashboard) tabs, each with
//! its create button and row actions; the header menu edits the load balancer,
//! (dis)associates its VIP's floating IP and deletes it. The list is in `lists.rs`.
//! *L7 Policies* shows the listeners' policies and their rules;
//! octavia-dashboard has them on a listener page, which Moonlight doesn't have.

use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::components::A;

use super::details::{Card, DetailHeader, Kv, Tabs, mono, param, yes_no};
use super::lists::{admin_state, link, status_pill};
use crate::api::*;
use crate::components::dialog::{BulkButton, CreateButton, Dialog, RowActions, data_version};
use crate::components::ops;
use crate::components::table::{DataTable, Select, col};
use crate::components::widgets::{Copyable, Skeleton, dash, poll_while_busy, render, short_time};
use crate::i18n::{K, Lang, use_i18n};
use crate::model::*;

/// A count or limit Octavia may leave out, or `-`.
fn num(n: Option<i64>) -> String {
    n.map(|n| n.to_string()).unwrap_or_else(|| "-".into())
}

/// A link to a resource of this load balancer, opening its tab.
fn tab_link(r: &Ref, tab: RwSignal<usize>, to: usize) -> AnyView {
    let name = r.name.clone();
    view! { <a href="#" on:click=move |ev| { ev.prevent_default(); tab.set(to); }>{name}</a> }.into_any()
}

/// Several of this load balancer's resources as tab links (`to` is the tab), or `-`.
fn refs(items: &[Ref], tab: RwSignal<usize>, to: usize) -> AnyView {
    if items.is_empty() {
        return "-".into_any();
    }
    items
        .iter()
        .map(|r| view! { <span class="ref">{tab_link(r, tab, to)}</span> })
        .collect_view()
        .into_any()
}

/// Operating and provisioning status side by side, as Horizon shows them.
fn statuses(op: &str, prov: &str, l: Lang) -> AnyView {
    view! { {status_pill(op, l)} " " {status_pill(prov, l)} }.into_any()
}

/// Project › Network › Load Balancers › one (`/project/load_balancers/:id`):
/// octavia-dashboard's tabs (*Overview*, *Listeners*, *Pools*, *Members*, *Health Monitors*)
/// and *L7 Policies*, polling while Octavia has the load balancer in `PENDING_*`.
#[component]
pub fn LoadBalancerDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_load_balancer(id));
    // Follows PENDING_* states, e.g. after a change in one of its tabs.
    let polling = poll_while_busy(data, move || data.with(|r| matches!(r, Some(Ok(td)) if td.data.busy())));
    let tab = RwSignal::new(0usize);
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let whole = d.clone();
                let items = Signal::derive(move || ops::load_balancer_detail_items(i18n.lang(), &whole));
                let d = StoredValue::new(d);
                view! {
                    <DetailHeader group=K::NetworkGroup panel=K::LoadBalancers href="/project/load_balancers"
                        name=d.with_value(|d| d.row.name.clone()) timing=(td.millis, td.calls) items=items/>
                    {move || polling.get().then(|| view! { <p class="updating">{i18n.t(K::Updating)}</p> })}
                    <Tabs labels=vec![K::Overview, K::Listeners, K::Pools, K::Members, K::HealthMonitors, K::L7Policies] active=tab/>
                    {move || match tab.get() {
                        0 => d.with_value(|d| overview(d.clone())).into_any(),
                        1 => d.with_value(|d| listeners(d.clone(), tab)).into_any(),
                        2 => d.with_value(|d| pools(d.clone(), tab)).into_any(),
                        3 => d.with_value(|d| members(d.clone())).into_any(),
                        4 => d.with_value(|d| monitors(d.clone())).into_any(),
                        _ => d.with_value(|d| l7(d.clone(), tab)).into_any(),
                    }}
                }
            })}
        </Transition>
    }
}

/// *Overview*: *Information* (statuses, provider, flavor, zone) and *VIP* (address,
/// floating IP, port, network, subnet).
fn overview(d: LoadBalancerDetail) -> impl IntoView {
    let i18n = use_i18n();
    let (op, prov) = (d.row.operating_status.clone(), d.row.provisioning_status.clone());
    let network = d
        .vip_network
        .clone()
        .map(|n| view! { <A href=format!("/project/networks/{}", n.id)>{n.name}</A> }.into_any());
    let subnet = d
        .vip_subnet
        .clone()
        .map(|s| view! { <A href=format!("/project/networks/subnets/{}", s.id)>{s.name}</A> }.into_any());
    let port = d
        .vip_port_id
        .clone()
        .map(|p| link(format!("/project/networks/ports/{p}"), p));
    let or_dash = |v: Option<AnyView>| v.unwrap_or_else(|| "-".into_any());
    view! {
        <div class="cards">
            <Card title=K::Information>
                <Kv label=K::Name><Copyable value=d.row.name.clone()/></Kv>
                <Kv label=K::Id><Copyable value=d.row.id.clone() mono=true/></Kv>
                <Kv label=K::Description>{dash(d.row.description.clone())}</Kv>
                <Kv label=K::Project><Copyable value=d.project_id.clone() mono=true/></Kv>
                <Kv label=K::OperatingStatus>{move || status_pill(&op, i18n.lang())}</Kv>
                <Kv label=K::ProvisioningStatus>{move || status_pill(&prov, i18n.lang())}</Kv>
                <Kv label=K::AdminState>{move || admin_state(i18n.lang(), d.row.admin_up)}</Kv>
                <Kv label=K::Provider>{dash(d.row.provider.clone())}</Kv>
                <Kv label=K::Flavor>{mono(dash(d.flavor_id.clone()))}</Kv>
                <Kv label=K::AvailabilityZone>{dash(d.row.availability_zone.clone())}</Kv>
                <Kv label=K::Created>{d.created.as_deref().map(short_time).unwrap_or_else(|| "-".into())}</Kv>
                <Kv label=K::Updated>{d.updated.as_deref().map(short_time).unwrap_or_else(|| "-".into())}</Kv>
            </Card>
            <Card title=K::Vip>
                <Kv label=K::IpAddress>{match d.row.vip_address.clone() {
                    Some(ip) => view! { <Copyable value=ip mono=true/> }.into_any(),
                    None => "-".into_any(),
                }}</Kv>
                <Kv label=K::FloatingIp>{match d.floating_ip.clone() {
                    Some(ip) => view! { <Copyable value=ip mono=true/> }.into_any(),
                    None => "-".into_any(),
                }}</Kv>
                <Kv label=K::Port>{or_dash(port)}</Kv>
                <Kv label=K::Network>{or_dash(network)}</Kv>
                <Kv label=K::Subnet>{or_dash(subnet)}</Kv>
            </Card>
        </div>
    }
}

/// A tab's create button, for [`tab_actions`].
fn create_button(label: K, open: impl Fn() -> Dialog + Send + Sync + 'static) -> AnyView {
    let i18n = use_i18n();
    view! { <CreateButton label=Signal::derive(move || i18n.t(label).to_string()) open=Arc::new(open)/> }.into_any()
}

/// The buttons above a tab's table: the bulk delete of the ticked rows (`label`, one `make`
/// action per selection key, in a batch that waits out Octavia's lock between them) and the
/// create button, if any.
fn tab_actions(
    label: K,
    picked: RwSignal<Vec<(String, String)>>,
    make: fn(String) -> OsAction,
    create: Option<AnyView>,
) -> AnyView {
    view! {
        <div class="subtable-actions">
            <BulkButton label=label picked=picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, label, items, make))/>
            {create}
        </div>
    }
    .into_any()
}

/// A selection entry `(id, label)`: the name, or the ID when it has none.
fn named_key(id: &str, name: &str) -> Option<(String, String)> {
    let label = if name.trim().is_empty() { id } else { name };
    Some((id.to_string(), label.to_string()))
}

/// The key of a resource that lives under another (a pool's member, a policy's rule):
/// `parent/id`. Octavia's IDs are UUIDs, so `/` can't be part of one.
fn pair_key(parent: &str, id: &str) -> String {
    format!("{parent}/{id}")
}

/// [`pair_key`] back into `(parent, id)`.
fn split_pair(key: &str) -> (String, String) {
    let (parent, id) = key.split_once('/').unwrap_or(("", key));
    (parent.to_string(), id.to_string())
}

/// A listener's bulk-delete entry.
fn listener_key(l: &LbListener) -> Option<(String, String)> {
    named_key(&l.id, &l.name)
}

/// A pool's bulk-delete entry.
fn pool_key(p: &LbPool) -> Option<(String, String)> {
    named_key(&p.id, &p.name)
}

/// A member's bulk-delete entry, keyed by [`pair_key`] with its pool, which the delete's path
/// needs; unnamed members are labelled `address:port`, as they are added.
fn member_key(m: &LbMember) -> Option<(String, String)> {
    let label = if m.name.trim().is_empty() {
        match m.port {
            Some(port) => format!("{}:{port}", m.address),
            None => m.address.clone(),
        }
    } else {
        m.name.clone()
    };
    Some((pair_key(&m.pool.id, &m.id), label))
}

/// A health monitor's bulk-delete entry.
fn monitor_key(m: &LbMonitor) -> Option<(String, String)> {
    named_key(&m.id, &m.name)
}

/// An L7 policy's bulk-delete entry.
fn policy_key(p: &LbL7Policy) -> Option<(String, String)> {
    named_key(&p.id, &p.name)
}

/// An L7 rule's bulk-delete entry, keyed by [`pair_key`] with its policy. Rules have no name:
/// the policy's, then what the rule matches.
fn rule_key((p, r): &(LbL7Policy, LbL7Rule)) -> Option<(String, String)> {
    let what = [
        r.rule_type.as_str(),
        r.key.as_deref().unwrap_or(""),
        r.compare_type.as_str(),
        r.value.as_str(),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    Some((
        pair_key(&p.id, &r.id),
        format!("{}: {what}", named_key(&p.id, &p.name)?.1),
    ))
}

/// A fresh, empty selection over a tab's rows.
fn select<T>(key: fn(&T) -> Option<(String, String)>) -> Select<T> {
    Select {
        key,
        picked: RwSignal::new(Vec::new()),
    }
}

/// *Listeners*: protocol and port, default pool (a link to *Pools*), connection limit,
/// status, admin state and each listener's menu; *Create Listener* above.
fn listeners(d: LoadBalancerDetail, tab: RwSignal<usize>) -> impl IntoView {
    let i18n = use_i18n();
    let (rows, dd, da) = (d.listeners.clone(), d.clone(), d.clone());
    let sel = select(listener_key);
    let columns = vec![
        col(
            K::Name,
            |l: &LbListener, _| view! { <span class="strong">{l.name.clone()}</span> },
        )
        .sort_text(|l| l.name.clone()),
        col(K::Protocol, |l: &LbListener, _| mono(l.protocol.clone())).sort_text(|l| l.protocol.clone()),
        col(K::Port, |l: &LbListener, _| num(l.port))
            .sort_num(|l| l.port.unwrap_or(0))
            .class("num"),
        col(K::DefaultPool, move |l: &LbListener, _| {
            l.default_pool
                .as_ref()
                .map(|p| tab_link(p, tab, 2))
                .unwrap_or_else(|| "-".into_any())
        }),
        col(K::ConnectionLimit, |l: &LbListener, lang| match l.connection_limit {
            Some(n) => n.to_string(),
            None => crate::i18n::t(lang, K::Unlimited).to_string(),
        })
        .class("num"),
        col(K::Status, |l: &LbListener, lang| {
            statuses(&l.operating_status, &l.provisioning_status, lang)
        }),
        col(K::AdminState, |l: &LbListener, lang| admin_state(lang, l.admin_up)),
        col(
            K::Actions,
            move |l: &LbListener, lang| view! { <RowActions items=ops::listener_items(lang, &da, l)/> },
        )
        .class("actions"),
    ];
    view! {
        {tab_actions(K::DeleteListeners, sel.picked, |id| OsAction::ListenerDelete { id },
            Some(create_button(K::CreateListener, move || ops::listener_dialog(i18n.lang(), &dd, None))))}
        <DataTable rows=rows columns=columns select=sel search=|l: &LbListener| format!("{} {} {:?}", l.name, l.protocol, l.port)/>
    }
}

/// *Pools*: protocol, algorithm, session persistence, the listeners, members and monitor
/// using them (links to their tabs), status and each pool's menu; *Create Pool* above.
fn pools(d: LoadBalancerDetail, tab: RwSignal<usize>) -> impl IntoView {
    let i18n = use_i18n();
    let (rows, dd, da) = (d.pools.clone(), d.clone(), d.clone());
    let sel = select(pool_key);
    let columns = vec![
        col(
            K::Name,
            |p: &LbPool, _| view! { <span class="strong">{p.name.clone()}</span> },
        )
        .sort_text(|p| p.name.clone()),
        col(K::Protocol, |p: &LbPool, _| mono(p.protocol.clone())),
        col(K::Algorithm, |p: &LbPool, _| mono(p.algorithm.clone())),
        col(K::SessionPersistence, |p: &LbPool, _| {
            dash(p.session_persistence.clone())
        }),
        col(K::Listeners, move |p: &LbPool, _| refs(&p.listeners, tab, 1)),
        col(K::Members, |p: &LbPool, _| p.members.to_string())
            .sort_num(|p| p.members as i64)
            .class("num"),
        col(K::HealthMonitor, move |p: &LbPool, _| {
            p.monitor
                .as_ref()
                .map(|m| tab_link(m, tab, 4))
                .unwrap_or_else(|| "-".into_any())
        }),
        col(K::Status, |p: &LbPool, lang| {
            statuses(&p.operating_status, &p.provisioning_status, lang)
        }),
        col(K::AdminState, |p: &LbPool, lang| admin_state(lang, p.admin_up)),
        col(
            K::Actions,
            move |p: &LbPool, lang| view! { <RowActions items=ops::pool_items(lang, &da, p)/> },
        )
        .class("actions"),
    ];
    view! {
        {tab_actions(K::DeletePools, sel.picked, |id| OsAction::PoolDelete { id },
            Some(create_button(K::CreatePool, move || ops::pool_dialog(i18n.lang(), &dd, None))))}
        <DataTable rows=rows columns=columns select=sel search=|p: &LbPool| format!("{} {} {}", p.name, p.protocol, p.algorithm)/>
    }
}

/// *Members*: every pool's members (address, port, weight, subnet, backup), each with its
/// pool and menu; *Add Member* only once the load balancer has a pool.
fn members(d: LoadBalancerDetail) -> impl IntoView {
    let i18n = use_i18n();
    let (rows, dd, da) = (d.members.clone(), d.clone(), d.clone());
    let sel = select(member_key);
    // Members go into a pool: none yet, no button.
    let can_add = !d.pools.is_empty();
    let columns = vec![
        col(
            K::Name,
            |m: &LbMember, _| view! { <span class="strong">{m.name.clone()}</span> },
        )
        .sort_text(|m| m.name.clone()),
        col(K::Pool, |m: &LbMember, _| m.pool.name.clone()).sort_text(|m| m.pool.name.clone()),
        col(K::IpAddress, |m: &LbMember, _| mono(m.address.clone())).sort_text(|m| m.address.clone()),
        col(K::Port, |m: &LbMember, _| num(m.port))
            .sort_num(|m| m.port.unwrap_or(0))
            .class("num"),
        col(K::Weight, |m: &LbMember, _| num(m.weight))
            .sort_num(|m| m.weight.unwrap_or(0))
            .class("num"),
        col(K::Subnet, |m: &LbMember, _| match m.subnet.clone() {
            Some(s) => view! { <A href=format!("/project/networks/subnets/{}", s.id)>{s.name}</A> }.into_any(),
            None => "-".into_any(),
        }),
        col(K::Backup, |m: &LbMember, lang| yes_no(lang, m.backup)),
        col(K::Status, |m: &LbMember, lang| {
            statuses(&m.operating_status, &m.provisioning_status, lang)
        }),
        col(K::AdminState, |m: &LbMember, lang| admin_state(lang, m.admin_up)),
        col(
            K::Actions,
            move |m: &LbMember, lang| view! { <RowActions items=ops::member_items(lang, &da, m)/> },
        )
        .class("actions"),
    ];
    view! {
        {tab_actions(K::DeleteMembers, sel.picked, |key| {
            let (pool_id, id) = split_pair(&key);
            OsAction::MemberDelete { pool_id, id }
        }, can_add.then(|| create_button(K::AddMember, move || ops::member_dialog(i18n.lang(), &dd, None))))}
        <DataTable rows=rows columns=columns select=sel search=|m: &LbMember| format!("{} {} {} {:?}", m.name, m.pool.name, m.address, m.port)/>
    }
}

/// *Health Monitors*: type, delay, timeout, retries, the HTTP check and its pool; *Create
/// Health Monitor* only while a pool lacks one (Octavia allows one per pool).
fn monitors(d: LoadBalancerDetail) -> impl IntoView {
    let i18n = use_i18n();
    let (rows, dd, da) = (d.monitors.clone(), d.clone(), d.clone());
    let sel = select(monitor_key);
    // One monitor per pool: only pools without one can get a new monitor.
    let can_add = d.pools.iter().any(|p| p.monitor.is_none());
    let columns = vec![
        col(
            K::Name,
            |m: &LbMonitor, _| view! { <span class="strong">{m.name.clone()}</span> },
        )
        .sort_text(|m| m.name.clone()),
        col(K::Type, |m: &LbMonitor, _| mono(m.kind.clone())),
        col(K::DelaySeconds, |m: &LbMonitor, _| num(m.delay)).class("num"),
        col(K::TimeoutSeconds, |m: &LbMonitor, _| num(m.timeout)).class("num"),
        col(K::MaxRetries, |m: &LbMonitor, _| num(m.max_retries)).class("num"),
        col(K::MaxRetriesDown, |m: &LbMonitor, _| num(m.max_retries_down)).class("num"),
        col(K::HttpCheck, |m: &LbMonitor, _| mono(dash(m.http_check.clone()))),
        col(K::Pools, |m: &LbMonitor, _| {
            m.pools.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", ")
        }),
        col(K::Status, |m: &LbMonitor, lang| {
            statuses(&m.operating_status, &m.provisioning_status, lang)
        }),
        col(K::AdminState, |m: &LbMonitor, lang| admin_state(lang, m.admin_up)),
        col(
            K::Actions,
            move |m: &LbMonitor, lang| view! { <RowActions items=ops::monitor_items(lang, &da, m)/> },
        )
        .class("actions"),
    ];
    view! {
        {tab_actions(K::DeleteHealthMonitors, sel.picked, |id| OsAction::MonitorDelete { id },
            can_add.then(|| create_button(K::CreateHealthMonitor, move || ops::monitor_dialog(i18n.lang(), &dd, None))))}
        <DataTable rows=rows columns=columns select=sel search=|m: &LbMonitor| format!("{} {}", m.name, m.kind)/>
    }
}

/// An L7 policy's redirect target: its pool (to the *Pools* tab) or URL.
fn l7_target(p: &LbL7Policy, tab: RwSignal<usize>) -> AnyView {
    match (&p.redirect_pool, &p.redirect_url) {
        (Some(pool), _) => tab_link(pool, tab, 2),
        (_, Some(url)) => mono(url.clone()).into_any(),
        _ => "-".into_any(),
    }
}

/// *L7 Policies*: the policies, then their rules, each table with
/// its create button and row actions. Policies need an HTTP or
/// TERMINATED_HTTPS listener; rules need a policy.
fn l7(d: LoadBalancerDetail, tab: RwSignal<usize>) -> impl IntoView {
    let i18n = use_i18n();
    let (dp, da, dr, dra) = (d.clone(), d.clone(), d.clone(), d.clone());
    let can_create = !ops::l7_listeners(&d).is_empty();
    let can_add_rule = !d.l7policies.is_empty();
    let (policy_sel, rule_sel) = (select(policy_key), select(rule_key));
    let policies = d.l7policies.clone();
    let rules: Vec<(LbL7Policy, LbL7Rule)> = d
        .l7policies
        .iter()
        .flat_map(|p| p.rules.iter().map(move |r| (p.clone(), r.clone())))
        .collect();
    let policy_columns = vec![
        col(
            K::Name,
            |p: &LbL7Policy, _| view! { <span class="strong">{p.name.clone()}</span> },
        )
        .sort_text(|p| p.name.clone()),
        col(K::Listener, move |p: &LbL7Policy, _| tab_link(&p.listener, tab, 1)).sort_text(|p| p.listener.name.clone()),
        col(K::Position, |p: &LbL7Policy, _| num(p.position))
            .class("num")
            .sort_num(|p| p.position.unwrap_or(0)),
        col(K::Action, |p: &LbL7Policy, _| mono(p.action.clone())),
        col(K::L7Target, move |p: &LbL7Policy, _| l7_target(p, tab)),
        col(K::Rules, |p: &LbL7Policy, _| p.rules.len().to_string()).class("num"),
        col(K::Status, |p: &LbL7Policy, lang| {
            statuses(&p.operating_status, &p.provisioning_status, lang)
        }),
        col(K::AdminState, |p: &LbL7Policy, lang| admin_state(lang, p.admin_up)),
        col(
            K::Actions,
            move |p: &LbL7Policy, lang| view! { <RowActions items=ops::l7_policy_items(lang, &da, p)/> },
        )
        .class("actions"),
    ];
    let rule_columns = vec![
        col(K::L7Policy, |(p, _): &(LbL7Policy, LbL7Rule), _| p.name.clone()).sort_text(|(p, _)| p.name.clone()),
        col(K::RuleType, |(_, r): &(LbL7Policy, LbL7Rule), _| mono(r.rule_type.clone())),
        col(K::CompareType, |(_, r): &(LbL7Policy, LbL7Rule), _| mono(r.compare_type.clone())),
        col(K::L7Key, |(_, r): &(LbL7Policy, LbL7Rule), _| mono(dash(r.key.clone()))),
        col(K::Value, |(_, r): &(LbL7Policy, LbL7Rule), _| mono(r.value.clone())),
        col(K::Invert, |(_, r): &(LbL7Policy, LbL7Rule), lang| yes_no(lang, r.invert)),
        col(K::Status, |(_, r): &(LbL7Policy, LbL7Rule), lang| statuses(&r.operating_status, &r.provisioning_status, lang)),
        col(K::AdminState, |(_, r): &(LbL7Policy, LbL7Rule), lang| admin_state(lang, r.admin_up)),
        col(K::Actions, move |(p, r): &(LbL7Policy, LbL7Rule), lang| view! { <RowActions items=ops::l7_rule_items(lang, &dra, p, r)/> }).class("actions"),
    ];
    view! {
        <p class="muted">{move || i18n.t(if can_create { K::L7PoliciesHelp } else { K::L7NeedsHttpListener })}</p>
        {tab_actions(K::DeleteL7Policies, policy_sel.picked, |id| OsAction::L7PolicyDelete { id },
            can_create.then(|| create_button(K::CreateL7Policy, move || ops::l7_policy_dialog(i18n.lang(), &dp, None))))}
        <DataTable rows=policies columns=policy_columns select=policy_sel search=|p: &LbL7Policy| format!("{} {} {}", p.name, p.listener.name, p.action)/>
        <h3 class="subtable-title">{move || i18n.t(K::L7Rules)}</h3>
        {tab_actions(K::DeleteL7Rules, rule_sel.picked, |key| {
            let (policy_id, id) = split_pair(&key);
            OsAction::L7RuleDelete { policy_id, id }
        }, can_add_rule.then(|| create_button(K::AddL7Rule, move || ops::l7_rule_dialog(i18n.lang(), &dr, None, None))))}
        <DataTable rows=rules columns=rule_columns select=rule_sel search=|(p, r): &(LbL7Policy, LbL7Rule)| format!("{} {} {}", p.name, r.rule_type, r.value)/>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(id: &str, name: &str) -> Ref {
        Ref {
            id: id.into(),
            name: name.into(),
        }
    }

    #[test]
    fn members_and_rules_carry_their_parent_in_the_selection_key() {
        let mut m = LbMember {
            id: "m1".into(),
            name: String::new(),
            pool: r("p1", "web"),
            address: "10.0.0.5".into(),
            port: Some(80),
            weight: None,
            subnet: None,
            backup: false,
            operating_status: "ONLINE".into(),
            provisioning_status: "ACTIVE".into(),
            admin_up: true,
            monitor_address: None,
            monitor_port: None,
        };
        let (key, label) = member_key(&m).unwrap();
        assert_eq!(
            (key.as_str(), label.as_str()),
            ("p1/m1", "10.0.0.5:80"),
            "unnamed: address and port"
        );
        assert_eq!(split_pair(&key), ("p1".into(), "m1".into()));
        m.name = "app-1".into();
        assert_eq!(member_key(&m).unwrap().1, "app-1");
        let rule = LbL7Rule {
            id: "r1".into(),
            rule_type: "HEADER".into(),
            compare_type: "EQUAL_TO".into(),
            key: Some("X-Env".into()),
            value: "beta".into(),
            invert: false,
            operating_status: "ONLINE".into(),
            provisioning_status: "ACTIVE".into(),
            admin_up: true,
        };
        let policy = LbL7Policy {
            id: "l7p".into(),
            name: String::new(),
            description: None,
            listener: r("li", "http"),
            action: "REJECT".into(),
            position: Some(1),
            redirect_pool: None,
            redirect_url: None,
            operating_status: "ONLINE".into(),
            provisioning_status: "ACTIVE".into(),
            admin_up: true,
            rules: vec![],
        };
        assert_eq!(
            rule_key(&(policy, rule)),
            Some(("l7p/r1".into(), "l7p: HEADER X-Env EQUAL_TO beta".into())),
            "rules have no name: the policy (its ID when unnamed) and what the rule matches"
        );
        assert_eq!(named_key("i", " "), Some(("i".into(), "i".into())));
    }
}
