use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_location;

use super::dialog::Dialogs;
use super::nav;
use crate::i18n::{K, use_i18n};
use crate::model::{AppError, ErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Off,
    Warn,
    Err,
    Info,
}

impl Tone {
    fn class(self) -> &'static str {
        match self {
            Tone::Ok => "pill ok",
            Tone::Off => "pill off",
            Tone::Warn => "pill warn",
            Tone::Err => "pill err",
            Tone::Info => "pill info",
        }
    }

    /// The tone's CSS class without `pill`, for the topology drawings.
    pub fn key(self) -> &'static str {
        &self.class()[5..]
    }

    /// Tone for Nova server statuses.
    pub fn server(status: &str) -> Tone {
        match status.to_ascii_uppercase().as_str() {
            "ACTIVE" => Tone::Ok,
            "ERROR" => Tone::Err,
            "SHUTOFF" | "SHELVED" | "SHELVED_OFFLOADED" | "DELETED" | "SOFT_DELETED" => Tone::Off,
            "PAUSED" | "SUSPENDED" | "RESCUE" => Tone::Info,
            _ => Tone::Warn, // BUILD, REBOOT, RESIZE, MIGRATING…
        }
    }

    /// Tone for other resources (volumes, networks, images, floating IPs…).
    pub fn resource(status: &str) -> Tone {
        match status.to_ascii_lowercase().as_str() {
            "active" | "available" | "in-use" | "online" => Tone::Ok,
            "down" | "deactivated" | "offline" | "no_monitor" | "inactive" => Tone::Off,
            s if s.contains("error") || s == "killed" => Tone::Err,
            _ => Tone::Warn,
        }
    }
}

#[component]
pub fn Pill(tone: Tone, #[prop(into)] label: String) -> impl IntoView {
    view! {
        <span class=tone.class()>
            <span class="pill-dot" aria-hidden="true"></span>
            {label}
        </span>
    }
}

/// Breadcrumb + page title, derived from the navigation model.
#[component]
pub fn PageHeader(
    #[prop(optional, into)] title: Option<Signal<String>>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let i18n = use_i18n();
    let location = use_location();
    let crumbs = move || {
        let path = location.pathname.get();
        nav::active(&path)
    };
    let title = move || match &title {
        Some(t) => t.get(),
        None => crumbs().map(|(_, p)| i18n.t(p.label).to_string()).unwrap_or_default(),
    };
    view! {
        <Title text=title/>
        <nav class="breadcrumb" aria-label="Breadcrumb">
            <span>{move || i18n.t(nav::dashboard(&location.pathname.get()).0)}</span>
            {move || crumbs().map(|(g, p)| {
                // Identity has one group named like the dashboard: *Identity › Projects*, as Horizon.
                let group = (g.label != nav::dashboard(&location.pathname.get()).0).then(|| view! {
                    <span class="sep" aria-hidden="true">"/"</span>
                    <span>{i18n.t(g.label)}</span>
                });
                view! {
                    {group}
                    <span class="sep" aria-hidden="true">"/"</span>
                    <a href=p.href>{i18n.t(p.label)}</a>
                }
            })}
        </nav>
        <div class="page-header">
            <h1>{title}</h1>
            <div class="page-actions">{children.map(|c| c())}</div>
        </div>
    }
}

#[component]
pub fn ErrorBox(#[prop(into)] message: String) -> impl IntoView {
    let i18n = use_i18n();
    let unauth = message.contains("unauthenticated");
    view! {
        <div class="alert alert-error" role="alert">
            <strong>{move || i18n.t(K::ErrorLoading)}</strong>
            <code>{message.clone()}</code>
            {unauth.then(|| view! { <a href="/login">{move || i18n.t(K::LoginTitle)}</a> })}
        </div>
    }
}

/// Placeholder rows while a table loads.
#[component]
pub fn Skeleton(#[prop(default = 6)] rows: usize) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="skeleton" aria-busy="true">
            <span class="sr-only">{move || i18n.t(K::Loading)}</span>
            {(0..rows).map(|_| view! { <div class="skeleton-row"></div> }).collect_view()}
        </div>
    }
}

/// `-` for empty values, like Horizon.
pub fn dash(v: Option<impl Into<String>>) -> String {
    v.map(Into::into)
        .filter(|s: &String| !s.is_empty())
        .unwrap_or_else(|| "-".into())
}

/// `data:` URL for a plain-text download link (`<a href=… download=…>`).
pub fn data_url(text: &str) -> String {
    let mut out = String::from("data:text/plain;charset=utf-8,");
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out += &format!("%{b:02X}");
        }
    }
    out
}

/// Renders a server function result, or the error page.
pub fn render<T, V>(r: Option<Result<T, AppError>>, f: impl FnOnce(T) -> V) -> AnyView
where
    V: IntoView + 'static,
{
    match r {
        None => view! { <Skeleton/> }.into_any(),
        Some(Ok(v)) => f(v).into_any(),
        Some(Err(e)) => view! { <ErrorPage error=e/> }.into_any(),
    }
}

