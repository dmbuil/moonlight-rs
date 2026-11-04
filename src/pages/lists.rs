//! List panels. Each one is: a server function, a column set, a search
//! string and, optionally, a page-level create action. Names link to the
//! detail pages; the last column holds Horizon-style row actions.

use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::components::A;

use crate::api::*;
use crate::components::dialog::{caps, data_version, BulkButton, CreateButton, Dialog, RowActions};
use crate::components::ops;
use crate::components::table::{available, col, Col, DataTable, Select};
use crate::components::widgets::{
    dash, data_url, image_busy, poll_while_busy, render, short_time, snapshot_busy, volume_busy, AzPill, AzPills, Busy, Copyable, PageHeader, Pill, Skeleton, Tone,
};
use crate::pages::details::{Card, Kv};
use crate::i18n::{bytes, image_visibility, key_manager_term, resource_status, server_group_policy, t, use_i18n, Lang, K};
use crate::model::*;

type Opener = &'static [(K, fn(Lang) -> Dialog)];

/// Horizon's bulk *Delete …* button: its label, the `(id, name)` of a row that
/// can be deleted (`None` when its menu offers no delete), and the action.
type Bulk<T> = (K, fn(&T) -> Option<(String, String)>, fn(String) -> OsAction);

/// Every row can be deleted; `(id, name)`.
macro_rules! key {
    ($id:ident, $name:ident) => {
        |x| Some((x.$id.clone(), x.$name.clone()))
    };
}

/// A list panel. With `busy = fn(&T) -> bool`, the page refetches every
/// `[ui] poll_interval_ms` while any row is busy and shows *Updating…*
/// (roadmap 46; GET only, see `widgets::poll_while_busy`).
macro_rules! list_page {
    (@busy) => { |_| false };
    (@busy $busy:expr) => { $busy };
    (@drawer) => { None };
    (@drawer $drawer:expr) => { Some($drawer) };
    ($name:ident, $fetch:ident, $ty:ty, $cols:expr, $search:expr, $create:expr, $bulk:expr $(, busy = $busy:expr)? $(, drawer = $drawer:expr)?) => {
        #[component]
        pub fn $name() -> impl IntoView {
            let i18n = use_i18n();
            let data = Resource::new(data_version, |_| $fetch());
            let busy: fn(&$ty) -> bool = list_page!(@busy $($busy)?);
            let drawer: Option<fn(&$ty, Lang) -> AnyView> = list_page!(@drawer $($drawer)?);
            let polling = poll_while_busy(data, move || data.with(|r| matches!(r, Some(Ok(td)) if td.data.iter().any(busy))));
            let columns: StoredValue<Vec<Col<$ty>>> = StoredValue::new($cols);
            let create: Opener = $create;
            let (bulk_label, key, make): Bulk<$ty> = $bulk;
            let select = Select { key, picked: RwSignal::new(Vec::new()) };
            view! {
                <PageHeader>
                    {move || polling.get().then(|| view! { <span class="updating">{i18n.t(K::Updating)}</span> })}
                    <BulkButton label=bulk_label picked=select.picked
                        open=Arc::new(move |l, items| ops::bulk_delete(l, bulk_label, items, make))/>
                    {create.iter().map(|&(label, open)| view! {
                        <CreateButton
                            label=Signal::derive(move || i18n.t(label).to_string())
                            open=Arc::new(move || open(i18n.lang()))
                        />
                    }).collect_view()}
                </PageHeader>
                <Transition fallback=|| view! { <Skeleton/> }>
                    {move || render(data.get(), |td| view! {
                        <DataTable
                            rows=td.data
                            columns=available(columns.get_value(), &caps())
                            search=$search
                            timing=(td.millis, td.calls)
                            on_refresh=move |_| data.refetch()
                            select=select
                            drawer=drawer
                        />
                    })}
                </Transition>
            }
        }
    };
}

fn yes_no(l: Lang, b: bool) -> &'static str {
    t(l, if b { K::Yes } else { K::No })
}

pub fn status_pill(status: &str, l: Lang) -> AnyView {
    view! { <Pill tone=Tone::resource(status) label=resource_status(l, status)/> }.into_any()
}

/// The status pill, with the busy bar beside it while the resource changes
/// state on its own (`creating`, `extending`…; roadmap 46).
pub fn moving_status(status: &str, l: Lang, busy: bool) -> AnyView {
    let label = resource_status(l, status).to_string();
    view! {
        <Pill tone=Tone::resource(status) label=label.clone()/>
        {busy.then(|| view! { <Busy label=label/> })}
    }
    .into_any()
}

pub fn admin_state(l: Lang, up: bool) -> &'static str {
    t(l, if up { K::AdminUp } else { K::AdminDown })
}

