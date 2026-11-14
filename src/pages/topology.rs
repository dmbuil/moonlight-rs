//! *Network › Network Topology*, as in Horizon: a **Topology** tab (networks as
//! vertical bars, routers and instances wired to them) and a **Graph** tab
//! (force layout of the same nodes), plus a *List* fallback. Inline SVG drawn
//! by Leptos; geometry lives in [`topo_layout`](crate::components::topo_layout).

use std::collections::HashMap;
use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;

use super::details::Tabs;
use super::lists::{link, status_pill};
use crate::api::get_topology;
use crate::components::dialog::{CreateButton, data_version};
use crate::components::ops;
use crate::components::table::TimingChip;
use crate::components::topo_canvas::{Canvas, KeptView};
use crate::components::topo_layout::{self as geo, Size};
use crate::components::widgets::{PageHeader, Skeleton, Tone, az_color, render};
use crate::i18n::{K, Lang, t, topology_too_many, use_i18n};
use crate::model::*;

/// Toolbar state.
#[derive(Clone, Copy)]
struct Opts {
    /// *Small*: smaller boxes, icons only, and no addresses on the wires.
    small: RwSignal<bool>,
    /// *Toggle Labels*: names and addresses on the drawing.
    labels: RwSignal<bool>,
    /// *Toggle Collapse*: devices whose networks do not overlap share a row (*Topology* tab).
    collapsed: RwSignal<bool>,
    /// Bumped by *Center Topology*; the canvas re-fits the drawing each time it changes.
    center: RwSignal<u32>,
}

/// Shared by every node: the data, the hovered/focused node, where to show its
/// popover, and whether the pointer moved since the button went down (a pan,
/// not a click).
///
/// Navigating away disposes these signals while the node still gets
/// `blur`/`mouseleave`, so every access uses the `try_` variants (a panic there
/// aborts the WASM app and the next page never renders).
#[derive(Clone, Copy)]
struct Ctx {
    /// The whole topology, for the neighbours and the popover.
    topo: StoredValue<Topology>,
    /// The focused node: a device's or a network's ID.
    focus: RwSignal<Option<String>>,
    /// Where the popover goes, in viewport pixels: the pointer, or the focused node's top right.
    at: RwSignal<(f64, f64)>,
    /// The canvas was just dragged: the click that ends a pan must not open a node.
    moved: RwSignal<bool>,
}

impl Ctx {
    /// `" hl"` for the focused node and its neighbours, `" dim"` for the rest.
    fn node_state(&self, id: &str) -> &'static str {
        match self.focus.try_get().flatten() {
            None => "",
            Some(h) if h == id => " hl",
            Some(h) => {
                let near = self
                    .topo
                    .try_with_value(|t| {
                        t.links.iter().any(|l| {
                            (l.device_id == h && l.network_id == id) || (l.network_id == h && l.device_id == id)
                        })
                    })
                    .unwrap_or(false);
                if near { " hl" } else { " dim" }
            }
        }
    }
    /// `" hl"` for a wire that touches the focused node, `" dim"` for the others.
    fn wire_state(&self, device: &str, network: &str) -> &'static str {
        match self.focus.try_get().flatten() {
            None => "",
            Some(h) if h == device || h == network => " hl",
            Some(_) => " dim",
        }
    }
}

/// The detail page a router or instance node opens.
fn device_href(d: &TopoDevice) -> String {
    match d.kind {
        TopoKind::Router => format!("/project/routers/{}", d.id),
        TopoKind::Server => format!("/project/instances/{}", d.id),
    }
}

/// The detail page a network bar opens.
fn network_href(id: &str) -> String {
    format!("/project/networks/{id}")
}

/// The device's kind as the popover and the list name it.
fn kind_label(k: TopoKind) -> K {
    match k {
        TopoKind::Router => K::Router,
        TopoKind::Server => K::Instance,
    }
}

/// The status dot's colour class: Nova's statuses for an instance, Neutron's for a router.
fn tone_class(d: &TopoDevice) -> &'static str {
    let tone = match d.kind {
        TopoKind::Server => Tone::server(&d.status),
        TopoKind::Router => Tone::resource(&d.status),
    };
    tone.key()
}

/// Each network keeps one colour (the AZ pill palette, readable in both themes).
fn net_style(name: &str) -> String {
    let (light, dark) = az_color(name);
    format!("--net:{light};--net-dark:{dark}")
}

