//! Octavia: load balancers, listeners, pools, L7 policies.

use super::*;

/// A listener's *Advanced* options (roadmap 54). `insert_headers` values are
/// strings (`"true"`/`"false"`); only the true ones count. Missing keys (older
/// Octavia) read as defaults.
fn listener_options(v: &Value) -> LbListenerOptions {
    let ms = |k: &str| i(v, k).and_then(|n| u32::try_from(n).ok());
    let on = |x: &Value| x.as_bool() == Some(true) || x.as_str().is_some_and(|s| s.eq_ignore_ascii_case("true"));
    let mut insert_headers: Vec<String> = v
        .get("insert_headers")
        .and_then(Value::as_object)
        .map(|m| m.iter().filter(|(_, x)| on(x)).map(|(k, _)| k.clone()).collect())
        .unwrap_or_default();
    insert_headers.sort();
    LbListenerOptions {
        timeout_client_data: ms("timeout_client_data"),
        timeout_member_connect: ms("timeout_member_connect"),
        timeout_member_data: ms("timeout_member_data"),
        timeout_tcp_inspect: ms("timeout_tcp_inspect"),
        insert_headers,
        // Present (even `null`) from Octavia 2.12: offered in the edit dialog then.
        allowed_cidrs: v.get("allowed_cidrs").map(|_| strs(v, "allowed_cidrs")),
        tls_ciphers: s_ne(v, "tls_ciphers"),
    }
}

pub(super) fn ids(v: &Value, k: &str) -> Vec<String> {
    arr(v, k).iter().filter_map(|x| s(x, "id")).collect()
}

pub(super) fn named(id: String, names: &HashMap<String, String>) -> Ref {
    Ref {
        name: names.get(&id).cloned().unwrap_or_else(|| id.clone()),
        id,
    }
}

pub(super) fn status(v: &Value, k: &str) -> String {
    s(v, k).unwrap_or_else(|| "UNKNOWN".into())
}

/// Load balancer rows (Octavia), with VIP and statuses.
pub fn load_balancers(raw: &[Value]) -> Vec<LoadBalancerRow> {
    let mut out: Vec<_> = raw.iter().map(load_balancer_row).collect();
    out.sort_by_key(|l| l.name.to_lowercase());
    out
}

fn load_balancer_row(l: &Value) -> LoadBalancerRow {
    LoadBalancerRow {
        id: s(l, "id").unwrap_or_default(),
        name: name_or_id(l),
        description: s_ne(l, "description"),
        vip_address: s_ne(l, "vip_address"),
        availability_zone: s_ne(l, "availability_zone"),
        operating_status: status(l, "operating_status"),
        provisioning_status: status(l, "provisioning_status"),
        admin_up: b(l, "admin_state_up"),
        provider: s_ne(l, "provider"),
    }
}

/// What the detail page needs, beyond the load balancer itself.
pub struct LbInputs<'a> {
    pub listeners: &'a [Value],
    pub pools: &'a [Value],
    /// Every pool's members, each tagged with its pool by the caller.
    pub members: &'a [(String, Vec<Value>)],
    pub monitors: &'a [Value],
    pub networks: &'a [Value],
    pub subnets: &'a [Value],
    pub floating_ips: &'a [Value],
    /// The project's L7 policies; those of other load balancers are dropped.
    pub l7policies: &'a [Value],
    /// Each policy's rules, tagged with the policy by the caller.
    pub l7rules: &'a [(String, Vec<Value>)],
}

