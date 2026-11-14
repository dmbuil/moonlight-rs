//! A generic, client-side filterable and sortable table, with Horizon's
//! "Displaying N items" header and footer. On a server-paged list
//! the columns the API can sort by sort on the server instead ([`ServerSort`]).

use std::sync::Arc;

use leptos::prelude::*;

use crate::i18n::{K, Lang, displaying, use_i18n};
use crate::model::{Capabilities, Requires};

/// What a column sorts by: a number, or text.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SortKey {
    Num(i64),
    Text(String),
}

/// Renders a row's cell in the given language.
type CellFn<T> = Arc<dyn Fn(&T, Lang) -> AnyView + Send + Sync>;
/// A row's sort key for a column.
type SortFn<T> = Arc<dyn Fn(&T) -> SortKey + Send + Sync>;

/// One column of a [`DataTable`]: header, cell renderer, and optionally a sort key,
/// a CSS class and a capability it needs.
pub struct Col<T> {
    pub header: K,
    pub cell: CellFn<T>,
    pub sort: Option<SortFn<T>>,
    pub class: &'static str,
    /// Shown only when the cloud offers it (e.g. a Neutron extension).
    pub requires: Requires,
    /// Neutral sort key (`model::SortDef::key`) of this column on a server-paged
    /// list; used only when the table's [`ServerSort`] lists it.
    pub server: Option<&'static str>,
}

impl<T> Clone for Col<T> {
    fn clone(&self) -> Self {
        Self {
            header: self.header,
            cell: self.cell.clone(),
            sort: self.sort.clone(),
            class: self.class,
            requires: self.requires,
            server: self.server,
        }
    }
}

/// How a server-paged table sorts on the server: the neutral keys
/// the API sorts by, the URL's current sort (`(key, descending)`) and what a
/// header click asks for (`None` = the API's default order).
#[derive(Clone)]
pub struct ServerSort {
    pub keys: Vec<&'static str>,
    pub current: Option<(String, bool)>,
    pub on_sort: Callback<Option<(String, bool)>>,
}

/// The sort after a click on `key`'s header: ascending, then descending, then the
/// API's default order, as the client-side headers cycle.
pub fn next_server_sort(current: Option<(&str, bool)>, key: &str) -> Option<(String, bool)> {
    match current {
        Some((k, false)) if k == key => Some((key.to_string(), true)),
        Some((k, true)) if k == key => None,
        _ => Some((key.to_string(), false)),
    }
}