/// A bold link to a detail page.
pub fn link(href: String, text: String) -> AnyView {
    view! { <A href=href attr:class="strong">{text}</A> }.into_any()
}

fn actions_col<T: 'static>(f: fn(Lang, &T) -> Vec<crate::components::dialog::MenuItem>) -> Col<T> {
    col(K::Actions, move |x: &T, l| view! { <RowActions items=f(l, x)/> }).class("actions")
}

// --- Compute ------------------------------------------------------------------

list_page!(
    ImagesPage,
    list_images,
    Image,
    vec![
        col(K::Name, |i: &Image, _| link(format!("/project/images/{}", i.id), i.name.clone())).sort_text(|i| i.name.clone()),
        col(K::Type, |i: &Image, l| t(l, if i.is_snapshot { K::ImageTypeSnapshot } else { K::ImageTypeImage }))
            .sort_num(|i| i.is_snapshot as i64),
        col(K::Status, |i: &Image, l| image_status(i, l)).sort_text(|i| i.status.clone()),
        col(K::Visibility, |i: &Image, l| image_visibility(l, &i.visibility)).sort_text(|i| i.visibility.clone()),
        col(K::Protected, |i: &Image, l| yes_no(l, i.protected)),
        col(K::DiskFormat, |i: &Image, _| dash(i.disk_format.clone()).to_uppercase()),
        col(K::Size, |i: &Image, _| i.size.map(bytes).unwrap_or_else(|| "-".into()))
            .sort_num(|i| i.size.unwrap_or(0) as i64)
            .class("num"),
        actions_col(|l, i: &Image| ops::image_items(l, i)),
    ],
    // The owner left the columns (roadmap 56) but can still be filtered on.
    |i: &Image| {
        let owner = [i.owner_name.as_deref(), i.owner_id.as_deref()].into_iter().flatten().collect::<Vec<_>>().join(" ");
        format!("{} {} {} {} {owner}", i.name, i.id, i.status, i.visibility)
    },
    &[(K::CreateImage, ops::create_image_dialog)],
    (K::DeleteImages, |i: &Image| (i.owned_by_project && !i.protected).then(|| (i.id.clone(), i.name.clone())), |id| OsAction::ImageDelete { id }),
    busy = |i: &Image| image_busy(&i.status),
    drawer = image_drawer
);

/// The Images list's status: the Glance status, plus a warning pill with Glance's reason when
/// the last import failed (`os_glance_failed_import`, roadmap 45). The image goes back to
/// `queued`, so without it a failed *URL* import looks like one that has not started.
fn image_status(i: &Image, l: Lang) -> AnyView {
    let failed = i.import_failed.clone().map(|why| {
        view! { <span title=why><Pill tone=Tone::Err label=t(l, K::ImageImportFailed).to_string()/></span> }
    });
    view! { {moving_status(&i.status, l, image_busy(&i.status))}{failed} }.into_any()
}

/// The Images list's row drawer: the image's metadata (custom properties,
/// sorted by key), as Horizon shows it.
fn image_drawer(i: &Image, l: Lang) -> AnyView {
    let mut props = i.properties.clone();
    props.sort();
    if props.is_empty() {
        return view! { <p class="muted drawer-empty">{t(l, K::NoMetadata)}</p> }.into_any();
    }
    view! {
        <dl class="kv drawer-kv">
            {props.into_iter().map(|(k, v)| view! { <dt class="mono">{k}</dt><dd>{dash(Some(v))}</dd> }).collect_view()}
        </dl>
    }
    .into_any()
}

list_page!(
    KeyPairsPage,
    list_keypairs,
    KeyPair,
    vec![
        col(K::Name, |k: &KeyPair, _| link(format!("/project/key_pairs/{}", k.name), k.name.clone())).sort_text(|k| k.name.clone()),
        col(K::Type, |k: &KeyPair, _| dash(k.key_type.clone())),
        col(K::Fingerprint, |k: &KeyPair, _| view! { <span class="mono">{k.fingerprint.clone()}</span> }),
        actions_col(|l, k: &KeyPair| ops::keypair_items(l, &k.name)),
    ],
    |k: &KeyPair| format!("{} {}", k.name, k.fingerprint),
    &[(K::CreateKeyPair, ops::generate_keypair_dialog), (K::ImportPublicKey, ops::import_keypair_dialog)],
    (K::DeleteKeyPairs, key!(name, name), |name| OsAction::KeyPairDelete { name })
);

