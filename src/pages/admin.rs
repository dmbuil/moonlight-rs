//! The Admin dashboard, Horizon's dashboard for the `admin` role.
//! Overview and the Compute, Volume, Network and System groups, with Horizon's admin
//! writes where built (flavors, aggregates, services, migrations, networks,
//! routers, floating IPs, RBAC, images, volumes, snapshots, volume and group types,
//! QoS, default quotas;
//!). Lists of
//! every project's resources reuse the Project panels' rows ([`Owned`] adds
//! *Project* and *Host*) and link to their detail pages. Its server functions
//! refuse a
//! token without the role (`api::require_admin`); the menu only hides it.

use leptos::prelude::*;

use super::details::{Card, DetailHeader, Kv, Tabs, mono, param};
use crate::api::*;
use std::collections::HashMap;
use std::sync::Arc;

use crate::components::dialog::{BulkButton, CreateButton, RowActions, caps, data_version};
use crate::components::ops;
use crate::components::ops::quota_label;
use crate::components::pager::{
    LateNames, Pager, PagerState, late_host, late_label, late_text, load_late, paged_table, use_list_query,
    with_server_keys,
};
use crate::components::table::{Col, DataTable, Select, TimingChip, available, col};
use crate::components::widgets::{
    AzPill, AzPills, ErrorBox, LockIcon, PageHeader, Pill, Skeleton, Tone, age_cell_since, dash, render, short_time,
};
use crate::components::widgets::{FlavorPopover, SpecsPopover};
use crate::i18n::decimal;
use crate::i18n::{K, Lang, t, use_i18n};
use crate::i18n::{image_visibility, power_state, server_status};
use crate::model::needs;
use crate::model::*;
use crate::pages::details::yes_no;
use crate::pages::instances::{BootImages, IpList, late_image_cell, load_boot_images, task_cell};
use crate::pages::lists::{admin_state, late_attachments, link, status_pill};
use crate::pages::overview::{Donut, UsageTotals, fmt_amount, period_form};
use leptos_router::components::A;
use leptos_router::hooks::use_query_map;

/// *Enabled* / *Disabled*, Horizon's *Status* column.
fn enabled(l: Lang, on: bool) -> &'static str {
    t(l, if on { K::EnabledCol } else { K::Disabled })
}

/// *Up* / *Down*, Horizon's *State* column.
fn up_pill(l: Lang, up: bool) -> impl IntoView {
    view! { <Pill tone=if up { Tone::Ok } else { Tone::Err } label=t(l, if up { K::Up } else { K::Down })/> }
}

/// An RFC 3339 timestamp as an age (`3 hours ago`), or `-` when the service never reported one.
fn age_or_dash(l: Lang, v: &Option<String>) -> AnyView {
    v.as_deref().map_or_else(|| "-".into_any(), |s| age_cell_since(l, s))
}

/// System Information › *Services*: Keystone's catalog, one row per service.
fn service_columns() -> Vec<Col<CatalogService>> {
    vec![
        col(K::Name, |s: &CatalogService, _| s.name.clone()).sort_text(|s| s.name.clone()),
        col(K::Service, |s: &CatalogService, _| s.service_type.clone()).sort_text(|s| s.service_type.clone()),
        col(K::Region, |s: &CatalogService, _| s.region.clone()).sort_text(|s| s.region.clone()),
        col(K::Endpoints, |s: &CatalogService, _| {
            let lines = s.endpoints.clone();
            view! {
                <ul class="plain">
                    {lines.into_iter().map(|(i, u)| view! { <li><span class="muted">{i}</span> " " <span class="mono">{u}</span></li> }).collect_view()}
                </ul>
            }
        }),
    ]
}

/// `actions`: Hypervisors › Compute Host's *Enable* / *Disable Service*
/// (System Information's tables have none, as Horizon).
fn host_service_columns(actions: bool) -> Vec<Col<HostService>> {
    let mut cols = vec![
        col(K::Name, |s: &HostService, _| s.binary.clone()).sort_text(|s| s.binary.clone()),
        col(K::Host, |s: &HostService, _| s.host.clone()).sort_text(|s| s.host.clone()),
        col(K::Zone, |s: &HostService, _| view! { <AzPill name=s.zone.clone()/> })
            .sort_text(|s| s.zone.clone().unwrap_or_default()),
        col(K::Status, |s: &HostService, l| {
            // Why it was disabled, as Horizon's tooltip.
            let reason = s.disabled_reason.clone().unwrap_or_default();
            view! { <span title=reason>{enabled(l, s.enabled)}</span> }
        }),
        col(K::ServiceState, |s: &HostService, l| up_pill(l, s.up)),
        col(K::LastUpdated, |s: &HostService, l| age_or_dash(l, &s.updated_at)),
    ];
    if actions {
        cols.push(
            col(
                K::Actions,
                |s: &HostService, l| view! { <RowActions items=ops::compute_service_items(l, s)/> },
            )
            .class("actions"),
        );
    }
    cols
}

/// System Information › *Network Agents*: Neutron's agents with their host, zone, state and
/// last heartbeat.
fn agent_columns() -> Vec<Col<NetworkAgent>> {
    vec![
        col(K::Type, |a: &NetworkAgent, _| a.agent_type.clone()).sort_text(|a| a.agent_type.clone()),
        col(K::Name, |a: &NetworkAgent, _| a.binary.clone()).sort_text(|a| a.binary.clone()),
        col(K::Host, |a: &NetworkAgent, _| a.host.clone()).sort_text(|a| a.host.clone()),
        col(K::Zone, |a: &NetworkAgent, _| view! { <AzPill name=a.zone.clone()/> })
            .sort_text(|a| a.zone.clone().unwrap_or_default()),
        col(K::Status, |a: &NetworkAgent, l| enabled(l, a.admin_up)),
        col(K::ServiceState, |a: &NetworkAgent, l| up_pill(l, a.alive)),
        col(K::LastUpdated, |a: &NetworkAgent, l| age_or_dash(l, &a.heartbeat)),
    ]
}

/// One tab's table, with its error above when that list failed.
fn fallible<T: Clone + Send + Sync + 'static>(
    f: Fallible<T>,
    columns: Vec<Col<T>>,
    search: fn(&T) -> String,
) -> AnyView {
    view! {
        {f.error.map(|e| view! { <ErrorBox message=e/> })}
        <DataTable rows=f.rows columns=columns search=search/>
    }
    .into_any()
}

/// The tabs this cloud has, in Horizon's order, then Moonlight's capabilities.
fn tabs(info: &SystemInfo) -> Vec<K> {
    let mut out = vec![K::CatalogServices];
    out.extend(info.compute.as_ref().map(|_| K::ComputeServices));
    out.extend(info.volume.as_ref().map(|_| K::BlockStorageServices));
    out.extend(info.agents.as_ref().map(|_| K::NetworkAgents));
    out.push(K::Modules);
    out
}

/// Admin › System › System Information, Horizon's four tabs plus *Detected
/// capabilities*.
#[component]
pub fn SystemInfoPage() -> impl IntoView {
    let data = Resource::new(data_version, |_| get_system_info());
    let tab = RwSignal::new(0usize);
    let services = StoredValue::new(service_columns());
    let hosts = StoredValue::new(host_service_columns(false));
    let agents = StoredValue::new(agent_columns());
    view! {
        <PageHeader>
            <Transition fallback=|| ()>
                {move || data.get().and_then(Result::ok).map(|t| view! { <TimingChip millis=t.millis calls=t.calls/> })}
            </Transition>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| {
                let labels = tabs(&t.data);
                let info = StoredValue::new(t.data);
                let shown = StoredValue::new(labels.clone());
                view! {
                    <Tabs labels=labels active=tab/>
                    {move || {
                        let i = info.get_value();
                        match shown.with_value(|l| l.get(tab.get()).copied()).unwrap_or(K::CatalogServices) {
                            K::ComputeServices => i.compute.map(|f| fallible(f, hosts.get_value(), host_search)).unwrap_or_else(|| ().into_any()),
                            K::BlockStorageServices => i.volume.map(|f| fallible(f, hosts.get_value(), host_search)).unwrap_or_else(|| ().into_any()),
                            K::NetworkAgents => i.agents.map(|f| fallible(f, agents.get_value(), agent_search)).unwrap_or_else(|| ().into_any()),
                            K::Modules => view! { <ModulesCard/> }.into_any(),
                            _ => view! { <DataTable rows=i.services columns=services.get_value() search=service_search/> }.into_any(),
                        }
                    }}
                }
            })}
        </Transition>
    }
}

// --- Overview ---------------------------------------------------------------------

/// Admin › Overview's per-project table: Horizon's usage columns (current totals, then the
/// period's vCPU, disk and RAM hours). A project Keystone no longer has keeps Horizon's
/// *(Deleted)* suffix.
fn usage_columns() -> Vec<Col<ProjectUsage>> {
    vec![
        // Horizon's "(Deleted)" after a project Keystone no longer has.
        col(K::ProjectName, |p: &ProjectUsage, l| {
            if p.deleted {
                format!("{} {}", p.project, t(l, K::DeletedSuffix))
            } else {
                p.project.clone()
            }
        })
        .sort_text(|p| p.project.clone()),
        col(K::Vcpus, |p: &ProjectUsage, _| p.vcpus().to_string())
            .sort_num(|p| p.vcpus())
            .class("num"),
        col(K::Disk, |p: &ProjectUsage, _| fmt_amount(p.local_gb(), QuotaUnit::Gib))
            .sort_num(|p| p.local_gb())
            .class("num"),
        col(K::Ram, |p: &ProjectUsage, _| fmt_amount(p.memory_mb(), QuotaUnit::Mib))
            .sort_num(|p| p.memory_mb())
            .class("num"),
        col(K::VcpuHoursCol, |p: &ProjectUsage, l| decimal(l, p.vcpu_hours(), 2))
            .sort_num(|p| p.vcpu_hours().round() as i64)
            .class("num"),
        col(K::DiskGbHours, |p: &ProjectUsage, l| decimal(l, p.gb_hours(), 2))
            .sort_num(|p| p.gb_hours().round() as i64)
            .class("num"),
        col(K::MemoryMbHours, |p: &ProjectUsage, l| decimal(l, p.ram_mb_hours(), 2))
            .sort_num(|p| p.ram_mb_hours().round() as i64)
            .class("num"),
    ]
}

