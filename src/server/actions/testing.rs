//! Helpers shared by the action tests.

use super::*;

/// Dry-runs an action and returns the requests it would send.
pub(super) async fn dry(action: OsAction) -> Vec<RequestPreview> {
    // Every valid action of every test also passes `run_action`'s ID check (roadmap 48).
    action
        .check_ids()
        .unwrap_or_else(|e| panic!("{action:?}: IDs refused: {e}"));
    assert_paths_checked(&action).await;
    let mut x = Exec::new(WriteMode::DryRun, None);
    apply(&mut x, &action)
        .await
        .unwrap_or_else(|e| panic!("{action:?}: {e}"));
    x.outcome().requests
}

/// Roadmap 67: `check_ids` picks the fields to check by name (`id`, `*_id`…),
/// so a path parameter named otherwise would skip it. Each string of the
/// action is set in turn to a value with a `/`; the action must then be
/// refused (by `check_ids` or by `apply`) or keep that value out of every
/// request path. Run by `dry`, so on every action the tests build.
pub(super) async fn assert_paths_checked(action: &OsAction) {
    // `/` is what changes a path; dot segments would be normalised away
    // by the preview's URL and hide the marker.
    const MARKER: &str = "zz/zz";
    fn leaves(v: &Value, at: String, out: &mut Vec<String>) {
        match v {
            Value::String(_) => out.push(at),
            Value::Array(a) => a
                .iter()
                .enumerate()
                .for_each(|(i, x)| leaves(x, format!("{at}/{i}"), out)),
            Value::Object(m) => m.iter().for_each(|(k, x)| leaves(x, format!("{at}/{k}"), out)),
            _ => {}
        }
    }
    let v = serde_json::to_value(action).unwrap();
    let mut at = Vec::new();
    leaves(&v, String::new(), &mut at);
    for field in at {
        let mut marked = v.clone();
        *marked.pointer_mut(&field).unwrap() = Value::String(MARKER.into());
        // Strings that are really enums (`kind`, `source`) don't deserialise.
        let Ok(a) = serde_json::from_value::<OsAction>(marked) else {
            continue;
        };
        if a.check_ids().is_err() {
            continue;
        }
        let mut x = Exec::new(WriteMode::DryRun, None);
        if apply(&mut x, &a).await.is_err() {
            continue;
        }
        for r in x.outcome().requests {
            assert!(
                !r.path.contains(MARKER),
                "{}: `{field}` reaches the request path `{}` unchecked (roadmap 67)",
                a.kind(),
                r.path
            );
        }
    }
}

pub(super) fn body(r: &RequestPreview) -> Value {
    serde_json::from_str(r.body.as_deref().expect("request has a body")).unwrap()
}

pub(super) fn id(s: &str) -> String {
    s.to_string()
}

pub(super) fn calls(r: &[RequestPreview]) -> Vec<(String, String)> {
    r.iter().map(|q| (q.method.clone(), q.path.clone())).collect()
}

pub(super) fn deletes(ids: &[&str]) -> OsAction {
    OsAction::Batch(
        ids.iter()
            .map(|i| (format!("vol {i}"), OsAction::VolumeDelete { id: id(i) }))
            .collect(),
    )
}

pub(super) fn spec() -> StackSpec {
    StackSpec {
        name: id("web"),
        source: TemplateSource {
            template: "heat_template_version: 2018-08-31".into(),
            url: None,
            environment: Some("parameters: {}".into()),
        },
        parameters: vec![
            ("flavor".into(), "m1.small".into()),
            ("db_password".into(), "s3cret".into()),
        ],
        hidden: vec!["db_password".into()],
        timeout_mins: Some(30),
        disable_rollback: true,
    }
}

pub(super) async fn dry_err(action: OsAction) -> bool {
    let mut x = Exec::new(WriteMode::DryRun, None);
    apply(&mut x, &action).await.is_err()
}