list_page!(
    ServerGroupsPage,
    list_server_groups,
    ServerGroup,
    vec![
        col(K::Name, |g: &ServerGroup, _| link(format!("/project/server_groups/{}", g.id), g.name.clone())).sort_text(|g| g.name.clone()),
        col(K::Id, |g: &ServerGroup, _| view! { <span class="mono">{g.id.clone()}</span> }),
        col(K::Policies, |g: &ServerGroup, l| g.policies.iter().map(|p| server_group_policy(l, p)).collect::<Vec<_>>().join(", ")),
        col(K::Members, |g: &ServerGroup, _| g.members.len().to_string()).sort_num(|g| g.members.len() as i64).class("num"),
        actions_col(|l, g: &ServerGroup| ops::server_group_items(l, g)),
    ],
    |g: &ServerGroup| format!("{} {} {}", g.name, g.id, g.policies.join(" ")),
    &[(K::CreateServerGroup, ops::create_server_group_dialog)],
    (K::DeleteServerGroups, key!(id, name), |id| OsAction::ServerGroupDelete { id })
);

// --- Volumes ------------------------------------------------------------------

pub fn attachments(v: &Volume) -> AnyView {
    if v.attachments.is_empty() {
        return "-".into_any();
    }
    view! {
        <ul class="plain">
            {v.attachments.iter().map(|a| {
                let name = a.server_name.clone().unwrap_or_else(|| a.server_id.clone());
                view! {
                    <li>
                        <A href=format!("/project/instances/{}", a.server_id)>{name}</A>
                        {a.device.clone().map(|d| view! { " " <span class="mono muted">{d}</span> })}
                    </li>
                }
            }).collect_view()}
        </ul>
    }
    .into_any()
}

/// Owner project name; unknown owners show a short id (full id on hover).
pub fn owner_cell(i: &Image) -> AnyView {
    match (&i.owner_name, &i.owner_id) {
        (Some(n), _) => view! { <span class:strong=i.owned_by_project>{n.clone()}</span> }.into_any(),
        (None, Some(id)) => view! { <span class="mono muted" title=id.clone()>{id.chars().take(8).collect::<String>()}"…"</span> }.into_any(),
        (None, None) => "-".into_any(),
    }
}

fn create_volume(l: Lang) -> Dialog {
    ops::create_volume_dialog(l, None)
}

list_page!(
    VolumesPage,
    list_volumes,
    Volume,
    vec![
        col(K::Name, |v: &Volume, _| link(format!("/project/volumes/{}", v.id), v.name.clone())).sort_text(|v| v.name.clone()),
        col(K::Description, |v: &Volume, _| dash(v.description.clone())),
        col(K::Size, |v: &Volume, _| format!("{} GiB", v.size_gb)).sort_num(|v| v.size_gb).class("num"),
        col(K::Status, |v: &Volume, l| moving_status(&v.status, l, volume_busy(&v.status))).sort_text(|v| v.status.clone()),
        col(K::Type, |v: &Volume, _| dash(v.volume_type.clone())).sort_text(|v| v.volume_type.clone().unwrap_or_default()),
        col(K::AttachedTo, |v: &Volume, _| attachments(v)),
        col(K::AvailabilityZone, |v: &Volume, _| view! { <AzPill name=v.availability_zone.clone()/> }),
        col(K::Bootable, |v: &Volume, l| yes_no(l, v.bootable)),
        col(K::Encrypted, |v: &Volume, l| yes_no(l, v.encrypted)),
        actions_col(|l, v: &Volume| ops::volume_items(l, v)),
    ],
    |v: &Volume| {
        let servers: Vec<_> = v.attachments.iter().filter_map(|a| a.server_name.clone()).collect();
        format!("{} {} {} {} {}", v.name, v.id, v.status, v.volume_type.as_deref().unwrap_or(""), servers.join(" "))
    },
    &[(K::AcceptTransfer, ops::accept_transfer_dialog), (K::CreateVolume, create_volume)],
    (K::DeleteVolumes, key!(id, name), |id| OsAction::VolumeDelete { id }),
    busy = |v: &Volume| volume_busy(&v.status)
);

list_page!(
    SnapshotsPage,
    list_snapshots,
    Snapshot,
    vec![
        col(K::Name, |s: &Snapshot, _| link(format!("/project/snapshots/{}", s.id), s.name.clone())).sort_text(|s| s.name.clone()),
        col(K::Description, |s: &Snapshot, _| dash(s.description.clone())),
        col(K::Size, |s: &Snapshot, _| format!("{} GiB", s.size_gb)).sort_num(|s| s.size_gb).class("num"),
        col(K::Status, |s: &Snapshot, l| moving_status(&s.status, l, snapshot_busy(&s.status))).sort_text(|s| s.status.clone()),
        col(K::VolumeName, |s: &Snapshot, _| {
            let name = s.volume_name.clone().unwrap_or_else(|| s.volume_id.clone());
            view! { <A href=format!("/project/volumes/{}", s.volume_id)>{name}</A> }
        }),
        col(K::Created, |s: &Snapshot, _| short_time(&s.created)).sort_text(|s| s.created.clone()).class("nowrap"),
        actions_col(|l, s: &Snapshot| ops::snapshot_items(l, s)),
    ],
    |s: &Snapshot| format!("{} {} {} {}", s.name, s.id, s.status, s.volume_name.as_deref().unwrap_or("")),
    &[],
    (K::DeleteSnapshots, key!(id, name), |id| OsAction::SnapshotDelete { id }),
    busy = |s: &Snapshot| snapshot_busy(&s.status)
);