/// Title and explanation shown for each kind of error.
pub fn error_text(kind: ErrorKind) -> (K, K) {
    match kind {
        ErrorKind::Unauthenticated => (K::ErrUnauthTitle, K::ErrUnauthText),
        ErrorKind::Forbidden => (K::ErrForbiddenTitle, K::ErrForbiddenText),
        ErrorKind::NotFound => (K::ErrNotFoundTitle, K::ErrNotFoundText),
        ErrorKind::ServiceUnavailable => (K::ErrServiceTitle, K::ErrServiceText),
        ErrorKind::RegionUnavailable => (K::ErrRegionTitle, K::ErrRegionText),
        ErrorKind::Timeout => (K::ErrTimeoutTitle, K::ErrTimeoutText),
        ErrorKind::OpenStack(_) => (K::ErrOpenStackTitle, K::ErrOpenStackText),
        ErrorKind::Invalid => (K::ErrInvalidTitle, K::ErrInvalidText),
        ErrorKind::TooManyAttempts => (K::ErrTooManyTitle, K::LoginThrottled),
        ErrorKind::TooLarge => (K::ErrTooLargeTitle, K::ErrTooLargeText),
        ErrorKind::Internal => (K::ErrInternalTitle, K::ErrInternalText),
    }
}

/// Handle to the header's region menu, so the error page can open it, and
/// the user's other regions (filled after hydration; empty during SSR).
#[derive(Clone, Copy)]
pub struct RegionMenuRef {
    pub menu: NodeRef<leptos::html::Details>,
    pub others: RwSignal<Vec<String>>,
}

/// Errors that another region may not have: a missing service, a region gone
/// from the catalog, or one that does not answer.
pub fn region_may_help(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::ServiceUnavailable | ErrorKind::RegionUnavailable | ErrorKind::Timeout
    )
}

/// Page-level failure: what happened, in plain words, and a way out that
/// always works: *Retry* (when it may help) and *Back to safety*, which reloads
/// the Overview; when the region may be the cause, it first switches to another
/// region of the same project.
#[component]
pub fn ErrorPage(error: AppError) -> impl IntoView {
    let i18n = use_i18n();
    let (title, text) = error_text(error.kind);
    let sign_in = error.kind == ErrorKind::Unauthenticated;
    let dialogs = use_context::<Dialogs>().filter(|_| error.retryable() && !sign_in);
    let region_menu = use_context::<RegionMenuRef>().filter(|_| region_may_help(error.kind));
    // Another region of the same project, if the user has one.
    let escape = move || region_menu.and_then(|m| m.others.with(|o| o.first().cloned()));
    // The first contextual action is the primary button; else Back to safety.
    let contextual = sign_in || dialogs.is_some();
    let safety_class = move || {
        if contextual || escape().is_some() {
            "btn"
        } else {
            "btn btn-primary"
        }
    };
    let kind = match error.kind {
        ErrorKind::OpenStack(s) => format!("HTTP {s}"),
        k => format!("{k:?}"),
    };
    let AppError {
        detail,
        service,
        request_id,
        ..
    } = error;
    view! {
        <section class="error-page" role="alert">
            <svg class="error-icon" aria-hidden="true" viewBox="0 0 24 24" width="44" height="44" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                <path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z"></path>
                <path d="M12 9v4M12 17h.01"></path>
            </svg>
            <h2>{move || i18n.t(title)}</h2>
            <p class="error-text">{move || i18n.t(text)}</p>
            <div class="error-actions">
                {sign_in.then(|| view! {
                    <a class="btn btn-primary" href="/login" rel="external">{move || i18n.t(K::SignIn)}</a>
                })}
                {dialogs.map(|d| view! {
                    <button type="button" class="btn btn-primary" on:click=move |_| d.version.update(|v| *v += 1)>
                        {move || i18n.t(K::Retry)}
                    </button>
                })}
                {move || match escape() {
                    Some(region) => {
                        let label = region.clone();
                        view! {
                            <button type="button" class=safety_class on:click=move |_| {
                                let region = region.clone();
                                leptos::task::spawn_local(async move {
                                    // Whatever happens, end up on a fresh Overview.
                                    let _ = crate::api::switch_region(region).await;
                                    super::layout::hard_reload(Some("/project"));
                                });
                            }>
                                {move || i18n.t(K::BackToSafety)}" · "{label.clone()}
                            </button>
                        }.into_any()
                    }
                    None => view! {
                        <a class=safety_class href="/project" rel="external">{move || i18n.t(K::BackToSafety)}</a>
                    }.into_any(),
                }}
            </div>
            <details class="error-details">
                <summary>{move || i18n.t(K::TechnicalDetails)}</summary>
                <dl class="kv mini">
                    <dt>{move || i18n.t(K::ErrorKindLabel)}</dt>
                    <dd class="mono">{kind}</dd>
                    {service.map(|s| view! { <dt>{move || i18n.t(K::Service)}</dt><dd class="mono">{s}</dd> })}
                    {request_id.map(|r| view! { <dt>{move || i18n.t(K::RequestId)}</dt><dd><Copyable value=r mono=true/></dd> })}
                </dl>
                <pre class="error-raw">{detail}</pre>
            </details>
        </section>
    }
}

