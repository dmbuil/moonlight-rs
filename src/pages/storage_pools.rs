//! Admin › Volume › Storage Pools: Cinder's backend pools with their capacity
//! (`scheduler-stats/get_pools`), a *Storage Summary* of donuts like *Hypervisor
//! Summary*, and one page per pool with the capabilities its driver reports
//! beyond the canonical ones. Read-only; the server functions refuse a token
//! without the `admin` role.

use leptos::prelude::*;

use super::details::{Card, DetailHeader, Kv, mono, param, yes_no};
use crate::api::{get_admin_storage_pool, get_admin_storage_pools};
use crate::components::dialog::data_version;
use crate::components::table::{Col, DataTable, TimingChip, col};
use crate::components::widgets::{PageHeader, Skeleton, age_cell_since, dash, render};
use crate::i18n::{K, Lang, grouped, t, use_i18n};
use crate::model::*;
use crate::pages::lists::link;
use crate::pages::overview::Donut;
use leptos_router::components::A;

/// `s` as one URL path segment: everything but RFC 3986's unreserved
/// characters percent-encoded, so a pool's `@` and `#` survive the trip.
fn path_segment(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A pool's detail page.
fn pool_href(name: &str) -> String {
    format!("/admin/storage_pools/{}", path_segment(name))
}

/// Admin › Volumes filtered by this host: the volumes on the pool.
fn volumes_href(pool: &str) -> String {
    let mut m = leptos_router::params::ParamsMap::new();
    m.insert("f", "host".to_string());
    m.insert("q", pool.to_string());
    format!("/admin/volumes{}", m.to_query_string())
}

/// A capacity as the user reads it: GB with two decimals, *Infinite*,
/// *Unknown*, or `-` when not reported.
fn capacity_text(l: Lang, c: Option<Capacity>) -> String {
    match c {
        Some(Capacity::Gb(v)) => grouped(l, v, 2),
        Some(Capacity::Infinite) => t(l, K::Infinite).to_string(),
        Some(Capacity::Unknown) => t(l, K::Unknown).to_string(),
        None => "-".to_string(),
    }
}

/// Sort key of a capacity: hundredths of GB, *Infinite* above any number,
/// *Unknown* and missing below.
fn capacity_sort(c: Option<Capacity>) -> i64 {
    match c {
        Some(Capacity::Gb(v)) => (v * 100.0).round() as i64,
        Some(Capacity::Infinite) => i64::MAX,
        Some(Capacity::Unknown) => -1,
        None => -2,
    }
}

/// *Thin*, *Thick*, both, or `-` when the driver reports neither.
fn provisioning(l: Lang, p: &StoragePoolRow) -> String {
    let kinds: Vec<&str> = [(p.thin, K::Thin), (p.thick, K::Thick)]
        .into_iter()
        .filter(|(on, _)| *on == Some(true))
        .map(|(_, k)| t(l, k))
        .collect();
    if kinds.is_empty() { "-".into() } else { kinds.join(", ") }
}

/// The reserved percentage, `5 %` (`-` when not reported).
fn reserved(l: Lang, p: &StoragePoolRow) -> String {
    p.reserved_percentage
        .map_or_else(|| "-".into(), |v| format!("{} %", grouped(l, v, 0)))
}

/// The volume count, linked to Admin › Volumes filtered by the pool.
fn volume_count(p: &StoragePoolRow) -> AnyView {
    if p.volumes == 0 {
        return "0".into_any();
    }
    let (href, n) = (volumes_href(&p.name), p.volumes.to_string());
    view! { <A href=href>{n}</A> }.into_any()
}

/// The *Storage Pools* table's columns.
fn pool_columns() -> Vec<Col<StoragePoolRow>> {
    let gb = |header: K, f: fn(&StoragePoolRow) -> Option<Capacity>| {
        col(header, move |p: &StoragePoolRow, l| capacity_text(l, f(p)))
            .sort_num(move |p| capacity_sort(f(p)))
            .class("num")
    };
    vec![
        col(K::Name, |p: &StoragePoolRow, _| {
            link(pool_href(&p.name), p.name.clone())
        })
        .sort_text(|p| p.name.clone()),
        col(K::Backend, |p: &StoragePoolRow, _| dash(p.backend.clone()))
            .sort_text(|p| p.backend.clone().unwrap_or_default()),
        col(K::Protocol, |p: &StoragePoolRow, _| dash(p.protocol.clone())),
        col(K::Vendor, |p: &StoragePoolRow, _| {
            let version = p
                .driver_version
                .clone()
                .map(|v| view! { " " <span class="muted">{v}</span> });
            view! { {dash(p.vendor.clone())} {version} }
        }),
        gb(K::TotalGb, |p| p.total),
        gb(K::FreeGb, |p| p.free),
        gb(K::AllocatedGb, |p| p.allocated),
        gb(K::ProvisionedGb, |p| p.provisioned),
        col(K::Reserved, |p: &StoragePoolRow, l| reserved(l, p)).class("num"),
        col(K::OverSubscription, |p: &StoragePoolRow, _| {
            dash(p.over_subscription.clone())
        })
        .class("num"),
        col(K::Provisioning, |p: &StoragePoolRow, l| provisioning(l, p)),
        col(K::Volumes, |p: &StoragePoolRow, _| volume_count(p))
            .sort_num(|p| p.volumes as i64)
            .class("num"),
        col(K::Snapshots, |p: &StoragePoolRow, _| p.snapshots.to_string())
            .sort_num(|p| p.snapshots as i64)
            .class("num"),
    ]
}

/// Search text: pool name, backend and vendor.
fn pool_search(p: &StoragePoolRow) -> String {
    format!(
        "{} {} {}",
        p.name,
        p.backend.as_deref().unwrap_or(""),
        p.vendor.as_deref().unwrap_or("")
    )
}

/// Admin › Volume › Storage Pools: the *Storage Summary* donuts, then every pool.
#[component]
pub fn StoragePoolsPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| get_admin_storage_pools());
    let columns = StoredValue::new(pool_columns());
    view! {
        <PageHeader>
            <Transition fallback=|| ()>
                {move || data.get().and_then(Result::ok).map(|t| view! { <TimingChip millis=t.millis calls=t.calls/> })}
            </Transition>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |t| {
                let [used, allocated, provisioned] = t.data.totals();
                let quota = |(used, total): (i64, i64)| Quota { key: QuotaKey::VolumeStorage, used, limit: Some(total), unit: QuotaUnit::Gib };
                let mut summary = vec![(K::CapacityUsage, quota(used)), (K::AllocatedCapacity, quota(allocated))];
                if t.data.reports_provisioned() {
                    summary.push((K::ProvisionedCapacity, quota(provisioned)));
                }
                view! {
                    <section class="quota-group">
                        <h2 class="section-title">{move || i18n.t(K::StorageSummary)}</h2>
                        <div class="donuts">
                            {summary.into_iter().map(|(label, q)| view! { <Donut q=q label=label/> }).collect_view()}
                        </div>
                    </section>
                    <DataTable rows=t.data.pools columns=columns.get_value() search=pool_search on_refresh=move |_| data.refetch()/>
                }
            })}
        </Transition>
    }
}