/// Admin › Overview: Horizon's usage of every project over a period (`?start=&end=`,
/// this month by default), its totals and *Download CSV Summary*.
#[component]
pub fn AdminOverviewPage() -> impl IntoView {
    let query = use_query_map();
    let period = move || query.with(|q| (q.get("start").unwrap_or_default(), q.get("end").unwrap_or_default()));
    let data = Resource::new(
        move || (period(), data_version()),
        |((start, end), _)| get_admin_usage(start, end),
    );
    let columns = StoredValue::new(usage_columns());
    view! {
        <PageHeader/>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| {
                let u: GlobalUsage = t.data;
                let csv = u.csv();
                let period = period_form(u.start.clone(), u.end.clone());
                view! {
                    <UsageTotals usage=u.summary() csv=csv/>
                    <DataTable rows=u.projects columns=columns.get_value() search=|p: &ProjectUsage| format!("{} {}", p.project, p.project_id) timing=(t.millis, t.calls) on_refresh=move |_| data.refetch() extra=period/>
                }
            })}
        </Transition>
    }
}

// --- Every project's resources ------------------------------------------------

/// A Project panel's column for an Admin list's [`Owned`] rows.
fn lift<T: 'static>(c: Col<T>) -> Col<Owned<T>> {
    let cell = c.cell.clone();
    let sort = c.sort.clone();
    Col {
        header: c.header,
        cell: std::sync::Arc::new(move |o: &Owned<T>, l| cell(&o.row, l)),
        sort: sort.map(|f| {
            std::sync::Arc::new(move |o: &Owned<T>| f(&o.row))
                as std::sync::Arc<dyn Fn(&Owned<T>) -> crate::components::table::SortKey + Send + Sync>
        }),
        class: c.class,
        requires: c.requires,
        server: c.server,
    }
}

/// Horizon's *Project* column, first on every Admin list of resources.
fn project_col<T: 'static>() -> Col<Owned<T>> {
    col(K::Project, |o: &Owned<T>, _| o.project.clone()).sort_text(|o| o.project.clone())
}

/// Horizon's *Host* column (Nova's and Cinder's lists).
fn host_col<T: 'static>() -> Col<Owned<T>> {
    col(K::Host, |o: &Owned<T>, _| dash(o.host.clone())).sort_text(|o| o.host.clone().unwrap_or_default())
}

/// `project`, `host` and the row's own search text.
fn owned_search<T>(o: &Owned<T>, row: String) -> String {
    format!("{} {} {row}", o.project, o.host.as_deref().unwrap_or(""))
}

/// A paged Admin list page without actions: a server function taking
/// the URL's [`ListQuery`], its [`PagedList`], its columns (built once, outside
/// `view!`, AGENTS.md §6) and search text (used when the whole list came back).
macro_rules! admin_list {
    ($page:ident, $fetch:ident, $list:expr, $row:ty, $columns:expr, $search:expr) => {
        /// An Admin list page generated by `admin_list!`, read-only: one page of its rows, filtered
        /// and sorted on the server, or the whole list with `[ui] page_size = 0`.
        #[component]
        pub fn $page() -> impl IntoView {
            let query = use_list_query();
            let data = Resource::new(move || (query.get(), data_version()), |(q, _)| $fetch(q));
            let columns: StoredValue<Vec<Col<$row>>> = StoredValue::new($columns);
            let stack = PagerState::new();
            view! {
                <PageHeader/>
                <Transition fallback=|| view! { <Skeleton/> }>
                    {move || render(data.get(), move |t| paged_table(t, $list, ListScope::Admin, available(columns.get_value(), &caps()), $search, None, None, stack, Callback::new(move |_| data.refetch())))}
                </Transition>
            }
        }
    };
}

/// Admin › Instances' columns, Horizon's admin table with each row's menu.
fn admin_instance_columns(boot: BootImages) -> Vec<Col<Owned<Instance>>> {
    vec![
        project_col(),
        host_col(),
        // Horizon's padlock before the name, as Project › Instances.
        lift(col(K::Name, |i: &Instance, _| {
            let (href, name) = (format!("/project/instances/{}", i.id), i.name.clone());
            view! { <span class="name-cell"><LockIcon locked=i.locked/><A href=href attr:class="strong">{name}</A></span> }
        }).sort_text(|i| i.name.clone())),
        lift(col(K::ImageName, move |i: &Instance, l| late_image_cell(i, l, boot)).sort_text(|i| i.image_name.clone().unwrap_or_default())),
        lift(col(K::IpAddress, |i: &Instance, _| view! { <IpList addresses=i.addresses.clone()/> })),
        lift(col(K::Flavor, |i: &Instance, _| view! { <FlavorPopover flavor=i.flavor.clone() key=i.id.clone()/> }).sort_text(|i| i.flavor_name.clone())),
        lift(col(K::Status, |i: &Instance, l| view! { <Pill tone=Tone::server(&i.status) label=server_status(l, &i.status)/> }).sort_text(|i| i.status.clone())),
        lift(col(K::Task, |i: &Instance, l| task_cell(i, l))),
        lift(col(K::PowerState, |i: &Instance, l| power_state(l, i.power_state)).sort_num(|i| i.power_state)),
        lift(col(K::Age, |i: &Instance, l| age_cell_since(l, &i.created)).sort_text(|i| i.created.clone())),
        col(K::Actions, |o: &Owned<Instance>, l| view! { <RowActions items=ops::admin_instance_items(l, &o.row, o.host.as_deref())/> }).class("actions"),
    ]
}

/// Search text for Admin › Instances when the whole list came back: project, host, name, ID,
/// status and image.
fn admin_instance_search(o: &Owned<Instance>) -> String {
    owned_search(
        o,
        format!(
            "{} {} {} {}",
            o.row.name,
            o.row.id,
            o.row.status,
            o.row.image_name.as_deref().unwrap_or("")
        ),
    )
}

/// Admin › Compute › Instances: one page of `[ui] page_size` servers,
/// filtered on the server through the page's query string (`?marker=&f=&q=`), so its
/// cost doesn't grow with the cloud. With `page_size = 0` the whole list and its
/// search box, as before.
#[component]
pub fn AdminInstancesPage() -> impl IntoView {
    let query = use_list_query();
    let data = Resource::new(move || (query.get(), data_version()), |(q, _)| list_admin_instances(q));
    let boot: BootImages = RwSignal::new(HashMap::new());
    let columns = StoredValue::new(admin_instance_columns(boot));
    let stack = PagerState::new();
    Effect::new(move |_| {
        let Some(Ok(t)) = data.get() else { return };
        load_boot_images(
            t.data
                .rows
                .iter()
                .map(|o| (o.row.id.clone(), o.row.boot_volume_ids.clone()))
                .collect(),
            boot,
        );
    });
    view! {
        <PageHeader/>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| {
                let page = t.data;
                let cols = available(columns.get_value(), &caps());
                if !page.paged {
                    return view! {
                        <DataTable rows=page.rows columns=cols search=admin_instance_search timing=(t.millis, t.calls) on_refresh=move |_| data.refetch()/>
                    }.into_any();
                }
                let cols = with_server_keys(cols);
                let footer = view! { <Pager list=PagedList::Instances next=page.next.clone() shown=page.rows.len() count=page.count page_size=page.page_size stack=stack/> }.into_any();
                view! {
                    <DataTable rows=page.rows columns=cols search=admin_instance_search timing=(t.millis, t.calls) on_refresh=move |_| data.refetch()
                        paged=true extra=crate::components::pager::filter_bar(PagedList::Instances, ListScope::Admin, page.needs_filter) footer=footer server_sort=crate::components::pager::server_sort(PagedList::Instances)/>
                }.into_any()
            })}
        </Transition>
    }
}

/// Admin › Images' columns: the Project › Images columns behind *Project*, and each row's
/// admin menu.
fn image_columns() -> Vec<Col<Owned<Image>>> {
    vec![
        project_col(),
        lift(
            col(K::Name, |i: &Image, _| {
                link(format!("/project/images/{}", i.id), i.name.clone())
            })
            .sort_text(|i| i.name.clone()),
        ),
        lift(col(K::Type, |i: &Image, l| {
            t(
                l,
                if i.is_snapshot {
                    K::ImageTypeSnapshot
                } else {
                    K::ImageTypeImage
                },
            )
        })),
        lift(col(K::Status, |i: &Image, l| status_pill(&i.status, l)).sort_text(|i| i.status.clone())),
        lift(col(K::Visibility, |i: &Image, l| image_visibility(l, &i.visibility)).sort_text(|i| i.visibility.clone())),
        lift(col(K::Protected, |i: &Image, l| yes_no(l, i.protected))),
        lift(col(K::DiskFormat, |i: &Image, _| {
            dash(i.disk_format.clone()).to_uppercase()
        })),
        lift(
            col(K::Size, |i: &Image, _| {
                i.size.map(crate::i18n::bytes).unwrap_or_else(|| "-".into())
            })
            .sort_num(|i| i.size.unwrap_or(0) as i64)
            .class("num"),
        ),
        col(
            K::Actions,
            |o: &Owned<Image>, l| view! { <RowActions items=ops::admin_image_items(l, &o.row)/> },
        )
        .class("actions"),
    ]
}

/// Search text for Admin › Images: project, host, name, ID, status and visibility.
fn image_search(o: &Owned<Image>) -> String {
    owned_search(
        o,
        format!("{} {} {} {}", o.row.name, o.row.id, o.row.status, o.row.visibility),
    )
}

/// Bulk selection key of an image: `None` for a protected one, which Glance refuses to delete
/// (Horizon greys its checkbox out too).
fn image_key(o: &Owned<Image>) -> Option<(String, String)> {
    // Glance refuses deleting a protected image: not selectable, as Horizon.
    (!o.row.protected).then(|| (o.row.id.clone(), o.row.name.clone()))
}

/// Admin › Compute › Images: every project's images, the bulk *Delete Images*
/// and each row's *Edit Image*, *Update Metadata* and *Delete Image*.
#[component]
pub fn AdminImagesPage() -> impl IntoView {
    let i18n = use_i18n();
    let query = use_list_query();
    let data = Resource::new(move || (query.get(), data_version()), |(q, _)| list_admin_images(q));
    let columns = StoredValue::new(image_columns());
    let select = Select {
        key: image_key,
        picked: RwSignal::new(Vec::new()),
    };
    let stack = PagerState::new();
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteImages picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteImages, items, |id| OsAction::ImageDelete { id }))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateImage).to_string())
                open=Arc::new(move || ops::admin_create_image_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| paged_table(t, PagedList::Images, ListScope::Admin, columns.get_value(), image_search, Some(select), None, stack, Callback::new(move |_| data.refetch())))}
        </Transition>
    }
}

