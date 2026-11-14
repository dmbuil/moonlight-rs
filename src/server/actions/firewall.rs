//! Neutron FWaaS v2 writes: rules, policies, groups.

use super::*;

/// A Neutron FWaaS v2 request (`fwaas/{path}`).
fn fwaas(method: http::Method, path: String, body: Option<Value>) -> JsonRequest {
    JsonRequest::new(
        method,
        openstack_sdk::types::ServiceType::Network,
        format!("fwaas/{path}"),
        body,
    )
}

/// `OsAction::FirewallRuleCreate`: `POST /v2.0/fwaas/firewall_rules` with
/// `FirewallRuleSpec::body`.
///
/// None of the FWaaS writes are in the SDK (`openstack-sdk-network` has no `fwaas` module),
/// so all of them are JSON requests built with [`fwaas`].
pub(super) async fn firewall_rule_create(x: &mut Exec<'_>, spec: &FirewallRuleSpec) -> R<()> {
    x.create(
        fwaas(
            http::Method::POST,
            "firewall_rules".into(),
            Some(spec.body().map_err(AppError::invalid)?),
        ),
        "firewall rule",
    )
    .await?;
    Ok(())
}

/// `OsAction::FirewallRuleUpdate`: `PUT /v2.0/fwaas/firewall_rules/{id}` with the whole rule
/// body, as for create.
pub(super) async fn firewall_rule_update(x: &mut Exec<'_>, id: &str, spec: &FirewallRuleSpec) -> R<()> {
    let path = format!("firewall_rules/{}", non_empty(id, "firewall rule")?);
    x.send(fwaas(
        http::Method::PUT,
        path,
        Some(spec.body().map_err(AppError::invalid)?),
    ))
    .await?;
    Ok(())
}

/// `OsAction::FirewallRuleDelete`: `DELETE /v2.0/fwaas/firewall_rules/{id}`.
pub(super) async fn firewall_rule_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    x.send(fwaas(
        http::Method::DELETE,
        format!("firewall_rules/{}", non_empty(id, "firewall rule")?),
        None,
    ))
    .await?;
    Ok(())
}

/// `OsAction::FirewallPolicyCreate`: `POST /v2.0/fwaas/firewall_policies` with its ordered
/// rules.
pub(super) async fn firewall_policy_create(
    x: &mut Exec<'_>,
    name: &str,
    description: &str,
    rule_ids: &[String],
    audited: &bool,
) -> R<()> {
    let rules = rule_ids
        .iter()
        .map(|r| non_empty(r, "firewall rule"))
        .collect::<R<Vec<_>>>()?;
    let mut p = serde_json::Map::new();
    p.insert("name".into(), json!(non_empty(name, "name")?));
    p.insert("description".into(), json!(description.trim()));
    p.insert("firewall_rules".into(), json!(rules));
    p.insert("audited".into(), json!(audited));
    x.create(
        fwaas(
            http::Method::POST,
            "firewall_policies".into(),
            Some(json!({ "firewall_policy": p })),
        ),
        "firewall policy",
    )
    .await?;
    Ok(())
}

/// `OsAction::FirewallPolicyUpdate`: `PUT /v2.0/fwaas/firewall_policies/{id}` with name,
/// description and `audited`. The rules are changed with `insert_rule` / `remove_rule`, not
/// here.
pub(super) async fn firewall_policy_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    description: &str,
    audited: &bool,
) -> R<()> {
    let mut p = serde_json::Map::new();
    p.insert("name".into(), json!(non_empty(name, "name")?));
    p.insert("description".into(), json!(description.trim()));
    p.insert("audited".into(), json!(audited));
    let path = format!("firewall_policies/{}", non_empty(id, "firewall policy")?);
    x.send(fwaas(http::Method::PUT, path, Some(json!({ "firewall_policy": p }))))
        .await?;
    Ok(())
}

