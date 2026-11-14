//! Alarming (Aodh).

use super::*;

/// The selected alarm type, while creating.
fn alarm_type(v: &Vals) -> &str {
    v.get("type").map(String::as_str).unwrap_or("")
}

/// Types with a threshold and a comparison (`prometheus` and the Gnocchi ones; `event` and
/// `composite` have none).
fn alarm_compares(v: &Vals) -> bool {
    let t = alarm_type(v);
    t == "prometheus" || t.starts_with("gnocchi_")
}

/// Gnocchi types: they take an aggregation method, a granularity and evaluation periods.
fn alarm_gnocchi(v: &Vals) -> bool {
    alarm_type(v).starts_with("gnocchi_")
}

/// Types with one metric name and a resource type.
fn alarm_metric(v: &Vals) -> bool {
    matches!(
        alarm_type(v),
        "gnocchi_resources_threshold" | "gnocchi_aggregation_by_resources_threshold"
    )
}

/// The fields' types: `(field, types it belongs to, visibility while creating)`.
type AlarmFieldTypes = (&'static [&'static str], fn(&Vals) -> bool);

/// The threshold and comparison fields: see `alarm_compares`.
const COMPARES: AlarmFieldTypes = (
    &[
        "prometheus",
        "gnocchi_resources_threshold",
        "gnocchi_aggregation_by_metrics_threshold",
        "gnocchi_aggregation_by_resources_threshold",
    ],
    alarm_compares,
);

/// The aggregation, granularity and evaluation period fields: see `alarm_gnocchi`.
const GNOCCHI: AlarmFieldTypes = (
    &[
        "gnocchi_resources_threshold",
        "gnocchi_aggregation_by_metrics_threshold",
        "gnocchi_aggregation_by_resources_threshold",
    ],
    alarm_gnocchi,
);

/// The metric and resource type fields: see `alarm_metric`.
const METRIC: AlarmFieldTypes = (
    &[
        "gnocchi_resources_threshold",
        "gnocchi_aggregation_by_resources_threshold",
    ],
    alarm_metric,
);

