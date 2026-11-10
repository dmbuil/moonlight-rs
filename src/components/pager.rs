//! Controls of the server-paged lists (roadmap 88): the page's query string read into
//! a [`ListQuery`], the filter bar and *Previous* / *Next*. The state lives in the URL
//! (`?marker=…&f=…&q=…&sort=…&dir=…`), so a page, its filter and its sort can be
//! linked and reloaded; kept outside it are only the stack of markers *Previous*
//! pops and the result of *Count all* ([`PagerState`]).

use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};
use leptos_router::params::ParamsMap;

use std::collections::HashMap;

use crate::api::{count_rows, get_choices, get_late_names};
use crate::components::dialog::data_version;
use crate::components::select::Combo;
use crate::components::table::{Col, DataTable, Select, ServerSort};
use crate::i18n::{page_range, use_i18n, K};
use crate::model::{Choice, ChoiceSet, FilterKind, LateName, ListQuery, ListScope, NameKind, Page, PagedList, RowCount, Timed, BOOT_IMAGE_CHUNK};

/// What a *Count all* is for: the filter (field, value) and the page's
/// `data_version` when it was asked. A new filter or a write makes it stale.
pub type CountKey = (Option<String>, Option<String>, u64);

/// Where a *Count all* is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counting {
    Running,
    Done(RowCount),
    Failed,
}

/// A paged page's state outside the URL, created once by the page (not by its
/// table, which is rebuilt on every page): `markers` are the pages walked so far,
/// so *Previous* goes back without asking the API to page backwards (`None` is the
/// first page; empty on a deep link); `total` is the last *Count all*.
#[derive(Clone, Copy)]
pub struct PagerState {
    pub markers: RwSignal<Vec<Option<String>>>,
    pub total: RwSignal<Option<(CountKey, Counting)>>,
}

impl PagerState {
    /// No history, nothing counted.
    pub fn new() -> Self {
        PagerState { markers: RwSignal::new(Vec::new()), total: RwSignal::new(None) }
    }
}

impl Default for PagerState {
    fn default() -> Self {
        Self::new()
    }
}

/// The *Count all* key of this query at this data version.
pub fn count_key(q: &ListQuery, version: u64) -> CountKey {
    (q.field.clone(), q.value.clone(), version)
}

/// The total the pager shows: the API's own (`api`, Cinder's), else a finished
/// *Count all* for this very filter and data version; `(n, more)`.
pub fn shown_total(api: Option<u64>, total: Option<&(CountKey, Counting)>, key: &CountKey) -> Option<(u64, bool)> {
    api.map(|n| (n, false)).or_else(|| match total {
        Some((k, Counting::Done(c))) if k == key => Some((c.count, c.more)),
        _ => None,
    })
}

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
/// `get_late_names` GET each, so the query string stays a few KiB.
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
            let found: HashMap<String, LateName> = get_late_names(kind, chunk.clone()).await.map(|t| t.data.into_iter().map(|n| (n.id.clone(), n)).collect()).unwrap_or_default();
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

/// The query string (with its `?`) of a new server-side sort over `m` (`None`:
/// the API's default order). The filter survives; the marker doesn't, since the
/// rows of page *N* change with the order.
pub fn with_sort(m: &ParamsMap, sort: Option<(&str, bool)>) -> String {
    let mut out = ParamsMap::new();
    for k in ["f", "q"] {
        if let Some(v) = m.get(k).filter(|v| !v.is_empty()) {
            out.replace(k, v);
        }
    }
    if let Some((key, desc)) = sort {
        out.replace("sort", key.to_string());
        if desc {
            out.replace("dir", "desc".to_string());
        }
    }
    out.to_query_string()
}

