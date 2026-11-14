//! Eases a box's height when its content changes size: a dialog going to another wizard step,
//! a field that appears or disappears, a choice list that loads, the result. CSS can only
//! transition `height: auto` in Chromium (`interpolate-size`), so a `ResizeObserver` pins the
//! old height and moves it to the new one, and the box's own `transition` (`--motion`, `0ms`
//! under `prefers-reduced-motion`) eases it. Client-only; nothing happens during SSR.

use leptos::prelude::*;

/// From now until the current owner is disposed, eases every change of `node`'s height with
/// the element's CSS `transition` on `height`. The new height is measured first (after layout,
/// before paint), so the box never shows the new size before easing to it, and a height capped
/// by `max-height` stops at the cap. With no transition (reduced motion), it does nothing.
pub fn ease_height(node: NodeRef<leptos::html::Div>) {
    #[cfg(feature = "hydrate")]
    imp::ease_height(node);
    #[cfg(not(feature = "hydrate"))]
    let _ = node;
}

/// Whether a computed `transition-duration` (`"0s"`, `"0.16s, 0s"`, `"160ms"`) has any
/// non-zero part, i.e. whether the box animates at all.
pub fn has_duration(computed: &str) -> bool {
    computed.split(',').map(str::trim).any(|p| {
        let secs = match p.strip_suffix("ms") {
            Some(ms) => ms.parse::<f64>().map(|v| v / 1000.0),
            None => p.strip_suffix('s').unwrap_or(p).parse::<f64>(),
        };
        secs.is_ok_and(|v| v > 0.0)
    })
}

#[cfg(feature = "hydrate")]
mod imp {
    use std::cell::Cell;
    use std::rc::Rc;

    use leptos::prelude::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use web_sys::{HtmlElement, ResizeObserver, TransitionEvent};

    /// What must live as long as the observed box: the observer and its callbacks (a dropped
    /// `Closure` called from JS throws).
    struct Live {
        observer: ResizeObserver,
        _on_resize: Closure<dyn FnMut()>,
        _on_end: Closure<dyn FnMut(TransitionEvent)>,
    }

    pub fn ease_height(node: NodeRef<leptos::html::Div>) {
        let live = StoredValue::new_local(None::<Live>);
        // Disconnect before the callbacks are dropped with the owner: the dialog may close
        // mid-animation.
        on_cleanup(move || {
            let _ = live.try_update_value(|l| {
                if let Some(l) = l.take() {
                    l.observer.disconnect();
                }
            });
        });
        node.on_load(move |div| {
            let el: HtmlElement = div.unchecked_into();
            // The last height eased to; 0 until the first measure (the opening needs none).
            let last = Rc::new(Cell::new(0));
            // While pinned the observer sees the animation's own frames: ignore them.
            let pinned = Rc::new(Cell::new(false));

            let on_resize = {
                let (el, last, pinned) = (el.clone(), last.clone(), pinned.clone());
                Closure::<dyn FnMut()>::new(move || {
                    if pinned.get() {
                        return;
                    }
                    let new = el.offset_height();
                    let old = last.replace(new);
                    if old == 0 || old == new || !animates(&el) {
                        return;
                    }
                    let s = el.style();
                    let _ = s.set_property("height", &format!("{old}px"));
                    let _ = s.set_property("overflow", "hidden");
                    // Flush the old height so the next change transitions from it.
                    let _ = el.offset_height();
                    let _ = s.set_property("height", &format!("{new}px"));
                    pinned.set(true);
                })
            };
            let on_end = {
                let el = el.clone();
                Closure::<dyn FnMut(TransitionEvent)>::new(move |e: TransitionEvent| {
                    // Transitions of the content bubble up too.
                    let own = e
                        .target()
                        .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                        .is_some_and(|t| t.is_same_node(Some(&el)));
                    if !own || e.property_name() != "height" {
                        return;
                    }
                    let s = el.style();
                    let _ = s.remove_property("height");
                    let _ = s.remove_property("overflow");
                    // Back to `auto`: if the content changed meanwhile, the observer sees the
                    // difference and eases it too.
                    pinned.set(false);
                })
            };
            let f = on_end.as_ref().unchecked_ref();
            let _ = el.add_event_listener_with_callback("transitionend", f);
            let _ = el.add_event_listener_with_callback("transitioncancel", f);
            let Ok(observer) = ResizeObserver::new(on_resize.as_ref().unchecked_ref()) else {
                return;
            };
            observer.observe(&el);
            last.set(el.offset_height());
            let _ = live.try_set_value(Some(Live {
                observer,
                _on_resize: on_resize,
                _on_end: on_end,
            }));
        });
    }

    /// Whether `el` has a running `transition-duration` (not reduced motion).
    fn animates(el: &HtmlElement) -> bool {
        window()
            .get_computed_style(el)
            .ok()
            .flatten()
            .and_then(|s| s.get_property_value("transition-duration").ok())
            .is_some_and(|d| super::has_duration(&d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_duration_does_not_animate() {
        assert!(!has_duration("0s"), "reduced motion: --motion is 0ms");
        assert!(!has_duration("0s, 0ms"));
        assert!(!has_duration(""));
        assert!(has_duration("0.16s"));
        assert!(has_duration("0s, 160ms"));
    }
}