// Volume backups (roadmap 5, `backup` plugin). `creating`, `restoring` and `deleting`
// are in `snapshot_busy`'s list.
list_page!(
    BackupsPage,
    list_backups,
    BackupRow,
    vec![
        col(K::Name, |b: &BackupRow, _| link(format!("/project/backups/{}", b.id), b.name.clone())).sort_text(|b| b.name.clone()),
        col(K::Description, |b: &BackupRow, _| dash(b.description.clone())),
        col(K::Size, |b: &BackupRow, _| format!("{} GiB", b.size_gb)).sort_num(|b| b.size_gb).class("num"),
        col(K::Status, |b: &BackupRow, l| moving_status(&b.status, l, snapshot_busy(&b.status))).sort_text(|b| b.status.clone()),
        col(K::VolumeName, |b: &BackupRow, _| link(format!("/project/volumes/{}", b.volume.id), b.volume.name.clone())),
        col(K::IncrementalBackup, |b: &BackupRow, l| yes_no(l, b.incremental)),
        col(K::Created, |b: &BackupRow, _| short_time(&b.created)).sort_text(|b| b.created.clone()).class("nowrap"),
        actions_col(|l, b: &BackupRow| ops::backup_items(l, b)),
    ],
    |b: &BackupRow| format!("{} {} {} {}", b.name, b.id, b.status, b.volume.name),
    &[(K::CreateBackup, ops::create_backup)],
    (K::DeleteBackups, key!(id, name), |id| OsAction::BackupDelete { id }),
    busy = |b: &BackupRow| snapshot_busy(&b.status)
);

// Generic volume groups (roadmap 5). Their transitional statuses (`creating`,
// `updating`, `deleting`) are a subset of the snapshots', hence `snapshot_busy`.
list_page!(
    VolumeGroupsPage,
    list_volume_groups,
    VolumeGroupRow,
    vec![
        col(K::Name, |g: &VolumeGroupRow, _| link(format!("/project/volume_groups/{}", g.id), g.name.clone())).sort_text(|g| g.name.clone()),
        col(K::Description, |g: &VolumeGroupRow, _| dash(g.description.clone())),
        col(K::Status, |g: &VolumeGroupRow, l| moving_status(&g.status, l, snapshot_busy(&g.status))).sort_text(|g| g.status.clone()),
        col(K::AvailabilityZone, |g: &VolumeGroupRow, _| view! { <AzPill name=g.availability_zone.clone()/> }),
        col(K::VolumeTypes, |g: &VolumeGroupRow, _| g.volume_types.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", ")),
        col(K::HasSnapshots, |g: &VolumeGroupRow, l| yes_no(l, g.has_snapshots)),
        actions_col(|l, g: &VolumeGroupRow| ops::volume_group_items(l, g)),
    ],
    |g: &VolumeGroupRow| format!("{} {} {} {}", g.name, g.id, g.status, g.group_type.name),
    &[(K::CreateGroup, ops::create_volume_group)],
    (K::DeleteGroups, key!(id, name), |id| OsAction::VolumeGroupDelete { id, delete_volumes: false }),
    busy = |g: &VolumeGroupRow| snapshot_busy(&g.status)
);

list_page!(
    GroupSnapshotsPage,
    list_group_snapshots,
    GroupSnapshotRow,
    vec![
        col(K::Name, |s: &GroupSnapshotRow, _| link(format!("/project/vg_snapshots/{}", s.id), s.name.clone())).sort_text(|s| s.name.clone()),
        col(K::Description, |s: &GroupSnapshotRow, _| dash(s.description.clone())),
        col(K::Status, |s: &GroupSnapshotRow, l| moving_status(&s.status, l, snapshot_busy(&s.status))).sort_text(|s| s.status.clone()),
        col(K::VolumeGroup, |s: &GroupSnapshotRow, _| link(format!("/project/volume_groups/{}", s.group.id), s.group.name.clone())).sort_text(|s| s.group.name.clone()),
        col(K::Created, |s: &GroupSnapshotRow, _| short_time(&s.created)).sort_text(|s| s.created.clone()).class("nowrap"),
        actions_col(|l, s: &GroupSnapshotRow| ops::group_snapshot_items(l, s)),
    ],
    |s: &GroupSnapshotRow| format!("{} {} {} {}", s.name, s.id, s.status, s.group.name),
    &[],
    (K::DeleteGroupSnapshots, key!(id, name), |id| OsAction::GroupSnapshotDelete { id }),
    busy = |s: &GroupSnapshotRow| snapshot_busy(&s.status)
);

