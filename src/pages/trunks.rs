//! Trunk detail: the parent port and the sub-ports carried on it, with
//! *Add Sub-port* and per-row removal. The list is in `lists.rs`.

use std::sync::Arc;

use leptos::prelude::*;

use super::details::{Card, DetailHeader, Kv, mono, param};
use super::lists::{admin_state, link, status_pill};
use crate::api::*;
use crate::components::dialog::{CreateButton, RowActions, data_version};
use crate::components::ops;
use crate::components::table::{DataTable, col};
use crate::components::widgets::{Copyable, Skeleton, dash, render};
use crate::i18n::{K, use_i18n};
use crate::model::*;

#[component]
pub fn TrunkDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_trunk(id));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || render(data.get(), move |td| {
                let tk = td.data;
                let whole = tk.clone();
                let items = Signal::derive(move || ops::trunk_items(i18n.lang(), &whole));
                let (tid, tid2) = (tk.id.clone(), tk.id.clone());
                let status = tk.status.clone();
                let columns = vec![
                    col(K::Port, |s: &Subport, _| link(format!("/project/networks/ports/{}", s.port_id), s.port_name.clone())).sort_text(|s| s.port_name.clone()),
                    col(K::SegmentationType, |s: &Subport, _| mono(s.segmentation_type.clone())),
                    col(K::SegmentationId, |s: &Subport, _| s.segmentation_id.map(|n| n.to_string()).unwrap_or_else(|| "-".into()))
                        .sort_num(|s| s.segmentation_id.unwrap_or(0))
                        .class("num"),
                    col(K::Actions, move |s: &Subport, l| view! { <RowActions items=ops::subport_items(l, &tid2, s)/> }).class("actions"),
                ];
                view! {
                    <DetailHeader group=K::NetworkGroup panel=K::Trunks href="/project/trunks"
                        name=tk.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=tk.name.clone()/></Kv>
                            <Kv label=K::Id><Copyable value=tk.id.clone() mono=true/></Kv>
                            <Kv label=K::Description>{dash(tk.description.clone())}</Kv>
                            <Kv label=K::ParentPort>{link(format!("/project/networks/ports/{}", tk.parent.id), tk.parent.name.clone())}</Kv>
                            <Kv label=K::Status>{move || status_pill(&status, i18n.lang())}</Kv>
                            <Kv label=K::AdminState>{move || admin_state(i18n.lang(), tk.admin_up)}</Kv>
                        </Card>
                    </div>
                    <h2 class="section-title">{move || i18n.t(K::Subports)}</h2>
                    <div class="subtable-actions">
                        <CreateButton label=Signal::derive(move || i18n.t(K::AddSubport).to_string())
                            open=Arc::new(move || ops::add_subport_dialog(i18n.lang(), &tid))/>
                    </div>
                    <DataTable rows=tk.subports columns=columns search=|s: &Subport| format!("{} {} {:?}", s.port_name, s.segmentation_type, s.segmentation_id)/>
                }
            })}
        </Transition>
    }
}
