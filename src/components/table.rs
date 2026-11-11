//! A generic, client-side filterable and sortable table, with Horizon's
//! "Displaying N items" header and footer. On a server-paged list (roadmap 88)
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

type CellFn<T> = Arc<dyn Fn(&T, Lang) -> AnyView + Send + Sync>;
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
    /// list; used only when the table's [`ServerSort`] lists it (roadmap 88).
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

/// How a server-paged table sorts on the server (roadmap 88): the neutral keys
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
    /// Hidden when the cloud lacks `r` (roadmap 19).
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

/// Renders the drawer opened under a row (roadmap 56).
pub type DrawerFn<T> = fn(&T, Lang) -> AnyView;

/// Opens or closes the drawer of row `i` (an index into the table's rows,
/// which a sort doesn't change, so drawers stay open across sorts).
pub fn toggle_open(open: &mut Vec<usize>, i: usize) {
    if let Some(p) = open.iter().position(|&x| x == i) {
        open.remove(p);
    } else {
        open.push(i);
    }
}

/// Closes the drawers of rows the filter hides. Returns whether anything changed.
pub fn retain_visible(open: &mut Vec<usize>, visible: &[usize]) -> bool {
    let before = open.len();
    open.retain(|i| visible.contains(i));
    open.len() != before
}

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
    /// A server-paged list (roadmap 88): the rows are one page and the page's filter
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
    let keys = StoredValue::new(keys);
    let rows = StoredValue::new(rows);
    let haystacks = StoredValue::new(haystacks);
    let columns = StoredValue::new(columns);
    let filter = RwSignal::new(String::new());
    let sort: RwSignal<Option<(usize, bool)>> = RwSignal::new(None);
    let ncols = columns.with_value(|c| c.len()) + select.is_some() as usize + drawer.is_some() as usize;
    let open: RwSignal<Vec<usize>> = RwSignal::new(Vec::new());

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
            let vis = visible.get();
            // Untracked check first so an unchanged set doesn't re-render the drawers.
            if open.with_untracked(|o| o.iter().any(|i| !vis.contains(i))) {
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

    let body = move || {
        let lang = i18n.lang();
        let idx = visible.get();
        if idx.is_empty() {
            return view! {
                <tr>
                    <td class="empty" colspan=ncols>{i18n.t(K::NoItems)}</td>
                </tr>
            }
            .into_any();
        }
        let stored = rows;
        rows.with_value(|rows| {
            columns.with_value(|cols| {
                idx.into_iter()
                    .map(|i| {
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
                        let is_open = move || open.with(|o| o.contains(&i));
                        let toggle = drawer.map(|_| {
                            view! {
                                <td class="drawer-toggle">
                                    <button type="button" class="btn-drawer" aria-expanded=move || is_open().to_string()
                                        aria-label=move || i18n.t(if is_open() { K::HideDetails } else { K::ShowDetails })
                                        title=move || i18n.t(if is_open() { K::HideDetails } else { K::ShowDetails })
                                        on:click=move |_| open.update(|o| toggle_open(o, i))>
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
                    .collect_view()
                    .into_any()
            })
        })
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
                    prop:value=filter
                    on:input=move |ev| filter.set(event_target_value(&ev))
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
        let mut open = vec![];
        toggle_open(&mut open, 2);
        toggle_open(&mut open, 0);
        // Row indices don't move when the table is re-sorted: same rows open.
        let keys = vec![SortKey::Num(10), SortKey::Num(2), SortKey::Num(33)];
        let sorted = visible_rows(&hay(), Some(&keys), "", false);
        assert!(!retain_visible(&mut open, &sorted));
        assert_eq!(open, [2, 0]);
        toggle_open(&mut open, 2);
        assert_eq!(open, [0], "a second click closes it");
        let filtered = visible_rows(&hay(), None, "db", true);
        assert!(retain_visible(&mut open, &filtered), "row 0 is hidden by the filter");
        assert!(open.is_empty());
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