/// `OsAction::FirewallPolicyDelete`: `DELETE /v2.0/fwaas/firewall_policies/{id}`.
pub(super) async fn firewall_policy_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    x.send(fwaas(
        http::Method::DELETE,
        format!("firewall_policies/{}", non_empty(id, "firewall policy")?),
        None,
    ))
    .await?;
    Ok(())
}

/// `OsAction::FirewallPolicyInsertRule`: `PUT
/// /v2.0/fwaas/firewall_policies/{id}/insert_rule`, before or after another rule (at most one;
/// neither puts it first).
pub(super) async fn firewall_policy_insert_rule(
    x: &mut Exec<'_>,
    id: &str,
    rule_id: &str,
    before_id: &Option<String>,
    after_id: &Option<String>,
) -> R<()> {
    // Neutron puts the rule first when neither is given; both is ambiguous.
    if opt(before_id).is_some() && opt(after_id).is_some() {
        return Err(AppError::invalid(
            "insert the rule either before or after another one, not both",
        ));
    }
    let mut b = serde_json::Map::new();
    b.insert("firewall_rule_id".into(), json!(non_empty(rule_id, "firewall rule")?));
    if let Some(r) = opt(before_id) {
        b.insert("insert_before".into(), json!(r));
    }
    if let Some(r) = opt(after_id) {
        b.insert("insert_after".into(), json!(r));
    }
    let path = format!("firewall_policies/{}/insert_rule", non_empty(id, "firewall policy")?);
    x.send(fwaas(http::Method::PUT, path, Some(Value::Object(b)))).await?;
    Ok(())
}

/// `OsAction::FirewallPolicyRemoveRule`: `PUT
/// /v2.0/fwaas/firewall_policies/{id}/remove_rule`; the rule itself is kept.
pub(super) async fn firewall_policy_remove_rule(x: &mut Exec<'_>, id: &str, rule_id: &str) -> R<()> {
    let path = format!("firewall_policies/{}/remove_rule", non_empty(id, "firewall policy")?);
    x.send(fwaas(
        http::Method::PUT,
        path,
        Some(json!({ "firewall_rule_id": non_empty(rule_id, "firewall rule")? })),
    ))
    .await?;
    Ok(())
}

/// `OsAction::FirewallGroupCreate`: `POST /v2.0/fwaas/firewall_groups` with its policies (each
/// optional) and ports.
pub(super) async fn firewall_group_create(
    x: &mut Exec<'_>,
    name: &str,
    description: &str,
    ingress_policy_id: &Option<String>,
    egress_policy_id: &Option<String>,
    port_ids: &[String],
    admin_up: &bool,
) -> R<()> {
    let ports = port_ids.iter().map(|p| non_empty(p, "port")).collect::<R<Vec<_>>>()?;
    let mut g = serde_json::Map::new();
    g.insert("name".into(), json!(name.trim()));
    g.insert("description".into(), json!(description.trim()));
    if let Some(p) = opt(ingress_policy_id) {
        g.insert("ingress_firewall_policy_id".into(), json!(p));
    }
    if let Some(p) = opt(egress_policy_id) {
        g.insert("egress_firewall_policy_id".into(), json!(p));
    }
    g.insert("ports".into(), json!(ports));
    g.insert("admin_state_up".into(), json!(admin_up));
    x.create(
        fwaas(
            http::Method::POST,
            "firewall_groups".into(),
            Some(json!({ "firewall_group": g })),
        ),
        "firewall group",
    )
    .await?;
    Ok(())
}

