//! Heat's read-only POSTs: `validate` (a template's parameters, for
//! the launch form) and `stacks/preview` (*Preview Stack*). They change nothing,
//! but are POSTs, so like the console calls (`console.rs`) they go through
//! [`Exec`]: sent only with `[writes] mode = on`, previewed in dry-run. In
//! dry-run the parameters are read from the template text instead (`local`).

use openstack_sdk::AsyncOpenStack;
use serde_json::Value;

use super::actions::{Exec, JsonRequest, stack_body, template_body};
use crate::model::*;

/// Every function here fails with an [`AppError`].
type R<T> = Result<T, AppError>;

/// `POST validate` with the template and environment.
pub fn validate_request(src: &TemplateSource) -> R<JsonRequest> {
    Ok(JsonRequest::orchestration(
        http::Method::POST,
        "validate",
        Some(Value::Object(template_body(src)?)),
    ))
}

/// `POST stacks/preview` with the same body as a create.
pub fn preview_request(spec: &StackSpec) -> R<JsonRequest> {
    Ok(JsonRequest::orchestration(
        http::Method::POST,
        "stacks/preview",
        Some(stack_body(spec, true)?),
    ))
}

/// A template's parameters: from Heat, or (dry-run, or a URL Heat must fetch)
/// read from the text.
pub async fn validate(
    mode: WriteMode,
    os: Option<&AsyncOpenStack>,
    src: &TemplateSource,
) -> R<ConsoleReply<TemplateInfo>> {
    let mut x = Exec::new(mode, os);
    let data = match x.call(validate_request(src)?).await? {
        Some(v) => validate_info(&v),
        None => local_info(&src.template),
    };
    Ok(ConsoleReply {
        mode: x.mode(),
        requests: x.requests(),
        data: Some(data),
    })
}

/// The resources the stack would create; `None` in dry-run.
pub async fn preview(
    mode: WriteMode,
    os: Option<&AsyncOpenStack>,
    spec: &StackSpec,
) -> R<ConsoleReply<Vec<PreviewResource>>> {
    let mut x = Exec::new(mode, os);
    let reply = x.call(preview_request(spec)?).await?;
    x.hide_params(&spec.hidden);
    Ok(ConsoleReply {
        mode: x.mode(),
        requests: x.requests(),
        data: reply.map(|v| preview_resources(&v)),
    })
}

/// A JSON or YAML scalar as text; lists of scalars joined with commas (Heat's
/// `comma_delimited_list`); maps and nested lists as JSON.
fn text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Array(a) if a.iter().all(|x| !x.is_array() && !x.is_object()) => {
            Some(a.iter().filter_map(text).collect::<Vec<_>>().join(","))
        }
        other => Some(other.to_string()),
    }
}

/// Heat's `Type` (`String`, `CommaDelimitedList`) or HOT's `type` as the form's kind.
fn kind(t: &str) -> String {
    match t.to_ascii_lowercase().as_str() {
        "number" => "number",
        "json" => "json",
        "boolean" => "boolean",
        "commadelimitedlist" | "comma_delimited_list" => "comma_delimited_list",
        _ => "string",
    }
    .into()
}

/// `POST validate`'s answer (CFN-style keys: `Parameters`, `Type`, `NoEcho`…).
/// Parameters follow `ParameterGroups`, then the rest by name (the JSON map loses
/// the template's order).
pub fn validate_info(v: &Value) -> TemplateInfo {
    let s = |x: &Value, k: &str| {
        x.get(k)
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|t| !t.is_empty())
    };
    let empty = serde_json::Map::new();
    let params = v.get("Parameters").and_then(Value::as_object).unwrap_or(&empty);
    let mut order: Vec<String> = v
        .get("ParameterGroups")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|g| {
            g.get("parameters")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|n| n.as_str().map(str::to_string))
        .filter(|n| params.contains_key(n))
        .collect();
    let mut rest: Vec<String> = params.keys().filter(|k| !order.contains(k)).cloned().collect();
    rest.sort();
    order.extend(rest);
    let params = order
        .into_iter()
        .filter_map(|name| {
            let p = params.get(&name)?;
            // `NoEcho` is the string "true" in most releases, a boolean in some.
            let hidden = matches!(p.get("NoEcho"), Some(Value::Bool(true)))
                || s(p, "NoEcho").is_some_and(|x| x.eq_ignore_ascii_case("true"));
            Some(TemplateParam {
                label: s(p, "Label").filter(|l| *l != name),
                description: s(p, "Description"),
                kind: kind(&s(p, "Type").unwrap_or_default()),
                default: p.get("Default").and_then(text),
                hidden,
                allowed: p
                    .get("AllowedValues")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(text).collect())
                    .unwrap_or_default(),
                name,
            })
        })
        .collect();
    TemplateInfo {
        description: s(v, "Description"),
        params,
        local: false,
    }
}

