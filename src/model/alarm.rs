//! Aodh alarms (roadmap 79).

use super::*;

/// An Aodh alarm, as the *Alarms* list shows it. Aodh's times have no zone; they are UTC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlarmRow {
    /// `alarm_id`.
    pub id: String,
    /// The name (Aodh requires one), else the ID.
    pub name: String,
    pub description: Option<String>,
    /// Aodh's `type`: `prometheus`, `gnocchi_resources_threshold`, `event`, `composite`…
    pub alarm_type: String,
    /// `ok`, `alarm` or `insufficient data`.
    pub state: String,
    pub state_reason: Option<String>,
    /// When the state last changed.
    pub state_since: Option<String>,
    /// `low`, `moderate` or `critical`; `None` when unset.
    pub severity: Option<String>,
    pub enabled: bool,
    /// What the alarm watches, one line and language-neutral
    /// (`mean(cpu) > 80`, the PromQL query, the event type); `-` when unknown.
    pub condition: String,
    /// When the alarm last changed (Aodh's `timestamp`).
    pub updated: Option<String>,
    /// The alarm as *Edit Alarm* starts from (the list's GET returns whole alarms).
    pub spec: AlarmSpec,
}

/// An alarm's rule, per type. Aodh keeps it under `<type>_rule`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AlarmRule {
    /// Gnocchi threshold alarms: `gnocchi_resources_threshold` (one resource),
    /// `gnocchi_aggregation_by_metrics_threshold` (`metrics` IDs) and
    /// `gnocchi_aggregation_by_resources_threshold` (resources matching `query`).
    Threshold {
        metrics: Vec<String>,
        resource_type: Option<String>,
        resource_id: Option<String>,
        /// The resource's page in Moonlight, when it has one (an instance).
        resource_href: Option<String>,
        /// The Gnocchi search of `…_by_resources_…`, as Aodh stores it (JSON text).
        query: Option<String>,
        aggregation: Option<String>,
        /// Aodh's operator (`gt`, `le`…).
        comparison: Option<String>,
        threshold: Option<f64>,
        /// Seconds.
        granularity: Option<i64>,
        evaluation_periods: Option<i64>,
    },
    /// `prometheus`: a PromQL query compared with a threshold (RHOSO's telemetry).
    Prometheus {
        query: String,
        comparison: Option<String>,
        threshold: Option<f64>,
    },
    /// `event`: fires on a notification of `event_type` matching `query`.
    Event { event_type: String, query: Vec<String> },
    /// Any other type (`composite`, `loadbalancer_member_health`…): the rule as
    /// Aodh returns it, pretty-printed JSON; empty when the alarm has none.
    Raw(String),
}

/// A time window in which the alarm is evaluated (`time_constraints`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlarmTimeConstraint {
    pub name: String,
    /// Cron expression of the window's start.
    pub start: String,
    /// Seconds.
    pub duration: Option<i64>,
    pub timezone: Option<String>,
    pub description: Option<String>,
}

/// One entry of an alarm's history (`GET v2/alarms/{id}/history`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlarmChange {
    pub time: String,
    /// `creation`, `rule change`, `state transition` or `deletion`.
    pub kind: String,
    /// The new state, for a state transition.
    pub state: Option<String>,
    /// A state transition's reason, or a rule change's new fields (compact JSON).
    pub detail: Option<String>,
    /// Who made the change; `None` for the evaluator's state transitions.
    pub user_id: Option<String>,
}

/// The alarm detail page: *Overview* and *History*.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlarmDetail {
    pub alarm: AlarmRow,
    pub rule: AlarmRule,
    /// URLs called on entering `alarm`, `ok` and `insufficient data`.
    pub alarm_actions: Vec<String>,
    pub ok_actions: Vec<String>,
    pub insufficient_data_actions: Vec<String>,
    /// Call the actions on every evaluation while in the state, not only on entering it.
    pub repeat_actions: bool,
    pub time_constraints: Vec<AlarmTimeConstraint>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    /// Newest first.
    pub history: Vec<AlarmChange>,
}

/// Aodh's comparison operator as a symbol (`gt` → `>`); unknown ones as they are.
pub fn comparison_symbol(op: &str) -> &str {
    match op {
        "lt" => "<",
        "le" => "≤",
        "eq" => "=",
        "ne" => "≠",
        "ge" => "≥",
        "gt" => ">",
        other => other,
    }
}

