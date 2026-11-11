//! Detail pages for every clickable resource, following Horizon's layout:
//! breadcrumb, title + actions, tabs, and key/value cards or sub-tables.

use std::sync::Arc;

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;

use super::lists::{admin_state, attachments, link, moving_status, status_pill};
use crate::api::*;
use crate::components::dialog::{CreateButton, MenuItem, RowActions, caps, data_version};
use crate::components::ops;
use crate::components::table::{DataTable, TimingChip, col};
use crate::components::widgets::{
    Copyable, Skeleton, dash, poll_while_busy, render, short_time, snapshot_busy, volume_busy,
};
use crate::i18n::{K, Lang, bytes, image_visibility, server_group_policy, server_status, t, use_i18n};
use crate::model::*;

// ---------------------------------------------------------------------------
// Shared building blocks
// ---------------------------------------------------------------------------

pub(crate) fn param(name: &'static str) -> impl Fn() -> String + Copy + Send + Sync + 'static {
    let params = use_params_map();
    move || params.read().get(name).unwrap_or_default()
}

/// Breadcrumb, title, timing and the action menu of a detail page.
#[component]
pub(crate) fn DetailHeader(
    group: K,
    panel: K,
    href: &'static str,
    #[prop(into)] name: String,
    timing: (u64, u32),
    #[prop(into)] items: Signal<Vec<MenuItem>>,
    #[prop(optional, into)] title_prefix: Option<K>,
) -> impl IntoView {
    let i18n = use_i18n();
    let title = name.clone();
    let location = leptos_router::hooks::use_location();
    let dashboard = move || crate::components::nav::dashboard(&location.pathname.get()).0;
    view! {
        <Title text=title.clone()/>
        <nav class="breadcrumb" aria-label="Breadcrumb">
            <span>{move || i18n.t(dashboard())}</span>
            // Identity's only group is named like the dashboard: *Identity › Users › name*.
            {move || (group != dashboard()).then(|| view! {
                <span class="sep" aria-hidden="true">"/"</span>
                <span>{i18n.t(group)}</span>
            })}
            <span class="sep" aria-hidden="true">"/"</span>
            <A href=href>{move || i18n.t(panel)}</A>
            <span class="sep" aria-hidden="true">"/"</span>
            <span>{name.clone()}</span>
        </nav>
        <div class="page-header">
            <h1>
                {title_prefix.map(|k| view! { <span class="muted">{move || format!("{}: ", i18n.t(k))}</span> })}
                {title}
            </h1>
            <div class="page-actions">
                <TimingChip millis=timing.0 calls=timing.1/>
                {move || view! { <RowActions items=items.get()/> }}
            </div>
        </div>
    }
}

#[component]
pub(crate) fn Tabs(labels: Vec<K>, active: RwSignal<usize>) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="tabs" role="tablist">
            {labels.into_iter().enumerate().map(|(i, k)| view! {
                <button type="button" role="tab"
                    aria-selected=move || (active.get() == i).to_string()
                    class:active=move || active.get() == i
                    on:click=move |_| active.set(i)>
                    {move || i18n.t(k)}
                </button>
            }).collect_view()}
        </div>
    }
}

#[component]
pub(crate) fn Card(title: K, children: Children) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <section class="card">
            <h2>{move || i18n.t(title)}</h2>
            <dl class="kv">{children()}</dl>
        </section>
    }
}

#[component]
pub(crate) fn Kv(label: K, children: Children) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <dt>{move || i18n.t(label)}</dt>
        <dd>{children()}</dd>
    }
}

pub(crate) fn yes_no(l: Lang, b: bool) -> &'static str {
    t(l, if b { K::Yes } else { K::No })
}

pub(crate) fn mono(s: impl Into<String>) -> AnyView {
    view! { <span class="mono">{s.into()}</span> }.into_any()
}

/// One monospace line per item, or `-`.
pub(crate) fn lines(items: Vec<String>) -> AnyView {
    if items.is_empty() {
        return "-".into_any();
    }
    view! { <ul class="plain">{items.into_iter().map(|x| view! { <li class="mono">{x}</li> }).collect_view()}</ul> }
        .into_any()
}

fn pairs(items: Vec<(String, String)>) -> AnyView {
    if items.is_empty() {
        return view! { <dt>"-"</dt><dd></dd> }.into_any();
    }
    items
        .into_iter()
        .map(|(k, v)| view! { <dt class="mono">{k}</dt><dd>{v}</dd> })
        .collect_view()
        .into_any()
}

fn port_label(p: &PortRow) -> String {
    if p.name.is_empty() {
        format!("({})", &p.id[..p.id.len().min(8)])
    } else {
        p.name.clone()
    }
}