/// Whose a network is, drawn without relying on colour: external and shared
/// networks get a hatched fill and a badge (globe, share) over their own colour,
/// both when a network is both (cross-hatched), and a legend lists the kinds on
/// screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NetKind {
    /// The project's own network, neither external nor another project's.
    Own,
    /// `router:external`: a provider / floating IP network, whoever owns it.
    External,
    /// Another project's network the project sees (`shared` or an RBAC grant);
    /// on an external network, Neutron's `shared` flag (it is always another
    /// project's, usually the admin's).
    Shared,
}

impl NetKind {
    /// Every kind, in the order the marks and the legend list them.
    const ALL: [NetKind; 3] = [NetKind::Own, NetKind::External, NetKind::Shared];

    /// `[Own]`, or the marks of the network in [`NetKind::ALL`] order.
    fn of(n: &TopoNetwork) -> Vec<NetKind> {
        let shared = if n.external { n.shared } else { !n.owned };
        let marks: Vec<NetKind> = [(n.external, NetKind::External), (shared, NetKind::Shared)]
            .into_iter()
            .filter_map(|(on, k)| on.then_some(k))
            .collect();
        if marks.is_empty() { vec![NetKind::Own] } else { marks }
    }

    /// The kind's name in the legend.
    fn label(self) -> K {
        match self {
            NetKind::Own => K::ProjectNetwork,
            NetKind::External => K::External,
            NetKind::Shared => K::Shared,
        }
    }

    /// The `<pattern>` drawn over the network's colour (see [`hatch_defs`]).
    fn hatch(self) -> Option<&'static str> {
        match self {
            NetKind::Own => None,
            NetKind::External => Some("topo-hatch-ext"),
            NetKind::Shared => Some("topo-hatch-shared"),
        }
    }
}

/// The kinds to explain in the legend, in a fixed order; none when every
/// network is the project's own (nothing to tell apart).
fn legend_kinds(networks: &[TopoNetwork]) -> Vec<NetKind> {
    let present: Vec<NetKind> = networks.iter().flat_map(NetKind::of).collect();
    if present.iter().all(|k| *k == NetKind::Own) {
        return Vec::new();
    }
    NetKind::ALL.into_iter().filter(|k| present.contains(k)).collect()
}

/// *Network*, plus *External* / *Shared* from Neutron's flags: the tooltip and the list.
fn net_kind_text(lang: Lang, n: &TopoNetwork) -> String {
    let mut kind = t(lang, K::Network).to_string();
    if n.external {
        kind += &format!(" · {}", t(lang, K::External));
    }
    if n.shared {
        kind += &format!(" · {}", t(lang, K::Shared));
    }
    kind
}

/// Accessible name of a network node: its name, and its kind when not the project's own.
fn net_label(lang: Lang, n: &TopoNetwork) -> String {
    let marks: Vec<&str> = NetKind::of(n)
        .into_iter()
        .filter(|k| *k != NetKind::Own)
        .map(|k| t(lang, k.label()))
        .collect();
    std::iter::once(n.name.as_str())
        .chain(marks)
        .collect::<Vec<_>>()
        .join(", ")
}

/// How many badges a network shows: none for the project's own.
fn badge_count(kinds: &[NetKind]) -> usize {
    kinds.iter().filter(|k| **k != NetKind::Own).count()
}

/// The stripes over an external or shared network's shape; both cross-hatch.
fn hatch(kinds: &[NetKind], w: f64, h: f64, rx: f64) -> AnyView {
    kinds
        .iter()
        .filter_map(|k| k.hatch())
        .map(|id| view! { <rect class="topo-hatch" width=w height=h rx=rx fill=format!("url(#{id})")></rect> })
        .collect_view()
        .into_any()
}

/// One badge per mark, of radius `r`, the first centred on `(cx, cy)` and each
/// next one `step` further along `(dx, dy)`: down a bar, right along a pill.
fn badges(kinds: &[NetKind], (cx, cy): (f64, f64), r: f64, (dx, dy): (f64, f64)) -> AnyView {
    kinds
        .iter()
        .filter(|k| **k != NetKind::Own)
        .enumerate()
        .filter_map(|(i, &k)| badge(k, cx + dx * i as f64, cy + dy * i as f64, r))
        .collect_view()
        .into_any()
}

/// A round badge of radius `r` centred on `(cx, cy)`: a globe for external
/// networks, a share glyph for shared ones. Drawn on a 16-unit grid, then scaled.
fn badge(kind: NetKind, cx: f64, cy: f64, r: f64) -> Option<AnyView> {
    let icon = match kind {
        NetKind::Own => return None,
        NetKind::External => view! {
            <circle r="5.5"></circle>
            <ellipse rx="2.4" ry="5.5"></ellipse>
            <path d="M-5.5 0h11"></path>
        }
        .into_any(),
        NetKind::Shared => view! {
            <path d="M-2 -1 2 -3M-2 1 2 3"></path>
            <circle class="dot" cx="-3.5" r="1.9"></circle>
            <circle class="dot" cx="3.5" cy="-3.6" r="1.9"></circle>
            <circle class="dot" cx="3.5" cy="3.6" r="1.9"></circle>
        }
        .into_any(),
    };
    Some(
        view! {
            <g class="topo-badge" transform=format!("translate({cx},{cy}) scale({})", r / 8.0)>
                <circle class="topo-badge-bg" r="8"></circle>
                {icon}
            </g>
        }
        .into_any(),
    )
}

