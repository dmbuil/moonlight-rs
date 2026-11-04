use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::dialog::{data_version, BulkButton, CreateButton, RowActions};
use crate::components::ops;

use crate::api::list_instances;
use crate::components::table::{col, DataTable, Select};
use crate::components::widgets::{age_cell_since, dash, is_busy, poll_while_busy, render, AzPill, Busy, FlavorPopover, LockIcon, PageHeader, Pill, Skeleton, Tone};
use crate::i18n::{power_state, server_status, t, task_state, use_i18n, K};
use crate::model::{Address, Instance, OsAction};

#[component]
pub fn IpList(addresses: Vec<Address>) -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    if addresses.is_empty() {
        return view! { <span>"-"</span> }.into_any();
    }
    view! {
        <ul class="ip-list">
            {addresses.into_iter().map(|a| view! {
                <li>
                    <span class="mono">{a.addr}</span>
                    {a.floating.then(|| view! { <span class="tag">{move || i18n.t(K::Floating)}</span> })}
                </li>
            }).collect_view()}
        </ul>
    }
    .into_any()
}

/// Text of the *Task* column: the localised task, or the status when the
/// instance is busy without one (e.g. `BUILD`), so the bar is never unlabelled.
fn task_label(task: Option<&str>, status: &str, l: crate::i18n::Lang) -> Option<String> {
    match task.filter(|t| !t.is_empty()) {
        Some(t) => Some(task_state(l, t)),
        None => is_busy(None, status).then(|| server_status(l, status)),
    }
}

/// Task name plus an indeterminate bar while the instance is busy.
pub fn task_cell(i: &Instance, l: crate::i18n::Lang) -> AnyView {
    let busy = is_busy(i.task_state.as_deref(), &i.status);
    let label = task_label(i.task_state.as_deref(), &i.status, l);
    view! {
        <span class="task-cell">
            <span>{dash(label.clone())}</span>
            {busy.then(|| view! { <Busy label=label.clone().unwrap_or_default()/> })}
        </span>
    }
    .into_any()
}

/// Image name; for boot-from-volume, the boot volume's image with a "from volume" tag.
pub fn image_cell(i: &Instance, l: crate::i18n::Lang) -> AnyView {
    match (&i.image_name, i.image_from_volume) {
        (Some(n), true) => view! {
            <span>{n.clone()}</span>" "<span class="tag" title=t(l, K::BootFromVolume)>{t(l, K::FromVolume)}</span>
        }.into_any(),
        (Some(n), false) => n.clone().into_any(),
        (None, _) => "-".into_any(),
    }
}

fn search(i: &Instance) -> String {
    let ips: Vec<&str> = i.addresses.iter().map(|a| a.addr.as_str()).collect();
    format!(
        "{} {} {} {} {} {} {} {}",
        i.name,
        i.id,
        i.image_name.as_deref().unwrap_or(""),
        ips.join(" "),
        i.flavor_name,
        i.key_name.as_deref().unwrap_or(""),
        i.status,
        i.availability_zone.as_deref().unwrap_or("")
    )
}

#[component]
pub fn InstancesPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_instances());
    let polling = poll_while_busy(data, move || {
        data.with(|r| matches!(r, Some(Ok(td)) if td.data.iter().any(|i| is_busy(i.task_state.as_deref(), &i.status))))
    });
    let columns = vec![
        col(K::InstanceName, |i: &Instance, _| {
            let href = format!("/project/instances/{}", i.id);
            let name = i.name.clone();
            view! { <span class="name-cell"><LockIcon locked=i.locked/><A href=href attr:class="strong">{name}</A></span> }
        })
        .sort_text(|i| i.name.clone()),
        col(K::ImageName, |i: &Instance, l| image_cell(i, l))
        .sort_text(|i| i.image_name.clone().unwrap_or_default()),
        col(K::IpAddress, |i: &Instance, _| view! { <IpList addresses=i.addresses.clone()/> }),
        col(K::Flavor, |i: &Instance, _| view! { <FlavorPopover flavor=i.flavor.clone() key=i.id.clone()/> })
            .sort_text(|i| i.flavor_name.clone()),
        col(K::KeyPair, |i: &Instance, _| dash(i.key_name.clone())).sort_text(|i| i.key_name.clone().unwrap_or_default()),
        col(K::Status, |i: &Instance, l| view! { <Pill tone=Tone::server(&i.status) label=server_status(l, &i.status)/> }.into_any())
            .sort_text(|i| i.status.clone()),
        col(K::AvailabilityZone, |i: &Instance, _| view! { <AzPill name=i.availability_zone.clone()/> })
            .sort_text(|i| i.availability_zone.clone().unwrap_or_default()),
        col(K::Task, |i: &Instance, l| task_cell(i, l))
            .sort_text(|i| i.task_state.clone().unwrap_or_default()),
        col(K::PowerState, |i: &Instance, l| power_state(l, i.power_state)).sort_num(|i| i.power_state),
        col(K::Age, |i: &Instance, l| age_cell_since(l, &i.created))
            .sort_text(|i| i.created.clone()),
        col(K::Actions, |i: &Instance, l| {
            view! { <RowActions items=ops::instance_items(l, i)/> }
        })
        .class("actions"),
    ];
    let columns = StoredValue::new(columns);
    let select = Select { key: |i: &Instance| Some((i.id.clone(), i.name.clone())), picked: RwSignal::new(Vec::new()) };
    view! {
        <PageHeader>
            {move || polling.get().then(|| view! { <span class="updating">{i18n.t(K::Updating)}</span> })}
            {move || {
                let picked = select.picked.get();
                (!picked.is_empty()).then(|| view! { <RowActions items=ops::bulk_instance_items(i18n.lang(), picked)/> })
            }}
            <BulkButton label=K::DeleteInstances picked=select.picked
                open=Arc::new(|l, items| ops::bulk_delete(l, K::DeleteInstances, items, |id| OsAction::ServerDelete { id }))/>
            <CreateButton
                label=Signal::derive(move || i18n.t(K::LaunchInstance).to_string())
                open=Arc::new(move || ops::launch_dialog(i18n.lang()))
            />
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), |td| view! {
                <DataTable
                    rows=td.data
                    columns=columns.get_value()
                    search=search
                    timing=(td.millis, td.calls)
                    on_refresh=move |_| data.refetch()
                    select=select
                />
            })}
        </Transition>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    #[test]
    fn task_label_prefers_the_task_then_a_transitional_status() {
        assert_eq!(task_label(Some("spawning"), "BUILD", Lang::EnUs).as_deref(), Some("Spawning"));
        assert_eq!(task_label(None, "BUILD", Lang::EnUs).as_deref(), Some("Build"));
        assert_eq!(task_label(Some(""), "ACTIVE", Lang::EnUs), None);
        assert_eq!(task_label(None, "ACTIVE", Lang::EnUs), None);
    }
}