/// The [`ServerSort`] of `list`'s table on this page: the keys its API sorts by
/// and the URL's sort; a header click navigates to the new sort.
pub fn server_sort(list: PagedList) -> ServerSort {
    let qmap = use_query_map();
    let q = qmap.with_untracked(query_of);
    ServerSort {
        keys: list.sorts().iter().map(|d| d.key).collect(),
        current: q.sort.map(|k| (k, q.desc)),
        on_sort: Callback::new(move |sort: Option<(String, bool)>| {
            go(qmap.with_untracked(|m| with_sort(m, sort.as_ref().map(|(k, d)| (k.as_str(), *d)))));
        }),
    }
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
/// picked by name, according to the field's [`FilterKind`]. A Project panel
/// (`scope`) offers no *Project* or *Host* field ([`PagedList::filters_in`]).
#[component]
pub fn FilterBar(list: PagedList, #[prop(optional)] scope: ListScope) -> impl IntoView {
    let i18n = use_i18n();
    let query = use_list_query();
    let qmap = use_query_map();
    let defs = StoredValue::new(list.filters_in(scope));
    let now = query.get_untracked();
    let field = RwSignal::new(defs.with_value(|ds| now.field.filter(|f| ds.iter().any(|d| d.key == f)).unwrap_or_else(|| ds[0].key.to_string())));
    let value = RwSignal::new(now.value.unwrap_or_default());
    let kind = move || defs.with_value(|ds| ds.iter().find(|d| d.key == field.get()).map_or(FilterKind::Text, |d| d.kind));
    // The picked field's choices, loaded when it is chosen; a different `Pick` field
    // reloads them (projects, flavors, images).
    let choices: RwSignal<Option<(ChoiceSet, Vec<Choice>)>> = RwSignal::new(None);
    let options = Signal::derive(move || choices.with(|c| c.as_ref().map(|(_, o)| o.clone())));
    Effect::new(move |_| {
        if let FilterKind::Pick(set) = kind()
            && choices.with_untracked(|c| c.as_ref().map(|(s, _)| *s)) != Some(set)
        {
            choices.set(None);
            let asked = field.get_untracked();
            leptos::task::spawn_local(async move {
                let got = get_choices(set).await.unwrap_or_default();
                // A slower answer for a field left since must not replace the new one's.
                if field.try_get_untracked().as_deref() == Some(asked.as_str()) {
                    let _ = choices.try_set(Some((set, got)));
                }
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
        "shared" => K::Shared,
        "external" => K::External,
        "visibility" => K::Visibility,
        "disk_format" => K::DiskFormat,
        "port" => K::FilterPortId,
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
                {defs.get_value().into_iter().map(|d| {
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
                FilterKind::Pick(_) => view! {
                    <input type="hidden" name="q" prop:value=value/>
                    <Combo id="filter-pick" options=options value=value
                        on_change=Callback::new(move |v| value.set(v))
                        aria_label=Signal::derive(move || i18n.t(label(&field.get())).to_string())/>
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
/// following page's marker (`None` on the last page); `stack` is the page's
/// [`PagerState`]. A deep link (a marker, no history) has no known row numbers, and
/// its *Previous* reads *First page*. Without the API's `count`, a *Count all*
/// link asks `count_rows` for `list`'s total in `scope` on a click (roadmap 88).
#[component]
pub fn Pager(
    list: PagedList,
    next: Option<String>,
    shown: usize,
    count: Option<u64>,
    page_size: u32,
    stack: PagerState,
    #[prop(optional)] scope: ListScope,
) -> impl IntoView {
    let i18n = use_i18n();
    let map = use_query_map();
    let marker = move || map.with(|m| m.get("marker").filter(|v| !v.is_empty()));
    let key = move || count_key(&map.with(query_of), data_version());
    let total = move || stack.total.with(|t| shown_total(count, t.as_ref(), &key()));
    let counting = move || stack.total.with(|t| t.as_ref().filter(|(k, _)| *k == key()).map(|(_, c)| *c));
    let count_all = move |_| {
        let (q, k) = (map.with_untracked(query_of), key());
        stack.total.set(Some((k.clone(), Counting::Running)));
        leptos::task::spawn_local(async move {
            let done = match count_rows(list, scope, q).await {
                Ok(t) => Counting::Done(t.data),
                Err(_) => Counting::Failed,
            };
            // The page may be gone, or the filter changed meanwhile: keep only this count's.
            let _ = stack.total.try_update(|t| {
                if t.as_ref().is_some_and(|(key, _)| *key == k) {
                    *t = Some((k, done));
                }
            });
        });
    };
    let count_link = move || {
        (total().is_none()).then(|| match counting() {
            Some(Counting::Running) => view! { <span class="muted">{move || i18n.t(K::Counting)}</span> }.into_any(),
            failed => view! {
                {(failed == Some(Counting::Failed)).then(|| view! { <span class="muted">{move || i18n.t(K::CountFailed)}</span> " " })}
                <button type="button" class="count-all" on:click=count_all>{move || i18n.t(K::CountAll)}</button>
            }
            .into_any(),
        })
    };
    let stack = stack.markers;
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
            Some(from) => page_range(lang, from, shown, total()),
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
            {count_link}
            <span class="spacer"></span>
            <button type="button" class="btn" disabled=move || marker().is_none() on:click=prev>
                {move || i18n.t(if stack.with(|s| s.is_empty()) { K::PageFirst } else { K::PagePrev })}
            </button>
            <button type="button" class="btn" disabled=next.is_none() on:click=on_next>{move || i18n.t(K::PageNext)}</button>
        </div>
    }
}

/// The neutral server-side sort key of a paged column, by its header: the same
/// header means the same field on every list. `DataTable` uses a key only when
/// the list's API sorts by it (`PagedList::sorts`), so *IP Address* sorts Floating
/// IPs on the server and an instance's addresses on the page.
pub fn server_key(header: K) -> Option<&'static str> {
    Some(match header {
        K::Name | K::InstanceName => "name",
        K::Status => "status",
        K::Size => "size",
        K::Age | K::Created => "created",
        K::Host => "host",
        K::IpAddress => "address",
        _ => return None,
    })
}

/// `cols` with their [`server_key`]s, for a paged table.
pub fn with_server_keys<T>(cols: Vec<Col<T>>) -> Vec<Col<T>> {
    cols.into_iter().map(|c| match server_key(c.header) { Some(k) => c.server_sort(k), None => c }).collect()
}

/// A paged list's [`FilterBar`], and under it, when the server waited for a
/// filter (`[admin] filter_first`, roadmap 89), why the table is empty.
pub fn filter_bar(list: PagedList, scope: ListScope, needs_filter: bool) -> AnyView {
    let i18n = use_i18n();
    let note = needs_filter.then(|| view! { <p class="muted filter-first" role="status">{move || i18n.t(K::FilterFirstNote)}</p> });
    view! { <FilterBar list=list scope=scope/> {note} }.into_any()
}

/// The table of a paged list (roadmap 88), Admin or Project (`scope`): one page
/// with the filter bar and *Previous* / *Next*, or, when the server sent the whole
/// list (`page_size = 0`, or a Project panel with `paged_lists = "admin"`), the
/// client-side table and search box as before. Bulk actions act on the rows shown.
#[allow(clippy::too_many_arguments)]
pub fn paged_table<T: Clone + Send + Sync + 'static>(
    t: Timed<Page<T>>,
    list: PagedList,
    scope: ListScope,
    cols: Vec<Col<T>>,
    search: fn(&T) -> String,
    select: Option<Select<T>>,
    drawer: Option<fn(&T, crate::i18n::Lang) -> AnyView>,
    stack: PagerState,
    refresh: Callback<()>,
) -> AnyView {
    let page = t.data;
    let timing = (t.millis, t.calls);
    // `select` is an optional prop that takes a `Select`, not an `Option`.
    if !page.paged {
        return match select {
            Some(select) => view! { <DataTable rows=page.rows columns=cols search=search timing=timing on_refresh=refresh select=select drawer=drawer/> }.into_any(),
            None => view! { <DataTable rows=page.rows columns=cols search=search timing=timing on_refresh=refresh drawer=drawer/> }.into_any(),
        };
    }
    let footer = view! { <Pager list=list next=page.next.clone() shown=page.rows.len() count=page.count page_size=page.page_size stack=stack scope=scope/> }.into_any();
    let bar = filter_bar(list, scope, page.needs_filter);
    let sort = server_sort(list);
    let cols = with_server_keys(cols);
    match select {
        Some(select) => view! {
            <DataTable rows=page.rows columns=cols search=search timing=timing on_refresh=refresh select=select drawer=drawer paged=true extra=bar footer=footer server_sort=sort/>
        }
        .into_any(),
        None => view! { <DataTable rows=page.rows columns=cols search=search timing=timing on_refresh=refresh drawer=drawer paged=true extra=bar footer=footer server_sort=sort/> }.into_any(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_server_sort_key_comes_from_a_header() {
        use crate::model::PagedList;
        let all = [PagedList::Instances, PagedList::Volumes, PagedList::Snapshots, PagedList::FloatingIps, PagedList::Networks, PagedList::Routers, PagedList::Trunks, PagedList::Images, PagedList::VolumeGroups, PagedList::GroupSnapshots];
        let headers = [K::Name, K::Status, K::Size, K::Age, K::Created, K::Host, K::IpAddress];
        let keys: Vec<_> = headers.iter().filter_map(|h| server_key(*h)).collect();
        for l in all {
            for d in l.sorts() {
                assert!(keys.contains(&d.key), "{l:?}: no header sorts by {:?}", d.key);
            }
        }
        assert_eq!(server_key(K::Actions), None);
    }

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
    fn a_new_sort_keeps_the_filter_and_drops_the_marker() {
        let m = map(&[("marker", "m1"), ("f", "host"), ("q", "c1"), ("sort", "name")]);
        let q = query_of(&parse(&with_sort(&m, Some(("created", true)))));
        assert_eq!(q, ListQuery { marker: None, field: Some("host".into()), value: Some("c1".into()), sort: Some("created".into()), desc: true });
        let q = query_of(&parse(&with_sort(&m, None)));
        assert_eq!((q.sort, q.desc, q.value.as_deref()), (None, false, Some("c1")), "back to the API's order, still filtered");
    }

    #[test]
    fn a_count_shows_only_for_its_own_filter_and_data() {
        let q = ListQuery { field: Some("host".into()), value: Some("c1".into()), ..Default::default() };
        let key = count_key(&q, 3);
        let done = |k: CountKey| Some((k, Counting::Done(RowCount { count: 42, more: false })));
        assert_eq!(shown_total(None, done(key.clone()).as_ref(), &key), Some((42, false)));
        assert_eq!(shown_total(Some(7), done(key.clone()).as_ref(), &key), Some((7, false)), "the API's own count wins");
        let other_filter = count_key(&ListQuery { value: Some("c2".into()), ..q.clone() }, 3);
        assert_eq!(shown_total(None, done(key.clone()).as_ref(), &other_filter), None, "a new filter needs a new count");
        assert_eq!(shown_total(None, done(key.clone()).as_ref(), &count_key(&q, 4)), None, "a write makes it stale");
        let paged_on = count_key(&ListQuery { marker: Some("m".into()), sort: Some("name".into()), ..q.clone() }, 3);
        assert_eq!(shown_total(None, done(key.clone()).as_ref(), &paged_on), Some((42, false)), "the next page or another sort keep it");
        assert_eq!(shown_total(None, Some(&(key.clone(), Counting::Running)), &key), None);
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