fn device_cell(p: &PortRow, lang: Lang) -> AnyView {
    match (p.device_id.clone(), p.device_name.clone()) {
        (Some(id), Some(name)) => view! { <A href=format!("/project/instances/{id}")>{name}</A> }.into_any(),
        (Some(_), None) => dash(p.device_owner.clone()).into_any(),
        _ => t(lang, K::NotAttached).into_any(),
    }
}

// ---------------------------------------------------------------------------
// Network → Overview / Subnets / Ports
// ---------------------------------------------------------------------------

/// Network detail: overview, *Subnets* and *Ports* tabs.
#[component]
pub fn NetworkDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_network(id));
    let tab = RwSignal::new(0usize);
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let row = d.row.clone();
                let items = Signal::derive(move || ops::network_items(i18n.lang(), &row));
                let d = StoredValue::new(d);
                view! {
                    <DetailHeader group=K::NetworkGroup panel=K::Networks href="/project/networks"
                        name=d.with_value(|d| d.row.name.clone()) timing=(td.millis, td.calls) items=items/>
                    <Tabs labels=vec![K::Overview, K::Subnets, K::Ports] active=tab/>
                    {move || match tab.get() {
                        0 => d.with_value(|d| network_overview(d.clone())).into_any(),
                        1 => d.with_value(|d| network_subnets(d.clone())).into_any(),
                        _ => d.with_value(|d| network_ports(d.clone())).into_any(),
                    }}
                }
            })}
        </Transition>
    }
}

fn network_overview(d: NetworkDetail) -> impl IntoView {
    let i18n = use_i18n();
    let status = d.row.status.clone();
    view! {
        <div class="cards">
            <Card title=K::Information>
                <Kv label=K::Name><Copyable value=d.row.name.clone()/></Kv>
                <Kv label=K::Id><Copyable value=d.row.id.clone() mono=true/></Kv>
                <Kv label=K::Description>{dash(d.description.clone())}</Kv>
                <Kv label=K::Project><Copyable value=d.project_id.clone() mono=true/></Kv>
                <Kv label=K::Status>{move || status_pill(&status, i18n.lang())}</Kv>
                <Kv label=K::AdminState>{move || admin_state(i18n.lang(), d.row.admin_up)}</Kv>
                <Kv label=K::Shared>{move || yes_no(i18n.lang(), d.row.shared)}</Kv>
                <Kv label=K::External>{move || yes_no(i18n.lang(), d.row.external)}</Kv>
                <Kv label=K::Mtu>{d.mtu.map(|m| m.to_string()).unwrap_or_else(|| "-".into())}</Kv>
                <Kv label=K::PortSecurity>{move || yes_no(i18n.lang(), d.port_security)}</Kv>
                <Kv label=K::AvailabilityZone>{dash(Some(d.row.availability_zones.join(", ")))}</Kv>
                <Kv label=K::Created>{d.created.as_deref().map(short_time).unwrap_or_else(|| "-".into())}</Kv>
            </Card>
        </div>
    }
}

/// `owned`: the network is the project's; other networks' subnets are read-only.
fn subnet_columns(owned: bool) -> Vec<crate::components::table::Col<SubnetRow>> {
    let mut cols = vec![
        col(K::Name, |s: &SubnetRow, _| {
            link(format!("/project/networks/subnets/{}", s.id), s.name.clone())
        })
        .sort_text(|s| s.name.clone()),
        col(K::NetworkAddress, |s: &SubnetRow, _| mono(s.cidr.clone())).sort_text(|s| s.cidr.clone()),
        col(K::IpVersion, |s: &SubnetRow, _| format!("IPv{}", s.ip_version)),
        col(K::GatewayIp, |s: &SubnetRow, _| mono(dash(s.gateway_ip.clone()))),
        col(K::EnableDhcp, |s: &SubnetRow, l| yes_no(l, s.enable_dhcp)),
    ];
    if owned {
        cols.push(
            col(
                K::Actions,
                |s: &SubnetRow, l| view! { <RowActions items=ops::subnet_items(l, s)/> },
            )
            .class("actions"),
        );
    }
    cols
}

fn network_subnets(d: NetworkDetail) -> impl IntoView {
    let i18n = use_i18n();
    let (id, owned) = (d.row.id.clone(), d.row.owned);
    view! {
        {owned.then(|| view! {
            <div class="subtable-actions">
                <CreateButton label=Signal::derive(move || i18n.t(K::CreateSubnet).to_string())
                    open=Arc::new(move || ops::create_subnet_dialog(i18n.lang(), &id))/>
            </div>
        })}
        <DataTable rows=d.subnets columns=subnet_columns(owned) search=|s: &SubnetRow| format!("{} {} {}", s.name, s.id, s.cidr)/>
    }
}

