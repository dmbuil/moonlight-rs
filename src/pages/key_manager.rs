//! Key Manager (Barbican, roadmap 55): secret and container detail pages, and
//! the expiration cell the lists share. The lists are in `lists.rs`. Secret
//! payloads are only fetched on *Show Payload* (`[key_manager] allow_payload`)
//! and dropped (zeroed, `model::Hidden`) on *Hide Payload*.

use leptos::prelude::*;

use super::details::{mono, param, Card, DetailHeader, Kv};
use super::lists::{link, status_pill};
use crate::api::*;
use crate::components::dialog::data_version;
use crate::components::ops;
use crate::components::table::{col, Col, DataTable};
use crate::components::widgets::{dash, render, utc_time, Copyable, ErrorBox, Pill, Skeleton, Tone};
use crate::i18n::{key_manager_term, t, use_i18n, Lang, K};
use crate::model::*;

/// A secret's expiration (UTC): *Never*, or the date, flagged when it is
/// within 30 days (warning) or past (error).
pub fn expiry_cell(l: Lang, expiration: Option<&str>) -> AnyView {
    let Some(e) = expiration else { return t(l, K::Never).into_any() };
    let text = utc_time(e);
    match expiry(Some(e), chrono::Utc::now().naive_utc()) {
        Expiry::Past => view! { <Pill tone=Tone::Err label=format!("{} · {text}", t(l, K::Expired))/> }.into_any(),
        Expiry::Soon => view! { <span title=t(l, K::ExpiresSoon)><Pill tone=Tone::Warn label=text/></span> }.into_any(),
        _ => text.into_any(),
    }
}

/// `aes-256-cbc`-style summary of a secret's algorithm, bit length and mode.
pub fn cipher(x: &SecretRow) -> String {
    let parts: Vec<String> = [x.algorithm.clone(), x.bit_length.map(|b| b.to_string()), x.mode.clone()].into_iter().flatten().collect();
    if parts.is_empty() { "-".into() } else { parts.join(" / ") }
}

fn secret_consumer_columns() -> Vec<Col<SecretConsumer>> {
    vec![
        col(K::Service, |c: &SecretConsumer, _| c.service.clone()).sort_text(|c| c.service.clone()),
        col(K::ResourceType, |c: &SecretConsumer, _| c.resource_type.clone()),
        col(K::ResourceId, |c: &SecretConsumer, _| mono(c.resource_id.clone())),
    ]
}

fn container_secret_columns() -> Vec<Col<ContainerSecret>> {
    vec![
        col(K::Role, |s: &ContainerSecret, l| key_manager_term(l, &s.role)).sort_text(|s| s.role.clone()),
        col(K::Secret, |s: &ContainerSecret, _| link(format!("/project/key_manager/secrets/{}", s.id), s.name.clone())).sort_text(|s| s.name.clone()),
    ]
}

fn container_consumer_columns() -> Vec<Col<ContainerConsumer>> {
    vec![
        col(K::Name, |k: &ContainerConsumer, _| k.name.clone()).sort_text(|k| k.name.clone()),
        col(K::Url, |k: &ContainerConsumer, _| mono(k.url.clone())),
    ]
}

#[component]
pub fn SecretDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_secret(id));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let whole = d.clone();
                let items = Signal::derive(move || ops::secret_detail_items(i18n.lang(), &whole));
                let name = d.secret.name.clone();
                view! {
                    <DetailHeader group=K::KeyManager panel=K::Secrets href="/project/key_manager/secrets"
                        name=name timing=(td.millis, td.calls) items=items/>
                    <SecretCards d=d/>
                }
            })}
        </Transition>
    }
}

