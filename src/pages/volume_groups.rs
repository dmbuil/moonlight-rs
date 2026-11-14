//! Volume group and group snapshot detail pages (generic volume groups,
//!): overview plus the volumes or volume snapshots each holds. The
//! lists are in `lists.rs`. The header menus hold the group actions
//! (`ops::volume_group_detail_items`, `ops::group_snapshot_items`).

use leptos::prelude::*;

use super::details::{Card, DetailHeader, Kv, param};
use super::lists::{link, moving_status};
use crate::api::*;
use crate::components::dialog::data_version;
use crate::components::ops;
use crate::components::table::{DataTable, col};
use crate::components::widgets::{
    Copyable, Skeleton, dash, poll_while_busy, render, short_time, snapshot_busy, volume_busy,
};
use crate::i18n::{K, use_i18n};
use crate::model::*;

/// Volume group detail: overview, its volumes and its group snapshots.
#[component]
pub fn VolumeGroupDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_volume_group(id));
    // Same transitional statuses as snapshots (see `lists::VolumeGroupsPage`).
    let polling = poll_while_busy(data, move || {
        data.with(|r| matches!(r, Some(Ok(td)) if snapshot_busy(&td.data.row.status)))
    });
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let g = d.row.clone();
                let d2 = d.clone();
                let items = Signal::derive(move || ops::volume_group_detail_items(i18n.lang(), &d2));
                let status = g.status.clone();
                let types = g.volume_types.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", ");
                let volume_cols = vec![
                    col(K::Name, |v: &Volume, _| link(format!("/project/volumes/{}", v.id), v.name.clone())).sort_text(|v| v.name.clone()),
                    col(K::Size, |v: &Volume, _| format!("{} GiB", v.size_gb)).sort_num(|v| v.size_gb).class("num"),
                    col(K::Status, |v: &Volume, l| moving_status(&v.status, l, volume_busy(&v.status))).sort_text(|v| v.status.clone()),
                    col(K::VolumeType, |v: &Volume, _| dash(v.volume_type.clone())),
                ];
                let snapshot_cols = vec![
                    col(K::Name, |s: &GroupSnapshotRow, _| link(format!("/project/vg_snapshots/{}", s.id), s.name.clone())).sort_text(|s| s.name.clone()),
                    col(K::Status, |s: &GroupSnapshotRow, l| moving_status(&s.status, l, snapshot_busy(&s.status))).sort_text(|s| s.status.clone()),
                    col(K::Created, |s: &GroupSnapshotRow, _| short_time(&s.created)).sort_text(|s| s.created.clone()).class("nowrap"),
                ];
                view! {
                    <DetailHeader group=K::VolumesGroup panel=K::Groups href="/project/volume_groups"
                        name=g.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=g.name.clone()/></Kv>
                            <Kv label=K::Id><Copyable value=g.id.clone() mono=true/></Kv>
                            <Kv label=K::Description>{dash(g.description.clone())}</Kv>
                            <Kv label=K::Status>{move || moving_status(&status, i18n.lang(), snapshot_busy(&status))} {move || polling.get().then(|| view! { <span class="updating">{i18n.t(K::Updating)}</span> })}</Kv>
                            <Kv label=K::AvailabilityZone>{dash(g.availability_zone.clone())}</Kv>
                            <Kv label=K::GroupType>{g.group_type.name.clone()}</Kv>
                            <Kv label=K::VolumeTypes>{dash(Some(types).filter(|t| !t.is_empty()))}</Kv>
                            <Kv label=K::Created>{short_time(&g.created)}</Kv>
                        </Card>
                    </div>
                    <h2 class="section-title">{move || i18n.t(K::Volumes)}</h2>
                    <DataTable rows=d.volumes columns=volume_cols search=|v: &Volume| format!("{} {} {}", v.name, v.id, v.status)/>
                    <h2 class="section-title">{move || i18n.t(K::GroupSnapshots)}</h2>
                    <DataTable rows=d.snapshots columns=snapshot_cols search=|s: &GroupSnapshotRow| format!("{} {} {}", s.name, s.id, s.status)/>
                }
            })}
        </Transition>
    }
}

/// Group snapshot detail: overview and the volume snapshots taken with it.
#[component]
pub fn GroupSnapshotDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_group_snapshot(id));
    let polling = poll_while_busy(data, move || {
        data.with(|r| matches!(r, Some(Ok(td)) if snapshot_busy(&td.data.row.status)))
    });
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let s = d.row.clone();
                let s2 = s.clone();
                let items = Signal::derive(move || ops::group_snapshot_items(i18n.lang(), &s2));
                let status = s.status.clone();
                let columns = vec![
                    col(K::Name, |x: &Snapshot, _| link(format!("/project/snapshots/{}", x.id), x.name.clone())).sort_text(|x| x.name.clone()),
                    col(K::Volume, |x: &Snapshot, _| link(format!("/project/volumes/{}", x.volume_id), x.volume_name.clone().unwrap_or_else(|| x.volume_id.clone()))),
                    col(K::Size, |x: &Snapshot, _| format!("{} GiB", x.size_gb)).sort_num(|x| x.size_gb).class("num"),
                    col(K::Status, |x: &Snapshot, l| moving_status(&x.status, l, snapshot_busy(&x.status))).sort_text(|x| x.status.clone()),
                ];
                view! {
                    <DetailHeader group=K::VolumesGroup panel=K::GroupSnapshots href="/project/vg_snapshots"
                        name=s.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name><Copyable value=s.name.clone()/></Kv>
                            <Kv label=K::Id><Copyable value=s.id.clone() mono=true/></Kv>
                            <Kv label=K::Description>{dash(s.description.clone())}</Kv>
                            <Kv label=K::Status>{move || moving_status(&status, i18n.lang(), snapshot_busy(&status))} {move || polling.get().then(|| view! { <span class="updating">{i18n.t(K::Updating)}</span> })}</Kv>
                            <Kv label=K::VolumeGroup>{link(format!("/project/volume_groups/{}", s.group.id), s.group.name.clone())}</Kv>
                            <Kv label=K::GroupType>{s.group_type.name.clone()}</Kv>
                            <Kv label=K::Created>{short_time(&s.created)}</Kv>
                        </Card>
                    </div>
                    <h2 class="section-title">{move || i18n.t(K::Snapshots)}</h2>
                    <DataTable rows=d.snapshots columns=columns search=|x: &Snapshot| format!("{} {} {}", x.name, x.id, x.volume_name.as_deref().unwrap_or(""))/>
                }
            })}
        </Transition>
    }
}
