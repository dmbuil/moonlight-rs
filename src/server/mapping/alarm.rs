//! Aodh: alarms and their history.

use super::*;

/// Aodh `GET v2/alarms` → the *Alarms* list, by name.
pub fn alarms(raw: &[Value]) -> Vec<AlarmRow> {
    let mut out: Vec<_> = raw.iter().map(|x| alarm_row(x, &alarm_rule(x))).collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// One alarm, its rule already summarised by `alarm_rule`; unnamed alarms show their ID.
fn alarm_row(x: &Value, rule: &AlarmRule) -> AlarmRow {
    let id = s(x, "alarm_id").unwrap_or_default();
    AlarmRow {
        name: s_ne(x, "name").unwrap_or_else(|| id.clone()),
        id,
        description: s_ne(x, "description"),
        alarm_type: s(x, "type").unwrap_or_default(),
        state: s(x, "state").unwrap_or_default(),
        state_reason: s_ne(x, "state_reason"),
        state_since: s_ne(x, "state_timestamp"),
        severity: s_ne(x, "severity"),
        // Aodh defaults to enabled; a missing field is not "disabled".
        enabled: x.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        condition: condition(rule),
        updated: s_ne(x, "timestamp"),
        spec: alarm_spec(x),
    }
}

/// The alarm as *Edit Alarm* starts from: Aodh's PUT replaces the whole alarm,
/// so everything the form doesn't show (time constraints) is carried over.
fn alarm_spec(x: &Value) -> AlarmSpec {
    let kind = s(x, "type").unwrap_or_default();
    let rule = x.get(format!("{kind}_rule")).unwrap_or(&Value::Null);
    let text = |k| s(rule, k).unwrap_or_default();
    AlarmSpec {
        name: s(x, "name").unwrap_or_default(),
        description: s(x, "description").unwrap_or_default(),
        severity: s_ne(x, "severity").unwrap_or_else(|| "low".into()),
        enabled: x.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        repeat_actions: b(x, "repeat_actions"),
        alarm_actions: strs(x, "alarm_actions"),
        ok_actions: strs(x, "ok_actions"),
        insufficient_data_actions: strs(x, "insufficient_data_actions"),
        time_constraints: time_constraints(x),
        query: text("query"),
        comparison: text("comparison_operator"),
        threshold: num(rule, "threshold"),
        metric: text("metric"),
        metrics: strs(rule, "metrics"),
        resource_type: text("resource_type"),
        resource_id: text("resource_id"),
        aggregation: text("aggregation_method"),
        granularity: i(rule, "granularity"),
        evaluation_periods: i(rule, "evaluation_periods"),
        event_type: text("event_type"),
        event_query: match alarm_rule(x) {
            AlarmRule::Event { query, .. } => query,
            _ => vec![],
        },
        alarm_type: kind,
    }
}

/// The alarm's `time_constraints` (when it may fire: cron `start`, `duration` in seconds).
fn time_constraints(x: &Value) -> Vec<AlarmTimeConstraint> {
    arr(x, "time_constraints")
        .iter()
        .map(|t| AlarmTimeConstraint {
            name: s(t, "name").unwrap_or_default(),
            start: s(t, "start").unwrap_or_default(),
            duration: i(t, "duration"),
            timezone: s_ne(t, "timezone"),
            description: s_ne(t, "description"),
        })
        .collect()
}

/// A number Aodh may send as JSON number or string (`threshold` is a float).
pub(super) fn num(v: &Value, k: &str) -> Option<f64> {
    match v.get(k)? {
        Value::Number(n) => n.as_f64(),
        Value::String(x) => x.parse().ok(),
        _ => None,
    }
}

/// The alarm's rule, from `<type>_rule`. Gnocchi's three threshold types share
/// one shape; types Moonlight doesn't know are kept as JSON.
fn alarm_rule(x: &Value) -> AlarmRule {
    let kind = s(x, "type").unwrap_or_default();
    let rule = x.get(format!("{kind}_rule")).unwrap_or(&Value::Null);
    match kind.as_str() {
        "gnocchi_resources_threshold"
        | "gnocchi_aggregation_by_metrics_threshold"
        | "gnocchi_aggregation_by_resources_threshold" => {
            let metrics = match s_ne(rule, "metric") {
                Some(m) => vec![m],
                None => strs(rule, "metrics"),
            };
            let resource_type = s_ne(rule, "resource_type");
            let resource_id = s_ne(rule, "resource_id");
            // Gnocchi's `instance` resources are Nova servers, with the server's ID.
            let resource_href = match (resource_type.as_deref(), resource_id.as_deref()) {
                (Some("instance"), Some(id)) => Some(format!("/project/instances/{id}")),
                _ => None,
            };
            AlarmRule::Threshold {
                metrics,
                resource_type,
                resource_id,
                resource_href,
                query: s_ne(rule, "query"),
                aggregation: s_ne(rule, "aggregation_method"),
                comparison: s_ne(rule, "comparison_operator"),
                threshold: num(rule, "threshold"),
                granularity: i(rule, "granularity"),
                evaluation_periods: i(rule, "evaluation_periods"),
            }
        }
        "prometheus" => AlarmRule::Prometheus {
            query: s(rule, "query").unwrap_or_default(),
            comparison: s_ne(rule, "comparison_operator"),
            threshold: num(rule, "threshold"),
        },
        "event" => AlarmRule::Event {
            event_type: s(rule, "event_type").unwrap_or_else(|| "*".into()),
            query: arr(rule, "query")
                .iter()
                .map(|q| {
                    let value = q
                        .get("value")
                        .map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))
                        .unwrap_or_default();
                    format!(
                        "{} {} {value}",
                        s(q, "field").unwrap_or_default(),
                        comparison_symbol(&s(q, "op").unwrap_or_else(|| "eq".into()))
                    )
                })
                .collect(),
        },
        _ if rule.is_null() => AlarmRule::Raw(String::new()),
        _ => AlarmRule::Raw(serde_json::to_string_pretty(rule).unwrap_or_default()),
    }
}

