//! Neutron FWaaS v2 (roadmap 93).

use super::*;

/// Whether a FWaaS row is the project's (`project_id`, or `tenant_id` on older Neutron).
fn owned_by(v: &Value, project_id: &str) -> bool {
    s(v, "project_id").or_else(|| s(v, "tenant_id")).as_deref() == Some(project_id)
}

/// A rule's `firewall_policy_id`: a list in FWaaS v2 (a rule can be in several
/// policies), a single ID or `null` on some releases.
fn rule_policy_ids(rule: &Value) -> Vec<String> {
    match rule.get("firewall_policy_id") {
        Some(Value::String(id)) if !id.is_empty() => vec![id.clone()],
        Some(Value::Array(ids)) => ids.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => vec![],
    }
}

/// Firewall rules, sorted by name, with the policies that use them: the rule's
/// `firewall_policy_id` and every visible policy that lists it.
pub fn firewall_rules(rules: &[Value], policies: &[Value], project_id: &str) -> Vec<FirewallRuleRow> {
    let policy_names = names_by_id(policies);
    let mut out: Vec<_> = rules
        .iter()
        .map(|r| {
            let id = s(r, "id").unwrap_or_default();
            let listed = policies
                .iter()
                .filter(|p| strs(p, "firewall_rules").contains(&id))
                .filter_map(|p| s(p, "id"));
            let ids = distinct(rule_policy_ids(r).into_iter().chain(listed));
            FirewallRuleRow {
                name: name_or_id(r),
                description: s_ne(r, "description"),
                protocol: s_ne(r, "protocol"),
                ip_version: i(r, "ip_version").and_then(|v| u8::try_from(v).ok()).unwrap_or(4),
                source_ip: s_ne(r, "source_ip_address"),
                source_port: s_ne(r, "source_port"),
                destination_ip: s_ne(r, "destination_ip_address"),
                destination_port: s_ne(r, "destination_port"),
                action: s(r, "action").unwrap_or_default(),
                // Neutron's default is enabled.
                enabled: r.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                shared: b(r, "shared"),
                policies: ids.into_iter().map(|p| named_ref(p, &policy_names)).collect(),
                owned: owned_by(r, project_id),
                id,
            }
        })
        .collect();
    out.sort_by_key(|r| r.name.to_lowercase());
    out
}

/// Firewall policies, sorted by name, their rules in evaluation order.
pub fn firewall_policies(policies: &[Value], rules: &[Value], project_id: &str) -> Vec<FirewallPolicyRow> {
    let rule_names = names_by_id(rules);
    let mut out: Vec<_> = policies
        .iter()
        .map(|p| FirewallPolicyRow {
            id: s(p, "id").unwrap_or_default(),
            name: name_or_id(p),
            description: s_ne(p, "description"),
            rules: strs(p, "firewall_rules")
                .into_iter()
                .map(|r| named_ref(r, &rule_names))
                .collect(),
            audited: b(p, "audited"),
            shared: b(p, "shared"),
            owned: owned_by(p, project_id),
        })
        .collect();
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

/// A policy's page: [`firewall_policies`] of one, and its rules in its order.
pub fn firewall_policy_detail(
    policy: &Value,
    rules: &[Value],
    all_policies: &[Value],
    project_id: &str,
) -> FirewallPolicyDetail {
    let p = firewall_policies(std::slice::from_ref(policy), rules, project_id).remove(0);
    let rows = firewall_rules(rules, all_policies, project_id);
    let by_id: HashMap<&str, &FirewallRuleRow> = rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let ordered = p
        .rules
        .iter()
        .filter_map(|r| by_id.get(r.id.as_str()).map(|x| (*x).clone()))
        .collect();
    FirewallPolicyDetail {
        policy: p,
        rules: ordered,
    }
}

/// What a port belongs to, from its `device_owner`: `router` for router
/// interfaces (FWaaS v2's L3 ports), `instance` for `compute:*`.
fn port_kind(port: &Value) -> Option<&'static str> {
    let owner = s(port, "device_owner").unwrap_or_default();
    if owner.starts_with("network:router_interface") || owner == "network:ha_router_replicated_interface" {
        Some("router")
    } else if owner.starts_with("compute:") {
        Some("instance")
    } else {
        None
    }
}