/// Admin › Volumes' columns, Horizon's admin table, with each row's menu.
fn volume_columns(late: LateNames) -> Vec<Col<Owned<Volume>>> {
    vec![
        project_col(),
        host_col(),
        lift(
            col(K::Name, |v: &Volume, _| {
                link(format!("/project/volumes/{}", v.id), v.name.clone())
            })
            .sort_text(|v| v.name.clone()),
        ),
        lift(
            col(K::Size, |v: &Volume, _| format!("{} GiB", v.size_gb))
                .sort_num(|v| v.size_gb)
                .class("num"),
        ),
        lift(col(K::Status, |v: &Volume, l| status_pill(&v.status, l)).sort_text(|v| v.status.clone())),
        lift(
            col(K::Type, |v: &Volume, _| dash(v.volume_type.clone()))
                .sort_text(|v| v.volume_type.clone().unwrap_or_default()),
        ),
        lift(col(K::AttachedTo, move |v: &Volume, _| late_attachments(v, late))),
        lift(col(K::Bootable, |v: &Volume, l| yes_no(l, v.bootable))),
        lift(col(K::Encrypted, |v: &Volume, l| yes_no(l, v.encrypted))),
        col(
            K::Actions,
            |o: &Owned<Volume>, l| view! { <RowActions items=ops::admin_volume_items(l, &o.row, o.host.as_deref())/> },
        )
        .class("actions"),
    ]
}

/// Search text for Admin › Volumes: project, host and the volume's own fields.
fn volume_search(o: &Owned<Volume>) -> String {
    owned_search(
        o,
        format!(
            "{} {} {} {}",
            o.row.name,
            o.row.id,
            o.row.status,
            o.row.volume_type.as_deref().unwrap_or("")
        ),
    )
}

/// Bulk selection key of an [`Owned`] row: its ID and name.
fn volume_key(o: &Owned<Volume>) -> Option<(String, String)> {
    Some((o.row.id.clone(), o.row.name.clone()))
}

/// Admin › Volume › Volumes: every project's volumes with Horizon's *Manage
/// Volume*, the bulk *Delete Volumes* and each row's admin menu. One page at a
/// time; attached servers are named after the table shows.
#[component]
pub fn AdminVolumesPage() -> impl IntoView {
    let i18n = use_i18n();
    let query = use_list_query();
    let data = Resource::new(move || (query.get(), data_version()), |(q, _)| list_admin_volumes(q));
    let late = LateNames::new();
    let columns = StoredValue::new(volume_columns(late));
    let select = Select {
        key: volume_key,
        picked: RwSignal::new(Vec::new()),
    };
    let stack = PagerState::new();
    Effect::new(move |_| {
        let Some(Ok(t)) = data.get() else { return };
        late.active.set(t.data.paged);
        let ids = t
            .data
            .rows
            .iter()
            .flat_map(|o| {
                o.row
                    .attachments
                    .iter()
                    .filter(|a| a.server_name.is_none())
                    .map(|a| a.server_id.clone())
            })
            .collect();
        if t.data.paged {
            load_late(NameKind::Servers, ids, late);
        }
    });
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteVolumes picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteVolumes, items, |id| OsAction::VolumeDelete { id }))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::ManageVolume).to_string())
                open=Arc::new(move || ops::manage_volume_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| paged_table(t, PagedList::Volumes, ListScope::Admin, columns.get_value(), volume_search, Some(select), None, stack, Callback::new(move |_| data.refetch())))}
        </Transition>
    }
}

/// Admin › Snapshots' columns, with each row's menu. The host is the volume's,
/// and a paged page names the volume (and so the host) later ([`LateNames`]).
fn snapshot_columns(late: LateNames) -> Vec<Col<Owned<Snapshot>>> {
    vec![
        project_col(),
        col(K::Host, move |o: &Owned<Snapshot>, _| match &o.host {
            Some(h) => h.clone().into_any(),
            None => late_text(late, o.row.volume_id.clone(), late_host, "-".into()),
        })
        .sort_text(|o| o.host.clone().unwrap_or_default()),
        lift(
            col(K::Name, |s: &Snapshot, _| {
                link(format!("/project/snapshots/{}", s.id), s.name.clone())
            })
            .sort_text(|s| s.name.clone()),
        ),
        lift(col(K::Description, |s: &Snapshot, _| dash(s.description.clone()))),
        lift(
            col(K::Size, |s: &Snapshot, _| format!("{} GiB", s.size_gb))
                .sort_num(|s| s.size_gb)
                .class("num"),
        ),
        lift(col(K::Status, |s: &Snapshot, l| status_pill(&s.status, l)).sort_text(|s| s.status.clone())),
        lift(col(K::VolumeName, move |s: &Snapshot, _| match &s.volume_name {
            Some(n) => link(format!("/project/volumes/{}", s.volume_id), n.clone()),
            None => {
                let (href, id) = (format!("/project/volumes/{}", s.volume_id), s.volume_id.clone());
                view! { <A href=href>{late_text(late, id.clone(), late_label, id)}</A> }.into_any()
            }
        })),
        col(
            K::Actions,
            |o: &Owned<Snapshot>, l| view! { <RowActions items=ops::admin_snapshot_items(l, &o.row)/> },
        )
        .class("actions"),
    ]
}

/// Search text for Admin › Volume Snapshots: project, host and the snapshot's own fields.
fn snapshot_search(o: &Owned<Snapshot>) -> String {
    owned_search(
        o,
        format!(
            "{} {} {} {}",
            o.row.name,
            o.row.id,
            o.row.status,
            o.row.volume_name.as_deref().unwrap_or("")
        ),
    )
}

/// Bulk selection key of a snapshot: its ID and name; every snapshot can be picked.
fn snapshot_key(o: &Owned<Snapshot>) -> Option<(String, String)> {
    Some((o.row.id.clone(), o.row.name.clone()))
}

/// Admin › Volume › Snapshots: every project's volume snapshots, the bulk
/// *Delete Volume Snapshots* and each row's admin menu. One page at a time
///; each snapshot's volume and host are named after the table shows.
#[component]
pub fn AdminSnapshotsPage() -> impl IntoView {
    let query = use_list_query();
    let data = Resource::new(move || (query.get(), data_version()), |(q, _)| list_admin_snapshots(q));
    let late = LateNames::new();
    let columns = StoredValue::new(snapshot_columns(late));
    let select = Select {
        key: snapshot_key,
        picked: RwSignal::new(Vec::new()),
    };
    let stack = PagerState::new();
    Effect::new(move |_| {
        let Some(Ok(t)) = data.get() else { return };
        late.active.set(t.data.paged);
        if t.data.paged {
            load_late(
                NameKind::Volumes,
                t.data.rows.iter().map(|o| o.row.volume_id.clone()).collect(),
                late,
            );
        }
    });
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteSnapshots picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteSnapshots, items, |id| OsAction::SnapshotDelete { id }))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| paged_table(t, PagedList::Snapshots, ListScope::Admin, columns.get_value(), snapshot_search, Some(select), None, stack, Callback::new(move |_| data.refetch())))}
        </Transition>
    }
}

admin_list!(
    AdminVolumeGroupsPage,
    list_admin_volume_groups,
    PagedList::VolumeGroups,
    Owned<VolumeGroupRow>,
    vec![
        project_col(),
        lift(
            col(K::Name, |g: &VolumeGroupRow, _| link(
                format!("/project/volume_groups/{}", g.id),
                g.name.clone()
            ))
            .sort_text(|g| g.name.clone())
        ),
        lift(col(K::Description, |g: &VolumeGroupRow, _| dash(g.description.clone()))),
        lift(col(K::Status, |g: &VolumeGroupRow, l| status_pill(&g.status, l)).sort_text(|g| g.status.clone())),
        lift(col(
            K::AvailabilityZone,
            |g: &VolumeGroupRow, _| view! { <AzPill name=g.availability_zone.clone()/> }
        )),
        lift(col(K::VolumeTypes, |g: &VolumeGroupRow, _| g
            .volume_types
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", "))),
        lift(col(K::HasSnapshots, |g: &VolumeGroupRow, l| yes_no(l, g.has_snapshots))),
    ],
    |o: &Owned<VolumeGroupRow>| owned_search(o, format!("{} {} {}", o.row.name, o.row.id, o.row.status))
);

admin_list!(
    AdminGroupSnapshotsPage,
    list_admin_group_snapshots,
    PagedList::GroupSnapshots,
    Owned<GroupSnapshotRow>,
    vec![
        project_col(),
        lift(
            col(K::Name, |s: &GroupSnapshotRow, _| link(
                format!("/project/vg_snapshots/{}", s.id),
                s.name.clone()
            ))
            .sort_text(|s| s.name.clone())
        ),
        lift(col(K::Description, |s: &GroupSnapshotRow, _| dash(
            s.description.clone()
        ))),
        lift(col(K::Status, |s: &GroupSnapshotRow, l| status_pill(&s.status, l)).sort_text(|s| s.status.clone())),
        lift(col(K::VolumeGroup, |s: &GroupSnapshotRow, _| link(
            format!("/project/volume_groups/{}", s.group.id),
            s.group.name.clone()
        ))),
    ],
    |o: &Owned<GroupSnapshotRow>| owned_search(
        o,
        format!("{} {} {} {}", o.row.name, o.row.id, o.row.status, o.row.group.name)
    )
);

/// Admin › Volume › Group Types' columns: name, description, *Public*, specs and each row's menu.
fn group_type_columns() -> Vec<Col<GroupTypeRow>> {
    vec![
        col(K::Name, |g: &GroupTypeRow, _| g.name.clone()).sort_text(|g| g.name.clone()),
        col(K::Description, |g: &GroupTypeRow, _| dash(g.description.clone())),
        col(K::Public, |g: &GroupTypeRow, l| yes_no(l, g.public)),
        col(K::Specs, |g: &GroupTypeRow, l| specs_cell(l, &g.specs)),
        col(
            K::Actions,
            |g: &GroupTypeRow, l| view! { <RowActions items=ops::group_type_items(l, g)/> },
        )
        .class("actions"),
    ]
}

/// Bulk selection key of a group type: its ID and name.
fn group_type_key(g: &GroupTypeRow) -> Option<(String, String)> {
    Some((g.id.clone(), g.name.clone()))
}

/// Admin › Volume › Group Types: *Create Group Type*, the bulk delete and each
/// row's *View Specs*, *Edit* and *Delete*.
#[component]
pub fn AdminGroupTypesPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_admin_group_types());
    let columns = StoredValue::new(group_type_columns());
    let select = Select {
        key: group_type_key,
        picked: RwSignal::new(Vec::new()),
    };
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteGroupTypes picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteGroupTypes, items, |id| OsAction::GroupTypeDelete { id }))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateGroupType).to_string())
                open=Arc::new(move || ops::create_group_type_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| view! {
                <DataTable rows=t.data columns=columns.get_value() search=|g: &GroupTypeRow| format!("{} {}", g.name, g.id)
                    timing=(t.millis, t.calls) on_refresh=move |_| data.refetch() select=select/>
            })}
        </Transition>
    }
}