fn port_columns() -> Vec<crate::components::table::Col<PortRow>> {
    vec![
        col(K::Name, |p: &PortRow, _| {
            link(format!("/project/networks/ports/{}", p.id), port_label(p))
        })
        .sort_text(port_label),
        col(K::FixedIps, |p: &PortRow, _| lines(p.fixed_ips.clone())),
        col(K::MacAddress, |p: &PortRow, _| mono(p.mac.clone())),
        col(K::AttachedDevice, |p: &PortRow, l| device_cell(p, l)),
        col(K::Status, |p: &PortRow, l| status_pill(&p.status, l)).sort_text(|p| p.status.clone()),
        col(K::AdminState, |p: &PortRow, l| admin_state(l, p.admin_up)),
        col(
            K::Actions,
            |p: &PortRow, l| view! { <RowActions items=ops::port_items(l, p)/> },
        )
        .class("actions"),
    ]
}

fn port_search(p: &PortRow) -> String {
    format!(
        "{} {} {} {} {}",
        p.name,
        p.id,
        p.fixed_ips.join(" "),
        p.mac,
        p.device_name.as_deref().unwrap_or("")
    )
}

fn network_ports(d: NetworkDetail) -> impl IntoView {
    let i18n = use_i18n();
    let id = d.row.id.clone();
    view! {
        <div class="subtable-actions">
            <CreateButton label=Signal::derive(move || i18n.t(K::CreatePort).to_string())
                open=Arc::new(move || ops::create_port_dialog(i18n.lang(), &id))/>
        </div>
        <DataTable rows=d.ports columns=port_columns() search=port_search/>
    }
}

// ---------------------------------------------------------------------------
// Subnet & port
// ---------------------------------------------------------------------------

/// Subnet detail page.
#[component]
pub fn SubnetDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_subnet(id));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let s = td.data;
                let s2 = s.clone();
                let items = Signal::derive(move || ops::subnet_items(i18n.lang(), &s2));
                let net = s.network_name.clone().unwrap_or_else(|| s.network_id.clone());
                view! {
                    <DetailHeader group=K::NetworkGroup panel=K::Networks href="/project/networks"
                        name=s.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=s.name.clone()/></Kv>
                            <Kv label=K::Id><Copyable value=s.id.clone() mono=true/></Kv>
                            <Kv label=K::Network><A href=format!("/project/networks/{}", s.network_id)>{net}</A></Kv>
                            <Kv label=K::Description>{dash(s.description.clone())}</Kv>
                            <Kv label=K::NetworkAddress><Copyable value=s.cidr.clone() mono=true/></Kv>
                            <Kv label=K::IpVersion>{format!("IPv{}", s.ip_version)}</Kv>
                            <Kv label=K::GatewayIp>{mono(dash(s.gateway_ip.clone()))}</Kv>
                            <Kv label=K::EnableDhcp>{move || yes_no(i18n.lang(), s.enable_dhcp)}</Kv>
                            <Kv label=K::AllocationPools>{lines(s.allocation_pools.clone())}</Kv>
                            <Kv label=K::DnsNameServers>{lines(s.dns_nameservers.clone())}</Kv>
                            <Kv label=K::HostRoutes>{lines(s.host_routes.clone())}</Kv>
                        </Card>
                    </div>
                }
            })}
        </Transition>
    }
}

/// Port detail page.
#[component]
pub fn PortDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_port(id));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let p = td.data;
                let p2 = p.clone();
                let items = Signal::derive(move || ops::port_items(i18n.lang(), &p2));
                let net = p.network_name.clone().unwrap_or_else(|| p.network_id.clone());
                let status = p.status.clone();
                let device = device_cell(&p, i18n.lang());
                let label = port_label(&p);
                view! {
                    <DetailHeader group=K::NetworkGroup panel=K::Networks href="/project/networks"
                        name=label timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name>{dash(Some(p.name.clone()))}</Kv>
                            <Kv label=K::Id><Copyable value=p.id.clone() mono=true/></Kv>
                            <Kv label=K::Network><A href=format!("/project/networks/{}", p.network_id)>{net}</A></Kv>
                            <Kv label=K::Description>{dash(p.description.clone())}</Kv>
                            <Kv label=K::MacAddress><Copyable value=p.mac.clone() mono=true/></Kv>
                            <Kv label=K::Status>{move || status_pill(&status, i18n.lang())}</Kv>
                            <Kv label=K::AdminState>{move || admin_state(i18n.lang(), p.admin_up)}</Kv>
                            <Kv label=K::VnicType>{dash(p.vnic_type.clone())}</Kv>
                        </Card>
                        <Card title=K::FixedIps>
                            <Kv label=K::IpAddress>{lines(p.fixed_ips.clone())}</Kv>
                        </Card>
                        <Card title=K::AttachedDevice>
                            <Kv label=K::DeviceOwner>{mono(dash(p.device_owner.clone()))}</Kv>
                            <Kv label=K::AttachedDevice>{device}</Kv>
                            <Kv label=K::SecurityGroups>{lines(p.security_groups.clone())}</Kv>
                        </Card>
                    </div>
                }
            })}
        </Transition>
    }
}