/// A column with this header and cell renderer (not sortable until `sort_*`).
pub fn col<T, V>(header: K, cell: impl Fn(&T, Lang) -> V + Send + Sync + 'static) -> Col<T>
where
    V: IntoView + 'static,
{
    Col {
        header,
        cell: Arc::new(move |t, l| cell(t, l).into_any()),
        sort: None,
        class: "",
        requires: Requires::Always,
        server: None,
    }
}

/// The columns the cloud offers; read `caps` reactively so they follow detection.
pub fn available<T>(cols: Vec<Col<T>>, caps: &Capabilities) -> Vec<Col<T>> {
    cols.into_iter().filter(|c| c.requires.met(caps)).collect()
}

impl<T> Col<T> {
    /// Sortable by this text.
    pub fn sort_text(mut self, f: impl Fn(&T) -> String + Send + Sync + 'static) -> Self {
        self.sort = Some(Arc::new(move |t| SortKey::Text(f(t).to_lowercase())));
        self
    }
    /// Sortable by this number.
    pub fn sort_num(mut self, f: impl Fn(&T) -> i64 + Send + Sync + 'static) -> Self {
        self.sort = Some(Arc::new(move |t| SortKey::Num(f(t))));
        self
    }
    /// CSS class on the column's cells (`num`, `nowrap`, `actions`…).
    pub fn class(mut self, c: &'static str) -> Self {
        self.class = c;
        self
    }
    /// Hidden when the cloud lacks `r`.
    pub fn requires(mut self, r: Requires) -> Self {
        self.requires = r;
        self
    }
    /// Sorts on the server under this neutral key on a paged list whose API can
    /// (see [`ServerSort`]); elsewhere the column keeps its client-side sort.
    pub fn server_sort(mut self, key: &'static str) -> Self {
        self.server = Some(key);
        self
    }
}

/// Row checkboxes for bulk actions. `key` gives `(id, name)` for rows that can
/// be selected (`None` = checkbox disabled, e.g. an image of another project);
/// `picked` holds the selection in click order and is read by the page's bulk
/// button.
pub struct Select<T> {
    pub key: fn(&T) -> Option<(String, String)>,
    pub picked: RwSignal<Vec<(String, String)>>,
}

impl<T> Clone for Select<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Select<T> {}

/// Adds or removes one row.
pub fn toggle(picked: &mut Vec<(String, String)>, row: &(String, String), on: bool) {
    picked.retain(|(id, _)| *id != row.0);
    if on {
        picked.push(row.clone());
    }
}

/// The header checkbox: selects every visible row, or clears them (rows hidden
/// by the filter keep their state).
pub fn toggle_all(picked: &mut Vec<(String, String)>, visible: &[(String, String)], on: bool) {
    for row in visible {
        toggle(picked, row, on);
    }
}

/// Drops picks whose row is gone (deleted, or filtered out by a refetch).
/// Returns whether anything changed.
pub fn retain_present(picked: &mut Vec<(String, String)>, present: &[&str]) -> bool {
    let before = picked.len();
    picked.retain(|(id, _)| present.contains(&id.as_str()));
    picked.len() != before
}

/// Renders the drawer opened under a row.
pub type DrawerFn<T> = fn(&T, Lang) -> AnyView;

/// Opens or closes the drawer of the row with key `k` (its [`row_keys`] entry,
/// which neither a sort nor a refetch changes, so drawers stay open across both).
pub fn toggle_open<K: PartialEq + Clone>(open: &mut Vec<K>, k: &K) {
    if let Some(p) = open.iter().position(|x| x == k) {
        open.remove(p);
    } else {
        open.push(k.clone());
    }
}

/// Closes the drawers of rows the filter hides or a refetch removed. Returns
/// whether anything changed.
pub fn retain_visible<K: PartialEq>(open: &mut Vec<K>, visible: &[K]) -> bool {
    let before = open.len();
    open.retain(|k| visible.contains(k));
    open.len() != before
}

/// A stable key per row, for the keyed `<For>` and the open
/// drawers: the row's ID when it has one (`ids[i]`), else its search text. A
/// repeated key gets `#2`, `#3`… so every key is unique, as `<For>` needs.
pub fn row_keys(ids: &[Option<String>], haystacks: &[String]) -> Vec<String> {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    ids.iter()
        .zip(haystacks)
        .map(|(id, hay)| {
            let base = id.clone().unwrap_or_else(|| format!("~{hay}"));
            let n = seen.entry(base.clone()).or_insert(0);
            *n += 1;
            if *n == 1 { base } else { format!("{base}#{n}") }
        })
        .collect()
}

/// What a table remembers when it is rebuilt: every refetch (a
/// dialog's `data_version` bump, a polling round) renders the page's
/// `DataTable` anew, and with state of its own the filter, sort and open
/// drawers were lost each time. Reference-counted signals, so they outlive the
/// table that uses them and go away with the map entry.
#[derive(Clone)]
struct TableState {
    /// What the filter box shows; `filter` follows it after `FILTER_DEBOUNCE_MS`.
    typed: ArcRwSignal<String>,
    filter: ArcRwSignal<String>,
    sort: ArcRwSignal<Option<(usize, bool)>>,
    /// [`row_keys`] of the rows whose drawer is open.
    open: ArcRwSignal<Vec<String>>,
}

impl TableState {
    /// An empty filter, no sort, no drawer open.
    fn new() -> Self {
        TableState {
            typed: ArcRwSignal::new(String::new()),
            filter: ArcRwSignal::new(String::new()),
            sort: ArcRwSignal::new(None),
            open: ArcRwSignal::new(Vec::new()),
        }
    }
}

/// The table states of the page being shown, provided once by `Layout`, keyed
/// by [`table_key`]. Only the current path's are kept: leaving a page forgets
/// its filters, as in Horizon.
#[derive(Clone, Default)]
pub struct TableStates(Arc<std::sync::Mutex<std::collections::HashMap<String, TableState>>>);

impl TableStates {
    /// Provides the empty map as context (once, in `Layout`).
    pub fn provide() {
        provide_context(TableStates::default());
    }

    /// The state of table `key` on page `path`, created on first use; the states of other pages are
    /// dropped here, which is how leaving a page forgets them. A poisoned lock is taken over (the
    /// map holds only UI state).
    fn get(&self, path: &str, key: String) -> TableState {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let prefix = format!("{path}#");
        map.retain(|k, _| k.starts_with(&prefix));
        map.entry(key).or_insert_with(TableState::new).clone()
    }
}

/// Identifies one table of a page across rebuilds: the path, the table's search
/// function and its column headers. Two tables of one page share a key only if
/// they search and show the same thing.
pub fn table_key(path: &str, search: usize, headers: &[K]) -> String {
    format!("{path}#{search:x}#{headers:?}")
}

/// How long the filter box waits after the last keystroke before filtering
///, so typing in a long list stays smooth.
#[cfg(feature = "hydrate")]
const FILTER_DEBOUNCE_MS: u64 = 150;

/// Indices of the rows to show, filtered by `query` and sorted by `sort`.
/// Pure, so it is unit-tested.
pub fn visible_rows(haystacks: &[String], keys: Option<&[SortKey]>, query: &str, ascending: bool) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    let terms: Vec<&str> = q.split_whitespace().collect();
    let mut idx: Vec<usize> = (0..haystacks.len())
        .filter(|&i| terms.iter().all(|t| haystacks[i].contains(t)))
        .collect();
    if let Some(keys) = keys {
        idx.sort_by(|&a, &b| {
            let o = keys[a].cmp(&keys[b]);
            if ascending { o } else { o.reverse() }
        });
    }
    idx
}

