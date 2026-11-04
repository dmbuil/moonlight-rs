use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use super::instances::IpList;
use super::lists::{admin_state, link};
use crate::api::{get_instance, run_instance_console, run_instance_log};
use crate::components::dialog::{data_version, prefs, RequestList, RowActions};
use crate::components::ops;
use crate::components::table::TimingChip;
use crate::components::widgets::{age_since, dash, is_busy, poll_while_busy, render, short_time, AzPill, Busy, Copyable, LockIcon, Pill, Skeleton, Tone};
use crate::i18n::{bytes, power_state, server_status, task_state, use_i18n, K};
use crate::model::{Address, AppError, ConsoleReply, Instance, InstanceDetail};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Interfaces,
    Log,
    Console,
    Actions,
}

const TABS: [(Tab, K); 5] = [
    (Tab::Overview, K::Overview),
    (Tab::Interfaces, K::Interfaces),
    (Tab::Log, K::LogTab),
    (Tab::Console, K::ConsoleTab),
    (Tab::Actions, K::ActionLog),
];

/// Groups addresses by network, keeping order.
fn by_network(addresses: &[Address]) -> Vec<(String, Vec<Address>)> {
    let mut out: Vec<(String, Vec<Address>)> = Vec::new();
    for a in addresses {
        match out.iter_mut().find(|(n, _)| *n == a.network) {
            Some((_, v)) => v.push(a.clone()),
            None => out.push((a.network.clone(), vec![a.clone()])),
        }
    }
    out
}

#[component]
fn Row(label: K, children: Children) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <dt>{move || i18n.t(label)}</dt>
        <dd>{children()}</dd>
    }
}

#[component]
pub fn InstanceDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let params = use_params_map();
    let id = move || params.read().get("id").unwrap_or_default();
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_instance(id));
    let polling = poll_while_busy(data, move || {
        data.with(|r| matches!(r, Some(Ok(td)) if is_busy(td.data.base.task_state.as_deref(), &td.data.base.status)))
    });
    let tab = RwSignal::new(Tab::Overview);

    view! {
        <nav class="breadcrumb" aria-label="Breadcrumb">
            <span>{move || i18n.t(K::Project)}</span>
            <span class="sep" aria-hidden="true">"/"</span>
            <span>{move || i18n.t(K::Compute)}</span>
            <span class="sep" aria-hidden="true">"/"</span>
            <a href="/project/instances">{move || i18n.t(K::Instances)}</a>
            <span class="sep" aria-hidden="true">"/"</span>
            <Transition fallback=|| ()>
                <span>{move || data.get().and_then(Result::ok).map(|d| d.data.base.name)}</span>
            </Transition>
        </nav>
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let status = d.base.status.clone();
                view! {
                    <Title text=d.base.name.clone()/>
                    <div class="page-header">
                        <h1>
                            <LockIcon locked=d.base.locked/>
                            {d.base.name.clone()}
                            {move || view! { <Pill tone=Tone::server(&status) label=server_status(i18n.lang(), &status)/> }}
                            {is_busy(d.base.task_state.as_deref(), &d.base.status).then(|| {
                                let t = d.base.task_state.clone();
                                let st = d.base.status.clone();
                                view! { <Busy label=Signal::derive(move || t.as_deref().map(|t| task_state(i18n.lang(), t)).unwrap_or_else(|| server_status(i18n.lang(), &st)))/> }
                            })}
                        </h1>
                        <div class="page-actions">
                            {move || polling.get().then(|| view! { <span class="updating">{i18n.t(K::Updating)}</span> })}
                            <TimingChip millis=td.millis calls=td.calls/>
                            {
                                let inst = Instance { locked: d.locked, ..d.base.clone() };
                                move || view! { <RowActions items=ops::instance_items(i18n.lang(), &inst)/> }
                            }
                        </div>
                    </div>
                    <div class="tabs" role="tablist">
                        {TABS.into_iter().map(|(t, k)| view! {
                            <button type="button" role="tab"
                                aria-selected=move || (tab.get() == t).to_string()
                                class:active=move || tab.get() == t
                                on:click=move |_| tab.set(t)>
                                {move || i18n.t(k)}
                            </button>
                        }).collect_view()}
                    </div>
                    <DetailBody d=d tab=tab/>
                }
            })}
        </Transition>
    }
}