/// A port with its IPs and device; `routers` and `servers` name the device
/// (both may be empty: then the device is its ID).
fn firewall_port(
    id: String,
    ports: &HashMap<String, &Value>,
    routers: &HashMap<String, String>,
    servers: &HashMap<String, String>,
) -> FirewallPort {
    let Some(p) = ports.get(&id) else {
        return FirewallPort {
            port: Ref { name: id.clone(), id },
            ips: vec![],
            kind: None,
            device: None,
        };
    };
    let kind = port_kind(p);
    let device = s_ne(p, "device_id").map(|d| match kind {
        Some("router") => named_ref(d, routers),
        Some("instance") => named_ref(d, servers),
        _ => Ref { name: d.clone(), id: d },
    });
    FirewallPort {
        port: Ref {
            name: name_or_id(p),
            id,
        },
        ips: arr(p, "fixed_ips").iter().filter_map(|f| s(f, "ip_address")).collect(),
        kind: kind.map(str::to_string),
        device,
    }
}

/// Firewall groups, sorted by name, policies named and ports resolved from
/// `ports` (and their devices from `routers` / `servers`, which may be empty).
pub fn firewall_groups(
    groups: &[Value],
    policies: &[Value],
    ports: &[Value],
    routers: &[Value],
    servers: &[Value],
    project_id: &str,
) -> Vec<FirewallGroupRow> {
    let (policy_names, port_index, router_names, server_names) = (
        names_by_id(policies),
        index_by_id(ports),
        names_by_id(routers),
        names_by_id(servers),
    );
    let policy = |g: &Value, k: &str| s_ne(g, k).map(|id| named_ref(id, &policy_names));
    let mut out: Vec<_> = groups
        .iter()
        .map(|g| FirewallGroupRow {
            id: s(g, "id").unwrap_or_default(),
            name: name_or_id(g),
            description: s_ne(g, "description"),
            ingress: policy(g, "ingress_firewall_policy_id"),
            egress: policy(g, "egress_firewall_policy_id"),
            ports: strs(g, "ports")
                .into_iter()
                .map(|p| firewall_port(p, &port_index, &router_names, &server_names))
                .collect(),
            status: s(g, "status").unwrap_or_default(),
            admin_up: g.get("admin_state_up").and_then(Value::as_bool).unwrap_or(true),
            shared: b(g, "shared"),
            owned: owned_by(g, project_id),
        })
        .collect();
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

/// *Insert Rule*, *Add Policy*: the rules the project can use; cells `[action, protocol]`.
/// Rule pickers: labelled by name; cells `[name, action, protocol]` (empty: any),
/// which `pick::columns` translates in the browser.
pub fn firewall_rule_choices(rules: &[Value]) -> Vec<Choice> {
    sorted(
        rules
            .iter()
            .filter_map(|r| {
                let name = name_or_id(r);
                let cells = vec![
                    name.clone(),
                    s(r, "action").unwrap_or_default(),
                    s_ne(r, "protocol").unwrap_or_default(),
                ];
                Some(Choice {
                    value: s(r, "id")?,
                    label: name,
                    cells,
                    ..Default::default()
                })
            })
            .collect(),
    )
}

/// Ingress / egress policy pickers.
pub fn firewall_policy_choices(policies: &[Value]) -> Vec<Choice> {
    sorted(
        policies
            .iter()
            .filter_map(|p| Some(Choice::new(s(p, "id")?, name_or_id(p))))
            .collect(),
    )
}

/// Ports a firewall group can protect: the project's router interfaces and
/// instance ports that no group has yet (Neutron refuses a port another group
/// protects). Ports are seldom named, so the row says what the port belongs
/// to: cells `[device, kind, IPs, port]` (`pick::columns`), the port by name
/// or, unnamed, the start of its ID; labelled `device: IPs`.
pub fn firewall_port_choices(ports: &[Value], groups: &[Value], routers: &[Value], servers: &[Value]) -> Vec<Choice> {
    let taken: HashSet<String> = groups.iter().flat_map(|g| strs(g, "ports")).collect();
    let (index, router_names, server_names) = (index_by_id(ports), names_by_id(routers), names_by_id(servers));
    sorted(
        ports
            .iter()
            .filter(|p| port_kind(p).is_some())
            .filter_map(|p| {
                let id = s(p, "id")?;
                if taken.contains(&id) {
                    return None;
                }
                let fp = firewall_port(id.clone(), &index, &router_names, &server_names);
                let device = fp.device.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| "-".into());
                let ips = if fp.ips.is_empty() {
                    "-".to_string()
                } else {
                    fp.ips.join(", ")
                };
                let port = s_ne(p, "name").unwrap_or_else(|| id.chars().take(8).collect());
                let label = format!("{device}: {ips}");
                Some(Choice {
                    cells: vec![device, fp.kind.unwrap_or_default(), ips, port],
                    value: id,
                    label,
                    ..Default::default()
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn firewall_rules_read_null_as_any_and_name_their_policies() {
        let rules = vec![
            json!({"id": "r1", "name": "ssh", "protocol": "tcp", "ip_version": 4, "source_ip_address": null, "destination_ip_address": "10.0.0.0/24",
                   "source_port": null, "destination_port": "22", "action": "allow", "enabled": true, "shared": false, "project_id": "p1",
                   "firewall_policy_id": ["fp1"]}),
            json!({"id": "r2", "name": "", "protocol": null, "ip_version": 6, "action": "deny", "enabled": false, "shared": true, "tenant_id": "other",
                   "firewall_policy_id": null}),
        ];
        let policies = vec![
            json!({"id": "fp1", "name": "web", "firewall_rules": ["r1"]}),
            json!({"id": "fp2", "name": "", "firewall_rules": ["r2", "r1"]}),
        ];
        let r = firewall_rules(&rules, &policies, "p1");
        assert_eq!(
            (
                r[0].name.as_str(),
                r[0].protocol.as_deref(),
                r[0].source_ip.as_deref(),
                r[0].destination_port.as_deref()
            ),
            ("r2", None, None, None),
            "unnamed: the ID; null: any"
        );
        assert_eq!(
            (r[0].owned, r[0].shared, r[0].enabled, r[0].ip_version),
            (false, true, false, 6),
            "tenant_id on older Neutron"
        );
        assert_eq!(
            r[0].policies,
            [Ref {
                id: "fp2".into(),
                name: "fp2".into()
            }],
            "listed by a policy, though its own field is null"
        );
        assert_eq!(
            (
                r[1].destination_ip.as_deref(),
                r[1].destination_port.as_deref(),
                r[1].owned
            ),
            (Some("10.0.0.0/24"), Some("22"), true)
        );
        let names: Vec<_> = r[1].policies.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            ["web", "fp2"],
            "its own field and every policy that lists it, each once"
        );
        let one = firewall_rules(&[json!({"id": "r3", "firewall_policy_id": "fp1"})], &[], "p1");
        assert_eq!(one[0].policies[0].id, "fp1", "a single ID on some releases");
    }

    #[test]
    fn firewall_policies_keep_rule_order() {
        let p = firewall_policies(
            &[
                json!({"id": "fp1", "name": "web", "firewall_rules": ["r2", "r1", "gone"], "audited": true, "shared": false, "project_id": "p1"}),
            ],
            &[json!({"id": "r1", "name": "ssh"}), json!({"id": "r2", "name": "http"})],
            "p1",
        );
        let rules: Vec<_> = p[0].rules.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            rules,
            ["http", "ssh", "gone"],
            "evaluation order, not sorted; unknown rule: its ID"
        );
        assert!(p[0].audited && p[0].owned && !p[0].shared);
    }

    #[test]
    fn firewall_rule_choices_leave_action_and_protocol_to_the_columns() {
        let c = firewall_rule_choices(&[
            json!({"id": "r1", "name": "ssh", "action": "allow", "protocol": "tcp"}),
            json!({"id": "r2", "name": "", "action": "deny", "protocol": null}),
        ]);
        let ssh = c.iter().find(|c| c.value == "r1").unwrap();
        assert_eq!(
            (ssh.label.as_str(), ssh.cells.clone()),
            ("ssh", vec!["ssh".to_string(), "allow".into(), "tcp".into()]),
            "no English in the label"
        );
        let any = c.iter().find(|c| c.value == "r2").unwrap();
        assert_eq!(any.cells, ["r2", "deny", ""], "unnamed: its ID; any protocol: empty");
    }

    #[test]
    fn firewall_groups_resolve_ports_to_their_router_or_instance() {
        let groups = vec![
            json!({"id": "g1", "name": "gw", "ingress_firewall_policy_id": "fp1", "egress_firewall_policy_id": null,
                                 "ports": ["prt", "pvm", "pgone"], "status": "PENDING_UPDATE", "admin_state_up": true, "project_id": "p1"}),
        ];
        let ports = vec![
            json!({"id": "prt", "name": "", "device_owner": "network:router_interface", "device_id": "r1", "fixed_ips": [{"ip_address": "10.0.0.1"}]}),
            json!({"id": "pvm", "name": "vm-port", "device_owner": "compute:nova", "device_id": "s1", "fixed_ips": [{"ip_address": "10.0.0.5"}]}),
        ];
        let g = &firewall_groups(
            &groups,
            &[json!({"id": "fp1", "name": "web"})],
            &ports,
            &[json!({"id": "r1", "name": "router1"})],
            &[json!({"id": "s1", "name": "vm1"})],
            "p1",
        )[0];
        assert_eq!(
            (g.ingress.as_ref().map(|p| p.name.as_str()), g.egress.as_ref()),
            (Some("web"), None)
        );
        assert!(g.busy() && g.owned);
        let p: Vec<_> = g
            .ports
            .iter()
            .map(|p| {
                (
                    p.port.name.as_str(),
                    p.kind.as_deref(),
                    p.device.as_ref().map(|d| d.name.as_str()),
                    p.device_href(),
                )
            })
            .collect();
        assert_eq!(
            p,
            [
                (
                    "prt",
                    Some("router"),
                    Some("router1"),
                    Some("/project/routers/r1".into())
                ),
                (
                    "vm-port",
                    Some("instance"),
                    Some("vm1"),
                    Some("/project/instances/s1".into())
                ),
                ("pgone", None, None, None),
            ]
        );
    }

    #[test]
    fn firewall_port_choices_are_router_and_instance_ports_with_their_group() {
        let mut ports = vec![
            json!({"id": "prt", "device_owner": "network:router_interface_distributed", "device_id": "r1", "fixed_ips": [{"ip_address": "10.0.0.1"}]}),
            json!({"id": "pvm", "device_owner": "compute:az1", "device_id": "s1", "fixed_ips": []}),
            json!({"id": "pdhcp", "device_owner": "network:dhcp", "device_id": "d"}),
            json!({"id": "pgw", "device_owner": "network:router_gateway", "device_id": "r1"}),
        ];
        ports.push(json!({"id": "pvm2", "name": "web-nic", "device_owner": "compute:nova", "device_id": "s2", "fixed_ips": [{"ip_address": "10.0.0.7"}, {"ip_address": "fd00::7"}]}));
        let c = firewall_port_choices(
            &ports,
            &[json!({"id": "g1", "ports": ["pvm"]})],
            &[json!({"id": "r1", "name": "router1"})],
            &[json!({"id": "s2", "name": "vm2"})],
        );
        let got: Vec<_> = c
            .iter()
            .map(|c| (c.value.as_str(), c.label.as_str(), c.cells.clone()))
            .collect();
        assert_eq!(
            got,
            [
                (
                    "prt",
                    "router1: 10.0.0.1",
                    vec!["router1".to_string(), "router".into(), "10.0.0.1".into(), "prt".into()]
                ),
                (
                    "pvm2",
                    "vm2: 10.0.0.7, fd00::7",
                    vec![
                        "vm2".to_string(),
                        "instance".into(),
                        "10.0.0.7, fd00::7".into(),
                        "web-nic".into()
                    ]
                ),
            ],
            "no DHCP or gateway ports, none another group has; the device named, an unnamed port by its ID's start"
        );
    }
}
