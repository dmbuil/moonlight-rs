//! Orchestration (Heat, roadmap 62): the *Stacks* list and the stack detail page
//! with Horizon's tabs, the *Topology* one a graph drawn with draw.io's OpenStack
//! icons. Actions: *Check*, *Suspend*, *Resume*, *Delete* (single and bulk).
//! Both pages refresh while Heat is working on a stack (`*_IN_PROGRESS`, roadmap 46).

use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;

use super::details::{mono, param, Card, DetailHeader, Kv, Tabs};
use super::lists::link;
use super::topology::clip;
use crate::api::*;
use crate::components::dialog::{data_version, BulkButton, RowActions};
use crate::components::ops;
use crate::components::heat_icons::BOX;
use crate::components::stack_graph::{self, Edge};
use crate::components::topo_canvas::{Canvas, KeptView};
use crate::components::topo_layout as geo;
use crate::components::table::{col, Col, DataTable, Select};
use crate::components::widgets::{dash, poll_while_busy, render, short_time, Busy, Copyable, PageHeader, Pill, Skeleton, Tone};
use crate::i18n::{stack_status, t, use_i18n, Lang, K};
use crate::model::*;

/// Heat's status colours: failed red, in progress amber, deleted, suspended or
/// never created grey, the rest green.
fn stack_tone(status: &str) -> Tone {
    if status.ends_with("_FAILED") {
        Tone::Err
    } else if stack_busy(status) {
        Tone::Warn
    } else if matches!(status, "DELETE_COMPLETE" | "SUSPEND_COMPLETE" | "INIT_COMPLETE") {
        Tone::Off
    } else {
        Tone::Ok
    }
}

/// A Heat status pill, with the busy bar beside it while Heat works on it.
fn stack_pill(status: &str, l: Lang) -> AnyView {
    let label = stack_status(l, status);
    let busy = stack_busy(status).then(|| view! { <Busy label=label.clone()/> });
    view! { <Pill tone=stack_tone(status) label=label.clone()/> {busy} }.into_any()
}

/// An event's status pill, without the busy bar: an event is a past transition,
/// so `UPDATE_IN_PROGRESS` there says when the update started, not that it still runs.
fn event_pill(status: &str, l: Lang) -> AnyView {
    view! { <Pill tone=stack_tone(status) label=stack_status(l, status)/> }.into_any()
}

/// A list of names, or `-`.
fn names(v: &[String]) -> String {
    if v.is_empty() { "-".into() } else { v.join(", ") }
}

fn stack_columns() -> Vec<Col<StackRow>> {
    vec![
        col(K::Name, |x: &StackRow, _| link(format!("/project/stacks/{}", x.id), x.name.clone())).sort_text(|x| x.name.clone()),
        col(K::Created, |x: &StackRow, _| short_time(&x.created)).sort_text(|x| x.created.clone()).class("nowrap"),
        col(K::Updated, |x: &StackRow, _| dash(x.updated.as_deref().map(short_time))).sort_text(|x| x.updated.clone().unwrap_or_default()).class("nowrap"),
        col(K::Status, |x: &StackRow, l| stack_pill(&x.status, l)).sort_text(|x| x.status.clone()),
        col(K::Actions, |x: &StackRow, l| view! { <RowActions items=ops::stack_items(l, x)/> }).class("actions"),
    ]
}

/// Orchestration › Stacks, with Horizon's bulk *Check*/*Suspend*/*Resume*/*Delete*.
/// Not `list_page!`: that has one bulk button, and the selection key here is
/// [`stack_key`] (Heat paths need the name too).
#[component]
pub fn StacksPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_stacks());
    let polling = poll_while_busy(data, move || data.with(|r| matches!(r, Some(Ok(td)) if td.data.iter().any(|s| stack_busy(&s.status)))));
    let columns = StoredValue::new(stack_columns());
    let select = Select { key: |x: &StackRow| Some((stack_key(&x.name, &x.id), x.name.clone())), picked: RwSignal::new(Vec::new()) };
    view! {
        <PageHeader>
            {move || polling.get().then(|| view! { <span class="updating">{i18n.t(K::Updating)}</span> })}
            {move || {
                let picked = select.picked.get();
                (!picked.is_empty()).then(|| view! { <RowActions items=ops::bulk_stack_items(i18n.lang(), picked)/> })
            }}
            <A href="/project/stacks/launch" attr:class="btn btn-primary">
                <span aria-hidden="true">"+ "</span>{move || i18n.t(K::LaunchStack)}
            </A>
            <BulkButton label=K::DeleteStacks picked=select.picked
                open=Arc::new(|l, items| ops::bulk_delete(l, K::DeleteStacks, items, |k| {
                    let (name, id) = split_stack_key(&k);
                    OsAction::StackDelete { name, id }
                }))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |td| view! {
                <DataTable
                    rows=td.data
                    columns=columns.get_value()
                    search=|x: &StackRow| format!("{} {} {} {}", x.name, x.id, x.status, x.description.as_deref().unwrap_or(""))
                    timing=(td.millis, td.calls)
                    on_refresh=move |_| data.refetch()
                    select=select
                />
            })}
        </Transition>
    }
}