/// *Create Alarm* (`spec` = `None`) and *Edit Alarm*. The fields follow the
/// type; on edit the type is fixed and only its fields are shown. Aodh's PUT
/// replaces the whole alarm, so an edit starts from the alarm's [`AlarmSpec`]
/// (its time constraints, which the form doesn't edit, go back unchanged).
pub fn alarm_dialog(lang: Lang, edit: Option<(&str, &AlarmSpec)>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let base = edit.map(|(_, s)| s.clone()).unwrap_or_else(|| AlarmSpec {
        alarm_type: "prometheus".into(),
        severity: "low".into(),
        enabled: true,
        comparison: "gt".into(),
        resource_type: "instance".into(),
        aggregation: "mean".into(),
        granularity: Some(300),
        evaluation_periods: Some(1),
        event_type: "*".into(),
        ..Default::default()
    });
    let fixed = edit.map(|(_, s)| s.alarm_type.clone());
    // While creating, a field shows for its types; on edit, it is there only for the alarm's type.
    let of = |(types, show): AlarmFieldTypes, f: Field| -> Option<Field> {
        match &fixed {
            Some(ty) => types.contains(&ty.as_str()).then_some(f),
            None => Some(f.show_if(show)),
        }
    };
    let only = |ty: &'static str, f: Field| -> Option<Field> {
        match &fixed {
            Some(t) => (t == ty).then_some(f),
            None => Some(f.when("type", ty)),
        }
    };
    let num = |n: Option<i64>| n.map(|n| n.to_string()).unwrap_or_default();
    let types = options(
        &[
            ("prometheus", K::AlarmTypePrometheus),
            ("gnocchi_resources_threshold", K::AlarmTypeGnocchiResource),
            ("gnocchi_aggregation_by_metrics_threshold", K::AlarmTypeGnocchiMetrics),
            (
                "gnocchi_aggregation_by_resources_threshold",
                K::AlarmTypeGnocchiResources,
            ),
            ("event", K::AlarmTypeEvent),
        ],
        lang,
    );
    let comparisons = Source::Static(
        ALARM_COMPARISONS
            .iter()
            .map(|c| (c.to_string(), format!("{} ({c})", comparison_symbol(c))))
            .collect(),
    );
    // An aggregation Gnocchi defines but the list doesn't have stays selectable.
    let mut aggregations: Vec<&str> = ALARM_AGGREGATIONS.to_vec();
    if !base.aggregation.is_empty() && !aggregations.contains(&base.aggregation.as_str()) {
        aggregations.push(&base.aggregation);
    }
    let lines = |v: &[String]| v.join("\n");
    let mut fields = vec![
        Field::text("name", tr(K::Name))
            .value(base.name.clone())
            .required()
            .help(tr(K::AlarmNameHelp)),
        Field::text("description", tr(K::Description))
            .value(base.description.clone())
            .help(tr(K::AlarmDescriptionHelp)),
    ];
    if fixed.is_none() {
        fields.push(
            Field::select("type", tr(K::Type), types)
                .value(base.alarm_type.clone())
                .required()
                .help(tr(K::AlarmTypeHelp))
                .tip(tr(K::TipAlarmType)),
        );
    }
    fields.extend(
        [
            only(
                "prometheus",
                Field::area("promql", tr(K::Query))
                    .value(base.query.clone())
                    .required()
                    .help(tr(K::PromqlHelp))
                    .tip(tr(K::TipPromql)),
            ),
            of(
                METRIC,
                Field::text("metric", tr(K::Metric))
                    .value(base.metric.clone())
                    .required()
                    .help(tr(K::AlarmMetricHelp))
                    .tip(tr(K::TipMetric)),
            ),
            only(
                "gnocchi_aggregation_by_metrics_threshold",
                Field::area("metrics", tr(K::MetricIds))
                    .value(lines(&base.metrics))
                    .required()
                    .help(tr(K::MetricIdsHelp))
                    .tip(tr(K::TipMetricIds)),
            ),
            of(
                METRIC,
                Field::text("resource_type", tr(K::ResourceType))
                    .value(base.resource_type.clone())
                    .required()
                    .help(tr(K::AlarmResourceTypeHelp))
                    .tip(tr(K::TipResourceType)),
            ),
            only(
                "gnocchi_resources_threshold",
                Field::text("resource_id", tr(K::ResourceId))
                    .value(base.resource_id.clone())
                    .required()
                    .help(tr(K::AlarmResourceIdHelp))
                    .tip(tr(K::TipResourceId)),
            ),
            only(
                "gnocchi_aggregation_by_resources_threshold",
                Field::area("search", tr(K::Query))
                    .value(base.query.clone())
                    .required()
                    .help(tr(K::GnocchiSearchHelp))
                    .tip(tr(K::TipGnocchiSearch)),
            ),
            of(
                GNOCCHI,
                Field::select("aggregation", tr(K::Aggregation), options_raw(&aggregations))
                    .value(base.aggregation.clone())
                    .required()
                    .help(tr(K::AggregationHelp))
                    .tip(tr(K::TipAggregation)),
            ),
            of(
                COMPARES,
                Field::select("comparison", tr(K::Comparison), comparisons)
                    .value(base.comparison.clone())
                    .required()
                    .help(tr(K::ComparisonHelp)),
            ),
            of(
                COMPARES,
                Field::text("threshold", tr(K::Threshold))
                    .value(base.threshold.map(|t| t.to_string()).unwrap_or_default())
                    .required()
                    .help(tr(K::ThresholdHelp))
                    .tip(tr(K::TipThreshold)),
            ),
            of(
                GNOCCHI,
                Field::number("granularity", tr(K::GranularitySeconds), 1)
                    .value(num(base.granularity))
                    .required()
                    .help(tr(K::GranularityHelp))
                    .tip(tr(K::TipGranularity)),
            ),
            of(
                GNOCCHI,
                Field::number("evaluation_periods", tr(K::EvaluationPeriods), 1)
                    .value(num(base.evaluation_periods))
                    .required()
                    .help(tr(K::EvaluationPeriodsHelp))
                    .tip(tr(K::TipEvaluationPeriods)),
            ),
            only(
                "event",
                Field::text("event_type", tr(K::EventType))
                    .value(base.event_type.clone())
                    .help(tr(K::EventTypeHelp))
                    .tip(tr(K::TipEventType)),
            ),
            only(
                "event",
                Field::area("event_query", tr(K::Query))
                    .value(lines(&base.event_query))
                    .help(tr(K::EventQueryHelp))
                    .tip(tr(K::TipEventQuery)),
            ),
        ]
        .into_iter()
        .flatten(),
    );
    let severities = options(
        &[
            ("low", K::SeverityLow),
            ("moderate", K::SeverityModerate),
            ("critical", K::SeverityCritical),
        ],
        lang,
    );
    fields.extend([
        Field::select("severity", tr(K::Severity), severities)
            .value(base.severity.clone())
            .required()
            .help(tr(K::SeverityHelp)),
        Field::check("enabled", tr(K::EnabledCol), base.enabled).help(tr(K::AlarmEnabledHelp)),
        Field::area("alarm_actions", tr(K::OnAlarm))
            .value(lines(&base.alarm_actions))
            .help(tr(K::AlarmActionsHelp))
            .tip(tr(K::TipAlarmActions)),
        Field::area("ok_actions", tr(K::OnOk))
            .value(lines(&base.ok_actions))
            .help(tr(K::OkActionsHelp)),
        Field::area("insufficient_data_actions", tr(K::OnInsufficientData))
            .value(lines(&base.insufficient_data_actions))
            .help(tr(K::InsufficientDataActionsHelp)),
        Field::check("repeat_actions", tr(K::RepeatActions), base.repeat_actions).help(tr(K::RepeatActionsHelp)),
    ]);
    let id = edit.map(|(id, _)| id.to_string());
    let title = tr(if id.is_some() { K::EditAlarm } else { K::CreateAlarm });
    let submit = if id.is_some() { tr(K::Save) } else { title.clone() };
    Dialog::form(title, submit, fields, move |v| {
        let alarm_type = fixed.clone().unwrap_or_else(|| v.get("type"));
        let bad = |e: String| invalid(lang, e);
        // Spanish users type decimal commas.
        let threshold = match v.opt("threshold") {
            None => None,
            Some(x) => Some(
                x.replace(',', ".")
                    .parse::<f64>()
                    .map_err(|_| bad(format!("{}: ?", t(lang, K::Threshold))))?,
            ),
        };
        let query = match alarm_type.as_str() {
            "prometheus" => v.get("promql"),
            "gnocchi_aggregation_by_resources_threshold" => v.get("search"),
            _ => String::new(),
        };
        let spec = AlarmSpec {
            name: req(v, "name", K::Name, lang)?,
            description: v.get("description"),
            severity: v.get("severity"),
            enabled: v.flag("enabled"),
            repeat_actions: v.flag("repeat_actions"),
            alarm_actions: v.lines("alarm_actions"),
            ok_actions: v.lines("ok_actions"),
            insufficient_data_actions: v.lines("insufficient_data_actions"),
            time_constraints: base.time_constraints.clone(),
            query,
            comparison: v.get("comparison"),
            threshold,
            metric: v.get("metric"),
            metrics: v.lines("metrics"),
            resource_type: v.get("resource_type"),
            resource_id: v.get("resource_id"),
            aggregation: v.get("aggregation"),
            granularity: v.num("granularity").map_err(bad)?,
            evaluation_periods: v.num("evaluation_periods").map_err(bad)?,
            event_type: v.get("event_type"),
            event_query: v.lines("event_query"),
            alarm_type,
        };
        spec.body().map_err(bad)?;
        Ok(match &id {
            Some(id) => OsAction::AlarmUpdate { id: id.clone(), spec },
            None => OsAction::AlarmCreate(spec),
        })
    })
    .sized(DialogSize::Medium)
}