/// The secret's cards, payload card and consumers.
#[component]
fn SecretCards(d: SecretDetail) -> AnyView {
    let i18n = use_i18n();
    let x = d.secret;
    let (status, kind, exp) = (x.status.clone(), x.secret_type.clone(), x.expiration.clone());
    let updated = dash(x.updated.as_deref().map(utc_time));
    let bits = x.bit_length.map(|b| b.to_string()).unwrap_or_else(|| "-".into());
    let creator = x.creator_id.clone().unwrap_or_else(|| "-".into());
    let acl = d.acl.clone();
    let acl_octavia = d.acl.octavia.clone();
    let payload = d.payload_allowed.then(|| view! { <PayloadCard id=x.id.clone()/> });
    view! {
        <div class="cards">
            <Card title=K::Information>
                <Kv label=K::Name><Copyable value=x.name.clone()/></Kv>
                <Kv label=K::Id><Copyable value=x.id.clone() mono=true/></Kv>
                <Kv label=K::SecretType>{move || key_manager_term(i18n.lang(), &kind)}</Kv>
                <Kv label=K::Status>{move || status_pill(&status, i18n.lang())}</Kv>
                <Kv label=K::SecretRef><Copyable value=x.secret_ref.clone() mono=true/></Kv>
                <Kv label=K::ContentType>{dash(x.content_type.clone())}</Kv>
                <Kv label=K::Algorithm>{dash(x.algorithm.clone())}</Kv>
                <Kv label=K::BitLength>{bits}</Kv>
                <Kv label=K::CipherMode>{dash(x.mode.clone())}</Kv>
            </Card>
            <Card title=K::AccessControl>
                <Kv label=K::Created>{utc_time(&x.created)}</Kv>
                <Kv label=K::Updated>{updated}</Kv>
                <Kv label=K::Expiration>{move || expiry_cell(i18n.lang(), exp.as_deref())}</Kv>
                <Kv label=K::CreatorId>{mono(creator)}</Kv>
                <AclRows acl=acl/>
            </Card>
            {payload}
        </div>
        <h2 class="section-title">{move || i18n.t(K::Consumers)}</h2>
        <SecretConsumers consumers=d.consumers octavia=!acl_octavia.is_empty()/>
        <OctaviaUses uses=acl_octavia/>
    }
    .into_any()
}

/// *Project members can read it* (with a *Private* badge when not) and the users
/// with read access, by name where Keystone allowed it, else by ID (roadmap 75).
#[component]
fn AclRows(acl: KmAcl) -> impl IntoView {
    let i18n = use_i18n();
    let project_access = acl.project_access;
    let users = if acl.users.is_empty() {
        view! { "-" }.into_any()
    } else {
        acl.users
            .iter()
            .map(|id| match acl.user_names.iter().find(|(i, _)| i == id) {
                Some((_, n)) => view! { <div>{n.clone()}" "<span class="mono muted">{id.clone()}</span></div> }.into_any(),
                None => view! { <div class="mono">{id.clone()}</div> }.into_any(),
            })
            .collect_view()
            .into_any()
    };
    let private = move || (project_access == Some(false)).then(|| view! { " " <Pill tone=Tone::Warn label=t(i18n.lang(), K::AccessPrivate)/> });
    view! {
        <Kv label=K::ProjectAccess>{move || yes_no(i18n.lang(), project_access)}{private}</Kv>
        <Kv label=K::ReadUsers>{users}</Kv>
    }
}

fn yes_no(l: Lang, v: Option<bool>) -> &'static str {
    match v {
        Some(true) => t(l, K::Yes),
        Some(false) => t(l, K::No),
        None => "-",
    }
}

/// `None`: this Barbican has no consumers API.
#[component]
fn SecretConsumers(consumers: Option<Vec<SecretConsumer>>, octavia: bool) -> AnyView {
    let i18n = use_i18n();
    match consumers {
        None => view! { <p class="muted">{move || i18n.t(K::ConsumersUnsupported)}</p> }.into_any(),
        // "No service…" would contradict the load balancers listed below.
        Some(c) if c.is_empty() && octavia => ().into_any(),
        Some(c) if c.is_empty() => view! { <p class="muted">{move || i18n.t(K::NoConsumers)}</p> }.into_any(),
        Some(c) => {
            let columns = secret_consumer_columns();
            view! { <DataTable rows=c columns=columns search=|c: &SecretConsumer| format!("{} {} {}", c.service, c.resource_type, c.resource_id)/> }.into_any()
        }
    }
}