/// Alarm types the *Create Alarm* form offers (roadmap 79); the list and detail show every type.
pub const ALARM_TYPES: [&str; 5] = [
    "prometheus",
    "gnocchi_resources_threshold",
    "gnocchi_aggregation_by_metrics_threshold",
    "gnocchi_aggregation_by_resources_threshold",
    "event",
];
/// Aodh's severities.
pub const ALARM_SEVERITIES: [&str; 3] = ["low", "moderate", "critical"];
/// Aodh's comparison operators.
pub const ALARM_COMPARISONS: [&str; 6] = ["gt", "ge", "eq", "ne", "le", "lt"];
/// Gnocchi's usual aggregation methods (`rate:` variants included); other
/// values are accepted too, since archive policies can define more.
pub const ALARM_AGGREGATIONS: [&str; 11] = [
    "mean",
    "max",
    "min",
    "sum",
    "last",
    "first",
    "median",
    "std",
    "count",
    "rate:mean",
    "rate:last",
];
/// Notifier schemes Aodh can call (`test://` left out: it only exists for Aodh's own tests).
pub const ALARM_ACTION_SCHEMES: [&str; 7] = [
    "http://",
    "https://",
    "trust+http://",
    "trust+https://",
    "log://",
    "zaqar://",
    "trust+zaqar://",
];

/// An alarm to create (`POST v2/alarms`) or replace (`PUT v2/alarms/{id}`):
/// Aodh's PUT replaces the whole alarm, so an edit sends every field, including
/// the time constraints the form doesn't edit. [`AlarmSpec::body`] keeps only
/// the rule fields of `alarm_type` and checks them, in the dialog and on the server.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AlarmSpec {
    pub name: String,
    pub description: String,
    /// One of [`ALARM_TYPES`].
    pub alarm_type: String,
    /// One of [`ALARM_SEVERITIES`].
    pub severity: String,
    pub enabled: bool,
    pub repeat_actions: bool,
    /// URLs, each with one of [`ALARM_ACTION_SCHEMES`].
    pub alarm_actions: Vec<String>,
    pub ok_actions: Vec<String>,
    pub insufficient_data_actions: Vec<String>,
    /// Kept as they are (not edited by the form).
    pub time_constraints: Vec<AlarmTimeConstraint>,
    /// `prometheus`: the PromQL query; `…_by_resources_threshold`: the Gnocchi
    /// search, JSON text (Aodh stores it as a string).
    pub query: String,
    /// One of [`ALARM_COMPARISONS`] (threshold types).
    pub comparison: String,
    pub threshold: Option<f64>,
    /// Gnocchi: the metric name (`…_resources_…`, `…_by_resources_…`).
    pub metric: String,
    /// `…_by_metrics_threshold`: Gnocchi metric IDs.
    pub metrics: Vec<String>,
    pub resource_type: String,
    /// `gnocchi_resources_threshold`: the Gnocchi resource (for `instance`, the server ID).
    pub resource_id: String,
    pub aggregation: String,
    /// Seconds.
    pub granularity: Option<i64>,
    pub evaluation_periods: Option<i64>,
    /// `event`: the notification type (`*` = any).
    pub event_type: String,
    /// `event`: trait filters, one `field op value` each (`traits.instance_id = abc`).
    pub event_query: Vec<String>,
}