/// The list table of every panel: toolbar (filter, count, refresh, timing), sortable headers,
/// optional checkboxes and drawers, and Horizon's *Displaying N items* line. Its filter, sort and
/// open drawers live in `TableStates`, so a refetch that rebuilds it keeps them.
#[component]
pub fn DataTable<T>(
    rows: Vec<T>,
    columns: Vec<Col<T>>,
    /// Text the filter box searches in (all terms must match).
    search: fn(&T) -> String,
    /// (millis, calls) shown discreetly in the toolbar.
    #[prop(optional)]
    timing: Option<(u64, u32)>,
    #[prop(optional, into)] on_refresh: Option<Callback<()>>,
    /// Adds a checkbox column for bulk actions.
    #[prop(optional)]
    select: Option<Select<T>>,
    /// Extra controls in the toolbar, just before the filter (e.g. a date range).
    #[prop(optional)]
    extra: Option<AnyView>,
    /// A chevron toggle at the start of each row that opens this view in a full-width
    /// row under it, like Horizon's Angular tables. Drawers close on refetch
    /// (the table is rebuilt) and when the filter hides their row.
    #[prop(default = None)]
    drawer: Option<DrawerFn<T>>,
    /// A server-paged list: the rows are one page and the page's filter
    /// bar (in `extra`) searches every record on the server, so the client-side
    /// search box, which would only see this page, is not drawn.
    #[prop(optional)]
    paged: bool,
    /// Under the table, in place of nothing: the page's *Previous* / *Next*.
    #[prop(optional)]
    footer: Option<AnyView>,
    /// A server-paged list's sort: its columns with a key in `keys` sort every
    /// record on the server; the other sortable ones sort the page, and say so.
    #[prop(optional)]
    server_sort: Option<ServerSort>,
) -> impl IntoView
where
    T: Clone + Send + Sync + 'static,
{
    let i18n = use_i18n();
    let haystacks: Vec<String> = rows.iter().map(|r| search(r).to_lowercase()).collect();
    let keys: Vec<Option<(String, String)>> = select.map(|s| rows.iter().map(s.key).collect()).unwrap_or_default();
    if let Some(s) = select {
        let present: Vec<&str> = keys.iter().flatten().map(|(id, _)| id.as_str()).collect();
        // Untracked check first so an unchanged selection doesn't notify the bulk button.
        if s.picked
            .with_untracked(|p| p.iter().any(|(id, _)| !present.contains(&id.as_str())))
        {
            s.picked.update(|p| {
                retain_present(p, &present);
            });
        }
    }
    let ids: Vec<Option<String>> = if keys.is_empty() {
        vec![None; rows.len()]
    } else {
        keys.iter().map(|k| k.as_ref().map(|(id, _)| id.clone())).collect()
    };
    let row_keys = StoredValue::new(row_keys(&ids, &haystacks));
    // Filter, sort and open drawers survive the rebuild a refetch causes.
    let state = use_context::<TableStates>()
        .map(|st| {
            let path = leptos_router::hooks::use_location().pathname.get_untracked();
            let headers: Vec<K> = columns.iter().map(|c| c.header).collect();
            st.get(&path, table_key(&path, search as usize, &headers))
        })
        .unwrap_or_else(TableState::new);
    let keys = StoredValue::new(keys);
    let rows = StoredValue::new(rows);
    let haystacks = StoredValue::new(haystacks);
    let columns = StoredValue::new(columns);
    let filter_arc = state.filter.clone();
    let filter: RwSignal<String> = state.filter.into();
    let sort: RwSignal<Option<(usize, bool)>> = state.sort.into();
    let ncols = columns.with_value(|c| c.len()) + select.is_some() as usize + drawer.is_some() as usize;
    let open: RwSignal<Vec<String>> = state.open.into();
    let typed: RwSignal<String> = state.typed.into();
    #[cfg(feature = "hydrate")]
    let pending = StoredValue::new(None::<TimeoutHandle>);
    // Through the shared signal, so a keystroke still lands if a refetch rebuilds the table
    // within the pause.
    let on_type = move |v: String| {
        typed.set(v.clone());
        #[cfg(feature = "hydrate")]
        {
            let filter = filter_arc.clone();
            let h = set_timeout_with_handle(
                move || filter.set(v),
                std::time::Duration::from_millis(FILTER_DEBOUNCE_MS),
            )
            .ok();
            if let Some(old) = pending.try_update_value(|p| std::mem::replace(p, h)).flatten() {
                old.clear();
            }
        }
        #[cfg(not(feature = "hydrate"))]
        filter_arc.set(v);
    };

    let visible = Memo::new(move |_| {
        let q = filter.get();
        let s = sort.get();
        rows.with_value(|rows| {
            haystacks.with_value(|h| {
                let keys: Option<Vec<SortKey>> = s.and_then(|(ci, _)| {
                    columns
                        .with_value(|c| c[ci].sort.clone())
                        .map(|f| rows.iter().map(|r| f(r)).collect())
                });
                visible_rows(h, keys.as_deref(), &q, s.is_none_or(|(_, asc)| asc))
            })
        })
    });

    if drawer.is_some() {
        Effect::new(move |_| {
            let vis: Vec<String> = row_keys.with_value(|k| visible.get().into_iter().map(|i| k[i].clone()).collect());
            // Untracked check first so an unchanged set doesn't re-render the drawers.
            if open.with_untracked(|o| o.iter().any(|k| !vis.contains(k))) {
                open.update(|o| {
                    retain_visible(o, &vis);
                });
            }
        });
    }

    // Selectable rows currently visible, for the header checkbox.
    let visible_keys = move || -> Vec<(String, String)> {
        keys.with_value(|k| {
            visible
                .get()
                .into_iter()
                .filter_map(|i| k.get(i).cloned().flatten())
                .collect()
        })
    };
    let select_header = move || {
        select.map(|s| {
            let state = move || {
                let vis = visible_keys();
                let n = s
                    .picked
                    .with(|p| vis.iter().filter(|r| p.iter().any(|(id, _)| *id == r.0)).count());
                (n > 0 && n == vis.len(), n > 0 && n < vis.len(), vis.is_empty())
            };
            view! {
                <th class="select">
                    <input type="checkbox" aria-label=move || i18n.t(K::SelectAllRows)
                        prop:checked=move || state().0
                        prop:indeterminate=move || state().1
                        disabled=move || state().2
                        on:change=move |ev| {
                            let on = event_target_checked(&ev);
                            let vis = visible_keys();
                            s.picked.update(|p| toggle_all(p, &vis, on));
                        }/>
                </th>
            }
        })
    };

    let server_sort = StoredValue::new(server_sort);
    let header = move || {
        columns.with_value(|cols| {
            cols.iter()
                .enumerate()
                .map(|(ci, c)| {
                    let sortable = c.sort.is_some();
                    let label = c.header;
                    let on_server = server_sort.with_value(|ss| ss.as_ref().and_then(|ss| c.server.filter(|k| ss.keys.contains(k)).map(|k| (k, ss.current.clone(), ss.on_sort))));
                    if let Some((key, current, on_sort)) = on_server {
                        let (aria, ind) = match &current {
                            Some((k, false)) if k == key => ("ascending", "▲"),
                            Some((k, true)) if k == key => ("descending", "▼"),
                            _ => ("none", "↕"),
                        };
                        return view! {
                            <th class=c.class aria-sort=aria>
                                <button class="th-sort" type="button" title=move || i18n.t(K::SortAllRows) on:click=move |_| {
                                    on_sort.run(next_server_sort(current.as_ref().map(|(k, d)| (k.as_str(), *d)), key));
                                }>
                                    {move || i18n.t(label)}
                                    <span class="sort-ind" aria-hidden="true">{ind}</span>
                                </button>
                            </th>
                        }
                        .into_any();
                    }
                    // On a paged list a client-side sort only reorders this page.
                    let page_only = server_sort.with_value(Option::is_some);
                    let aria = move || match sort.get() {
                        Some((i, true)) if i == ci => "ascending",
                        Some((i, false)) if i == ci => "descending",
                        _ => "none",
                    };
                    let class = c.class;
                    if sortable {
                        view! {
                            <th class=class aria-sort=aria>
                                <button class="th-sort" type="button" title=move || page_only.then(|| i18n.t(K::SortThisPage)) on:click=move |_| {
                                    sort.update(|s| {
                                        *s = match *s {
                                            Some((i, true)) if i == ci => Some((ci, false)),
                                            Some((i, false)) if i == ci => None,
                                            _ => Some((ci, true)),
                                        }
                                    })
                                }>
                                    {move || i18n.t(label)}
                                    <span class="sort-ind" aria-hidden="true">
                                        {move || match sort.get() {
                                            Some((i, true)) if i == ci => "▲",
                                            Some((i, false)) if i == ci => "▼",
                                            _ => "↕",
                                        }}
                                    </span>
                                </button>
                            </th>
                        }
                        .into_any()
                    } else {
                        view! { <th class=class>{move || i18n.t(label)}</th> }.into_any()
                    }
                })
                .collect_view()
        })
    };

    // Keyed rows: a filter keystroke or a sort moves, adds or drops rows instead
    // of rebuilding every one. The language is part of the key, so switching it redraws them.
    let empty = Memo::new(move |_| visible.with(Vec::is_empty));
    let stored = rows;
    let row_view = move |lang: Lang, i: usize| {
        let key = row_keys.with_value(|k| k[i].clone());
        rows.with_value(|rows| {
            columns.with_value(|cols| {
                let r = &rows[i];
                let check = select.map(|s| {
                    let key = keys.with_value(|k| k[i].clone());
                    let label = format!("{} {}", i18n.t(K::SelectRow), key.as_ref().map(|k| k.1.as_str()).unwrap_or(""));
                    let id = key.as_ref().map(|k| k.0.clone()).unwrap_or_default();
                    let picked = move || s.picked.with(|p| p.iter().any(|(x, _)| *x == id));
                    view! {
                        <td class="select">
                            <input type="checkbox" aria-label=label disabled=key.is_none() prop:checked=picked
                                on:change=move |ev| {
                                    if let Some(k) = &key {
                                        s.picked.update(|p| toggle(p, k, event_target_checked(&ev)));
                                    }
                                }/>
                        </td>
                    }
                });
                let k2 = key.clone();
                let is_open = move || open.with(|o| o.contains(&k2));
                let toggle = drawer.map(|_| {
                    let is_open = is_open.clone();
                    let is_open2 = is_open.clone();
                    let is_open3 = is_open.clone();
                    let key = key.clone();
                    view! {
                        <td class="drawer-toggle">
                            <button type="button" class="btn-drawer" aria-expanded=move || is_open().to_string()
                                aria-label=move || i18n.t(if is_open2() { K::HideDetails } else { K::ShowDetails })
                                title=move || i18n.t(if is_open3() { K::HideDetails } else { K::ShowDetails })
                                on:click=move |_| open.update(|o| toggle_open(o, &key))>
                                <svg aria-hidden="true" viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                                    <path d="m9 6 6 6-6 6"></path>
                                </svg>
                            </button>
                        </td>
                    }
                });
                // Its own reactive block, so opening one drawer doesn't
                // re-render the table (and the toggle keeps keyboard focus).
                let panel = drawer.map(|f| {
                    let is_open = is_open.clone();
                    move || {
                        is_open().then(|| {
                            let content = stored.with_value(|rows| f(&rows[i], i18n.lang()));
                            view! { <tr class="drawer"><td colspan=ncols>{content}</td></tr> }
                        })
                    }
                });
                view! {
                    <tr class:open=is_open>
                        {toggle}
                        {check}
                        {cols
                            .iter()
                            .map(|c| view! { <td class=c.class>{(c.cell)(r, lang)}</td> })
                            .collect_view()}
                    </tr>
                    {panel}
                }
            })
        })
    };
    let body = move || {
        if empty.get() {
            return view! {
                <tr>
                    <td class="empty" colspan=ncols>{move || i18n.t(K::NoItems)}</td>
                </tr>
            }
            .into_any();
        }
        view! {
            <For
                each=move || {
                    let lang = i18n.lang();
                    row_keys.with_value(|k| visible.get().into_iter().map(|i| (lang, k[i].clone(), i)).collect::<Vec<_>>())
                }
                key=|(lang, k, _)| (*lang, k.clone())
                children=move |(lang, _, i)| row_view(lang, i)
            />
        }
        .into_any()
    };

    let count = move || displaying(i18n.lang(), visible.with(Vec::len));

    view! {
        <div class="table-toolbar">
            <span class="count">{count}</span>
            <span class="spacer"></span>
            {timing.map(|(ms, calls)| view! { <TimingChip millis=ms calls=calls/> })}
            {extra}
            {(!paged).then(|| view! {
            <label class="filter">
                <span class="sr-only">{move || i18n.t(K::FilterPlaceholder)}</span>
                <svg aria-hidden="true" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                    <circle cx="11" cy="11" r="7"></circle>
                    <path d="m20 20-3.5-3.5"></path>
                </svg>
                <input
                    type="search"
                    placeholder=move || i18n.t(K::FilterPlaceholder)
                    prop:value=typed
                    on:input=move |ev| on_type(event_target_value(&ev))
                />
            </label>
            })}
            {on_refresh.map(|cb| view! {
                <button class="btn btn-icon" type="button" title=move || i18n.t(K::Refresh)
                    aria-label=move || i18n.t(K::Refresh) on:click=move |_| cb.run(())>
                    <svg aria-hidden="true" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M21 12a9 9 0 1 1-2.64-6.36"></path>
                        <path d="M21 3v6h-6"></path>
                    </svg>
                </button>
            })}
        </div>
        <div class="table-wrap">
            <table class="table">
                <thead><tr>
                    {drawer.map(|_| view! { <th class="drawer-toggle"><span class="sr-only">{move || i18n.t(K::ShowDetails)}</span></th> })}
                    {select_header}{header}
                </tr></thead>
                <tbody>{body}</tbody>
            </table>
        </div>
        <div class="table-footer">{count}</div>
        {footer}
    }
}