fn octavia_columns(via: bool) -> Vec<Col<OctaviaUse>> {
    let mut cols = vec![
        col(K::LoadBalancer, |u: &OctaviaUse, _| match &u.lb_id {
            Some(id) => link(format!("/project/load_balancers/{id}"), u.lb_name.clone()),
            None => "-".into_any(),
        })
        .sort_text(|u| u.lb_name.clone()),
        col(K::Type, |u: &OctaviaUse, l| t(l, if u.kind == "pool" { K::Pool } else { K::Listener }).into_any()).sort_text(|u| u.kind.clone()),
        col(K::Name, |u: &OctaviaUse, _| u.name.clone().into_any()).sort_text(|u| u.name.clone()),
    ];
    if via {
        cols.push(col(K::ViaContainer, |u: &OctaviaUse, _| match &u.via {
            Some((id, name)) => link(format!("/project/key_manager/containers/{id}"), name.clone()),
            None => "-".into_any(),
        }));
    }
    cols
}

/// The Octavia listeners and pools using a secret or container (roadmap 75);
/// nothing when there are none. Barbican's consumer list misses them: Octavia
/// never registers there.
#[component]
fn OctaviaUses(uses: Vec<OctaviaUse>) -> AnyView {
    let i18n = use_i18n();
    if uses.is_empty() {
        return ().into_any();
    }
    let columns = octavia_columns(uses.iter().any(|u| u.via.is_some()));
    view! {
        <h3 class="subsection-title">{move || i18n.t(K::UsedByLoadBalancers)}</h3>
        <DataTable rows=uses columns=columns search=|u: &OctaviaUse| format!("{} {} {}", u.lb_name, u.kind, u.name)/>
    }
    .into_any()
}

/// Where *Show Payload* stands.
#[derive(Clone)]
enum Reveal {
    Closed,
    /// The confirmation modal is open.
    Confirm,
    Loading,
    Shown(SecretPayload),
    Failed(AppError),
}

/// *Show Payload* / *Hide Payload*: asks for confirmation, then fetches the
/// payload (never cached); hiding drops it (the `Hidden` value is zeroed).
#[component]
fn PayloadCard(id: String) -> impl IntoView {
    let i18n = use_i18n();
    let state = RwSignal::new(Reveal::Closed);
    let id = StoredValue::new(id);
    let fetch = move || {
        let id = id.get_value();
        state.set(Reveal::Loading);
        leptos::task::spawn_local(async move {
            let next = match get_secret_payload(id).await {
                Ok(p) => Reveal::Shown(p),
                Err(e) => Reveal::Failed(e),
            };
            // The page may be gone by now (AGENTS.md §6: disposed signals).
            let _ = state.try_set(next);
        });
    };
    let hide = move || state.set(Reveal::Closed);
    view! {
        <section class="card">
            <h2>{move || i18n.t(K::Payload)}</h2>
            <div class="payload">{move || match state.get() {
                Reveal::Closed | Reveal::Confirm => view! {
                    <button type="button" class="btn" on:click=move |_| state.set(Reveal::Confirm)>{move || i18n.t(K::ShowPayload)}</button>
                }.into_any(),
                Reveal::Loading => view! { <button type="button" class="btn" disabled=true>{move || i18n.t(K::ShowPayload)}</button> }.into_any(),
                Reveal::Failed(e) => view! { <ErrorBox message=e.detail/> }.into_any(),
                Reveal::Shown(p) => payload_view(p, hide),
            }}</div>
        </section>
        {move || matches!(state.get(), Reveal::Confirm).then(|| confirm_modal(move || state.set(Reveal::Closed), fetch))}
    }
}

/// The warning before the payload is shown: Cancel, or *Show Payload*.
fn confirm_modal(cancel: impl Fn() + Copy + Send + Sync + 'static, confirm: impl Fn() + Copy + Send + Sync + 'static) -> AnyView {
    let i18n = use_i18n();
    view! {
        <div class="modal-backdrop" on:click=move |_| cancel()>
            <div class="modal s" role="dialog" aria-modal="true" aria-label=move || i18n.t(K::ShowPayload)
                on:click=|ev| ev.stop_propagation()
                on:keydown=move |ev| if ev.key() == "Escape" { cancel() }>
                <header class="modal-head">
                    <h2>{move || i18n.t(K::ShowPayload)}</h2>
                    <button class="btn-close" type="button" aria-label=move || i18n.t(K::Close) on:click=move |_| cancel()>"×"</button>
                </header>
                <div class="modal-body">
                    <div class="alert alert-warn" role="alert">{move || i18n.t(K::ShowPayloadConfirm)}</div>
                    <footer class="modal-foot">
                        <button type="button" class="btn" on:click=move |_| cancel()>{move || i18n.t(K::Cancel)}</button>
                        <button type="button" class="btn btn-primary" autofocus=true on:click=move |_| confirm()>{move || i18n.t(K::ShowPayload)}</button>
                    </footer>
                </div>
            </div>
        </div>
    }
    .into_any()
}

