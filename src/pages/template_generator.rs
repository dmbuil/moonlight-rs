//! Orchestration › *Template Generator* (Heat, roadmap 92): build a HOT
//! template from a palette of resource types, fill each resource's properties
//! in a form generated from the type's schema, then copy, download or open it
//! in *Launch Stack*. Reads only (resource types, template versions, a type's
//! schema); the draft lives in this browser's `localStorage`. The model and the
//! YAML are pure, in [`hot`](crate::components::hot).
//!
//! The resources are nodes on a canvas, as in Horizon: dragged from the
//! palette, moved, and linked by dragging a node's handle onto another, which
//! sets a `get_resource` or `depends_on` (the choices come from
//! [`Draft::link_options`]). The same resources are listed under it, for the
//! keyboard.

use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;

use super::stack_launch::hand_template;
use crate::api::*;
use crate::components::hot::{self, Draft, DraftOutput, DraftParam, Link, Problem, PropValue};
use crate::components::hot_icons::{self, BOX};
use crate::components::select::Combo;
use crate::components::stack_graph;
use crate::components::topo_canvas::{Canvas, KeptView};
use crate::components::topo_layout as geo;
use crate::components::widgets::{copy_to_clipboard, PageHeader};
use crate::i18n::{t, use_i18n, K};
use crate::model::*;

/// `localStorage` key of the draft.
#[cfg(feature = "hydrate")]
const STORAGE_KEY: &str = "moonlight.template_generator.draft";

/// Properties edited in a text area rather than a one-line input.
const LONG_TEXT: &[&str] = &["user_data", "config", "script", "template", "content"];

fn load_draft() -> Option<Draft> {
    #[cfg(feature = "hydrate")]
    {
        let text = window().local_storage().ok()??.get_item(STORAGE_KEY).ok()??;
        serde_json::from_str(&text).ok()
    }
    #[cfg(not(feature = "hydrate"))]
    None
}

fn save_draft(_d: &Draft) {
    #[cfg(feature = "hydrate")]
    if let Ok(Some(st)) = window().local_storage()
        && let Ok(text) = serde_json::to_string(_d)
    {
        // A full or blocked storage only loses the draft on reload: best effort.
        let _ = st.set_item(STORAGE_KEY, &text);
    }
}

fn confirm(_msg: &str) -> bool {
    #[cfg(feature = "hydrate")]
    {
        window().confirm_with_message(_msg).unwrap_or(false)
    }
    #[cfg(not(feature = "hydrate"))]
    {
        false
    }
}

/// A resource type's glyph in the 55×55 box: heat-dashboard's own icon for
/// Horizon's 18 generator types ([`hot_icons`]), else its initials in a ring.
fn glyph(kind: &str) -> AnyView {
    match hot_icons::markup(kind) {
        Some(m) => view! { <g class="hot-icon" inner_html=m></g> }.into_any(),
        None => view! {
            <circle class="stack-ring" cx=BOX / 2.0 cy=BOX / 2.0 r=BOX / 2.0 - 3.0></circle>
            <text class="stack-initials tg-initials" x=BOX / 2.0 y=BOX / 2.0 text-anchor="middle" dominant-baseline="central">
                {stack_graph::initials(kind)}
            </text>
        }
        .into_any(),
    }
}

/// A resource type's icon at `size` px.
#[component]
pub fn TypeIcon(#[prop(into)] kind: String, #[prop(default = 28.0)] size: f64) -> impl IntoView {
    view! {
        <svg class="tg-icon" aria-hidden="true" width=size height=size viewBox=format!("0 0 {BOX} {BOX}")>{glyph(&kind)}</svg>
    }
}

/// The problem as one line of the list under the preview.
fn problem_text(lang: crate::i18n::Lang, p: &Problem) -> String {
    match p {
        Problem::NoResources => t(lang, K::TgPNoResources).to_string(),
        Problem::EmptyName => t(lang, K::TgPEmptyName).to_string(),
        Problem::Duplicate(n) => format!("{} {n}", t(lang, K::TgPDuplicate)),
        Problem::MissingRequired(r, p) => format!("{} {r}.{p}", t(lang, K::TgPMissing)),
        Problem::BadValue(r, p, k) => format!("{} {r}.{p} ({k})", t(lang, K::TgPBadValue)),
        Problem::Cycle(c) => format!("{} {}", t(lang, K::TgPCycle), c.join(" → ")),
        Problem::IncompleteOutput(o) => format!("{} {o}", t(lang, K::TgPOutput)),
    }
}

