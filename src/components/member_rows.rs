//! The members editor of the load balancer dialogs (roadmap 54), laid out like
//! Horizon's *Pool Members* step: an *Allocated Members* table (IP address,
//! subnet, port, weight, *Remove*; a chevron opens the row's monitor address
//! and port, admin state, backup and name) with *Add external member* under it,
//! and an *Available Instances* table to add instance addresses from. The value
//! is a JSON list of [`MemberRow`]; `model::member_specs` validates it on submit.

use leptos::prelude::*;

use crate::components::select::Combo;
use crate::i18n::{K, use_i18n};
use crate::model::{Choice, LbMember, MemberRow, removed_members};

/// One allocated row: its key, the row, and whether its details are open.
type Entry = (usize, RwSignal<MemberRow>, RwSignal<bool>);

/// `targets`: instance addresses (`ip@subnet` values, cells `[instance, ip,
/// subnet name]`); `subnets`: for external members. `default_port` seeds new
/// rows (the listener's port in the wizard). `existing`: the pool's members when
/// editing them, to list the ones the save would delete.
#[component]
pub fn MemberRows(
    id: String,
    labelled_by: String,
    initial: String,
    targets: Signal<Option<Vec<Choice>>>,
    subnets: Signal<Option<Vec<Choice>>>,
    default_port: Signal<String>,
    existing: Vec<LbMember>,
    on_change: Callback<String>,
) -> impl IntoView {
    let i18n = use_i18n();
    let start: Vec<MemberRow> = serde_json::from_str(&initial).unwrap_or_default();
    let next = StoredValue::new(start.len());
    // Each row in its own signal, so typing doesn't rebuild the list (and lose focus).
    let rows: RwSignal<Vec<Entry>> = RwSignal::new(
        start
            .into_iter()
            .enumerate()
            .map(|(i, r)| (i, RwSignal::new(r), RwSignal::new(false)))
            .collect(),
    );
    let current = move || rows.with(|r| r.iter().map(|(_, row, _)| row.get()).collect::<Vec<MemberRow>>());
    let sync = move || {
        let list: Vec<MemberRow> = rows.with_untracked(|r| r.iter().map(|(_, row, _)| row.get_untracked()).collect());
        on_change.run(serde_json::to_string(&list).unwrap_or_default());
    };
    // New rows' signals belong to this component, not to whatever owner runs the
    // click handler (e.g. a re-rendering list), which would dispose them; reading
    // a disposed signal panics, which stops the WASM app.
    let owner = StoredValue::new(Owner::current().expect("MemberRows renders under an owner"));
    let push = move |row: MemberRow| {
        let i = next.get_value();
        next.set_value(i + 1);
        let entry = owner.with_value(|o| o.with(|| (i, RwSignal::new(row), RwSignal::new(false))));
        rows.update(|r| r.push(entry));
        sync();
    };
    let port_for_new = move || {
        let p = default_port.get_untracked();
        if !p.trim().is_empty() {
            return p;
        }
        rows.with_untracked(|r| r.first().map(|(_, row, _)| row.get_untracked().port))
            .unwrap_or_else(|| "80".into())
    };
    let add_instance = move |c: Choice| {
        let (address, subnet) = c
            .value
            .split_once('@')
            .map(|(a, s)| (a.to_string(), s.to_string()))
            .unwrap_or((c.value.clone(), String::new()));
        push(MemberRow {
            address,
            subnet_id: subnet,
            subnet_name: c.cells.get(2).cloned().unwrap_or_default(),
            label: c.cells.first().cloned(),
            port: port_for_new(),
            weight: "1".into(),
            admin_up: true,
            ..Default::default()
        });
    };
    let add_external = move |_| {
        push(MemberRow {
            typed: true,
            port: port_for_new(),
            weight: "1".into(),
            admin_up: true,
            ..Default::default()
        });
    };
    let existing = StoredValue::new(existing);
    // Members a save would delete: Octavia matches by address and port, so a row
    // whose port no longer parses counts as removed too.
    let removed = move || {
        let keep: Vec<_> = current().iter().filter_map(|r| r.spec().ok()).collect();
        existing.with_value(|e| {
            removed_members(e, &keep)
                .into_iter()
                .map(|m| format!("{} ({}:{})", m.name, m.address, m.port.unwrap_or(0)))
                .collect::<Vec<_>>()
        })
    };
    let show_allocated = RwSignal::new(true);
    let show_available = RwSignal::new(true);
    let filter = RwSignal::new(String::new());
    // Like Horizon, an allocated instance stays available: the same address can
    // be a member again on another port.
    let available = move || {
        let q = filter.get().to_lowercase();
        targets.get().map(|t| {
            t.into_iter()
                .filter(|c| {
                    q.split_whitespace()
                        .all(|w| c.cells.join(" ").to_lowercase().contains(w))
                })
                .collect::<Vec<_>>()
        })
    };
    let star = || view! { <span class="req" aria-hidden="true">" *"</span> };

    view! {
        <div class="member-rows" id=id role="group" aria-labelledby=labelled_by>
            <section>
                <h3 class="members-title">
                    <button type="button" class="members-toggle" aria-expanded=move || show_allocated.get().to_string()
                        on:click=move |_| show_allocated.update(|v| *v = !*v)>
                        <Chevron/>{move || i18n.t(K::AllocatedMembers)}
                        <span class="badge">{move || rows.with(Vec::len)}</span>
                    </button>
                </h3>
                <Show when=move || show_allocated.get()>
                    <table class="table members-table">
                        <thead><tr>
                            <th class="drawer-toggle"><span class="sr-only">{move || i18n.t(K::ShowDetails)}</span></th>
                            <th>{move || i18n.t(K::IpAddress)}{star()}</th>
                            <th>{move || i18n.t(K::Subnet)}{star()}</th>
                            <th class="members-num">{move || i18n.t(K::Port)}{star()}</th>
                            <th class="members-num">{move || i18n.t(K::Weight)}</th>
                            <th class="actions"></th>
                        </tr></thead>
                        <tbody>
                            {move || rows.with(Vec::is_empty).then(|| view! {
                                <tr><td class="empty" colspan="6">{i18n.t(K::NoMembers)}</td></tr>
                            })}
                            <For each=move || rows.get() key=|(i, _, _)| *i let:item>
                                <AllocatedRow item=item subnets=subnets sync=Callback::new(move |_| sync())
                                    remove=Callback::new(move |i: usize| { rows.update(|r| r.retain(|(j, _, _)| *j != i)); sync(); })/>
                            </For>
                        </tbody>
                    </table>
                    <div class="members-footer">
                        <button type="button" class="btn btn-sm" on:click=add_external>{move || i18n.t(K::AddExternalMember)}</button>
                    </div>
                    {move || {
                        let gone = removed();
                        (!gone.is_empty()).then(|| view! {
                            <div class="member-removed" role="status">
                                <strong>{i18n.t(K::MembersRemoved)}</strong>
                                <ul>{gone.into_iter().map(|g| view! { <li>{g}</li> }).collect_view()}</ul>
                            </div>
                        })
                    }}
                </Show>
            </section>
            <section>
                <h3 class="members-title">
                    <button type="button" class="members-toggle" aria-expanded=move || show_available.get().to_string()
                        on:click=move |_| show_available.update(|v| *v = !*v)>
                        <Chevron/>{move || i18n.t(K::AvailableInstances)}
                    </button>
                </h3>
                <Show when=move || show_available.get()>
                    <label class="filter members-filter">
                        <span class="sr-only">{move || i18n.t(K::FilterPlaceholder)}</span>
                        <svg aria-hidden="true" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                            <circle cx="11" cy="11" r="7"></circle>
                            <path d="m20 20-3.5-3.5"></path>
                        </svg>
                        <input type="search" placeholder=move || i18n.t(K::FilterPlaceholder) prop:value=filter
                            on:input=move |ev| filter.set(event_target_value(&ev))/>
                    </label>
                    <table class="table members-table">
                        <thead><tr>
                            <th>{move || i18n.t(K::Name)}</th>
                            <th>{move || i18n.t(K::IpAddress)}</th>
                            <th class="actions"></th>
                        </tr></thead>
                        <tbody>
                            {move || match available() {
                                None => view! { <tr><td class="empty" colspan="3">{i18n.t(K::LoadingChoices)}</td></tr> }.into_any(),
                                Some(list) if list.is_empty() => view! { <tr><td class="empty" colspan="3">{i18n.t(K::NoItems)}</td></tr> }.into_any(),
                                Some(list) => list.into_iter().map(|c| {
                                    let name = c.cells.first().cloned().unwrap_or_else(|| c.label.clone());
                                    let ip = c.cells.get(1).cloned().unwrap_or_default();
                                    view! {
                                        <tr>
                                            <td>{name}</td>
                                            <td class="mono">{ip}</td>
                                            <td class="actions">
                                                <button type="button" class="btn btn-sm" on:click=move |_| add_instance(c.clone())>
                                                    {move || i18n.t(K::AddAction)}
                                                </button>
                                            </td>
                                        </tr>
                                    }
                                }).collect_view().into_any(),
                            }}
                        </tbody>
                    </table>
                </Show>
            </section>
        </div>
    }
}