// --- Network ------------------------------------------------------------------

/// Only the project's own networks can be picked for *Delete Networks*: external
/// and shared ones have no row menu either (`ops::network_items`), and Neutron
/// refuses the delete (roadmap 73).
fn network_bulk_key(n: &NetworkRow) -> Option<(String, String)> {
    n.owned.then(|| (n.id.clone(), n.name.clone()))
}

/// The `default` security group can't be deleted (Neutron recreates it; its menu
/// offers no *Delete*, `ops::security_group_items`), so it isn't selectable (roadmap 73).
fn security_group_bulk_key(g: &SecurityGroupRow) -> Option<(String, String)> {
    (g.name != "default").then(|| (g.id.clone(), g.name.clone()))
}

list_page!(
    NetworksPage,
    list_networks,
    NetworkRow,
    vec![
        col(K::Name, |n: &NetworkRow, _| link(format!("/project/networks/{}", n.id), n.name.clone())).sort_text(|n| n.name.clone()),
        col(K::Subnets, |n: &NetworkRow, _| view! {
            <ul class="plain">{n.subnets.iter().map(|s| view! { <li class="mono">{s.clone()}</li> }).collect_view()}</ul>
        }),
        col(K::Shared, |n: &NetworkRow, l| yes_no(l, n.shared)),
        col(K::External, |n: &NetworkRow, l| yes_no(l, n.external)).sort_num(|n| n.external as i64),
        col(K::Status, |n: &NetworkRow, l| status_pill(&n.status, l)),
        col(K::AdminState, |n: &NetworkRow, l| admin_state(l, n.admin_up)),
        col(K::AvailabilityZone, |n: &NetworkRow, _| view! { <AzPills names=n.availability_zones.clone()/> }),
        actions_col(|l, n: &NetworkRow| ops::network_items(l, n)),
    ],
    |n: &NetworkRow| format!("{} {} {}", n.name, n.id, n.subnets.join(" ")),
    &[(K::CreateNetwork, ops::create_network_dialog)],
    (K::DeleteNetworks, network_bulk_key, |id| OsAction::NetworkDelete { id })
);

list_page!(
    RoutersPage,
    list_routers,
    RouterRow,
    vec![
        col(K::Name, |r: &RouterRow, _| link(format!("/project/routers/{}", r.id), r.name.clone())).sort_text(|r| r.name.clone()),
        col(K::Status, |r: &RouterRow, l| status_pill(&r.status, l)),
        col(K::ExternalNetwork, |r: &RouterRow, _| dash(r.external_network.clone())),
        col(K::AdminState, |r: &RouterRow, l| admin_state(l, r.admin_up)),
        col(K::AvailabilityZone, |r: &RouterRow, _| view! { <AzPills names=r.availability_zones.clone()/> }),
        actions_col(|l, r: &RouterRow| ops::router_items(l, r)),
    ],
    |r: &RouterRow| format!("{} {} {}", r.name, r.id, r.external_network.as_deref().unwrap_or("")),
    &[(K::CreateRouter, ops::create_router_dialog)],
    (K::DeleteRouters, key!(id, name), |id| OsAction::RouterDelete { id })
);

list_page!(
    SecurityGroupsPage,
    list_security_groups,
    SecurityGroupRow,
    vec![
        col(K::Name, |g: &SecurityGroupRow, _| link(format!("/project/security_groups/{}", g.id), g.name.clone()))
            .sort_text(|g| g.name.clone()),
        col(K::Id, |g: &SecurityGroupRow, _| view! { <span class="mono">{g.id.clone()}</span> }),
        col(K::Description, |g: &SecurityGroupRow, _| dash(g.description.clone())),
        col(K::Shared, |g: &SecurityGroupRow, l| yes_no(l, g.shared)),
        col(K::Rules, |g: &SecurityGroupRow, _| g.rules.len().to_string()).sort_num(|g| g.rules.len() as i64).class("num"),
        actions_col(|l, g: &SecurityGroupRow| ops::security_group_items(l, &g.id, &g.name, g.description.as_deref(), true)),
    ],
    |g: &SecurityGroupRow| format!("{} {} {}", g.name, g.id, g.description.as_deref().unwrap_or("")),
    &[(K::CreateSecurityGroup, ops::create_security_group_dialog)],
    (K::DeleteSecurityGroups, security_group_bulk_key, |id| OsAction::SecurityGroupDelete { id })
);