/// Age since an RFC 3339 timestamp, localised.
pub fn age_since(lang: crate::i18n::Lang, rfc3339: &str) -> String {
    seconds_since(rfc3339).map_or_else(|| "-".into(), |s| crate::i18n::age(lang, s))
}

/// Seconds from an RFC 3339 timestamp to now; `None` if it doesn't parse.
fn seconds_since(rfc3339: &str) -> Option<i64> {
    let t = chrono::DateTime::parse_from_rfc3339(rfc3339).ok()?;
    Some((chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds())
}

/// An age for a table cell: each unit in a `nowrap` span, the comma kept with
/// the first, so a narrow column breaks between `3 semanas,` and `2 días` and
/// never inside a unit (roadmap 72: the Spanish Instances table overflowed).
pub fn age_cell(lang: crate::i18n::Lang, seconds: i64) -> AnyView {
    let parts = crate::i18n::age_parts(lang, seconds);
    let last = parts.len() - 1;
    parts
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let text = if i < last { format!("{p},") } else { p };
            view! { <span class="nowrap">{text}</span>{(i < last).then_some(" ")} }
        })
        .collect_view()
        .into_any()
}

/// [`age_cell`] since an RFC 3339 timestamp; `-` if it doesn't parse.
pub fn age_cell_since(lang: crate::i18n::Lang, rfc3339: &str) -> AnyView {
    seconds_since(rfc3339).map_or_else(|| "-".into_any(), |s| age_cell(lang, s))
}

/// `short_time` plus ` UTC` for values in UTC (Keystone's expiries have no
/// zone; they are UTC), so a reader in another zone doesn't take them for
/// local time (roadmap 57).
pub fn utc_time(v: &str) -> String {
    let utc = v.ends_with('Z')
        || v.ends_with("+00:00")
        || chrono::NaiveDateTime::parse_from_str(v, "%Y-%m-%dT%H:%M:%S%.f").is_ok();
    if utc {
        format!("{} UTC", short_time(v))
    } else {
        short_time(v)
    }
}

/// Short, readable timestamp: `2026-09-14 10:22`.
pub fn short_time(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| rfc3339.replace('T', " ").chars().take(16).collect())
}

// ---------------------------------------------------------------------------
// Availability-zone pills, lock icon, flavor popover
// ---------------------------------------------------------------------------

/// Outline colours for AZ pills: 12 well-separated hues as (light, dark theme)
/// pairs, each ≥ 4.5:1 on its theme's background (text and 1px border share the
/// colour; the zone name is always printed).
const AZ_PALETTE: [(&str, &str); 12] = [
    ("#1d4ed8", "#60a5fa"), // blue
    ("#15803d", "#4ade80"), // green
    ("#b91c1c", "#f87171"), // red
    ("#7e22ce", "#c084fc"), // purple
    ("#c2410c", "#fb923c"), // orange
    ("#0f766e", "#2dd4bf"), // teal
    ("#be185d", "#f472b6"), // pink
    ("#a16207", "#facc15"), // amber
    ("#4338ca", "#a5b4fc"), // indigo
    ("#0e7490", "#22d3ee"), // cyan
    ("#4d7c0f", "#a3e635"), // lime
    ("#475569", "#94a3b8"), // slate
];

/// Stable (light, dark) colours for an AZ name (FNV-1a), identical on every page and render.
pub fn az_color(name: &str) -> (&'static str, &'static str) {
    let mut h: u32 = 0x811c9dc5;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    AZ_PALETTE[(h % AZ_PALETTE.len() as u32) as usize]
}

#[component]
pub fn AzPill(name: Option<String>) -> impl IntoView {
    match name.filter(|n| !n.is_empty()) {
        None => view! { <span>"-"</span> }.into_any(),
        Some(n) => {
            // Outline only: transparent background, 1px border + text in the zone
            // colour; the stylesheet picks the variant for the active theme.
            let (light, dark) = az_color(&n);
            view! { <span class="az-pill" style=format!("--az:{light};--az-dark:{dark}")>{n}</span> }.into_any()
        }
    }
}

/// One [`AzPill`] per zone (Neutron's `availability_zones`, an aggregate's
/// hosts' zone list); `-` when there are none.
#[component]
pub fn AzPills(names: Vec<String>) -> impl IntoView {
    if names.is_empty() {
        return view! { <span>"-"</span> }.into_any();
    }
    view! { <span class="az-pills">{names.into_iter().map(|n| view! { <AzPill name=Some(n)/> " " }).collect_view()}</span> }.into_any()
}

/// Horizon-style padlock next to an instance name.
#[component]
pub fn LockIcon(locked: bool) -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let label = move || i18n.t(if locked { K::Locked } else { K::Unlocked });
    view! {
        <span class=if locked { "lock locked" } else { "lock" } title=label role="img" aria-label=label>
            <svg aria-hidden="true" viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round">
                <rect x="5" y="11" width="14" height="10" rx="2"></rect>
                {if locked {
                    view! { <path d="M8 11V8a4 4 0 0 1 8 0v3"></path> }.into_any()
                } else {
                    view! { <path d="M8 11V8a4 4 0 0 1 7.5-2"></path> }.into_any()
                }}
            </svg>
        </span>
    }
}

