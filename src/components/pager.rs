//! Controls of the server-paged lists (roadmap 88): the page's query string read into
//! a [`ListQuery`], the filter bar and *Previous* / *Next*. The state lives in the URL
//! (`?marker=…&f=…&q=…`), so a page and its filters can be linked and reloaded; the
//! only thing kept outside it is the stack of markers *Previous* pops.

use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};
use leptos_router::params::ParamsMap;

use std::collections::HashMap;

use crate::api::{get_admin_names, get_choices};
use crate::components::select::Combo;
use crate::i18n::{page_range, use_i18n, K};
use crate::model::{Choice, ChoiceSet, FilterKind, LateName, ListQuery, NameKind, PagedList, BOOT_IMAGE_CHUNK};

/// The markers of the pages walked so far, so *Previous* goes back without asking the
/// API to page backwards; `None` is the first page. Empty on a deep link.
pub type MarkerStack = RwSignal<Vec<Option<String>>>;

/// Names a paged page fills in after its rows show (roadmap 88). `names` is by
/// resource ID: absent = still loading, `None` = not found (the cell shows the ID).
/// `active` is false on a whole list (`page_size = 0`), whose rows come named:
/// nothing is loaded and a missing name is not "loading". It starts true (paging
/// is the default), so the cells show "…" rather than flashing IDs before the
/// page's effect runs.
#[derive(Clone, Copy)]
pub struct LateNames {
    pub names: RwSignal<HashMap<String, Option<LateName>>>,
    pub active: RwSignal<bool>,
}

impl LateNames {
    /// Empty and active, for a page's columns (built before its rows load).
    pub fn new() -> Self {
        LateNames { names: RwSignal::new(HashMap::new()), active: RwSignal::new(true) }
    }
}

impl Default for LateNames {
    fn default() -> Self {
        Self::new()
    }
}

/// `ids` without blanks or repeats, in chunks of [`BOOT_IMAGE_CHUNK`]: one
/// `get_admin_names` GET each, so the query string stays a few KiB.
pub fn late_chunks(mut ids: Vec<String>) -> Vec<Vec<String>> {
    ids.retain(|id| !id.is_empty());
    ids.sort();
    ids.dedup();
    ids.chunks(BOOT_IMAGE_CHUNK).map(<[String]>::to_vec).collect()
}

/// Reads the names of `ids` into `names`, one chunk after another (the server
/// reads each chunk `FAN_OUT` at a time). IDs already known are skipped; a failed
/// chunk marks its IDs not found rather than leaving "…" forever.
pub fn load_late(kind: NameKind, ids: Vec<String>, late: LateNames) {
    let names = late.names;
    let todo: Vec<String> = names.with_untracked(|m| ids.into_iter().filter(|id| !m.contains_key(id)).collect());
    let chunks = late_chunks(todo);
    if chunks.is_empty() {
        return;
    }
    leptos::task::spawn_local(async move {
        for chunk in chunks {
            let found: HashMap<String, LateName> = get_admin_names(kind, chunk.clone()).await.map(|t| t.data.into_iter().map(|n| (n.id.clone(), n)).collect()).unwrap_or_default();
            // The page may be gone by now (navigated away): `try_update` then does nothing.
            let _ = names.try_update(|m| {
                for id in chunk {
                    let n = found.get(&id).cloned();
                    m.insert(id, n);
                }
            });
        }
    });
}

/// A late name as text: "…" while loading, what `pick` takes from it once found
/// (its name, a volume's host), else `missing` (the ID, or "-").
pub fn late_text(late: LateNames, id: String, pick: fn(&LateName) -> Option<String>, missing: String) -> AnyView {
    (move || match late.names.with(|m| m.get(&id).cloned()) {
        None if late.active.get() => view! { <span class="muted">"…"</span> }.into_any(),
        None => missing.clone().into_any(),
        Some(found) => found.as_ref().and_then(pick).unwrap_or_else(|| missing.clone()).into_any(),
    })
    .into_any()
}

/// The name of a [`LateName`], for [`late_text`].
pub fn late_label(n: &LateName) -> Option<String> {
    Some(n.name.clone())
}

/// The host of a [`LateName`] (a volume's), for [`late_text`].
pub fn late_host(n: &LateName) -> Option<String> {
    n.host.clone()
}