fn resource_name_cell(r: &StackResource) -> AnyView {
    let name = r.name.clone();
    match r.href.clone() {
        Some(href) => link(href, name),
        None => view! { <span class="strong">{name}</span> }.into_any(),
    }
}

fn resource_columns() -> Vec<Col<StackResource>> {
    vec![
        col(K::StackResource, |r: &StackResource, _| resource_name_cell(r)).sort_text(|r| r.name.clone()),
        col(K::ResourceId, |r: &StackResource, _| mono(r.physical_id.clone().unwrap_or_else(|| "-".into()))),
        col(K::ResourceType, |r: &StackResource, _| mono(r.resource_type.clone())).sort_text(|r| r.resource_type.clone()),
        col(K::Updated, |r: &StackResource, _| short_time(&r.updated)).sort_text(|r| r.updated.clone()).class("nowrap"),
        col(K::Status, |r: &StackResource, l| stack_pill(&r.status, l)).sort_text(|r| r.status.clone()),
        col(K::StatusReason, |r: &StackResource, _| dash(r.status_reason.clone())),
    ]
}

fn topology_columns() -> Vec<Col<StackResource>> {
    vec![
        col(K::StackResource, |r: &StackResource, _| resource_name_cell(r)).sort_text(|r| r.name.clone()),
        col(K::ResourceType, |r: &StackResource, _| mono(r.resource_type.clone())).sort_text(|r| r.resource_type.clone()),
        col(K::Status, |r: &StackResource, l| stack_pill(&r.status, l)).sort_text(|r| r.status.clone()),
        col(K::DependsOn, |r: &StackResource, _| names(&r.depends_on)).sort_num(|r| r.depends_on.len() as i64),
        col(K::RequiredBy, |r: &StackResource, _| names(&r.required_by)).sort_num(|r| r.required_by.len() as i64),
    ]
}

fn event_columns() -> Vec<Col<StackEvent>> {
    vec![
        col(K::EventTime, |e: &StackEvent, _| short_time(&e.time)).sort_text(|e| e.time.clone()).class("nowrap"),
        col(K::StackResource, |e: &StackEvent, _| e.resource_name.clone()).sort_text(|e| e.resource_name.clone()),
        col(K::ResourceId, |e: &StackEvent, _| mono(e.physical_id.clone().unwrap_or_else(|| "-".into()))),
        col(K::Status, |e: &StackEvent, l| event_pill(&e.status, l)).sort_text(|e| e.status.clone()),
        col(K::StatusReason, |e: &StackEvent, _| dash(e.status_reason.clone())),
    ]
}

fn output_columns() -> Vec<Col<StackOutput>> {
    vec![
        col(K::Key, |o: &StackOutput, _| mono(o.key.clone())).sort_text(|o| o.key.clone()),
        col(K::Value, |o: &StackOutput, _| match o.error.clone() {
            Some(e) => view! { <Pill tone=Tone::Err label=e/> }.into_any(),
            None => mono(o.value.clone()),
        }),
        col(K::Description, |o: &StackOutput, _| dash(o.description.clone())),
    ]
}