/// "3 API calls · 0.41 s" — keeps the speed of the backend visible.
#[component]
pub fn TimingChip(millis: u64, calls: u32) -> impl IntoView {
    let i18n = use_i18n();
    let secs = millis as f64 / 1000.0;
    view! {
        <span class="timing" title=move || format!("{} {} API · {:.2} s", i18n.t(K::FetchedIn), calls, secs)>
            <span class="dot" aria-hidden="true"></span>
            {move || {
                let s = format!("{secs:.2} s");
                let s = if i18n.lang() == Lang::EsEs { s.replace('.', ",") } else { s };
                format!("{calls} API · {s}")
            }}
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_sorted_header_cycles_like_a_client_one() {
        let first = next_server_sort(None, "name");
        assert_eq!(first, Some(("name".into(), false)), "ascending first");
        assert_eq!(
            next_server_sort(Some(("name", false)), "name"),
            Some(("name".into(), true))
        );
        assert_eq!(
            next_server_sort(Some(("name", true)), "name"),
            None,
            "then the API's default order"
        );
        assert_eq!(
            next_server_sort(Some(("status", true)), "name"),
            Some(("name".into(), false)),
            "another column starts ascending"
        );
    }

    fn hay() -> Vec<String> {
        [
            "web-prod-01 10.0.0.1 active",
            "db-primary 10.0.0.5 active",
            "ci-runner error",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    #[test]
    fn hides_columns_the_cloud_does_not_offer() {
        let cols = || {
            vec![
                col(K::Name, |_: &u8, _| ""),
                col(K::DnsName, |_: &u8, _| "").requires(Requires::NetExt("dns-integration")),
            ]
        };
        let headers = |c: &Capabilities| available(cols(), c).iter().map(|c| c.header).collect::<Vec<_>>();
        let without = Capabilities {
            known: true,
            network_extensions: vec!["router".into()],
            ..Default::default()
        };
        let with = Capabilities {
            known: true,
            network_extensions: vec!["dns-integration".into()],
            ..Default::default()
        };
        assert_eq!(headers(&without), [K::Name]);
        assert_eq!(headers(&with), [K::Name, K::DnsName]);
        assert_eq!(
            headers(&Capabilities::default()),
            [K::Name, K::DnsName],
            "unknown: show everything"
        );
    }

    #[test]
    fn filters_with_all_terms_case_insensitively() {
        assert_eq!(visible_rows(&hay(), None, "", true), vec![0, 1, 2]);
        assert_eq!(visible_rows(&hay(), None, "ACTIVE", true), vec![0, 1]);
        assert_eq!(visible_rows(&hay(), None, "active 10.0.0.5", true), vec![1]);
        assert!(visible_rows(&hay(), None, "nothing", true).is_empty());
    }

    fn row(id: &str) -> (String, String) {
        (id.to_string(), format!("name-{id}"))
    }

    #[test]
    fn selection_toggles_rows_and_all_visible_rows() {
        let mut p = vec![];
        toggle(&mut p, &row("a"), true);
        toggle(&mut p, &row("a"), true);
        assert_eq!(p, [row("a")], "no duplicates");
        toggle_all(&mut p, &[row("b"), row("c")], true);
        assert_eq!(p, [row("a"), row("b"), row("c")], "click order kept");
        toggle_all(&mut p, &[row("a"), row("c")], false);
        assert_eq!(p, [row("b")], "hidden rows keep their state");
        assert!(retain_present(&mut p, &["x"]));
        assert!(p.is_empty());
        assert!(!retain_present(&mut p, &[]));
    }

    #[test]
    fn drawers_stay_open_across_sorts_and_close_when_filtered_out() {
        let keys = row_keys(&[None, None, None], &hay());
        let mut open: Vec<String> = vec![];
        toggle_open(&mut open, &keys[2]);
        toggle_open(&mut open, &keys[0]);
        // Keys don't move when the table is re-sorted: same rows open.
        let sort = vec![SortKey::Num(10), SortKey::Num(2), SortKey::Num(33)];
        let vis = |idx: Vec<usize>| idx.into_iter().map(|i| keys[i].clone()).collect::<Vec<_>>();
        assert!(!retain_visible(
            &mut open,
            &vis(visible_rows(&hay(), Some(&sort), "", false))
        ));
        assert_eq!(open, [keys[2].clone(), keys[0].clone()]);
        toggle_open(&mut open, &keys[2]);
        assert_eq!(open, [keys[0].clone()], "a second click closes it");
        assert!(
            retain_visible(&mut open, &vis(visible_rows(&hay(), None, "db", true))),
            "row 0 is hidden by the filter"
        );
        assert!(open.is_empty());
    }

    /// A drawer follows its row's key, so a refetch that reorders or
    /// adds rows keeps the same row open.
    #[test]
    fn row_keys_use_the_id_and_stay_unique() {
        let ids = [Some("s1".to_string()), None, None, Some("s1".to_string())];
        let hay = ["web".to_string(), "db".to_string(), "db".to_string(), "x".to_string()];
        assert_eq!(row_keys(&ids, &hay), ["s1", "~db", "~db#2", "s1#2"]);
        let before = row_keys(&[Some("a".into()), Some("b".into())], &["".into(), "".into()]);
        let after = row_keys(
            &[Some("c".into()), Some("b".into()), Some("a".into())],
            &["".into(), "".into(), "".into()],
        );
        let mut open = vec![before[1].clone()];
        assert!(!retain_visible(&mut open, &after), "b is still there after the refetch");
        assert_eq!(open, ["b"]);
    }

    #[test]
    fn a_table_key_names_the_path_the_search_and_the_columns() {
        let a = table_key("/project/instances", 0x10, &[K::Name, K::Status]);
        assert_eq!(
            a,
            table_key("/project/instances", 0x10, &[K::Name, K::Status]),
            "stable across rebuilds"
        );
        assert_ne!(a, table_key("/project/instances", 0x20, &[K::Name, K::Status]));
        assert_ne!(a, table_key("/project/instances", 0x10, &[K::Name]));
        assert!(
            a.starts_with("/project/instances#"),
            "TableStates::get keeps a path's keys by this prefix"
        );
    }

    #[test]
    fn table_states_forget_other_pages() {
        let st = TableStates::default();
        let a = st.get("/a", table_key("/a", 1, &[]));
        a.filter.set("web".into());
        assert_eq!(
            st.get("/a", table_key("/a", 1, &[])).filter.get_untracked(),
            "web",
            "same table, same state"
        );
        st.get("/b", table_key("/b", 1, &[]));
        assert_eq!(
            st.get("/a", table_key("/a", 1, &[])).filter.get_untracked(),
            "",
            "leaving /a forgot it"
        );
    }

    #[test]
    fn sorts_both_ways_with_numbers_numerically() {
        let keys = vec![SortKey::Num(10), SortKey::Num(2), SortKey::Num(33)];
        assert_eq!(visible_rows(&hay(), Some(&keys), "", true), vec![1, 0, 2]);
        assert_eq!(visible_rows(&hay(), Some(&keys), "", false), vec![2, 0, 1]);
        // filter then sort
        assert_eq!(visible_rows(&hay(), Some(&keys), "active", false), vec![0, 1]);
    }
}