// ---------------------------------------------------------------------------
// Router → Overview / Interfaces / Static Routes
// ---------------------------------------------------------------------------

/// Router detail: overview, *Interfaces* and *Static Routes* tabs.
#[component]
pub fn RouterDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_router(id));
    let tab = RwSignal::new(0usize);
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let row = d.row.clone();
                let items = Signal::derive(move || ops::router_items(i18n.lang(), &row));
                let d = StoredValue::new(d);
                view! {
                    <DetailHeader group=K::NetworkGroup panel=K::Routers href="/project/routers"
                        name=d.with_value(|d| d.row.name.clone()) timing=(td.millis, td.calls) items=items/>
                    <Tabs labels=vec![K::Overview, K::Interfaces, K::StaticRoutes] active=tab/>
                    {move || match tab.get() {
                        0 => d.with_value(|d| router_overview(d.clone())).into_any(),
                        1 => d.with_value(|d| router_interfaces(d.clone())).into_any(),
                        _ => d.with_value(|d| router_routes(d.clone())).into_any(),
                    }}
                }
            })}
        </Transition>
    }
}

fn router_overview(d: RouterDetail) -> impl IntoView {
    let i18n = use_i18n();
    let status = d.row.status.clone();
    let ext = match (d.row.external_network_id.clone(), d.row.external_network.clone()) {
        (Some(id), name) => {
            let label = name.unwrap_or_else(|| id.clone());
            view! { <A href=format!("/project/networks/{id}")>{label}</A> }.into_any()
        }
        _ => "-".into_any(),
    };
    view! {
        <div class="cards">
            <Card title=K::Information>
                <Kv label=K::Name><Copyable value=d.row.name.clone()/></Kv>
                <Kv label=K::Id><Copyable value=d.row.id.clone() mono=true/></Kv>
                <Kv label=K::Description>{dash(d.description.clone())}</Kv>
                <Kv label=K::Project><Copyable value=d.project_id.clone() mono=true/></Kv>
                <Kv label=K::Status>{move || status_pill(&status, i18n.lang())}</Kv>
                <Kv label=K::AdminState>{move || admin_state(i18n.lang(), d.row.admin_up)}</Kv>
                <Kv label=K::AvailabilityZone>{dash(Some(d.row.availability_zones.join(", ")))}</Kv>
            </Card>
            <Card title=K::ExternalNetwork>
                <Kv label=K::ExternalNetwork>{ext}</Kv>
                <Kv label=K::ExternalFixedIps>{lines(d.gateway_ips.iter().map(|(ip, sn)| if sn.is_empty() { ip.clone() } else { format!("{ip} · {} {sn}", t(i18n.lang(), K::Subnet)) }).collect())}</Kv>
                <Kv label=K::Snat>{move || d.row.snat.map(|s| yes_no(i18n.lang(), s)).unwrap_or("-")}</Kv>
            </Card>
        </div>
    }
}

fn router_interfaces(d: RouterDetail) -> impl IntoView {
    let i18n = use_i18n();
    let rid = d.row.id.clone();
    let rid2 = rid.clone();
    let columns = vec![
        col(K::Name, |p: &PortRow, _| {
            link(format!("/project/networks/ports/{}", p.id), port_label(p))
        }),
        col(K::FixedIps, |p: &PortRow, _| lines(p.fixed_ips.clone())),
        col(K::Network, |p: &PortRow, _| {
            let n = p.network_name.clone().unwrap_or_else(|| p.network_id.clone());
            view! { <A href=format!("/project/networks/{}", p.network_id)>{n}</A> }
        }),
        col(K::Status, |p: &PortRow, l| status_pill(&p.status, l)),
        col(K::Type, |p: &PortRow, _| mono(dash(p.device_owner.clone()))),
        col(K::AdminState, |p: &PortRow, l| admin_state(l, p.admin_up)),
        col(
            K::Actions,
            move |p: &PortRow, l| view! { <RowActions items=ops::interface_items(l, &rid2, p)/> },
        )
        .class("actions"),
    ];
    view! {
        <div class="subtable-actions">
            <CreateButton label=Signal::derive(move || i18n.t(K::AddInterface).to_string())
                open=Arc::new(move || ops::add_interface_dialog(i18n.lang(), &rid))/>
        </div>
        <DataTable rows=d.interfaces columns=columns search=port_search/>
    }
}

