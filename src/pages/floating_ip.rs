//! Floating IP detail: Overview and, when Neutron has the
//! `floating-ip-port-forwarding` extension, its port forwarding rules
//! (Horizon's *Manage Port Forwarding Rules*).

use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_query_map;

use super::details::{Card, DetailHeader, Kv, Tabs, mono, param};
use super::lists::{link, status_pill};
use crate::api::*;
use crate::components::dialog::{CreateButton, RowActions, caps, data_version};
use crate::components::ops;
use crate::components::table::{DataTable, col};
use crate::components::widgets::{Copyable, Skeleton, dash, render, short_time};
use crate::i18n::{K, use_i18n};
use crate::model::*;

#[component]
pub fn FloatingIpDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_floating_ip(id));
    let forwarding = Signal::derive(move || needs::PORT_FORWARDING.met(&caps()));
    // `?tab=forwarding` (the row menu's *Port Forwarding Rules*) opens the rules.
    let query = use_query_map();
    let tab = RwSignal::new(usize::from(
        query.read_untracked().get("tab").as_deref() == Some("forwarding"),
    ));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let row = d.row.clone();
                let items = Signal::derive(move || ops::floating_ip_items(i18n.lang(), &row));
                let d = StoredValue::new(d);
                view! {
                    <DetailHeader group=K::NetworkGroup panel=K::FloatingIps href="/project/floating_ips"
                        name=d.with_value(|d| d.row.ip.clone()) timing=(td.millis, td.calls) items=items/>
                    {move || if forwarding.get() {
                        view! { <Tabs labels=vec![K::Overview, K::PortForwarding] active=tab/> }.into_any()
                    } else {
                        ().into_any()
                    }}
                    {move || match (tab.get(), forwarding.get()) {
                        (1, true) => d.with_value(|d| rules(d.clone())).into_any(),
                        _ => d.with_value(|d| overview(d.clone())).into_any(),
                    }}
                }
            })}
        </Transition>
    }
}

fn overview(d: FloatingIpDetail) -> impl IntoView {
    let i18n = use_i18n();
    let status = d.row.status.clone();
    let mapped = match (d.row.instance_id.clone(), d.row.instance_name.clone()) {
        (Some(id), name) => {
            let label = name.unwrap_or_else(|| id.clone());
            view! { <A href=format!("/project/instances/{id}")>{label}</A> }.into_any()
        }
        _ => "-".into_any(),
    };
    let port = d
        .port_id
        .clone()
        .map(|p| link(format!("/project/networks/ports/{p}"), p))
        .unwrap_or_else(|| "-".into_any());
    let router = d
        .router_id
        .clone()
        .map(|r| link(format!("/project/routers/{r}"), r))
        .unwrap_or_else(|| "-".into_any());
    view! {
        <div class="cards">
            <Card title=K::Information>
                <Kv label=K::IpAddress><Copyable value=d.row.ip.clone() mono=true/></Kv>
                <Kv label=K::Id><Copyable value=d.row.id.clone() mono=true/></Kv>
                <Kv label=K::Description>{dash(d.row.description.clone())}</Kv>
                <Kv label=K::Pool>{d.row.pool.clone()}</Kv>
                <Kv label=K::Status>{move || status_pill(&status, i18n.lang())}</Kv>
                <Kv label=K::DnsName>{dash(d.row.dns_name.clone())}</Kv>
                <Kv label=K::Created>{d.created.as_deref().map(short_time).unwrap_or_else(|| "-".into())}</Kv>
            </Card>
            <Card title=K::MappedFixedIp>
                <Kv label=K::Instance>{mapped}</Kv>
                <Kv label=K::MappedFixedIp>{mono(dash(d.row.fixed_ip.clone()))}</Kv>
                <Kv label=K::Port>{port}</Kv>
                <Kv label=K::Router>{router}</Kv>
            </Card>
        </div>
    }
}

fn rules(d: FloatingIpDetail) -> impl IntoView {
    let i18n = use_i18n();
    let fip = d.row.id.clone();
    let fip2 = fip.clone();
    // Neutron refuses rules on a floating IP that is associated with a port.
    let associated = d.row.fixed_ip.is_some();
    let columns = vec![
        col(K::Protocol, |r: &PortForwardingRow, _| mono(r.protocol.to_uppercase())).sort_text(|r| r.protocol.clone()),
        col(K::ExternalPort, |r: &PortForwardingRow, _| mono(r.external.clone())).sort_text(|r| r.external.clone()),
        col(K::InternalIpAddress, |r: &PortForwardingRow, _| {
            mono(r.internal_ip.clone())
        })
        .sort_text(|r| r.internal_ip.clone()),
        col(K::InternalPort, |r: &PortForwardingRow, _| mono(r.internal.clone())),
        col(K::Instance, |r: &PortForwardingRow, _| match r.instance.clone() {
            Some(i) => view! { <A href=format!("/project/instances/{}", i.id)>{i.name}</A> }.into_any(),
            None => "-".into_any(),
        }),
        col(K::Description, |r: &PortForwardingRow, _| dash(r.description.clone())),
        col(
            K::Actions,
            move |r: &PortForwardingRow, l| view! { <RowActions items=ops::port_forwarding_items(l, &fip2, r)/> },
        )
        .class("actions"),
    ];
    view! {
        {if associated {
            view! { <div class="alert alert-info" role="status">{move || i18n.t(K::ForwardingNeedsUnassociated)}</div> }.into_any()
        } else {
            view! {
                <div class="subtable-actions">
                    <CreateButton label=Signal::derive(move || i18n.t(K::AddRule).to_string())
                        open=Arc::new(move || ops::port_forwarding_dialog(i18n.lang(), &fip, None))/>
                </div>
            }.into_any()
        }}
        <DataTable rows=d.forwardings columns=columns
            search=|r: &PortForwardingRow| format!("{} {} {} {} {}", r.protocol, r.external, r.internal_ip, r.internal, r.description.as_deref().unwrap_or(""))/>
    }
}
