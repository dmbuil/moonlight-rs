//! Horizon's *Allocated / Available* tables (launch wizard): pick one or more
//! rows; options that cannot be used (e.g. a flavor too small for the image)
//! stay visible, greyed out, with the reason.

use leptos::prelude::*;

use crate::i18n::{K, Lang, resource_status, use_i18n};
use crate::model::{Choice, ChoiceSet};

/// How a raw cell is shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fmt {
    Text,
    /// `true`/`false` → Yes/No
    YesNo,
    /// `true`/`false` → Up/Down (admin state)
    UpDown,
    /// Localised status word
    State,
    /// What a port belongs to: `router` / `instance` → Router / Instance
    Device,
    /// A firewall rule's `allow` / `deny` / `reject`
    FwAction,
    /// A firewall rule's protocol, upper case; empty → *Any*
    Protocol,
}

/// Columns of each choice set, matching `mapping::*_choices` cells and the
/// columns of Horizon's wizard tables.
pub fn columns(set: Option<ChoiceSet>) -> Vec<(K, Fmt)> {
    use Fmt::*;
    use K::*;
    match set {
        Some(ChoiceSet::LaunchImages | ChoiceSet::InstanceSnapshots) => {
            vec![
                (Name, Text),
                (Updated, Text),
                (Size, Text),
                (Format, Text),
                (Visibility, Text),
            ]
        }
        Some(ChoiceSet::AvailableVolumes) => {
            vec![
                (Name, Text),
                (Description, Text),
                (Size, Text),
                (Type, Text),
                (AvailabilityZone, Text),
            ]
        }
        Some(ChoiceSet::VolumeSnapshots) => vec![
            (Name, Text),
            (Description, Text),
            (Size, Text),
            (Created, Text),
            (Status, State),
        ],
        Some(ChoiceSet::Flavors) => vec![
            (Name, Text),
            (Vcpus, Text),
            (Ram, Text),
            (TotalDisk, Text),
            (RootDisk, Text),
            (EphemeralDisk, Text),
            (Public, YesNo),
        ],
        Some(ChoiceSet::Networks) => vec![
            (Network, Text),
            (SubnetsAssociated, Text),
            (Shared, YesNo),
            (AdminState, UpDown),
            (Status, State),
        ],
        Some(ChoiceSet::FreePorts) => vec![(Name, Text), (IpAddress, Text), (AdminState, UpDown), (Status, State)],
        // Ports are seldom named: what they belong to comes first.
        Some(ChoiceSet::FirewallPorts) => vec![
            (K::Device, Text),
            (Type, Fmt::Device),
            (IpAddresses, Text),
            (Port, Text),
        ],
        Some(ChoiceSet::FirewallRules) => vec![(Name, Text), (Action, FwAction), (K::Protocol, Fmt::Protocol)],
        Some(ChoiceSet::SecurityGroups | ChoiceSet::SecurityGroupIds) => vec![(Name, Text), (Description, Text)],
        Some(ChoiceSet::KeyPairs) => vec![(Name, Text), (Type, Text), (Fingerprint, Text)],
        Some(ChoiceSet::ServerGroups) => vec![(Name, Text), (Policy, Text)],
        _ => vec![(Name, Text)],
    }
}

/// A raw cell of a choice as the column's [`Fmt`] shows it; unknown values pass through.
fn cell(lang: Lang, fmt: Fmt, raw: &str) -> String {
    let t = |k| crate::i18n::t(lang, k).to_string();
    match (fmt, raw) {
        (Fmt::YesNo, "true") => t(K::Yes),
        (Fmt::YesNo, "false") => t(K::No),
        (Fmt::UpDown, "true") => t(K::Up),
        (Fmt::UpDown, "false") => t(K::Down),
        (Fmt::State, s) if !s.is_empty() => resource_status(lang, s),
        (Fmt::Device, "router") => t(K::Router),
        (Fmt::Device, "instance") => t(K::Instance),
        (Fmt::FwAction, "allow") => t(K::FirewallAllow),
        (Fmt::FwAction, "deny") => t(K::FirewallDeny),
        (Fmt::FwAction, "reject") => t(K::FirewallReject),
        (Fmt::Protocol, "") => t(K::AnyValue),
        (Fmt::Protocol, p) => p.to_uppercase(),
        (_, "") => "-".into(),
        (_, s) => s.to_string(),
    }
}

/// What an empty *Available* table says: nothing to choose at all, everything
/// already chosen, or nothing matching the filter.
fn available_hint(options: usize, left: usize) -> K {
    match (options, left) {
        (0, _) => K::NothingToChoose,
        (_, 0) => K::AllAllocated,
        // Items left but none shown: the filter hides them.
        _ => K::NoMatches,
    }
}