#[component]
fn DetailBody(d: InstanceDetail, tab: RwSignal<Tab>) -> impl IntoView {
    let i18n = use_i18n();
    let d = StoredValue::new(d);
    move || match tab.get() {
        Tab::Overview => d.with_value(|d| overview(d.clone())).into_any(),
        Tab::Interfaces => d.with_value(|d| interfaces(d.clone())).into_any(),
        Tab::Actions => d.with_value(|d| actions(d.clone())).into_any(),
        Tab::Log | Tab::Console if !prefs().console.enabled => view! {
            <div class="alert alert-info">{move || i18n.t(K::ConsoleDisabled)}</div>
        }
        .into_any(),
        Tab::Log => view! { <LogTab id=d.with_value(|d| d.base.id.clone())/> }.into_any(),
        Tab::Console if d.with_value(|d| d.base.status != "ACTIVE") => view! {
            <div class="alert alert-info">{move || i18n.t(K::ConsoleOnlyActive)}</div>
        }
        .into_any(),
        Tab::Console => view! { <ConsoleTab id=d.with_value(|d| d.base.id.clone())/> }.into_any(),
    }
}

/// What a *Log* / *Console* call came back with, before its data is shown:
/// the error, or the dry-run preview (`Err`), else the data (`Ok`).
fn console_reply<T>(r: Result<ConsoleReply<T>, AppError>) -> Result<T, AnyView> {
    match r {
        Err(e) => Err(view! { <div class="alert alert-error" role="alert">{e.to_string()}</div> }.into_any()),
        Ok(ConsoleReply { data: Some(v), .. }) => Ok(v),
        Ok(ConsoleReply { requests, .. }) => {
            let i18n = use_i18n();
            Err(view! {
                <div class="alert alert-warn">{move || i18n.t(K::ConsoleDryRun)}</div>
                <RequestList requests=requests/>
            }
            .into_any())
        }
    }
}

/// The last lines of the serial console log, fetched when the tab opens and
/// on *Go* / *View Full Log* (`None` lines = the whole log). No polling.
#[component]
fn LogTab(id: String) -> impl IntoView {
    let i18n = use_i18n();
    let default = prefs().console.log_length;
    let input = RwSignal::new(default.to_string());
    let lines = RwSignal::new(Some(default));
    let round = RwSignal::new(0u32);
    let id = StoredValue::new(id);
    let log = LocalResource::new(move || {
        let (l, _) = (lines.get(), round.get());
        run_instance_log(id.get_value(), l)
    });
    let go = move || {
        lines.set(input.get_untracked().trim().parse::<u32>().ok().filter(|n| *n > 0).or(Some(default)));
        round.update(|r| *r += 1);
    };
    view! {
        <div class="log-bar">
            <label for="log-length">{move || i18n.t(K::LogLength)}</label>
            <input id="log-length" type="number" min="1" prop:value=move || input.get()
                on:input=move |e| input.set(event_target_value(&e))
                on:keydown=move |e| if e.key() == "Enter" { go() }/>
            <button type="button" class="btn" on:click=move |_| go()>{move || i18n.t(K::Go)}</button>
            <button type="button" class="btn" on:click=move |_| {
                lines.set(None);
                round.update(|r| *r += 1);
            }>{move || i18n.t(K::ViewFullLog)}</button>
        </div>
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || log.get().map(|r| match console_reply(r) {
                Err(v) => v,
                Ok(text) if text.is_empty() => view! { <p class="muted">{move || i18n.t(K::LogEmpty)}</p> }.into_any(),
                Ok(text) => view! { <pre class="console-log">{text}</pre> }.into_any(),
            })}
        </Transition>
    }
}

/// The remote console in a frame. Each call makes a console token on the
/// cloud, so it is fetched only when the tab opens.
#[component]
fn ConsoleTab(id: String) -> impl IntoView {
    let i18n = use_i18n();
    let console = LocalResource::new(move || run_instance_console(id.clone()));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || console.get().map(|r| match console_reply(r) {
                Err(v) => v,
                Ok(c) => view! {
                    <p class="console-bar">
                        <span class="muted">{move || i18n.t(K::ConsoleKeyboardHint)}</span>
                        <a href=c.url.clone() target="_blank" rel="noopener noreferrer">{move || i18n.t(K::ConsoleOpenAlone)}</a>
                    </p>
                    <iframe class="console-frame" src=c.url title=move || i18n.t(K::ConsoleTab)></iframe>
                }.into_any(),
            })}
        </Transition>
    }
}