list_page!(
    FloatingIpsPage,
    list_floating_ips,
    FloatingIpRow,
    vec![
        col(K::IpAddress, |f: &FloatingIpRow, _| {
            let (href, ip) = (format!("/project/floating_ips/{}", f.id), f.ip.clone());
            view! { <A href=href attr:class="mono strong">{ip}</A> }
        })
        .sort_text(|f| f.ip.clone()),
        col(K::Description, |f: &FloatingIpRow, _| dash(f.description.clone())),
        col(K::DnsName, |f: &FloatingIpRow, _| dash(f.dns_name.clone())).requires(needs::DNS),
        col(K::DnsDomain, |f: &FloatingIpRow, _| dash(f.dns_domain.clone())).requires(needs::DNS),
        col(K::MappedFixedIp, |f: &FloatingIpRow, _| {
            let fixed = dash(f.fixed_ip.clone());
            match (&f.instance_id, &f.instance_name) {
                (Some(id), name) => {
                    let label = name.clone().unwrap_or_else(|| id.clone());
                    view! {
                        <A href=format!("/project/instances/{id}")>{label}</A>
                        " " <span class="mono muted">{fixed}</span>
                    }
                    .into_any()
                }
                _ => view! { <span class="mono">{fixed}</span> }.into_any(),
            }
        }),
        col(K::Pool, |f: &FloatingIpRow, _| f.pool.clone()).sort_text(|f| f.pool.clone()),
        col(K::Status, |f: &FloatingIpRow, l| status_pill(&f.status, l)).sort_text(|f| f.status.clone()),
        actions_col(|l, f: &FloatingIpRow| ops::floating_ip_items(l, f)),
    ],
    |f: &FloatingIpRow| {
        let o = |v: &Option<String>| v.clone().unwrap_or_default();
        format!("{} {} {} {} {} {}", f.ip, o(&f.fixed_ip), o(&f.instance_name), f.pool, o(&f.description), o(&f.dns_name))
    },
    &[(K::AllocateIp, ops::allocate_ip_dialog)],
    (K::ReleaseIps, key!(id, ip), |id| OsAction::FloatingIpRelease { id })
);

list_page!(
    LoadBalancersPage,
    list_load_balancers,
    LoadBalancerRow,
    vec![
        col(K::Name, |l: &LoadBalancerRow, _| link(format!("/project/load_balancers/{}", l.id), l.name.clone())).sort_text(|l| l.name.clone()),
        col(K::Description, |l: &LoadBalancerRow, _| dash(l.description.clone())),
        col(K::IpAddress, |l: &LoadBalancerRow, _| view! { <span class="mono">{dash(l.vip_address.clone())}</span> })
            .sort_text(|l| l.vip_address.clone().unwrap_or_default()),
        col(K::AvailabilityZone, |l: &LoadBalancerRow, _| view! { <AzPill name=l.availability_zone.clone()/> }),
        col(K::OperatingStatus, |l: &LoadBalancerRow, lang| status_pill(&l.operating_status, lang)).sort_text(|l| l.operating_status.clone()),
        col(K::ProvisioningStatus, |l: &LoadBalancerRow, lang| status_pill(&l.provisioning_status, lang))
            .sort_text(|l| l.provisioning_status.clone()),
        col(K::AdminState, |l: &LoadBalancerRow, lang| admin_state(lang, l.admin_up)),
        actions_col(|l, b: &LoadBalancerRow| ops::load_balancer_items(l, b)),
    ],
    |l: &LoadBalancerRow| format!("{} {} {} {}", l.name, l.id, l.vip_address.as_deref().unwrap_or(""), l.description.as_deref().unwrap_or("")),
    &[(K::CreateLoadBalancer, ops::create_load_balancer_dialog)],
    (K::DeleteLoadBalancers, key!(id, name), |id| OsAction::LoadBalancerDelete { id, cascade: true }),
    busy = |l: &LoadBalancerRow| lb_pending(&l.provisioning_status)
);

list_page!(
    TrunksPage,
    list_trunks,
    TrunkRow,
    vec![
        col(K::Name, |t: &TrunkRow, _| link(format!("/project/trunks/{}", t.id), t.name.clone())).sort_text(|t| t.name.clone()),
        col(K::ParentPort, |t: &TrunkRow, _| link(format!("/project/networks/ports/{}", t.parent.id), t.parent.name.clone())),
        col(K::Subports, |t: &TrunkRow, _| t.subports.len().to_string()).sort_num(|t| t.subports.len() as i64).class("num"),
        col(K::Status, |t: &TrunkRow, l| status_pill(&t.status, l)).sort_text(|t| t.status.clone()),
        col(K::AdminState, |t: &TrunkRow, l| admin_state(l, t.admin_up)),
        actions_col(|l, t: &TrunkRow| ops::trunk_items(l, t)),
    ],
    |t: &TrunkRow| format!("{} {} {}", t.name, t.id, t.parent.name),
    &[(K::CreateTrunk, ops::create_trunk_dialog)],
    (K::DeleteTrunks, key!(id, name), |id| OsAction::TrunkDelete { id })
);