/// The hatch patterns, once per canvas; the legend's swatches use them too.
fn hatch_defs() -> impl IntoView {
    view! {
        <defs>
            <pattern id="topo-hatch-ext" width="7" height="7" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
                <rect class="topo-stripe" width="2.5" height="7"></rect>
            </pattern>
            <pattern id="topo-hatch-shared" width="7" height="7" patternUnits="userSpaceOnUse" patternTransform="rotate(-45)">
                <rect class="topo-stripe" width="2.5" height="7"></rect>
            </pattern>
        </defs>
    }
}

/// Own · External · Shared, with the same marks as the drawing.
fn legend(kinds: &[NetKind], lang: Lang) -> AnyView {
    let items = kinds
        .iter()
        .map(|&k| {
            view! {
                <li>
                    <svg class="topo-swatch" width="34" height="16" viewBox="0 0 34 16" aria-hidden="true">
                        <rect class="topo-swatch-fill" width="34" height="16" rx="8"></rect>
                        {hatch(&[k], 34.0, 16.0, 8.0)}
                        {badge(k, 8.0, 8.0, 6.0)}
                    </svg>
                    {t(lang, k.label())}
                </li>
            }
        })
        .collect_view();
    view! { <ul class="plain topo-legend" aria-label=t(lang, K::Legend)>{items}</ul> }.into_any()
}

/// `s` cut to `max` characters, the last one an ellipsis.
pub(crate) fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}

/// One port's addresses for the popover and the wire label: its fixed IPs, then its floating
/// IPs (marked), or *Gateway* for a router's external gateway without addresses.
fn link_ips(l: &TopoLink, lang: Lang) -> String {
    let mut out = l.ips.clone();
    out.extend(l.floating.iter().map(|f| format!("{f} ({})", t(lang, K::Floating))));
    if l.gateway && out.is_empty() {
        out.push(t(lang, K::Gateway).to_string());
    }
    out.join(", ")
}

/// The addresses of every port a device has on one network (see [`geo::pairs`]).
fn pair_ips(topo: &Topology, links: &[usize], lang: Lang) -> String {
    let all: Vec<String> = links
        .iter()
        .map(|&i| link_ips(&topo.links[i], lang))
        .filter(|s| !s.is_empty())
        .collect();
    all.join(", ")
}

/// *Network › Network Topology*: Horizon's create buttons, the *Topology* / *Graph* tabs, the
/// toolbar (size, labels, collapse, center, *List*) and the drawing, from one `get_topology`
/// call. The drawing's controls are disabled when the too-many note replaces it, and *Collapse*
/// and *Center* also while the list does.
#[component]
pub fn NetworkTopologyPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| get_topology());
    let tab = RwSignal::new(0usize);
    let list = RwSignal::new(false);
    // Set by `TopoBody`: the network count when it is over the limit.
    let too_many = RwSignal::new(None::<usize>);
    // The drawing's controls do nothing while the note replaces it.
    let no_drawing = move || list.get() || too_many.get().is_some();
    let opts = Opts {
        small: RwSignal::new(false),
        labels: RwSignal::new(true),
        collapsed: RwSignal::new(false),
        center: RwSignal::new(0),
    };
    let toggle = |s: RwSignal<bool>| move |_| s.update(|v| *v = !*v);
    let pressed = |s: RwSignal<bool>| move || s.get().to_string();
    view! {
        <PageHeader>
            <CreateButton label=Signal::derive(move || i18n.t(K::LaunchInstance).to_string())
                open=Arc::new(move || ops::launch_dialog(i18n.lang()))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateNetwork).to_string())
                open=Arc::new(move || ops::create_network_dialog(i18n.lang()))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateRouter).to_string())
                open=Arc::new(move || ops::create_router_dialog(i18n.lang()))/>
        </PageHeader>
        <Tabs labels=vec![K::TopologyTab, K::GraphTab] active=tab/>
        <div class="topo-toolbar" role="toolbar" aria-label=move || i18n.t(K::NetworkTopology)>
            <div class="seg">
                <button type="button" class="btn btn-sm" aria-pressed=move || (!opts.small.get()).to_string()
                    disabled=move || too_many.get().is_some()
                    on:click=move |_| opts.small.set(false)>{move || i18n.t(K::SizeNormal)}</button>
                <button type="button" class="btn btn-sm" aria-pressed=pressed(opts.small)
                    disabled=move || too_many.get().is_some()
                    on:click=move |_| opts.small.set(true)>{move || i18n.t(K::SizeSmall)}</button>
            </div>
            <button type="button" class="btn btn-sm" aria-pressed=pressed(opts.labels)
                disabled=move || too_many.get().is_some()
                on:click=toggle(opts.labels)>{move || i18n.t(K::ToggleLabels)}</button>
            <button type="button" class="btn btn-sm" aria-pressed=pressed(opts.collapsed)
                disabled=move || tab.get() != 0 || no_drawing()
                on:click=toggle(opts.collapsed)>{move || i18n.t(K::ToggleCollapse)}</button>
            <button type="button" class="btn btn-sm" disabled=no_drawing
                on:click=move |_| opts.center.update(|n| *n += 1)>{move || i18n.t(K::CenterTopology)}</button>
            <button type="button" class="btn btn-sm" aria-pressed=pressed(list)
                on:click=toggle(list)>{move || i18n.t(K::ListView)}</button>
        </div>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), |td| view! {
                <TopoBody topo=td.data timing=(td.millis, td.calls) tab list too_many opts/>
            })}
        </Transition>
    }
}