/// The list query a page's query string spells (`marker`, `f`, `q`, `sort`, `dir`).
/// Empty values count as absent.
pub fn query_of(m: &ParamsMap) -> ListQuery {
    let get = |k: &str| m.get(k).filter(|v| !v.is_empty());
    ListQuery { marker: get("marker"), field: get("f"), value: get("q"), sort: get("sort"), desc: get("dir").as_deref() == Some("desc") }
}

/// The query string (with its `?`, percent-encoded) of `m` with `marker` replaced
/// (`None` removes it), keeping the filter and sort.
pub fn with_marker(m: &ParamsMap, marker: Option<&str>) -> String {
    let mut m = m.clone();
    match marker {
        // `insert` adds a value to a multi-valued key: *Next* then piled up
        // `?marker=a&marker=b` and the server paged from the wrong one.
        Some(id) => m.replace("marker", id.to_string()),
        None => {
            m.remove("marker");
        }
    };
    m.to_query_string()
}

/// The query string (with its `?`) of a new filter over `m`: the sort survives,
/// the marker doesn't (a filter starts at the first page). An empty value clears
/// the filter.
pub fn with_filter(m: &ParamsMap, field: &str, value: &str) -> String {
    let mut out = ParamsMap::new();
    for k in ["sort", "dir"] {
        if let Some(v) = m.get(k).filter(|v| !v.is_empty()) {
            out.replace(k, v);
        }
    }
    let value = value.trim();
    if !value.is_empty() {
        out.replace("f", field.to_string());
        out.replace("q", value.to_string());
    }
    out.to_query_string()
}

/// 1-based number of a page's first row, or `None` when it can't be known: a page
/// reached by a link with a marker and no history.
pub fn first_row(has_marker: bool, depth: usize, page_size: u32) -> Option<usize> {
    (!has_marker || depth > 0).then(|| depth * page_size as usize + 1)
}

/// The query of the page being shown, from the URL.
pub fn use_list_query() -> Memo<ListQuery> {
    let map = use_query_map();
    Memo::new(move |_| map.with(query_of))
}

/// Navigates to the current path with this query string.
fn go(qs: String) {
    let path = use_location().pathname.get_untracked();
    use_navigate()(&format!("{path}{qs}"), Default::default());
}