/// Widest a [`FlavorPopover`] card gets (`.popover`'s `max-width`), px.
const POPOVER_W: f64 = 360.0;
/// Widest a [`SpecsPopover`] card gets (`.popover.specs-card`), px: long spec
/// keys need more room than the flavor card (roadmap 109).
const SPECS_POPOVER_W: f64 = 460.0;

/// The `style` of a hover card (`.popover`, `position: fixed`) at most `card_w`
/// px wide, for a trigger at `left`, `top`, `bottom` (viewport px) in a `vw` ×
/// `vh` viewport: under the trigger, or above it when fewer than 280 px are
/// left below, and kept inside the right edge. Fixed, so scrolling table
/// wrappers don't clip it (AGENTS.md §6).
pub fn popover_style(left: f64, top: f64, bottom: f64, vw: f64, vh: f64, card_w: f64) -> String {
    let left = left.min(vw - card_w - 20.0).max(8.0);
    if bottom + 280.0 > vh {
        format!("left:{left}px;top:{top}px;transform:translateY(calc(-100% - 8px))")
    } else {
        format!("left:{left}px;top:{}px", bottom + 8.0)
    }
}

/// Sets `pos` to [`popover_style`] for `host`'s current rect (a no-op on the server).
fn place_popover(host: NodeRef<leptos::html::Span>, pos: RwSignal<String>, card_w: f64) {
    #[cfg(feature = "hydrate")]
    if let Some(el) = host.get_untracked() {
        let r = el.get_bounding_client_rect();
        let vh = window().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
        let vw = window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1200.0);
        // `try_set`: a hover can arrive after navigating away disposed the signal (AGENTS.md §6).
        let _ = pos.try_set(popover_style(r.left(), r.top(), r.bottom(), vw, vh, card_w));
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (host, pos, card_w);
}

/// `label` (Admin › Flavors' *Yes* in the *Metadata* column, roadmap 109) that
/// shows `specs` as `key` / `value` pairs in a card on hover and keyboard
/// focus, titled *title: name*. `key` makes the card's ID unique on the page.
/// With no `specs` it is the plain `label`, with no card.
#[component]
pub fn SpecsPopover(
    #[prop(into)] label: String,
    #[prop(into)] name: String,
    specs: Vec<(String, String)>,
    title: K,
    #[prop(into)] key: String,
) -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    if specs.is_empty() {
        return label.into_any();
    }
    let id = format!("specs-{key}");
    let host = NodeRef::<leptos::html::Span>::new();
    let pos = RwSignal::new(String::new());
    let place = move || place_popover(host, pos, SPECS_POPOVER_W);
    let host_id = id.clone();
    view! {
        <span class="popover-host" tabindex="0" aria-describedby=host_id node_ref=host
            on:mouseenter=move |_| place() on:focusin=move |_| place()>
            <span>{label}</span>
            <span class="popover specs-card" role="tooltip" id=id style=move || pos.get()>
                <strong class="popover-title">{move || i18n.t(title)}": "{name.clone()}</strong>
                <dl class="kv mini specs">
                    {specs.into_iter().map(|(k, v)| view! { <dt class="mono small">{k}</dt><dd class="mono small">{v}</dd> }).collect_view()}
                </dl>
            </span>
        </span>
    }
    .into_any()
}

/// Flavor name with a details card shown on hover and keyboard focus.
#[component]
pub fn FlavorPopover(flavor: crate::model::FlavorInfo, #[prop(into)] key: String) -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let id = format!("flv-{key}");
    let f = flavor;
    let gb = |v: Option<i64>| v.map(|g| format!("{g} GB")).unwrap_or_else(|| "-".into());
    let ram = f
        .ram_mb
        .map(|m| crate::i18n::bytes(m.max(0) as u64 * 1024 * 1024))
        .unwrap_or_else(|| "-".into());
    let row = move |k: K, v: String| view! { <dt>{move || i18n.t(k)}</dt><dd>{v}</dd> };
    let host = NodeRef::<leptos::html::Span>::new();
    let pos = RwSignal::new(String::new());
    let place = move || place_popover(host, pos, POPOVER_W);
    let host_id = id.clone();
    view! {
        <span class="popover-host" tabindex="0" aria-describedby=host_id node_ref=host
            on:mouseenter=move |_| place() on:focusin=move |_| place()>
            <span class="flavor-name">{f.name.clone()}</span>
            <span class="popover" role="tooltip" id=id style=move || pos.get()>
                <strong class="popover-title">{move || i18n.t(K::FlavorDetails)}": "{f.name.clone()}</strong>
                <dl class="kv mini">
                    {row(K::Id, dash(Some(f.id.clone())))}
                    {row(K::Vcpus, f.vcpus.map(|v| v.to_string()).unwrap_or_else(|| "-".into()))}
                    {row(K::Ram, ram)}
                    {row(K::RootDisk, gb(f.disk_gb))}
                </dl>
            </span>
        </span>
    }
}

// ---------------------------------------------------------------------------
// Busy indicator (ongoing instance tasks)
// ---------------------------------------------------------------------------