/// Admin › Compute › Flavors' columns, Horizon's admin table: sizes, *Public*, *Metadata* and
/// each flavor's menu.
fn flavor_columns() -> Vec<Col<FlavorRow>> {
    let mut cols = vec![
        col(K::FlavorName, |f: &FlavorRow, _| f.name.clone()).sort_text(|f| f.name.clone()),
        col(K::Vcpus, |f: &FlavorRow, _| f.vcpus.to_string()).sort_num(|f| f.vcpus).class("num"),
        col(K::Ram, |f: &FlavorRow, _| fmt_amount(f.ram_mb, QuotaUnit::Mib)).sort_num(|f| f.ram_mb).class("num"),
        col(K::RootDisk, |f: &FlavorRow, _| format!("{} GB", f.disk_gb)).sort_num(|f| f.disk_gb).class("num"),
        col(K::EphemeralDisk, |f: &FlavorRow, _| format!("{} GB", f.ephemeral_gb)).sort_num(|f| f.ephemeral_gb).class("num"),
        col(K::SwapDisk, |f: &FlavorRow, _| format!("{} MB", f.swap_mb)).sort_num(|f| f.swap_mb).class("num"),
        col(K::RxtxFactor, |f: &FlavorRow, _| f.rxtx_factor.map_or_else(|| "-".to_string(), |x| x.to_string())).class("num"),
        col(K::Id, |f: &FlavorRow, _| view! { <span class="mono small">{f.id.clone()}</span> }),
        col(K::Public, |f: &FlavorRow, l| {
            // Private: the projects allowed to use it, under *No* (Horizon only
            // shows them in *Modify Access*).
            let access = f.access.clone();
            view! {
                {yes_no(l, f.public)}
                {(!access.is_empty()).then(|| view! {
                    <ul class="plain small muted" title=t(l, K::FlavorAccess)>{access.into_iter().map(|(_, p)| view! { <li>{p}</li> }).collect_view()}</ul>
                })}
            }
        }).sort_num(|f| f.public as i64),
        // Horizon's *Metadata* column: whether there are any, with the specs in
        // a card on the *Yes*.
        col(K::Metadata, |f: &FlavorRow, l| {
            view! {
                <SpecsPopover label=yes_no(l, has_metadata(f)) name=f.name.clone()
                    specs=f.extra_specs.clone() title=K::Metadata key=f.id.clone()/>
            }
        })
        .sort_text(|f| has_metadata(f).to_string()),
    ];
    cols.push(
        col(
            K::Actions,
            |f: &FlavorRow, l| view! { <RowActions items=ops::admin_flavor_items(l, f)/> },
        )
        .class("actions"),
    );
    cols
}

/// Whether a flavor has extra specs (Admin › Flavors' *Metadata* column).
fn has_metadata(f: &FlavorRow) -> bool {
    !f.extra_specs.is_empty()
}

/// Name, ID and every `key=value` extra spec, so the search still finds a
/// flavor by a spec the row no longer shows.
fn flavor_search(f: &FlavorRow) -> String {
    let specs = f
        .extra_specs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{} {} {specs}", f.name, f.id)
}

/// Every flavor can be picked for the bulk delete.
fn flavor_key(f: &FlavorRow) -> Option<(String, String)> {
    Some((f.id.clone(), f.name.clone()))
}

/// Admin › Compute › Flavors: public and private flavors with Horizon's
/// *Create Flavor*, the bulk *Delete Flavors* and each row's *Update
/// Metadata*, *Modify Access* and *Delete Flavor*.
#[component]
pub fn AdminFlavorsPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_admin_flavors());
    let columns = StoredValue::new(flavor_columns());
    let select = Select {
        key: flavor_key,
        picked: RwSignal::new(Vec::new()),
    };
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteFlavors picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteFlavors, items, |id| OsAction::FlavorDelete { id }))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateFlavor).to_string())
                open=Arc::new(move || ops::create_flavor_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| view! {
                <DataTable rows=t.data columns=columns.get_value() search=flavor_search
                    timing=(t.millis, t.calls) on_refresh=move |_| data.refetch() select=select/>
            })}
        </Transition>
    }
}

/// Key/value specs as `key=value` lines; *No* when empty (Horizon's *Metadata*).
fn specs_cell(l: Lang, specs: &[(String, String)]) -> AnyView {
    if specs.is_empty() {
        return t(l, K::No).into_any();
    }
    let lines = specs.to_vec();
    view! { <ul class="plain">{lines.into_iter().map(|(k, v)| view! { <li class="mono small">{format!("{k}={v}")}</li> }).collect_view()}</ul> }.into_any()
}

/// Admin › Network › Networks' columns: *Project*, then the Project › Networks columns, the
/// DHCP agent count (dropped by the caller when no row has one) and each row's admin menu.
fn admin_network_columns() -> Vec<Col<Owned<NetworkRow>>> {
    let mut cols = vec![
        project_col(),
        lift(
            col(K::Name, |n: &NetworkRow, _| {
                link(format!("/project/networks/{}", n.id), n.name.clone())
            })
            .sort_text(|n| n.name.clone()),
        ),
        lift(col(
            K::Subnets,
            |n: &NetworkRow, _| view! { <ul class="plain">{n.subnets.iter().map(|s| view! { <li class="mono">{s.clone()}</li> }).collect_view()}</ul> },
        )),
        lift(
            col(K::DhcpAgents, |n: &NetworkRow, _| {
                n.dhcp_agents.map_or_else(|| "-".to_string(), |c| c.to_string())
            })
            .sort_num(|n| n.dhcp_agents.unwrap_or(0) as i64)
            .class("num")
            .requires(needs::DHCP_AGENTS),
        ),
        lift(col(K::Shared, |n: &NetworkRow, l| yes_no(l, n.shared))),
        lift(col(K::External, |n: &NetworkRow, l| yes_no(l, n.external)).sort_num(|n| n.external as i64)),
        lift(col(K::Status, |n: &NetworkRow, l| status_pill(&n.status, l))),
        lift(col(K::AdminState, |n: &NetworkRow, l| admin_state(l, n.admin_up))),
        lift(col(
            K::AvailabilityZone,
            |n: &NetworkRow, _| view! { <AzPills names=n.availability_zones.clone()/> },
        )),
    ];
    cols.push(
        col(
            K::Actions,
            |o: &Owned<NetworkRow>, l| view! { <RowActions items=ops::admin_network_items(l, &o.row)/> },
        )
        .class("actions"),
    );
    cols
}

/// `cols` without *DHCP Agents* when no row has a count: the cloud has no DHCP
/// agent (OVN), lacks `dhcp_agent_scheduler`, or counting failed. Every row has
/// one otherwise, so a column of `-` or `0` says nothing.
fn drop_empty_dhcp_col(cols: Vec<Col<Owned<NetworkRow>>>, rows: &[Owned<NetworkRow>]) -> Vec<Col<Owned<NetworkRow>>> {
    if rows.iter().any(|o| o.row.dhcp_agents.is_some()) {
        return cols;
    }
    cols.into_iter().filter(|c| c.header != K::DhcpAgents).collect()
}

/// Search text: project, host and the row's own fields.
fn admin_network_search(o: &Owned<NetworkRow>) -> String {
    owned_search(o, format!("{} {} {}", o.row.name, o.row.id, o.row.subnets.join(" ")))
}

/// Every network can be picked for the bulk delete (Admin: any project's).
fn admin_network_key(o: &Owned<NetworkRow>) -> Option<(String, String)> {
    Some((o.row.id.clone(), o.row.name.clone()))
}

/// Admin › Network › Networks: every project's networks, with Horizon's admin
/// *Create Network* (provider attributes), the bulk *Delete Networks* and each
/// row's *Edit Network*, *Create Subnet* and *Delete Network*.
#[component]
pub fn AdminNetworksPage() -> impl IntoView {
    let i18n = use_i18n();
    let query = use_list_query();
    let data = Resource::new(move || (query.get(), data_version()), |(q, _)| list_admin_networks(q));
    let columns = StoredValue::new(admin_network_columns());
    let select = Select {
        key: admin_network_key,
        picked: RwSignal::new(Vec::new()),
    };
    let stack = PagerState::new();
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteNetworks picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteNetworks, items, |id| OsAction::NetworkDelete { id }))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateNetwork).to_string())
                open=Arc::new(move || ops::admin_create_network_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| {
                let cols = drop_empty_dhcp_col(available(columns.get_value(), &caps()), &t.data.rows);
                paged_table(t, PagedList::Networks, ListScope::Admin, cols, admin_network_search, Some(select), None, stack, Callback::new(move |_| data.refetch()))
            })}
        </Transition>
    }
}

/// Admin › Network › Routers' columns: *Project*, then the Project › Routers columns and each
/// row's admin menu.
fn admin_router_columns() -> Vec<Col<Owned<RouterRow>>> {
    let mut cols = vec![
        project_col(),
        lift(
            col(K::Name, |r: &RouterRow, _| {
                link(format!("/project/routers/{}", r.id), r.name.clone())
            })
            .sort_text(|r| r.name.clone()),
        ),
        lift(col(K::Status, |r: &RouterRow, l| status_pill(&r.status, l))),
        lift(col(K::ExternalNetwork, |r: &RouterRow, _| {
            dash(r.external_network.clone())
        })),
        lift(col(
            K::AvailabilityZone,
            |r: &RouterRow, _| view! { <AzPills names=r.availability_zones.clone()/> },
        )),
        lift(col(K::AdminState, |r: &RouterRow, l| admin_state(l, r.admin_up))),
    ];
    cols.push(
        col(
            K::Actions,
            |o: &Owned<RouterRow>, l| view! { <RowActions items=ops::admin_router_items(l, &o.row)/> },
        )
        .class("actions"),
    );
    cols
}

/// Search text: project, host and the row's own fields.
fn admin_router_search(o: &Owned<RouterRow>) -> String {
    owned_search(
        o,
        format!(
            "{} {} {}",
            o.row.name,
            o.row.id,
            o.row.external_network.as_deref().unwrap_or("")
        ),
    )
}

/// Every router can be picked for the bulk delete (Admin: any project's).
fn admin_router_key(o: &Owned<RouterRow>) -> Option<(String, String)> {
    Some((o.row.id.clone(), o.row.name.clone()))
}