/// The selected values, in order.
pub fn selected(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Adds `id` (at the end); a single pick replaces the current one.
pub fn allocate(value: &str, id: &str, multi: bool) -> String {
    if !multi {
        return id.to_string();
    }
    let mut v = selected(value);
    if !v.iter().any(|x| x == id) {
        v.push(id.to_string());
    }
    v.join(",")
}

/// Removes `id` from the allocated list, keeping the others in order.
pub fn deallocate(value: &str, id: &str) -> String {
    selected(value)
        .into_iter()
        .filter(|x| x != id)
        .collect::<Vec<_>>()
        .join(",")
}

/// Moves `id` one place earlier (network order = NIC order).
pub fn move_up(value: &str, id: &str) -> String {
    let mut v = selected(value);
    if let Some(i) = v.iter().position(|x| x == id).filter(|&i| i > 0) {
        v.swap(i, i - 1);
    }
    v.join(",")
}

/// Whether every word of `query` is in the choice's label or cells (case-insensitive): the
/// *Available* table's filter.
fn matches(c: &Choice, query: &str) -> bool {
    let hay = format!("{} {}", c.label, c.cells.join(" ")).to_lowercase();
    query.split_whitespace().all(|t| hay.contains(&t.to_lowercase()))
}

/// Horizon's *Allocated* / *Available* pair for a `Kind::Pick` field: allocated rows on top (with
/// up/down arrows when `multi`), the filterable available rows under them, blocked rows greyed
/// with their reason. The value is the comma-separated allocated IDs.
#[component]
pub fn PickTable(
    #[prop(into)] id: String,
    multi: bool,
    /// `None` while loading.
    #[prop(into)]
    options: Signal<Option<Vec<Choice>>>,
    #[prop(into)] set: Signal<Option<ChoiceSet>>,
    #[prop(into)] value: Signal<String>,
    on_change: Callback<String>,
    /// Why an option cannot be allocated, if it cannot.
    blocked: Callback<Choice, Option<String>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let query = RwSignal::new(String::new());
    let cols = move || columns(set.get());
    let cells = move |c: &Choice, n: usize| -> Vec<String> {
        if c.cells.len() == n {
            c.cells.clone()
        } else {
            std::iter::once(c.label.clone())
                .chain(std::iter::repeat_n(String::new(), n.saturating_sub(1)))
                .collect()
        }
    };
    let header = move || {
        view! {
            <thead><tr>
                {cols().into_iter().map(|(k, _)| view! { <th scope="col">{move || i18n.t(k)}</th> }).collect_view()}
                <th class="pick-act"><span class="sr-only">{move || i18n.t(K::Actions)}</span></th>
            </tr></thead>
        }
    };
    let row_cells = move |c: &Choice| {
        let l = i18n.lang();
        let fmts: Vec<Fmt> = cols().into_iter().map(|(_, f)| f).collect();
        cells(c, fmts.len())
            .into_iter()
            .zip(fmts)
            .enumerate()
            .map(|(i, (raw, f))| {
                let text = cell(l, f, &raw);
                if i == 0 {
                    view! { <td class="strong">{text}</td> }.into_any()
                } else {
                    view! { <td>{text}</td> }.into_any()
                }
            })
            .collect_view()
    };

    let allocated = move || {
        let list = options.get().unwrap_or_default();
        selected(&value.get())
            .into_iter()
            .filter_map(|v| list.iter().find(|c| c.value == v).cloned())
            .collect::<Vec<_>>()
    };
    let available = move || {
        let chosen = selected(&value.get());
        let q = query.get();
        options
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|c| !chosen.contains(&c.value) && matches(c, &q))
            .collect::<Vec<_>>()
    };
    let total_available = move || {
        let chosen = selected(&value.get());
        options
            .get()
            .map(|l| l.iter().filter(|c| !chosen.contains(&c.value)).count())
    };

    view! {
        <div class="pick" id=id>
            <h4 class="pick-title">{move || i18n.t(K::Allocated)} <span class="badge">{move || allocated().len()}</span></h4>
            <div class="pick-scroll">
                <table class="pick-table">
                    {header}
                    <tbody>
                        {move || {
                            let rows = allocated();
                            if rows.is_empty() {
                                let none = options.get().is_some_and(|l| l.is_empty());
                                let hint = if none { K::NothingToChoose } else if multi { K::SelectManyHint } else { K::SelectOneHint };
                                return view! { <tr><td class="pick-hint" colspan="99">{i18n.t(hint)}</td></tr> }.into_any();
                            }
                            rows.into_iter().enumerate().map(|(i, c)| {
                                let (v1, v2, label) = (c.value.clone(), c.value.clone(), c.label.clone());
                                view! {
                                    <tr class="pick-chosen" data-value=c.value.clone()>
                                        {row_cells(&c)}
                                        <td class="pick-act">
                                            {(multi && i > 0).then(|| view! {
                                                <button type="button" class="btn btn-sm btn-icon" title=move || i18n.t(K::MoveUp)
                                                    aria-label=format!("{}: {}", i18n.t(K::MoveUp), label)
                                                    on:click=move |_| on_change.run(move_up(&value.get_untracked(), &v1))>"↑"</button>
                                            })}
                                            <button type="button" class="btn btn-sm btn-icon" title=move || i18n.t(K::Deallocate)
                                                aria-label=format!("{}: {}", i18n.t(K::Deallocate), c.label)
                                                on:click=move |_| on_change.run(deallocate(&value.get_untracked(), &v2))>"↓"</button>
                                        </td>
                                    </tr>
                                }
                            }).collect_view().into_any()
                        }}
                    </tbody>
                </table>
            </div>
            <div class="pick-bar">
                <h4 class="pick-title">{move || i18n.t(K::Available)} " "
                    <span class="badge">{move || total_available().map(|n| n.to_string()).unwrap_or_else(|| "…".into())}</span>
                </h4>
                <input type="search" class="pick-filter" placeholder=move || i18n.t(K::FilterPlaceholder)
                    aria-label=move || i18n.t(K::FilterPlaceholder)
                    prop:value=move || query.get() on:input=move |ev| query.set(event_target_value(&ev))/>
            </div>
            <div class="pick-scroll">
                <table class="pick-table">
                    {header}
                    <tbody>
                        {move || match options.get() {
                            None => view! { <tr><td class="pick-hint" colspan="99">{i18n.t(K::LoadingChoices)}</td></tr> }.into_any(),
                            Some(list) => {
                                let rows = available();
                                if rows.is_empty() {
                                    let hint = available_hint(list.len(), total_available().unwrap_or(0));
                                    return view! { <tr><td class="pick-hint" colspan="99">{i18n.t(hint)}</td></tr> }.into_any();
                                }
                                rows.into_iter().map(|c| {
                                    let why = blocked.run(c.clone());
                                    let v = c.value.clone();
                                    let label = c.label.clone();
                                    view! {
                                        <tr class:pick-blocked=why.is_some() data-value=c.value.clone() title=why.clone()>
                                            {row_cells(&c)}
                                            <td class="pick-act">
                                                {why.clone().map(|w| view! { <span class="pick-warn" title=w.clone() aria-label=w>"⚠"</span> })}
                                                <button type="button" class="btn btn-sm btn-icon" disabled=why.is_some()
                                                    title=move || i18n.t(K::Allocate)
                                                    aria-label=format!("{}: {}", i18n.t(K::Allocate), label)
                                                    on:click=move |_| on_change.run(allocate(&value.get_untracked(), &v, multi))>"↑"</button>
                                            </td>
                                        </tr>
                                    }
                                }).collect_view().into_any()
                            }
                        }}
                    </tbody>
                </table>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_pick_replaces_and_multi_appends_once() {
        assert_eq!(allocate("a", "b", false), "b");
        assert_eq!(allocate("a", "b", true), "a,b");
        assert_eq!(allocate("a,b", "a", true), "a,b");
        assert_eq!(allocate("", "a", true), "a");
    }

    #[test]
    fn remove_and_reorder_keep_the_rest_in_order() {
        assert_eq!(deallocate("a,b,c", "b"), "a,c");
        assert_eq!(move_up("a,b,c", "c"), "a,c,b");
        assert_eq!(move_up("a,b,c", "a"), "a,b,c");
        assert_eq!(selected(" a, ,b "), vec!["a", "b"]);
    }

    #[test]
    fn cells_are_localised_by_column_format() {
        assert_eq!(cell(Lang::EnUs, Fmt::YesNo, "true"), "Yes");
        assert_eq!(cell(Lang::EsEs, Fmt::UpDown, "false"), "Inactivo");
        assert_eq!(cell(Lang::EnUs, Fmt::Text, ""), "-");
        assert_eq!(columns(Some(ChoiceSet::Flavors)).len(), 7);
        assert_eq!(
            (
                cell(Lang::EsEs, Fmt::Device, "instance"),
                cell(Lang::EnUs, Fmt::Device, "router")
            ),
            ("Instancia".into(), "Router".into())
        );
        assert_eq!(
            columns(Some(ChoiceSet::FirewallPorts)).len(),
            4,
            "one per cell of mapping::firewall_port_choices"
        );
        assert_eq!(
            columns(Some(ChoiceSet::FirewallRules)).len(),
            3,
            "one per cell of mapping::firewall_rule_choices"
        );
        assert_eq!(
            (
                cell(Lang::EsEs, Fmt::FwAction, "deny"),
                cell(Lang::EnUs, Fmt::FwAction, "allow")
            ),
            ("Denegar".into(), "Allow".into())
        );
        assert_eq!(
            (
                cell(Lang::EsEs, Fmt::Protocol, ""),
                cell(Lang::EnUs, Fmt::Protocol, "icmp")
            ),
            ("Cualquiera".into(), "ICMP".into())
        );
    }

    #[test]
    fn an_empty_available_table_says_why() {
        assert_eq!(
            available_hint(0, 0),
            K::NothingToChoose,
            "e.g. no router or instance ports for a firewall group"
        );
        assert_eq!(available_hint(3, 0), K::AllAllocated);
        assert_eq!(available_hint(3, 2), K::NoMatches, "the filter hides them");
    }

    #[test]
    fn filter_looks_at_every_cell() {
        let c = Choice {
            value: "f".into(),
            label: "C3.1-2".into(),
            cells: vec!["C3.1-2".into(), "1".into(), "2 GB".into()],
            ..Default::default()
        };
        assert!(matches(&c, "c3 2 gb"));
        assert!(!matches(&c, "a3"));
    }
}