/// Orchestration › Template Generator.
#[component]
pub fn TemplateGeneratorPage() -> impl IntoView {
    let i18n = use_i18n();
    let navigate = use_navigate();
    let draft = RwSignal::new(Draft::default());
    let selected = RwSignal::new(None::<String>);
    let schemas = RwSignal::new(HashMap::<String, ResourceTypeDetail>::new());
    let requested = StoredValue::new(HashSet::<String>::new());
    let schema_error = RwSignal::new(None::<String>);
    let types = LocalResource::new(list_resource_types);
    let versions = LocalResource::new(list_template_versions);

    // Load the saved draft once, then save every change (client only: the
    // server renders the empty draft, so hydration matches).
    Effect::new(move |prev: Option<()>| {
        let d = draft.get();
        if prev.is_none() {
            if let Some(saved) = load_draft() {
                selected.set(saved.resources.first().map(|r| r.name.clone()));
                draft.set(saved);
            }
        } else {
            save_draft(&d);
        }
    });
    // Each type in the draft gets its schema once: the forms and the
    // required-property check need it.
    Effect::new(move |_| {
        let kinds: Vec<String> = draft.with(|d| d.resources.iter().map(|r| r.kind.clone()).collect());
        for kind in kinds {
            if requested.with_value(|r| r.contains(&kind)) {
                continue;
            }
            requested.update_value(|r| {
                r.insert(kind.clone());
            });
            spawn_local(async move {
                match get_resource_type(kind.clone()).await {
                    Ok(td) => schemas.update(|s| {
                        s.insert(kind, td.data);
                    }),
                    Err(e) => schema_error.set(Some(format!("{kind}: {e}"))),
                }
            });
        }
    });

    let available = Memo::new(move |_| {
        types.get().and_then(|r| r.ok()).map(|td| td.data.into_iter().map(|t| t.name).collect::<HashSet<_>>())
    });
    let add = move |kind: String| {
        let mut name = String::new();
        draft.update(|d| name = d.add(&kind));
        selected.set(Some(name));
    };
    let yaml = Memo::new(move |_| draft.with(Draft::to_yaml));
    let problems = Memo::new(move |_| draft.with(|d| schemas.with(|s| d.problems(s))));

    let launch = move |_| {
        hand_template(yaml.get_untracked());
        navigate("/project/stacks/launch", Default::default());
    };
    let clear = move |_| {
        if draft.with_untracked(|d| d == &Draft::default()) || confirm(t(i18n.lang(), K::TgClearConfirm)) {
            draft.set(Draft::default());
            selected.set(None);
        }
    };

    let palette = move || {
        let avail = available.get();
        palette_view(avail, add)
    };
    let other_types = Signal::derive(move || {
        types.get().map(|r| {
            r.map(|td| td.data.into_iter().map(|t| Choice { value: t.name.clone(), label: t.name, ..Default::default() }).collect())
                .unwrap_or_default()
        })
    });

    view! {
        <PageHeader>
            <button type="button" class="btn" on:click=clear>{move || i18n.t(K::TgClear)}</button>
            <a class="btn" download="template.yaml" href=move || hot::data_url("application/x-yaml", &yaml.get())>
                {move || i18n.t(K::DownloadFile)}
            </a>
            <button type="button" class="btn btn-primary" on:click=launch
                disabled=move || draft.with(|d| d.resources.is_empty())>{move || i18n.t(K::LaunchStack)}</button>
        </PageHeader>
        <p class="muted">{move || i18n.t(K::TgDraftKept)}</p>
        {move || schema_error.get().map(|e| view! { <p class="alert alert-error" role="alert">{e}</p> })}
        <div class="tg">
            <aside class="card tg-palette" aria-label=move || i18n.t(K::TgAddResource)>
                <h2>{move || i18n.t(K::TgAddResource)}</h2>
                {palette}
                <div class="field">
                    <label for="tg-other">{move || i18n.t(K::TgOtherType)}</label>
                    <Combo id="tg-other" options=other_types value=Signal::derive(String::new)
                        on_change=Callback::new(move |v: String| if !v.is_empty() { add(v) })/>
                </div>
            </aside>
            <div class="tg-main">
                <TemplateCard draft versions/>
                <ResourcesCard draft selected schemas problems/>
                {move || selected.get().map(|name| view! { <ResourceEditor draft selected schemas name/> })}
                <ParametersCard draft/>
                <OutputsCard draft schemas/>
                <section class="card tg-preview">
                    <div class="tg-preview-head">
                        <h2>{move || i18n.t(K::StackTemplate)}</h2>
                        <button type="button" class="btn btn-sm" on:click=move |_| copy_to_clipboard(&yaml.get_untracked())>
                            {move || i18n.t(K::Copy)}
                        </button>
                    </div>
                    {move || {
                        let lang = i18n.lang();
                        let list = problems.get();
                        if list.is_empty() {
                            view! { <p class="muted">{t(lang, K::TgNoProblems)}</p> }.into_any()
                        } else {
                            view! {
                                <ul class="tg-problems" role="status">
                                    {list.iter().map(|p| view! { <li>{problem_text(lang, p)}</li> }).collect_view()}
                                </ul>
                            }.into_any()
                        }
                    }}
                    <pre class="console-log tg-yaml">{move || yaml.get()}</pre>
                </section>
            </div>
        </div>
    }
}

/// The palette: Horizon's 18 types with their icons, by service, less those
/// this cloud's Heat doesn't offer (its list hides types whose service is
/// missing). All of them while the list loads or if it fails.
fn palette_view(avail: Option<HashSet<String>>, add: impl Fn(String) + Copy + Send + Sync + 'static) -> impl IntoView {
    hot::PALETTE
        .iter()
        .filter_map(|(service, kinds)| {
            let shown: Vec<&str> = kinds.iter().copied().filter(|k| avail.as_ref().is_none_or(|a| a.contains(*k))).collect();
            (!shown.is_empty()).then(|| {
                view! {
                    <h3 class="tg-service">{*service}</h3>
                    <div class="tg-types">
                        {shown.into_iter().map(|k| {
                            // Breaks only between words: `Router​Interface`, not `RouterInterf ace`.
                            let short = camel_breaks(k.rsplit("::").next().unwrap_or(k));
                            view! {
                                <button type="button" class="tg-type" title=k draggable="true"
                                    on:dragstart=move |ev: leptos::ev::DragEvent| drag_type(&ev, k)
                                    on:click=move |_| add(k.to_string())>
                                    <TypeIcon kind=k/>
                                    <span>{short}</span>
                                </button>
                            }
                        }).collect_view()}
                    </div>
                }
            })
        })
        .collect_view()
}

/// MIME type of a palette type dragged onto the canvas.
const DRAG_MIME: &str = "application/x-moonlight-heat-type";

fn drag_type(ev: &leptos::ev::DragEvent, kind: &str) {
    if let Some(dt) = ev.data_transfer() {
        let _ = dt.set_data(DRAG_MIME, kind);
        dt.set_effect_allowed("copy");
    }
}

/// `FloatingIPAssociation` with a zero-width space before each word after the
/// first (`FloatingIP​Association`), so a narrow palette button wraps there.
fn camel_breaks(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for (i, ch) in c.iter().enumerate() {
        let next_lower = c.get(i + 1).is_some_and(char::is_ascii_lowercase);
        if i > 0 && ch.is_ascii_uppercase() && (c[i - 1].is_ascii_lowercase() || (next_lower && c[i - 1].is_ascii_uppercase())) {
            out.push('\u{200B}');
        }
        out.push(*ch);
    }
    out
}