/// Admin › Network › Routers: every project's routers, with Horizon's admin
/// *Create Router* (the owner project), the bulk *Delete Routers* and each
/// row's *Edit Router* and *Delete Router*.
#[component]
pub fn AdminRoutersPage() -> impl IntoView {
    let i18n = use_i18n();
    let query = use_list_query();
    let data = Resource::new(move || (query.get(), data_version()), |(q, _)| list_admin_routers(q));
    let columns = StoredValue::new(admin_router_columns());
    let select = Select {
        key: admin_router_key,
        picked: RwSignal::new(Vec::new()),
    };
    let stack = PagerState::new();
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteRouters picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteRouters, items, |id| OsAction::RouterDelete { id }))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateRouter).to_string())
                open=Arc::new(move || ops::admin_create_router_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| paged_table(t, PagedList::Routers, ListScope::Admin, available(columns.get_value(), &caps()), admin_router_search, Some(select), None, stack, Callback::new(move |_| data.refetch())))}
        </Transition>
    }
}

/// Admin › Network › Floating IPs' columns. `late` holds the names loaded after the table
/// shows (the instance or port each address is mapped to), so the page renders before those
/// lookups finish.
fn admin_floating_ip_columns(late: LateNames) -> Vec<Col<Owned<FloatingIpRow>>> {
    let mut cols = vec![
        project_col(),
        lift(
            col(K::IpAddress, |f: &FloatingIpRow, _| {
                let (href, ip) = (format!("/project/floating_ips/{}", f.id), f.ip.clone());
                view! { <A href=href attr:class="mono strong">{ip}</A> }
            })
            .sort_text(|f| f.ip.clone()),
        ),
        lift(col(K::Description, |f: &FloatingIpRow, _| dash(f.description.clone()))),
        // A paged page names the instance later ([`LateNames`]).
        lift(col(K::MappedFixedIp, move |f: &FloatingIpRow, _| {
            let fixed = dash(f.fixed_ip.clone());
            match (&f.instance_id, &f.instance_name) {
                (Some(id), name) => {
                    let href = format!("/project/instances/{id}");
                    let label = match name {
                        Some(n) => n.clone().into_any(),
                        None => late_text(late, id.clone(), late_label, id.clone()),
                    };
                    view! { <A href=href>{label}</A> " " <span class="mono muted">{fixed}</span> }.into_any()
                }
                _ => view! { <span class="mono">{fixed}</span> }.into_any(),
            }
        })),
        lift(col(K::Pool, |f: &FloatingIpRow, _| f.pool.clone()).sort_text(|f| f.pool.clone())),
        lift(col(K::Status, |f: &FloatingIpRow, l| status_pill(&f.status, l)).sort_text(|f| f.status.clone())),
        lift(col(K::DnsName, |f: &FloatingIpRow, _| dash(f.dns_name.clone())).requires(needs::DNS)),
        lift(col(K::DnsDomain, |f: &FloatingIpRow, _| dash(f.dns_domain.clone())).requires(needs::DNS)),
    ];
    cols.push(col(K::Actions, |o: &Owned<FloatingIpRow>, l| view! { <RowActions items=ops::admin_floating_ip_items(l, &o.row, o.project_id.as_deref())/> }).class("actions"));
    cols
}

/// Search text: project, host and the row's own fields.
fn admin_floating_ip_search(o: &Owned<FloatingIpRow>) -> String {
    owned_search(
        o,
        format!(
            "{} {} {} {}",
            o.row.ip,
            o.row.pool,
            o.row.fixed_ip.as_deref().unwrap_or(""),
            o.row.instance_name.as_deref().unwrap_or("")
        ),
    )
}

/// Every floating IP can be picked for the bulk release, named by its address.
fn admin_floating_ip_key(o: &Owned<FloatingIpRow>) -> Option<(String, String)> {
    Some((o.row.id.clone(), o.row.ip.clone()))
}

/// Admin › Network › Floating IPs: every project's, with Horizon's *Allocate IP
/// To Project*, row checkboxes with the bulk *Release Floating IPs*, and each
/// row's *Disassociate* and *Release Floating IP*. One page at a time;
/// instances are named after the table shows.
#[component]
pub fn AdminFloatingIpsPage() -> impl IntoView {
    let i18n = use_i18n();
    let query = use_list_query();
    let data = Resource::new(
        move || (query.get(), data_version()),
        |(q, _)| list_admin_floating_ips(q),
    );
    let late = LateNames::new();
    let columns = StoredValue::new(admin_floating_ip_columns(late));
    let select = Select {
        key: admin_floating_ip_key,
        picked: RwSignal::new(Vec::new()),
    };
    let stack = PagerState::new();
    Effect::new(move |_| {
        let Some(Ok(t)) = data.get() else { return };
        late.active.set(t.data.paged);
        if t.data.paged {
            let ids = t
                .data
                .rows
                .iter()
                .filter(|o| o.row.instance_name.is_none())
                .filter_map(|o| o.row.instance_id.clone())
                .collect();
            load_late(NameKind::Servers, ids, late);
        }
    });
    view! {
        <PageHeader>
            <BulkButton label=K::ReleaseIps picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::ReleaseIps, items, |id| OsAction::FloatingIpRelease { id }))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::AllocateIp).to_string())
                open=Arc::new(move || ops::admin_allocate_ip_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            // `available`: columns that need an extension (DNS).
            {move || render(data.get(), move |t| paged_table(t, PagedList::FloatingIps, ListScope::Admin, available(columns.get_value(), &caps()), admin_floating_ip_search, Some(select), None, stack, Callback::new(move |_| data.refetch())))}
        </Transition>
    }
}

admin_list!(
    AdminTrunksPage,
    list_admin_trunks,
    PagedList::Trunks,
    Owned<TrunkRow>,
    vec![
        project_col(),
        lift(
            col(K::Name, |t: &TrunkRow, _| link(
                format!("/project/trunks/{}", t.id),
                t.name.clone()
            ))
            .sort_text(|t| t.name.clone())
        ),
        lift(col(K::ParentPort, |t: &TrunkRow, _| link(
            format!("/project/networks/ports/{}", t.parent.id),
            t.parent.name.clone()
        ))),
        lift(
            col(K::Subports, |t: &TrunkRow, _| t.subports.len().to_string())
                .sort_num(|t| t.subports.len() as i64)
                .class("num")
        ),
        lift(col(K::Status, |t: &TrunkRow, l| status_pill(&t.status, l)).sort_text(|t| t.status.clone())),
        lift(col(K::AdminState, |t: &TrunkRow, l| admin_state(l, t.admin_up))),
    ],
    |o: &Owned<TrunkRow>| owned_search(o, format!("{} {} {}", o.row.name, o.row.id, o.row.parent.name))
);

/// Admin › Network › RBAC Policies' columns: owner project, ID, object type and object,
/// target project (*All projects* for `*`), action and each policy's menu.
fn rbac_columns() -> Vec<Col<RbacPolicyRow>> {
    let mut cols = vec![
        col(K::Project, |r: &RbacPolicyRow, _| r.project.clone()).sort_text(|r| r.project.clone()),
        col(
            K::Id,
            |r: &RbacPolicyRow, _| view! { <span class="mono small">{r.id.clone()}</span> },
        ),
        col(K::ObjectType, |r: &RbacPolicyRow, _| r.object_type.clone()).sort_text(|r| r.object_type.clone()),
        col(K::RbacObject, |r: &RbacPolicyRow, _| {
            // Types with a detail page link to it; the others show the name (or ID).
            let page = match r.object_type.as_str() {
                "network" => Some("networks"),
                "qos_policy" => Some("qos"),
                "security_group" => Some("security_groups"),
                _ => None,
            };
            match page {
                Some(p) => link(format!("/project/{p}/{}", r.object.id), r.object.name.clone()),
                None => view! { <span class="mono small">{r.object.name.clone()}</span> }.into_any(),
            }
        }),
        col(K::TargetProject, |r: &RbacPolicyRow, l| {
            if r.target == "*" {
                t(l, K::AllProjects).to_string()
            } else {
                r.target.clone()
            }
        })
        .sort_text(|r| r.target.clone()),
        col(
            K::Action,
            |r: &RbacPolicyRow, _| view! { <span class="mono small">{r.action.clone()}</span> },
        ),
    ];
    cols.push(
        col(
            K::Actions,
            |r: &RbacPolicyRow, l| view! { <RowActions items=ops::rbac_policy_items(l, r)/> },
        )
        .class("actions"),
    );
    cols
}

/// Every policy can be picked for the bulk delete; named `type object → target`.
fn rbac_key(r: &RbacPolicyRow) -> Option<(String, String)> {
    Some((
        r.id.clone(),
        format!("{} {} → {}", r.object_type, r.object.name, r.target),
    ))
}

/// Admin › Network › RBAC Policies: Horizon's table with *Create RBAC Policy*,
/// row checkboxes with the bulk *Delete RBAC Policies*, and each row's delete.
#[component]
pub fn AdminRbacPoliciesPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_admin_rbac_policies());
    let columns = StoredValue::new(rbac_columns());
    let select = Select {
        key: rbac_key,
        picked: RwSignal::new(Vec::new()),
    };
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteRbacPolicies picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteRbacPolicies, items, |id| OsAction::RbacPolicyDelete { id }))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateRbacPolicy).to_string())
                open=Arc::new(move || ops::create_rbac_policy_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| view! {
                <DataTable rows=t.data columns=available(columns.get_value(), &caps()) search=|r: &RbacPolicyRow| format!("{} {} {} {} {}", r.project, r.id, r.object_type, r.object.name, r.target)
                    timing=(t.millis, t.calls) on_refresh=move |_| data.refetch() select=select/>
            })}
        </Transition>
    }
}

// --- Cloud-wide objects ----------------------------------------------------------

/// Admin › Compute › Hypervisors' columns, Horizon's: used and total vCPUs, RAM and disk,
/// formatted in MiB / GiB as quotas are.
fn hypervisor_columns() -> Vec<Col<HypervisorRow>> {
    let mib = |n: i64| fmt_amount(n, QuotaUnit::Mib);
    let gib = |n: i64| fmt_amount(n, QuotaUnit::Gib);
    vec![
        col(K::Hostname, |h: &HypervisorRow, _| h.hostname.clone()).sort_text(|h| h.hostname.clone()),
        col(K::Type, |h: &HypervisorRow, _| h.kind.clone()),
        col(K::VcpusUsed, |h: &HypervisorRow, _| h.vcpus_used.to_string())
            .sort_num(|h| h.vcpus_used)
            .class("num"),
        col(K::VcpusTotal, |h: &HypervisorRow, _| h.vcpus.to_string())
            .sort_num(|h| h.vcpus)
            .class("num"),
        col(K::RamUsed, move |h: &HypervisorRow, _| mib(h.ram_used_mb))
            .sort_num(|h| h.ram_used_mb)
            .class("num"),
        col(K::RamTotal, move |h: &HypervisorRow, _| mib(h.ram_mb))
            .sort_num(|h| h.ram_mb)
            .class("num"),
        col(K::DiskUsed, move |h: &HypervisorRow, _| gib(h.disk_used_gb))
            .sort_num(|h| h.disk_used_gb)
            .class("num"),
        col(K::DiskTotal, move |h: &HypervisorRow, _| gib(h.disk_gb))
            .sort_num(|h| h.disk_gb)
            .class("num"),
        col(K::Instances, |h: &HypervisorRow, _| h.instances.to_string())
            .sort_num(|h| h.instances)
            .class("num"),
    ]
}