/// The stack detail page. Tabs keep Horizon's order; *Topology* is a graph,
/// with the dependency list behind its *List* button.
#[component]
pub fn StackDetailPage() -> impl IntoView {
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_stack(id));
    let polling = poll_while_busy(data, move || data.with(|r| matches!(r, Some(Ok(td)) if stack_busy(&td.data.stack.status))));
    let i18n = use_i18n();
    let tab = RwSignal::new(1usize);
    // Out here so that the polling's re-renders keep the list toggle, zoom and pan.
    let graph = GraphOpts { list: RwSignal::new(false), center: RwSignal::new(0), keep: StoredValue::new(None) };
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let name = td.data.stack.name.clone();
                let row = td.data.stack.clone();
                let d = StoredValue::new(td.data);
                let items = Signal::derive(move || ops::stack_items(i18n.lang(), &row));
                view! {
                    <DetailHeader group=K::Orchestration panel=K::Stacks href="/project/stacks"
                        name=name timing=(td.millis, td.calls) items=items/>
                    <Tabs labels=vec![K::StackTopology, K::Overview, K::StackResources, K::StackEvents, K::StackTemplate] active=tab/>
                    {move || match tab.get() {
                        0 => topology_tab(d.get_value(), graph),
                        2 => resources_tab(&d.read_value().resources),
                        3 => events_tab(&d.read_value().events),
                        4 => template_tab(&d.read_value().template),
                        _ => overview_tab(d.get_value(), polling),
                    }}
                }
            })}
        </Transition>
    }
}

/// The *Topology* tab's state, kept by the page across refreshes.
#[derive(Clone, Copy)]
struct GraphOpts {
    list: RwSignal<bool>,
    center: RwSignal<u32>,
    keep: KeptView,
}

/// *Topology*: the graph (toolbar: *Center Topology*, *List*) or the dependency list.
fn topology_tab(d: StackDetail, opts: GraphOpts) -> AnyView {
    let i18n = use_i18n();
    if d.resources.is_empty() {
        return view! { <p class="muted">{move || i18n.t(K::NoStackResources)}</p> }.into_any();
    }
    let rows = d.resources.clone();
    let d = StoredValue::new(d);
    view! {
        <div class="topo-toolbar" role="toolbar" aria-label=move || i18n.t(K::StackTopology)>
            <button type="button" class="btn btn-sm" disabled=move || opts.list.get()
                on:click=move |_| opts.center.update(|n| *n += 1)>{move || i18n.t(K::CenterTopology)}</button>
            <button type="button" class="btn btn-sm" aria-pressed=move || opts.list.get().to_string()
                on:click=move |_| opts.list.update(|v| *v = !*v)>{move || i18n.t(K::ListView)}</button>
            <span class="muted">{move || i18n.t(if opts.list.get() { K::TopologyAsList } else { K::TopologyHint })}</span>
            {move || (!opts.list.get()).then(|| graph_legend(i18n.lang()))}
        </div>
        {move || if opts.list.get() {
            let (rows, cols) = (rows.clone(), topology_columns());
            view! {
                <DataTable rows=rows columns=cols
                    search=|r: &StackResource| format!("{} {} {}", r.name, r.resource_type, r.depends_on.join(" "))/>
            }
            .into_any()
        } else {
            view! { <StackGraph d=d.get_value() opts/> }.into_any()
        }}
    }
    .into_any()
}

/// What the graph's lines and colours mean (the icons speak for themselves).
fn graph_legend(lang: Lang) -> AnyView {
    let line = |dashed: bool, key: K| {
        let (class, marker) = if dashed { ("stack-edge root", None) } else { ("stack-edge", Some("url(#stack-legend-arrow)")) };
        view! {
            <li>
                <svg width="34" height="12" viewBox="0 0 34 12" aria-hidden="true">
                    <line class=class x1="2" y1="6" x2="30" y2="6" marker-end=marker></line>
                </svg>
                {t(lang, key)}
            </li>
        }
    };
    let colour = |tone: Tone, key: K| {
        view! {
            <li class=format!("stack-node {}", tone.key())>
                <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
                    <circle class="stack-ring" cx="7" cy="7" r="5"></circle>
                </svg>
                {t(lang, key)}
            </li>
        }
    };
    view! {
        <ul class="plain topo-legend" aria-label=t(lang, K::Legend)>
            <svg width="0" height="0" aria-hidden="true">
                <defs>
                    <marker id="stack-legend-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">
                        <path class="stack-arrow" d="M0 0L10 5L0 10z"></path>
                    </marker>
                </defs>
            </svg>
            {line(false, K::LegendDependsOn)}
            {line(true, K::LegendInStack)}
            {colour(Tone::Ok, K::LegendComplete)}
            {colour(Tone::Warn, K::LegendInProgress)}
            {colour(Tone::Err, K::LegendFailed)}
            {colour(Tone::Off, K::LegendInactive)}
        </ul>
    }
    .into_any()
}

/// Radius of a resource node and of the stack node.
const NODE_R: f64 = 22.0;
const STACK_R: f64 = 28.0;

/// Room per node for the force layout: the node and its label.
const ROOM: f64 = 130.0;