/// Version and description.
#[component]
fn TemplateCard(draft: RwSignal<Draft>, versions: LocalResource<Result<Timed<Vec<TemplateVersion>>, AppError>>) -> impl IntoView {
    let i18n = use_i18n();
    let options = move || {
        let list = versions.get().and_then(|r| r.ok()).map(|td| hot::hot_versions(&td.data)).unwrap_or_else(|| hot::hot_versions(&[]));
        let current = draft.with(|d| d.version().to_string());
        let mut list = list;
        if !list.contains(&current) {
            list.push(current.clone());
        }
        // `selected` on the option, not `prop:value` on the select: the options
        // arrive after the value, and a select can't take a value it lacks yet.
        list.into_iter().map(|v| view! { <option value=v.clone() selected=v == current>{v.clone()}</option> }).collect_view()
    };
    // A fresh draft takes the cloud's newest HOT version.
    Effect::new(move |_| {
        if let Some(Ok(td)) = versions.get()
            && draft.with_untracked(|d| d.version.is_empty())
            && let Some(newest) = hot::hot_versions(&td.data).into_iter().next()
        {
            draft.update(|d| d.version = newest);
        }
    });
    view! {
        <section class="card tg-card">
            <div class="tg-row">
                <div class="field">
                    <label for="tg-version">{move || i18n.t(K::Version)}</label>
                    <select id="tg-version"
                        on:change=move |ev| draft.update(|d| d.version = event_target_value(&ev))>
                        {options}
                    </select>
                </div>
                <div class="field grow">
                    <label for="tg-description">{move || i18n.t(K::Description)}</label>
                    <input id="tg-description" type="text" prop:value=move || draft.with(|d| d.description.clone())
                        on:input=move |ev| draft.update(|d| d.description = event_target_value(&ev))/>
                </div>
            </div>
        </section>
    }
}

/// The draft's resources: the canvas, then the same as a list; a click selects
/// one for the editor.
#[component]
fn ResourcesCard(
    draft: RwSignal<Draft>,
    selected: RwSignal<Option<String>>,
    schemas: RwSignal<HashMap<String, ResourceTypeDetail>>,
    problems: Memo<Vec<Problem>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let rows = move || {
        let list: Vec<(String, String)> = draft.with(|d| d.resources.iter().map(|r| (r.name.clone(), r.kind.clone())).collect());
        if list.is_empty() {
            return view! { <p class="muted">{move || i18n.t(K::TgNoResourcesYet)}</p> }.into_any();
        }
        view! {
            <ul class="tg-resources">
                {list.into_iter().map(|(name, kind)| {
                    let (n1, n2, n3) = (name.clone(), name.clone(), name.clone());
                    view! {
                        <li class:active=move || selected.with(|s| s.as_deref() == Some(n1.as_str()))>
                            <button type="button" class="tg-pick" aria-pressed=move || selected.with(|s| s.as_deref() == Some(n2.as_str())).to_string()
                                on:click=move |_| selected.set(Some(n3.clone()))>
                                <TypeIcon kind=kind.clone() size=24.0/>
                                <strong>{name.clone()}</strong>
                                <span class="mono muted">{kind.clone()}</span>
                            </button>
                            <button type="button" class="btn btn-sm btn-danger-ghost" title=move || i18n.t(K::Remove) aria-label={ let n = name.clone(); move || format!("{} {}", i18n.t(K::Remove), n) }
                                on:click={
                                    let name = name.clone();
                                    move |_| {
                                        draft.update(|d| d.remove(&name));
                                        if selected.with_untracked(|s| s.as_deref() == Some(name.as_str())) {
                                            selected.set(None);
                                        }
                                    }
                                }>"×"</button>
                        </li>
                    }
                }).collect_view()}
            </ul>
        }
        .into_any()
    };
    view! {
        <section class="card tg-card">
            <h2>{move || i18n.t(K::TgResources)}</h2>
            <DraftCanvas draft selected schemas problems/>
            {rows}
        </section>
    }
}

/// Radius of a node on the canvas, in canvas units.
const NODE_R: f64 = 28.0;

/// A drag in progress on the canvas.
#[derive(Clone)]
enum Drag {
    /// Moving node `name`: where the pointer and the node were when it started.
    Move { name: String, start: (f64, f64), orig: (f64, f64) },
    /// Drawing a link from `from`.
    Link { from: String },
}

/// A link drawn between two resources that can mean several things: the
/// choices (`from`, `to`, what it sets) and where to show them, in px from the
/// canvas' top left.
#[derive(Clone)]
struct Pending {
    choices: Vec<(String, String, Link)>,
    at: (f64, f64),
}

