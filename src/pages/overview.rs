use leptos::prelude::*;

use leptos_router::hooks::use_query_map;

use crate::api::{get_usage, overview};
use crate::components::dialog::data_version;
use crate::components::table::{DataTable, TimingChip, col};
use crate::components::widgets::{ErrorBox, PageHeader, Skeleton, age_cell, data_url, render};
use crate::i18n::{K, bytes, grouped, use_i18n, used_of};
use crate::model::{Quota, QuotaGroup, QuotaKey, QuotaUnit, ServerUsage, UsageSummary};

fn group_label(g: QuotaGroup) -> K {
    match g {
        QuotaGroup::Compute => K::Compute,
        QuotaGroup::Volume => K::VolumesGroup,
        QuotaGroup::Network => K::NetworkGroup,
    }
}

fn quota_label(k: QuotaKey) -> K {
    match k {
        QuotaKey::Instances => K::Instances,
        QuotaKey::Vcpus => K::Vcpus,
        QuotaKey::Ram => K::Ram,
        QuotaKey::Volumes => K::Volumes,
        QuotaKey::VolumeSnapshots => K::VolumeSnapshots,
        QuotaKey::VolumeStorage => K::VolumeStorage,
        QuotaKey::FloatingIps => K::FloatingIps,
        QuotaKey::SecurityGroups => K::SecurityGroups,
        QuotaKey::SecurityGroupRules => K::SecurityGroupRules,
        QuotaKey::Networks => K::Networks,
        QuotaKey::Ports => K::Ports,
        QuotaKey::Routers => K::Routers,
    }
}

pub fn fmt_amount(v: i64, unit: QuotaUnit) -> String {
    match unit {
        QuotaUnit::Count => v.to_string(),
        QuotaUnit::Mib => bytes((v.max(0) as u64) * 1024 * 1024),
        QuotaUnit::Gib => bytes((v.max(0) as u64) * 1024 * 1024 * 1024),
    }
}

/// Fraction used, 0..=1 (0 for unlimited).
pub fn ratio(q: &Quota) -> f64 {
    match q.limit {
        Some(l) if l > 0 => (q.used as f64 / l as f64).clamp(0.0, 1.0),
        Some(_) => 1.0,
        None => 0.0,
    }
}

/// A used-of-limit ring with its caption; `label` replaces the quota's name
/// (Admin › Hypervisors' *VCPU Usage*…).
#[component]
pub fn Donut(q: Quota, #[prop(optional)] label: Option<K>) -> impl IntoView {
    let i18n = use_i18n();
    let r = ratio(&q);
    let circumference = 2.0 * std::f64::consts::PI * 30.0;
    let dash = format!("{:.2} {:.2}", r * circumference, circumference);
    let level = if r >= 0.9 {
        "full"
    } else if r >= 0.7 {
        "high"
    } else {
        "normal"
    };
    let label = label.unwrap_or_else(|| quota_label(q.key));
    let used = fmt_amount(q.used, q.unit);
    let limit = q.limit.map(|l| fmt_amount(l, q.unit));
    let center = fmt_amount(q.used, q.unit);
    // No arc at 0% or unlimited: a zero-length round-capped stroke draws a dot.
    let show_arc = r > 0.0;
    let sub = Signal::derive(move || used_of(i18n.lang(), &used, limit.as_deref()));
    view! {
        <figure class=format!("donut {level}")>
            <svg viewBox="0 0 72 72" width="72" height="72" role="img" aria-label=move || format!("{}: {}", i18n.t(label), sub.get())>
                <circle class="track" cx="36" cy="36" r="30"></circle>
                {show_arc.then(|| view! {
                    <circle class="arc" cx="36" cy="36" r="30" stroke-dasharray=dash.clone() transform="rotate(-90 36 36)"></circle>
                })}
                <text x="36" y="41" text-anchor="middle">{center}</text>
            </svg>
            <figcaption>
                <strong>{move || i18n.t(label)}</strong>
                <span>{sub}</span>
            </figcaption>
        </figure>
    }
}

#[component]
pub fn OverviewPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| overview());
    view! {
        <PageHeader>
            <Transition fallback=|| ()>
                {move || data.get().and_then(Result::ok).map(|t| view! { <TimingChip millis=t.millis calls=t.calls/> })}
            </Transition>
        </PageHeader>
        <h2 class="section-title">{move || i18n.t(K::LimitSummary)}</h2>
        <Transition fallback=|| view! { <crate::components::widgets::Skeleton rows=3/> }>
            {move || render(data.get(), |t| t.data.into_iter().map(|s| view! {
                <section class="quota-group">
                    <h3>{move || i18n.t(group_label(s.group))}</h3>
                    {s.error.clone().map(|e| view! { <ErrorBox message=e/> })}
                    <div class="donuts">
                        {s.quotas.into_iter().map(|q| view! { <Donut q=q/> }).collect_view()}
                    </div>
                </section>
            }).collect_view())}
        </Transition>
        <UsageSection/>
    }
}