/// *Filter by* `[field] [value] [Filter]`: a GET form, so the router navigates to
/// `?f=…&q=…` (no marker: a filter starts at the first page) and the server filters
/// every record. The value is a text box, a list of the API's values, or a project
/// picked by name, according to the field's [`FilterKind`].
#[component]
pub fn FilterBar(list: PagedList) -> impl IntoView {
    let i18n = use_i18n();
    let query = use_list_query();
    let qmap = use_query_map();
    let defs = list.filters();
    let now = query.get_untracked();
    let field = RwSignal::new(now.field.filter(|f| defs.iter().any(|d| d.key == f)).unwrap_or_else(|| defs[0].key.to_string()));
    let value = RwSignal::new(now.value.unwrap_or_default());
    let kind = move || defs.iter().find(|d| d.key == field.get()).map_or(FilterKind::Text, |d| d.kind);
    let projects: RwSignal<Option<Vec<Choice>>> = RwSignal::new(None);
    Effect::new(move |_| {
        if kind() == FilterKind::Project && projects.get_untracked().is_none() {
            leptos::task::spawn_local(async move {
                projects.set(Some(get_choices(ChoiceSet::AllProjects).await.unwrap_or_default()));
            });
        }
    });
    let label = move |key: &str| match key {
        "name" => K::Name,
        "project" => K::Project,
        "host" => K::Host,
        "status" => K::Status,
        "ip" => K::IpAddress,
        "ip6" => K::FilterIp6,
        "flavor" => K::Flavor,
        "image" => K::ImageName,
        "zone" => K::Zone,
        "bootable" => K::Bootable,
        "volume" => K::FilterVolumeId,
        "address" => K::FloatingIp,
        "fixed" => K::MappedFixedIp,
        "network" => K::FilterPoolId,
        "router" => K::FilterRouterId,
        _ => K::FilterSearch,
    };
    let now_sort = query.get_untracked();
    view! {
        // A GET form for a click before hydration; after it, the router navigates
        // (a plain submit reloaded the whole app on every filter).
        <form class="filter-bar" method="get" on:submit=move |ev| {
            ev.prevent_default();
            let qs = qmap.with_untracked(|m| with_filter(m, &field.get_untracked(), &value.get_untracked()));
            go(qs);
        }>
            // The sort survives a new filter; the marker doesn't.
            {now_sort.sort.map(|s| view! { <input type="hidden" name="sort" value=s/> })}
            {now_sort.desc.then(|| view! { <input type="hidden" name="dir" value="desc"/> })}
            <label class="sr-only" for="filter-field">{move || i18n.t(K::FilterBy)}</label>
            <select id="filter-field" name="f" on:change=move |ev| {
                field.set(event_target_value(&ev));
                value.set(String::new());
            }>
                {defs.iter().map(|d| {
                    let (key, k) = (d.key, label(d.key));
                    view! { <option value=key selected=move || field.get() == key>{move || i18n.t(k)}</option> }
                }).collect_view()}
            </select>
            {move || match kind() {
                k @ (FilterKind::Text | FilterKind::Exact) => {
                    // Neutron, IDs and hosts match whole values: say so (roadmap 88).
                    let hint = if k == FilterKind::Exact { K::FilterExact } else { K::FilterSearch };
                    view! {
                        <input type="search" name="q" class="filter-value" prop:value=value
                            placeholder=move || i18n.t(hint)
                            aria-label=move || i18n.t(hint)
                            on:input=move |ev| value.set(event_target_value(&ev))/>
                    }.into_any()
                }
                FilterKind::Choice(options) => view! {
                    <select name="q" aria-label=move || i18n.t(K::FilterSearch)
                        on:change=move |ev| value.set(event_target_value(&ev))>
                        <option value="">{move || i18n.t(K::FilterAny)}</option>
                        {options.iter().map(|o| view! { <option value=*o selected=move || value.get() == *o>{*o}</option> }).collect_view()}
                    </select>
                }.into_any(),
                FilterKind::Project => view! {
                    <input type="hidden" name="q" prop:value=value/>
                    <Combo id="filter-project" options=projects value=value
                        on_change=Callback::new(move |v| value.set(v))
                        aria_label=Signal::derive(move || i18n.t(K::Project).to_string())/>
                }.into_any(),
            }}
            <button type="submit" class="btn">{move || i18n.t(K::FilterApply)}</button>
            {move || (query.get().value.is_some()).then(|| view! {
                <a class="btn" href="?" on:click=move |ev| { ev.prevent_default(); go(String::new()); }>{move || i18n.t(K::Clear)}</a>
            })}
        </form>
    }
}