/// The draft as Horizon's generator draws it: a node per resource with its
/// icon (gray while it lacks required properties, as Horizon's are until
/// edited), an arrow per link. Positions are saved with the draft
/// (`DraftResource.pos`).
///
/// Handlers use the `try_` accessors (AGENTS.md §6): leaving the page disposes
/// the signals while the elements still get `pointerup`/`pointerleave`.
#[component]
fn DraftCanvas(
    draft: RwSignal<Draft>,
    selected: RwSignal<Option<String>>,
    schemas: RwSignal<HashMap<String, ResourceTypeDetail>>,
    problems: Memo<Vec<Problem>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let wrap = NodeRef::<leptos::html::Div>::new();
    let center = RwSignal::new(0u32);
    let keep: KeptView = StoredValue::new(None);
    let moved = RwSignal::new(false);
    let drag = StoredValue::new(None::<Drag>);
    // A node's place while it is dragged; the draft gets it on release.
    let dragged = RwSignal::new(None::<(String, (f64, f64))>);
    let rubber = RwSignal::new(None::<((f64, f64), (f64, f64))>);
    let pending = RwSignal::new(None::<Pending>);
    let edge_sel = RwSignal::new(None::<(String, String)>);
    let link_error = RwSignal::new(false);

    // Canvas units under a pointer, from the canvas' current view (`keep`
    // follows it). `None` before the canvas is mounted.
    let to_world = move |cx: f64, cy: f64| -> Option<(f64, f64)> {
        let svg = wrap.get_untracked()?.query_selector("svg").ok()??;
        let r = svg.get_bounding_client_rect();
        let (_, v) = keep.try_get_value()??;
        let (scale, off) = geo::fit(v, r.width(), r.height());
        // Never a NaN: it would end up in the saved draft as a node's place.
        Some((v.0 + (cx - r.left() - off) / scale, v.1 + (cy - r.top()) / scale)).filter(|(x, y)| x.is_finite() && y.is_finite())
    };
    let to_px = move |cx: f64, cy: f64| {
        wrap.get_untracked().map(|w| {
            let r = w.get_bounding_client_rect();
            (cx - r.left(), cy - r.top())
        })
    };
    // Only what the drawing needs, so typing in the form doesn't redraw it.
    let kinds = Memo::new(move |_| draft.with(|d| d.resources.iter().map(|r| (r.name.clone(), r.kind.clone())).collect::<Vec<_>>()));
    let positions = Memo::new(move |_| {
        let mut p = draft.with(Draft::positions);
        if let Some((n, at)) = dragged.get() {
            p.iter_mut().filter(|(m, _)| *m == n).for_each(|(_, xy)| *xy = at);
        }
        p
    });
    let edges = Memo::new(move |_| draft.with(|d| hot::edge_pairs(&d.edges())));
    let incomplete = Memo::new(move |_| {
        problems.with(|ps| {
            ps.iter()
                .filter_map(|p| match p {
                    Problem::MissingRequired(r, _) | Problem::BadValue(r, _, _) => Some(r.clone()),
                    _ => None,
                })
                .collect::<HashSet<_>>()
        })
    });
    // Grows in steps of four cells, so a moved node rarely changes it (a new
    // size makes the canvas fit the view again).
    let full = Memo::new(move |_| {
        let (mx, my) = positions.with(|p| p.iter().fold((0.0_f64, 0.0_f64), |(a, b), (_, (x, y))| (a.max(*x), b.max(*y))));
        let step = hot::GRID * 4.0;
        (((mx + hot::GRID) / step).ceil().max(2.0) * step, ((my + hot::GRID) / step).ceil().max(1.0) * step)
    });

    let apply = move |from: &str, to: &str, link: &Link| {
        let mut ok = false;
        draft.update(|d| ok = d.link(from, to, link));
        link_error.set(!ok);
    };
    let cancel = move || {
        drag.try_set_value(None);
        dragged.try_set(None);
        rubber.try_set(None);
    };
    let on_move = move |ev: leptos::ev::PointerEvent| {
        if !ev.is_primary() {
            return;
        }
        let Some(d) = drag.try_get_value().flatten() else { return };
        let Some(w) = to_world(ev.client_x() as f64, ev.client_y() as f64) else { return };
        match d {
            Drag::Move { name, start, orig } => {
                let (dx, dy) = (w.0 - start.0, w.1 - start.1);
                if dragged.try_get_untracked().flatten().is_none() && dx.abs() + dy.abs() < 3.0 {
                    return;
                }
                dragged.try_set(Some((name, ((orig.0 + dx).max(NODE_R), (orig.1 + dy).max(NODE_R)))));
            }
            Drag::Link { from } => {
                let a = positions.try_with_untracked(|p| p.iter().find(|(n, _)| *n == from).map(|(_, xy)| *xy)).flatten();
                if let Some(a) = a {
                    rubber.try_set(Some((a, w)));
                }
            }
        }
    };
    let on_up = move |ev: leptos::ev::PointerEvent| {
        let Some(d) = drag.try_get_value().flatten() else { return };
        match d {
            Drag::Move { .. } => {
                if let Some((name, at)) = dragged.try_get_untracked().flatten() {
                    draft.try_update(|dr| {
                        if let Some(r) = dr.resource_mut(&name) {
                            r.pos = Some(at);
                        }
                    });
                }
            }
            Drag::Link { from } => {
                let (cx, cy) = (ev.client_x() as f64, ev.client_y() as f64);
                let to = to_world(cx, cy).and_then(|w| positions.try_with_untracked(|p| hot::node_at(p, w, NODE_R)).flatten());
                if let Some(to) = to.filter(|t| *t != from) {
                    let choices = draft.with_untracked(|d| schemas.with_untracked(|s| d.link_choices(s, &from, &to)));
                    match choices.as_slice() {
                        [] => {}
                        [(f, t, l)] => apply(f, t, l),
                        _ => pending.set(Some(Pending { choices, at: to_px(cx, cy).unwrap_or_default() })),
                    }
                }
            }
        }
        cancel();
    };
    let on_drop = move |ev: leptos::ev::DragEvent| {
        ev.prevent_default();
        let Some(kind) = ev.data_transfer().and_then(|dt| dt.get_data(DRAG_MIME).ok()).filter(|k| !k.is_empty()) else { return };
        let at = to_world(ev.client_x() as f64, ev.client_y() as f64);
        let mut name = String::new();
        draft.update(|d| {
            name = d.add(&kind);
            if let (Some(r), Some(at)) = (d.resource_mut(&name), at) {
                r.pos = Some((at.0.max(NODE_R), at.1.max(NODE_R)));
            }
        });
        selected.set(Some(name));
    };

    // Each edge's line and label, with the pairs drawn both ways set apart.
    let placed = Memo::new(move |_| {
        positions.with(|pos| {
            let at = |n: &str| pos.iter().find(|(m, _)| m == n).map(|(_, p)| *p);
            edges.with(|es| {
                es.iter()
                    .filter_map(|e| {
                        let seg = stack_graph::trim(at(&e.from)?, at(&e.to)?, NODE_R, NODE_R + 2.0)?;
                        let reverse = es.iter().any(|o| o.from == e.to && o.to == e.from);
                        Some((e.clone(), hot::edge_place(seg, reverse)))
                    })
                    .collect::<Vec<_>>()
            })
        })
    });
    let edge_lines = move || {
        let sel = edge_sel.get();
        placed
            .get()
            .into_iter()
            .map(|(e, p)| {
                let ((x1, y1), (x2, y2)) = p.line;
                let is_sel = sel.as_ref().is_some_and(|(f, t)| *f == e.from && *t == e.to);
                let pair = (e.from.clone(), e.to.clone());
                let title = format!("{} → {}: {}", e.from, e.to, e.label);
                view! {
                    <g class="tg-edge" class:sel=is_sel
                        on:pointerdown=|ev| ev.stop_propagation()
                        on:click=move |_| { edge_sel.set(Some(pair.clone())); pending.set(None); }>
                        <title>{title}</title>
                        <line class="tg-edge-hit" x1=x1 y1=y1 x2=x2 y2=y2></line>
                        <line class="stack-edge" class:root=e.dashed x1=x1 y1=y1 x2=x2 y2=y2 marker-end="url(#tg-arrow)"></line>
                    </g>
                }
            })
            .collect_view()
    };
    // Over the nodes, so a short edge's label isn't hidden behind them.
    let edge_labels = move || {
        placed
            .get()
            .into_iter()
            .map(|(e, p)| view! { <text class="tg-edge-label" x=p.label.0 y=p.label.1 text-anchor=p.anchor>{e.label}</text> })
            .collect_view()
    };
    let nodes = move || {
        let inc = incomplete.get();
        let sel = selected.get();
        let k = 2.0 * NODE_R / BOX;
        positions
            .get()
            .into_iter()
            .zip(kinds.get())
            .map(|((name, (x, y)), (_, kind))| {
                let (n1, n2, n3) = (name.clone(), name.clone(), name.clone());
                view! {
                    <g class="tg-node" class:sel=sel.as_deref() == Some(name.as_str()) class:incomplete=inc.contains(&name)
                        transform=format!("translate({x},{y})") role="button" tabindex="0"
                        aria-label=format!("{name} ({kind})")
                        on:pointerdown=move |ev| {
                            if ev.button() != 0 || !ev.is_primary() { return; }
                            ev.stop_propagation();
                            if let Some(w) = to_world(ev.client_x() as f64, ev.client_y() as f64) {
                                drag.try_set_value(Some(Drag::Move { name: n1.clone(), start: w, orig: (x, y) }));
                            }
                        }
                        on:click=move |_| { selected.set(Some(n2.clone())); edge_sel.set(None); }
                        on:keydown=move |ev| if ev.key() == "Enter" || ev.key() == " " {
                            ev.prevent_default();
                            selected.set(Some(n3.clone()));
                        }>
                        <title>{kind.clone()}</title>
                        <circle class="tg-node-bg" r=NODE_R + 4.0></circle>
                        <g transform=format!("translate({},{}) scale({k})", -NODE_R, -NODE_R)>{glyph(&kind)}</g>
                        <text class="topo-label tg-node-name" y=NODE_R + 16.0 text-anchor="middle">{name.clone()}</text>
                        <g class="tg-handle" transform=format!("translate({},{})", NODE_R * 0.8, -NODE_R * 0.8)
                            on:pointerdown={
                                let name = name.clone();
                                move |ev| {
                                    if ev.button() != 0 || !ev.is_primary() { return; }
                                    ev.stop_propagation();
                                    pending.set(None);
                                    drag.try_set_value(Some(Drag::Link { from: name.clone() }));
                                }
                            }>
                            <title>{move || i18n.t(K::TgLinkHandle)}</title>
                            // A finger-sized target around the small visible one.
                            <circle class="tg-handle-hit" r="16"></circle>
                            <circle r="8"></circle>
                            <path d="M-4 0H4M0 -4V4"></path>
                        </g>
                    </g>
                }
            })
            .collect_view()
    };
    let drawing = move || {
        let size = full.get();
        view! {
            <Canvas full=size center keep moved label=K::TgCanvas>
                <defs>
                    <marker id="tg-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
                        <path class="stack-arrow" d="M0 0L10 5L0 10z"></path>
                    </marker>
                </defs>
                {edge_lines}
                {move || rubber.get().map(|((x1, y1), (x2, y2))| view! {
                    <line class="tg-rubber" x1=x1 y1=y1 x2=x2 y2=y2></line>
                })}
                {nodes}
                {edge_labels}
            </Canvas>
        }
    };
    let remove_link = move |_| {
        if let Some((f, t)) = edge_sel.get_untracked() {
            draft.update(|d| d.unlink(&f, &t));
            edge_sel.set(None);
        }
    };
    view! {
        <div class="topo-toolbar tg-canvas-bar">
            <button type="button" class="btn btn-sm" on:click=move |_| center.update(|c| *c += 1)>{move || i18n.t(K::CenterTopology)}</button>
            {move || edge_sel.get().map(|(f, t)| view! {
                <button type="button" class="btn btn-sm btn-danger-ghost" on:click=remove_link>
                    {move || i18n.t(K::TgRemoveLink)}" "<span class="mono">{format!("{f} → {t}")}</span>
                </button>
            })}
        </div>
        <p class="muted tg-help">{move || i18n.t(K::TgCanvasHelp)}</p>
        {move || link_error.get().then(|| view! { <p class="alert alert-error" role="alert">{move || i18n.t(K::TgLinkFailed)}</p> })}
        <div class="tg-canvas" node_ref=wrap
            on:pointermove=on_move on:pointerup=on_up on:pointercancel=move |_| cancel() on:pointerleave=move |_| cancel()
            on:dragover=|ev: leptos::ev::DragEvent| ev.prevent_default()
            on:drop=on_drop
            on:keydown=move |ev| if ev.key() == "Escape" { pending.set(None); edge_sel.set(None); }>
            {drawing}
            {move || pending.get().map(|p| view! {
                <div class="tg-link-menu" role="menu" style=format!("left:{}px;top:{}px", p.at.0, p.at.1)>
                    <p>{move || i18n.t(K::TgLinkAs)}</p>
                    {p.choices.into_iter().map(|(f, t, l)| {
                        let label = format!("{f}.{} → {t}", l.label());
                        view! {
                            <button type="button" role="menuitem" class="btn btn-sm"
                                on:click=move |_| { apply(&f, &t, &l); pending.set(None); }>
                                <span class="mono">{label}</span>
                            </button>
                        }
                    }).collect_view()}
                    <button type="button" role="menuitem" class="btn btn-sm" on:click=move |_| pending.set(None)>
                        {move || i18n.t(K::Cancel)}
                    </button>
                </div>
            })}
        </div>
    }
}