/// Statuses Nova uses while an operation is in flight, even without a task_state.
const TRANSITIONAL: &[&str] = &[
    "BUILD",
    "REBOOT",
    "HARD_REBOOT",
    "RESIZE",
    "MIGRATING",
    "REBUILD",
    "PASSWORD",
];

/// True while the instance has an operation in progress.
pub fn is_busy(task_state: Option<&str>, status: &str) -> bool {
    task_state.is_some_and(|t| !t.is_empty()) || TRANSITIONAL.contains(&status.to_ascii_uppercase().as_str())
}

/// Cinder volume statuses that change on their own (roadmap 46): the page keeps
/// refetching while a row is in one. Lowercase, as Cinder reports them.
const VOLUME_TRANSITIONAL: &[&str] = &[
    "creating",
    "deleting",
    "extending",
    "attaching",
    "detaching",
    "downloading",
    "uploading",
    "retyping",
    "backing-up",
    "restoring-backup",
    "reserved",
    "awaiting-transfer",
    "maintenance",
];

/// Cinder snapshot statuses that change on their own (roadmap 46).
const SNAPSHOT_TRANSITIONAL: &[&str] = &[
    "creating",
    "deleting",
    "updating",
    "restoring",
    "backing-up",
    "unmanaging",
];

/// Whether a Glance image is still being created or imported (`queued` until its data
/// arrives, then `saving` / `importing`; roadmap 45). `killed`, `deleted` and the like are final.
pub fn image_busy(status: &str) -> bool {
    matches!(status.to_ascii_lowercase().as_str(), "queued" | "saving" | "importing")
}

/// True while a volume is in a transitional state (`creating`, `extending`…).
pub fn volume_busy(status: &str) -> bool {
    VOLUME_TRANSITIONAL.contains(&status.to_ascii_lowercase().as_str())
}

/// True while a volume snapshot is in a transitional state.
pub fn snapshot_busy(status: &str) -> bool {
    SNAPSHOT_TRANSITIONAL.contains(&status.to_ascii_lowercase().as_str())
}

/// Indeterminate progress bar (no `aria-valuenow` = indeterminate for screen readers).
#[component]
pub fn Busy(#[prop(into)] label: Signal<String>) -> impl IntoView {
    view! {
        <span class="busy" role="progressbar" aria-label=move || label.get() title=move || label.get()>
            <span class="busy-bar"></span>
        </span>
    }
}

