//! The role assignments editor of *Manage Members* and *Modify Groups* on a
//! project (roadmap 57), in the Allocated/Available picker's layout (as a
//! group's *Manage Members*): the project's users (or groups) with a checkbox
//! per role and ↓ to remove, and the *Available* ones, filtered, with ↑ to add. The value is a JSON list of [`AssignmentRow`];
//! `model::assignment_changes` turns it into grants and revokes.

use leptos::prelude::*;

use crate::i18n::{K, use_i18n};
use crate::model::{AssignmentRow, Choice};

/// `actors`: the users or groups that can be added; `roles`: every role (ID →
/// name). `default_role` is ticked on a newly added row (Horizon: `member`).
#[component]
pub fn AssignmentRows(
    id: String,
    labelled_by: String,
    initial: String,
    actors: Signal<Option<Vec<Choice>>>,
    roles: Signal<Option<Vec<Choice>>>,
    on_change: Callback<String>,
) -> impl IntoView {
    let i18n = use_i18n();
    let rows: RwSignal<Vec<AssignmentRow>> = RwSignal::new(serde_json::from_str(&initial).unwrap_or_default());
    let sync = move || on_change.run(serde_json::to_string(&rows.get_untracked()).unwrap_or_default());
    let filter = RwSignal::new(String::new());
    let toggle = move |actor: String, role: String, on: bool| {
        rows.update(|r| {
            if let Some(row) = r.iter_mut().find(|x| x.actor_id == actor) {
                row.role_ids.retain(|x| *x != role);
                if on {
                    row.role_ids.push(role);
                }
            }
        });
        sync();
    };
    let add = move |c: Choice| {
        // Horizon gives a new member the `member` role; else the first role.
        let default = roles.get_untracked().and_then(|rs| {
            rs.iter()
                .find(|r| r.label == "member")
                .or(rs.first())
                .map(|r| r.value.clone())
        });
        rows.update(|r| {
            r.push(AssignmentRow {
                actor_id: c.value,
                label: c.label,
                role_ids: default.into_iter().collect(),
            })
        });
        sync();
    };
    let remove = move |actor: String| {
        rows.update(|r| r.retain(|x| x.actor_id != actor));
        sync();
    };
    let available = move || {
        let q = filter.get().to_lowercase();
        let taken: Vec<String> = rows.with(|r| r.iter().map(|x| x.actor_id.clone()).collect());
        actors.get().map(|a| {
            a.into_iter()
                .filter(|c| !taken.contains(&c.value) && c.label.to_lowercase().contains(q.trim()))
                .collect::<Vec<_>>()
        })
    };
    let total_available = move || {
        let taken: Vec<String> = rows.with(|r| r.iter().map(|x| x.actor_id.clone()).collect());
        actors
            .get()
            .map(|a| a.iter().filter(|c| !taken.contains(&c.value)).count())
    };

    // The Allocated/Available picker's layout (`pick.rs`, as in a group's
    // *Manage Members*), with a checkbox per role on each allocated row.
    view! {
        <div class="pick assignment-rows" id=id role="group" aria-labelledby=labelled_by>
            <h4 class="pick-title">{move || i18n.t(K::Allocated)} <span class="badge">{move || rows.with(Vec::len)}</span></h4>
            <div class="pick-scroll">
                <table class="pick-table">
                    <thead><tr>
                        <th scope="col">{move || i18n.t(K::Name)}</th>
                        <th scope="col">{move || i18n.t(K::Roles)}</th>
                        <th class="pick-act"><span class="sr-only">{move || i18n.t(K::Actions)}</span></th>
                    </tr></thead>
                    <tbody>
                        {move || rows.with(Vec::is_empty).then(|| view! { <tr><td class="pick-hint" colspan="99">{i18n.t(K::SelectManyHint)}</td></tr> })}
                        <For each=move || rows.get() key=|r| r.actor_id.clone() let:row>
                            {
                                let actor = row.actor_id.clone();
                                let actor2 = actor.clone();
                                let label = row.label.clone();
                                view! {
                                    <tr class="pick-chosen">
                                        <td class="strong">{row.label.clone()}</td>
                                        <td>
                                            <div class="role-checks">
                                                {move || roles.get().unwrap_or_default().into_iter().map(|r| {
                                                    let (a, rid) = (actor.clone(), r.value.clone());
                                                    let (a2, rid2) = (a.clone(), rid.clone());
                                                    let checked = move || rows.with(|rs| rs.iter().any(|x| x.actor_id == a2 && x.role_ids.contains(&rid2)));
                                                    view! {
                                                        <label class="role-check">
                                                            <input type="checkbox" prop:checked=checked
                                                                on:change=move |ev| toggle(a.clone(), rid.clone(), event_target_checked(&ev))/>
                                                            <span>{r.label.clone()}</span>
                                                        </label>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        </td>
                                        <td class="pick-act">
                                            <button type="button" class="btn btn-sm btn-icon" title=move || i18n.t(K::Deallocate)
                                                aria-label=format!("{}: {}", i18n.t(K::Deallocate), label)
                                                on:click=move |_| remove(actor2.clone())>"↓"</button>
                                        </td>
                                    </tr>
                                }
                            }
                        </For>
                    </tbody>
                </table>
            </div>
            <div class="pick-bar">
                <h4 class="pick-title">{move || i18n.t(K::Available)} " "
                    <span class="badge">{move || total_available().map(|n| n.to_string()).unwrap_or_else(|| "…".into())}</span>
                </h4>
                <input type="search" class="pick-filter" placeholder=move || i18n.t(K::FilterPlaceholder)
                    aria-label=move || i18n.t(K::FilterPlaceholder)
                    prop:value=move || filter.get() on:input=move |ev| filter.set(event_target_value(&ev))/>
            </div>
            <div class="pick-scroll">
                <table class="pick-table">
                    <thead><tr>
                        <th scope="col">{move || i18n.t(K::Name)}</th>
                        <th class="pick-act"><span class="sr-only">{move || i18n.t(K::Actions)}</span></th>
                    </tr></thead>
                    <tbody>
                        {move || match available() {
                            None => view! { <tr><td class="pick-hint" colspan="99">{i18n.t(K::LoadingChoices)}</td></tr> }.into_any(),
                            Some(list) if list.is_empty() => view! { <tr><td class="pick-hint" colspan="99">{i18n.t(K::NoMatches)}</td></tr> }.into_any(),
                            Some(list) => list.into_iter().map(|c| {
                                let label = c.label.clone();
                                view! {
                                    <tr>
                                        <td class="strong">{c.label.clone()}</td>
                                        <td class="pick-act">
                                            <button type="button" class="btn btn-sm btn-icon" title=move || i18n.t(K::Allocate)
                                                aria-label=format!("{}: {}", i18n.t(K::Allocate), label)
                                                on:click=move |_| add(c.clone())>"↑"</button>
                                        </td>
                                    </tr>
                                }
                            }).collect_view().into_any(),
                        }}
                    </tbody>
                </table>
            </div>
        </div>
    }
}