/// The hovered or focused node of the stack graph and where to show its popover.
/// Every access uses `try_`, as in the network topology: navigating away disposes
/// these signals while the nodes still get `blur`/`mouseleave`.
#[derive(Clone, Copy)]
struct GraphCtx {
    focus: RwSignal<Option<String>>,
    at: RwSignal<(f64, f64)>,
    moved: RwSignal<bool>,
}

impl GraphCtx {
    /// `" hl"` for the focused node and the nodes it shares an edge with, `" dim"` for the rest.
    fn state(&self, id: &str, edges: &[Edge]) -> &'static str {
        match self.focus.try_get().flatten() {
            None => "",
            Some(h) if h == id => " hl",
            Some(h) if edges.iter().any(|e| (e.from == h && e.to == id) || (e.to == h && e.from == id)) => " hl",
            Some(_) => " dim",
        }
    }
}

/// The stack as Horizon draws it: the stack in the middle, each resource with
/// its draw.io icon coloured by status, arrows to what it depends on.
#[component]
fn StackGraph(d: StackDetail, opts: GraphOpts) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = GraphCtx { focus: RwSignal::new(None), at: RwSignal::new((0.0, 0.0)), moved: RwSignal::new(false) };
    let stack = d.stack.clone();
    let edges = StoredValue::new(stack_graph::edges(&stack.id, &d.resources));
    let resources = StoredValue::new(d.resources);
    // Effects only run in the browser: the server renders the placeholder.
    let layout = RwSignal::new(None::<geo::Graph>);
    let stack_id = stack.id.clone();
    Effect::new(move |_| {
        let g = resources.with_value(|rs| {
            let ids: Vec<&str> = std::iter::once(stack_id.as_str()).chain(rs.iter().map(|r| r.name.as_str())).collect();
            edges.with_value(|es| geo::force_edges(&ids, es.iter().map(|e| (e.from.as_str(), e.to.as_str())), ROOM))
        });
        layout.set(Some(g));
    });
    let stack = StoredValue::new(stack);
    let graph = move || {
        let Some(g) = layout.get() else {
            return view! { <p class="muted topo-empty">{move || i18n.t(K::ComputingLayout)}</p> }.into_any();
        };
        let at = |id: &str| g.pos.get(id).copied().unwrap_or_default();
        let s = stack.get_value();
        let radius = |id: &str| if id == s.id { STACK_R } else { NODE_R };
        let lines = edges
            .get_value()
            .into_iter()
            .filter_map(|e| {
                let ((x1, y1), (x2, y2)) = stack_graph::trim(at(&e.from), at(&e.to), radius(&e.from), radius(&e.to) + 2.0)?;
                let class = if e.depends { "stack-edge" } else { "stack-edge root" };
                let marker = e.depends.then_some("url(#stack-arrow)");
                let (from, to) = (e.from, e.to);
                Some(view! {
                    <line class=move || {
                        let hl = match ctx.focus.try_get().flatten() {
                            None => "",
                            Some(h) if h == from || h == to => " hl",
                            Some(_) => " dim",
                        };
                        format!("{class}{hl}")
                    } x1=x1 y1=y1 x2=x2 y2=y2 marker-end=marker></line>
                })
            })
            .collect_view();
        let (sx, sy) = at(&s.id);
        let stack_node = view! {
            <GraphNode ctx id=s.id.clone() href=None label=s.name.clone() edges
                transform=format!("translate({sx},{sy})") tone=stack_tone(&s.status) busy=stack_busy(&s.status)>
                <circle class="stack-bg" r=STACK_R></circle>
                <circle class="stack-ring" r=STACK_R - 2.0></circle>
                <path class="stack-glyph stack-layers" d="M-13 -7l13 -6 13 6 -13 6z M-13 0l13 6 13 -6 M-13 7l13 6 13 -6"></path>
                <text class="topo-name" y=STACK_R + 15.0 text-anchor="middle">{clip(&s.name, 24)}</text>
            </GraphNode>
        };
        let nodes = resources
            .get_value()
            .into_iter()
            .map(|r| {
                let (x, y) = at(&r.name);
                let k = 2.0 * NODE_R / BOX;
                let glyph = match stack_graph::icon(&r.resource_type) {
                    Some(icon) => view! {
                        <path class="stack-glyph stack-icon" d=icon.path()
                            transform=format!("translate({},{}) scale({k})", -NODE_R, -NODE_R)></path>
                    }
                    .into_any(),
                    None => view! {
                        <circle class="stack-ring" r=NODE_R - 2.0></circle>
                        <text class="stack-glyph stack-initials" text-anchor="middle" dominant-baseline="central">
                            {stack_graph::initials(&r.resource_type)}
                        </text>
                    }
                    .into_any(),
                };
                view! {
                    <GraphNode ctx id=r.name.clone() href=r.href.clone() label=r.name.clone() edges
                        transform=format!("translate({x},{y})") tone=stack_tone(&r.status) busy=stack_busy(&r.status)>
                        <circle class="stack-bg" r=NODE_R></circle>
                        {glyph}
                        <text class="topo-label" y=NODE_R + 14.0 text-anchor="middle">{clip(&r.name, 20)}</text>
                    </GraphNode>
                }
            })
            .collect_view();
        view! {
            <Canvas full=(g.width, g.height) center=opts.center keep=opts.keep moved=ctx.moved label=K::StackTopology>
                <defs>
                    <marker id="stack-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7"
                        orient="auto-start-reverse">
                        <path class="stack-arrow" d="M0 0L10 5L0 10z"></path>
                    </marker>
                </defs>
                {lines}
                {stack_node}
                {nodes}
            </Canvas>
        }
        .into_any()
    };
    view! {
        {graph}
        <StackTip ctx stack resources/>
    }
}

