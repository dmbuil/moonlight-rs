//! Project › Quotas (roadmap 82): every quota each service reports for the
//! current project, with usage, and *Edit Quotas* per service. Unlike Horizon
//! (Overview charts plus Identity › Projects › *Modify Quotas*), it is one
//! panel for every user; the cloud's policy decides who may save, unless
//! `[ui] quota_edit` (roadmap 112) hides *Edit Quotas* from members or everyone.
//! The same cards serve Identity › Projects › *Modify Quotas* for any project
//! (admins), where that setting does not apply.

use leptos::prelude::*;

use crate::api::{list_project_quotas, list_quotas};
use crate::components::dialog::{caps, data_version, prefs_tracked, use_dialogs};
use crate::components::ops::{edit_quotas_dialog, quota_label, quota_service_label};
use crate::components::table::TimingChip;
use crate::components::widgets::{ErrorBox, PageHeader, Skeleton, render};
use crate::i18n::{K, use_i18n};
use crate::model::{QuotaCard, QuotaRow, QuotaService, needs};
use crate::pages::details::param;
use crate::pages::overview::fmt_amount;
use leptos_meta::Title;
use leptos_router::components::A;

/// Percentage used, 0..=100; `None` without usage or limit (nothing to draw).
fn percent(r: &QuotaRow) -> Option<f64> {
    match (r.used, r.limit) {
        (Some(u), Some(l)) if l > 0 => Some((u as f64 / l as f64 * 100.0).clamp(0.0, 100.0)),
        (Some(u), Some(_)) => Some(if u > 0 { 100.0 } else { 0.0 }),
        _ => None,
    }
}

/// The Overview donuts' thresholds: 70 % high, 90 % full.
fn level(p: f64) -> &'static str {
    if p >= 90.0 {
        "full"
    } else if p >= 70.0 {
        "high"
    } else {
        "normal"
    }
}

#[component]
pub fn QuotasPage() -> impl IntoView {
    let data = Resource::new(data_version, |_| list_quotas());
    view! {
        <PageHeader>
            <Transition fallback=|| ()>
                {move || data.get().and_then(Result::ok).map(|t| view! { <TimingChip millis=t.millis calls=t.calls/> })}
            </Transition>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton rows=4/> }>
            {move || render(data.get(), |t| view! { <QuotaCards cards=t.data own=true/> })}
        </Transition>
    }
}

/// Identity › Projects › a project › *Quotas* (roadmap 82, Horizon's *Modify
/// Quotas*): another project's cards, each with *Edit Quotas* for that project.
/// Admins only: the menu entry needs the role and the server checks it again.
#[component]
pub fn IdentityProjectQuotasPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(i, _)| list_project_quotas(i));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=4/> }>
            {move || render(data.get(), move |t| {
                let p = t.data;
                let project_href = format!("/identity/projects/{}", p.project_id);
                let name = p.project_name.clone();
                let (crumb, title) = (name.clone(), format!("{} · {}", i18n.t(K::Quotas), name));
                view! {
                    <Title text=title/>
                    <nav class="breadcrumb" aria-label="Breadcrumb">
                        <span>{move || i18n.t(K::Identity)}</span>
                        <span class="sep" aria-hidden="true">"/"</span>
                        <A href="/identity/projects">{move || i18n.t(K::Projects)}</A>
                        <span class="sep" aria-hidden="true">"/"</span>
                        <A href=project_href>{crumb}</A>
                        <span class="sep" aria-hidden="true">"/"</span>
                        <span>{move || i18n.t(K::Quotas)}</span>
                    </nav>
                    <div class="page-header">
                        <h1><span class="muted">{move || format!("{}: ", i18n.t(K::Quotas))}</span>{name}</h1>
                        <div class="page-actions"><TimingChip millis=t.millis calls=t.calls/></div>
                    </div>
                    <QuotaCards cards=p.cards own=false/>
                }
            })}
        </Transition>
    }
}

/// One card per service. `own`: the session's project (Project › Quotas), whose
/// *Edit Quotas* follows `[ui] quota_edit`; another project's are for admins,
/// whom the menu and the server already check.
#[component]
fn QuotaCards(cards: Vec<QuotaCard>, own: bool) -> impl IntoView {
    view! {
        <div class="quota-cards">
            {cards.into_iter().map(|c| view! { <ServiceCard card=c own=own/> }).collect_view()}
        </div>
    }
}

