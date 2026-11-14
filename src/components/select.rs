//! A themed single-select that replaces the native `<select>` in dialogs: the
//! WAI-ARIA *combobox + listbox* pattern, with a search box for long lists,
//! keyboard navigation (↑/↓, Home/End, Enter, Esc, type-ahead) and a popup
//! positioned with `position: fixed` so scrolling containers do not clip it.

use leptos::prelude::*;

use crate::i18n::{K, use_i18n};
use crate::model::Choice;

/// Lists at least this long get a search box.
const SEARCH_FROM: usize = 8;

/// Whether a combo of `len` options (with *None*) shows its search box: from
/// [`SEARCH_FROM`] options, or always when the caller asks (`search`: the
/// floating IP port pickers, roadmap 110).
pub fn shows_search(search: bool, len: usize) -> bool {
    search || len >= SEARCH_FROM
}

/// Options whose label contains every term of `query` (case-insensitive).
pub fn filter_choices(list: &[Choice], query: &str) -> Vec<Choice> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    list.iter()
        .filter(|c| {
            let l = c.label.to_lowercase();
            terms.iter().all(|t| l.contains(t))
        })
        .cloned()
        .collect()
}

/// Next active index for a navigation key; `None` for any other key.
pub fn step_index(cur: Option<usize>, len: usize, key: &str) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let last = len - 1;
    match key {
        "ArrowDown" => Some(cur.map_or(0, |i| (i + 1).min(last))),
        "ArrowUp" => Some(cur.map_or(last, |i| i.saturating_sub(1))),
        "Home" => Some(0),
        "End" => Some(last),
        _ => None,
    }
}

/// Type-ahead: the first option after `from` (wrapping) whose label starts
/// with `prefix`, case-insensitive.
pub fn typeahead(list: &[Choice], prefix: &str, from: usize) -> Option<usize> {
    let p = prefix.to_lowercase();
    if p.is_empty() || list.is_empty() {
        return None;
    }
    (1..=list.len())
        .map(|k| (from + k) % list.len())
        .find(|&i| list[i].label.to_lowercase().starts_with(&p))
}