/// A focusable node: hover and focus highlight it and its neighbours and show
/// the popover; click or Enter opens `href` (the resource's page) when there is one.
#[component]
fn GraphNode(
    ctx: GraphCtx,
    #[prop(into)] id: String,
    href: Option<String>,
    #[prop(into)] label: String,
    edges: StoredValue<Vec<Edge>>,
    #[prop(into)] transform: String,
    tone: Tone,
    busy: bool,
    children: Children,
) -> impl IntoView {
    let navigate = use_navigate();
    let g = NodeRef::<leptos::svg::G>::new();
    let (state_id, enter_id, focus_id) = (id.clone(), id.clone(), id);
    let link = href.is_some();
    let open = move || {
        if let Some(h) = &href {
            ctx.focus.try_set(None);
            navigate(h, Default::default());
        }
    };
    let open2 = open.clone();
    let class = format!("topo-node stack-node {}{}{}", tone.key(), if busy { " busy" } else { "" }, if link { " link" } else { "" });
    view! {
        <g node_ref=g class=move || format!("{class}{}", edges.try_with_value(|es| ctx.state(&state_id, es)).unwrap_or(""))
            transform=transform tabindex="0" role=if link { "link" } else { "img" } aria-label=label
            on:mouseenter=move |ev| {
                ctx.at.try_set((ev.client_x() as f64, ev.client_y() as f64));
                ctx.focus.try_set(Some(enter_id.clone()));
            }
            on:mousemove=move |ev| { ctx.at.try_set((ev.client_x() as f64, ev.client_y() as f64)); }
            on:mouseleave=move |_| { ctx.focus.try_set(None); }
            on:focus=move |_| {
                if let Some(e) = g.get_untracked() {
                    let r = e.get_bounding_client_rect();
                    ctx.at.try_set((r.right(), r.top()));
                }
                ctx.focus.try_set(Some(focus_id.clone()));
            }
            on:blur=move |_| { ctx.focus.try_set(None); }
            on:click=move |_| if !ctx.moved.try_get_untracked().unwrap_or(false) { open() }
            on:keydown=move |ev| if ev.key() == "Enter" { open2() }>
            {children()}
        </g>
    }
}

/// Name, type, status, reason and dependencies of the hovered or focused node.
#[component]
fn StackTip(ctx: GraphCtx, stack: StoredValue<StackRow>, resources: StoredValue<Vec<StackResource>>) -> impl IntoView {
    let i18n = use_i18n();
    move || {
        let id = ctx.focus.try_get().flatten()?;
        let lang = i18n.lang();
        let (x, y) = ctx.at.try_get()?;
        let (title, kind, status, reason, deps) = if stack.try_with_value(|s| s.id == id)? {
            let s = stack.try_get_value()?;
            (s.name, t(lang, K::StackNode).to_string(), s.status, s.status_reason, vec![])
        } else {
            let r = resources.try_with_value(|rs| rs.iter().find(|r| r.name == id).cloned())??;
            (r.name, r.resource_type, r.status, r.status_reason, r.depends_on)
        };
        let deps = (!deps.is_empty()).then(|| format!("{}: {}", t(lang, K::DependsOn), deps.join(", ")));
        // Left of the pointer near the right edge, so it stays on screen (max-width 340px).
        let vw = window().inner_width().ok().and_then(|w| w.as_f64()).unwrap_or(f64::MAX);
        let side = if x + 14.0 + 340.0 > vw { format!("right:{}px", vw - x + 14.0) } else { format!("left:{}px", x + 14.0) };
        Some(view! {
            <div class="topo-tip" role="tooltip" style=format!("{side};top:{}px", y + 14.0)>
                <strong>{title}</strong>
                <div class="muted"><span class="mono">{kind}</span>" · "{stack_status(lang, &status)}</div>
                {reason.map(|r| view! { <div>{r}</div> })}
                {deps.map(|d| view! { <div class="muted">{d}</div> })}
            </div>
        })
    }
}