/// The selected resource: its name, `depends_on` and the properties from its
/// type's schema. The layout depends only on the resource and the schema, not
/// on the values, so typing never re-renders (and never loses focus).
#[component]
fn ResourceEditor(draft: RwSignal<Draft>, selected: RwSignal<Option<String>>, schemas: RwSignal<HashMap<String, ResourceTypeDetail>>, name: String) -> impl IntoView {
    let i18n = use_i18n();
    let Some(kind) = draft.with_untracked(|d| d.resource(&name).map(|r| r.kind.clone())) else {
        return view! { <p class="muted">{move || i18n.t(K::TgPickResource)}</p> }.into_any();
    };
    let show_all = RwSignal::new(false);
    let name_error = RwSignal::new(false);
    let rn = StoredValue::new(name.clone());
    let rename = move |ev: leptos::ev::Event| {
        let new = event_target_value(&ev).trim().to_string();
        let old = rn.get_value();
        if new == old {
            return name_error.set(false);
        }
        let mut ok = false;
        draft.update(|d| ok = d.rename(&old, &new));
        name_error.set(!ok);
        if ok {
            selected.set(Some(new));
        }
    };
    // Follows resources being added, removed or renamed, not their values.
    let others = Memo::new(move |_| {
        let me = rn.get_value();
        draft.with(|d| d.resources.iter().filter(|r| r.name != me).map(|r| r.name.clone()).collect::<Vec<_>>())
    });
    let depends = move || {
        others
            .get()
            .into_iter()
            .map(|o| {
                let (o1, o2) = (o.clone(), o.clone());
                view! {
                    <label class="tg-check">
                        <input type="checkbox"
                            prop:checked=move || draft.with(|d| d.resource(&rn.get_value()).is_some_and(|r| r.depends_on.contains(&o1)))
                            on:change=move |ev| {
                                let on = event_target_checked(&ev);
                                draft.update(|d| if let Some(r) = d.resource_mut(&rn.get_value()) {
                                    r.depends_on.retain(|x| x != &o2);
                                    if on { r.depends_on.push(o2.clone()); }
                                });
                            }/>
                        <span class="mono">{o}</span>
                    </label>
                }
            })
            .collect_view()
    };
    let k2 = kind.clone();
    // Only this type's schema: another type loading must not re-render the form.
    let schema = Memo::new(move |_| schemas.with(|s| s.get(&k2).cloned()));
    let props = move || {
        let all = show_all.get();
        let Some(schema) = schema.get() else {
            return view! { <p class="muted">{move || i18n.t(K::Loading)}</p> }.into_any();
        };
        if schema.properties.is_empty() {
            return view! { <p class="muted">{move || i18n.t(K::TgNoProperties)}</p> }.into_any();
        }
        let set: Vec<String> = draft.with_untracked(|d| d.resource(&rn.get_value()).map(|r| r.props.iter().map(|p| p.name.clone()).collect()).unwrap_or_default());
        let mut list: Vec<TypeProperty> = schema.properties.into_iter().filter(|p| all || p.required || set.contains(&p.name)).collect();
        list.sort_by(|a, b| b.required.cmp(&a.required).then_with(|| a.name.cmp(&b.name)));
        view! { <div class="tg-props">{list.into_iter().map(|p| view! { <PropField draft rn p/> }).collect_view()}</div> }.into_any()
    };
    view! {
        <section class="card tg-card tg-editor">
            <h2 class="tg-editor-head"><TypeIcon kind=kind.clone()/>{name.clone()}<span class="mono muted">{kind.clone()}</span></h2>
            <div class="field">
                <label for="tg-name">{move || i18n.t(K::Name)}<span class="req">" *"</span></label>
                <input id="tg-name" type="text" prop:value=name.clone() on:change=rename aria-invalid=move || name_error.get().to_string()/>
                {move || name_error.get().then(|| view! { <small class="field-error">{move || i18n.t(K::TgNameTaken)}</small> })}
            </div>
            <fieldset class="field">
                <legend>{move || i18n.t(K::DependsOn)}</legend>
                <div class="tg-checks">{depends}</div>
                <small class="muted">{move || i18n.t(K::TgDependsHelp)}</small>
            </fieldset>
            <h3>{move || i18n.t(K::Properties)}</h3>
            {props}
            <button type="button" class="btn btn-sm" on:click=move |_| show_all.update(|a| *a = !*a)>
                {move || i18n.t(if show_all.get() { K::TgShowSet } else { K::TgShowAll })}
            </button>
        </section>
    }
    .into_any()
}