fn overview(d: InstanceDetail) -> impl IntoView {
    let i18n = use_i18n();
    let created = d.base.created.clone();
    let created2 = created.clone();
    let power = d.base.power_state;
    let status = d.base.status.clone();
    let flavor = d.flavor.clone();
    let server_id = d.base.id.clone();
    let image = super::instances::image_cell(&d.base, i18n.lang());
    view! {
        {d.fault.clone().map(|f| view! {
            <div class="alert alert-error" role="alert">
                <strong>{move || i18n.t(K::Fault)}" · "{move || i18n.t(K::Code)}" "{f.code}</strong>
                <code>{f.message}</code>
            </div>
        })}
        <div class="cards">
            <section class="card">
                <h2>{move || i18n.t(K::Information)}</h2>
                <dl class="kv">
                    <Row label=K::Name><Copyable value=d.base.name.clone()/></Row>
                    <Row label=K::Id><Copyable value=d.base.id.clone() mono=true/></Row>
                    <Row label=K::Description>{dash(d.description.clone())}</Row>
                    <Row label=K::Project><Copyable value=d.project_id.clone() mono=true/></Row>
                    <Row label=K::Status>{move || server_status(i18n.lang(), &status)}</Row>
                    <Row label=K::PowerState>{move || power_state(i18n.lang(), power)}</Row>
                    <Row label=K::Task>{move || dash(d.base.task_state.as_deref().map(|t| task_state(i18n.lang(), t)))}</Row>
                    <Row label=K::Locked>{move || i18n.t(if d.locked { K::Yes } else { K::No })}</Row>
                    <Row label=K::AvailabilityZone><AzPill name=d.base.availability_zone.clone()/></Row>
                    <Row label=K::Host><span class="mono">{dash(d.host.clone())}</span></Row>
                    <Row label=K::ServerGroup>{match d.server_group.clone() {
                        Some((id, name)) => link(format!("/project/server_groups/{id}"), name),
                        None => "-".into_any(),
                    }}</Row>
                    <Row label=K::Created>{short_time(&created)}</Row>
                    <Row label=K::Age>{move || age_since(i18n.lang(), &created2)}</Row>
                </dl>
            </section>
            <section class="card">
                <h2>{move || i18n.t(K::Specs)}</h2>
                <dl class="kv">
                    <Row label=K::Flavor>{flavor.name.clone()}</Row>
                    <Row label=K::FlavorId><span class="mono">{flavor.id.clone()}</span></Row>
                    <Row label=K::Ram>{flavor.ram_mb.map(|m| bytes(m as u64 * 1024 * 1024)).unwrap_or_else(|| "-".into())}</Row>
                    <Row label=K::Vcpus>{flavor.vcpus.map(|v| v.to_string()).unwrap_or_else(|| "-".into())}</Row>
                    <Row label=K::Disk>{flavor.disk_gb.map(|g| format!("{g} GB")).unwrap_or_else(|| "-".into())}</Row>
                    <Row label=K::Ephemeral>{flavor.ephemeral_gb.map(|g| format!("{g} GB")).unwrap_or_else(|| "-".into())}</Row>
                    <Row label=K::Swap>{flavor.swap_mb.map(|m| format!("{m} MB")).unwrap_or_else(|| "-".into())}</Row>
                </dl>
            </section>
            <section class="card">
                <h2>{move || i18n.t(K::IpAddresses)}</h2>
                <dl class="kv">
                    {by_network(&d.base.addresses).into_iter().map(|(net, addrs)| view! {
                        <dt>{net}</dt>
                        <dd><IpList addresses=addrs/></dd>
                    }).collect_view()}
                </dl>
            </section>
            <section class="card">
                <h2>{move || i18n.t(K::SecurityGroups)}</h2>
                <ul class="plain sg-rules">
                    {d.security_groups.iter().map(|g| {
                        let rules = d.security_group_rules.iter().find(|(n, _)| n == g).map(|(_, r)| r.clone()).unwrap_or_default();
                        view! {
                            <li>
                                <strong>{g.clone()}</strong>
                                {(!rules.is_empty()).then(|| view! {
                                    <ul class="plain muted mono">
                                        {rules.into_iter().map(|r| view! { <li>{r}</li> }).collect_view()}
                                    </ul>
                                })}
                            </li>
                        }
                    }).collect_view()}
                </ul>
            </section>
            <section class="card">
                <h2>{move || i18n.t(K::Metadata)}</h2>
                <dl class="kv">
                    <Row label=K::KeyPair><span class="mono">{dash(d.base.key_name.clone())}</span></Row>
                    <Row label=K::ImageName>{image}</Row>
                    <Row label=K::ImageId>{match d.image_id.clone() {
                        Some(id) => view! { <Copyable value=id mono=true/> }.into_any(),
                        None => "-".into_any(),
                    }}</Row>
                    {d.metadata.iter().map(|(k, v)| view! {
                        <dt class="mono">{k.clone()}</dt>
                        <dd>{v.clone()}</dd>
                    }).collect_view()}
                </dl>
            </section>
            <section class="card">
                <h2>{move || i18n.t(K::VolumesAttached)}</h2>
                {if d.volumes.is_empty() {
                    view! { <p class="muted">{move || i18n.t(K::NoneValue)}</p> }.into_any()
                } else {
                    view! {
                        <ul class="plain">
                            {d.volumes.iter().map(|v| {
                                let name = v.name.clone().unwrap_or_else(|| v.id.clone());
                                let device = v.device.clone();
                                let detach = ops::detach_volume_item(i18n.lang(), &server_id, &v.id, &name);
                                view! {
                                    <li class="vol-line">
                                        <a href=format!("/project/volumes/{}", v.id)>{name}</a>
                                        {device.map(|dv| view! {
                                            " "<span class="muted">{move || i18n.t(K::On)}</span>" "
                                            <span class="mono">{dv}</span>
                                        })}
                                        {v.size_gb.map(|s| view! { <span class="muted">" · "{s}" GB"</span> })}
                                        <RowActions items=vec![detach]/>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    }.into_any()
                }}
            </section>
        </div>
    }
}

fn interfaces(d: InstanceDetail) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="table-wrap">
            <table class="table">
                <thead>
                    <tr>
                        <th>{move || i18n.t(K::Name)}</th>
                        <th>{move || i18n.t(K::Network)}</th>
                        <th>{move || i18n.t(K::FixedIps)}</th>
                        <th>{move || i18n.t(K::MacAddress)}</th>
                        <th>{move || i18n.t(K::Status)}</th>
                        <th>{move || i18n.t(K::AdminState)}</th>
                        <th class="actions">{move || i18n.t(K::Actions)}</th>
                    </tr>
                </thead>
                <tbody>
                    {if d.interfaces.is_empty() {
                        view! { <tr><td class="empty" colspan="7">{move || i18n.t(K::NoItems)}</td></tr> }.into_any()
                    } else {
                        let server_id = d.base.id.clone();
                        d.interfaces.into_iter().map(|it| {
                            let state = it.state.clone();
                            // Like Horizon: the port's name, else the start of its ID.
                            let name = it.port.as_ref().map(|p| p.name.clone()).filter(|n| !n.is_empty());
                            let (label, mono) = match name {
                                Some(n) => (n, ""),
                                None => (format!("({})", &it.port_id[..it.port_id.len().min(13)]), "mono"),
                            };
                            let admin = it.port.as_ref().map(|p| p.admin_up);
                            let (sid, row) = (server_id.clone(), it.clone());
                            view! {
                                <tr>
                                    <td class=mono><a href=format!("/project/networks/ports/{}", it.port_id) title=it.port_id.clone()>{label}</a></td>
                                    <td><a href=format!("/project/networks/{}", it.network_id)>{it.network_name.unwrap_or(it.network_id.clone())}</a></td>
                                    <td class="mono">{it.fixed_ips.join(", ")}</td>
                                    <td class="mono">{it.mac}</td>
                                    <td><Pill tone=Tone::resource(&state) label=crate::i18n::resource_status(i18n.lang(), &state)/></td>
                                    <td>{move || admin.map_or("-", |up| admin_state(i18n.lang(), up))}</td>
                                    <td class="actions">
                                        {move || view! { <RowActions items=ops::instance_interface_items(i18n.lang(), &sid, &row)/> }}
                                    </td>
                                </tr>
                            }
                        }).collect_view().into_any()
                    }}
                </tbody>
            </table>
        </div>
    }
}

fn actions(d: InstanceDetail) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="table-wrap">
            <table class="table">
                <thead>
                    <tr>
                        <th>{move || i18n.t(K::RequestId)}</th>
                        <th>{move || i18n.t(K::Action)}</th>
                        <th>{move || i18n.t(K::StartTime)}</th>
                        <th>{move || i18n.t(K::UserId)}</th>
                        <th>{move || i18n.t(K::Message)}</th>
                    </tr>
                </thead>
                <tbody>
                    {if d.actions.is_empty() {
                        view! { <tr><td class="empty" colspan="5">{move || i18n.t(K::NoItems)}</td></tr> }.into_any()
                    } else {
                        d.actions.into_iter().map(|a| view! {
                            <tr>
                                <td class="mono">{a.request_id}</td>
                                <td>{a.action}</td>
                                <td class="nowrap">{short_time(&a.start_time)}</td>
                                <td class="mono">{dash(a.user_id)}</td>
                                <td>{dash(a.message)}</td>
                            </tr>
                        }).collect_view().into_any()
                    }}
                </tbody>
            </table>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_addresses_by_network_in_order() {
        let a = |n: &str, ip: &str| Address { network: n.into(), addr: ip.into(), floating: false, mac: None };
        let g = by_network(&[a("b", "1"), a("a", "2"), a("b", "3")]);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].0, "b");
        assert_eq!(g[0].1.len(), 2);
    }
}
