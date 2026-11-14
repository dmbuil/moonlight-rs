//! Aodh writes: alarms.

use super::*;

/// `AlarmCreate`: `POST /v2/alarms`. The body is built and checked by `AlarmSpec::body`,
/// as in the dialog.
pub(super) async fn alarm_create(x: &mut Exec<'_>, spec: &AlarmSpec) -> R<()> {
    let body = spec.body().map_err(AppError::invalid)?;
    x.create(JsonRequest::alarming(http::Method::POST, "alarms", Some(body)), "alarm")
        .await?;
    Ok(())
}

/// `AlarmUpdate`: `PUT /v2/alarms/{id}` with the whole alarm (Aodh replaces it).
pub(super) async fn alarm_update(x: &mut Exec<'_>, id: &str, spec: &AlarmSpec) -> R<()> {
    let body = spec.body().map_err(AppError::invalid)?;
    x.send(JsonRequest::alarming(
        http::Method::PUT,
        format!("alarms/{}", non_empty(id, "alarm")?),
        Some(body),
    ))
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    fn alarm_spec() -> AlarmSpec {
        AlarmSpec {
            name: id("cpu-high"),
            alarm_type: id("prometheus"),
            severity: id("critical"),
            enabled: true,
            alarm_actions: vec![id("https://hooks.example/alarm"), id("log://")],
            query: id("rate(cpu[5m])"),
            comparison: id("gt"),
            threshold: Some(0.8),
            // Ignored: not a field of prometheus alarms.
            metric: id("cpu"),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn alarm_create_and_update_send_the_whole_alarm() {
        let r = dry(OsAction::AlarmCreate(alarm_spec())).await;
        assert_eq!(
            (r[0].service.as_str(), r[0].method.as_str(), r[0].path.as_str()),
            ("alarming", "POST", "/v2/alarms")
        );
        assert_eq!(
            body(&r[0]),
            json!({"name": "cpu-high", "description": "", "type": "prometheus", "severity": "critical", "enabled": true,
                   "repeat_actions": false, "alarm_actions": ["https://hooks.example/alarm", "log://"], "ok_actions": [],
                   "insufficient_data_actions": [], "time_constraints": [],
                   "prometheus_rule": {"query": "rate(cpu[5m])", "comparison_operator": "gt", "threshold": 0.8}})
        );
        assert_eq!(OsAction::AlarmCreate(alarm_spec()).plugins(), [Plugin::Alarming]);
        let gnocchi = AlarmSpec {
            alarm_type: id("gnocchi_resources_threshold"),
            resource_type: id("instance"),
            resource_id: id("srv1"),
            aggregation: id("mean"),
            granularity: Some(300),
            evaluation_periods: Some(3),
            time_constraints: vec![AlarmTimeConstraint {
                name: id("office"),
                start: id("0 9 * * 1-5"),
                duration: Some(32400),
                timezone: None,
                description: None,
            }],
            ..alarm_spec()
        };
        let r = dry(OsAction::AlarmUpdate {
            id: id("a1"),
            spec: gnocchi,
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/v2/alarms/a1"));
        let b = body(&r[0]);
        assert_eq!(
            b["gnocchi_resources_threshold_rule"],
            json!({"metric": "cpu", "resource_type": "instance", "resource_id": "srv1", "aggregation_method": "mean",
                   "comparison_operator": "gt", "threshold": 0.8, "granularity": 300, "evaluation_periods": 3})
        );
        assert_eq!(
            b["time_constraints"],
            json!([{"name": "office", "start": "0 9 * * 1-5", "duration": 32400}]),
            "kept: the PUT replaces the alarm"
        );
        assert!(b.get("prometheus_rule").is_none());
        let by_metrics = AlarmSpec {
            alarm_type: id("gnocchi_aggregation_by_metrics_threshold"),
            metrics: vec![id("m1"), id(" m2 ")],
            aggregation: id("mean"),
            granularity: Some(60),
            evaluation_periods: Some(1),
            ..alarm_spec()
        };
        assert_eq!(
            body(&dry(OsAction::AlarmCreate(by_metrics)).await[0])["gnocchi_aggregation_by_metrics_threshold_rule"]["metrics"],
            json!(["m1", "m2"])
        );
        let by_resources = AlarmSpec {
            alarm_type: id("gnocchi_aggregation_by_resources_threshold"),
            resource_type: id("instance"),
            query: id(r#"{"=": {"server_group": "web"}}"#),
            aggregation: id("mean"),
            granularity: Some(60),
            evaluation_periods: Some(1),
            ..alarm_spec()
        };
        assert_eq!(
            body(&dry(OsAction::AlarmCreate(by_resources)).await[0])["gnocchi_aggregation_by_resources_threshold_rule"]
                ["query"],
            json!(r#"{"=": {"server_group": "web"}}"#),
            "Aodh stores the search as text"
        );
        let event = AlarmSpec {
            alarm_type: id("event"),
            event_type: id("compute.instance.delete.end"),
            event_query: vec![id("traits.instance_id = srv1"), id("traits.state ≠ error")],
            ..alarm_spec()
        };
        assert_eq!(
            body(&dry(OsAction::AlarmCreate(event)).await[0])["event_rule"],
            json!({"event_type": "compute.instance.delete.end", "query": [{"field": "traits.instance_id", "op": "eq", "value": "srv1"}, {"field": "traits.state", "op": "ne", "value": "error"}]}),
            "filters as the detail page shows them"
        );
    }

    #[tokio::test]
    async fn alarm_specs_are_checked() {
        let bad = |f: fn(&mut AlarmSpec)| {
            let mut s = alarm_spec();
            f(&mut s);
            OsAction::AlarmCreate(s)
        };
        assert!(dry_err(bad(|s| s.name = id(" "))).await, "name");
        assert!(
            dry_err(bad(|s| s.alarm_type = id("composite"))).await,
            "type the form doesn't offer"
        );
        assert!(dry_err(bad(|s| s.severity = id("urgent"))).await, "severity");
        assert!(dry_err(bad(|s| s.threshold = None)).await, "threshold");
        assert!(dry_err(bad(|s| s.comparison = id(">"))).await, "operator by name");
        assert!(dry_err(bad(|s| s.query = id(""))).await, "PromQL");
        assert!(
            dry_err(bad(|s| s.ok_actions = vec![id("ftp://x")])).await,
            "action scheme"
        );
        assert!(
            dry_err(bad(|s| s.alarm_actions = vec![id("https://")])).await,
            "a scheme alone"
        );
        assert!(
            dry_err(bad(|s| {
                s.alarm_type = id("gnocchi_aggregation_by_resources_threshold");
                s.resource_type = id("instance");
                s.query = id("{not json");
                s.aggregation = id("mean");
                s.granularity = Some(60);
                s.evaluation_periods = Some(1);
            }))
            .await,
            "the Gnocchi search must be JSON"
        );
        assert!(
            dry_err(bad(|s| {
                s.alarm_type = id("gnocchi_resources_threshold");
                s.resource_type = id("instance");
                s.resource_id = id("srv1");
                s.aggregation = id("mean");
                s.granularity = Some(0);
                s.evaluation_periods = Some(1);
            }))
            .await,
            "granularity 0"
        );
        assert!(
            dry_err(bad(|s| {
                s.alarm_type = id("event");
                s.event_query = vec![id("traits.x")];
            }))
            .await,
            "filter without operator"
        );
        assert!(
            dry_err(OsAction::AlarmUpdate {
                id: id(""),
                spec: alarm_spec()
            })
            .await,
            "no ID"
        );
    }
}