/// What the page shows once the topology loaded: the hint, legend and timing, then the list, the
/// too-many note, the *Topology* columns or the *Graph*. Provides the [`Ctx`] every node reads.
#[component]
fn TopoBody(
    topo: Topology,
    /// Milliseconds and API calls the fetch took, for the timing chip.
    timing: (u64, u32),
    /// `0` *Topology*, else *Graph*.
    tab: RwSignal<usize>,
    /// Shows [`TopoList`] instead of the drawing.
    list: RwSignal<bool>,
    /// Set here to the network count when it is over `topology_max_networks`, so the
    /// page disables the drawing's controls.
    too_many: RwSignal<Option<usize>>,
    opts: Opts,
) -> impl IntoView {
    let i18n = use_i18n();
    let (over, max_networks) = (topo.too_many(), topo.max_networks);
    too_many.set(over);
    if topo.networks.is_empty() && topo.devices.is_empty() {
        return view! { <p class="muted topo-empty">{move || i18n.t(K::TopologyEmpty)}</p> }.into_any();
    }
    let kinds = legend_kinds(&topo.networks);
    let ctx = Ctx {
        topo: StoredValue::new(topo),
        focus: RwSignal::new(None),
        at: RwSignal::new((0.0, 0.0)),
        moved: RwSignal::new(false),
    };
    provide_context(ctx);
    view! {
        <div class="topo-meta">
            <span class="muted">{move || (!list.get() && over.is_none()).then(|| i18n.t(K::TopologyHint))}</span>
            // Its swatches use the canvas' patterns, which the list doesn't draw.
            {move || (!list.get() && over.is_none() && !kinds.is_empty()).then(|| legend(&kinds, i18n.lang()))}
            <TimingChip millis=timing.0 calls=timing.1/>
        </div>
        {move || match (list.get(), over, tab.get()) {
            (true, _, _) => view! { <TopoList/> }.into_any(),
            (false, Some(n), _) => view! { <TooMany n max=max_networks list/> }.into_any(),
            (false, None, 0) => view! { <ColumnsView opts/> }.into_any(),
            (false, None, _) => view! { <GraphView opts/> }.into_any(),
        }}
        <TopoTip/>
    }
    .into_any()
}

/// Over `[network] topology_max_networks`: a note instead of the drawing, with
/// the way to the same data as a list or on the *Networks* panel.
#[component]
fn TooMany(n: usize, max: usize, list: RwSignal<bool>) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="topo-empty" role="status">
            <p class="muted">{move || topology_too_many(i18n.lang(), n, max)}</p>
            <p class="topo-empty-actions">
                <button type="button" class="btn btn-sm" on:click=move |_| list.set(true)>{move || i18n.t(K::ShowAsList)}</button>
                <A href="/project/networks" attr:class="btn btn-sm">{move || i18n.t(K::Networks)}</A>
            </p>
        </div>
    }
}