#[component]
fn ServiceCard(card: QuotaCard, own: bool) -> impl IntoView {
    let i18n = use_i18n();
    let dialogs = use_dialogs();
    let service = card.service;
    let title = quota_service_label(service);
    let reserved = card.rows.iter().any(|r| r.reserved.is_some_and(|n| n != 0));
    // The server refuses the same (`api::quota_edit_gate`): hiding is only for the eye.
    let allowed = move || !own || prefs_tracked().quota_edit.allows(needs::ADMIN.met(&caps()));
    let has_keys = card.rows.iter().any(|r| service.keys().contains(&r.key.as_str()));
    let error = card.error.clone();
    let rows = card.rows.clone();
    // Barbican reports no usage: `fetch::quotas` counts the lists, which skip
    // secrets with project access off (roadmap 82). For another project it
    // counts nothing (no row has a *Used*), and unknown defaults are left out.
    let other = card.rows.iter().all(|r| r.used.is_none());
    let note_key = if other {
        K::KeyManagerOtherProjectNote
    } else {
        K::KeyManagerUsageNote
    };
    let note = (service == QuotaService::KeyManager)
        .then(|| view! { <p class="muted quota-note">{move || i18n.t(note_key)}</p> });
    let card = StoredValue::new(card);
    let edit = move |_| dialogs.open(edit_quotas_dialog(i18n.lang(), &card.get_value()));
    view! {
        <section class="card quota-card">
            <header class="quota-card-head">
                <h2>{move || i18n.t(title)}</h2>
                {move || (has_keys && allowed()).then(|| view! { <button type="button" class="btn" on:click=edit>{move || i18n.t(K::EditQuotas)}</button> })}
            </header>
            {error.map(|e| view! { <ErrorBox message=e/> })}
            <div class="table-wrap">
                <table class="table quota-table">
                    <thead><tr>
                        <th scope="col">{move || i18n.t(K::QuotaCol)}</th>
                        <th scope="col" class="num">{move || i18n.t(K::QuotaUsed)}</th>
                        {reserved.then(|| view! { <th scope="col" class="num">{move || i18n.t(K::QuotaReserved)}</th> })}
                        <th scope="col" class="num">{move || i18n.t(K::QuotaLimit)}</th>
                        <th scope="col" class="quota-bar-col"><span class="sr-only">"%"</span></th>
                    </tr></thead>
                    <tbody>
                        {rows.into_iter().map(|r| row(service, r, reserved)).collect_view()}
                    </tbody>
                </table>
            </div>
            {note}
        </section>
    }
}

fn row(service: QuotaService, r: QuotaRow, reserved: bool) -> impl IntoView {
    let i18n = use_i18n();
    let unit = service.unit(&r.key);
    let amount = move |v: Option<i64>| v.map_or_else(|| "-".to_string(), |n| fmt_amount(n, unit));
    let used = amount(r.used);
    let res = amount(r.reserved);
    let limit = r.limit.map(|l| fmt_amount(l, unit));
    let p = percent(&r);
    let key = r.key.clone();
    // A per-type or unknown key: show the API's name too, so it can be looked up.
    let mono = (!service.keys().contains(&r.key.as_str()))
        .then(|| view! { <span class="quota-key mono">{key.clone()}</span> });
    let key = r.key.clone();
    view! {
        <tr>
            <td>{move || quota_label(i18n.lang(), service, &key)} {mono}</td>
            <td class="num">{used}</td>
            {reserved.then(|| view! { <td class="num">{res}</td> })}
            <td class="num">{move || limit.clone().unwrap_or_else(|| i18n.t(K::Unlimited).to_string())}</td>
            <td class="quota-bar-col">
                {p.map(|p| view! {
                    <div class=format!("quota-bar {}", level(p)) role="img" aria-label=format!("{p:.0}%") title=format!("{p:.0}%")>
                        <span style=format!("width: {p:.1}%")></span>
                    </div>
                })}
            </td>
        </tr>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_and_level() {
        let r = |used, limit| QuotaRow {
            key: "k".into(),
            used,
            reserved: None,
            limit,
        };
        assert_eq!(percent(&r(Some(5), Some(10))), Some(50.0));
        assert_eq!(percent(&r(Some(12), Some(10))), Some(100.0), "over quota is full");
        assert_eq!(percent(&r(Some(0), Some(0))), Some(0.0));
        assert_eq!(percent(&r(Some(1), Some(0))), Some(100.0));
        assert_eq!(percent(&r(Some(5), None)), None, "unlimited");
        assert_eq!(percent(&r(None, Some(10))), None, "no usage reported (Octavia)");
        assert_eq!((level(69.9), level(70.0), level(90.0)), ("normal", "high", "full"));
    }
}