/// "`threshold` compared": ` > 80`, or nothing without an operator.
fn compared(comparison: &Option<String>, threshold: Option<f64>) -> String {
    match (comparison, threshold) {
        (Some(c), Some(t)) => format!(" {} {t}", comparison_symbol(c)),
        _ => String::new(),
    }
}

/// The list's one-line *Condition*: `mean(cpu) > 80 · instance <id>`, the
/// PromQL query and its threshold, or the event type.
fn condition(rule: &AlarmRule) -> String {
    match rule {
        AlarmRule::Threshold {
            metrics,
            resource_type,
            resource_id,
            aggregation,
            comparison,
            threshold,
            ..
        } => {
            let what = metrics.join(", ");
            let what = match aggregation {
                Some(a) => format!("{a}({what})"),
                None => what,
            };
            let on = match (resource_type, resource_id) {
                (Some(t), Some(id)) => format!(" · {t} {id}"),
                (Some(t), None) => format!(" · {t}"),
                _ => String::new(),
            };
            format!("{what}{}{on}", compared(comparison, *threshold))
        }
        AlarmRule::Prometheus {
            query,
            comparison,
            threshold,
        } => format!("{query}{}", compared(comparison, *threshold)),
        AlarmRule::Event { event_type, .. } => event_type.clone(),
        AlarmRule::Raw(_) => "-".into(),
    }
}

/// `GET v2/alarms/{id}` and its history → the alarm detail page.
pub fn alarm_detail(x: &Value, history: &[Value]) -> AlarmDetail {
    let rule = alarm_rule(x);
    let mut history: Vec<_> = history.iter().map(alarm_change).collect();
    history.sort_by(|a, b| b.time.cmp(&a.time));
    AlarmDetail {
        alarm: alarm_row(x, &rule),
        rule,
        alarm_actions: strs(x, "alarm_actions"),
        ok_actions: strs(x, "ok_actions"),
        insufficient_data_actions: strs(x, "insufficient_data_actions"),
        repeat_actions: b(x, "repeat_actions"),
        time_constraints: time_constraints(x),
        project_id: s_ne(x, "project_id"),
        user_id: s_ne(x, "user_id"),
        history,
    }
}