/// A focusable, clickable node: hover and focus highlight it and its links and
/// show the popover; click or Enter opens its detail page.
#[component]
fn Node(
    #[prop(into)] id: String,
    #[prop(into)] href: String,
    #[prop(into)] label: String,
    #[prop(into)] transform: String,
    class: &'static str,
    children: Children,
) -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    let navigate = use_navigate();
    let g = NodeRef::<leptos::svg::G>::new();
    let state_id = id.clone();
    let (enter_id, focus_id) = (id.clone(), id);
    // Drop the hover before leaving so nothing reacts to it during teardown.
    let open = move |href: &str| {
        ctx.focus.try_set(None);
        navigate(href, Default::default());
    };
    let (open2, href2) = (open.clone(), href.clone());
    view! {
        <g node_ref=g class=move || format!("topo-node {class}{}", ctx.node_state(&state_id))
            transform=transform tabindex="0" role="link" aria-label=label
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
            on:click=move |_| if !ctx.moved.try_get_untracked().unwrap_or(false) { open(&href) }
            on:keydown=move |ev| if ev.key() == "Enter" { open2(&href2) }>
            {children()}
        </g>
    }
}

/// Box of a router or instance with its top-left corner at the origin.
fn device_shape(d: &TopoDevice, s: Size, small: bool, labels: bool) -> AnyView {
    let (w, h) = (s.box_w, s.box_h);
    let rx = if d.kind == TopoKind::Router { h / 2.0 } else { 6.0 };
    let icon_x = if small { w / 2.0 } else { 18.0 };
    let icon = match d.kind {
        TopoKind::Router => view! {
            <g class="topo-icon" transform=format!("translate({icon_x},{})", h / 2.0)>
                <circle r="8"></circle>
                <path d="M-5 -2.5h10m-3-3 3 3-3 3M5 2.5h-10m3-3-3 3 3 3"></path>
            </g>
        }
        .into_any(),
        TopoKind::Server => view! {
            <g class="topo-icon" transform=format!("translate({},{})", icon_x - 7.0, h / 2.0 - 7.0)>
                <rect width="14" height="6" rx="1.5"></rect>
                <rect y="8" width="14" height="6" rx="1.5"></rect>
            </g>
        }
        .into_any(),
    };
    let name = if small {
        labels
            .then(|| {
                view! { <text class="topo-label" x=w / 2.0 y=h + 13.0 text-anchor="middle">{clip(&d.name, 8)}</text> }
            })
            .into_any()
    } else {
        view! { <text class="topo-name" x="34" y=h / 2.0 dominant-baseline="central">{clip(&d.name, 15)}</text> }
            .into_any()
    };
    view! {
        <rect class="topo-box" width=w height=h rx=rx></rect>
        {icon}
        <circle class=format!("topo-status {}", tone_class(d)) cx=w - 9.0 cy="9" r="4"></circle>
        {name}
    }
    .into_any()
}

