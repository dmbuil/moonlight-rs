//! Header bars (utility, primary, secondary) in the style of the corporate
//! Horizon theme, plus the routed page below them.

use leptos::html::Details;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{A, Outlet};
use leptos_router::hooks::use_location;

use super::nav;
use crate::api::{Logout, get_session_info, switch_project, switch_region};
use crate::components::dialog::{DialogHost, Dialogs, focus_summary, install_menu_dismissal};
use crate::components::widgets::{ErrorBox, ErrorPage, RegionMenuRef};
use crate::i18n::{K, Lang, use_i18n};
use crate::model::{AppError, ErrorKind, SessionInfo, WriteMode, needs};
use crate::theme::{Theme, use_theme};

/// Makes `id` the session's project (header menu, Identity › Projects' *Set as
/// Active Project*) and reloads on the Project dashboard.
pub fn set_active_project(id: String) {
    spawn_local(async move {
        match switch_project(id).await {
            Ok(()) => hard_reload(Some("/project")),
            Err(e) if e.kind == ErrorKind::Unauthenticated => hard_reload(Some("/login")),
            Err(_) => {}
        }
    });
}

/// Full page load after changing project/region, so every cached resource
/// is re-fetched with the new scope.
pub fn hard_reload(to: Option<&str>) {
    #[cfg(feature = "hydrate")]
    {
        let loc = window().location();
        let _ = match to {
            Some(p) => loc.set_href(p),
            None => loc.reload(),
        };
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = to;
}

fn close(r: NodeRef<Details>) {
    if let Some(d) = r.get_untracked() {
        d.set_open(false);
    }
}

#[component]
pub fn Layout() -> impl IntoView {
    let i18n = use_i18n();
    let info = Resource::new(|| (), |_| get_session_info());
    let dialogs = Dialogs::provide();
    crate::components::jobs::Jobs::provide();
    crate::components::table::TableStates::provide();
    let location = use_location();
    let region_menu = NodeRef::<Details>::new();
    let other_regions = RwSignal::new(Vec::<String>::new());
    provide_context(RegionMenuRef {
        menu: region_menu,
        others: other_regions,
    });
    Effect::new(move |_| install_menu_dismissal());
    Effect::new(move |_| {
        if let Some(Ok(s)) = info.get() {
            dialogs.mode.set(s.write_mode);
            dialogs.caps.set(s.capabilities.clone());
            dialogs.prefs.set(s.prefs.clone());
            other_regions.set(s.regions.iter().filter(|r| **r != s.region).cloned().collect());
        }
    });
    let dash = move || nav::dashboard(&location.pathname.get()).0;

    view! {
        <a class="skip-link" href="#main">"Skip to content"</a>
        <header class="header">
            <MobileBar info=info/>
            <div class="utility-bar">
                <a class="brand" href="/project">
                    <MoonMark/>
                    <span class="brand-name">"Moonlight"</span>
                </a>
                <nav class="dashboards" aria-label="Dashboards">
                    <A href="/project" attr:class=move || if dash() == K::Project { "active" } else { "" }>{move || i18n.t(K::Project)}</A>
                    // Horizon shows the Admin dashboard to admins only (roadmap 85).
                    // From `info`, not
                    // `dialogs.caps`: those were the fail-open defaults during SSR, so a
                    // non-admin saw *Admin* until hydration (after a project switch).
                    <Transition fallback=|| ()>
                        {move || info.get().and_then(Result::ok).filter(|s| needs::ADMIN.met(&s.capabilities)).map(|_| view! {
                            <A href="/admin" attr:class=move || if dash() == K::Admin { "active" } else { "" }>{move || i18n.t(K::Admin)}</A>
                        })}
                    </Transition>
                    <A href="/identity/projects" attr:class=move || if dash() == K::Identity { "active" } else { "" }>{move || i18n.t(K::Identity)}</A>
                </nav>
                <span class="spacer"></span>
                <Transition fallback=|| ()>
                    {move || info.get().and_then(Result::ok).map(|s| view! { <WriteBadge mode=s.write_mode/> })}
                </Transition>
                <Transition fallback=|| ()>
                    {move || info.get().and_then(Result::ok).map(|s| view! {
                        <RegionMenu info=s.clone() details=region_menu/>
                        <ProjectMenu info=s.clone()/>
                    })}
                </Transition>
                <Transition fallback=|| ()>
                    {move || info.get().and_then(Result::ok).map(|s| view! { <UserMenu info=s/> })}
                </Transition>
            </div>
            <PrimaryNav info=info/>
        </header>
        <main id="main" class="content">
            <Transition fallback=|| ()>
                {move || info.get().map(|r| match r {
                    Ok(s) => {
                        let expiry = s.password_expires_in_days.map(|days| view! {
                            <div class="alert alert-warn" role="status">
                                <span>{move || crate::i18n::password_expires(i18n.lang(), days)}</span>
                                {s.change_password_url.clone().map(|href| view! {
                                    " " <a href=href target="_blank" rel="noopener noreferrer">{move || i18n.t(K::ChangePassword)}</a>
                                })}
                            </div>
                        });
                        let fallback = s.region_fallback.map(|lost| view! {
                            <div class="alert alert-info" role="status">
                                <span>{move || i18n.t(K::RegionFallback)}" "<strong>{s.region.clone()}</strong>"."</span>
                                <code>{lost}</code>
                            </div>
                        });
                        Some(view! { {expiry} {fallback} }.into_any())
                    }
                    Err(e) => Some(view! { <ErrorBox message=e.to_string()/> }.into_any()),
                })}
            </Transition>
            <ErrorBoundary fallback=|errors| view! {
                <ErrorPage error=AppError::internal(
                    errors.get().into_iter().map(|(_, e)| e.to_string()).collect::<Vec<_>>().join("\n")
                )/>
            }>
                // A direct URL to a panel this region doesn't offer (roadmap 19), or
                // one the user's roles don't allow (roadmap 85).
                // Capabilities from `info`, not `dialogs.caps`: the effect above fills
                // those only in the browser, so during SSR they were the defaults, where
                // `Requires::Plugin` fails and every plugin page rendered *Service not
                // available* until hydration. `info` never refetches, so the outlet is
                // built once; the memo keeps navigation between allowed pages from
                // rebuilding it. If `info` failed (the box above says why) the page answers.
                <Transition fallback=|| ()>
                    {move || info.get().map(|r| {
                        let caps = r.ok().map(|s| s.capabilities);
                        // Before the outlet renders, so pages' `caps()` (columns, tabs,
                        // actions) see this session's on the server too; the effect
                        // above runs only in the browser, after hydration.
                        if let Some(c) = &caps {
                            dialogs.caps.set(c.clone());
                        }
                        let gate = Memo::new(move |_| caps.as_ref().and_then(|c| nav::unavailable(&location.pathname.get(), c)));
                        move || match gate.get() {
                            None => view! { <Outlet/> }.into_any(),
                            Some(kind @ ErrorKind::Forbidden) => view! { <ErrorPage error=AppError::new(kind, "this page needs a role you don't have")/> }.into_any(),
                            Some(kind) => view! { <ErrorPage error=AppError::new(kind, "not offered by this region")/> }.into_any(),
                        }
                    })}
                </Transition>
            </ErrorBoundary>
        </main>
        <DialogHost/>
        <crate::components::jobs::JobsTray/>
    }
}

/// The read-only / dry-run badge (nothing when writes are on). At phone width
/// only its eye shows (roadmap 101): it is a safety signal, never hidden.
#[component]
fn WriteBadge(mode: WriteMode) -> impl IntoView {
    let i18n = use_i18n();
    match mode {
        WriteMode::Off => view! {
            <span class="badge-ro" title=move || i18n.t(K::ReadOnlyHint)>
                <EyeIcon/>
                <span class="badge-ro-text">{move || i18n.t(K::ReadOnly)}</span>
            </span>
        }
        .into_any(),
        WriteMode::DryRun => view! {
            <span class="badge-ro dry" title=move || i18n.t(K::DryRunHint)>
                <EyeIcon/>
                <span class="badge-ro-text">{move || i18n.t(K::DryRunBadge)}</span>
            </span>
        }
        .into_any(),
        WriteMode::On => ().into_any(),
    }
}

/// Where a path is, for the phone bar: the group and panel labels
/// (`Compute › Instances`, also on a detail page), or just the dashboard on a
/// page no panel owns.
fn place(path: &str) -> (K, Option<K>) {
    match nav::active(path) {
        Some((g, p)) => (g.label, Some(p.label)),
        None => (nav::dashboard(path).0, None),
    }
}

/// The header at phone width (roadmap 101): one slim bar with ☰ *Navigation*
/// (dashboards, then the current one's groups and panels), the project and the
/// current place, the write badge's eye, and *Account* (region, project, the
/// user menu). Always rendered, shown instead of the three bars by the
/// stylesheet's 720px media query: a width signal would render one header on
/// the server and another after hydration. The sheets are `<details class=
/// "menu">`, so they work before hydration and close on outside clicks and
/// Escape; picking a panel (any navigation) closes them.
#[component]
fn MobileBar(info: Resource<Result<SessionInfo, AppError>>) -> impl IntoView {
    let i18n = use_i18n();
    let location = use_location();
    let nav_sheet = NodeRef::<Details>::new();
    let account = NodeRef::<Details>::new();
    // Any navigation (a panel picked here, a link in the page) closes both.
    Effect::new(move |_| {
        location.pathname.track();
        close(nav_sheet);
        close(account);
    });
    let session = move || info.get().and_then(Result::ok);
    // Escape closes a sheet and gives focus back to its button (the header's
    // desktop menus close only on outside clicks).
    let escape = |sheet: NodeRef<Details>| {
        move |ev: leptos::ev::KeyboardEvent| {
            if ev.key() == "Escape"
                && let Some(d) = sheet.get_untracked()
            {
                d.set_open(false);
                focus_summary(&d);
            }
        }
    };
    view! {
        <div class="mobile-bar">
            <details class="menu mobile-sheet" node_ref=nav_sheet on:keydown=escape(nav_sheet)>
                <summary aria-label=move || i18n.t(K::Navigation) title=move || i18n.t(K::Navigation)>
                    <svg aria-hidden="true" viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor"
                        stroke-width="2" stroke-linecap="round">
                        <path d="M4 6h16M4 12h16M4 18h16"></path>
                    </svg>
                </summary>
                <div class="menu-list sheet">
                    <Transition fallback=|| ()>
                        {move || session().map(|s| view! { <NavSheet caps=s.capabilities/> })}
                    </Transition>
                </div>
            </details>
            <a class="brand" href="/project" aria-label="Moonlight"><MoonMark/></a>
            <div class="mobile-place">
                <Transition fallback=|| ()>
                    {move || session().map(|s| view! { <span class="mobile-project">{s.project.name}</span> })}
                </Transition>
                <span class="mobile-panel">
                    {move || {
                        let (group, panel) = place(&location.pathname.get());
                        match panel {
                            Some(p) => format!("{} › {}", i18n.t(group), i18n.t(p)),
                            None => i18n.t(group).to_string(),
                        }
                    }}
                </span>
            </div>
            <Transition fallback=|| ()>
                {move || session().map(|s| view! { <WriteBadge mode=s.write_mode/> })}
            </Transition>
            <details class="menu mobile-sheet" node_ref=account on:keydown=escape(account)>
                <summary aria-label=move || i18n.t(K::Account) title=move || i18n.t(K::Account)>
                    <svg aria-hidden="true" viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                        <circle cx="12" cy="8" r="4"></circle>
                        <path d="M4 21a8 8 0 0 1 16 0Z"></path>
                    </svg>
                </summary>
                <div class="menu-list sheet" role="menu">
                    <Transition fallback=|| ()>
                        {move || session().map(|s| view! {
                            <div class="menu-meta">
                                <strong class="sheet-user">{s.user.clone()}</strong>
                                {move || i18n.t(K::Domain)}": "{s.domain.clone()}
                            </div>
                            {(s.regions.len() > 1).then(|| view! {
                                <div class="menu-heading">{move || i18n.t(K::Region)}</div>
                                <ul><RegionChoices info=s.clone() on_pick=move || close(account)/></ul>
                            })}
                            <div class="menu-heading">{move || i18n.t(K::Project)}</div>
                            <ProjectChoices info=s.clone() on_pick=move || close(account)/>
                            <div class="menu-sep" role="separator"></div>
                            <UserItems info=s id="account-sheet-theme"/>
                        })}
                    </Transition>
                </div>
            </details>
        </div>
    }
}

/// The ☰ sheet's body: the dashboards as tabs, then the current dashboard's
/// groups (headings) and panels (links), as [`PrimaryNav`] and the secondary
/// bar show them, from the same `nav` data and this session's capabilities.
#[component]
fn NavSheet(caps: crate::model::Capabilities) -> impl IntoView {
    let i18n = use_i18n();
    let location = use_location();
    let admin = needs::ADMIN.met(&caps);
    let dash = move || nav::dashboard(&location.pathname.get()).0;
    let active_panel = move || nav::active(&location.pathname.get()).map(|(_, p)| p.href);
    let dashboards = [
        (K::Project, "/project", true),
        (K::Admin, "/admin", admin),
        (K::Identity, "/identity/projects", true),
    ];
    let caps = StoredValue::new(caps);
    view! {
        <nav class="sheet-dashboards" aria-label="Dashboards">
            {dashboards.into_iter().filter(|d| d.2).map(|(label, href, _)| view! {
                <A href=href attr:class=move || if dash() == label { "active" } else { "" }>{move || i18n.t(label)}</A>
            }).collect_view()}
        </nav>
        <nav aria-label="Panels">
            {move || {
                caps.with_value(|c| nav::dashboard(&location.pathname.get())
                    .1
                    .iter()
                    .filter_map(|g| g.visible(c).map(|panels| (g.label, panels)))
                    .map(|(group, panels)| view! {
                        <div class="menu-heading">{move || i18n.t(group)}</div>
                        <ul>
                            {panels.into_iter().map(|p| {
                                let (href, label) = (p.href, p.label);
                                view! {
                                    <li>
                                        <A href=href
                                            attr:class=move || if active_panel() == Some(href) { "menu-item selected" } else { "menu-item" }
                                            attr:aria-current=move || (active_panel() == Some(href)).then_some("page")>
                                            {move || i18n.t(label)}
                                        </A>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    })
                    .collect_view())
            }}
        </nav>
    }
}

#[component]
fn PrimaryNav(info: Resource<Result<SessionInfo, AppError>>) -> impl IntoView {
    let i18n = use_i18n();
    let location = use_location();
    // From `info`, in a transition, so the server renders the menu of this
    // session's cloud and roles (see `Layout`'s gate).
    let caps = move || {
        info.get()
            .and_then(Result::ok)
            .map(|s| s.capabilities)
            .unwrap_or_default()
    };
    let active_group = move || nav::active(&location.pathname.get()).map(|(g, _)| g.label);
    let active_panel = move || nav::active(&location.pathname.get()).map(|(_, p)| p.href);

    view! {
        <nav class="primary-bar" aria-label="Panel groups">
            <Transition fallback=|| ()>
            {move || {
                let c = caps();
                nav::dashboard(&location.pathname.get())
                    .1
                    .iter()
                    .filter_map(|g| g.visible(&c).map(|panels| (g.label, panels.first().map(|p| p.href))))
                    .map(|(label, first)| match first {
                        Some(href) => view! {
                            <A href=href attr:class=move || {
                                if active_group() == Some(label) { "group active" } else { "group" }
                            }>
                                {move || i18n.t(label)}
                            </A>
                        }
                        .into_any(),
                        None => view! {
                            <span class="group disabled" title=move || i18n.t(K::ComingSoon)>
                                {move || i18n.t(label)}
                            </span>
                        }
                        .into_any(),
                    })
                    .collect_view()
            }}
            </Transition>
        </nav>
        <nav class="secondary-bar" aria-label="Panels">
            <Transition fallback=|| ()>
            {move || {
                let group = active_group();
                let c = caps();
                nav::dashboard(&location.pathname.get())
                    .1
                    .iter()
                    .find(|g| Some(g.label) == group)
                    .and_then(|g| g.visible(&c))
                    .map(|panels| {
                        panels
                            .into_iter()
                            .map(|p| {
                                let href = p.href;
                                let label = p.label;
                                view! {
                                    <A href=href attr:class=move || {
                                        if active_panel() == Some(href) { "panel active" } else { "panel" }
                                    }>
                                        {move || i18n.t(label)}
                                    </A>
                                }
                            })
                            .collect_view()
                    })
            }}
            </Transition>
        </nav>
    }
}

#[component]
fn RegionMenu(info: SessionInfo, details: NodeRef<Details>) -> impl IntoView {
    let i18n = use_i18n();
    let current = info.region.clone();
    view! {
        <details class="menu" node_ref=details>
            <summary>
                <span class="menu-label">{move || i18n.t(K::Region)}":"</span>
                <strong>{if current.is_empty() { "-".to_string() } else { current.clone() }}</strong>
                <Caret/>
            </summary>
            <ul class="menu-list" role="menu">
                <RegionChoices info on_pick=move || close(details)/>
            </ul>
        </details>
    }
}

/// The session's regions as radio items (`<li>`s); a pick switches region and
/// reloads. Shared by the region menu and the phone's *Account* sheet.
#[component]
fn RegionChoices(info: SessionInfo, #[prop(into)] on_pick: Callback<()>) -> impl IntoView {
    info.regions
        .into_iter()
        .map(|r| {
            let selected = r == info.region;
            let target = r.clone();
            view! {
                <li>
                    <button type="button" role="menuitemradio" aria-checked=selected.to_string()
                        class:selected=selected
                        on:click=move |_| {
                            on_pick.run(());
                            let target = target.clone();
                            spawn_local(async move {
                                match switch_region(target).await {
                                    Ok(()) => hard_reload(None),
                                    Err(e) if e.kind == ErrorKind::Unauthenticated => hard_reload(Some("/login")),
                                    Err(_) => {}
                                }
                            });
                        }>
                        {r}
                    </button>
                </li>
            }
        })
        .collect_view()
}

#[component]
fn ProjectMenu(info: SessionInfo) -> impl IntoView {
    let i18n = use_i18n();
    let details = NodeRef::<Details>::new();
    view! {
        <details class="menu" node_ref=details>
            <summary>
                <svg aria-hidden="true" viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                    <path d="M3 6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"></path>
                </svg>
                <span class="sr-only">{move || i18n.t(K::SwitchProject)}</span>
                <span class="project-name">{info.project.name.clone()}</span>
                <Caret/>
            </summary>
            <div class="menu-list wide" role="menu">
                <ProjectChoices info on_pick=move || close(details)/>
            </div>
        </details>
    }
}

/// The project search and the user's projects; a pick switches project.
/// Shared by the project menu and the phone's *Account* sheet.
#[component]
fn ProjectChoices(info: SessionInfo, #[prop(into)] on_pick: Callback<()>) -> impl IntoView {
    let i18n = use_i18n();
    let query = RwSignal::new(String::new());
    let projects = StoredValue::new(info.projects);
    let current_id = info.project.id;
    view! {
        <input class="menu-search" type="search"
            placeholder=move || i18n.t(K::SearchProjects)
            aria-label=move || i18n.t(K::SearchProjects)
            on:input=move |ev| query.set(event_target_value(&ev))/>
        <ul>
            {move || {
                let q = query.get().to_lowercase();
                let current_id = current_id.clone();
                projects.with_value(|ps| ps.iter()
                    .filter(|p| q.is_empty() || p.name.to_lowercase().contains(&q))
                    .map(|p| {
                        let selected = p.id == current_id;
                        let id = p.id.clone();
                        view! {
                            <li>
                                <button type="button" role="menuitemradio"
                                    aria-checked=selected.to_string() class:selected=selected
                                    on:click=move |_| {
                                        on_pick.run(());
                                        set_active_project(id.clone());
                                    }>
                                    {p.name.clone()}
                                </button>
                            </li>
                        }
                    })
                    .collect_view())
            }}
        </ul>
    }
}

#[component]
pub fn LanguageMenu() -> impl IntoView {
    let i18n = use_i18n();
    let details = NodeRef::<Details>::new();
    view! {
        <details class="menu" node_ref=details>
            <summary>
                <svg aria-hidden="true" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2">
                    <circle cx="12" cy="12" r="9"></circle>
                    <path d="M3 12h18M12 3c2.5 2.7 3.8 5.7 3.8 9S14.5 18.3 12 21c-2.5-2.7-3.8-5.7-3.8-9S9.5 5.7 12 3Z"></path>
                </svg>
                <span class="sr-only">{move || i18n.t(K::Language)}</span>
                {move || i18n.lang().cookie_value().to_uppercase()}
                <Caret/>
            </summary>
            <ul class="menu-list" role="menu">
                <LanguageChoices on_pick=move || close(details)/>
            </ul>
        </details>
    }
}

/// The languages as radio items (`<li>`s), applied instantly; shared by the
/// login page's menu and the user menu.
#[component]
fn LanguageChoices(#[prop(into)] on_pick: Callback<()>) -> impl IntoView {
    let i18n = use_i18n();
    Lang::ALL
        .into_iter()
        .map(|l| {
            view! {
                <li>
                    <button type="button" role="menuitemradio" lang=l.tag()
                        aria-checked=move || (i18n.lang() == l).to_string()
                        class:selected=move || i18n.lang() == l
                        on:click=move |_| { i18n.set(l); on_pick.run(()); }>
                        {l.native_name()}
                    </button>
                </li>
            }
        })
        .collect_view()
}

/// Auto / Light / Dark as three joined buttons (a segmented control), labelled
/// by the heading whose id is `labelledby`.
#[component]
fn ThemeSegments(labelledby: &'static str) -> impl IntoView {
    let i18n = use_i18n();
    let theme = use_theme();
    view! {
        <div class="segmented" role="radiogroup" aria-labelledby=labelledby>
            {Theme::ALL.into_iter().map(|t| view! {
                <button type="button" role="radio"
                    aria-checked=move || (theme.get() == t).to_string()
                    aria-label=move || i18n.t(t.label())
                    title=move || i18n.t(t.label())
                    class:selected=move || theme.get() == t
                    on:click=move |_| theme.set(t)>
                    <ThemeIcon theme=t/>
                </button>
            }).collect_view()}
        </div>
    }
}

/// Auto: a half-filled circle; Light: a sun; Dark: a crescent moon.
#[component]
fn ThemeIcon(theme: Theme) -> impl IntoView {
    let shape = match theme {
        Theme::System => view! {
            <circle cx="12" cy="12" r="8"></circle>
            <path d="M12 4a8 8 0 0 1 0 16Z" fill="currentColor"></path>
        }
        .into_any(),
        Theme::Light => view! {
            <circle cx="12" cy="12" r="4"></circle>
            <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"></path>
        }
        .into_any(),
        Theme::Dark => view! { <path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5Z"></path> }.into_any(),
    };
    view! {
        <svg aria-hidden="true" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor"
            stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            {shape}
        </svg>
    }
}

/// Auto / Light / Dark as radio items (`<li>`s), applied instantly.
#[component]
fn ThemeChoices(#[prop(into)] on_pick: Callback<()>) -> impl IntoView {
    let i18n = use_i18n();
    let theme = use_theme();
    Theme::ALL
        .into_iter()
        .map(|t| {
            view! {
                <li>
                    <button type="button" role="menuitemradio"
                        aria-checked=move || (theme.get() == t).to_string()
                        class:selected=move || theme.get() == t
                        on:click=move |_| { theme.set(t); on_pick.run(()); }>
                        {move || i18n.t(t.label())}
                    </button>
                </li>
            }
        })
        .collect_view()
}

/// Light / Dark / System switch, applied instantly.
#[component]
pub fn ThemeMenu() -> impl IntoView {
    let i18n = use_i18n();
    let details = NodeRef::<Details>::new();
    view! {
        <details class="menu" node_ref=details>
            <summary title=move || i18n.t(K::Theme)>
                <svg aria-hidden="true" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2">
                    <circle cx="12" cy="12" r="9"></circle>
                    <path d="M12 3a9 9 0 0 1 0 18Z" fill="currentColor"></path>
                </svg>
                <span class="sr-only">{move || i18n.t(K::Theme)}</span>
                <Caret/>
            </summary>
            <ul class="menu-list" role="menu">
                <ThemeChoices on_pick=move || close(details)/>
            </ul>
        </details>
    }
}

#[component]
fn UserMenu(info: SessionInfo) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <details class="menu">
            <summary>
                <svg aria-hidden="true" viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                    <circle cx="12" cy="8" r="4"></circle>
                    <path d="M4 21a8 8 0 0 1 16 0Z"></path>
                </svg>
                {info.user.clone()}
                <Caret/>
            </summary>
            <div class="menu-list" role="menu">
                <div class="menu-meta">{move || i18n.t(K::Domain)}": "{info.domain.clone()}</div>
                <UserItems info id="user-menu-theme"/>
            </div>
        </details>
    }
}

/// The user menu's items after the domain: *Theme*, *Language*, the links and
/// *Sign Out*. Shared by the user menu and the phone's *Account* sheet; `id`
/// names the theme heading, which must be unique in the page.
#[component]
fn UserItems(info: SessionInfo, id: &'static str) -> impl IntoView {
    let i18n = use_i18n();
    let languages = NodeRef::<Details>::new();
    let action = ServerAction::<Logout>::new();
    view! {
                // Picks keep the menu open, so both can be changed at once.
                <div class="menu-heading" id=id>{move || i18n.t(K::Theme)}</div>
                <ThemeSegments labelledby=id/>
                <details class="submenu" node_ref=languages>
                    <summary class="menu-item">
                        <span>{move || i18n.t(K::Language)}</span>
                        <span class="submenu-value">{move || i18n.lang().native_name()}</span>
                        <Caret/>
                    </summary>
                    <ul role="group" aria-label=move || i18n.t(K::Language)>
                        <LanguageChoices on_pick=move || close(languages)/>
                    </ul>
                </details>
                <div class="menu-sep" role="separator"></div>
                {info.help_url.clone().map(|href| view! {
                    <a class="menu-item" role="menuitem" href=href target="_blank" rel="noopener noreferrer">{move || i18n.t(K::Help)}</a>
                })}
                {info.change_password_url.clone().map(|href| view! {
                    <a class="menu-item" role="menuitem" href=href target="_blank" rel="noopener noreferrer">{move || i18n.t(K::ChangePassword)}</a>
                })}
                {info.prefs.bug_url.clone().map(|href| view! {
                    <a class="menu-item" role="menuitem" href=href target="_blank" rel="noopener noreferrer">{move || i18n.t(K::ReportBug)}</a>
                })}
                <ActionForm action=action>
                    <button type="submit" class="menu-item">{move || i18n.t(K::SignOut)}</button>
                </ActionForm>
    }
}

#[component]
fn Caret() -> impl IntoView {
    view! {
        <svg class="caret" aria-hidden="true" viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5">
            <path d="m6 9 6 6 6-6"></path>
        </svg>
    }
}

/// Crescent-moon logo mark.
#[component]
pub fn MoonMark() -> impl IntoView {
    view! {
        <svg class="moon" aria-hidden="true" viewBox="0 0 32 32" width="22" height="22">
            <circle cx="16" cy="16" r="15" fill="#0066ff"></circle>
            <path d="M20.5 7.5a9.5 9.5 0 1 0 4 14.6A8 8 0 1 1 20.5 7.5Z" fill="#fff"></path>
        </svg>
    }
}

#[component]
fn EyeIcon() -> impl IntoView {
    view! {
        <svg aria-hidden="true" viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.2">
            <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z"></path>
            <circle cx="12" cy="12" r="3"></circle>
        </svg>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_names_group_and_panel_also_on_detail_pages() {
        assert_eq!(place("/project/instances"), (K::Compute, Some(K::Instances)));
        assert_eq!(
            place("/project/instances/abc"),
            (K::Compute, Some(K::Instances)),
            "a detail page keeps its panel"
        );
        assert_eq!(place("/nowhere").1, None, "no panel: the dashboard alone");
    }
}
