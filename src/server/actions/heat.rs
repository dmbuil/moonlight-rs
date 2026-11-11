//! Heat writes (roadmap 62): stacks.

use super::*;

/// `POST /v1/secrets`. Barbican refuses a content type without a payload, so
/// both go together; `application/octet-stream` payloads are base64
/// (`payload_content_encoding`), checked here so a typo fails before sending.
/// `stacks/{name}/{id}`: Heat needs both (`stacks/{id}` alone is a redirect).
fn stack_path(name: &str, id: &str) -> R<String> {
    Ok(format!(
        "stacks/{}/{}",
        non_empty(name, "stack name")?,
        non_empty(id, "stack")?
    ))
}

/// The template part of a Heat body (`template` or `template_url`, `environment`),
/// shared with `validate` and `preview` (`server/heat.rs`).
pub fn template_body(src: &TemplateSource) -> R<serde_json::Map<String, Value>> {
    let mut m = serde_json::Map::new();
    match src.url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) if u.starts_with("https://") || u.starts_with("http://") => {
            m.insert("template_url".into(), json!(u));
        }
        Some(_) => {
            return Err(AppError::invalid(
                "the template URL must start with http:// or https://",
            ));
        }
        None if src.template.trim().is_empty() => return Err(AppError::invalid("a template is required")),
        // Heat accepts the text as a string and parses YAML or JSON itself.
        None => {
            m.insert("template".into(), json!(src.template));
        }
    }
    let env = src.environment.as_deref().filter(|e| !e.trim().is_empty());
    if src.template.len() + env.map_or(0, str::len) > MAX_TEMPLATE_BYTES {
        return Err(AppError::invalid(format!(
            "the template and environment exceed {} KiB",
            MAX_TEMPLATE_BYTES / 1024
        )));
    }
    if let Some(e) = env {
        m.insert("environment".into(), json!(e));
    }
    Ok(m)
}

/// `POST stacks` (`create`, with `stack_name`) or `PATCH stacks/{name}/{id}`.
pub fn stack_body(spec: &StackSpec, create: bool) -> R<Value> {
    if !valid_stack_name(&spec.name) {
        return Err(AppError::invalid(
            "a stack name starts with a letter and has only letters, digits, _, - and .",
        ));
    }
    let mut m = template_body(&spec.source)?;
    if create {
        m.insert("stack_name".into(), json!(spec.name));
    }
    let params: serde_json::Map<String, Value> = spec.parameters.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
    m.insert("parameters".into(), Value::Object(params));
    if let Some(t) = spec.timeout_mins {
        if t == 0 {
            return Err(AppError::invalid("the timeout must be at least one minute"));
        }
        m.insert("timeout_mins".into(), json!(t));
    }
    m.insert("disable_rollback".into(), json!(spec.disable_rollback));
    Ok(Value::Object(m))
}

/// A stack action (`check`, `suspend`, `resume`): `POST …/actions` with `{action: null}`.
fn stack_action(name: &str, id: &str, action: &str) -> R<JsonRequest> {
    let body = serde_json::json!({ action: null });
    Ok(JsonRequest::orchestration(
        http::Method::POST,
        format!("{}/actions", stack_path(name, id)?),
        Some(body),
    ))
}

pub(super) async fn stack_check(x: &mut Exec<'_>, name: &str, id: &str) -> R<()> {
    x.send(stack_action(name, id, "check")?).await?;
    Ok(())
}

pub(super) async fn stack_suspend(x: &mut Exec<'_>, name: &str, id: &str) -> R<()> {
    x.send(stack_action(name, id, "suspend")?).await?;
    Ok(())
}

pub(super) async fn stack_resume(x: &mut Exec<'_>, name: &str, id: &str) -> R<()> {
    x.send(stack_action(name, id, "resume")?).await?;
    Ok(())
}

pub(super) async fn stack_delete(x: &mut Exec<'_>, name: &str, id: &str) -> R<()> {
    x.send(JsonRequest::orchestration(
        http::Method::DELETE,
        stack_path(name, id)?,
        None,
    ))
    .await?;
    Ok(())
}

pub(super) async fn stack_create(x: &mut Exec<'_>, spec: &StackSpec) -> R<()> {
    x.create(
        JsonRequest::orchestration(http::Method::POST, "stacks", Some(stack_body(spec, true)?)),
        "stack",
    )
    .await?;
    x.hide_params(&spec.hidden);
    Ok(())
}