/// The *Topology* tab, as Horizon draws it: each network a vertical bar, routers and instances
/// as boxes in rows across them, wired to the networks they have ports on. Geometry from
/// `topo_layout::columns`.
#[component]
fn ColumnsView(opts: Opts) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = expect_context::<Ctx>();
    let layout = Memo::new(move |_| {
        ctx.topo
            .with_value(|t| geo::columns(t, Size::of(opts.small.get()), opts.collapsed.get()))
    });
    let keep: KeptView = StoredValue::new(None);
    move || {
        let c = layout.get();
        let (small, labels, lang) = (opts.small.get(), opts.labels.get(), i18n.lang());
        let s = Size::of(small);
        let topo = ctx.topo.get_value();
        let nets: HashMap<&str, &TopoNetwork> = topo.networks.iter().map(|n| (n.id.as_str(), n)).collect();
        let devs: HashMap<&str, &TopoDevice> = topo.devices.iter().map(|d| (d.id.as_str(), d)).collect();
        let bars = c
            .bars
            .iter()
            .filter_map(|b| {
                let n = nets.get(b.id.as_str())?;
                let kinds = NetKind::of(n);
                // Pill-shaped like the Graph's networks; the badges stack from the top cap
                // down and the label starts below them (or below the cap).
                let rx = s.bar_w / 2.0;
                let top = match badge_count(&kinds) {
                    0 => rx,
                    c => (s.bar_w - 2.0) * c as f64 + 4.0,
                };
                let text = if small {
                    n.name.clone()
                } else {
                    format!("{}   {}", n.name, n.cidrs.join("  "))
                };
                let text = clip(&text, ((b.h - top - rx / 2.0) / 7.0).max(0.0) as usize);
                let (style, h, label_at) = (
                    net_style(&n.name),
                    b.h,
                    format!("translate({},{top}) rotate(90)", s.bar_w / 2.0),
                );
                Some(view! {
                    <Node id=n.id.clone() href=network_href(&n.id) label=net_label(lang, n)
                        transform=format!("translate({},{})", b.x, b.y) class="topo-net">
                        <g style=style>
                            <rect class="topo-bar" width=s.bar_w height=h rx=rx></rect>
                            {hatch(&kinds, s.bar_w, h, rx)}
                            {badges(&kinds, (rx, rx), rx - 3.0, (0.0, s.bar_w - 2.0))}
                            <text class="topo-bar-label" transform=label_at dominant-baseline="central">{text}</text>
                        </g>
                    </Node>
                })
            })
            .collect_view();
        let wires = c
            .wires
            .iter()
            .map(|w| {
                let (dev, net) = (w.device_id.clone(), w.network_id.clone());
                let style = nets.get(net.as_str()).map(|n| net_style(&n.name)).unwrap_or_default();
                let ips = (labels && !small)
                    .then(|| pair_ips(&topo, &w.links, lang))
                    .filter(|s| !s.is_empty());
                let right = w.x2 > w.x1;
                // Along the wire next to the bar, or under the box when the wire is too short.
                let fits = ips
                    .as_ref()
                    .is_some_and(|t| t.chars().count() as f64 * 6.5 + 14.0 < (w.x2 - w.x1).abs());
                let (tx, ty, anchor) = match (fits, right) {
                    (true, true) => (w.x2 - 6.0, w.y - 5.0, "end"),
                    (true, false) => (w.x2 + 6.0, w.y - 5.0, "start"),
                    // Second line, so it never overlaps the left label.
                    (false, true) => (w.x1, w.y + s.box_h / 2.0 + 25.0, "end"),
                    (false, false) => (w.x1, w.y + s.box_h / 2.0 + 12.0, "start"),
                };
                view! {
                    <g class=move || format!("topo-wire{}", ctx.wire_state(&dev, &net)) style=style>
                        <line x1=w.x1 y1=w.y x2=w.x2 y2=w.y></line>
                        <circle cx=w.x2 cy=w.y r="3.5"></circle>
                        {ips.map(|ip| view! { <text class="topo-ip" x=tx y=ty text-anchor=anchor>{ip}</text> })}
                    </g>
                }
            })
            .collect_view();
        let boxes = c
            .boxes
            .iter()
            .filter_map(|b| {
                let d = devs.get(b.id.as_str())?;
                let shape = device_shape(d, s, small, labels);
                Some(view! {
                    <Node id=d.id.clone() href=device_href(d) label=d.name.clone()
                        transform=format!("translate({},{})", b.x, b.y) class="topo-dev">
                        {shape}
                    </Node>
                })
            })
            .collect_view();
        view! {
            <Canvas full=(c.width, c.height) center=opts.center keep moved=ctx.moved label=K::NetworkTopology>
                {hatch_defs()}
                {bars}
                {wires}
                {boxes}
            </Canvas>
        }
    }
}

/// Space per node for the force layout.
fn room(small: bool) -> f64 {
    if small { 90.0 } else { 180.0 }
}