/// A provider's resource class: `used / total`, with the reserved amount and
/// the allocation ratio under it; `-` when it has none.
fn provider_cell(l: Lang, r: &Option<ProviderResource>, unit: QuotaUnit) -> AnyView {
    match r {
        None => view! { <span>"-"</span> }.into_any(),
        Some(r) => {
            let main = format!("{} / {}", fmt_amount(r.used, unit), fmt_amount(r.total, unit));
            let sub = format!(
                "{} {} · ×{}",
                t(l, K::ReservedRatio),
                fmt_amount(r.reserved, unit),
                decimal(l, r.allocation_ratio, 1)
            );
            view! { <span>{main}</span><br/><span class="small muted">{sub}</span> }.into_any()
        }
    }
}

/// The tab's columns: Horizon's VCPU, RAM and disk, then, only when some
/// provider has them, vGPUs, their type (custom traits) and the other classes.
/// The name is not sortable: the rows are a tree (hosts, their GPUs under them),
/// and a host with devices has a toggle that shows or hides them (`expanded`).
fn provider_columns(
    rows: &[ResourceProviderRow],
    expanded: RwSignal<std::collections::HashSet<String>>,
) -> Vec<Col<ResourceProviderRow>> {
    let mut cols = vec![
        col(K::Name, move |p: &ResourceProviderRow, l| {
            // GPUs (and other nested providers) indented under their host.
            let style = format!("padding-left: {}em", f32::from(p.depth) * 1.5);
            let name = p.name.clone();
            if p.depth > 0 {
                return view! { <span style=style><span class="muted">"└ "</span>{name}</span> }.into_any();
            }
            if p.descendants == 0 {
                return view! { <span>{name}</span> }.into_any();
            }
            let (host, count) = (p.uuid.clone(), p.descendants);
            let open = {
                let host = host.clone();
                move || expanded.try_with(|e| e.contains(&host)).unwrap_or(false)
            };
            let open2 = open.clone();
            let toggle = move |_| {
                expanded.try_update(|e| {
                    if !e.remove(&host) {
                        e.insert(host.clone());
                    }
                });
            };
            view! {
                <button type="button" class="link tree-toggle" title=t(l, K::ToggleDevices) aria-expanded=move || open().to_string() on:click=toggle>
                    {move || if open2() { "▾" } else { "▸" }}
                </button>
                " " {name} " " <span class="muted small">{format!("({count})")}</span>
            }
            .into_any()
        }),
        col(K::Vcpus, |p: &ResourceProviderRow, l| {
            provider_cell(l, &p.vcpus, QuotaUnit::Count)
        })
        .class("num"),
        col(K::Ram, |p: &ResourceProviderRow, l| {
            provider_cell(l, &p.memory_mb, QuotaUnit::Mib)
        })
        .class("num"),
        col(K::Disk, |p: &ResourceProviderRow, l| {
            provider_cell(l, &p.disk_gb, QuotaUnit::Gib)
        })
        .class("num"),
    ];
    if rows.iter().any(|r| r.vgpus.is_some()) {
        cols.push(
            col(K::Vgpus, |p: &ResourceProviderRow, l| {
                provider_cell(l, &p.vgpus, QuotaUnit::Count)
            })
            .class("num"),
        );
    }
    // Only when the cloud names vGPU types with custom traits (ttcp-dev doesn't).
    if rows.iter().any(|r| !r.custom_traits.is_empty()) {
        cols.push(col(K::VgpuType, |p: &ResourceProviderRow, _| {
            let traits = p.custom_traits.clone();
            if traits.is_empty() {
                return view! { <span>"-"</span> }.into_any();
            }
            view! { <ul class="plain">{traits.into_iter().map(|tr| view! { <li class="mono small">{tr}</li> }).collect_view()}</ul> }.into_any()
        }));
    }
    if rows.iter().any(|r| !r.others.is_empty()) {
        cols.push(col(K::OtherResources, |p: &ResourceProviderRow, _| {
            let others: Vec<String> = p.others.iter().map(|(k, r)| format!("{k} {} / {}", r.used, r.total)).collect();
            if others.is_empty() {
                return view! { <span>"-"</span> }.into_any();
            }
            view! { <ul class="plain">{others.into_iter().map(|o| view! { <li class="mono small">{o}</li> }).collect_view()}</ul> }.into_any()
        }));
    }
    cols
}

/// Search text for the *Resource Provider* tab: name and UUID.
fn provider_search(p: &ResourceProviderRow) -> String {
    format!("{} {}", p.name, p.uuid)
}

/// Hypervisors' *Resource Provider* tab: its own request, made only when the tab
/// is opened (Placement takes two calls per provider; see `fetch::FAN_OUT`).
#[component]
fn ResourceProvidersTab() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| get_admin_resource_providers());
    // Hosts whose devices are shown: none at first (199 devices on ttcp-dev).
    let expanded = RwSignal::new(std::collections::HashSet::<String>::new());
    view! {
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| {
                // Built from the rows (which columns apply), outside `view!` (AGENTS.md §6).
                let columns = StoredValue::new(provider_columns(&t.data, expanded));
                let (used, total, gpus, hosts) = vgpu_summary(&t.data);
                let summary = (total > 0).then(|| {
                    let q = Quota { key: QuotaKey::Vcpus, used, limit: Some(total), unit: QuotaUnit::Count };
                    view! {
                        <div class="donuts">
                            <Donut q=q label=K::VgpuUsage/>
                            <p class="muted small">{move || crate::i18n::gpus_on_hosts(i18n.lang(), gpus, hosts)}</p>
                        </div>
                    }
                });
                let rows = StoredValue::new(t.data);
                let (millis, calls) = (t.millis, t.calls);
                view! {
                    {summary}
                    // Rebuilt on each toggle with the expanded hosts' devices.
                    {move || {
                        let shown: Vec<ResourceProviderRow> = rows.with_value(|r| {
                            r.iter().filter(|p| p.depth == 0 || expanded.with(|e| e.contains(&p.host))).cloned().collect()
                        });
                        view! { <DataTable rows=shown columns=columns.get_value() search=provider_search timing=(millis, calls)/> }
                    }}
                }
            })}
        </Transition>
    }
}

/// Admin › Compute › Hypervisors: Horizon's *Hypervisor Summary* charts, then
/// the *Hypervisor*, *Compute Host* and (with Placement) *Resource Provider* tabs.
#[component]
pub fn HypervisorsPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| get_admin_hypervisors());
    let tab = RwSignal::new(0usize);
    let hypervisors = StoredValue::new(hypervisor_columns());
    let hosts = StoredValue::new(host_service_columns(true));
    view! {
        <PageHeader>
            <Transition fallback=|| ()>
                {move || data.get().and_then(Result::ok).map(|t| view! { <TimingChip millis=t.millis calls=t.calls/> })}
            </Transition>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| {
                let [cpu, ram, disk] = t.data.totals();
                let quota = |(used, total): (i64, i64), unit| Quota { key: QuotaKey::Vcpus, used, limit: Some(total), unit };
                let summary = [(K::VcpuUsage, quota(cpu, QuotaUnit::Count)), (K::MemoryUsage, quota(ram, QuotaUnit::Mib)), (K::LocalDiskUsage, quota(disk, QuotaUnit::Gib))];
                let mut labels = vec![K::HypervisorTab, K::ComputeHost];
                if t.data.placement {
                    labels.push(K::ResourceProvider);
                }
                let d = StoredValue::new(t.data);
                view! {
                    <section class="quota-group">
                        <h2 class="section-title">{move || i18n.t(K::HypervisorSummary)}</h2>
                        <div class="donuts">
                            {summary.into_iter().map(|(label, q)| view! { <Donut q=q label=label/> }).collect_view()}
                        </div>
                    </section>
                    <Tabs labels=labels active=tab/>
                    {move || match tab.get() {
                        1 => fallible(d.with_value(|d| d.hosts.clone()), hosts.get_value(), host_search),
                        2 => view! { <ResourceProvidersTab/> }.into_any(),
                        _ => view! { <DataTable rows=d.with_value(|d| d.hypervisors.clone()) columns=hypervisors.get_value() search=|h: &HypervisorRow| format!("{} {}", h.hostname, h.kind)/> }.into_any(),
                    }}
                }
            })}
        </Transition>
    }
}

/// Admin › Compute › Host Aggregates' columns: name, zone, hosts, metadata and each
/// aggregate's menu.
fn aggregate_columns() -> Vec<Col<AggregateRow>> {
    vec![
        col(K::Name, |a: &AggregateRow, _| a.name.clone()).sort_text(|a| a.name.clone()),
        col(
            K::AvailabilityZone,
            |a: &AggregateRow, _| view! { <AzPill name=a.zone.clone()/> },
        )
        .sort_text(|a| a.zone.clone().unwrap_or_default()),
        col(
            K::Hosts,
            |a: &AggregateRow, _| view! { <ul class="plain">{a.hosts.iter().map(|h| view! { <li>{h.clone()}</li> }).collect_view()}</ul> },
        ),
        col(K::Metadata, |a: &AggregateRow, l| specs_cell(l, &a.metadata)),
        col(
            K::Actions,
            |a: &AggregateRow, l| view! { <RowActions items=ops::aggregate_items(l, a)/> },
        )
        .class("actions"),
    ]
}

/// Search text: name, zone and hosts.
fn aggregate_search(a: &AggregateRow) -> String {
    format!("{} {} {}", a.name, a.zone.as_deref().unwrap_or(""), a.hosts.join(" "))
}

/// Every aggregate can be picked for the bulk delete.
fn aggregate_key(a: &AggregateRow) -> Option<(String, String)> {
    Some((a.id.clone(), a.name.clone()))
}