/// One allocated member and, when its chevron is open, its details row.
#[component]
fn AllocatedRow(
    item: Entry,
    subnets: Signal<Option<Vec<Choice>>>,
    sync: Callback<()>,
    remove: Callback<usize>,
) -> impl IntoView {
    let i18n = use_i18n();
    let (i, row, open) = item;
    let field = move |f: fn(&mut MemberRow, String)| {
        move |ev| {
            row.update(|r| f(r, event_target_value(&ev)));
            sync.run(());
        }
    };
    let typed = row.with_untracked(|r| r.typed);
    let (address, subnet) = if typed {
        (
            view! {
                <input type="text" class="mono" placeholder="192.0.2.10" prop:value=move || row.with(|r| r.address.clone())
                    aria-label=move || i18n.t(K::IpAddress) on:input=field(|r, v| r.address = v)/>
            }
            .into_any(),
            view! {
                // Blank until picked, like Horizon (no *— None —*: the subnet is required).
                <Combo id=format!("member-subnet-{i}") options=subnets required=true empty_label=Some(String::new())
                    value=Signal::derive(move || row.with(|r| r.subnet_id.clone()))
                    aria_label=Signal::derive(move || i18n.t(K::Subnet).to_string())
                    on_change=Callback::new(move |v: String| { row.update(|r| r.subnet_id = v); sync.run(()); })/>
            }
            .into_any(),
        )
    } else {
        let r = row.get_untracked();
        let title = r.label.clone().unwrap_or_default();
        (
            view! { <span class="mono" title=title>{r.address}</span> }.into_any(),
            view! { <span>{if r.subnet_name.is_empty() { r.subnet_id } else { r.subnet_name }}</span> }.into_any(),
        )
    };
    // Horizon's Yes / No toggles for the two flags of the details row.
    let yes_no = move |label: K, get: fn(&MemberRow) -> bool, set: fn(&mut MemberRow, bool)| {
        let on = move || row.with(get);
        let put = move |v: bool| {
            row.update(|r| set(r, v));
            sync.run(());
        };
        view! {
            <div class="member-detail">
                <span>{move || i18n.t(label)}</span>
                <div class="yes-no" role="radiogroup" aria-label=move || i18n.t(label)>
                    <button type="button" role="radio" aria-checked=move || on().to_string() class:on=on on:click=move |_| put(true)>
                        {move || i18n.t(K::Yes)}
                    </button>
                    <button type="button" role="radio" aria-checked=move || (!on()).to_string() class:on=move || !on() on:click=move |_| put(false)>
                        {move || i18n.t(K::No)}
                    </button>
                </div>
            </div>
        }
    };
    view! {
        <tr class:open=move || open.get()>
            <td class="drawer-toggle">
                <button type="button" class="btn-drawer" aria-expanded=move || open.get().to_string()
                    aria-label=move || i18n.t(if open.get() { K::HideDetails } else { K::ShowDetails })
                    on:click=move |_| open.update(|o| *o = !*o)>
                    <Chevron/>
                </button>
            </td>
            <td>{address}</td>
            <td>{subnet}</td>
            <td class="members-num">
                <input type="number" min="1" max="65535" prop:value=move || row.with(|r| r.port.clone())
                    aria-label=move || i18n.t(K::MemberPort) on:input=field(|r, v| r.port = v)/>
            </td>
            <td class="members-num">
                <input type="number" min="0" max="256" prop:value=move || row.with(|r| r.weight.clone())
                    aria-label=move || i18n.t(K::Weight) on:input=field(|r, v| r.weight = v)/>
            </td>
            <td class="actions">
                <button type="button" class="btn btn-sm" on:click=move |_| remove.run(i)>{move || i18n.t(K::Remove)}</button>
            </td>
        </tr>
        <Show when=move || open.get()>
            <tr class="drawer">
                <td colspan="6">
                    <div class="member-details">
                        <label class="member-detail">{move || i18n.t(K::MonitorAddress)}
                            <input type="text" class="mono" prop:value=move || row.with(|r| r.monitor_address.clone())
                                on:input=field(|r, v| r.monitor_address = v)/></label>
                        <label class="member-detail">{move || i18n.t(K::MonitorPort)}
                            <input type="number" min="1" max="65535" prop:value=move || row.with(|r| r.monitor_port.clone())
                                on:input=field(|r, v| r.monitor_port = v)/></label>
                        {yes_no(K::AdminStateUp, |r| r.admin_up, |r, v| r.admin_up = v)}
                        {yes_no(K::Backup, |r| r.backup, |r, v| r.backup = v)}
                        <label class="member-detail">{move || i18n.t(K::Name)}
                            <input type="text" prop:value=move || row.with(|r| r.name.clone()) on:input=field(|r, v| r.name = v)/></label>
                    </div>
                </td>
            </tr>
        </Show>
    }
}

/// The ▸ that turns down when its section or row is open (CSS rotates it).
#[component]
fn Chevron() -> impl IntoView {
    view! {
        <svg class="chevron" aria-hidden="true" viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor"
            stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="m9 6 6 6-6 6"></path>
        </svg>
    }
}