/// The *Graph* tab: networks and devices as nodes of a force layout, computed in an effect (so
/// only in the browser; the server renders a placeholder), with one edge per device and network
/// it has ports on ([`geo::pairs`]).
#[component]
fn GraphView(opts: Opts) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = expect_context::<Ctx>();
    // Effects only run in the browser: the server renders the placeholder.
    let graph = RwSignal::new(None::<geo::Graph>);
    let keep: KeptView = StoredValue::new(None);
    Effect::new(move |_| {
        let small = opts.small.get();
        graph.set(Some(ctx.topo.with_value(|t| geo::force(t, room(small)))));
    });
    move || {
        let Some(g) = graph.get() else {
            return view! { <p class="muted topo-empty">{move || i18n.t(K::ComputingLayout)}</p> }.into_any();
        };
        let (small, labels, lang) = (opts.small.get(), opts.labels.get(), i18n.lang());
        let s = Size::of(small);
        let topo = ctx.topo.get_value();
        let at = |id: &str| g.pos.get(id).copied().unwrap_or_default();
        let net_names: HashMap<&str, &str> = topo.networks.iter().map(|n| (n.id.as_str(), n.name.as_str())).collect();
        let edges = geo::pairs(&topo)
            .into_iter()
            .map(|(dev, net, links)| {
                let ((x1, y1), (x2, y2)) = (at(dev), at(net));
                let style = net_names.get(net).map(|n| net_style(n)).unwrap_or_default();
                let ips = (labels && !small).then(|| pair_ips(&topo, &links, lang)).filter(|s| !s.is_empty());
                let (dev, net) = (dev.to_string(), net.to_string());
                view! {
                    <g class=move || format!("topo-wire{}", ctx.wire_state(&dev, &net)) style=style>
                        <line x1=x1 y1=y1 x2=x2 y2=y2></line>
                        {ips.map(|ip| view! {
                            <text class="topo-ip" x=(x1 + x2) / 2.0 y=(y1 + y2) / 2.0 - 4.0 text-anchor="middle">{ip}</text>
                        })}
                    </g>
                }
            })
            .collect_view();
        let (pill_h, max_chars) = if small { (20.0, 10) } else { (26.0, 22) };
        let networks = topo
            .networks
            .iter()
            .map(|n| {
                let (x, y) = at(&n.id);
                let kinds = NetKind::of(n);
                let name = clip(&n.name, max_chars);
                // Room for the badges at the start of the pill; the name is centred on the rest.
                let lead = (pill_h - 4.0) * badge_count(&kinds) as f64;
                let w = (name.chars().count() as f64 * 7.0 + 22.0).max(48.0) + lead;
                let style = net_style(&n.name);
                view! {
                    <Node id=n.id.clone() href=network_href(&n.id) label=net_label(lang, n)
                        transform=format!("translate({},{})", x - w / 2.0, y - pill_h / 2.0) class="topo-net">
                        <g style=style>
                            <rect class="topo-bar" width=w height=pill_h rx=pill_h / 2.0></rect>
                            {hatch(&kinds, w, pill_h, pill_h / 2.0)}
                            {badges(&kinds, (pill_h / 2.0, pill_h / 2.0), pill_h / 2.0 - 3.0, (pill_h - 4.0, 0.0))}
                            <text class="topo-bar-label" x=(w + lead) / 2.0 y=pill_h / 2.0 text-anchor="middle"
                                dominant-baseline="central">{name}</text>
                        </g>
                    </Node>
                }
            })
            .collect_view();
        let devices = topo
            .devices
            .iter()
            .map(|d| {
                let (x, y) = at(&d.id);
                let shape = device_shape(d, s, small, labels);
                view! {
                    <Node id=d.id.clone() href=device_href(d) label=d.name.clone()
                        transform=format!("translate({},{})", x - s.box_w / 2.0, y - s.box_h / 2.0) class="topo-dev">
                        {shape}
                    </Node>
                }
            })
            .collect_view();
        view! {
            <Canvas full=(g.width, g.height) center=opts.center keep moved=ctx.moved label=K::NetworkTopology>
                {hatch_defs()}
                {edges}
                {networks}
                {devices}
            </Canvas>
        }
        .into_any()
    }
}

/// Name, type, status and addresses of the hovered or focused node.
#[component]
fn TopoTip() -> impl IntoView {
    let i18n = use_i18n();
    let ctx = expect_context::<Ctx>();
    move || {
        let id = ctx.focus.try_get().flatten()?;
        let lang = i18n.lang();
        let (x, y) = ctx.at.try_get()?;
        let (title, kind, status, lines) = ctx
            .topo
            .try_with_value(|topo| {
                if let Some(n) = topo.networks.iter().find(|n| n.id == id) {
                    return Some((
                        n.name.clone(),
                        net_kind_text(lang, n),
                        n.status.clone(),
                        n.cidrs.clone(),
                    ));
                }
                let d = topo.devices.iter().find(|d| d.id == id)?;
                let names: HashMap<&str, &str> =
                    topo.networks.iter().map(|n| (n.id.as_str(), n.name.as_str())).collect();
                let lines = topo
                    .links_of(&d.id)
                    .map(|l| {
                        format!(
                            "{}: {}",
                            names.get(l.network_id.as_str()).unwrap_or(&"-"),
                            link_ips(l, lang)
                        )
                    })
                    .collect();
                Some((
                    d.name.clone(),
                    t(lang, kind_label(d.kind)).to_string(),
                    d.status.clone(),
                    lines,
                ))
            })
            .flatten()?;
        // Left of the pointer near the right edge, so it stays on screen (max-width 340px).
        let vw = window().inner_width().ok().and_then(|w| w.as_f64()).unwrap_or(f64::MAX);
        let side = if x + 14.0 + 340.0 > vw {
            format!("right:{}px", vw - x + 14.0)
        } else {
            format!("left:{}px", x + 14.0)
        };
        Some(view! {
            <div class="topo-tip" role="tooltip" style=format!("{side};top:{}px", y + 14.0)>
                <strong>{title}</strong>
                <div class="muted">{kind}" · "{status}</div>
                <ul class="plain mono">{lines.into_iter().map(|l| view! { <li>{l}</li> }).collect_view()}</ul>
            </div>
        })
    }
}