fn router_routes(d: RouterDetail) -> impl IntoView {
    let i18n = use_i18n();
    let rid = d.row.id.clone();
    let rid2 = rid.clone();
    // Atomic add/remove needs `extraroute-atomic`; with plain `extraroute` the
    // whole list is replaced; with neither, routes are read-only (roadmap 19).
    let c = caps();
    let atomic = needs::STATIC_ROUTES.met(&c);
    let editable = atomic || needs::STATIC_ROUTES_LIST.met(&c);
    let current = (!atomic).then(|| d.routes.clone());
    let current2 = current.clone();
    let mut columns = vec![
        col(K::Destination, |r: &(String, String), _| mono(r.0.clone())),
        col(K::NextHop, |r: &(String, String), _| mono(r.1.clone())),
    ];
    if editable {
        columns.push(
            col(K::Actions, move |r: &(String, String), l| view! { <RowActions items=ops::route_items(l, &rid2, r, current2.as_deref())/> })
                .class("actions"),
        );
    }
    view! {
        {editable.then(|| view! {
            <div class="subtable-actions">
                <CreateButton label=Signal::derive(move || i18n.t(K::AddStaticRoute).to_string())
                    open=Arc::new(move || ops::add_route_dialog(i18n.lang(), &rid, current.clone()))/>
            </div>
        })}
        <DataTable rows=d.routes columns=columns search=|r: &(String, String)| format!("{} {}", r.0, r.1)/>
    }
}

// ---------------------------------------------------------------------------
// Security group → rules
// ---------------------------------------------------------------------------

/// Security group detail: its rules.
#[component]
pub fn SecurityGroupDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_security_group(id));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let g = td.data;
                let (gid, gname, gdesc) = (g.id.clone(), g.name.clone(), g.description.clone());
                let items = Signal::derive(move || ops::security_group_items(i18n.lang(), &gid, &gname, gdesc.as_deref(), false));
                let gid = g.id.clone();
                let columns = vec![
                    col(K::Direction, |r: &RuleRow, l| t(l, if r.direction == "egress" { K::Egress } else { K::Ingress }))
                        .sort_text(|r| r.direction.clone()),
                    col(K::EtherType, |r: &RuleRow, _| r.ethertype.clone()).sort_text(|r| r.ethertype.clone()),
                    col(K::IpProtocol, |r: &RuleRow, _| r.protocol.clone()).sort_text(|r| r.protocol.clone()),
                    col(K::PortRange, |r: &RuleRow, _| mono(r.port_range.clone())),
                    col(K::Remote, |r: &RuleRow, _| mono(r.remote.clone())),
                    col(K::Description, |r: &RuleRow, _| dash(r.description.clone())),
                    col(K::Actions, |r: &RuleRow, l| view! { <RowActions items=ops::rule_items(l, r)/> }).class("actions"),
                ];
                view! {
                    <DetailHeader group=K::NetworkGroup panel=K::SecurityGroups href="/project/security_groups"
                        name=g.name.clone() timing=(td.millis, td.calls) items=items title_prefix=K::ManageRules/>
                    <div class="subtable-actions">
                        <CreateButton label=Signal::derive(move || i18n.t(K::AddRule).to_string())
                            open=Arc::new(move || ops::add_rule_dialog(i18n.lang(), &gid))/>
                    </div>
                    <DataTable rows=g.rules columns=columns
                        search=|r: &RuleRow| format!("{} {} {} {} {}", r.direction, r.ethertype, r.protocol, r.port_range, r.remote)/>
                }
            })}
        </Transition>
    }
}

// ---------------------------------------------------------------------------
// Volume, snapshot, image, key pair
// ---------------------------------------------------------------------------