/// The *Availability Zones* table under Host Aggregates: each zone, its hosts and whether it
/// is available.
fn zone_columns() -> Vec<Col<ZoneRow>> {
    vec![
        col(
            K::AvailabilityZoneName,
            |z: &ZoneRow, _| view! { <AzPill name=Some(z.name.clone())/> },
        )
        .sort_text(|z| z.name.clone()),
        col(
            K::Hosts,
            |z: &ZoneRow, _| view! { <ul class="plain">{z.hosts.iter().map(|h| view! { <li>{h.clone()}</li> }).collect_view()}</ul> },
        ),
        col(K::Available, |z: &ZoneRow, l| yes_no(l, z.available)),
    ]
}

/// Admin › Compute › Host Aggregates, and the *Availability Zones* table
/// below it, as Horizon shows both on one page.
#[component]
pub fn AggregatesPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| get_admin_aggregates());
    let aggregates = StoredValue::new(aggregate_columns());
    let zones = StoredValue::new(zone_columns());
    let select = Select {
        key: aggregate_key,
        picked: RwSignal::new(Vec::new()),
    };
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteAggregates picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteAggregates, items, |id| OsAction::AggregateDelete { id }))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateAggregate).to_string())
                open=Arc::new(move || ops::create_aggregate_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| view! {
                <DataTable rows=t.data.aggregates columns=aggregates.get_value() search=aggregate_search timing=(t.millis, t.calls) on_refresh=move |_| data.refetch() select=select/>
                <h2 class="section-title">{move || i18n.t(K::AvailabilityZones)}</h2>
                {fallible(t.data.zones, zones.get_value(), |z: &ZoneRow| format!("{} {}", z.name, z.hosts.join(" ")))}
            })}
        </Transition>
    }
}

/// Admin › Volume › Volume Types' columns: name, description, associated QoS spec,
/// encryption, *Public*, extra specs and each type's menu.
fn volume_type_columns() -> Vec<Col<VolumeTypeRow>> {
    vec![
        col(K::Name, |v: &VolumeTypeRow, _| v.name.clone()).sort_text(|v| v.name.clone()),
        col(K::Description, |v: &VolumeTypeRow, _| dash(v.description.clone())),
        col(K::AssociatedQosSpec, |v: &VolumeTypeRow, _| {
            dash(v.qos.as_ref().map(|q| q.name.clone()))
        }),
        col(K::Encryption, |v: &VolumeTypeRow, l| yes_no(l, v.encrypted)),
        col(K::Public, |v: &VolumeTypeRow, l| yes_no(l, v.public)),
        col(K::ExtraSpecs, |v: &VolumeTypeRow, l| specs_cell(l, &v.extra_specs)),
        col(
            K::Actions,
            |v: &VolumeTypeRow, l| view! { <RowActions items=ops::volume_type_items(l, v)/> },
        )
        .class("actions"),
    ]
}

/// Bulk selection key of a volume type: its ID and name.
fn volume_type_key(v: &VolumeTypeRow) -> Option<(String, String)> {
    Some((v.id.clone(), v.name.clone()))
}

/// The *QoS Specs* table under Volume Types: name, consumer, specs and each spec's menu.
fn qos_columns() -> Vec<Col<QosSpecRow>> {
    vec![
        col(K::Name, |q: &QosSpecRow, _| q.name.clone()).sort_text(|q| q.name.clone()),
        col(K::Consumer, |q: &QosSpecRow, _| q.consumer.clone()),
        col(K::Specs, |q: &QosSpecRow, l| specs_cell(l, &q.specs)),
        col(
            K::Actions,
            |q: &QosSpecRow, l| view! { <RowActions items=ops::qos_spec_items(l, q)/> },
        )
        .class("actions"),
    ]
}

/// Admin › Volume › Volume Types, with the *QoS Specs* table below, as Horizon:
/// *Create Volume Type*, the bulk delete and each type's menu; *Create QoS
/// Spec* in the QoS table's toolbar and each spec's menu.
#[component]
pub fn VolumeTypesPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| get_admin_volume_types());
    let types = StoredValue::new(volume_type_columns());
    let qos = StoredValue::new(qos_columns());
    let select = Select {
        key: volume_type_key,
        picked: RwSignal::new(Vec::new()),
    };
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteVolumeTypes picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteVolumeTypes, items, |id| OsAction::VolumeTypeDelete { id }))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateVolumeType).to_string())
                open=Arc::new(move || ops::create_volume_type_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| {
                let create_qos = view! {
                    <CreateButton label=Signal::derive(move || i18n.t(K::CreateQosSpec).to_string())
                        open=Arc::new(move || ops::create_qos_spec_dialog(i18n.lang()))/>
                }.into_any();
                let q = t.data.qos;
                view! {
                    <DataTable rows=t.data.types columns=types.get_value() search=|v: &VolumeTypeRow| format!("{} {} {}", v.name, v.id, v.description.as_deref().unwrap_or(""))
                        timing=(t.millis, t.calls) on_refresh=move |_| data.refetch() select=select/>
                    <h2 class="section-title">{move || i18n.t(K::QosSpecs)}</h2>
                    {q.error.map(|e| view! { <ErrorBox message=e/> })}
                    <DataTable rows=q.rows columns=qos.get_value() search=|q: &QosSpecRow| format!("{} {}", q.name, q.consumer) extra=create_qos/>
                }
            })}
        </Transition>
    }
}

/// Admin › System › Defaults: Horizon's tab per service, *Quota Name* and
/// *Limit*, with *Update Defaults* on the Compute and Volume tabs.
#[component]
pub fn DefaultQuotasPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| get_default_quotas());
    let tab = RwSignal::new(0usize);
    view! {
        <PageHeader>
            <Transition fallback=|| ()>
                {move || data.get().and_then(Result::ok).map(|t| view! { <TimingChip millis=t.millis calls=t.calls/> })}
            </Transition>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| {
                let labels: Vec<K> = t.data.iter().map(|c| defaults_tab(c.service)).collect();
                let cards = StoredValue::new(t.data);
                view! {
                    <Tabs labels=labels active=tab/>
                    {move || cards.with_value(|c| c.get(tab.get()).cloned()).map(|c| {
                        let service = c.service;
                        // Nova and Cinder have a `default` quota class; Neutron's
                        // defaults are configuration, so its tab stays read-only.
                        let update = if matches!(service, QuotaService::Compute | QuotaService::Volume) && c.error.is_none() {
                            let card = c.clone();
                            view! {
                                <CreateButton label=Signal::derive(move || i18n.t(K::UpdateDefaults).to_string())
                                    open=Arc::new(move || ops::update_defaults_dialog(i18n.lang(), &card))/>
                            }.into_any()
                        } else {
                            ().into_any()
                        };
                        view! {
                            {c.error.map(|e| view! { <ErrorBox message=e/> })}
                            <DataTable rows=c.rows columns=default_columns(service) search=|r: &QuotaRow| r.key.clone() extra=update/>
                        }
                    })}
                }
            })}
        </Transition>
    }
}

/// Horizon's tab names on Defaults.
fn defaults_tab(s: QuotaService) -> K {
    match s {
        QuotaService::Compute => K::ComputeQuotas,
        QuotaService::Volume => K::VolumeQuotas,
        _ => K::NetworkQuotas,
    }
}

/// Admin › System › Defaults' columns for one service's tab: Horizon's label for the quota and
/// its default limit (`-1` shown as unlimited).
fn default_columns(service: QuotaService) -> Vec<Col<QuotaRow>> {
    vec![
        col(K::QuotaName, move |r: &QuotaRow, l| quota_label(l, service, &r.key)).sort_text(|r| r.key.clone()),
        col(K::QuotaLimit, move |r: &QuotaRow, l| {
            r.limit.map_or_else(
                || t(l, K::Unlimited).to_string(),
                |n| fmt_amount(n, service.unit(&r.key)),
            )
        })
        .sort_num(|r| r.limit.unwrap_or(i64::MAX)),
    ]
}

/// Admin › System › Metadata Definitions: Glance's namespaces, with Horizon's
/// *Import Namespace*, the bulk delete (unprotected ones) and each row's *Edit
/// Namespace*, *Update Associations* and *Delete Namespace*. The name opens
/// [`MetadefDetailPage`].
#[component]
pub fn MetadefsPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_metadef_namespaces());
    let columns = StoredValue::new(metadef_columns());
    let select = Select {
        key: metadef_key,
        picked: RwSignal::new(Vec::new()),
    };
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteNamespaces picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteNamespaces, items, |id| OsAction::MetadefNamespaceDelete { id }))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::ImportNamespace).to_string())
                open=Arc::new(move || ops::import_namespace_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| view! {
                <DataTable rows=t.data columns=columns.get_value() search=metadef_search timing=(t.millis, t.calls) on_refresh=move |_| data.refetch() select=select/>
            })}
        </Transition>
    }
}

/// Admin › Metadata Definitions › a namespace, Horizon's
/// `/admin/metadata_defs/<namespace>/detail`: *Overview* (the namespace and the
/// resource types it applies to) and *Contents* (its properties and objects,
/// and the definition's JSON). The header has the row's menu.
#[component]
pub fn MetadefDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_metadef_namespace(id));
    let tab = RwSignal::new(0usize);
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let d = StoredValue::new(td.data);
                view! {
                    <DetailHeader group=K::System panel=K::MetadataDefinitions href="/admin/metadata_defs"
                        name=d.with_value(|d| d.namespace.name.clone()) timing=(td.millis, td.calls)
                        items=Signal::derive(move || d.with_value(|d| ops::metadef_items(i18n.lang(), &d.namespace)))/>
                    <Tabs labels=vec![K::Overview, K::Contents] active=tab/>
                    {move || if tab.get() == 1 { d.with_value(|d| metadef_contents(d, i18n.lang())) } else { d.with_value(|d| metadef_overview(d, i18n.lang())) }}
                }
            })}
        </Transition>
    }
}

/// *Overview*: Horizon's fields, then the resource types with their prefix and target.
fn metadef_overview(d: &MetadefNamespaceDetail, l: Lang) -> AnyView {
    let n = d.namespace.clone();
    let (created, updated) = (
        d.created_at.as_deref().map(short_time),
        d.updated_at.as_deref().map(short_time),
    );
    let associations = d.associations.clone();
    let columns = vec![
        col(K::Name, |a: &MetadefAssociation, _| mono(a.name.clone())).sort_text(|a| a.name.clone()),
        col(K::Prefix, |a: &MetadefAssociation, _| {
            a.prefix.clone().map_or_else(|| dash(None::<String>).into_any(), mono)
        }),
        col(K::PropertiesTarget, |a: &MetadefAssociation, _| {
            dash(a.properties_target.clone())
        }),
    ];
    view! {
        <div class="cards">
            <Card title=K::Information>
                <Kv label=K::DisplayName>{n.name.clone()}</Kv>
                <Kv label=K::Namespace>{mono(n.namespace.clone())}</Kv>
                <Kv label=K::Description>{dash(n.description.clone())}</Kv>
                <Kv label=K::Public>{yes_no(l, n.public)}</Kv>
                <Kv label=K::Protected>{yes_no(l, n.protected)}</Kv>
                <Kv label=K::Created>{dash(created)}</Kv>
                <Kv label=K::Updated>{dash(updated)}</Kv>
            </Card>
        </div>
        <h2 class="section-title">{t(l, K::ResourceTypes)}</h2>
        <DataTable rows=associations columns=columns search=|a: &MetadefAssociation| a.name.clone()/>
    }
    .into_any()
}