/// Where a property's value comes from, as the source select shows it.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    Value,
    Param,
    Resource,
}

/// One property: a source select (value, `get_param`, `get_resource`) when
/// there is more than a value to pick, and the control for that source.
#[component]
fn PropField(draft: RwSignal<Draft>, rn: StoredValue<String>, p: TypeProperty) -> impl IntoView {
    let i18n = use_i18n();
    let id = format!("tg-p-{}", p.name);
    let p = StoredValue::new(p);
    let get = move || draft.with(|d| d.resource(&rn.get_value()).and_then(|r| r.prop(&p.with_value(|p| p.name.clone())).cloned()));
    let put = move |v: Option<PropValue>| {
        let (name, kind) = p.with_value(|p| (p.name.clone(), p.kind.clone()));
        draft.update(|d| {
            if let Some(r) = d.resource_mut(&rn.get_value()) {
                r.set_prop(&name, &kind, v);
            }
        });
    };
    let source = Memo::new(move |_| match get() {
        Some(PropValue::Param(_)) => Source::Param,
        Some(PropValue::Resource(_)) => Source::Resource,
        _ => Source::Value,
    });
    let params = Memo::new(move |_| draft.with(|d| d.parameters.iter().map(|x| x.name.clone()).filter(|n| !n.is_empty()).collect::<Vec<_>>()));
    let candidates = Memo::new(move |_| draft.with(|d| p.with_value(|p| hot::ref_candidates(d, &rn.get_value(), p))));
    let text = move || match get() {
        Some(PropValue::Text(t)) => t,
        _ => String::new(),
    };
    let control_id = id.clone();
    let control = move || {
        let id = control_id.clone();
        match source.get() {
            Source::Param => {
                let current = move || match get() {
                    Some(PropValue::Param(x)) => x,
                    _ => String::new(),
                };
                view! {
                    <select id=id prop:value=current on:change=move |ev| put(Some(PropValue::Param(event_target_value(&ev))))>
                        {move || params.get().into_iter().map(|n| view! { <option value=n.clone() selected=n == current()>{n.clone()}</option> }).collect_view()}
                    </select>
                }
                .into_any()
            }
            Source::Resource => {
                let current = move || match get() {
                    Some(PropValue::Resource(x)) => x,
                    _ => String::new(),
                };
                view! {
                    <select id=id prop:value=current on:change=move |ev| put(Some(PropValue::Resource(event_target_value(&ev))))>
                        {move || candidates.get().into_iter().map(|n| view! { <option value=n.clone() selected=n == current()>{n.clone()}</option> }).collect_view()}
                    </select>
                }
                .into_any()
            }
            Source::Value => {
                let (kind, allowed, name, default) = p.with_value(|p| (p.kind.clone(), p.allowed.clone(), p.name.clone(), p.default.clone()));
                let set = move |v: String| put(Some(PropValue::Text(v)));
                let choices = if kind == "boolean" && allowed.is_empty() { vec!["true".to_string(), "false".to_string()] } else { allowed };
                let placeholder = default.unwrap_or_default();
                if !choices.is_empty() {
                    view! {
                        <select id=id prop:value=text on:change=move |ev| set(event_target_value(&ev))>
                            <option value="">{if placeholder.is_empty() { "-".to_string() } else { format!("- ({placeholder})") }}</option>
                            {choices.into_iter().map(|c| view! { <option value=c.clone()>{c.clone()}</option> }).collect_view()}
                        </select>
                    }
                    .into_any()
                } else if kind == "list" || kind == "map" || LONG_TEXT.contains(&name.as_str()) {
                    let rows = if kind == "string" { "6" } else { "3" };
                    view! {
                        <textarea id=id rows=rows spellcheck="false" class="mono" placeholder=placeholder prop:value=text
                            on:input=move |ev| set(event_target_value(&ev))></textarea>
                    }
                    .into_any()
                } else {
                    let mode = if matches!(kind.as_str(), "integer" | "number") { "decimal" } else { "text" };
                    view! {
                        <input id=id type="text" inputmode=mode placeholder=placeholder prop:value=text
                            on:input=move |ev| set(event_target_value(&ev))/>
                    }
                    .into_any()
                }
            }
        }
    };
    let pick_source = move |ev: leptos::ev::Event| {
        let v = match event_target_value(&ev).as_str() {
            "param" => params.get_untracked().first().cloned().map(PropValue::Param),
            "resource" => candidates.get_untracked().first().cloned().map(PropValue::Resource),
            _ => None,
        };
        put(v);
    };
    let (name, kind, required, description) = p.with_value(|p| (p.name.clone(), p.kind.clone(), p.required, p.description.clone()));
    let json = kind == "list" || kind == "map";
    view! {
        <div class="field tg-prop">
            <label for=id.clone()>
                <span class="mono">{name}</span>
                {required.then(|| view! { <span class="req">" *"</span> })}
                " "<span class="pill tg-kind">{kind}</span>
            </label>
            <div class="tg-value">
                {move || (!params.get().is_empty() || !candidates.get().is_empty()).then(|| view! {
                    <select class="tg-source" aria-label=move || i18n.t(K::TgValueSource) on:change=pick_source
                        prop:value=move || match source.get() { Source::Value => "value", Source::Param => "param", Source::Resource => "resource" }>
                        <option value="value">{move || i18n.t(K::TgFromValue)}</option>
                        {move || (!params.get().is_empty()).then(|| view! { <option value="param">{move || i18n.t(K::TgFromParam)}</option> })}
                        {move || (!candidates.get().is_empty()).then(|| view! { <option value="resource">{move || i18n.t(K::TgFromResource)}</option> })}
                    </select>
                })}
                {control}
            </div>
            {description.map(|d| view! { <small class="muted">{d}</small> })}
            {json.then(|| view! { <small class="muted">{move || i18n.t(K::TgJsonHelp)}</small> })}
        </div>
    }
}

