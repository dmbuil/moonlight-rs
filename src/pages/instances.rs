use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::dialog::{BulkButton, CreateButton, RowActions, data_version};
use crate::components::ops;

use std::collections::HashMap;

use crate::api::{get_boot_images, list_instances};
use crate::components::pager::{PagerState, paged_table, use_list_query};
use crate::components::table::{Select, col};
use crate::components::widgets::{
    AzPill, Busy, FlavorPopover, LockIcon, PageHeader, Pill, Skeleton, Tone, age_cell_since, dash, is_busy,
    poll_while_busy, render,
};
use crate::i18n::{K, Lang, power_state, server_status, t, task_state, use_i18n};
use crate::model::{Address, BOOT_IMAGE_CHUNK, Instance, ListScope, OsAction, PagedList};

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
        }
        .into_any(),
        (Some(n), false) => n.clone().into_any(),
        (None, _) => "-".into_any(),
    }
}

/// Images of a paged page's boot-from-volume rows by server ID, filled after the
/// table shows (roadmap 88): absent = still loading, `None` = no image found.
pub type BootImages = RwSignal<HashMap<String, Option<String>>>;

/// The *Image* cell: Nova's or the boot volume's image, or, on a paged page, the
/// one [`BootImages`] fills in later (a placeholder until then).
pub fn late_image_cell(i: &Instance, l: Lang, boot: BootImages) -> AnyView {
    if i.image_name.is_some() || i.boot_volume_ids.is_empty() {
        return image_cell(i, l);
    }
    let id = i.id.clone();
    (move || match boot.with(|m| m.get(&id).cloned()) {
        None => view! { <span class="muted" title=t(l, K::LoadingChoices)>"…"</span> }.into_any(),
        Some(None) => "-".into_any(),
        Some(Some(n)) => {
            view! { <span>{n}</span>" "<span class="tag" title=t(l, K::BootFromVolume)>{t(l, K::FromVolume)}</span> }
                .into_any()
        }
    })
    .into_any()
}

/// The rows' boot volumes in [`BOOT_IMAGE_CHUNK`]-sized requests, as `(servers,
/// volume IDs)`. A server's volumes are never split across two requests: the boot
/// volume is picked among all of them. A server with more volumes than a chunk
/// gets a request of its own.
pub fn boot_chunks(rows: &[(String, Vec<String>)]) -> Vec<(Vec<String>, Vec<String>)> {
    let mut out: Vec<(Vec<String>, Vec<String>)> = Vec::new();
    for (server, vols) in rows.iter().filter(|(_, v)| !v.is_empty()) {
        match out.last_mut() {
            Some((servers, ids)) if ids.len() + vols.len() <= BOOT_IMAGE_CHUNK => {
                servers.push(server.clone());
                ids.extend(vols.iter().cloned());
            }
            _ => out.push((vec![server.clone()], vols.clone())),
        }
    }
    out
}

/// Names the images of a paged page's boot-from-volume rows (`(server ID, its
/// boot_volume_ids)`) into `boot`, one chunk at a time, after the table shows (the
/// server reads FAN_OUT volumes at once): reading them before cost ≈10 s a page on
/// ttcp-dev, which boots every server from a volume. A whole list's rows come with
/// their images, so there is nothing to read.
pub fn load_boot_images(rows: Vec<(String, Vec<String>)>, boot: BootImages) {
    let chunks = boot_chunks(&rows);
    if chunks.is_empty() {
        return;
    }
    leptos::task::spawn_local(async move {
        for (servers, ids) in chunks {
            let found: HashMap<String, String> = get_boot_images(ids)
                .await
                .map(|t| t.data.into_iter().collect())
                .unwrap_or_default();
            // A failed chunk or a server without an image shows "-", not "…" forever.
            let _ = boot.try_update(|m| {
                for s in servers {
                    let name = found.get(&s).cloned();
                    m.insert(s, name);
                }
            });
        }
    });
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
    let query = use_list_query();
    let data = Resource::new(move || (query.get(), data_version()), |(q, _)| list_instances(q));
    let polling = poll_while_busy(data, move || {
        data.with(
            |r| matches!(r, Some(Ok(td)) if td.data.rows.iter().any(|i| is_busy(i.task_state.as_deref(), &i.status))),
        )
    });
    // A paged page (roadmap 88 slice E) names its boot-from-volume rows' images later.
    let boot: BootImages = RwSignal::new(HashMap::new());
    Effect::new(move |_| {
        let Some(Ok(t)) = data.get() else { return };
        if t.data.paged {
            load_boot_images(
                t.data
                    .rows
                    .iter()
                    .map(|i| (i.id.clone(), i.boot_volume_ids.clone()))
                    .collect(),
                boot,
            );
        }
    });
    let stack = PagerState::new();
    let columns = vec![
        col(K::InstanceName, |i: &Instance, _| {
            let href = format!("/project/instances/{}", i.id);
            let name = i.name.clone();
            view! { <span class="name-cell"><LockIcon locked=i.locked/><A href=href attr:class="strong">{name}</A></span> }
        })
        .sort_text(|i| i.name.clone()),
        col(K::ImageName, move |i: &Instance, l| late_image_cell(i, l, boot))
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
    let select = Select {
        key: |i: &Instance| Some((i.id.clone(), i.name.clone())),
        picked: RwSignal::new(Vec::new()),
    };
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
            {move || render(data.get(), move |td| {
                paged_table(td, PagedList::Instances, ListScope::Project, columns.get_value(), search, Some(select), None, stack, Callback::new(move |_| data.refetch()))
            })}
        </Transition>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_volumes_go_in_chunks_without_splitting_a_server() {
        let ids = |p: &str, n: usize| (0..n).map(|i| format!("{p}{i}")).collect::<Vec<_>>();
        let rows = vec![
            ("a".to_string(), ids("a", 30)),
            ("img".to_string(), vec![]),
            ("b".to_string(), ids("b", 20)),
            ("c".to_string(), ids("c", 1)),
            ("big".to_string(), ids("x", BOOT_IMAGE_CHUNK + 5)),
        ];
        let chunks = boot_chunks(&rows);
        let shape: Vec<(Vec<&str>, usize)> = chunks
            .iter()
            .map(|(s, v)| (s.iter().map(String::as_str).collect(), v.len()))
            .collect();
        assert_eq!(
            shape,
            [
                (vec!["a", "b"], 50),
                (vec!["c"], 1),
                (vec!["big"], BOOT_IMAGE_CHUNK + 5)
            ],
            "a and b fill a chunk; c would make 51, so it starts the next; a server is never split; rows without volumes ask nothing"
        );
        assert!(boot_chunks(&[]).is_empty());
    }

    #[test]
    fn task_label_prefers_the_task_then_a_transitional_status() {
        assert_eq!(
            task_label(Some("spawning"), "BUILD", Lang::EnUs).as_deref(),
            Some("Spawning")
        );
        assert_eq!(task_label(None, "BUILD", Lang::EnUs).as_deref(), Some("Build"));
        assert_eq!(task_label(Some(""), "ACTIVE", Lang::EnUs), None);
        assert_eq!(task_label(None, "ACTIVE", Lang::EnUs), None);
    }
}
