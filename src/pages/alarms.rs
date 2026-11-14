//! Monitoring › Alarms (Aodh): the project's alarms and an alarm's
//! page with its rule, actions and history. *Create Alarm* and *Edit Alarm*
//! ([`ops::alarm_dialog`]); delete, enable/disable and set state are the item's next slice.
//! Upstream Horizon has no maintained Alarming panel, so the layout follows
//! Moonlight's other plugin panels (Stacks).

use std::sync::Arc;

use leptos::prelude::*;

use super::details::{Card, DetailHeader, Kv, Tabs, lines, mono, param, yes_no};
use super::lists::link;
use crate::api::*;
use crate::components::dialog::{CreateButton, RowActions, data_version};
use crate::components::ops;
use crate::components::table::{Col, DataTable, col};
use crate::components::widgets::{Copyable, PageHeader, Pill, Skeleton, Tone, dash, render, short_time};
use crate::i18n::{K, Lang, t, use_i18n};
use crate::model::*;

/// Aodh's state as a pill: `alarm` red, `ok` green, `insufficient data` (and
/// anything else) grey.
fn state_pill(state: &str, l: Lang) -> AnyView {
    let (tone, label) = match state {
        "alarm" => (Tone::Err, t(l, K::StateAlarm).to_string()),
        "ok" => (Tone::Ok, t(l, K::StateOk).to_string()),
        "insufficient data" => (Tone::Off, t(l, K::StateInsufficientData).to_string()),
        other => (Tone::Off, other.to_string()),
    };
    view! { <Pill tone=tone label=label/> }.into_any()
}

/// The severity as a pill (`critical` red, `moderate` amber, `low` grey), or `-`.
fn severity_pill(severity: Option<&str>, l: Lang) -> AnyView {
    let (tone, label) = match severity {
        None => return "-".into_any(),
        Some("critical") => (Tone::Err, t(l, K::SeverityCritical).to_string()),
        Some("moderate") => (Tone::Warn, t(l, K::SeverityModerate).to_string()),
        Some("low") => (Tone::Off, t(l, K::SeverityLow).to_string()),
        Some(other) => (Tone::Off, other.to_string()),
    };
    view! { <Pill tone=tone label=label/> }.into_any()
}

/// A history entry's kind, translated; unknown kinds as Aodh names them.
fn change_label(kind: &str, l: Lang) -> String {
    match kind {
        "creation" => t(l, K::ChangeCreation).into(),
        "rule change" => t(l, K::ChangeRule).into(),
        "state transition" => t(l, K::ChangeState).into(),
        "deletion" => t(l, K::ChangeDeletion).into(),
        other => other.into(),
    }
}

/// Monitoring › Alarms' columns: the name links to the alarm's page, then type, state,
/// severity (sorted by rank, not alphabetically), enabled, condition and the time of the last
/// state change.
fn alarm_columns() -> Vec<Col<AlarmRow>> {
    vec![
        col(K::Name, |x: &AlarmRow, _| {
            link(format!("/project/alarms/{}", x.id), x.name.clone())
        })
        .sort_text(|x| x.name.to_lowercase()),
        col(K::Type, |x: &AlarmRow, _| mono(x.alarm_type.clone())).sort_text(|x| x.alarm_type.clone()),
        col(K::AlarmState, |x: &AlarmRow, l| state_pill(&x.state, l)).sort_text(|x| x.state.clone()),
        col(K::Severity, |x: &AlarmRow, l| severity_pill(x.severity.as_deref(), l))
            .sort_num(|x| severity_rank(x.severity.as_deref())),
        col(K::EnabledCol, |x: &AlarmRow, l| yes_no(l, x.enabled).into_any()).sort_num(|x| x.enabled as i64),
        col(K::Condition, |x: &AlarmRow, _| mono(x.condition.clone())).class("condition"),
        col(K::StateSince, |x: &AlarmRow, _| {
            dash(x.state_since.as_deref().map(short_time))
        })
        .sort_text(|x| x.state_since.clone().unwrap_or_default())
        .class("nowrap"),
        col(
            K::Actions,
            |x: &AlarmRow, l| view! { <RowActions items=ops::alarm_items(l, x)/> },
        )
        .class("actions"),
    ]
}

/// Sorts severities by gravity rather than alphabetically.
fn severity_rank(s: Option<&str>) -> i64 {
    match s {
        Some("critical") => 3,
        Some("moderate") => 2,
        Some("low") => 1,
        _ => 0,
    }
}