/// Template parameters as a small table.
#[component]
fn ParametersCard(draft: RwSignal<Draft>) -> impl IntoView {
    let i18n = use_i18n();
    let count = Memo::new(move |_| draft.with(|d| d.parameters.len()));
    let row = move |i: usize| {
        let field = move |f: fn(&DraftParam) -> String| move || draft.with(|d| d.parameters.get(i).map(f).unwrap_or_default());
        view! {
            <tr>
                <td><input type="text" class="mono" aria-label=move || i18n.t(K::Name) prop:value=field(|p| p.name.clone())
                    on:change=move |ev| draft.update(|d| d.rename_param(i, event_target_value(&ev).trim()))/></td>
                <td><select aria-label=move || i18n.t(K::Type) prop:value=field(|p| p.kind.clone())
                    on:change=move |ev| draft.update(|d| d.parameters[i].kind = event_target_value(&ev))>
                    {hot::PARAM_TYPES.iter().map(|k| view! { <option value=*k>{*k}</option> }).collect_view()}
                </select></td>
                <td><input type="text" aria-label=move || i18n.t(K::DefaultValue) prop:value=field(|p| p.default.clone())
                    on:input=move |ev| draft.update(|d| d.parameters[i].default = event_target_value(&ev))/></td>
                <td><input type="text" aria-label=move || i18n.t(K::Description) prop:value=field(|p| p.description.clone())
                    on:input=move |ev| draft.update(|d| d.parameters[i].description = event_target_value(&ev))/></td>
                <td><input type="checkbox" aria-label=move || i18n.t(K::TgHidden) prop:checked=move || draft.with(|d| d.parameters.get(i).is_some_and(|p| p.hidden))
                    on:change=move |ev| draft.update(|d| d.parameters[i].hidden = event_target_checked(&ev))/></td>
                <td><button type="button" class="btn btn-sm btn-danger-ghost" aria-label=move || i18n.t(K::Remove)
                    on:click=move |_| draft.update(|d| d.remove_param(i))>"×"</button></td>
            </tr>
        }
    };
    let add = move |_| {
        draft.update(|d| {
            let name = (1..).map(|n| format!("param_{n}")).find(|n| !d.parameters.iter().any(|p| &p.name == n)).unwrap_or_default();
            d.parameters.push(DraftParam { name, kind: "string".into(), ..Default::default() });
        })
    };
    view! {
        <section class="card tg-card">
            <h2>{move || i18n.t(K::StackParameters)}</h2>
            {move || if count.get() == 0 {
                view! { <p class="muted">{move || i18n.t(K::TgNoParameters)}</p> }.into_any()
            } else {
                view! {
                    <div class="table-wrap"><table class="tg-table">
                        <thead><tr>
                            <th>{move || i18n.t(K::Name)}</th><th>{move || i18n.t(K::Type)}</th>
                            <th>{move || i18n.t(K::DefaultValue)}</th><th>{move || i18n.t(K::Description)}</th>
                            <th>{move || i18n.t(K::TgHidden)}</th><th></th>
                        </tr></thead>
                        <tbody>{(0..count.get()).map(row).collect_view()}</tbody>
                    </table></div>
                }.into_any()
            }}
            <button type="button" class="btn btn-sm" on:click=add>{move || i18n.t(K::TgAddParameter)}</button>
        </section>
    }
}