#[component]
pub fn Combo(
    #[prop(into)] id: String,
    /// `None` while the options are loading.
    #[prop(into)]
    options: Signal<Option<Vec<Choice>>>,
    #[prop(into)] value: Signal<String>,
    on_change: Callback<String>,
    /// Without it, a "None" option clears the value.
    #[prop(optional)]
    required: bool,
    /// The empty choice's label; *— None —* when unset. On a required combo
    /// it is the placeholder shown while nothing is picked (*— Select —* when unset).
    #[prop(default = None)]
    empty_label: Option<String>,
    /// Accessible name when no `<label for=id>` names it (e.g. a table cell).
    #[prop(optional, into)]
    aria_label: Option<Signal<String>>,
    /// Picking toggles a tick and keeps the list open; its *Add (n)* footer then calls
    /// `on_change` once with the ticked values as a JSON list (roadmap 25: several catalog
    /// properties at once). Closing any other way drops the ticks.
    #[prop(optional)]
    multi: bool,
    /// Always show the search box, however short the list (else from [`SEARCH_FROM`]).
    #[prop(optional)]
    search: bool,
) -> impl IntoView {
    let i18n = use_i18n();
    let empty_label = StoredValue::new(empty_label);
    let open = RwSignal::new(false);
    let query = RwSignal::new(String::new());
    let active: RwSignal<Option<usize>> = RwSignal::new(None);
    let pos = RwSignal::new(String::new());
    let trigger: NodeRef<leptos::html::Button> = NodeRef::new();
    let force_search = search;
    let search: NodeRef<leptos::html::Input> = NodeRef::new();
    // Copy handles, so the nested `<Show>` closures stay `Fn`.
    let base = StoredValue::new(id.clone());
    let list_id = move || format!("{}-list", base.get_value());
    let opt_id = move |i: usize| format!("{}-opt-{i}", base.get_value());

    // All options, with "None" first for optional fields.
    let all = move || {
        options.get().map(|mut l| {
            if !required {
                let label = empty_label
                    .get_value()
                    .unwrap_or_else(|| i18n.t(K::NoneOption).to_string());
                l.insert(0, Choice::new("", label));
            }
            l
        })
    };
    let searchable = move || all().is_some_and(|l| shows_search(force_search, l.len()));
    let shown = move || all().map(|l| filter_choices(&l, &query.get())).unwrap_or_default();
    let label = move || match all() {
        None => i18n.t(K::LoadingChoices).to_string(),
        Some(l) => {
            let v = value.get();
            // Nothing picked: the empty label, else *— Select —* on required ones
            // (there is no *None* to pick) and *— None —* on optional ones.
            let fallback = if required { K::SelectChoice } else { K::NoneOption };
            l.iter()
                .find(|c| c.value == v)
                .map(|c| c.label.clone())
                .unwrap_or_else(|| empty_label.get_value().unwrap_or_else(|| i18n.t(fallback).to_string()))
        }
    };

    let ticked: RwSignal<Vec<String>> = RwSignal::new(Vec::new());
    let close = move |refocus: bool| {
        open.set(false);
        query.set(String::new());
        ticked.set(Vec::new());
        if refocus && let Some(t) = trigger.get_untracked() {
            let _ = t.focus();
        }
    };
    let pick = move |c: Choice| {
        if multi {
            ticked.update(|t| match t.iter().position(|v| *v == c.value) {
                Some(i) => {
                    t.remove(i);
                }
                None => t.push(c.value),
            });
            return;
        }
        on_change.run(c.value);
        close(true);
    };
    let confirm = move |_| {
        let values = ticked.get_untracked();
        if !values.is_empty() {
            // Close first: `on_change` may re-render the owner (the metadata editor rebuilds its
            // picker as rows are added), which disposes this combo's signals.
            close(true);
            on_change.run(serde_json::to_string(&values).unwrap_or_default());
        }
    };
    let show = move || {
        if options.get_untracked().is_none() {
            return;
        }
        let v = value.get_untracked();
        active.set(shown().iter().position(|c| c.value == v).or(Some(0)));
        pos.set(place(trigger));
        open.set(true);
        #[cfg(feature = "hydrate")]
        request_animation_frame(move || {
            if let Some(s) = search.get_untracked() {
                let _ = s.focus();
            }
        });
    };

    // Keys while the list is open (from the trigger or the search box).
    let on_key = move |ev: leptos::ev::KeyboardEvent| {
        let key = ev.key();
        if !open.get_untracked() {
            match key.as_str() {
                "ArrowDown" | "ArrowUp" | "Enter" | " " => {
                    ev.prevent_default();
                    show();
                }
                k if k.chars().count() == 1 => {
                    // Closed type-ahead, like a native select.
                    let list = all().unwrap_or_default();
                    let cur = list.iter().position(|c| c.value == value.get_untracked()).unwrap_or(0);
                    if let Some(i) = typeahead(&list, k, cur) {
                        on_change.run(list[i].value.clone());
                    }
                }
                _ => {}
            }
            return;
        }
        let list = shown();
        if let Some(i) = step_index(active.get_untracked(), list.len(), &key) {
            ev.prevent_default();
            active.set(Some(i));
            scroll_into_view(&opt_id(i));
            return;
        }
        match key.as_str() {
            "Enter" => {
                ev.prevent_default();
                if let Some(c) = active.get_untracked().and_then(|i| list.get(i).cloned()) {
                    pick(c);
                }
            }
            "Escape" => {
                // Close the list only, not the dialog around it.
                ev.prevent_default();
                ev.stop_propagation();
                close(true);
            }
            "Tab" => close(false),
            k if k.chars().count() == 1 && search.get_untracked().is_none() => {
                if let Some(i) = typeahead(&list, k, active.get_untracked().unwrap_or(0)) {
                    active.set(Some(i));
                    scroll_into_view(&opt_id(i));
                }
            }
            _ => {}
        }
    };

    let on_key = StoredValue::new(on_key);
    let key = move |ev: leptos::ev::KeyboardEvent| on_key.with_value(|f| f(ev));
    view! {
        <div class="combo">
            <button type="button" class="combo-trigger" id=id.clone() node_ref=trigger
                role="combobox" aria-haspopup="listbox" aria-controls=list_id
                aria-expanded=move || open.get().to_string()
                aria-activedescendant=move || if open.get() && !searchable() { active.get().map(opt_id) } else { None }
                data-value=move || value.get()
                aria-label=move || aria_label.map(|l| l.get())
                on:click=move |_| if open.get_untracked() { close(true) } else { show() }
                on:keydown=key>
                <span class="combo-label">{label}</span>
                <svg aria-hidden="true" viewBox="0 0 10 6" width="10" height="6"><path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>
            </button>
            <Show when=move || open.get()>
                <div class="combo-veil" on:mousedown=move |_| close(false)></div>
                <div class="combo-pop" style=move || pos.get()>
                    <Show when=searchable>
                        <input type="search" class="combo-search" node_ref=search
                            placeholder=move || i18n.t(K::FilterPlaceholder)
                            aria-controls=list_id aria-autocomplete="list"
                            aria-activedescendant=move || active.get().map(opt_id)
                            prop:value=move || query.get()
                            on:input=move |ev| {
                                query.set(event_target_value(&ev));
                                active.set(Some(0));
                            }
                            on:keydown=key/>
                    </Show>
                    <ul role="listbox" id=list_id class="combo-list">
                        {
                            move || {
                                let list = shown();
                                if list.is_empty() {
                                    return view! { <li class="combo-empty">{i18n.t(K::NoMatches)}</li> }.into_any();
                                }
                                let current = value.get();
                                list.into_iter().enumerate().map(|(i, c)| {
                                    let selected = if multi { ticked.with(|t| t.contains(&c.value)) } else { c.value == current };
                                    let (c2, v2) = (c.clone(), c.value.clone());
                                    view! {
                                        <li role="option" id=opt_id(i) data-value=c.value.clone()
                                            aria-selected=move || if multi { ticked.with(|t| t.contains(&v2)).to_string() } else { selected.to_string() }
                                            class:active=move || active.get() == Some(i)
                                            on:mousemove=move |_| active.set(Some(i))
                                            on:mousedown=move |ev| { ev.prevent_default(); pick(c2.clone()) }>
                                            {c.label}
                                        </li>
                                    }
                                }).collect_view().into_any()
                            }
                        }
                    </ul>
                    {multi.then(|| view! {
                        <div class="combo-foot">
                            <button type="button" class="btn btn-sm" disabled=move || ticked.with(Vec::is_empty) on:click=confirm>
                                {move || format!("{} ({})", i18n.t(K::AddEntry), ticked.with(Vec::len))}
                            </button>
                        </div>
                    })}
                </div>
            </Show>
        </div>
    }
}