pub(super) async fn stack_update(x: &mut Exec<'_>, id: &str, spec: &StackSpec) -> R<()> {
    x.send(JsonRequest::orchestration(
        http::Method::PATCH,
        stack_path(&spec.name, id)?,
        Some(stack_body(spec, false)?),
    ))
    .await?;
    x.hide_params(&spec.hidden);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[tokio::test]
    async fn stack_actions_and_delete() {
        let (name, sid) = (id("web"), id("s1"));
        for (action, verb) in [
            (
                OsAction::StackCheck {
                    name: name.clone(),
                    id: sid.clone(),
                },
                "check",
            ),
            (
                OsAction::StackSuspend {
                    name: name.clone(),
                    id: sid.clone(),
                },
                "suspend",
            ),
            (
                OsAction::StackResume {
                    name: name.clone(),
                    id: sid.clone(),
                },
                "resume",
            ),
        ] {
            let r = dry(action.clone()).await;
            assert_eq!((r[0].service.as_str(), r[0].method.as_str()), ("orchestration", "POST"));
            assert!(r[0].path.ends_with("/stacks/web/s1/actions"), "{}", r[0].path);
            assert_eq!(body(&r[0]), json!({ verb: null }));
            assert_eq!(action.plugins(), [Plugin::Orchestration]);
        }
        let r = dry(OsAction::StackDelete {
            name: name.clone(),
            id: sid.clone(),
        })
        .await;
        assert_eq!(r[0].method.as_str(), "DELETE");
        assert!(r[0].path.ends_with("/stacks/web/s1"), "{}", r[0].path);
        assert!(
            OsAction::StackDelete {
                name: id("a/b"),
                id: sid.clone()
            }
            .check_ids()
            .is_err(),
            "the name is a path segment"
        );
        assert!(
            dry_err(OsAction::StackDelete {
                name: String::new(),
                id: sid
            })
            .await,
            "a malformed selection key"
        );
    }

    #[tokio::test]
    async fn stack_create_and_update_bodies() {
        let r = dry(OsAction::StackCreate(spec())).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/stacks"));
        assert_eq!(
            body(&r[0]),
            json!({
                "stack_name": "web", "template": "heat_template_version: 2018-08-31", "environment": "parameters: {}",
                "parameters": {"flavor": "m1.small", "db_password": "********"}, "timeout_mins": 30, "disable_rollback": true
            }),
            "hidden parameters masked in the preview"
        );
        let r = dry(OsAction::StackUpdate {
            id: id("s1"),
            spec: spec(),
        })
        .await;
        assert_eq!(
            r[0].method.as_str(),
            "PATCH",
            "PATCH keeps the parameters the form leaves out"
        );
        assert!(r[0].path.ends_with("/stacks/web/s1"), "{}", r[0].path);
        assert!(body(&r[0]).get("stack_name").is_none());
        assert_eq!(OsAction::StackCreate(spec()).plugins(), [Plugin::Orchestration]);
    }

    #[tokio::test]
    async fn stack_bodies_are_checked() {
        let url = StackSpec {
            source: TemplateSource {
                template: String::new(),
                url: Some("https://git.example/t.yaml".into()),
                environment: None,
            },
            ..spec()
        };
        let r = dry(OsAction::StackCreate(url)).await;
        assert_eq!(body(&r[0])["template_url"], json!("https://git.example/t.yaml"));
        assert!(body(&r[0]).get("environment").is_none());
        let bad = |f: fn(&mut StackSpec)| {
            let mut s = spec();
            f(&mut s);
            OsAction::StackCreate(s)
        };
        assert!(
            dry_err(bad(|s| s.name = "1web".into())).await,
            "names start with a letter"
        );
        assert!(
            dry_err(bad(|s| s.source.template = " ".into())).await,
            "a template is required"
        );
        assert!(
            dry_err(bad(|s| s.source.url = Some("file:///etc/passwd".into()))).await,
            "http(s) URLs only"
        );
        assert!(
            dry_err(bad(|s| s.source.template = "x".repeat(MAX_TEMPLATE_BYTES + 1))).await,
            "Heat's size limit"
        );
        assert!(dry_err(bad(|s| s.timeout_mins = Some(0))).await);
    }
}
