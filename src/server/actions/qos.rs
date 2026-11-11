//! Neutron QoS writes: policies and rules.

use super::*;

pub(super) async fn qos_policy_create(
    x: &mut Exec<'_>,
    name: &str,
    description: &Option<String>,
    shared: &bool,
) -> R<()> {
    use openstack_sdk::api::network::v2::qos::policy::create as m;
    let mut b = m::PolicyBuilder::default();
    b.name(non_empty(name, "name")?).shared(*shared);
    if let Some(d) = opt(description) {
        b.description(d);
    }
    x.create(
        m::Request::builder()
            .policy(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "QoS policy",
    )
    .await?;
    Ok(())
}

pub(super) async fn qos_policy_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    description: &str,
    shared: &bool,
) -> R<()> {
    use openstack_sdk::api::network::v2::qos::policy::set as m;
    let body = m::PolicyBuilder::default()
        .name(non_empty(name, "name")?)
        .description(description.trim())
        .shared(*shared)
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).policy(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

pub(super) async fn qos_policy_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::network::v2::qos::policy::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// Rules: one JSON path for the four types (`…/<type>_rules`), the body
/// built and checked by `QosRuleSpec::body`.
pub(super) async fn qos_rule_create(x: &mut Exec<'_>, policy_id: &str, spec: &QosRuleSpec) -> R<()> {
    let body = spec.body().map_err(AppError::invalid)?;
    let path = format!("qos/policies/{}/{}_rules", non_empty(policy_id, "policy")?, spec.kind);
    let ep = JsonRequest::new(
        http::Method::POST,
        openstack_sdk::types::ServiceType::Network,
        path,
        Some(body),
    );
    x.create(ep, "QoS rule").await?;
    Ok(())
}

pub(super) async fn qos_rule_update(x: &mut Exec<'_>, policy_id: &str, id: &str, spec: &QosRuleSpec) -> R<()> {
    let body = spec.body().map_err(AppError::invalid)?;
    let path = format!(
        "qos/policies/{}/{}_rules/{}",
        non_empty(policy_id, "policy")?,
        spec.kind,
        non_empty(id, "rule")?
    );
    x.send(JsonRequest::new(
        http::Method::PUT,
        openstack_sdk::types::ServiceType::Network,
        path,
        Some(body),
    ))
    .await?;
    Ok(())
}

pub(super) async fn qos_rule_delete(x: &mut Exec<'_>, policy_id: &str, kind: &str, id: &str) -> R<()> {
    if !QOS_RULE_TYPES.contains(&kind) {
        return Err(AppError::invalid(format!("unknown rule type {kind}")));
    }
    let path = format!(
        "qos/policies/{}/{kind}_rules/{}",
        non_empty(policy_id, "policy")?,
        non_empty(id, "rule")?
    );
    x.send(JsonRequest::new(
        http::Method::DELETE,
        openstack_sdk::types::ServiceType::Network,
        path,
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
    async fn qos_policies_and_rules() {
        let r = dry(OsAction::QosPolicyCreate {
            name: id("gold"),
            description: Some(id("10G")),
            shared: false,
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/qos/policies"));
        assert_eq!(
            body(&r[0]),
            json!({"policy": {"name": "gold", "description": "10G", "shared": false}})
        );
        let r = dry(OsAction::QosPolicyUpdate {
            id: id("q1"),
            name: id("gold"),
            description: String::new(),
            shared: true,
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/qos/policies/q1"));
        let spec = QosRuleSpec {
            kind: id("bandwidth_limit"),
            direction: Some(id("egress")),
            max_kbps: Some(10_000),
            ..Default::default()
        };
        let r = dry(OsAction::QosRuleCreate {
            policy_id: id("q1"),
            spec: spec.clone(),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("POST", "/qos/policies/q1/bandwidth_limit_rules")
        );
        assert_eq!(
            body(&r[0]),
            json!({"bandwidth_limit_rule": {"direction": "egress", "max_kbps": 10000}})
        );
        let r = dry(OsAction::QosRuleUpdate {
            policy_id: id("q1"),
            id: id("r1"),
            spec,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/qos/policies/q1/bandwidth_limit_rules/r1")
        );
        let r = dry(OsAction::QosRuleDelete {
            policy_id: id("q1"),
            kind: id("dscp_marking"),
            id: id("r2"),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/qos/policies/q1/dscp_marking_rules/r2")
        );
        assert!(
            dry_err(OsAction::QosRuleDelete {
                policy_id: id("q1"),
                kind: id("../x"),
                id: id("r")
            })
            .await,
            "no path tricks"
        );
        let r = dry(OsAction::QosPolicyDelete { id: id("q1") }).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/qos/policies/q1")
        );
    }
}