/// The same data as a table: for screen readers and very large projects.
#[component]
fn TopoList() -> impl IntoView {
    let i18n = use_i18n();
    let ctx = expect_context::<Ctx>();
    move || {
        let lang = i18n.lang();
        let topo = ctx.topo.get_value();
        let names: HashMap<&str, &str> = topo.networks.iter().map(|n| (n.id.as_str(), n.name.as_str())).collect();
        let devices = topo
            .devices
            .iter()
            .map(|d| {
                let conns: Vec<_> = topo
                    .links_of(&d.id)
                    .map(|l| {
                        let name = names.get(l.network_id.as_str()).map(|n| n.to_string()).unwrap_or_else(|| l.network_id.clone());
                        (l.network_id.clone(), name, link_ips(l, lang))
                    })
                    .collect();
                view! {
                    <tr>
                        <td>{link(device_href(d), d.name.clone())}</td>
                        <td>{t(lang, kind_label(d.kind))}</td>
                        <td>{status_pill(&d.status, lang)}</td>
                        <td>
                            <ul class="plain">
                                {conns.into_iter().map(|(id, name, ips)| view! {
                                    <li><A href=network_href(&id)>{name}</A>" "<span class="mono muted">{ips}</span></li>
                                }).collect_view()}
                            </ul>
                        </td>
                    </tr>
                }
            })
            .collect_view();
        let networks = topo
            .networks
            .iter()
            .map(|n| {
                let kind = net_kind_text(lang, n);
                view! {
                    <tr>
                        <td>{link(network_href(&n.id), n.name.clone())}</td>
                        <td>{kind}</td>
                        <td>{status_pill(&n.status, lang)}</td>
                        <td class="mono">{if n.cidrs.is_empty() { "-".to_string() } else { n.cidrs.join(", ") }}</td>
                    </tr>
                }
            })
            .collect_view();
        view! {
            <div class="table-wrap">
                <table class="table">
                    <thead>
                        <tr>
                            <th>{t(lang, K::Name)}</th>
                            <th>{t(lang, K::Type)}</th>
                            <th>{t(lang, K::Status)}</th>
                            <th>{t(lang, K::Connections)}</th>
                        </tr>
                    </thead>
                    <tbody>{networks}{devices}</tbody>
                </table>
            </div>
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(external: bool, shared: bool, owned: bool) -> TopoNetwork {
        TopoNetwork {
            id: "n".into(),
            name: "n".into(),
            external,
            shared,
            owned,
            status: "ACTIVE".into(),
            cidrs: vec![],
        }
    }

    #[test]
    fn net_kind_marks_external_and_other_projects_networks() {
        use NetKind::*;
        assert_eq!(NetKind::of(&net(false, false, true)), [Own]);
        assert_eq!(
            NetKind::of(&net(false, true, true)),
            [Own],
            "the project's own shared network is its own"
        );
        assert_eq!(NetKind::of(&net(false, true, false)), [Shared]);
        assert_eq!(
            NetKind::of(&net(false, false, false)),
            [Shared],
            "an RBAC grant without `shared`"
        );
        assert_eq!(
            NetKind::of(&net(true, false, false)),
            [External],
            "an external network's owner doesn't make it shared"
        );
        assert_eq!(
            NetKind::of(&net(true, true, false)),
            [External, Shared],
            "both badges when both"
        );
        assert_eq!(badge_count(&NetKind::of(&net(true, true, false))), 2);
        assert_eq!(badge_count(&[Own]), 0);
    }

    #[test]
    fn legend_lists_only_the_kinds_on_screen() {
        assert!(
            legend_kinds(&[net(false, false, true)]).is_empty(),
            "nothing to tell apart"
        );
        assert!(legend_kinds(&[]).is_empty());
        assert_eq!(
            legend_kinds(&[net(true, false, false), net(false, false, true)]),
            [NetKind::Own, NetKind::External]
        );
        assert_eq!(
            legend_kinds(&[net(true, true, false)]),
            [NetKind::External, NetKind::Shared]
        );
        assert_eq!(
            legend_kinds(&[net(false, true, false), net(true, false, false)]),
            [NetKind::External, NetKind::Shared]
        );
    }

    #[test]
    fn net_kind_text_joins_both_flags() {
        assert_eq!(
            net_kind_text(Lang::EnUs, &net(true, true, false)),
            "Network · External · Shared"
        );
        assert_eq!(net_kind_text(Lang::EsEs, &net(false, false, true)), "Red");
        assert_eq!(net_label(Lang::EnUs, &net(true, false, false)), "n, External");
        assert_eq!(net_label(Lang::EnUs, &net(true, true, false)), "n, External, Shared");
        assert_eq!(net_label(Lang::EnUs, &net(false, false, true)), "n");
    }
}