/// `OsAction::FirewallGroupUpdate`: `PUT /v2.0/fwaas/firewall_groups/{id}`. Both policies
/// are always sent, so `None` sends `null` and stops filtering in that direction. Ports change
/// with `firewall_group_set_ports`.
pub(super) async fn firewall_group_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    description: &str,
    ingress_policy_id: &Option<String>,
    egress_policy_id: &Option<String>,
    admin_up: &bool,
) -> R<()> {
    // `null` policies remove the filtering in that direction.
    let mut g = serde_json::Map::new();
    g.insert("name".into(), json!(name.trim()));
    g.insert("description".into(), json!(description.trim()));
    g.insert("ingress_firewall_policy_id".into(), json!(opt(ingress_policy_id)));
    g.insert("egress_firewall_policy_id".into(), json!(opt(egress_policy_id)));
    g.insert("admin_state_up".into(), json!(admin_up));
    let path = format!("firewall_groups/{}", non_empty(id, "firewall group")?);
    x.send(fwaas(http::Method::PUT, path, Some(json!({ "firewall_group": g }))))
        .await?;
    Ok(())
}

/// `OsAction::FirewallGroupSetPorts`: `PUT /v2.0/fwaas/firewall_groups/{id}` with the full
/// `ports` list (Neutron replaces it).
pub(super) async fn firewall_group_set_ports(x: &mut Exec<'_>, id: &str, port_ids: &[String]) -> R<()> {
    // Neutron replaces the whole list: an empty one leaves the group INACTIVE.
    let ports = port_ids.iter().map(|p| non_empty(p, "port")).collect::<R<Vec<_>>>()?;
    let path = format!("firewall_groups/{}", non_empty(id, "firewall group")?);
    x.send(fwaas(
        http::Method::PUT,
        path,
        Some(json!({ "firewall_group": { "ports": ports } })),
    ))
    .await?;
    Ok(())
}