/// Volume detail: overview, snapshots and Cinder's messages; refreshes while the
/// volume changes state (roadmap 46).
#[component]
pub fn VolumeDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_volume(id));
    // Follows a volume that is changing state (roadmap 46). A volume deleted
    // meanwhile turns the next fetch into a 404: polling stops and the
    // friendly not-found page shows.
    let polling = poll_while_busy(data, move || {
        data.with(|r| matches!(r, Some(Ok(td)) if volume_busy(&td.data.volume.status)))
    });
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let v = d.volume.clone();
                let v2 = v.clone();
                let items = Signal::derive(move || ops::volume_items(i18n.lang(), &v2));
                let status = v.status.clone();
                let attached = attachments(&v);
                let snap_cols = vec![
                    col(K::Name, |s: &Snapshot, _| link(format!("/project/snapshots/{}", s.id), s.name.clone())),
                    col(K::Size, |s: &Snapshot, _| format!("{} GiB", s.size_gb)).class("num"),
                    col(K::Status, |s: &Snapshot, l| status_pill(&s.status, l)),
                    col(K::Created, |s: &Snapshot, _| short_time(&s.created)).class("nowrap"),
                ];
                let msg_cols = vec![
                    col(K::Created, |m: &VolumeMessage, _| short_time(&m.created)).sort_text(|m| m.created.clone()).class("nowrap"),
                    col(K::MessageLevel, |m: &VolumeMessage, _| m.level.clone()),
                    col(K::Message, |m: &VolumeMessage, _| m.message.clone()),
                    col(K::RequestId, |m: &VolumeMessage, _| view! { <span class="mono">{dash(m.request_id.clone())}</span> }),
                ];
                view! {
                    <DetailHeader group=K::VolumesGroup panel=K::Volumes href="/project/volumes"
                        name=v.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=v.name.clone()/></Kv>
                            <Kv label=K::Id><Copyable value=v.id.clone() mono=true/></Kv>
                            <Kv label=K::Description>{dash(v.description.clone())}</Kv>
                            <Kv label=K::Status>{move || moving_status(&status, i18n.lang(), volume_busy(&status))} {move || polling.get().then(|| view! { <span class="updating">{i18n.t(K::Updating)}</span> })}</Kv>
                            <Kv label=K::AvailabilityZone>{dash(v.availability_zone.clone())}</Kv>
                            <Kv label=K::Created>{short_time(&d.created)}</Kv>
                        </Card>
                        <Card title=K::Specs>
                            <Kv label=K::Size>{format!("{} GiB", v.size_gb)}</Kv>
                            <Kv label=K::VolumeType>{dash(v.volume_type.clone())}</Kv>
                            <Kv label=K::Bootable>{move || yes_no(i18n.lang(), v.bootable)}</Kv>
                            <Kv label=K::Encrypted>{move || yes_no(i18n.lang(), v.encrypted)}</Kv>
                            <Kv label=K::Multiattach>{move || yes_no(i18n.lang(), d.multiattach)}</Kv>
                            <Kv label=K::ImageName>{dash(d.source_image.clone())}</Kv>
                            <Kv label=K::SourceSnapshot>{match d.source_snapshot_id.clone() {
                                Some(s) => view! { <A href=format!("/project/snapshots/{s}")>{s.clone()}</A> }.into_any(),
                                None => "-".into_any(),
                            }}</Kv>
                        </Card>
                        <Card title=K::AttachedTo>
                            <Kv label=K::Instance>{attached}</Kv>
                        </Card>
                        <Card title=K::Metadata>{pairs(d.metadata.clone())}</Card>
                        <Card title=K::ImageMetadata>{pairs(d.image_metadata.clone())}</Card>
                    </div>
                    <h2 class="section-title">{move || i18n.t(K::Snapshots)}</h2>
                    <DataTable rows=d.snapshots.clone() columns=snap_cols search=|s: &Snapshot| s.name.clone()/>
                    {move || needs::VOLUME_MESSAGES.met(&caps()).then(|| {
                        let (rows, cols) = (d.messages.clone(), msg_cols.clone());
                        view! {
                            <h2 class="section-title">{move || i18n.t(K::VolumeMessages)}</h2>
                            <DataTable rows=rows columns=cols search=|m: &VolumeMessage| format!("{} {}", m.message, m.level)/>
                        }
                    })}
                }
            })}
        </Transition>
    }
}