/// `left/top/width` for the popup under (or above) the trigger.
fn place(trigger: NodeRef<leptos::html::Button>) -> String {
    #[cfg(feature = "hydrate")]
    {
        let Some(t) = trigger.get_untracked() else {
            return String::new();
        };
        let r = t.get_bounding_client_rect();
        let vh = window().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
        let below = vh - r.bottom();
        // Room for ~8 rows + search; flip up when the bottom is tight.
        if below < 300.0 && r.top() > below {
            format!(
                "left:{}px;bottom:{}px;width:{}px",
                r.left(),
                vh - r.top() + 4.0,
                r.width()
            )
        } else {
            format!("left:{}px;top:{}px;width:{}px", r.left(), r.bottom() + 4.0, r.width())
        }
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = trigger;
        String::new()
    }
}

fn scroll_into_view(_id: &str) {
    #[cfg(feature = "hydrate")]
    if let Some(el) = document().get_element_by_id(_id) {
        el.scroll_into_view_with_bool(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_lists_get_a_search_box_only_when_asked() {
        assert!(!shows_search(false, 3));
        assert!(shows_search(false, SEARCH_FROM));
        assert!(shows_search(true, 1));
        assert!(shows_search(true, 0));
    }

    fn list(labels: &[&str]) -> Vec<Choice> {
        labels.iter().map(|l| Choice::new(*l, *l)).collect()
    }

    #[test]
    fn filter_needs_every_term() {
        let l = list(&["ADM-RHEL9.7-ES", "Debian 12", "debian-13"]);
        assert_eq!(filter_choices(&l, "deb 13").len(), 1);
        assert_eq!(filter_choices(&l, "  ").len(), 3);
    }

    #[test]
    fn keys_move_within_bounds() {
        assert_eq!(step_index(None, 3, "ArrowDown"), Some(0));
        assert_eq!(step_index(Some(2), 3, "ArrowDown"), Some(2));
        assert_eq!(step_index(Some(0), 3, "ArrowUp"), Some(0));
        assert_eq!(step_index(None, 3, "End"), Some(2));
        assert_eq!(step_index(Some(1), 3, "a"), None);
        assert_eq!(step_index(None, 0, "ArrowDown"), None);
    }

    #[test]
    fn typeahead_wraps_after_the_current_option() {
        let l = list(&["alh1", "mad1", "mer1", "vld1"]);
        assert_eq!(typeahead(&l, "m", 0), Some(1));
        assert_eq!(typeahead(&l, "m", 1), Some(2));
        assert_eq!(typeahead(&l, "m", 2), Some(1));
        assert_eq!(typeahead(&l, "x", 0), None);
    }
}
