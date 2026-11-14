//! Octavia L7 policies and rules.

use super::*;

/// `OsAction::L7PolicyCreate`: `POST /v2/lbaas/l7policies` on a listener, with the redirect
/// target (pool or URL) its action needs. Checked by `LbL7PolicySpec::check`.
pub(super) async fn l7_policy_create(x: &mut Exec<'_>, listener_id: &str, spec: &LbL7PolicySpec) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::l7policy::create as m;
    spec.check().map_err(AppError::invalid)?;
    let mut b = m::L7policyBuilder::default();
    b.listener_id(non_empty(listener_id, "listener")?)
        .action(enum_of::<m::Action>(&spec.action, "L7 action")?)
        .admin_state_up(spec.admin_up);
    if let Some(v) = opt(&spec.name) {
        b.name(v);
    }
    if let Some(v) = opt(&spec.description) {
        b.description(v);
    }
    if let Some(p) = spec.position {
        b.position(p as i32);
    }
    match spec.targets() {
        (Some(pool), _) => {
            b.redirect_pool_id(pool);
        }
        (_, Some(url)) => {
            b.redirect_url(url);
        }
        _ => {}
    }
    x.create(
        m::Request::builder()
            .l7policy(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "L7 policy",
    )
    .await?;
    Ok(())
}

/// `OsAction::L7PolicyUpdate`: `PUT /v2/lbaas/l7policies/{id}`, as JSON so the redirect target
/// the action doesn't use is sent as `null` (the SDK builder can't).
pub(super) async fn l7_policy_update(x: &mut Exec<'_>, id: &str, spec: &LbL7PolicySpec) -> R<()> {
    spec.check().map_err(AppError::invalid)?;
    let (pool, url) = spec.targets();
    let mut p = json!({
        "name": opt(&spec.name).unwrap_or(""),
        "description": opt(&spec.description).unwrap_or(""),
        "action": spec.action,
        "redirect_pool_id": pool,
        "redirect_url": url,
        "admin_state_up": spec.admin_up,
    });
    // Octavia has no "last": an emptied position keeps the current one.
    if let Some(n) = spec.position {
        p["position"] = json!(n);
    }
    x.send(octavia(
        http::Method::PUT,
        format!("lbaas/l7policies/{}", non_empty(id, "L7 policy")?),
        json!({"l7policy": p}),
    ))
    .await?;
    Ok(())
}

/// `OsAction::L7PolicyDelete`: `DELETE /v2/lbaas/l7policies/{id}` (its rules go with it).
pub(super) async fn l7_policy_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::l7policy::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `OsAction::L7RuleCreate`: `POST /v2/lbaas/l7policies/{policy_id}/rules`. Checked by
/// `LbL7RuleSpec::check` (a key only for the header and cookie types).
pub(super) async fn l7_rule_create(x: &mut Exec<'_>, policy_id: &str, spec: &LbL7RuleSpec) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::l7policy::rule::create as m;
    spec.check().map_err(AppError::invalid)?;
    let mut b = m::RuleBuilder::default();
    b._type(enum_of::<m::Type>(&spec.rule_type, "rule type")?)
        .compare_type(enum_of::<m::CompareType>(&spec.compare_type, "compare type")?)
        .value(spec.value.trim())
        .invert(spec.invert)
        .admin_state_up(spec.admin_up);
    if let Some(k) = opt(&spec.key) {
        b.key(k);
    }
    let ep = m::Request::builder()
        .l7policy_id(non_empty(policy_id, "L7 policy")?)
        .rule(b.build().map_err(build_err)?)
        .build()
        .map_err(build_err)?;
    x.create(ep, "L7 rule").await?;
    Ok(())
}

/// `OsAction::L7RuleUpdate`: `PUT /v2/lbaas/l7policies/{policy_id}/rules/{id}`, as JSON so a
/// removed key is sent as `null`.
pub(super) async fn l7_rule_update(x: &mut Exec<'_>, policy_id: &str, id: &str, spec: &LbL7RuleSpec) -> R<()> {
    spec.check().map_err(AppError::invalid)?;
    let body = json!({"rule": {
        "type": spec.rule_type,
        "compare_type": spec.compare_type,
        "key": opt(&spec.key),
        "value": spec.value.trim(),
        "invert": spec.invert,
        "admin_state_up": spec.admin_up,
    }});
    let path = format!(
        "lbaas/l7policies/{}/rules/{}",
        non_empty(policy_id, "L7 policy")?,
        non_empty(id, "L7 rule")?
    );
    x.send(octavia(http::Method::PUT, path, body)).await?;
    Ok(())
}

/// `OsAction::L7RuleDelete`: `DELETE /v2/lbaas/l7policies/{policy_id}/rules/{id}`.
pub(super) async fn l7_rule_delete(x: &mut Exec<'_>, policy_id: &str, id: &str) -> R<()> {
    use openstack_sdk::api::load_balancer::v2::l7policy::rule::delete as m;
    x.send(
        m::Request::builder()
            .l7policy_id(policy_id)
            .id(id)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    /// L7 policies and rules, created with the SDK and
    /// saved as JSON so the unused redirect target and a removed key are `null`.
    #[tokio::test]
    async fn l7_policies_and_rules() {
        let to_pool = LbL7PolicySpec {
            name: Some(id("api")),
            description: None,
            action: id("REDIRECT_TO_POOL"),
            position: Some(1),
            redirect_pool_id: Some(id("pool1")),
            redirect_url: Some(id("https://ignored.example")),
            admin_up: true,
        };
        let r = dry(OsAction::L7PolicyCreate {
            listener_id: id("ls1"),
            spec: to_pool.clone(),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("POST", "/lbaas/l7policies")
        );
        assert_eq!(
            body(&r[0]),
            json!({"l7policy": {"listener_id": "ls1", "name": "api", "action": "REDIRECT_TO_POOL", "position": 1, "redirect_pool_id": "pool1", "admin_state_up": true}}),
            "only the target of the action"
        );
        let to_url = LbL7PolicySpec {
            action: id("REDIRECT_TO_URL"),
            position: None,
            redirect_url: Some(id("https://www.example.com")),
            ..to_pool.clone()
        };
        let r = dry(OsAction::L7PolicyUpdate {
            id: id("pol1"),
            spec: to_url,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/lbaas/l7policies/pol1")
        );
        let p = &body(&r[0])["l7policy"];
        assert_eq!(
            (
                p["redirect_pool_id"].clone(),
                p["redirect_url"].clone(),
                p.get("position")
            ),
            (Value::Null, json!("https://www.example.com"), None)
        );
        let r = dry(OsAction::L7PolicyDelete { id: id("pol1") }).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/lbaas/l7policies/pol1")
        );

        let header = LbL7RuleSpec {
            rule_type: id("HEADER"),
            compare_type: id("EQUAL_TO"),
            key: Some(id("X-Env")),
            value: id(" test "),
            invert: true,
            admin_up: true,
        };
        let r = dry(OsAction::L7RuleCreate {
            policy_id: id("pol1"),
            spec: header.clone(),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("POST", "/lbaas/l7policies/pol1/rules")
        );
        assert_eq!(
            body(&r[0]),
            json!({"rule": {"type": "HEADER", "compare_type": "EQUAL_TO", "key": "X-Env", "value": "test", "invert": true, "admin_state_up": true}})
        );
        let path = LbL7RuleSpec {
            rule_type: id("PATH"),
            compare_type: id("STARTS_WITH"),
            key: None,
            value: id("/api"),
            invert: false,
            admin_up: true,
        };
        let r = dry(OsAction::L7RuleUpdate {
            policy_id: id("pol1"),
            id: id("r1"),
            spec: path,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/lbaas/l7policies/pol1/rules/r1")
        );
        assert_eq!(body(&r[0])["rule"]["key"], Value::Null, "a PATH rule has no key");
        let r = dry(OsAction::L7RuleDelete {
            policy_id: id("pol1"),
            id: id("r1"),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/lbaas/l7policies/pol1/rules/r1")
        );

        let bad = |a: OsAction| async move { apply(&mut Exec::new(WriteMode::DryRun, None), &a).await.is_err() };
        assert!(
            bad(OsAction::L7PolicyCreate {
                listener_id: id("ls1"),
                spec: LbL7PolicySpec {
                    redirect_pool_id: None,
                    ..to_pool.clone()
                }
            })
            .await,
            "a pool redirect needs the pool"
        );
        assert!(
            bad(OsAction::L7RuleCreate {
                policy_id: id("pol1"),
                spec: LbL7RuleSpec {
                    key: None,
                    ..header.clone()
                }
            })
            .await,
            "a header rule needs its name"
        );
        assert!(
            bad(OsAction::L7RuleCreate {
                policy_id: id("pol1"),
                spec: LbL7RuleSpec {
                    rule_type: id("FILE_TYPE"),
                    key: None,
                    compare_type: id("STARTS_WITH"),
                    ..header
                }
            })
            .await
        );
    }
}