/// Volume snapshot detail; refreshes while the snapshot changes state (roadmap 46).
#[component]
pub fn SnapshotDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_snapshot(id));
    // As for volumes (roadmap 46).
    let polling = poll_while_busy(data, move || {
        data.with(|r| matches!(r, Some(Ok(td)) if snapshot_busy(&td.data.status)))
    });
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || render(data.get(), move |td| {
                let s = td.data;
                let s2 = s.clone();
                let items = Signal::derive(move || ops::snapshot_items(i18n.lang(), &s2));
                let status = s.status.clone();
                let vol = s.volume_name.clone().unwrap_or_else(|| s.volume_id.clone());
                view! {
                    <DetailHeader group=K::VolumesGroup panel=K::Snapshots href="/project/snapshots"
                        name=s.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=s.name.clone()/></Kv>
                            <Kv label=K::Id><Copyable value=s.id.clone() mono=true/></Kv>
                            <Kv label=K::Description>{dash(s.description.clone())}</Kv>
                            <Kv label=K::Status>{move || moving_status(&status, i18n.lang(), snapshot_busy(&status))} {move || polling.get().then(|| view! { <span class="updating">{i18n.t(K::Updating)}</span> })}</Kv>
                            <Kv label=K::Size>{format!("{} GiB", s.size_gb)}</Kv>
                            <Kv label=K::Volume><A href=format!("/project/volumes/{}", s.volume_id)>{vol}</A></Kv>
                            <Kv label=K::Created>{short_time(&s.created)}</Kv>
                        </Card>
                    </div>
                }
            })}
        </Transition>
    }
}

/// Volume backup detail (roadmap 5, `backup` plugin); refreshes while it is `creating`,
/// `restoring` or `deleting`, which `snapshot_busy` covers.
#[component]
pub fn BackupDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_backup(id));
    let polling = poll_while_busy(data, move || {
        data.with(|r| matches!(r, Some(Ok(td)) if snapshot_busy(&td.data.status)))
    });
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || render(data.get(), move |td| {
                let b = td.data;
                let b2 = b.clone();
                let items = Signal::derive(move || ops::backup_items(i18n.lang(), &b2));
                let status = b.status.clone();
                view! {
                    <DetailHeader group=K::VolumesGroup panel=K::Backups href="/project/backups"
                        name=b.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=b.name.clone()/></Kv>
                            <Kv label=K::Id><Copyable value=b.id.clone() mono=true/></Kv>
                            <Kv label=K::Description>{dash(b.description.clone())}</Kv>
                            <Kv label=K::Status>{move || moving_status(&status, i18n.lang(), snapshot_busy(&status))} {move || polling.get().then(|| view! { <span class="updating">{i18n.t(K::Updating)}</span> })}</Kv>
                            <Kv label=K::FailReason>{dash(b.fail_reason.clone())}</Kv>
                            <Kv label=K::Size>{format!("{} GiB", b.size_gb)}</Kv>
                            <Kv label=K::Volume><A href=format!("/project/volumes/{}", b.volume.id)>{b.volume.name.clone()}</A></Kv>
                            <Kv label=K::SourceSnapshot>{dash(b.snapshot_id.clone())}</Kv>
                            <Kv label=K::IncrementalBackup>{t(i18n.lang(), if b.incremental { K::Yes } else { K::No })}</Kv>
                            <Kv label=K::BackupContainer>{dash(b.container.clone())}</Kv>
                            <Kv label=K::AvailabilityZone>{dash(b.availability_zone.clone())}</Kv>
                            <Kv label=K::Created>{short_time(&b.created)}</Kv>
                        </Card>
                    </div>
                }
            })}
        </Transition>
    }
}

/// Image detail: overview and custom properties.
#[component]
pub fn ImageDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_image(id));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let im = d.image.clone();
                let im2 = im.clone();
                let items = Signal::derive(move || ops::image_items(i18n.lang(), &im2));
                let status = im.status.clone();
                let owner = super::lists::owner_cell(&im);
                let (description, vis) = (dash(im.property("description").map(str::to_string)), im.visibility.clone());
                view! {
                    <DetailHeader group=K::Compute panel=K::Images href="/project/images"
                        name=im.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=im.name.clone()/></Kv>
                            <Kv label=K::Id><Copyable value=im.id.clone() mono=true/></Kv>
                            <Kv label=K::Owner>
                                {owner}
                                {d.owner.clone().map(|o| view! { <div class="mono muted">{o}</div> })}
                            </Kv>
                            <Kv label=K::Status>{move || status_pill(&status, i18n.lang())}</Kv>
                            <Kv label=K::Type>{move || t(i18n.lang(), if im.is_snapshot { K::ImageTypeSnapshot } else { K::ImageTypeImage })}</Kv>
                            <Kv label=K::Description>{description}</Kv>
                            <Kv label=K::Visibility>{move || image_visibility(i18n.lang(), &vis)}</Kv>
                            <Kv label=K::Protected>{move || yes_no(i18n.lang(), im.protected)}</Kv>
                            <Kv label=K::Created>{d.created.as_deref().map(short_time).unwrap_or_else(|| "-".into())}</Kv>
                            <Kv label=K::Updated>{d.updated.as_deref().map(short_time).unwrap_or_else(|| "-".into())}</Kv>
                        </Card>
                        <Card title=K::Specs>
                            <Kv label=K::Size>{im.size.map(bytes).unwrap_or_else(|| "-".into())}</Kv>
                            <Kv label=K::VirtualSize>{d.virtual_size.map(bytes).unwrap_or_else(|| "-".into())}</Kv>
                            <Kv label=K::DiskFormat>{dash(im.disk_format.clone()).to_uppercase()}</Kv>
                            <Kv label=K::ContainerFormat>{dash(d.container_format.clone()).to_uppercase()}</Kv>
                            <Kv label=K::MinDisk>{d.image.min_disk_gb.map(|g| format!("{g} GB")).unwrap_or_else(|| "-".into())}</Kv>
                            <Kv label=K::MinRam>{d.image.min_ram_mb.map(|m| format!("{m} MB")).unwrap_or_else(|| "-".into())}</Kv>
                            <Kv label=K::Checksum>{mono(dash(d.checksum.clone()))}</Kv>
                            <Kv label=K::Tags>{dash(Some(d.tags.join(", ")))}</Kv>
                        </Card>
                        <Card title=K::CustomProperties>{pairs(d.properties.iter().filter(|(k, _)| k != "description").cloned().collect())}</Card>
                    </div>
                }
            })}
        </Transition>
    }
}