// Selects whose options render after their value (schemas and lists load
// later) mark the chosen option `selected` as well as setting `prop:value`.

/// Template outputs: a resource attribute each (`get_attr`), the attributes
/// from the resource type's schema.
#[component]
fn OutputsCard(draft: RwSignal<Draft>, schemas: RwSignal<HashMap<String, ResourceTypeDetail>>) -> impl IntoView {
    let i18n = use_i18n();
    let count = Memo::new(move |_| draft.with(|d| d.outputs.len()));
    let row = move |i: usize| {
        let field = move |f: fn(&DraftOutput) -> String| move || draft.with(|d| d.outputs.get(i).map(f).unwrap_or_default());
        let resources = move || draft.with(|d| d.resources.iter().map(|r| r.name.clone()).collect::<Vec<_>>());
        let attributes = move || {
            let kind = draft.with(|d| d.outputs.get(i).and_then(|o| d.resource(&o.resource)).map(|r| r.kind.clone()));
            kind.and_then(|k| schemas.with(|s| s.get(&k).map(|s| s.attributes.iter().map(|a| a.name.clone()).collect::<Vec<_>>()))).unwrap_or_default()
        };
        view! {
            <tr>
                <td><input type="text" class="mono" aria-label=move || i18n.t(K::Name) prop:value=field(|o| o.name.clone())
                    on:input=move |ev| draft.update(|d| d.outputs[i].name = event_target_value(&ev))/></td>
                <td><select aria-label=move || i18n.t(K::TgFromResource) prop:value=field(|o| o.resource.clone())
                    on:change=move |ev| draft.update(|d| { d.outputs[i].resource = event_target_value(&ev); d.outputs[i].attribute.clear(); })>
                    <option value="">"-"</option>
                    {move || resources().into_iter().map(|n| view! { <option value=n.clone() selected=n == field(|o| o.resource.clone())()>{n.clone()}</option> }).collect_view()}
                </select></td>
                <td><select aria-label=move || i18n.t(K::TgAttribute) prop:value=field(|o| o.attribute.clone())
                    on:change=move |ev| draft.update(|d| d.outputs[i].attribute = event_target_value(&ev))>
                    <option value="">"-"</option>
                    {move || attributes().into_iter().map(|n| view! { <option value=n.clone() selected=n == field(|o| o.attribute.clone())()>{n.clone()}</option> }).collect_view()}
                </select></td>
                <td><input type="text" aria-label=move || i18n.t(K::Description) prop:value=field(|o| o.description.clone())
                    on:input=move |ev| draft.update(|d| d.outputs[i].description = event_target_value(&ev))/></td>
                <td><button type="button" class="btn btn-sm btn-danger-ghost" aria-label=move || i18n.t(K::Remove)
                    on:click=move |_| draft.update(|d| { d.outputs.remove(i); })>"×"</button></td>
            </tr>
        }
    };
    let add = move |_| {
        draft.update(|d| {
            let name = (1..).map(|n| format!("output_{n}")).find(|n| !d.outputs.iter().any(|o| &o.name == n)).unwrap_or_default();
            d.outputs.push(DraftOutput { name, ..Default::default() });
        })
    };
    view! {
        <section class="card tg-card">
            <h2>{move || i18n.t(K::StackOutputs)}</h2>
            {move || if count.get() == 0 {
                view! { <p class="muted">{move || i18n.t(K::TgNoOutputs)}</p> }.into_any()
            } else {
                view! {
                    <div class="table-wrap"><table class="tg-table">
                        <thead><tr>
                            <th>{move || i18n.t(K::Name)}</th><th>{move || i18n.t(K::TgFromResource)}</th>
                            <th>{move || i18n.t(K::TgAttribute)}</th><th>{move || i18n.t(K::Description)}</th><th></th>
                        </tr></thead>
                        <tbody>{(0..count.get()).map(row).collect_view()}</tbody>
                    </table></div>
                }.into_any()
            }}
            <button type="button" class="btn btn-sm" on:click=add>{move || i18n.t(K::TgAddOutput)}</button>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_labels_break_between_words() {
        assert_eq!(camel_breaks("FloatingIPAssociation"), "Floating\u{200B}IP\u{200B}Association");
        assert_eq!(camel_breaks("Server"), "Server");
    }
}