/// The load balancer detail page from the load balancer and `x`, the rest.
pub fn load_balancer_detail(l: &Value, x: LbInputs) -> LoadBalancerDetail {
    let pool_names = names_by_id(x.pools);
    let listener_names = names_by_id(x.listeners);
    let monitor_names = names_by_id(x.monitors);
    let subnet_names = names_by_id(x.subnets);
    let net_names = names_by_id(x.networks);
    let pool_ids: std::collections::HashSet<String> = x.pools.iter().filter_map(|p| s(p, "id")).collect();
    let by_name = |a: &String, b: &String| a.to_lowercase().cmp(&b.to_lowercase());

    let mut listeners: Vec<_> = x
        .listeners
        .iter()
        .map(|v| LbListener {
            id: s(v, "id").unwrap_or_default(),
            name: name_or_id(v),
            description: s_ne(v, "description"),
            protocol: s(v, "protocol").unwrap_or_default(),
            port: i(v, "protocol_port"),
            default_pool: s_ne(v, "default_pool_id").map(|id| named(id, &pool_names)),
            connection_limit: i(v, "connection_limit").filter(|&n| n >= 0),
            operating_status: status(v, "operating_status"),
            provisioning_status: status(v, "provisioning_status"),
            admin_up: b(v, "admin_state_up"),
            options: listener_options(v),
            default_tls_container_ref: s_ne(v, "default_tls_container_ref"),
            sni_container_refs: strs(v, "sni_container_refs"),
        })
        .collect();
    listeners.sort_by(|a, b| a.port.cmp(&b.port).then_with(|| by_name(&a.name, &b.name)));

    let mut pools: Vec<_> = x
        .pools
        .iter()
        .map(|v| {
            let sp = v.get("session_persistence").filter(|p| p.is_object());
            let persistence = sp.and_then(|p| {
                let kind = s_ne(p, "type")?;
                Some(match s_ne(p, "cookie_name") {
                    Some(c) => format!("{kind} ({c})"),
                    None => kind,
                })
            });
            LbPool {
                id: s(v, "id").unwrap_or_default(),
                name: name_or_id(v),
                protocol: s(v, "protocol").unwrap_or_default(),
                algorithm: s(v, "lb_algorithm").unwrap_or_default(),
                session_persistence: persistence,
                persistence_type: sp.and_then(|p| s_ne(p, "type")),
                cookie_name: sp.and_then(|p| s_ne(p, "cookie_name")),
                description: s_ne(v, "description"),
                listeners: ids(v, "listeners")
                    .into_iter()
                    .map(|id| named(id, &listener_names))
                    .collect(),
                members: arr(v, "members").len(),
                monitor: s_ne(v, "healthmonitor_id").map(|id| named(id, &monitor_names)),
                operating_status: status(v, "operating_status"),
                provisioning_status: status(v, "provisioning_status"),
                admin_up: b(v, "admin_state_up"),
                tls_enabled: b(v, "tls_enabled"),
                tls_ciphers: s_ne(v, "tls_ciphers"),
            }
        })
        .collect();
    pools.sort_by(|a, b| by_name(&a.name, &b.name));

    let mut members: Vec<_> = x
        .members
        .iter()
        .flat_map(|(pool_id, list)| list.iter().map(move |v| (pool_id, v)))
        .map(|(pool_id, v)| LbMember {
            id: s(v, "id").unwrap_or_default(),
            name: name_or_id(v),
            pool: named(pool_id.clone(), &pool_names),
            address: s(v, "address").unwrap_or_default(),
            port: i(v, "protocol_port"),
            weight: i(v, "weight"),
            subnet: s_ne(v, "subnet_id").map(|id| named(id, &subnet_names)),
            backup: b(v, "backup"),
            operating_status: status(v, "operating_status"),
            provisioning_status: status(v, "provisioning_status"),
            admin_up: b(v, "admin_state_up"),
            monitor_address: s_ne(v, "monitor_address"),
            monitor_port: i(v, "monitor_port"),
        })
        .collect();
    members.sort_by(|a, b| {
        by_name(&a.pool.name, &b.pool.name)
            .then_with(|| a.address.cmp(&b.address))
            .then(a.port.cmp(&b.port))
    });

    // Monitors are listed per project: keep those of this load balancer's pools.
    let mut monitors: Vec<_> = x
        .monitors
        .iter()
        .filter(|v| ids(v, "pools").iter().any(|p| pool_ids.contains(p)))
        .map(|v| {
            let kind = s(v, "type").unwrap_or_default();
            let http = kind.starts_with("HTTP");
            let http_check = kind.starts_with("HTTP").then(|| {
                let method = s_ne(v, "http_method").unwrap_or_else(|| "GET".into());
                let path = s_ne(v, "url_path").unwrap_or_else(|| "/".into());
                let codes = s_ne(v, "expected_codes").unwrap_or_else(|| "200".into());
                format!("{method} {path} → {codes}")
            });
            LbMonitor {
                id: s(v, "id").unwrap_or_default(),
                name: name_or_id(v),
                kind,
                delay: i(v, "delay"),
                timeout: i(v, "timeout"),
                max_retries: i(v, "max_retries"),
                max_retries_down: i(v, "max_retries_down"),
                http_check,
                http_method: s_ne(v, "http_method").filter(|_| http),
                url_path: s_ne(v, "url_path").filter(|_| http),
                expected_codes: s_ne(v, "expected_codes").filter(|_| http),
                pools: ids(v, "pools").into_iter().map(|id| named(id, &pool_names)).collect(),
                operating_status: status(v, "operating_status"),
                provisioning_status: status(v, "provisioning_status"),
                admin_up: b(v, "admin_state_up"),
            }
        })
        .collect();
    monitors.sort_by(|a, b| by_name(&a.name, &b.name));

    let l7policies = l7_policies(x.l7policies, x.l7rules, &listener_names, &pool_names);
    let vip_port_id = s_ne(l, "vip_port_id");
    let fip = vip_port_id
        .as_ref()
        .and_then(|port| x.floating_ips.iter().find(|f| s(f, "port_id").as_ref() == Some(port)));
    LoadBalancerDetail {
        row: load_balancer_row(l),
        project_id: s(l, "project_id").unwrap_or_default(),
        flavor_id: s_ne(l, "flavor_id"),
        vip_network: s_ne(l, "vip_network_id").map(|id| named(id, &net_names)),
        vip_subnet: s_ne(l, "vip_subnet_id").map(|id| named(id, &subnet_names)),
        floating_ip: fip.and_then(|f| s(f, "floating_ip_address")),
        floating_ip_id: fip.and_then(|f| s(f, "id")),
        vip_port_id,
        created: s_ne(l, "created_at"),
        updated: s_ne(l, "updated_at"),
        listeners,
        pools,
        members,
        monitors,
        l7policies,
    }
}