/// *Previous* / *Next page* and the range shown, under a paged table. `next` is the
/// following page's marker (`None` on the last page); `stack` is the page's marker
/// history. A deep link (a marker, no history) has no known row numbers, and its
/// *Previous* reads *First page*.
#[component]
pub fn Pager(next: Option<String>, shown: usize, count: Option<u64>, page_size: u32, stack: MarkerStack) -> impl IntoView {
    let i18n = use_i18n();
    let map = use_query_map();
    let marker = move || map.with(|m| m.get("marker").filter(|v| !v.is_empty()));
    // Back on the first page (a new filter, the *Clear* link, the browser's back):
    // the history is stale.
    Effect::new(move |_| {
        if marker().is_none() && stack.with_untracked(|s| !s.is_empty()) {
            stack.set(Vec::new());
        }
    });
    let range = move || {
        let lang = i18n.lang();
        match first_row(marker().is_some(), stack.with(Vec::len), page_size) {
            Some(from) => page_range(lang, from, shown, count),
            None => crate::i18n::displaying(lang, shown),
        }
    };
    let prev = move |_| {
        let target = stack.try_update(Vec::pop).flatten().flatten();
        go(map.with_untracked(|m| with_marker(m, target.as_deref())));
    };
    let forward = next.clone();
    let on_next = move |_| {
        if let Some(n) = &forward {
            let here = marker();
            stack.update(|s| s.push(here));
            go(map.with_untracked(|m| with_marker(m, Some(n))));
        }
    };
    view! {
        <div class="pager">
            <span class="range">{range}</span>
            <span class="spacer"></span>
            <button type="button" class="btn" disabled=move || marker().is_none() on:click=prev>
                {move || i18n.t(if stack.with(|s| s.is_empty()) { K::PageFirst } else { K::PagePrev })}
            </button>
            <button type="button" class="btn" disabled=next.is_none() on:click=on_next>{move || i18n.t(K::PageNext)}</button>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> ParamsMap {
        let mut m = ParamsMap::new();
        for (k, v) in pairs {
            m.insert(k.to_string(), v.to_string());
        }
        m
    }

    #[test]
    fn the_query_string_spells_the_list_query() {
        let q = query_of(&map(&[("marker", "abc"), ("f", "host"), ("q", "cmp-1"), ("sort", "name"), ("dir", "desc")]));
        assert_eq!(q, ListQuery { marker: Some("abc".into()), field: Some("host".into()), value: Some("cmp-1".into()), sort: Some("name".into()), desc: true });
        assert_eq!(query_of(&map(&[("marker", ""), ("q", "")])), ListQuery::default(), "empty values are absent");
    }

    #[test]
    fn paging_keeps_the_filter_and_encodes_it() {
        let m = map(&[("f", "name"), ("q", "web & db/01")]);
        let qs = with_marker(&m, Some("m1"));
        let back = query_of(&ParamsMap::from_iter(
            qs.trim_start_matches('?').split('&').map(|kv| {
                let (k, v) = kv.split_once('=').unwrap();
                (k.to_string(), url_decoded(v))
            }),
        ));
        assert_eq!((back.marker.as_deref(), back.value.as_deref()), (Some("m1"), Some("web & db/01")), "{qs}");
        assert!(!qs.contains(' ') && !qs.contains("& "), "the value is percent-encoded: {qs}");
        assert!(!with_marker(&map(&[("marker", "m1")]), None).contains("marker"), "None drops the marker");
        let again = with_marker(&map(&[("marker", "m1")]), Some("m2"));
        assert_eq!(again.matches("marker=").count(), 1, "the next marker replaces the current one, not adds to it: {again}");
        assert!(again.contains("marker=m2"), "{again}");
    }

    fn url_decoded(s: &str) -> String {
        let bytes = s.replace('+', " ");
        let mut out = Vec::new();
        let b = bytes.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' {
                out.push(u8::from_str_radix(&bytes[i + 1..i + 3], 16).unwrap());
                i += 3;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn a_new_filter_keeps_the_sort_and_drops_the_marker() {
        let m = map(&[("marker", "m1"), ("f", "host"), ("q", "old"), ("sort", "name"), ("dir", "desc")]);
        let q = query_of(&parse(&with_filter(&m, "project", " p1 ")));
        assert_eq!(q, ListQuery { marker: None, field: Some("project".into()), value: Some("p1".into()), sort: Some("name".into()), desc: true });
        let cleared = query_of(&parse(&with_filter(&m, "host", "  ")));
        assert_eq!((cleared.field, cleared.value, cleared.sort.as_deref()), (None, None, Some("name")), "an empty value clears the filter, not the sort");
    }

    fn parse(qs: &str) -> ParamsMap {
        ParamsMap::from_iter(qs.trim_start_matches('?').split('&').filter(|kv| !kv.is_empty()).map(|kv| {
            let (k, v) = kv.split_once('=').unwrap();
            (k.to_string(), url_decoded(v))
        }))
    }

    #[test]
    fn late_names_go_in_chunks_each_id_once() {
        let mut ids: Vec<String> = (0..120).map(|i| format!("id{i:03}")).collect();
        ids.push("id000".into());
        ids.push(String::new());
        let chunks = late_chunks(ids);
        assert_eq!(chunks.iter().map(Vec::len).collect::<Vec<_>>(), [50, 50, 20], "repeats and blanks dropped, {BOOT_IMAGE_CHUNK} a request");
        assert!(late_chunks(vec![]).is_empty());
    }

    #[test]
    fn row_numbers_are_known_only_with_a_history() {
        assert_eq!(first_row(false, 0, 100), Some(1));
        assert_eq!(first_row(true, 2, 100), Some(201));
        assert_eq!(first_row(true, 0, 100), None, "a link to page N has no history to count");
    }
}