// --- Key Manager (Barbican, roadmap 55) --------------------------------------

list_page!(
    SecretsPage,
    list_secrets,
    SecretRow,
    vec![
        col(K::Name, |x: &SecretRow, _| link(format!("/project/key_manager/secrets/{}", x.id), x.name.clone())).sort_text(|x| x.name.clone()),
        col(K::SecretType, |x: &SecretRow, l| key_manager_term(l, &x.secret_type)).sort_text(|x| x.secret_type.clone()),
        col(K::Algorithm, |x: &SecretRow, _| crate::pages::key_manager::cipher(x)),
        col(K::ContentType, |x: &SecretRow, _| dash(x.content_type.clone())),
        col(K::Status, |x: &SecretRow, l| status_pill(&x.status, l)).sort_text(|x| x.status.clone()),
        col(K::Expiration, |x: &SecretRow, l| crate::pages::key_manager::expiry_cell(l, x.expiration.as_deref()))
            .sort_text(|x| x.expiration.clone().unwrap_or_default())
            .class("nowrap"),
        col(K::Created, |x: &SecretRow, _| short_time(&x.created)).sort_text(|x| x.created.clone()).class("nowrap"),
        actions_col(|l, x: &SecretRow| ops::secret_items(l, x)),
    ],
    |x: &SecretRow| format!("{} {} {} {}", x.name, x.id, x.secret_type, x.algorithm.as_deref().unwrap_or("")),
    &[(K::StoreSecret, ops::store_secret_dialog)],
    (K::DeleteSecrets, key!(id, name), |id| OsAction::SecretDelete { id })
);

list_page!(
    ContainersPage,
    list_containers,
    ContainerRow,
    vec![
        col(K::Name, |c: &ContainerRow, _| link(format!("/project/key_manager/containers/{}", c.id), c.name.clone())).sort_text(|c| c.name.clone()),
        col(K::ContainerType, |c: &ContainerRow, l| key_manager_term(l, &c.container_type)).sort_text(|c| c.container_type.clone()),
        col(K::Secrets, |c: &ContainerRow, l| {
            let roles: Vec<String> = c.secrets.iter().map(|s| format!("{}: {}", key_manager_term(l, &s.role), s.name)).collect();
            if roles.is_empty() { "-".to_string() } else { roles.join(", ") }
        }),
        col(K::Consumers, |c: &ContainerRow, _| c.consumers.len().to_string()).sort_num(|c| c.consumers.len() as i64).class("num"),
        col(K::Status, |c: &ContainerRow, l| status_pill(&c.status, l)).sort_text(|c| c.status.clone()),
        col(K::Created, |c: &ContainerRow, _| short_time(&c.created)).sort_text(|c| c.created.clone()).class("nowrap"),
        actions_col(|l, c: &ContainerRow| ops::container_items(l, c)),
    ],
    |c: &ContainerRow| format!("{} {} {} {}", c.name, c.id, c.container_type, c.secrets.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(" ")),
    &[(K::CreateContainer, ops::create_container_dialog)],
    (K::DeleteContainers, key!(id, name), |id| OsAction::ContainerDelete { id })
);

list_page!(
    QosPoliciesPage,
    list_qos_policies,
    QosPolicyRow,
    vec![
        col(K::Name, |p: &QosPolicyRow, _| link(format!("/project/qos/{}", p.id), p.name.clone())).sort_text(|p| p.name.clone()),
        col(K::Description, |p: &QosPolicyRow, _| dash(p.description.clone())),
        col(K::Shared, |p: &QosPolicyRow, l| yes_no(l, p.shared)),
        col(K::DefaultPolicy, |p: &QosPolicyRow, l| yes_no(l, p.is_default)),
        col(K::Rules, |p: &QosPolicyRow, l| {
            let kinds: Vec<String> = p.rules.iter().map(|r| ops::qos_kind(l, &r.kind)).collect();
            if kinds.is_empty() { "-".to_string() } else { kinds.join(", ") }
        }),
        actions_col(|l, p: &QosPolicyRow| ops::qos_policy_items(l, p)),
    ],
    |p: &QosPolicyRow| format!("{} {} {}", p.name, p.id, p.description.as_deref().unwrap_or("")),
    &[(K::CreatePolicy, ops::create_qos_policy_dialog)],
    // Shared policies of other projects cannot be selected.
    (K::DeletePolicies, |p: &QosPolicyRow| p.owned.then(|| (p.id.clone(), p.name.clone())), |id| OsAction::QosPolicyDelete { id })
);