/// Server group detail: policy, rules and the member instances (roadmap 52).
#[component]
pub fn ServerGroupDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(i, _)| get_server_group(i));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let g = d.group.clone();
                let items = Signal::derive(move || ops::server_group_detail_items(i18n.lang(), &g));
                let policy = d.group.policies.clone();
                let rules: Vec<String> = d.rules.iter().map(|(k, v)| format!("{k} = {v}")).collect();
                let columns = vec![
                    col(K::Name, |m: &GroupMember, _| link(format!("/project/instances/{}", m.id), m.name.clone().unwrap_or_else(|| m.id.clone())))
                        .sort_text(|m| m.name.clone().unwrap_or_default()),
                    col(K::Status, |m: &GroupMember, l| match &m.status {
                        Some(s) => server_status(l, s).into_any(),
                        None => "-".into_any(),
                    }),
                    col(K::Id, |m: &GroupMember, _| mono(m.id.clone())),
                ];
                view! {
                    <DetailHeader group=K::Compute panel=K::ServerGroups href="/project/server_groups"
                        name=d.group.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=d.group.name.clone()/></Kv>
                            <Kv label=K::Id><Copyable value=d.group.id.clone() mono=true/></Kv>
                            <Kv label=K::Policy>{move || dash(Some(policy.iter().map(|p| server_group_policy(i18n.lang(), p)).collect::<Vec<_>>().join(", ")))}</Kv>
                            <Kv label=K::Rules>{lines(rules)}</Kv>
                            <Kv label=K::ProjectId>{mono(dash(d.project_id.clone()))}</Kv>
                            <Kv label=K::UserId>{mono(dash(d.user_id.clone()))}</Kv>
                        </Card>
                    </div>
                    <h2 class="section-title">{move || i18n.t(K::Members)}</h2>
                    <DataTable rows=d.members columns=columns search=|m: &GroupMember| format!("{} {}", m.name.clone().unwrap_or_default(), m.id)/>
                }
            })}
        </Transition>
    }
}

/// Key pair detail: type, fingerprint and public key.
#[component]
pub fn KeyPairDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let name = param("name");
    let data = Resource::new(move || (name(), data_version()), |(n, _)| get_keypair(n));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=4/> }>
            {move || render(data.get(), move |td| {
                let k = td.data;
                let n = k.name.clone();
                let items = Signal::derive(move || ops::keypair_items(i18n.lang(), &n));
                view! {
                    <DetailHeader group=K::Compute panel=K::KeyPairs href="/project/key_pairs"
                        name=k.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=k.name.clone()/></Kv>
                            <Kv label=K::Type>{dash(k.key_type.clone())}</Kv>
                            <Kv label=K::Fingerprint><Copyable value=k.fingerprint.clone() mono=true/></Kv>
                            <Kv label=K::Created>{k.created.as_deref().map(short_time).unwrap_or_else(|| "-".into())}</Kv>
                        </Card>
                    </div>
                    <h2 class="section-title">{move || i18n.t(K::PublicKey)}</h2>
                    <div class="pubkey"><Copyable value=k.public_key.clone() mono=true/></div>
                }
            })}
        </Transition>
    }
}