fn mib(v: i64) -> String {
    bytes((v.max(0) as u64) * 1024 * 1024)
}

/// The usage period form (`?start=&end=`, dates), for a table's toolbar next
/// to the filter. Shared with Admin › Overview.
pub fn period_form(start: String, end: String) -> AnyView {
    let i18n = use_i18n();
    view! {
        <form class="usage-form" method="get" title=move || i18n.t(K::UsagePeriodHelp)>
            <label>{move || i18n.t(K::DateFrom)} " "
                <input type="date" name="start" value=start required/>
            </label>
            <label>{move || i18n.t(K::DateTo)} " "
                <input type="date" name="end" value=end required/>
            </label>
            <button type="submit" class="btn">{move || i18n.t(K::Submit)}</button>
        </form>
    }
    .into_any()
}

/// Horizon's usage totals (active instances and RAM, the period's hours) and
/// the *Download CSV Summary* link for `csv`, named after the period.
#[component]
pub fn UsageTotals(usage: UsageSummary, csv: String) -> impl IntoView {
    let i18n = use_i18n();
    let file = format!("usage-{}-{}.csv", usage.start, usage.end);
    // Numbers are formatted inside the view's closures: the separators follow
    // the language, read reactively (not in the component body). Grouped by
    // thousands for reading (roadmap 86); the CSV stays plain.
    let (instances, ram) = (usage.active_instances() as f64, mib(usage.active_ram_mb()));
    let hours = [
        (K::VcpuHours, usage.vcpu_hours()),
        (K::GbHours, usage.gb_hours()),
        (K::RamHours, usage.ram_mb_hours()),
    ];
    view! {
        <div class="usage-totals">
            <dl>
                <div><dt>{move || i18n.t(K::ActiveInstances)}</dt><dd>{move || grouped(i18n.lang(), instances, 0)}</dd></div>
                <div><dt>{move || i18n.t(K::ActiveRam)}</dt><dd>{ram}</dd></div>
                {hours.into_iter().map(|(k, v)| view! {
                    <div><dt>{move || i18n.t(k)}</dt><dd>{move || grouped(i18n.lang(), v, 2)}</dd></div>
                }).collect_view()}
            </dl>
            <a class="btn" href=data_url(&csv) download=file>{move || i18n.t(K::DownloadCsv)}</a>
        </div>
    }
}

/// Horizon's *Usage Summary*: period form, totals, per-instance table, CSV.
#[component]
fn UsageSection() -> impl IntoView {
    let i18n = use_i18n();
    let query = use_query_map();
    let period = move || query.with(|q| (q.get("start").unwrap_or_default(), q.get("end").unwrap_or_default()));
    let data = Resource::new(
        move || (period(), data_version()),
        |((start, end), _)| get_usage(start, end),
    );
    view! {
        <h2 class="section-title">{move || i18n.t(K::UsageSummary)}</h2>
        <Transition fallback=|| view! { <Skeleton rows=3/> }>
            {move || render(data.get(), |t| {
                let u: UsageSummary = t.data;
                let csv = u.csv();
                let period = period_form(u.start.clone(), u.end.clone());
                view! {
                    <UsageTotals usage=u.clone() csv=csv/>
                    <DataTable
                        rows=u.listed()
                        columns=vec![
                            col(K::InstanceName, |s: &ServerUsage, _| s.name.clone()).sort_text(|s| s.name.clone()),
                            col(K::Vcpus, |s: &ServerUsage, _| s.vcpus.to_string()).sort_num(|s| s.vcpus),
                            col(K::Disk, |s: &ServerUsage, _| format!("{} GB", s.local_gb)).sort_num(|s| s.local_gb),
                            col(K::Ram, |s: &ServerUsage, _| mib(s.memory_mb)).sort_num(|s| s.memory_mb),
                            col(K::Age, |s: &ServerUsage, l| age_cell(l, s.uptime)).sort_num(|s| s.uptime),
                        ]
                        search=|s: &ServerUsage| s.name.clone()
                        timing=(t.millis, t.calls)
                        on_refresh=move |_| data.refetch()
                        extra=period
                    />
                }
            })}
        </Transition>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_ratio_and_units() {
        let q = |used, limit, unit| Quota {
            key: QuotaKey::Ram,
            used,
            limit,
            unit,
        };
        assert_eq!(ratio(&q(5, Some(10), QuotaUnit::Count)), 0.5);
        assert_eq!(ratio(&q(5, None, QuotaUnit::Count)), 0.0);
        assert_eq!(ratio(&q(50, Some(10), QuotaUnit::Count)), 1.0);
        assert_eq!(ratio(&q(0, Some(0), QuotaUnit::Count)), 1.0);
        assert_eq!(fmt_amount(16384, QuotaUnit::Mib), "16 GB");
        assert_eq!(fmt_amount(0, QuotaUnit::Mib), "0 B");
        assert_eq!(fmt_amount(250, QuotaUnit::Gib), "250 GB");
    }
}
