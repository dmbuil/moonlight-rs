//! Network QoS policy detail: its rules, editable for the project's own
//! policies (shared ones of other projects are read-only). The list is in
//! `lists.rs`.

use std::sync::Arc;

use leptos::prelude::*;

use super::details::{param, yes_no, Card, DetailHeader, Kv};
use crate::api::*;
use crate::components::dialog::{data_version, CreateButton, RowActions};
use crate::components::ops;
use crate::components::table::{col, DataTable};
use crate::components::widgets::{dash, render, Copyable, Skeleton};
use crate::i18n::{use_i18n, K};
use crate::model::*;

fn num(n: Option<i64>) -> String {
    n.map(|n| n.to_string()).unwrap_or_else(|| "-".into())
}

#[component]
pub fn QosPolicyDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_qos_policy(id));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || render(data.get(), move |td| {
                let p = td.data;
                let whole = p.clone();
                let items = Signal::derive(move || ops::qos_policy_items(i18n.lang(), &whole));
                let (pid, pid2, owned) = (p.id.clone(), p.id.clone(), p.owned);
                let columns = vec![
                    col(K::Type, |r: &QosRule, l| ops::qos_kind(l, &r.kind)).sort_text(|r| r.kind.clone()),
                    col(K::Direction, |r: &QosRule, _| dash(r.direction.clone())),
                    col(K::MaxKbps, |r: &QosRule, _| num(r.max_kbps)).class("num"),
                    col(K::MaxBurstKbps, |r: &QosRule, _| num(r.max_burst_kbps)).class("num"),
                    col(K::DscpMark, |r: &QosRule, _| num(r.dscp_mark)).class("num"),
                    col(K::MinKbps, |r: &QosRule, _| num(r.min_kbps)).class("num"),
                    col(K::MinKpps, |r: &QosRule, _| num(r.min_kpps)).class("num"),
                    col(K::Actions, move |r: &QosRule, l| {
                        let items = if owned { ops::qos_rule_items(l, &pid2, r) } else { vec![] };
                        view! { <RowActions items=items/> }
                    })
                    .class("actions"),
                ];
                view! {
                    <DetailHeader group=K::NetworkGroup panel=K::NetworkQos href="/project/qos"
                        name=p.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=p.name.clone()/></Kv>
                            <Kv label=K::Id><Copyable value=p.id.clone() mono=true/></Kv>
                            <Kv label=K::Description>{dash(p.description.clone())}</Kv>
                            <Kv label=K::Shared>{move || yes_no(i18n.lang(), p.shared)}</Kv>
                            <Kv label=K::DefaultPolicy>{move || yes_no(i18n.lang(), p.is_default)}</Kv>
                        </Card>
                    </div>
                    <h2 class="section-title">{move || i18n.t(K::Rules)}</h2>
                    {owned.then(|| view! {
                        <div class="subtable-actions">
                            <CreateButton label=Signal::derive(move || i18n.t(K::AddRule).to_string())
                                open=Arc::new(move || ops::qos_rule_dialog(i18n.lang(), &pid, None))/>
                        </div>
                    })}
                    <DataTable rows=p.rules columns=columns search=|r: &QosRule| r.kind.clone()/>
                }
            })}
        </Transition>
    }
}