/// One pool: its canonical capabilities as two cards, then *Vendor
/// Capabilities*, the driver's own keys (raw, not translated), when it has any.
#[component]
pub fn StoragePoolDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let name = param("name");
    let data = Resource::new(move || (name(), data_version()), |(n, _)| get_admin_storage_pool(n));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let p = td.data;
                let l = i18n.lang();
                let columns = vec![
                    col(K::Key, |kv: &(String, String), _| mono(kv.0.clone())).sort_text(|kv| kv.0.clone()),
                    col(K::Value, |kv: &(String, String), _| mono(kv.1.clone())),
                ];
                // Owned values for the cards' children, which are closures.
                let (name, backend, protocol, vendor, version, ratio, extra) = (
                    p.name.clone(),
                    p.backend.clone(),
                    p.protocol.clone(),
                    p.vendor.clone(),
                    p.driver_version.clone(),
                    p.over_subscription.clone(),
                    p.extra.clone(),
                );
                let updated = p.updated.clone();
                let multiattach = p.multiattach.map(|m| yes_no(l, m));
                let [total, free, allocated, provisioned] = [p.total, p.free, p.allocated, p.provisioned].map(|c| capacity_text(l, c));
                let (reserved, provisioning, snapshots) = (reserved(l, &p), provisioning(l, &p), p.snapshots.to_string());
                let volumes = StoredValue::new(p);
                view! {
                    <DetailHeader group=K::VolumeAdminGroup panel=K::StoragePools href="/admin/storage_pools"
                        name=name.clone() timing=(td.millis, td.calls) items=Signal::derive(Vec::new)/>
                    <div class="cards">
                        <Card title=K::Information>
                            <Kv label=K::Name>{mono(name.clone())}</Kv>
                            <Kv label=K::Backend>{dash(backend.clone())}</Kv>
                            <Kv label=K::Protocol>{dash(protocol.clone())}</Kv>
                            <Kv label=K::Vendor>{dash(vendor.clone())}</Kv>
                            <Kv label=K::DriverVersion>{dash(version.clone())}</Kv>
                            <Kv label=K::Multiattach>{dash(multiattach.map(str::to_string))}</Kv>
                            <Kv label=K::Updated>{updated.clone().map_or_else(|| "-".into_any(), |u| age_cell_since(l, &u))}</Kv>
                        </Card>
                        <Card title=K::Capacity>
                            <Kv label=K::TotalGb>{total.clone()}</Kv>
                            <Kv label=K::FreeGb>{free.clone()}</Kv>
                            <Kv label=K::AllocatedGb>{allocated.clone()}</Kv>
                            <Kv label=K::ProvisionedGb>{provisioned.clone()}</Kv>
                            <Kv label=K::Reserved>{reserved.clone()}</Kv>
                            <Kv label=K::OverSubscription>{dash(ratio.clone())}</Kv>
                            <Kv label=K::Provisioning>{provisioning.clone()}</Kv>
                            <Kv label=K::Volumes>{volumes.with_value(volume_count)}</Kv>
                            <Kv label=K::Snapshots>{snapshots.clone()}</Kv>
                        </Card>
                    </div>
                    {(!extra.is_empty()).then(|| view! {
                        <h2 class="section-title">{move || i18n.t(K::VendorCapabilities)}</h2>
                        <DataTable rows=extra columns=columns search=|kv: &(String, String)| format!("{} {}", kv.0, kv.1)/>
                    })}
                }
            })}
        </Transition>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_names_survive_the_url() {
        assert_eq!(
            pool_href("ctl-0@tripleo_ceph#tripleo ceph"),
            "/admin/storage_pools/ctl-0%40tripleo_ceph%23tripleo%20ceph",
            "`#` would end the path and `@` is reserved"
        );
        assert_eq!(volumes_href("c@ceph#fast"), "/admin/volumes?f=host&q=c%40ceph%23fast");
    }

    #[test]
    fn capacities_read_as_text_and_sort_with_infinite_on_top() {
        let l = Lang::EnUs;
        assert_eq!(capacity_text(l, Some(Capacity::Gb(1234.5))), "1,234.50");
        assert_eq!(capacity_text(Lang::EsEs, Some(Capacity::Gb(12345.5))), "12.345,50");
        assert_eq!(capacity_text(l, Some(Capacity::Infinite)), "Infinite");
        assert_eq!(capacity_text(l, None), "-");
        let mut keys = [
            Some(Capacity::Unknown),
            Some(Capacity::Infinite),
            Some(Capacity::Gb(3.0)),
            None,
        ]
        .map(capacity_sort);
        keys.sort();
        assert_eq!(keys, [-2, -1, 300, i64::MAX]);
    }
}