/// The parameters of a HOT (or CFN) template read from its text, in template
/// order, for dry-run. Unreadable text gives none: Heat reports the error once
/// the request is really sent.
pub fn local_info(template: &str) -> TemplateInfo {
    use yaml_rust2::{Yaml, YamlLoader};
    fn json(y: &Yaml) -> Value {
        match y {
            Yaml::String(s) | Yaml::Real(s) => Value::String(s.clone()),
            Yaml::Integer(i) => Value::from(*i),
            Yaml::Boolean(b) => Value::Bool(*b),
            Yaml::Array(a) => Value::Array(a.iter().map(json).collect()),
            Yaml::Hash(h) => Value::Object(
                h.iter()
                    .filter_map(|(k, v)| Some((k.as_str()?.to_string(), json(v))))
                    .collect(),
            ),
            _ => Value::Null,
        }
    }
    let Some(doc) = YamlLoader::load_from_str(template)
        .ok()
        .and_then(|d| d.into_iter().next())
    else {
        return TemplateInfo {
            local: true,
            ..TemplateInfo::default()
        };
    };
    let hot = !doc["heat_template_version"].is_badvalue();
    let (params_key, desc_key) = if hot {
        ("parameters", "description")
    } else {
        ("Parameters", "Description")
    };
    let params = match &doc[params_key] {
        Yaml::Hash(h) => h
            .iter()
            .filter_map(|(k, p)| {
                let name = k.as_str()?.to_string();
                let p = json(p);
                let g = |hot_key: &str, cfn_key: &str| p.get(if hot { hot_key } else { cfn_key }).cloned();
                let allowed = if hot {
                    p.get("constraints")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .find_map(|c| c.get("allowed_values")?.as_array().cloned())
                } else {
                    p.get("AllowedValues").and_then(Value::as_array).cloned()
                };
                let hidden = match g("hidden", "NoEcho") {
                    Some(Value::Bool(b)) => b,
                    Some(Value::String(s)) => s.eq_ignore_ascii_case("true"),
                    _ => false,
                };
                Some(TemplateParam {
                    label: g("label", "Label").as_ref().and_then(text),
                    description: g("description", "Description").as_ref().and_then(text),
                    kind: kind(g("type", "Type").as_ref().and_then(Value::as_str).unwrap_or_default()),
                    default: g("default", "Default").as_ref().and_then(text),
                    hidden,
                    allowed: allowed.unwrap_or_default().iter().filter_map(text).collect(),
                    name,
                })
            })
            .collect(),
        _ => vec![],
    };
    TemplateInfo {
        description: doc[desc_key].as_str().map(str::to_string),
        params,
        local: true,
    }
}