/// Monitoring › Alarms (`alarming` plugin).
#[component]
pub fn AlarmsPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_alarms());
    let columns = StoredValue::new(alarm_columns());
    view! {
        <PageHeader>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateAlarm).to_string())
                open=Arc::new(move || ops::alarm_dialog(i18n.lang(), None))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |td| view! {
                <DataTable
                    rows=td.data
                    columns=columns.get_value()
                    search=|x: &AlarmRow| format!("{} {} {} {} {} {}", x.name, x.id, x.alarm_type, x.state, x.severity.as_deref().unwrap_or(""), x.condition)
                    timing=(td.millis, td.calls)
                    on_refresh=move |_| data.refetch()
                />
            })}
        </Transition>
    }
}

/// The alarm page: *Overview* (information, rule, actions, time constraints)
/// and *History*.
#[component]
pub fn AlarmDetailPage() -> impl IntoView {
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(id, _)| get_alarm(id));
    let i18n = use_i18n();
    let tab = RwSignal::new(0usize);
    view! {
        <Transition fallback=|| view! { <Skeleton rows=8/> }>
            {move || render(data.get(), move |td| {
                let name = td.data.alarm.name.clone();
                let row = td.data.alarm.clone();
                let d = StoredValue::new(td.data);
                let items = Signal::derive(move || ops::alarm_items(i18n.lang(), &row));
                view! {
                    <DetailHeader group=K::Monitoring panel=K::Alarms href="/project/alarms"
                        name=name timing=(td.millis, td.calls) items=items/>
                    <Tabs labels=vec![K::Overview, K::AlarmHistory] active=tab/>
                    {move || match tab.get() {
                        1 => history_tab(&d.read_value().history),
                        _ => overview_tab(d.get_value()),
                    }}
                }
            })}
        </Transition>
    }
}

/// A threshold with its operator (`> 80`), or `-`.
fn compared(comparison: &Option<String>, threshold: Option<f64>) -> String {
    match (comparison, threshold) {
        (Some(c), Some(t)) => format!("{} {t}", comparison_symbol(c)),
        (None, Some(t)) => t.to_string(),
        _ => "-".into(),
    }
}

/// The *Rule* card's rows for the alarm's type.
fn rule_rows(rule: AlarmRule, l: Lang) -> AnyView {
    match rule {
        AlarmRule::Threshold {
            metrics,
            resource_type,
            resource_id,
            resource_href,
            query,
            aggregation,
            comparison,
            threshold,
            granularity,
            evaluation_periods,
        } => {
            let resource = match (resource_id, resource_href) {
                (Some(id), Some(href)) => link(href, id),
                (Some(id), None) => mono(id),
                _ => "-".into_any(),
            };
            view! {
                <Kv label=K::Metric>{lines(metrics)}</Kv>
                <Kv label=K::ResourceType>{dash(resource_type)}</Kv>
                <Kv label=K::ResourceId>{resource}</Kv>
                {query.map(|q| view! { <Kv label=K::Query>{mono(q)}</Kv> })}
                <Kv label=K::Aggregation>{dash(aggregation)}</Kv>
                <Kv label=K::Threshold>{mono(compared(&comparison, threshold))}</Kv>
                <Kv label=K::Granularity>{dash(granularity.map(|g| format!("{g} {}", t(l, K::Seconds))))}</Kv>
                <Kv label=K::EvaluationPeriods>{dash(evaluation_periods.map(|n| n.to_string()))}</Kv>
            }
            .into_any()
        }
        AlarmRule::Prometheus {
            query,
            comparison,
            threshold,
        } => view! {
            <Kv label=K::Query><pre class="console-log">{query}</pre></Kv>
            <Kv label=K::Threshold>{mono(compared(&comparison, threshold))}</Kv>
        }
        .into_any(),
        AlarmRule::Event { event_type, query } => view! {
            <Kv label=K::EventType>{mono(event_type)}</Kv>
            <Kv label=K::Query>{lines(query)}</Kv>
        }
        .into_any(),
        AlarmRule::Raw(json) if json.is_empty() => view! { <Kv label=K::AlarmRuleCard>"-"</Kv> }.into_any(),
        AlarmRule::Raw(json) => view! { <pre class="console-log">{json}</pre> }.into_any(),
    }
}

/// *Time constraints* on the Overview: Aodh's cron windows when the alarm is evaluated (start,
/// duration in seconds, timezone).
fn constraint_columns() -> Vec<Col<AlarmTimeConstraint>> {
    vec![
        col(K::Name, |c: &AlarmTimeConstraint, _| c.name.clone().into_any()).sort_text(|c| c.name.clone()),
        col(K::CronStart, |c: &AlarmTimeConstraint, _| mono(c.start.clone())),
        col(K::Duration, |c: &AlarmTimeConstraint, l| {
            dash(c.duration.map(|d| format!("{d} {}", t(l, K::Seconds))))
        }),
        col(K::Timezone, |c: &AlarmTimeConstraint, _| dash(c.timezone.clone())),
        col(K::Description, |c: &AlarmTimeConstraint, _| dash(c.description.clone())),
    ]
}

