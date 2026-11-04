//! Pannable, zoomable SVG canvas shared by the *Network Topology* panel and the
//! stack *Topology* tab; its geometry (fit, pan, zoom) is in [`topo_layout`](super::topo_layout).

use leptos::prelude::*;

use super::topo_layout::{self as geo, ViewBox};
use crate::i18n::{use_i18n, K};

/// The view a canvas had, with the drawing size it was for: kept across the
/// re-renders that don't change the layout (*Toggle Labels*, language), so they
/// don't lose the user's zoom and pan.
pub type KeptView = StoredValue<Option<((f64, f64), ViewBox)>>;

/// Pannable, zoomable SVG. `full` is the drawing's size; *Center Topology*
/// (a bump of `center`) shows all of it again. `keep` restores the previous view
/// when the drawing size is the same.
///
/// Its handlers use the `try_` accessors: navigating away disposes the signals while
/// the element still gets `mouseup`/`mouseleave` (a panic there aborts the WASM app).
#[component]
pub fn Canvas(
    full: (f64, f64),
    center: RwSignal<u32>,
    keep: KeptView,
    /// Set once a drag passes 4 px, so the nodes' `click` can tell a pan from a click.
    moved: RwSignal<bool>,
    /// Accessible name of the drawing.
    label: K,
    children: Children,
) -> impl IntoView {
    let i18n = use_i18n();
    let kept = keep.get_value().filter(|(size, _)| *size == full).map(|(_, v)| v);
    let vb = RwSignal::new(kept.unwrap_or((0.0, 0.0, full.0, full.1)));
    let c0 = center.get_untracked();
    let svg = NodeRef::<leptos::svg::Svg>::new();
    let rect = move || {
        svg.get_untracked().map(|e| {
            let r = e.get_bounding_client_rect();
            (r.left(), r.top(), r.width(), r.height())
        })
    };
    // First view: actual size from the top left when fitting everything would make
    // the text too small to read; *Center Topology* always fits everything.
    Effect::new(move |_| {
        let fit = (0.0, 0.0, full.0, full.1);
        let first = center.get() == c0;
        if first && kept.is_some() {
            return;
        }
        vb.set(match rect() {
            Some((_, _, w, h)) if first && geo::fit(fit, w, h).0 < 0.8 => (0.0, 0.0, w, h),
            _ => fit,
        });
    });
    Effect::new(move |_| {
        let v = vb.get();
        keep.try_set_value(Some((full, v)));
    });
    let drag = StoredValue::new(None::<(f64, f64, ViewBox)>);
    let height = full.1.clamp(280.0, 640.0);
    view! {
        <svg node_ref=svg class="topo-canvas" role="group" aria-label=move || i18n.t(label)
            viewBox=move || { let v = vb.get(); format!("{} {} {} {}", v.0, v.1, v.2, v.3) }
            preserveAspectRatio="xMidYMin meet"
            style=format!("height:{height}px")
            on:mousedown=move |ev| {
                if ev.button() == 0 {
                    moved.try_set(false);
                    let Some(v) = vb.try_get_untracked() else { return };
                    drag.try_set_value(Some((ev.client_x() as f64, ev.client_y() as f64, v)));
                }
            }
            on:mousemove=move |ev| {
                let Some((x, y, start)) = drag.try_get_value().flatten() else { return };
                let (dx, dy) = (ev.client_x() as f64 - x, ev.client_y() as f64 - y);
                if !moved.try_get_untracked().unwrap_or(false) && dx.abs() + dy.abs() < 4.0 {
                    return;
                }
                moved.try_set(true);
                let (scale, _) = rect().map(|r| geo::fit(start, r.2, r.3)).unwrap_or((1.0, 0.0));
                vb.try_set(geo::pan(start, dx, dy, start.2 * scale));
            }
            on:mouseup=move |_| { drag.try_set_value(None); }
            on:mouseleave=move |_| { drag.try_set_value(None); }
            // On the element itself (Leptos' `delegation` feature is off), so it is not
            // passive and can stop the page from scrolling.
            on:wheel=move |ev: leptos::ev::WheelEvent| {
                let Some((left, top, w, h)) = rect() else { return };
                ev.prevent_default();
                let Some(v) = vb.try_get_untracked() else { return };
                let (scale, off) = geo::fit(v, w, h);
                let fx = ((ev.client_x() as f64 - left - off) / (v.2 * scale)).clamp(0.0, 1.0);
                let fy = ((ev.client_y() as f64 - top) / (v.3 * scale)).clamp(0.0, 1.0);
                let factor = geo::wheel_factor(ev.delta_y(), ev.delta_mode() == 1, ev.ctrl_key());
                vb.try_set(geo::zoom(v, factor, fx, fy, full));
            }>
            {children()}
        </svg>
    }
}