/// The payload, its content type and *Hide Payload*.
fn payload_view(p: SecretPayload, hide: impl Fn() + Copy + Send + Sync + 'static) -> AnyView {
    let i18n = use_i18n();
    let note = p.base64.then(|| view! { <p class="muted">{move || i18n.t(K::PayloadBase64Note)}</p> });
    let (content_type, value) = (p.content_type.clone(), p.payload.0.clone());
    view! {
        <p class="muted">{move || i18n.t(K::PayloadWarning)}</p>
        {note}
        <p class="muted">{move || i18n.t(K::ContentType)} ": " {content_type.clone()}</p>
        <Copyable value=value mono=true/>
        <button type="button" class="btn" on:click=move |_| hide()>{move || i18n.t(K::HidePayload)}</button>
    }
    .into_any()
}

#[component]
pub fn ContainerDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_container(id));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let whole = d.clone();
                let items = Signal::derive(move || ops::container_detail_items(i18n.lang(), &whole));
                let name = d.container.name.clone();
                view! {
                    <DetailHeader group=K::KeyManager panel=K::Containers href="/project/key_manager/containers"
                        name=name timing=(td.millis, td.calls) items=items/>
                    <ContainerCards c=d.container acl=d.acl/>
                }
            })}
        </Transition>
    }
}

/// The container's cards (with its access, roadmap 75), its secrets by role and
/// its consumers.
#[component]
fn ContainerCards(c: ContainerRow, acl: KmAcl) -> AnyView {
    let i18n = use_i18n();
    let (status, kind) = (c.status.clone(), c.container_type.clone());
    let updated = dash(c.updated.as_deref().map(utc_time));
    let creator = c.creator_id.clone().unwrap_or_else(|| "-".into());
    let secret_cols = container_secret_columns();
    let octavia = acl.octavia.clone();
    let consumers = if c.consumers.is_empty() && !octavia.is_empty() {
        ().into_any()
    } else if c.consumers.is_empty() {
        view! { <p class="muted">{move || i18n.t(K::NoConsumers)}</p> }.into_any()
    } else {
        let cols = container_consumer_columns();
        view! { <DataTable rows=c.consumers.clone() columns=cols search=|k: &ContainerConsumer| format!("{} {}", k.name, k.url)/> }.into_any()
    };
    view! {
        <div class="cards">
            <Card title=K::Information>
                <Kv label=K::Name><Copyable value=c.name.clone()/></Kv>
                <Kv label=K::Id><Copyable value=c.id.clone() mono=true/></Kv>
                <Kv label=K::ContainerType>{move || key_manager_term(i18n.lang(), &kind)}</Kv>
                <Kv label=K::Status>{move || status_pill(&status, i18n.lang())}</Kv>
                <Kv label=K::ContainerRef><Copyable value=c.container_ref.clone() mono=true/></Kv>
                <Kv label=K::Created>{utc_time(&c.created)}</Kv>
                <Kv label=K::Updated>{updated}</Kv>
                <Kv label=K::CreatorId>{mono(creator)}</Kv>
            </Card>
            <Card title=K::AccessControl>
                <AclRows acl=acl/>
            </Card>
        </div>
        <h2 class="section-title">{move || i18n.t(K::Secrets)}</h2>
        <DataTable rows=c.secrets columns=secret_cols search=|s: &ContainerSecret| format!("{} {} {}", s.role, s.name, s.id)/>
        <h2 class="section-title">{move || i18n.t(K::Consumers)}</h2>
        {consumers}
        <OctaviaUses uses=octavia/>
    }
    .into_any()
}