/// An alarm's menu (list row and detail page): *Edit Alarm*.
pub fn alarm_items(lang: Lang, a: &AlarmRow) -> Vec<MenuItem> {
    let (id, spec) = (a.id.clone(), a.spec.clone());
    vec![MenuItem::dialog(t(lang, K::EditAlarm), move || {
        alarm_dialog(lang, Some((&id, &spec)))
    })]
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[test]
    fn alarm_dialog_follows_the_type_and_keeps_what_it_does_not_edit() {
        let create = alarm_dialog(Lang::EsEs, None);
        assert!(
            create.fields.iter().any(|f| f.name == "type"),
            "the type is chosen on create"
        );
        let a = (create.build)(&vals(&[
            ("name", "cpu"),
            ("type", "prometheus"),
            ("promql", "up"),
            ("comparison", "lt"),
            ("threshold", "0,5"),
            ("severity", "low"),
            ("enabled", "true"),
        ]))
        .unwrap();
        let OsAction::AlarmCreate(spec) = a else {
            panic!("{a:?}")
        };
        assert_eq!(
            (spec.query.as_str(), spec.threshold),
            ("up", Some(0.5)),
            "decimal comma"
        );
        assert!(
            (create.build)(&vals(&[
                ("name", "cpu"),
                ("type", "prometheus"),
                ("promql", "up"),
                ("comparison", "lt"),
                ("severity", "low")
            ]))
            .is_err(),
            "no threshold"
        );
        let existing = AlarmSpec {
            name: "deleted".into(),
            alarm_type: "event".into(),
            severity: "moderate".into(),
            event_type: "compute.instance.delete.end".into(),
            event_query: vec!["traits.instance_id = srv1".into()],
            time_constraints: vec![AlarmTimeConstraint {
                name: "office".into(),
                start: "0 9 * * *".into(),
                duration: Some(3600),
                timezone: None,
                description: None,
            }],
            ..Default::default()
        };
        let edit = alarm_dialog(Lang::EnUs, Some(("a1", &existing)));
        let names: Vec<_> = edit.fields.iter().map(|f| f.name).collect();
        assert!(
            !names.contains(&"type") && !names.contains(&"threshold") && names.contains(&"event_query"),
            "only the fixed type's fields: {names:?}"
        );
        let event_query = edit.fields.iter().find(|f| f.name == "event_query").unwrap();
        assert_eq!(event_query.default, "traits.instance_id = srv1");
        let a = (edit.build)(&vals(&[
            ("name", "deleted"),
            ("event_type", "compute.instance.delete.end"),
            ("event_query", "traits.instance_id = srv1"),
            ("severity", "critical"),
        ]))
        .unwrap();
        let OsAction::AlarmUpdate { id, spec } = a else {
            panic!("{a:?}")
        };
        assert_eq!(
            (id.as_str(), spec.alarm_type.as_str(), spec.severity.as_str()),
            ("a1", "event", "critical")
        );
        assert_eq!(
            spec.time_constraints, existing.time_constraints,
            "Aodh's PUT replaces the alarm: constraints go back"
        );
        for d in [&create, &edit] {
            let bare: Vec<_> = d.fields.iter().filter(|f| f.help.is_none()).map(|f| f.name).collect();
            assert!(bare.is_empty(), "every alarm field has a help text: {bare:?}");
        }
        let tipped = |d: &Dialog| {
            d.fields
                .iter()
                .filter(|f| f.tip.is_some())
                .map(|f| f.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            tipped(&create),
            [
                "type",
                "promql",
                "metric",
                "metrics",
                "resource_type",
                "resource_id",
                "search",
                "aggregation",
                "threshold",
                "granularity",
                "evaluation_periods",
                "event_type",
                "event_query",
                "alarm_actions"
            ],
            "the complex fields explain themselves behind an (i)"
        );
        // Markup that doesn't pair up would show as raw `*` or backticks.
        use crate::components::widgets::{TipBlock, TipInline, tip_blocks, tip_inline};
        fn raw(parts: &[TipInline]) -> bool {
            parts.iter().any(|p| match p {
                TipInline::Text(t) => t.contains('`') || t.contains('*'),
                TipInline::Bold(i) | TipInline::Italic(i) => raw(i),
                TipInline::Code(_) => false,
            })
        }
        for lang in Lang::ALL {
            for f in alarm_dialog(lang, None).fields.iter().filter_map(|f| f.tip.as_deref()) {
                for b in tip_blocks(f) {
                    let lines = match b {
                        TipBlock::Para(p) => vec![p],
                        TipBlock::List(items) => items,
                    };
                    for l in lines {
                        assert!(!raw(&tip_inline(&l)), "{lang:?}: unpaired markup in {l:?}");
                    }
                }
            }
        }
    }
}