/// `OsAction::FirewallGroupDelete`: `DELETE /v2.0/fwaas/firewall_groups/{id}`.
pub(super) async fn firewall_group_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    x.send(fwaas(
        http::Method::DELETE,
        format!("firewall_groups/{}", non_empty(id, "firewall group")?),
        None,
    ))
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[tokio::test]
    async fn firewall_rules_policies_and_groups() {
        let ssh = FirewallRuleSpec {
            name: id(" ssh "),
            protocol: id("tcp"),
            action: id("allow"),
            ip_version: 4,
            destination_ip: id("10.0.0.0/24"),
            destination_port: id("22"),
            enabled: true,
            ..Default::default()
        };
        let r = dry(OsAction::FirewallRuleCreate(ssh.clone())).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
            ("POST", "/fwaas/firewall_rules", "network")
        );
        assert_eq!(
            body(&r[0]),
            json!({"firewall_rule": {"name": "ssh", "description": "", "protocol": "tcp", "action": "allow", "ip_version": 4,
                                     "source_ip_address": null, "source_port": null, "destination_ip_address": "10.0.0.0/24",
                                     "destination_port": "22", "enabled": true}}),
            "no `shared`: the Project panel never sends it (Admin only)"
        );
        let any = FirewallRuleSpec {
            protocol: id("any"),
            destination_port: String::new(),
            ..ssh.clone()
        };
        let r = dry(OsAction::FirewallRuleUpdate {
            id: id("r1"),
            spec: any,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/fwaas/firewall_rules/r1")
        );
        assert_eq!(body(&r[0])["firewall_rule"]["protocol"], Value::Null, "any: null");
        assert!(
            dry_err(OsAction::FirewallRuleCreate(FirewallRuleSpec {
                protocol: id("icmp"),
                ..ssh.clone()
            }))
            .await,
            "no ports on ICMP"
        );
        assert!(
            dry_err(OsAction::FirewallRuleCreate(FirewallRuleSpec {
                destination_port: id("90:80"),
                ..ssh.clone()
            }))
            .await,
            "range low to high"
        );
        assert!(
            dry_err(OsAction::FirewallRuleCreate(FirewallRuleSpec {
                destination_ip: id("fd00::/64"),
                ..ssh.clone()
            }))
            .await,
            "IPv6 CIDR on an IPv4 rule"
        );
        assert!(
            dry_err(OsAction::FirewallRuleCreate(FirewallRuleSpec {
                action: id("drop"),
                ..ssh.clone()
            }))
            .await
        );

        let r = dry(OsAction::FirewallPolicyCreate {
            name: id("web"),
            description: id(" front "),
            rule_ids: vec![id("r2"), id("r1")],
            audited: false,
        })
        .await;
        assert_eq!(r[0].path, "/fwaas/firewall_policies");
        assert_eq!(
            body(&r[0]),
            json!({"firewall_policy": {"name": "web", "description": "front", "firewall_rules": ["r2", "r1"], "audited": false}}),
            "rules in order"
        );
        let r = dry(OsAction::FirewallPolicyUpdate {
            id: id("fp1"),
            name: id("web"),
            description: String::new(),
            audited: true,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/fwaas/firewall_policies/fp1")
        );
        assert_eq!(
            body(&r[0]),
            json!({"firewall_policy": {"name": "web", "description": "", "audited": true}})
        );
        let r = dry(OsAction::FirewallPolicyInsertRule {
            id: id("fp1"),
            rule_id: id("r3"),
            before_id: None,
            after_id: Some(id("r1")),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/fwaas/firewall_policies/fp1/insert_rule")
        );
        assert_eq!(body(&r[0]), json!({"firewall_rule_id": "r3", "insert_after": "r1"}));
        let r = dry(OsAction::FirewallPolicyInsertRule {
            id: id("fp1"),
            rule_id: id("r3"),
            before_id: None,
            after_id: None,
        })
        .await;
        assert_eq!(body(&r[0]), json!({"firewall_rule_id": "r3"}), "neither: first");
        assert!(
            dry_err(OsAction::FirewallPolicyInsertRule {
                id: id("fp1"),
                rule_id: id("r3"),
                before_id: Some(id("r1")),
                after_id: Some(id("r2"))
            })
            .await
        );
        let r = dry(OsAction::FirewallPolicyRemoveRule {
            id: id("fp1"),
            rule_id: id("r3"),
        })
        .await;
        assert_eq!(
            (r[0].path.as_str(), body(&r[0])),
            (
                "/fwaas/firewall_policies/fp1/remove_rule",
                json!({"firewall_rule_id": "r3"})
            )
        );

        let r = dry(OsAction::FirewallGroupCreate {
            name: id("gw"),
            description: String::new(),
            ingress_policy_id: Some(id("fp1")),
            egress_policy_id: None,
            port_ids: vec![id("p1")],
            admin_up: true,
        })
        .await;
        assert_eq!(r[0].path, "/fwaas/firewall_groups");
        assert_eq!(
            body(&r[0]),
            json!({"firewall_group": {"name": "gw", "description": "", "ingress_firewall_policy_id": "fp1", "ports": ["p1"], "admin_state_up": true}})
        );
        let r = dry(OsAction::FirewallGroupUpdate {
            id: id("g1"),
            name: id("gw"),
            description: String::new(),
            ingress_policy_id: None,
            egress_policy_id: Some(id("fp2")),
            admin_up: false,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/fwaas/firewall_groups/g1")
        );
        assert_eq!(
            body(&r[0]),
            json!({"firewall_group": {"name": "gw", "description": "", "ingress_firewall_policy_id": null, "egress_firewall_policy_id": "fp2", "admin_state_up": false}}),
            "no policy: null clears it"
        );
        let r = dry(OsAction::FirewallGroupSetPorts {
            id: id("g1"),
            port_ids: vec![],
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"firewall_group": {"ports": []}}),
            "the whole list; empty removes every port"
        );

        for (a, path) in [
            (OsAction::FirewallRuleDelete { id: id("x") }, "/fwaas/firewall_rules/x"),
            (
                OsAction::FirewallPolicyDelete { id: id("x") },
                "/fwaas/firewall_policies/x",
            ),
            (
                OsAction::FirewallGroupDelete { id: id("x") },
                "/fwaas/firewall_groups/x",
            ),
        ] {
            let r = dry(a).await;
            assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", path));
        }
    }
}