impl AlarmSpec {
    /// Aodh's alarm body, or what is wrong with the spec.
    pub fn body(&self) -> Result<serde_json::Value, String> {
        use serde_json::{Map, Value, json};
        let req = |v: &str, what: &str| {
            if v.trim().is_empty() {
                Err(format!("{what} is required"))
            } else {
                Ok(v.trim().to_string())
            }
        };
        let name = req(&self.name, "name")?;
        one_of(&self.alarm_type, &ALARM_TYPES, "alarm type")?;
        one_of(&self.severity, &ALARM_SEVERITIES, "severity")?;
        for url in self
            .alarm_actions
            .iter()
            .chain(&self.ok_actions)
            .chain(&self.insufficient_data_actions)
        {
            if !ALARM_ACTION_SCHEMES
                .iter()
                .any(|s| url.starts_with(s) && url.len() > s.len() || *s == "log://" && url == "log://")
            {
                return Err(format!(
                    "action {url} must start with one of {}",
                    ALARM_ACTION_SCHEMES.join(", ")
                ));
            }
        }
        let mut rule = Map::new();
        let mut put = |k: &str, v: Value| {
            rule.insert(k.into(), v);
        };
        let gnocchi = self.alarm_type.starts_with("gnocchi_");
        if gnocchi || self.alarm_type == "prometheus" {
            one_of(&self.comparison, &ALARM_COMPARISONS, "comparison")?;
            let t = self
                .threshold
                .filter(|t| t.is_finite())
                .ok_or("threshold is required")?;
            put("comparison_operator", json!(self.comparison));
            put("threshold", json!(t));
        }
        if gnocchi {
            put("aggregation_method", json!(req(&self.aggregation, "aggregation")?));
            put(
                "granularity",
                json!(
                    self.granularity
                        .filter(|g| *g > 0)
                        .ok_or("granularity must be above 0")?
                ),
            );
            put(
                "evaluation_periods",
                json!(
                    self.evaluation_periods
                        .filter(|n| *n > 0)
                        .ok_or("evaluation periods must be above 0")?
                ),
            );
        }
        match self.alarm_type.as_str() {
            "prometheus" => put("query", json!(req(&self.query, "query")?)),
            "gnocchi_resources_threshold" => {
                put("metric", json!(req(&self.metric, "metric")?));
                put("resource_type", json!(req(&self.resource_type, "resource type")?));
                put("resource_id", json!(req(&self.resource_id, "resource ID")?));
            }
            "gnocchi_aggregation_by_metrics_threshold" => {
                let metrics: Vec<&str> = self
                    .metrics
                    .iter()
                    .map(|m| m.trim())
                    .filter(|m| !m.is_empty())
                    .collect();
                if metrics.is_empty() {
                    return Err("at least one metric ID is required".into());
                }
                put("metrics", json!(metrics));
            }
            "gnocchi_aggregation_by_resources_threshold" => {
                put("metric", json!(req(&self.metric, "metric")?));
                put("resource_type", json!(req(&self.resource_type, "resource type")?));
                let q = req(&self.query, "query")?;
                serde_json::from_str::<Value>(&q).map_err(|e| format!("query is not valid JSON: {e}"))?;
                put("query", json!(q));
            }
            _ => {
                let event_type = Some(self.event_type.trim()).filter(|e| !e.is_empty()).unwrap_or("*");
                let query = self
                    .event_query
                    .iter()
                    .map(|q| event_filter(q))
                    .collect::<Result<Vec<_>, _>>()?;
                put("event_type", json!(event_type));
                put("query", json!(query));
            }
        }
        let constraints: Vec<Value> = self
            .time_constraints
            .iter()
            .map(|c| {
                let mut m = Map::new();
                m.insert("name".into(), json!(c.name));
                m.insert("start".into(), json!(c.start));
                for (k, v) in [
                    ("duration", c.duration.map(|d| json!(d))),
                    ("timezone", c.timezone.as_ref().map(|z| json!(z))),
                    ("description", c.description.as_ref().map(|d| json!(d))),
                ] {
                    if let Some(v) = v {
                        m.insert(k.into(), v);
                    }
                }
                Value::Object(m)
            })
            .collect();
        let mut body = json!({
            "name": name,
            "description": self.description.trim(),
            "type": self.alarm_type,
            "severity": self.severity,
            "enabled": self.enabled,
            "repeat_actions": self.repeat_actions,
            "alarm_actions": self.alarm_actions,
            "ok_actions": self.ok_actions,
            "insufficient_data_actions": self.insufficient_data_actions,
            "time_constraints": constraints,
        });
        body[format!("{}_rule", self.alarm_type)] = Value::Object(rule);
        Ok(body)
    }
}

/// One event trait filter, `field op value` with `op` as a symbol (`=`, `!=`,
/// `≠`, `<`, `<=`, `≤`, `>`, `>=`, `≥`) or Aodh's name (`eq`…), as the detail
/// page shows them → Aodh's `{field, op, value}`.
fn event_filter(line: &str) -> Result<serde_json::Value, String> {
    let mut parts = line.trim().splitn(3, char::is_whitespace).filter(|p| !p.is_empty());
    let (Some(field), Some(op), Some(value)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(format!("filter {line:?} must be `field op value`"));
    };
    let op = match op {
        "=" | "==" | "eq" => "eq",
        "!=" | "≠" | "ne" => "ne",
        "<" | "lt" => "lt",
        "<=" | "≤" | "le" => "le",
        ">" | "gt" => "gt",
        ">=" | "≥" | "ge" => "ge",
        other => return Err(format!("unknown operator {other} in filter {line:?}")),
    };
    Ok(serde_json::json!({"field": field, "op": op, "value": value.trim()}))
}

// --- Key Manager (Barbican, roadmap 55) ----------------------------------------