/// One history entry. Aodh's `detail` is JSON *text*: `{"state", "transition_reason"}`
/// for a state transition, the changed fields for a rule change, the whole
/// alarm for a creation (left out: the overview shows it).
fn alarm_change(h: &Value) -> AlarmChange {
    let kind = s(h, "type").unwrap_or_default();
    let detail = s(h, "detail").unwrap_or_default();
    let parsed = serde_json::from_str::<Value>(&detail).ok();
    let (state, detail) = match (kind.as_str(), &parsed) {
        ("state transition", Some(d)) => (s(d, "state"), s_ne(d, "transition_reason")),
        ("rule change", Some(d)) => (None, Some(d.to_string())),
        ("rule change", None) => (None, Some(detail).filter(|x| !x.is_empty())),
        _ => (None, None),
    };
    AlarmChange {
        time: s(h, "timestamp").unwrap_or_default(),
        kind,
        state,
        detail,
        user_id: s_ne(h, "user_id"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- Alarming ---

    #[test]
    fn alarms_map_every_rule_type() {
        let raw = vec![
            json!({"alarm_id": "a1", "name": "cpu-high", "type": "gnocchi_resources_threshold", "state": "alarm",
                   "severity": "critical", "enabled": true, "timestamp": "2026-10-28T10:00:00.123456",
                   "state_timestamp": "2026-10-28T11:00:00", "state_reason": "Transition to alarm due to 3 samples outside threshold",
                   "gnocchi_resources_threshold_rule": {"metric": "cpu", "resource_type": "instance", "resource_id": "srv1",
                       "aggregation_method": "mean", "comparison_operator": "gt", "threshold": 80.0, "granularity": 300, "evaluation_periods": 3}}),
            json!({"alarm_id": "a2", "name": "", "type": "prometheus", "state": "insufficient data", "severity": "",
                   "enabled": false, "prometheus_rule": {"query": "rate(cpu[5m])", "comparison_operator": "ge", "threshold": "0.5"}}),
            json!({"alarm_id": "a3", "name": "Deleted", "type": "event", "state": "ok",
                   "event_rule": {"event_type": "compute.instance.delete.end", "query": [{"field": "traits.instance_id", "op": "eq", "type": "string", "value": "srv1"}]}}),
            json!({"alarm_id": "a4", "name": "both", "type": "composite", "state": "ok",
                   "composite_rule": {"or": [{"type": "prometheus", "threshold": 1}]}}),
        ];
        let rows = alarms(&raw);
        let names: Vec<_> = rows.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(
            names,
            ["a2", "both", "cpu-high", "Deleted"],
            "by name, case-insensitive; empty name: the ID"
        );
        let cpu = &rows[2];
        assert_eq!(cpu.condition, "mean(cpu) > 80 · instance srv1");
        assert_eq!(
            (cpu.severity.as_deref(), cpu.enabled, cpu.state.as_str()),
            (Some("critical"), true, "alarm")
        );
        assert_eq!(
            rows[0].condition, "rate(cpu[5m]) ≥ 0.5",
            "a string threshold is read too"
        );
        assert_eq!(
            (rows[0].severity.clone(), rows[0].enabled),
            (None, false),
            "\"\" severity is unset"
        );
        assert_eq!(rows[3].condition, "compute.instance.delete.end");
        assert_eq!(rows[1].condition, "-", "composite: no one-line summary");
        assert!(
            alarms(&[json!({"alarm_id": "a5", "name": "x", "type": "prometheus", "state": "ok"})])[0].enabled,
            "missing `enabled`: Aodh's default"
        );
    }

    #[test]
    fn alarm_detail_parses_rule_actions_and_history() {
        let alarm = json!({"alarm_id": "a1", "name": "cpu-high", "type": "gnocchi_resources_threshold", "state": "alarm",
            "gnocchi_resources_threshold_rule": {"metric": "cpu", "resource_type": "instance", "resource_id": "srv1",
                "aggregation_method": "mean", "comparison_operator": "gt", "threshold": 80, "granularity": 300, "evaluation_periods": 3},
            "alarm_actions": ["https://hook/alarm"], "ok_actions": [], "insufficient_data_actions": ["log://"],
            "repeat_actions": false, "project_id": "p1", "user_id": "u1",
            "time_constraints": [{"name": "office", "start": "0 9 * * 1-5", "duration": 32400, "timezone": "Europe/Madrid", "description": ""}]});
        let history = vec![
            json!({"event_id": "e1", "type": "creation", "timestamp": "2026-10-28T09:00:00", "user_id": "u1", "detail": "{\"name\": \"cpu-high\"}"}),
            json!({"event_id": "e3", "type": "state transition", "timestamp": "2026-10-28T11:00:00", "user_id": null,
                   "detail": "{\"state\": \"alarm\", \"transition_reason\": \"3 datapoints are unknown\"}"}),
            json!({"event_id": "e2", "type": "rule change", "timestamp": "2026-10-28T10:00:00", "user_id": "u1", "detail": "{\"severity\": \"critical\"}"}),
        ];
        let d = alarm_detail(&alarm, &history);
        let AlarmRule::Threshold {
            resource_href,
            threshold,
            granularity,
            ..
        } = &d.rule
        else {
            panic!("threshold rule: {:?}", d.rule)
        };
        assert_eq!(
            resource_href.as_deref(),
            Some("/project/instances/srv1"),
            "Gnocchi `instance` resources are servers"
        );
        assert_eq!((*threshold, *granularity), (Some(80.0), Some(300)));
        assert_eq!(
            (
                d.alarm_actions.len(),
                d.ok_actions.len(),
                d.insufficient_data_actions.len()
            ),
            (1, 0, 1)
        );
        let tc = &d.time_constraints[0];
        assert_eq!(
            (
                tc.start.as_str(),
                tc.duration,
                tc.timezone.as_deref(),
                tc.description.clone()
            ),
            ("0 9 * * 1-5", Some(32400), Some("Europe/Madrid"), None)
        );
        let kinds: Vec<_> = d.history.iter().map(|h| h.kind.as_str()).collect();
        assert_eq!(kinds, ["state transition", "rule change", "creation"], "newest first");
        assert_eq!(
            (
                d.history[0].state.as_deref(),
                d.history[0].detail.as_deref(),
                d.history[0].user_id.clone()
            ),
            (Some("alarm"), Some("3 datapoints are unknown"), None)
        );
        assert_eq!(d.history[1].detail.as_deref(), Some(r#"{"severity":"critical"}"#));
        assert_eq!(
            d.history[2].detail, None,
            "a creation repeats the whole alarm: left out"
        );
        let spec = &d.alarm.spec;
        assert_eq!(
            (
                spec.alarm_type.as_str(),
                spec.metric.as_str(),
                spec.resource_id.as_str(),
                spec.threshold
            ),
            ("gnocchi_resources_threshold", "cpu", "srv1", Some(80.0))
        );
        assert_eq!(
            (spec.severity.as_str(), spec.time_constraints.len()),
            ("low", 1),
            "no severity: Aodh's default; constraints kept for the PUT"
        );
        let body = spec.body().unwrap();
        assert_eq!(
            body["gnocchi_resources_threshold_rule"]["granularity"],
            json!(300),
            "an unchanged alarm rebuilds its rule"
        );
        assert_eq!(body["time_constraints"][0]["timezone"], json!("Europe/Madrid"));
        let unknown = alarm_detail(
            &json!({"alarm_id": "a9", "name": "lb", "type": "loadbalancer_member_health", "loadbalancer_member_health_rule": {"pool_id": "p"}}),
            &[],
        );
        assert!(
            matches!(&unknown.rule, AlarmRule::Raw(j) if j.contains("\"pool_id\"")),
            "unknown types keep their JSON"
        );
    }
}