/// The L7 policies of this load balancer's listeners (`listener_names` holds
/// exactly those), with their rules, by listener and then position.
fn l7_policies(
    policies: &[Value],
    rules: &[(String, Vec<Value>)],
    listener_names: &HashMap<String, String>,
    pool_names: &HashMap<String, String>,
) -> Vec<LbL7Policy> {
    let mut out: Vec<LbL7Policy> = policies
        .iter()
        .filter(|p| s(p, "listener_id").is_some_and(|l| listener_names.contains_key(&l)))
        .map(|p| {
            let id = s(p, "id").unwrap_or_default();
            let mut policy_rules: Vec<LbL7Rule> = rules
                .iter()
                .filter(|(pid, _)| *pid == id)
                .flat_map(|(_, r)| r.iter())
                .map(|r| LbL7Rule {
                    id: s(r, "id").unwrap_or_default(),
                    rule_type: s(r, "type").unwrap_or_default(),
                    compare_type: s(r, "compare_type").unwrap_or_default(),
                    key: s_ne(r, "key"),
                    value: s(r, "value").unwrap_or_default(),
                    invert: b(r, "invert"),
                    operating_status: status(r, "operating_status"),
                    provisioning_status: status(r, "provisioning_status"),
                    admin_up: b(r, "admin_state_up"),
                })
                .collect();
            policy_rules.sort_by(|a, b| (&a.rule_type, &a.value).cmp(&(&b.rule_type, &b.value)));
            LbL7Policy {
                name: name_or_id(p),
                description: s_ne(p, "description"),
                listener: named(s(p, "listener_id").unwrap_or_default(), listener_names),
                action: s(p, "action").unwrap_or_default(),
                position: i(p, "position"),
                redirect_pool: s_ne(p, "redirect_pool_id").map(|id| named(id, pool_names)),
                redirect_url: s_ne(p, "redirect_url"),
                operating_status: status(p, "operating_status"),
                provisioning_status: status(p, "provisioning_status"),
                admin_up: b(p, "admin_state_up"),
                rules: policy_rules,
                id,
            }
        })
        .collect();
    out.sort_by(|a, b| (&a.listener.name, a.position).cmp(&(&b.listener.name, b.position)));
    out
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::*;
    use super::*;
    use serde_json::json;

    // --- Orchestration (roadmap 62) ---

    #[test]
    fn stacks_newest_first_with_name_fallback() {
        let raw = vec![
            json!({"id": "s1", "stack_name": "old", "stack_status": "CREATE_COMPLETE", "stack_status_reason": "Stack CREATE completed successfully",
                   "creation_time": "2026-01-01T10:00:00Z", "updated_time": null, "description": "No description"}),
            json!({"id": "s2", "stack_name": "", "stack_status": "UPDATE_IN_PROGRESS", "creation_time": "2026-02-01T10:00:00Z",
                   "updated_time": "2026-02-02T10:00:00Z", "description": ""}),
        ];
        let out = stacks(&raw);
        assert_eq!(
            out.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["s2", "old"],
            "newest first, unnamed → ID"
        );
        assert_eq!(out[0].updated.as_deref(), Some("2026-02-02T10:00:00Z"));
        assert_eq!(out[1].updated, None, "null updated_time");
        assert_eq!(out[0].description, None, "empty description");
        assert!(stack_busy(&out[0].status) && !stack_busy(&out[1].status));
    }

    /// Roadmap 54: Octavia sends `insert_headers` values as strings; only the
    /// `"true"` ones are set. `allowed_cidrs` is absent before Octavia 2.12.
    #[test]
    fn listener_options_read_headers_timeouts_and_cidrs() {
        let v = serde_json::json!({
            "timeout_client_data": 50000, "timeout_tcp_inspect": 0,
            "insert_headers": {"X-Forwarded-Proto": "true", "X-Forwarded-For": "True", "X-Forwarded-Port": "false"},
            "allowed_cidrs": null
        });
        let o = listener_options(&v);
        assert_eq!(
            (o.timeout_client_data, o.timeout_tcp_inspect, o.timeout_member_data),
            (Some(50_000), Some(0), None)
        );
        assert_eq!(o.insert_headers, vec!["X-Forwarded-For", "X-Forwarded-Proto"]);
        assert_eq!(o.allowed_cidrs, Some(vec![]), "null: offered, none set");
        assert_eq!(
            listener_options(&serde_json::json!({})).allowed_cidrs,
            None,
            "missing: Octavia < 2.12"
        );
    }

    #[test]
    fn load_balancer_list_rows() {
        let (lb, ..) = octavia();
        let unnamed = json!({"id": "lb-0", "name": "", "admin_state_up": false});
        let rows = load_balancers(&[lb, unnamed]);
        assert_eq!(rows[0].name, "lb-0", "unnamed: falls back to the ID");
        assert_eq!(rows[0].operating_status, "UNKNOWN");
        let r = &rows[1];
        assert_eq!(
            (r.vip_address.as_deref(), r.availability_zone.as_deref()),
            (Some("10.0.0.50"), None)
        );
        assert_eq!(
            (r.operating_status.as_str(), r.provisioning_status.as_str()),
            ("DEGRADED", "ACTIVE")
        );
        assert!(r.admin_up && r.description.is_none());
    }

    #[test]
    fn instances_page_keeps_the_apis_order() {
        let servers = vec![
            json!({"id": "old", "name": "z", "created": "2020-01-01T00:00:00Z", "status": "ACTIVE"}),
            json!({"id": "new", "name": "a", "created": "2026-01-01T00:00:00Z", "status": "ACTIVE"}),
        ];
        let ids: Vec<_> = instances_page(&servers, &[]).into_iter().map(|i| i.id).collect();
        assert_eq!(
            ids,
            ["old", "new"],
            "the next-page marker is the last row, so the order is the API's, not newest-first"
        );
    }
}