/// `stacks/preview`'s resources; nested stacks come as nested lists.
pub fn preview_resources(v: &Value) -> Vec<PreviewResource> {
    fn walk(items: &[Value], depth: u32, out: &mut Vec<PreviewResource>) {
        for x in items {
            match x {
                Value::Array(a) => walk(a, depth + 1, out),
                Value::Object(_) => out.push(PreviewResource {
                    name: x
                        .get("resource_name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    kind: x
                        .get("resource_type")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    depth,
                }),
                _ => {}
            }
        }
    }
    let stack = v.get("stack").unwrap_or(v);
    let mut out = vec![];
    walk(
        stack
            .get("resources")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        0,
        &mut out,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const HOT: &str = r#"
heat_template_version: 2018-08-31
description: A web server.
parameters:
  flavor:
    type: string
    label: Flavor
    default: m1.small
    constraints:
      - allowed_values: [m1.small, m1.large]
  db_password:
    type: string
    hidden: true
  ports:
    type: comma_delimited_list
    default: [80, 443]
  count:
    type: number
    default: 2
resources: {}
"#;

    fn src(t: &str) -> TemplateSource {
        TemplateSource {
            template: t.into(),
            url: None,
            environment: None,
        }
    }

    #[test]
    fn local_parameters_keep_the_template_order() {
        let i = local_info(HOT);
        assert!(i.local);
        assert_eq!(i.description.as_deref(), Some("A web server."));
        let names: Vec<_> = i.params.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["flavor", "db_password", "ports", "count"]);
        assert_eq!(i.params[0].allowed, ["m1.small", "m1.large"]);
        assert_eq!(i.params[0].label.as_deref(), Some("Flavor"));
        assert!(i.params[1].hidden && i.params[1].default.is_none());
        assert_eq!(
            (i.params[2].kind.as_str(), i.params[2].default.as_deref()),
            ("comma_delimited_list", Some("80,443"))
        );
        assert_eq!(i.params[3].default.as_deref(), Some("2"));
        assert_eq!(
            local_info("not: [valid").params,
            vec![],
            "Heat reports the error when sent"
        );
    }

    #[test]
    fn heats_answer_uses_groups_then_names() {
        let i = validate_info(&json!({
            "Description": "A web server.",
            "Parameters": {
                "zeta": {"Type": "String", "NoEcho": "true", "Label": "zeta", "Description": ""},
                "flavor": {"Type": "String", "Default": "m1.small", "AllowedValues": ["m1.small", "m1.large"], "NoEcho": "false", "Label": "Flavor"},
                "count": {"Type": "Number", "Default": 2},
                "ports": {"Type": "CommaDelimitedList", "Default": "80,443"}
            },
            "ParameterGroups": [{"label": "Main", "parameters": ["flavor", "gone"]}]
        }));
        assert!(!i.local);
        let names: Vec<_> = i.params.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["flavor", "count", "ports", "zeta"]);
        assert_eq!(i.params[3].label, None, "a label equal to the name adds nothing");
        assert!(i.params[3].hidden && !i.params[0].hidden, "NoEcho is a string");
        assert_eq!(
            (i.params[1].kind.as_str(), i.params[1].default.as_deref()),
            ("number", Some("2"))
        );
        assert_eq!(i.params[2].kind, "comma_delimited_list");
    }

    #[tokio::test]
    async fn dry_run_previews_validate_and_reads_the_template() {
        let r = validate(WriteMode::DryRun, None, &src(HOT)).await.unwrap();
        assert_eq!(
            (r.requests[0].method.as_str(), r.requests[0].path.as_str()),
            ("POST", "/validate")
        );
        assert_eq!(r.data.unwrap().params.len(), 4, "parameters read locally");
        assert!(
            validate(WriteMode::Off, None, &src(HOT)).await.is_err(),
            "a POST: refused with writes off"
        );
    }

    #[tokio::test]
    async fn preview_is_a_create_body_with_hidden_values_masked() {
        let spec = StackSpec {
            name: "web".into(),
            source: src(HOT),
            parameters: vec![("db_password".into(), "s3cret".into())],
            hidden: vec!["db_password".into()],
            ..StackSpec::default()
        };
        let r = preview(WriteMode::DryRun, None, &spec).await.unwrap();
        assert_eq!(
            (r.requests[0].method.as_str(), r.requests[0].path.as_str(), r.data),
            ("POST", "/stacks/preview", None)
        );
        let body: Value = serde_json::from_str(r.requests[0].body.as_deref().unwrap()).unwrap();
        assert_eq!(body["parameters"]["db_password"], "********");
        assert_eq!(body["stack_name"], "web");
    }

    #[test]
    fn preview_flattens_nested_stacks() {
        let out = preview_resources(&json!({"stack": {"resources": [
            {"resource_name": "server", "resource_type": "OS::Nova::Server"},
            [{"resource_name": "0", "resource_type": "OS::Cinder::Volume"}]
        ]}}));
        let rows: Vec<_> = out
            .iter()
            .map(|r| (r.name.as_str(), r.kind.as_str(), r.depth))
            .collect();
        assert_eq!(
            rows,
            [("server", "OS::Nova::Server", 0), ("0", "OS::Cinder::Volume", 1)]
        );
    }
}