/// *Contents*: the properties, the objects with their properties' names, and
/// the definition's JSON (what Horizon's tab shows).
fn metadef_contents(d: &MetadefNamespaceDetail, l: Lang) -> AnyView {
    let props = vec![
        col(K::Name, |p: &MetadefProperty, _| mono(p.name.clone())).sort_text(|p| p.name.to_lowercase()),
        col(K::Title, |p: &MetadefProperty, _| dash(p.title.clone()))
            .sort_text(|p| p.title.clone().unwrap_or_default()),
        col(K::Type, |p: &MetadefProperty, _| mono(p.kind.clone())).sort_text(|p| p.kind.clone()),
        col(K::DefaultValue, |p: &MetadefProperty, _| dash(p.default.clone())),
        col(K::AllowedValues, |p: &MetadefProperty, _| {
            dash((!p.allowed.is_empty()).then(|| p.allowed.join(", ")))
        }),
        col(K::Description, |p: &MetadefProperty, _| dash(p.description.clone())),
    ];
    let objects = vec![
        col(K::Name, |o: &MetadefObject, _| o.name.clone()).sort_text(|o| o.name.to_lowercase()),
        col(K::Description, |o: &MetadefObject, _| dash(o.description.clone())),
        col(K::Properties, |o: &MetadefObject, _| {
            dash((!o.properties.is_empty()).then(|| o.properties.join(", ")))
        }),
    ];
    let (properties, objs, raw) = (d.properties.clone(), d.objects.clone(), d.raw.clone());
    view! {
        <h2 class="section-title">{t(l, K::Properties)}</h2>
        <DataTable rows=properties columns=props
            search=|p: &MetadefProperty| format!("{} {} {}", p.name, p.title.as_deref().unwrap_or(""), p.description.as_deref().unwrap_or(""))/>
        <h2 class="section-title">{t(l, K::Objects)}</h2>
        <DataTable rows=objs columns=objects
            search=|o: &MetadefObject| format!("{} {} {}", o.name, o.description.as_deref().unwrap_or(""), o.properties.join(" "))/>
        <details class="raw-schema">
            <summary>{t(l, K::MetadefDefinition)}</summary>
            <pre class="console-log">{raw}</pre>
        </details>
    }
    .into_any()
}

/// Protected namespaces can't be picked for the bulk delete (Glance refuses).
fn metadef_key(n: &MetadefNamespace) -> Option<(String, String)> {
    (!n.protected).then(|| (n.namespace.clone(), n.name.clone()))
}

/// Admin › System › Metadata Definitions' columns: the display name links to the namespace's
/// detail page (by its namespace, which the APIs use); then description, resource types,
/// *Public*, *Protected* and each row's menu.
fn metadef_columns() -> Vec<Col<MetadefNamespace>> {
    vec![
        col(K::Name, |n: &MetadefNamespace, _| {
            // The namespace is what the APIs and `Update Metadata` use.
            let ns =
                (n.name != n.namespace).then(|| view! { <div class="muted mono small">{n.namespace.clone()}</div> });
            view! { {link(format!("/admin/metadata_defs/{}", n.namespace), n.name.clone())} {ns} }
        })
        .sort_text(|n| n.name.to_lowercase()),
        col(K::Description, |n: &MetadefNamespace, _| dash(n.description.clone())),
        col(K::ResourceTypes, |n: &MetadefNamespace, _| {
            let types = n.resource_types.clone();
            view! { <ul class="plain">{types.into_iter().map(|t| view! { <li class="mono small">{t}</li> }).collect_view()}</ul> }
        }),
        col(K::Public, |n: &MetadefNamespace, l| yes_no(l, n.public)),
        col(K::Protected, |n: &MetadefNamespace, l| yes_no(l, n.protected)),
        col(
            K::Actions,
            |n: &MetadefNamespace, l| view! { <RowActions items=ops::metadef_items(l, n)/> },
        )
        .class("actions"),
    ]
}

/// Search text for Metadata Definitions: namespace, display name, description and resource
/// types.
fn metadef_search(n: &MetadefNamespace) -> String {
    format!(
        "{} {} {} {}",
        n.name,
        n.namespace,
        n.description.as_deref().unwrap_or(""),
        n.resource_types.join(" ")
    )
}

/// Search text for the *Services* tab: name, type, region and endpoints.
fn service_search(s: &CatalogService) -> String {
    format!(
        "{} {} {} {}",
        s.name,
        s.service_type,
        s.region,
        s.endpoints
            .iter()
            .map(|(_, u)| u.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    )
}

/// Search text for the Compute and Block Storage services tabs: binary, host and zone.
fn host_search(s: &HostService) -> String {
    format!("{} {} {}", s.binary, s.host, s.zone.as_deref().unwrap_or(""))
}

/// Search text for the *Network Agents* tab: type, binary, host and zone.
fn agent_search(a: &NetworkAgent) -> String {
    format!(
        "{} {} {} {}",
        a.agent_type,
        a.binary,
        a.host,
        a.zone.as_deref().unwrap_or("")
    )
}

/// What Moonlight detected in this project and region, and which panels it
/// hides because of it: operator information, so an Admin tab
/// rather than part of Project › API Access.
#[component]
pub fn ModulesCard() -> impl IntoView {
    let i18n = use_i18n();
    let caps = crate::components::dialog::use_dialogs().caps;
    let mv = |v: Option<(u16, u16)>| v.map(|(a, b)| format!("{a}.{b}")).unwrap_or_else(|| "-".into());
    move || {
        let c = caps.get();
        if !c.known {
            return view! { <Card title=K::Modules><p class="muted">{i18n.t(K::ModulesUnknown)}</p></Card> }.into_any();
        }
        let hidden: Vec<String> = crate::components::nav::PROJECT_NAV
            .iter()
            .flat_map(|g| g.panels.iter().map(move |p| (g, p)))
            .filter(|(g, p)| !(g.requires.met(&c) && p.requires.met(&c)))
            .map(|(_, p)| i18n.t(p.label).to_string())
            .collect();
        let mut exts = c.network_extensions.clone();
        exts.sort();
        view! {
            <Card title=K::Modules>
                <Kv label=K::ComputeApi>{format!("≤ {}", mv(c.compute_mv))}</Kv>
                <Kv label=K::VolumeApi>{format!("≤ {}", mv(c.volume_mv))}</Kv>
                <Kv label=K::LoadBalancerApi>{format!("≤ {}", mv(c.lb_mv))}</Kv>
                <Kv label=K::Plugins>{if c.plugins.is_empty() { "-".to_string() } else { c.plugins.iter().map(|p| p.name()).collect::<Vec<_>>().join(", ") }}</Kv>
                <Kv label=K::HiddenPanels>{if hidden.is_empty() { "-".to_string() } else { hidden.join(", ") }}</Kv>
                <Kv label=K::NetworkExtensions><span class="mono small">{exts.join(" ")}</span></Kv>
            </Card>
        }
        .into_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some<T>(on: bool) -> Option<Fallible<T>> {
        on.then(|| Fallible {
            rows: vec![],
            error: None,
        })
    }

    #[test]
    fn flavor_metadata_is_yes_no_and_still_searchable() {
        let mut f = FlavorRow {
            id: "f1".into(),
            name: "m1.large".into(),
            vcpus: 4,
            ram_mb: 8192,
            disk_gb: 40,
            ephemeral_gb: 0,
            swap_mb: 0,
            rxtx_factor: None,
            public: true,
            extra_specs: vec![],
            access: vec![],
        };
        assert!(!has_metadata(&f));
        f.extra_specs = vec![("hw:cpu_policy".into(), "dedicated".into())];
        assert!(has_metadata(&f));
        // The row no longer lists the specs, but the search box still finds them.
        assert!(flavor_search(&f).contains("hw:cpu_policy=dedicated"));
    }

    fn info(compute: bool, volume: bool, agents: bool) -> SystemInfo {
        SystemInfo {
            services: vec![],
            compute: some(compute),
            volume: some(volume),
            agents: some(agents),
        }
    }

    #[test]
    fn tabs_follow_what_the_cloud_has() {
        assert_eq!(
            tabs(&info(true, true, true)),
            [
                K::CatalogServices,
                K::ComputeServices,
                K::BlockStorageServices,
                K::NetworkAgents,
                K::Modules
            ],
            "Horizon's order, then the capabilities"
        );
        assert_eq!(
            tabs(&info(true, false, false)),
            [K::CatalogServices, K::ComputeServices, K::Modules],
            "no Cinder, no agent extension"
        );
    }

    #[test]
    fn a_failed_list_keeps_its_error() {
        let f: Fallible<HostService> = Err(AppError::new(ErrorKind::Forbidden, "GET compute services")).into();
        assert_eq!((f.rows.len(), f.error.as_deref()), (0, Some("GET compute services")));
    }

    #[test]
    fn defaults_tabs_use_horizons_names() {
        assert_eq!(
            [QuotaService::Compute, QuotaService::Volume, QuotaService::Network].map(defaults_tab),
            [K::ComputeQuotas, K::VolumeQuotas, K::NetworkQuotas]
        );
    }

    #[test]
    fn dhcp_agents_column_only_with_counts() {
        let net = |c: Option<usize>| Owned {
            project: "-".into(),
            host: None,
            project_id: None,
            row: NetworkRow {
                id: "n1".into(),
                name: "n".into(),
                subnets: vec![],
                shared: false,
                external: false,
                status: "ACTIVE".into(),
                admin_up: true,
                availability_zones: vec![],
                owned: false,
                dhcp_agents: c,
                provider: None,
            },
        };
        let has = |rows: &[Owned<NetworkRow>]| {
            drop_empty_dhcp_col(admin_network_columns(), rows)
                .iter()
                .any(|c| c.header == K::DhcpAgents)
        };
        assert!(has(&[net(Some(0)), net(Some(2))]), "ML2/OVS: counted, zeros included");
        assert!(
            !has(&[net(None), net(None)]),
            "OVN has no DHCP agent: no column of dashes"
        );
        assert!(!has(&[]), "an empty page has nothing to count");
    }
}
