//! Heat (roadmap 62): stacks, resources, events, resource types, templates.

use super::*;

/// Heat stacks, newest first like Horizon.
pub fn stacks(raw: &[Value]) -> Vec<StackRow> {
    let mut out: Vec<_> = raw.iter().map(stack_row).collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

fn stack_row(x: &Value) -> StackRow {
    let id = s(x, "id").unwrap_or_default();
    StackRow {
        name: s_ne(x, "stack_name").unwrap_or_else(|| id.clone()),
        id,
        description: s_ne(x, "description"),
        status: s(x, "stack_status").unwrap_or_default(),
        status_reason: s_ne(x, "stack_status_reason"),
        created: s(x, "creation_time").unwrap_or_default(),
        updated: s_ne(x, "updated_time"),
    }
}

/// The value of an output or parameter as text: strings as they are, other
/// JSON compact, `-` for `null`.
fn heat_value(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "-".into(),
        Some(Value::String(x)) => x.clone(),
        Some(other) => other.to_string(),
    }
}

/// The stack detail from `GET stacks/{name}/{id}` (`stack` unwrapped or not),
/// its resources, events and template.
pub fn stack_detail(stack: &Value, resources: &[Value], events: &[Value], template: &Value) -> StackDetail {
    let x = unwrap(stack, "stack");
    let mut parameters: Vec<(String, String)> = x
        .get("parameters")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        // Heat adds `OS::stack_id`, `OS::stack_name` and `OS::project_id` to every stack.
        .filter(|(k, _)| !k.starts_with("OS::"))
        .map(|(k, v)| (k.clone(), heat_value(Some(v))))
        .collect();
    parameters.sort();
    let mut outputs: Vec<StackOutput> = arr(x, "outputs")
        .iter()
        .map(|o| StackOutput {
            key: s(o, "output_key").unwrap_or_default(),
            value: heat_value(o.get("output_value")),
            description: s_ne(o, "description"),
            error: s_ne(o, "output_error"),
        })
        .collect();
    outputs.sort_by(|a, b| a.key.cmp(&b.key));
    StackDetail {
        stack: stack_row(x),
        parameters,
        outputs,
        timeout_mins: i(x, "timeout_mins"),
        disable_rollback: x.get("disable_rollback").and_then(Value::as_bool).unwrap_or(true),
        parent: s_ne(x, "parent"),
        tags: strs(x, "tags"),
        resources: stack_resources(resources),
        events: stack_events(events),
        template: serde_json::to_string_pretty(template).unwrap_or_default(),
    }
}