// --- API access -----------------------------------------------------------------

#[component]
pub fn ApiAccessPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| api_access());
    let creds = Resource::new(data_version, |_| get_credentials());
    let show = RwSignal::new(false);
    let columns: StoredValue<Vec<Col<ApiEndpoint>>> = StoredValue::new(vec![
        col(K::Service, |e: &ApiEndpoint, _| {
            let label = e.name.clone().unwrap_or_else(|| e.service_type.clone());
            view! { <span class="strong">{label}</span> " " <span class="muted">{e.service_type.clone()}</span> }
        })
        .sort_text(|e| e.service_type.clone()),
        col(K::ServiceEndpoint, |e: &ApiEndpoint, _| view! { <span class="mono">{dash(Some(e.url.clone()))}</span> }),
    ]);
    view! {
        <PageHeader>
            <Transition fallback=|| ()>
                {move || creds.get().and_then(Result::ok).map(|c| {
                    let stem = c.file_stem();
                    view! {
                        <button type="button" class="btn" aria-expanded=move || show.get().to_string()
                            on:click=move |_| show.update(|v| *v = !*v)>
                            {move || i18n.t(if show.get() { K::HideCredentials } else { K::ViewCredentials })}
                        </button>
                        <a class="btn" href=data_url(&c.openrc) download=format!("{stem}-openrc.sh")>
                            {move || i18n.t(K::DownloadOpenRc)}
                        </a>
                        <a class="btn" href=data_url(&c.clouds_yaml) download="clouds.yaml">
                            {move || i18n.t(K::DownloadCloudsYaml)}
                        </a>
                    }
                })}
            </Transition>
        </PageHeader>
        // WebSSO users have no password for these files (roadmap 80).
        <Transition fallback=|| ()>
            {move || creds.get().and_then(Result::ok).filter(|c| c.federated).map(|_| view! {
                // `.alert` is a flex row: one block per line keeps the link in its sentence.
                <div class="alert alert-info alert-lines" role="note">
                    <p>{move || i18n.t(K::FederatedApiAccessNote)}</p>
                    <p>
                        {move || i18n.t(K::FederatedApiAccessScripts)} " "
                        <A href="/identity/application_credentials">{move || i18n.t(K::ApplicationCredentialLink)}</A>
                    </p>
                </div>
            })}
        </Transition>
        <Transition fallback=|| ()>
            {move || show.get().then(|| creds.get().and_then(Result::ok)).flatten().map(|c| view! {
                <Card title=K::UserCredentials>
                    <Kv label=K::UserName><Copyable value=c.user_name.clone()/></Kv>
                    <Kv label=K::UserId><Copyable value=c.user_id.clone() mono=true/></Kv>
                    <Kv label=K::Domain><Copyable value=c.user_domain.clone()/></Kv>
                    <Kv label=K::ProjectName><Copyable value=c.project_name.clone()/></Kv>
                    <Kv label=K::ProjectId><Copyable value=c.project_id.clone() mono=true/></Kv>
                    <Kv label=K::Region>{dash(Some(c.region.clone()))}</Kv>
                    <Kv label=K::AuthUrl><Copyable value=c.auth_url.clone() mono=true/></Kv>
                </Card>
                <p class="muted">{move || i18n.t(K::NoSecretsNote)}</p>
            })}
        </Transition>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), |eps| view! {
                <DataTable
                    rows=eps
                    columns=columns.get_value()
                    search=|e: &ApiEndpoint| format!("{} {} {}", e.service_type, e.name.as_deref().unwrap_or(""), e.url)
                    on_refresh=move |_| data.refetch()
                />
            })}
        </Transition>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network(owned: bool) -> NetworkRow {
        NetworkRow {
            id: "n1".into(),
            name: "net".into(),
            subnets: vec![],
            shared: !owned,
            external: false,
            status: "ACTIVE".into(),
            admin_up: true,
            availability_zones: vec![],
            owned,
            dhcp_agents: None,
            provider: None,
        }
    }

    #[test]
    fn only_owned_networks_are_selectable_for_bulk_delete() {
        assert_eq!(network_bulk_key(&network(true)), Some(("n1".into(), "net".into())));
        assert_eq!(network_bulk_key(&network(false)), None, "shared/external networks of other projects");
    }

    #[test]
    fn default_security_group_is_not_selectable_for_bulk_delete() {
        let g = |name: &str| SecurityGroupRow { id: "g1".into(), name: name.into(), description: None, rules: vec![], shared: false };
        assert_eq!(security_group_bulk_key(&g("web")), Some(("g1".into(), "web".into())));
        assert_eq!(security_group_bulk_key(&g("default")), None);
    }
}