/// *Overview*: what the alarm is, what it watches, what it calls and when it is evaluated.
fn overview_tab(d: AlarmDetail) -> AnyView {
    let i18n = use_i18n();
    let l = i18n.lang();
    let x = d.alarm;
    let constraints = if d.time_constraints.is_empty() {
        view! { <p class="muted">{move || i18n.t(K::AlwaysEvaluated)}</p> }.into_any()
    } else {
        let cols = constraint_columns();
        view! { <DataTable rows=d.time_constraints.clone() columns=cols search=|c: &AlarmTimeConstraint| format!("{} {}", c.name, c.start)/> }.into_any()
    };
    let (state, severity, enabled, repeat) = (x.state.clone(), x.severity.clone(), x.enabled, d.repeat_actions);
    view! {
        <div class="cards">
            <Card title=K::Information>
                <Kv label=K::Name><Copyable value=x.name.clone()/></Kv>
                <Kv label=K::Id><Copyable value=x.id.clone() mono=true/></Kv>
                <Kv label=K::Description>{dash(x.description.clone())}</Kv>
                <Kv label=K::Type>{mono(x.alarm_type.clone())}</Kv>
                <Kv label=K::AlarmState>{move || state_pill(&state, i18n.lang())}</Kv>
                <Kv label=K::StatusReason>{dash(x.state_reason.clone())}</Kv>
                <Kv label=K::StateSince>{dash(x.state_since.as_deref().map(short_time))}</Kv>
                <Kv label=K::Severity>{move || severity_pill(severity.as_deref(), i18n.lang())}</Kv>
                <Kv label=K::EnabledCol>{move || yes_no(i18n.lang(), enabled)}</Kv>
                <Kv label=K::Updated>{dash(x.updated.as_deref().map(short_time))}</Kv>
                <Kv label=K::ProjectId>{dash(d.project_id.clone())}</Kv>
                <Kv label=K::UserId>{dash(d.user_id.clone())}</Kv>
            </Card>
            <Card title=K::AlarmRuleCard>{rule_rows(d.rule, l)}</Card>
            <Card title=K::AlarmActionsCard>
                <Kv label=K::OnAlarm>{lines(d.alarm_actions)}</Kv>
                <Kv label=K::OnOk>{lines(d.ok_actions)}</Kv>
                <Kv label=K::OnInsufficientData>{lines(d.insufficient_data_actions)}</Kv>
                <Kv label=K::RepeatActions>{move || yes_no(i18n.lang(), repeat)}</Kv>
            </Card>
        </div>
        <h2 class="section-title">{move || i18n.t(K::TimeConstraints)}</h2>
        {constraints}
    }
    .into_any()
}

/// *History*' columns: Aodh's changes to the alarm (creation, rule, state transitions), with
/// the new state when the change set one.
fn history_columns() -> Vec<Col<AlarmChange>> {
    vec![
        col(K::EventTime, |h: &AlarmChange, _| short_time(&h.time))
            .sort_text(|h| h.time.clone())
            .class("nowrap"),
        col(K::Change, |h: &AlarmChange, l| change_label(&h.kind, l).into_any()).sort_text(|h| h.kind.clone()),
        col(K::AlarmState, |h: &AlarmChange, l| match h.state.as_deref() {
            Some(s) => state_pill(s, l),
            None => "-".into_any(),
        }),
        col(K::Details, |h: &AlarmChange, _| dash(h.detail.clone())).class("condition"),
        col(K::UserId, |h: &AlarmChange, _| dash(h.user_id.clone())),
    ]
}

/// *History* tab: the alarm's changes, newest first (sorted by the mapper), or a note when it
/// has none.
fn history_tab(rows: &[AlarmChange]) -> AnyView {
    let i18n = use_i18n();
    if rows.is_empty() {
        return view! { <p class="muted">{move || i18n.t(K::NoAlarmHistory)}</p> }.into_any();
    }
    let (rows, cols) = (rows.to_vec(), history_columns());
    view! {
        <DataTable rows=rows columns=cols
            search=|h: &AlarmChange| format!("{} {} {}", h.kind, h.state.as_deref().unwrap_or(""), h.detail.as_deref().unwrap_or(""))/>
    }
    .into_any()
}