/// A stack's resources by name, with `depends_on` filled from the other
/// resources' `required_by` and a link to Moonlight's page for each.
pub fn stack_resources(raw: &[Value]) -> Vec<StackResource> {
    let mut out: Vec<StackResource> = raw
        .iter()
        .map(|r| {
            let resource_type = s(r, "resource_type").unwrap_or_default();
            let physical_id = s_ne(r, "physical_resource_id");
            StackResource {
                name: s(r, "resource_name").unwrap_or_default(),
                href: physical_id.as_deref().and_then(|id| resource_href(&resource_type, id)),
                physical_id,
                resource_type,
                status: s(r, "resource_status").unwrap_or_default(),
                status_reason: s_ne(r, "resource_status_reason"),
                updated: s(r, "updated_time")
                    .or_else(|| s(r, "creation_time"))
                    .unwrap_or_default(),
                required_by: strs(r, "required_by"),
                depends_on: Vec::new(),
            }
        })
        .collect();
    let edges: Vec<(String, String)> = out
        .iter()
        .flat_map(|r| r.required_by.iter().map(|by| (by.clone(), r.name.clone())))
        .collect();
    for r in &mut out {
        r.depends_on = edges
            .iter()
            .filter(|(by, _)| *by == r.name)
            .map(|(_, on)| on.clone())
            .collect();
        r.depends_on.sort();
        r.required_by.sort();
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Moonlight's page for a Heat resource, by its type. Nested stacks
/// (`OS::Heat::Stack`, groups, provider templates) link to their stack page.
pub fn resource_href(resource_type: &str, physical_id: &str) -> Option<String> {
    let path = match resource_type {
        "OS::Nova::Server" => "instances",
        "OS::Nova::KeyPair" => "key_pairs",
        "OS::Nova::ServerGroup" => "server_groups",
        "OS::Glance::WebImage" | "OS::Glance::Image" => "images",
        "OS::Cinder::Volume" => "volumes",
        "OS::Neutron::Net" | "OS::Neutron::ProviderNet" => "networks",
        "OS::Neutron::Subnet" => "networks/subnets",
        "OS::Neutron::Port" => "networks/ports",
        "OS::Neutron::Router" => "routers",
        "OS::Neutron::FloatingIP" => "floating_ips",
        "OS::Neutron::SecurityGroup" => "security_groups",
        "OS::Neutron::Trunk" => "trunks",
        "OS::Neutron::QoSPolicy" => "qos",
        "OS::Octavia::LoadBalancer" => "load_balancers",
        // Barbican's physical IDs are refs (URLs).
        "OS::Barbican::Secret" => return Some(format!("/project/key_manager/secrets/{}", ref_id(physical_id))),
        "OS::Barbican::GenericContainer" | "OS::Barbican::CertificateContainer" | "OS::Barbican::RSAContainer" => {
            return Some(format!("/project/key_manager/containers/{}", ref_id(physical_id)));
        }
        "OS::Heat::Stack" | "OS::Heat::ResourceGroup" | "OS::Heat::AutoScalingGroup" => "stacks",
        t if t.ends_with(".yaml") || t.ends_with(".template") || t.starts_with("file://") || t.starts_with("http") => {
            "stacks"
        }
        _ => return None,
    };
    Some(format!("/project/{path}/{physical_id}"))
}

/// Stack events, newest first (whatever order Heat sent them in).
pub fn stack_events(raw: &[Value]) -> Vec<StackEvent> {
    let mut out: Vec<StackEvent> = raw
        .iter()
        .map(|e| StackEvent {
            id: s(e, "id").unwrap_or_default(),
            time: s(e, "event_time").unwrap_or_default(),
            resource_name: s(e, "resource_name")
                .or_else(|| s(e, "logical_resource_id"))
                .unwrap_or_default(),
            physical_id: s_ne(e, "physical_resource_id"),
            status: s(e, "resource_status").unwrap_or_default(),
            status_reason: s_ne(e, "resource_status_reason"),
        })
        .collect();
    out.sort_by(|a, b| b.time.cmp(&a.time));
    out
}

/// Resource types by name. Heat lists `{resource_type, description}` with
/// `with_description=true` (Pike) and bare names before it.
pub fn resource_types(raw: &[Value]) -> Vec<ResourceTypeRow> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|x| match x {
            Value::String(n) => Some(ResourceTypeRow {
                name: n.clone(),
                description: None,
            }),
            _ => Some(ResourceTypeRow {
                name: s_ne(x, "resource_type")?,
                description: s_ne(x, "description"),
            }),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// A constraint without its `description`, compact: `allowed_values: ["a","b"]`.
fn heat_constraint(c: &Value) -> Option<String> {
    let m = c.as_object()?;
    let parts: Vec<String> = m
        .iter()
        .filter(|(k, _)| *k != "description")
        .map(|(k, v)| format!("{k}: {}", heat_value(Some(v))))
        .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// A resource type's schema (`GET resource_types/{type}`), properties and
/// attributes sorted by name.
pub fn resource_type_detail(v: &Value) -> ResourceTypeDetail {
    let support = v.get("support_status").and_then(|st| {
        let status = s_ne(st, "status")?;
        Some(match s_ne(st, "version") {
            Some(ver) => format!("{status} ({ver})"),
            None => status,
        })
    });
    let entries = |k: &str| {
        let mut e: Vec<(String, Value)> = v
            .get(k)
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, x)| (k.clone(), x.clone())).collect())
            .unwrap_or_default();
        e.sort_by(|a, b| a.0.cmp(&b.0));
        e
    };
    let properties = entries("properties")
        .into_iter()
        .map(|(name, p)| TypeProperty {
            kind: s(&p, "type").unwrap_or_default(),
            required: b(&p, "required"),
            update_allowed: b(&p, "update_allowed"),
            default: p.get("default").filter(|d| !d.is_null()).map(|d| heat_value(Some(d))),
            description: s_ne(&p, "description"),
            constraints: arr(&p, "constraints").iter().filter_map(heat_constraint).collect(),
            allowed: arr(&p, "constraints")
                .iter()
                .flat_map(|c| arr(c, "allowed_values"))
                .map(|x| x.as_str().map_or_else(|| x.to_string(), str::to_string))
                .collect(),
            custom: arr(&p, "constraints").iter().find_map(|c| s_ne(c, "custom_constraint")),
            name,
        })
        .collect();
    let attributes = entries("attributes")
        .into_iter()
        .map(|(name, a)| TypeAttribute {
            kind: s_ne(&a, "type"),
            description: s_ne(&a, "description"),
            name,
        })
        .collect();
    ResourceTypeDetail {
        name: s(v, "resource_type").unwrap_or_default(),
        description: s_ne(v, "description"),
        support,
        properties,
        attributes,
        raw: serde_json::to_string_pretty(v).unwrap_or_default(),
    }
}

/// Template versions, newest first within each kind (HOT, then CFN), as their
/// names sort by date.
pub fn template_versions(raw: &[Value]) -> Vec<TemplateVersion> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|x| {
            Some(TemplateVersion {
                version: s_ne(x, "version")?,
                kind: s(x, "type").unwrap_or_default(),
                aliases: strs(x, "aliases"),
            })
        })
        .collect();
    out.sort_by(|a, b| a.kind.cmp(&b.kind).reverse().then_with(|| b.version.cmp(&a.version)));
    out
}