/// While `busy()` is true, refetches `resource` every `[ui] poll_interval_ms`
/// (5 s by default; GET only), for at most 10 minutes. Client-only; the pending
/// timer is cleared on unmount.
pub fn poll_while_busy<T>(resource: Resource<T>, busy: impl Fn() -> bool + Send + Sync + 'static) -> RwSignal<bool>
where
    T: Clone + Send + Sync + 'static + serde::Serialize + serde::de::DeserializeOwned,
{
    let polling = RwSignal::new(false);
    #[cfg(feature = "hydrate")]
    {
        use std::sync::{Arc, Mutex};
        use std::time::Duration;
        let rounds = Arc::new(Mutex::new(0u32));
        let handle: Arc<Mutex<Option<TimeoutHandle>>> = Arc::new(Mutex::new(None));
        let h2 = handle.clone();
        on_cleanup(move || {
            if let Some(h) = h2.lock().ok().and_then(|mut g| g.take()) {
                h.clear();
            }
        });
        Effect::new(move |_| {
            // Track the resource so this re-runs after every (re)fetch.
            let loaded = resource.with(|r| r.is_some());
            let mut n = rounds.lock().unwrap();
            let p = crate::components::dialog::prefs();
            if loaded && busy() && *n < p.poll_rounds() {
                *n += 1;
                polling.set(true);
                if let Ok(h) = set_timeout_with_handle(
                    move || resource.refetch(),
                    Duration::from_millis(u64::from(p.poll_interval_ms)),
                ) && let Some(old) = handle.lock().unwrap().replace(h)
                {
                    old.clear();
                }
            } else if loaded {
                polling.set(false);
            }
        });
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (resource, busy);
    polling
}

// ---------------------------------------------------------------------------
// Copy to clipboard
// ---------------------------------------------------------------------------

/// Copies `_text` to the clipboard; best effort, silent when the browser refuses.
pub fn copy_to_clipboard(_text: &str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::{JsCast, JsValue};
        let nav = window().navigator();
        let clip = nav.clipboard();
        if !JsValue::from(clip.clone()).is_undefined() {
            let _ = clip.write_text(_text); // promise ignored: best effort
            return;
        }
        // Fallback for non-secure contexts: hidden textarea + execCommand.
        let Some(doc) = window().document() else { return };
        let Ok(el) = doc.create_element("textarea") else { return };
        let Ok(ta) = el.dyn_into::<web_sys::HtmlTextAreaElement>() else {
            return;
        };
        ta.set_value(_text);
        let _ = ta.set_attribute("style", "position:fixed;opacity:0");
        if let Some(body) = doc.body() {
            let _ = body.append_child(&ta);
            ta.select();
            if let Ok(hd) = doc.dyn_into::<web_sys::HtmlDocument>() {
                let _ = hd.exec_command("copy");
            }
            let _ = body.remove_child(&ta);
        }
    }
}

/// A value with a copy button; announces "Copied" politely for screen readers.
#[component]
pub fn Copyable(#[prop(into)] value: String, #[prop(optional)] mono: bool) -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let copied = RwSignal::new(false);
    let v = value.clone();
    view! {
        <span class="copyable">
            <span class=if mono { "mono" } else { "" }>{value.clone()}</span>
            <button type="button" class="btn-copy" title=move || i18n.t(K::Copy) aria-label=move || i18n.t(K::Copy)
                on:click=move |_| {
                    copy_to_clipboard(&v);
                    copied.set(true);
                    set_timeout(move || copied.set(false), std::time::Duration::from_millis(1500));
                }>
                {move || if copied.get() {
                    view! { <svg aria-hidden="true" viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.5"><path d="m5 12 5 5L20 7"></path></svg> }.into_any()
                } else {
                    view! { <svg aria-hidden="true" viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="11" height="11" rx="2"></rect><path d="M5 15V5a2 2 0 0 1 2-2h10"></path></svg> }.into_any()
                }}
            </button>
            <span class="sr-only" aria-live="polite">{move || if copied.get() { i18n.t(K::Copied) } else { "" }}</span>
        </span>
    }
}

// ---------------------------------------------------------------------------
// Info tips: an (i) beside a form label with a longer explanation (roadmap 79)
// ---------------------------------------------------------------------------

/// A paragraph or a bullet list of an [`InfoTip`]'s text.
#[derive(Clone, Debug, PartialEq)]
pub enum TipBlock {
    Para(String),
    List(Vec<String>),
}

/// Splits a tip's text into blocks: one paragraph per line, and consecutive
/// lines starting with `- ` into one list. Kept as plain text in the i18n
/// table so both languages share one format; each block's text may carry
/// [`tip_inline`] markup.
pub fn tip_blocks(text: &str) -> Vec<TipBlock> {
    let mut out: Vec<TipBlock> = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match (line.strip_prefix("- "), out.last_mut()) {
            (Some(item), Some(TipBlock::List(items))) => items.push(item.to_string()),
            (Some(item), _) => out.push(TipBlock::List(vec![item.to_string()])),
            (None, _) => out.push(TipBlock::Para(line.to_string())),
        }
    }
    out
}

/// Inline formatting of a tip's text.
#[derive(Clone, Debug, PartialEq)]
pub enum TipInline {
    Text(String),
    Bold(Vec<TipInline>),
    Italic(Vec<TipInline>),
    /// Monospace, never formatted inside: PromQL, JSON and wildcards (`compute.instance.*`) go here.
    Code(String),
}

/// A small Markdown subset: `` `code` ``, `**bold**` and `*italic*` (bold and
/// italic may contain code). A marker without its closing pair stays as text,
/// so a stray `*` (a wildcard outside code) never swallows the rest of the line.
pub fn tip_inline(text: &str) -> Vec<TipInline> {
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    let flush = |plain: &mut String, out: &mut Vec<TipInline>| {
        if !plain.is_empty() {
            out.push(TipInline::Text(std::mem::take(plain)));
        }
    };
    while let Some(c) = rest.chars().next() {
        let (marker, make): (&str, fn(&str) -> TipInline) = match c {
            '`' => ("`", |x| TipInline::Code(x.to_string())),
            '*' if rest.starts_with("**") => ("**", |x| TipInline::Bold(tip_inline(x))),
            '*' => ("*", |x| TipInline::Italic(tip_inline(x))),
            _ => {
                plain.push(c);
                rest = &rest[c.len_utf8()..];
                continue;
            }
        };
        let after = &rest[marker.len()..];
        match after.find(marker).filter(|&end| end > 0) {
            Some(end) => {
                flush(&mut plain, &mut out);
                out.push(make(&after[..end]));
                rest = &after[end + marker.len()..];
            }
            None => {
                plain.push_str(marker);
                rest = after;
            }
        }
    }
    flush(&mut plain, &mut out);
    out
}

fn tip_inline_view(parts: Vec<TipInline>) -> AnyView {
    parts
        .into_iter()
        .map(|p| match p {
            TipInline::Text(t) => t.into_any(),
            TipInline::Bold(inner) => view! { <strong>{tip_inline_view(inner)}</strong> }.into_any(),
            TipInline::Italic(inner) => view! { <em>{tip_inline_view(inner)}</em> }.into_any(),
            TipInline::Code(c) => view! { <code>{c}</code> }.into_any(),
        })
        .collect_view()
        .into_any()
}

/// An (i) icon that shows `text` ([`tip_blocks`] format) on hover and keyboard
/// focus. The card is `position: fixed`, placed from the icon's rect, so dialog
/// bodies and table wrappers that scroll don't clip it (like [`FlavorPopover`]).
#[component]
pub fn InfoTip(#[prop(into)] text: String, #[prop(into)] label: String, #[prop(into)] id: String) -> impl IntoView {
    let host = NodeRef::<leptos::html::Span>::new();
    let pos = RwSignal::new(String::new());
    let place = move || {
        #[cfg(feature = "hydrate")]
        if let Some(el) = host.get_untracked() {
            let r = el.get_bounding_client_rect();
            let vh = window().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
            let vw = window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1200.0);
            // Right of the icon when there is room, else under it, kept on screen.
            let left = (r.right() + 8.0).min(vw - 428.0).max(8.0);
            let top = if left > r.right() {
                r.top() - 8.0
            } else {
                r.bottom() + 6.0
            };
            let top = top.min(vh - 340.0).max(8.0);
            let _ = pos.try_set(format!("left:{left}px;top:{top}px"));
        }
    };
    let blocks = tip_blocks(&text)
        .into_iter()
        .map(|b| match b {
            TipBlock::Para(p) => view! { <p>{tip_inline_view(tip_inline(&p))}</p> }.into_any(),
            TipBlock::List(items) => view! { <ul>{items.into_iter().map(|i| view! { <li>{tip_inline_view(tip_inline(&i))}</li> }).collect_view()}</ul> }.into_any(),
        })
        .collect_view();
    let tip_id = format!("{id}-tip");
    let described = tip_id.clone();
    view! {
        <span class="popover-host info-tip" tabindex="0" role="button" aria-label=label aria-describedby=described node_ref=host
            on:mouseenter=move |_| place() on:focusin=move |_| place()>
            <svg aria-hidden="true" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                <circle cx="12" cy="12" r="9.5"></circle>
                <path d="M12 11v6"></path>
                <circle cx="12" cy="7.6" r="0.6" fill="currentColor"></circle>
            </svg>
            <span class="popover tip-card" role="tooltip" id=tip_id style=move || pos.get()>{blocks}</span>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popovers_open_below_or_above_and_stay_on_screen() {
        // Room below: under the trigger, 8 px down.
        assert_eq!(
            popover_style(100.0, 50.0, 70.0, 1200.0, 800.0, POPOVER_W),
            "left:100px;top:78px"
        );
        // Near the bottom: above it.
        assert_eq!(
            popover_style(100.0, 600.0, 620.0, 1200.0, 800.0, POPOVER_W),
            "left:100px;top:600px;transform:translateY(calc(-100% - 8px))"
        );
        // Near the right edge: pulled back by the card's width plus 20 px; never off the left edge.
        assert!(popover_style(1100.0, 50.0, 70.0, 1200.0, 800.0, POPOVER_W).starts_with("left:820px;"));
        assert!(popover_style(1100.0, 50.0, 70.0, 1200.0, 800.0, SPECS_POPOVER_W).starts_with("left:720px;"));
        assert!(popover_style(-20.0, 50.0, 70.0, 300.0, 800.0, POPOVER_W).starts_with("left:8px;"));
    }

    #[test]
    fn images_poll_until_their_data_is_in_and_stop_on_final_states() {
        for busy in ["queued", "saving", "importing", "Queued"] {
            assert!(image_busy(busy), "{busy}");
        }
        for done in ["active", "killed", "deactivated", "deleted", "pending_delete", ""] {
            assert!(!image_busy(done), "{done}");
        }
    }

    #[test]
    fn utc_values_say_so() {
        assert_eq!(
            utc_time("2026-10-27T03:15:00.000000"),
            "2026-10-27 03:15 UTC",
            "Keystone's expiry: no zone, UTC"
        );
        assert_eq!(utc_time("2026-10-27T03:15:00Z"), "2026-10-27 03:15 UTC");
        assert_eq!(
            utc_time("2026-10-27T04:15:00+01:00"),
            "2026-10-27 04:15",
            "another offset is shown as given, unlabelled"
        );
    }

    #[test]
    fn data_url_percent_encodes_everything_but_unreserved() {
        assert_eq!(data_url("a b\n'é"), "data:text/plain;charset=utf-8,a%20b%0A%27%C3%A9");
    }

    #[test]
    fn tones() {
        assert_eq!(Tone::server("ACTIVE"), Tone::Ok);
        assert_eq!((Tone::Ok.key(), Tone::Info.key()), ("ok", "info"));
        assert_eq!(Tone::server("error"), Tone::Err);
        assert_eq!(Tone::server("BUILD"), Tone::Warn);
        assert_eq!(Tone::resource("in-use"), Tone::Ok);
        assert_eq!(Tone::resource("error_deleting"), Tone::Err);
        assert_eq!(Tone::resource("DOWN"), Tone::Off);
    }

    #[test]
    fn busy_detection() {
        assert!(is_busy(Some("spawning"), "BUILD"));
        assert!(is_busy(None, "build"));
        assert!(is_busy(Some("image_snapshot"), "ACTIVE"));
        assert!(!is_busy(None, "ACTIVE"));
        assert!(!is_busy(Some(""), "SHUTOFF"));
        assert!(!is_busy(None, "ERROR"));
    }

    #[test]
    fn volume_and_snapshot_busy_states() {
        for s in [
            "creating",
            "deleting",
            "extending",
            "attaching",
            "detaching",
            "uploading",
            "retyping",
            "Reserved",
        ] {
            assert!(volume_busy(s), "{s}");
        }
        for s in ["available", "in-use", "error", "error_deleting", "error_extending"] {
            assert!(!volume_busy(s), "{s}: settled, no polling");
        }
        assert!(
            snapshot_busy("creating")
                && snapshot_busy("deleting")
                && !snapshot_busy("available")
                && !snapshot_busy("error")
        );
    }

    #[test]
    fn az_colours_are_stable_and_distinct_for_known_zones() {
        assert_eq!(az_color("alh1"), az_color("alh1"));
        // Zones seen on the production cloud must never share a colour.
        let zones = ["alh1", "mad1", "mer1", "nova"];
        let distinct: std::collections::HashSet<_> = zones.iter().map(|z| az_color(z)).collect();
        assert_eq!(
            distinct.len(),
            zones.len(),
            "{:?}",
            zones.iter().map(|z| (z, az_color(z))).collect::<Vec<_>>()
        );
    }

    /// WCAG contrast ratio between two `#rrggbb` colours.
    fn contrast(a: &str, b: &str) -> f64 {
        fn lum(hex: &str) -> f64 {
            let c = |i: usize| {
                let v = u8::from_str_radix(&hex[i..i + 2], 16).unwrap() as f64 / 255.0;
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * c(1) + 0.7152 * c(3) + 0.0722 * c(5)
        }
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn az_colours_are_readable_in_both_themes() {
        // Light: white page. Dark: the raised surface (cards, menus), the lightest dark background.
        for (light, dark) in AZ_PALETTE {
            assert!(
                contrast(light, "#ffffff") >= 4.5,
                "{light} on white: {:.2}",
                contrast(light, "#ffffff")
            );
            assert!(
                contrast(dark, "#1b2230") >= 4.5,
                "{dark} on dark: {:.2}",
                contrast(dark, "#1b2230")
            );
        }
        let darks: std::collections::HashSet<_> = AZ_PALETTE.iter().map(|p| p.1).collect();
        assert_eq!(darks.len(), AZ_PALETTE.len());
    }

    #[test]
    fn every_error_kind_has_its_own_text() {
        let kinds = [
            ErrorKind::Unauthenticated,
            ErrorKind::Forbidden,
            ErrorKind::NotFound,
            ErrorKind::ServiceUnavailable,
            ErrorKind::RegionUnavailable,
            ErrorKind::Timeout,
            ErrorKind::OpenStack(500),
            ErrorKind::Invalid,
            ErrorKind::Internal,
        ];
        let titles: std::collections::HashSet<_> = kinds.iter().map(|k| error_text(*k).0).collect();
        assert_eq!(titles.len(), kinds.len());
        assert_eq!(
            error_text(ErrorKind::OpenStack(409)),
            error_text(ErrorKind::OpenStack(503))
        );
    }

    #[test]
    fn only_region_related_errors_offer_another_region() {
        assert!(region_may_help(ErrorKind::Timeout));
        assert!(region_may_help(ErrorKind::ServiceUnavailable));
        assert!(region_may_help(ErrorKind::RegionUnavailable));
        assert!(!region_may_help(ErrorKind::NotFound));
        assert!(!region_may_help(ErrorKind::Forbidden));
        assert!(!region_may_help(ErrorKind::OpenStack(500)));
    }

    #[test]
    fn helpers() {
        assert_eq!(dash(None::<String>), "-");
        assert_eq!(dash(Some("")), "-");
        assert_eq!(dash(Some("x")), "x");
        assert_eq!(short_time("2026-09-14T10:22:31Z"), "2026-09-14 10:22");
        // Nova action timestamps come without zone
        assert_eq!(short_time("2026-09-14T10:22:31.000000"), "2026-09-14 10:22");
        assert_eq!(age_since(crate::i18n::Lang::EnUs, "garbage"), "-");
    }

    #[test]
    fn tip_text_becomes_paragraphs_and_lists() {
        let b = tip_blocks("Intro line.\n- cpu: CPU time\n- memory.usage: MB\n\nOutro.\n- one");
        assert_eq!(
            b,
            [
                TipBlock::Para("Intro line.".into()),
                TipBlock::List(vec!["cpu: CPU time".into(), "memory.usage: MB".into()]),
                TipBlock::Para("Outro.".into()),
                TipBlock::List(vec!["one".into()]),
            ]
        );
    }

    #[test]
    fn tip_markup_bold_italic_code() {
        use TipInline::*;
        assert_eq!(
            tip_inline("**Gnocchi** uses `cpu` in *ns*; see **`rate:mean`**."),
            [
                Bold(vec![Text("Gnocchi".into())]),
                Text(" uses ".into()),
                Code("cpu".into()),
                Text(" in ".into()),
                Italic(vec![Text("ns".into())]),
                Text("; see ".into()),
                Bold(vec![Code("rate:mean".into())]),
                Text(".".into())
            ]
        );
        assert_eq!(
            tip_inline("`a*b*c` and `{\"=\": 1}`"),
            [Code("a*b*c".into()), Text(" and ".into()), Code("{\"=\": 1}".into())],
            "no formatting inside code"
        );
        assert_eq!(
            tip_inline("2 * 3 = 6"),
            [Text("2 * 3 = 6".into())],
            "an unpaired marker is text"
        );
        assert_eq!(tip_inline("x ** y"), [Text("x ** y".into())]);
    }
}