fn resources_tab(rows: &[StackResource]) -> AnyView {
    let i18n = use_i18n();
    if rows.is_empty() {
        return view! { <p class="muted">{move || i18n.t(K::NoStackResources)}</p> }.into_any();
    }
    let (rows, cols) = (rows.to_vec(), resource_columns());
    view! {
        <DataTable rows=rows columns=cols
            search=|r: &StackResource| format!("{} {} {} {}", r.name, r.resource_type, r.status, r.physical_id.as_deref().unwrap_or(""))/>
    }
    .into_any()
}

fn events_tab(rows: &[StackEvent]) -> AnyView {
    let (rows, cols) = (rows.to_vec(), event_columns());
    view! {
        <DataTable rows=rows columns=cols
            search=|e: &StackEvent| format!("{} {} {}", e.resource_name, e.status, e.status_reason.as_deref().unwrap_or(""))/>
    }
    .into_any()
}

fn template_tab(template: &str) -> AnyView {
    view! { <pre class="console-log">{template.to_string()}</pre> }.into_any()
}

/// *Overview*: the stack's cards, its outputs and parameters.
fn overview_tab(d: StackDetail, polling: RwSignal<bool>) -> AnyView {
    let i18n = use_i18n();
    let x = d.stack;
    let status = x.status.clone();
    let rollback = !d.disable_rollback;
    let timeout = d.timeout_mins.map(|m| format!("{m} {}", t(i18n.lang(), K::Minutes)));
    let parent = match d.parent.clone() {
        Some(p) => view! { <A href=format!("/project/stacks/{p}")>{p.clone()}</A> }.into_any(),
        None => "-".into_any(),
    };
    let outputs = if d.outputs.is_empty() {
        view! { <p class="muted">{move || i18n.t(K::NoStackOutputs)}</p> }.into_any()
    } else {
        let cols = output_columns();
        view! { <DataTable rows=d.outputs.clone() columns=cols search=|o: &StackOutput| format!("{} {}", o.key, o.value)/> }.into_any()
    };
    let params = if d.parameters.is_empty() {
        view! { <dt>"-"</dt><dd></dd> }.into_any()
    } else {
        d.parameters.iter().map(|(k, v)| view! { <dt class="mono">{k.clone()}</dt><dd class="mono">{v.clone()}</dd> }).collect_view().into_any()
    };
    view! {
        <div class="cards">
            <Card title=K::Information>
                <Kv label=K::Name><Copyable value=x.name.clone()/></Kv>
                <Kv label=K::Id><Copyable value=x.id.clone() mono=true/></Kv>
                <Kv label=K::Description>{dash(x.description.clone())}</Kv>
                <Kv label=K::Status>
                    {move || stack_pill(&status, i18n.lang())}
                    {move || polling.get().then(|| view! { <span class="updating">{i18n.t(K::Updating)}</span> })}
                </Kv>
                <Kv label=K::StatusReason>{dash(x.status_reason.clone())}</Kv>
                <Kv label=K::Created>{short_time(&x.created)}</Kv>
                <Kv label=K::Updated>{dash(x.updated.as_deref().map(short_time))}</Kv>
            </Card>
            <Card title=K::Specs>
                <Kv label=K::StackTimeout>{dash(timeout)}</Kv>
                <Kv label=K::StackRollback>{move || t(i18n.lang(), if rollback { K::Yes } else { K::No })}</Kv>
                <Kv label=K::ParentStack>{parent}</Kv>
                <Kv label=K::Tags>{names(&d.tags)}</Kv>
            </Card>
            <section class="card">
                <h2>{move || i18n.t(K::StackParameters)}</h2>
                <dl class="kv">{params}</dl>
            </section>
        </div>
        <h2 class="section-title">{move || i18n.t(K::StackOutputs)}</h2>
        {outputs}
    }
    .into_any()
}