/// A template version's functions by name (Heat calls the name `functions`).
pub fn template_functions(raw: &[Value]) -> Vec<TemplateFunction> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|x| {
            Some(TemplateFunction {
                name: s_ne(x, "functions")?,
                description: s_ne(x, "description"),
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stack_detail_hides_heat_parameters_and_keeps_output_errors() {
        let stack = json!({"stack": {
            "id": "s1", "stack_name": "web", "stack_status": "CREATE_FAILED", "creation_time": "2026-01-01T10:00:00Z",
            "parameters": {"OS::stack_id": "s1", "OS::stack_name": "web", "OS::project_id": "p1", "flavor": "m1.small", "password": "******", "count": 2},
            "outputs": [
                {"output_key": "url", "output_value": "http://10.0.0.5", "description": "Site"},
                {"output_key": "ips", "output_value": ["10.0.0.5", "10.0.0.6"], "description": "No description given"},
                {"output_key": "broken", "output_value": null, "output_error": "Resource server failed"}
            ],
            "timeout_mins": 60, "disable_rollback": false, "parent": null, "tags": ["a", "b"]
        }});
        let template = json!({"heat_template_version": "2018-08-31", "resources": {}});
        let d = stack_detail(&stack, &[], &[], &template);
        assert_eq!(
            d.parameters,
            [
                ("count".into(), "2".into()),
                ("flavor".into(), "m1.small".into()),
                ("password".into(), "******".into())
            ],
            "OS::* hidden, sorted, numbers as text"
        );
        assert_eq!(d.outputs[0].key, "broken");
        assert_eq!(
            (d.outputs[0].value.as_str(), d.outputs[0].error.as_deref()),
            ("-", Some("Resource server failed"))
        );
        assert_eq!(d.outputs[1].value, r#"["10.0.0.5","10.0.0.6"]"#, "non-strings as JSON");
        assert_eq!(d.outputs[2].value, "http://10.0.0.5");
        assert_eq!(
            (d.timeout_mins, d.disable_rollback, d.parent.clone()),
            (Some(60), false, None)
        );
        assert_eq!(d.tags, ["a", "b"]);
        assert!(
            d.template.contains("\"heat_template_version\": \"2018-08-31\""),
            "pretty JSON"
        );
        assert_eq!(
            stack_detail(&stack["stack"], &[], &[], &Value::Null).stack.name,
            "web",
            "unwrapped too"
        );
    }

    #[test]
    fn stack_resources_link_and_invert_dependencies() {
        let raw = vec![
            json!({"resource_name": "server", "physical_resource_id": "vm1", "resource_type": "OS::Nova::Server", "resource_status": "CREATE_COMPLETE",
                   "updated_time": "2026-01-01T10:01:00Z", "required_by": []}),
            json!({"resource_name": "port", "physical_resource_id": "p1", "resource_type": "OS::Neutron::Port", "resource_status": "CREATE_COMPLETE",
                   "creation_time": "2026-01-01T10:00:00Z", "required_by": ["server", "fip"]}),
            json!({"resource_name": "fip", "physical_resource_id": "", "resource_type": "OS::Neutron::FloatingIP", "resource_status": "INIT_COMPLETE",
                   "required_by": []}),
            json!({"resource_name": "cert", "physical_resource_id": "https://kms:9311/v1/secrets/abc", "resource_type": "OS::Barbican::Secret", "required_by": []}),
            json!({"resource_name": "group", "physical_resource_id": "nested1", "resource_type": "OS::Heat::ResourceGroup", "required_by": []}),
            json!({"resource_name": "custom", "physical_resource_id": "x1", "resource_type": "OS::Zaqar::Queue", "required_by": []}),
        ];
        let out = stack_resources(&raw);
        let by = |n: &str| out.iter().find(|r| r.name == n).unwrap();
        assert_eq!(by("server").depends_on, ["port"]);
        assert_eq!(by("fip").depends_on, ["port"]);
        assert_eq!(by("port").required_by, ["fip", "server"], "sorted");
        assert_eq!(
            by("port").updated,
            "2026-01-01T10:00:00Z",
            "creation_time when never updated"
        );
        assert_eq!(by("server").href.as_deref(), Some("/project/instances/vm1"));
        assert_eq!(by("port").href.as_deref(), Some("/project/networks/ports/p1"));
        assert_eq!(
            (by("fip").physical_id.clone(), by("fip").href.clone()),
            (None, None),
            "not created yet"
        );
        assert_eq!(
            by("cert").href.as_deref(),
            Some("/project/key_manager/secrets/abc"),
            "Barbican refs"
        );
        assert_eq!(
            by("group").href.as_deref(),
            Some("/project/stacks/nested1"),
            "nested stack"
        );
        assert_eq!(by("custom").href, None, "no Moonlight page");
        assert_eq!(
            resource_href("templates/web.yaml", "n2").as_deref(),
            Some("/project/stacks/n2"),
            "provider template"
        );
    }

    #[test]
    fn stack_events_newest_first() {
        let raw = vec![
            json!({"id": "e1", "event_time": "2026-01-01T10:00:00Z", "resource_name": "web", "resource_status": "CREATE_IN_PROGRESS",
                   "resource_status_reason": "Stack CREATE started", "physical_resource_id": "s1"}),
            json!({"id": "e2", "event_time": "2026-01-01T10:02:00Z", "logical_resource_id": "server", "resource_status": "CREATE_FAILED",
                   "resource_status_reason": "Quota exceeded", "physical_resource_id": ""}),
        ];
        let out = stack_events(&raw);
        assert_eq!(out.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["e2", "e1"]);
        assert_eq!(
            out[0].resource_name, "server",
            "logical_resource_id when resource_name is missing"
        );
        assert_eq!(out[0].physical_id, None);
    }

    #[test]
    fn resource_type_schema_tables() {
        let d = resource_type_detail(&json!({
            "resource_type": "OS::Nova::Server",
            "description": "A server.",
            "support_status": {"status": "SUPPORTED", "version": "2014.1", "message": null},
            "properties": {
                "name": {"type": "string", "required": false, "update_allowed": true, "description": "Name."},
                "flavor": {"type": "string", "required": true, "update_allowed": true,
                           "constraints": [{"custom_constraint": "nova.flavor"}]},
                "count": {"type": "integer", "default": 1, "required": false, "update_allowed": false,
                          "constraints": [{"range": {"min": 0}, "description": "At least zero."}, {"allowed_values": [1, 2]}]}
            },
            "attributes": {"networks": {"type": "map", "description": "Addresses."}, "show": {"description": "All."}}
        }));
        assert_eq!(d.support.as_deref(), Some("SUPPORTED (2014.1)"));
        assert_eq!(
            d.properties.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["count", "flavor", "name"]
        );
        let count = &d.properties[0];
        assert_eq!(
            (
                count.kind.as_str(),
                count.required,
                count.update_allowed,
                count.default.as_deref()
            ),
            ("integer", false, false, Some("1"))
        );
        assert_eq!(
            count.constraints,
            [r#"range: {"min":0}"#, "allowed_values: [1,2]"],
            "descriptions left out"
        );
        assert_eq!(d.properties[1].constraints, ["custom_constraint: nova.flavor"]);
        assert_eq!(
            (count.allowed.as_slice(), count.custom.as_deref()),
            (["1".to_string(), "2".to_string()].as_slice(), None)
        );
        assert_eq!(
            (d.properties[1].allowed.len(), d.properties[1].custom.as_deref()),
            (0, Some("nova.flavor"))
        );
        assert_eq!(d.properties[1].default, None);
        assert_eq!(d.attributes[1].kind, None, "old Heat reports no attribute types");
        assert!(d.raw.contains("\"OS::Nova::Server\""));
    }

    #[test]
    fn template_versions_hot_first_newest_first() {
        let out = template_versions(&[
            json!({"version": "AWSTemplateFormatVersion.2010-09-09", "type": "cfn", "aliases": []}),
            json!({"version": "heat_template_version.2013-05-23", "type": "hot", "aliases": []}),
            json!({"version": "heat_template_version.2018-08-31", "type": "hot", "aliases": ["heat_template_version.rocky"]}),
        ]);
        assert_eq!(
            out.iter().map(|v| v.version.as_str()).collect::<Vec<_>>(),
            [
                "heat_template_version.2018-08-31",
                "heat_template_version.2013-05-23",
                "AWSTemplateFormatVersion.2010-09-09"
            ]
        );
        assert_eq!(out[0].aliases, ["heat_template_version.rocky"]);
        let f = template_functions(&[
            json!({"functions": "get_param", "description": "Get a parameter."}),
            json!({"functions": "get_attr", "description": ""}),
        ]);
        assert_eq!(
            f.iter()
                .map(|x| (x.name.as_str(), x.description.is_some()))
                .collect::<Vec<_>>(),
            [("get_attr", false), ("get_param", true)]
        );
    }
}
