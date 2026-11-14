//! Admin writes: quotas, metadata definitions, flavors, aggregates, compute services, instance migration.

use super::*;

/// *Edit Quotas*: the changed limits of one service. JSON, not
/// the typed `quota_set::set_*` builders: their fields are fixed per release,
/// and only `values` (each a key of `service.keys()`) is sent. Nova gets no
/// microversion, so the keys 2.57 dropped (`injected_files`…) are accepted.
pub(super) fn quota_update(
    service: QuotaService,
    project_id: &str,
    values: &std::collections::BTreeMap<String, i64>,
) -> R<JsonRequest> {
    use openstack_sdk::types::ServiceType;
    let project = non_empty(project_id, "project")?;
    if values.is_empty() {
        return Err(AppError::invalid("no quota changed"));
    }
    if let Some((k, v)) = values
        .iter()
        .find(|(k, v)| !service.keys().contains(&k.as_str()) || **v < -1)
    {
        return Err(AppError::invalid(format!("quota {k} = {v} is not allowed")));
    }
    let (ty, path, wrap) = match service {
        QuotaService::Compute => (ServiceType::Compute, format!("os-quota-sets/{project}"), "quota_set"),
        QuotaService::Volume => (
            ServiceType::BlockStorage,
            format!("os-quota-sets/{project}"),
            "quota_set",
        ),
        QuotaService::Network => (ServiceType::Network, format!("quotas/{project}"), "quota"),
        QuotaService::LoadBalancer => (ServiceType::LoadBalancer, format!("lbaas/quotas/{project}"), "quota"),
        // Barbican updates only the keys sent; the others keep their value,
        // or the cloud's default when the project had none configured.
        QuotaService::KeyManager => {
            return Ok(JsonRequest::key_manager(
                http::Method::PUT,
                format!("project-quotas/{project}"),
                Some(json!({ "project_quotas": values })),
            ));
        }
    };
    Ok(JsonRequest::new(
        http::Method::PUT,
        ty,
        path,
        Some(json!({ wrap: values })),
    ))
}

/// *Update Defaults*: the changed limits of the `default` quota class, keys
/// checked as in [`quota_update`]. Only Nova and Cinder have quota classes.
pub(super) fn default_quota_update(
    service: QuotaService,
    values: &std::collections::BTreeMap<String, i64>,
) -> R<JsonRequest> {
    use openstack_sdk::types::ServiceType;
    let ty = match service {
        QuotaService::Compute => ServiceType::Compute,
        QuotaService::Volume => ServiceType::BlockStorage,
        other => return Err(AppError::invalid(format!("{other:?} has no default quota class"))),
    };
    if values.is_empty() {
        return Err(AppError::invalid("no quota changed"));
    }
    if let Some((k, v)) = values
        .iter()
        .find(|(k, v)| !service.keys().contains(&k.as_str()) || **v < -1)
    {
        return Err(AppError::invalid(format!("quota {k} = {v} is not allowed")));
    }
    Ok(JsonRequest::new(
        http::Method::PUT,
        ty,
        "os-quota-class-sets/default",
        Some(json!({ "quota_class_set": values })),
    ))
}

/// A Glance metadata definitions request. Raw: the SDK's namespace builders
/// have fixed fields, so an imported definition's properties and objects
/// would be lost.
fn metadef(method: http::Method, path: String, body: Option<Value>) -> JsonRequest {
    JsonRequest::new(method, openstack_sdk::types::ServiceType::Image, path, body)
}

/// *Import Namespace*'s body: `definition` must be a JSON object with a
/// `namespace` (a valid path segment, as it names the namespace afterwards);
/// the form's *Public* and *Protected* replace whatever it says.
fn metadef_definition(definition: &str, public: bool, protected: bool) -> R<Value> {
    let mut v: Value = serde_json::from_str(definition)
        .map_err(|e| AppError::invalid(format!("the definition is not valid JSON: {e}")))?;
    let Some(obj) = v.as_object_mut() else {
        return Err(AppError::invalid("the definition must be a JSON object"));
    };
    let ns = obj.get("namespace").and_then(Value::as_str).unwrap_or("").to_string();
    crate::model::check_id(&ns).map_err(|_| AppError::invalid("the definition needs a \"namespace\" name"))?;
    obj.insert("visibility".into(), json!(if public { "public" } else { "private" }));
    obj.insert("protected".into(), json!(protected));
    Ok(v)
}

/// A Nova `servers/{id}/action` request, optionally at a microversion
/// (`"compute 2.30"`).
fn nova_action(id: &str, body: Value, microversion: Option<&'static str>) -> R<JsonRequest> {
    let mut r = JsonRequest::new(
        http::Method::POST,
        openstack_sdk::types::ServiceType::Compute,
        format!("servers/{}/action", non_empty(id, "instance")?),
        Some(body),
    );
    if let Some(mv) = microversion {
        let mut h = http::HeaderMap::new();
        // Headers replace the SDK's defaults, so `Accept` is set again.
        h.insert(http::header::ACCEPT, http::HeaderValue::from_static("application/json"));
        h.insert(
            http::HeaderName::from_static("openstack-api-version"),
            http::HeaderValue::from_static(mv),
        );
        r.headers = Some(h);
    }
    Ok(r)
}

/// *Create Flavor*'s `POST /flavors`, its values checked (Horizon: a flavor
/// given to projects is private; `auto` or no ID lets Nova pick one).
fn flavor_request(spec: &FlavorSpec) -> R<openstack_sdk::api::compute::v2::flavor::create_21::Request<'_>> {
    use openstack_sdk::api::compute::v2::flavor::create_21 as m;
    let (vcpus, ram, disk) = (
        positive(spec.vcpus, "VCPUs")?,
        positive(spec.ram_mb, "RAM")?,
        at_least_zero(spec.disk_gb, "root disk")?,
    );
    let mut b = m::FlavorBuilder::default();
    b.name(non_empty(&spec.name, "name")?)
        .vcpus(vcpus)
        .ram(ram)
        .disk(disk)
        .os_flv_ext_data_ephemeral(at_least_zero(spec.ephemeral_gb, "ephemeral disk")?)
        .swap(at_least_zero(spec.swap_mb, "swap disk")?)
        .os_flavor_access_is_public(spec.projects.is_empty());
    if let Some(id) = opt(&spec.flavor_id).filter(|i| !i.eq_ignore_ascii_case("auto")) {
        b.id(Some(Cow::from(id.to_string())));
    }
    if let Some(f) = spec.rxtx_factor {
        if f.is_nan() || f <= 0.0 {
            return Err(AppError::invalid("RX/TX factor must be positive"));
        }
        b.rxtx_factor(f.to_string());
    }
    m::Request::builder()
        .flavor(b.build().map_err(build_err)?)
        .build()
        .map_err(build_err)
}

/// `addTenantAccess` / `removeTenantAccess` of a flavor, one action per project.
async fn flavor_access(x: &mut Exec<'_>, id: &str, add: &[String], remove: &[String]) -> R<()> {
    use openstack_sdk::api::compute::v2::flavor::{add_tenant_access as a, remove_tenant_access as r};
    for p in add {
        let body = a::AddTenantAccessBuilder::default()
            .tenant(non_empty(p, "project")?)
            .build()
            .map_err(build_err)?;
        x.send(
            a::Request::builder()
                .id(id)
                .add_tenant_access(body)
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    for p in remove {
        let body = r::RemoveTenantAccessBuilder::default()
            .tenant(non_empty(p, "project")?)
            .build()
            .map_err(build_err)?;
        x.send(
            r::Request::builder()
                .id(id)
                .remove_tenant_access(body)
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    Ok(())
}

/// `add_host` / `remove_host` of an aggregate, one action per host.
async fn aggregate_hosts(x: &mut Exec<'_>, id: &str, add: &[String], remove: &[String]) -> R<()> {
    use openstack_sdk::api::compute::v2::aggregate::{add_host as a, remove_host as r};
    for h in add {
        let body = a::AddHostBuilder::default()
            .host(non_empty(h, "host")?)
            .build()
            .map_err(build_err)?;
        x.send(a::Request::builder().id(id).add_host(body).build().map_err(build_err)?)
            .await?;
    }
    for h in remove {
        let body = r::RemoveHostBuilder::default()
            .host(non_empty(h, "host")?)
            .build()
            .map_err(build_err)?;
        x.send(
            r::Request::builder()
                .id(id)
                .remove_host(body)
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    Ok(())
}

/// A size that may be 0 (disks, swap), as Nova's `int32`.
fn at_least_zero(n: i64, field: &str) -> R<i32> {
    i32::try_from(n)
        .ok()
        .filter(|n| *n >= 0)
        .ok_or_else(|| AppError::invalid(format!("{field} can't be negative")))
}

/// `MetadefNamespaceImport`: Glance `POST /metadefs/namespaces` with the whole definition
/// (properties, objects, resource type associations), as Horizon's *Import Namespace*.
pub(super) async fn metadef_namespace_import(
    x: &mut Exec<'_>,
    definition: &str,
    public: &bool,
    protected: &bool,
) -> R<()> {
    let body = metadef_definition(definition, *public, *protected)?;
    x.send(metadef(http::Method::POST, "metadefs/namespaces".into(), Some(body)))
        .await?;
    Ok(())
}

/// `MetadefNamespaceUpdate`: Glance `PUT /metadefs/namespaces/{id}`. The body repeats
/// `namespace` (Glance requires it); unset display name and description are left out.
pub(super) async fn metadef_namespace_update(
    x: &mut Exec<'_>,
    id: &str,
    display_name: &Option<String>,
    description: &Option<String>,
    public: &bool,
    protected: &bool,
) -> R<()> {
    let mut body =
        json!({"namespace": id, "visibility": if *public { "public" } else { "private" }, "protected": protected});
    if let Some(n) = opt(display_name) {
        body["display_name"] = json!(n);
    }
    if let Some(d) = opt(description) {
        body["description"] = json!(d);
    }
    x.send(metadef(
        http::Method::PUT,
        format!("metadefs/namespaces/{id}"),
        Some(body),
    ))
    .await?;
    Ok(())
}

/// `MetadefNamespaceDelete`: Glance `DELETE /metadefs/namespaces/{id}`.
pub(super) async fn metadef_namespace_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    x.send(metadef(http::Method::DELETE, format!("metadefs/namespaces/{id}"), None))
        .await?;
    Ok(())
}

/// `MetadefNamespaceSetTypes`: one `DELETE …/resource_types/{name}` per removed
/// association, then one `POST …/resource_types` per added one (with its prefix, if any).
pub(super) async fn metadef_namespace_set_types(
    x: &mut Exec<'_>,
    id: &str,
    add_type_ids: &[String],
    remove_type_ids: &[String],
    prefixes: &[(String, String)],
) -> R<()> {
    if add_type_ids.is_empty() && remove_type_ids.is_empty() {
        return Err(AppError::invalid("nothing changed"));
    }
    // Removals first: a new prefix removes the association, then adds it back.
    for name in remove_type_ids {
        x.send(metadef(
            http::Method::DELETE,
            format!("metadefs/namespaces/{id}/resource_types/{name}"),
            None,
        ))
        .await?;
    }
    for name in add_type_ids {
        let mut body = json!({ "name": name });
        if let Some((_, p)) = prefixes.iter().find(|(t, p)| t == name && !p.trim().is_empty()) {
            body["prefix"] = json!(p.trim());
        }
        x.send(metadef(
            http::Method::POST,
            format!("metadefs/namespaces/{id}/resource_types"),
            Some(body),
        ))
        .await?;
    }
    Ok(())
}

/// `FlavorCreate`: Nova `POST /flavors`, then `addTenantAccess` for each project it is
/// given to.
pub(super) async fn flavor_create(x: &mut Exec<'_>, spec: &FlavorSpec) -> R<()> {
    let id = x.create(flavor_request(spec)?, "flavor").await?;
    flavor_access(x, &id, &spec.projects, &[]).await?;
    Ok(())
}

/// `FlavorReplace`: Nova flavors are immutable, so *Edit Flavor* (as in Horizon) deletes
/// it and creates it again with the same ID, then restores its extra specs and access.
/// Everything is validated before the `DELETE`.
pub(super) async fn flavor_replace(x: &mut Exec<'_>, spec: &FlavorSpec, extra_specs: &[(String, String)]) -> R<()> {
    use openstack_sdk::api::compute::v2::flavor::{delete as del, extra_spec::create as ex};
    let id = opt(&spec.flavor_id)
        .filter(|i| !i.eq_ignore_ascii_case("auto"))
        .ok_or_else(|| AppError::invalid("the flavor's ID"))?
        .to_string();
    // Everything is checked before the old flavor goes: a refused value must not lose it.
    let create = flavor_request(spec)?;
    let specs = checked_metadata(extra_specs)?;
    x.send(del::Request::builder().id(id.as_str()).build().map_err(build_err)?)
        .await?;
    x.create(create, "flavor").await?;
    if !specs.is_empty() {
        x.send(
            ex::Request::builder()
                .flavor_id(id.as_str())
                .extra_specs(specs.into_iter())
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    flavor_access(x, &id, &spec.projects, &[]).await?;
    Ok(())
}

/// `FlavorDelete`: Nova `DELETE /flavors/{id}`.
pub(super) async fn flavor_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::flavor::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `FlavorSetExtraSpecs`: `POST /flavors/{id}/os-extra_specs` with the set keys, then one
/// `DELETE …/os-extra_specs/{key}` per removed key.
pub(super) async fn flavor_set_extra_specs(
    x: &mut Exec<'_>,
    id: &str,
    set: &[(String, String)],
    unset: &[String],
) -> R<()> {
    use openstack_sdk::api::compute::v2::flavor::extra_spec::{create as c, delete as d};
    if set.is_empty() && unset.is_empty() {
        return Err(AppError::invalid("nothing changed"));
    }
    if !set.is_empty() {
        let specs = checked_metadata(set)?;
        x.send(
            c::Request::builder()
                .flavor_id(id)
                .extra_specs(specs.into_iter())
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    for key in unset {
        // The key is a path segment (`os-extra_specs/hw:cpu_policy`).
        x.send(
            d::Request::builder()
                .flavor_id(id)
                .id(path_key(key)?)
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    Ok(())
}

/// `FlavorSetAccess`: `addTenantAccess` / `removeTenantAccess` per project.
pub(super) async fn flavor_set_access(x: &mut Exec<'_>, id: &str, add: &[String], remove: &[String]) -> R<()> {
    if add.is_empty() && remove.is_empty() {
        return Err(AppError::invalid("nothing changed"));
    }
    flavor_access(x, id, add, remove).await?;
    Ok(())
}

/// `AggregateCreate`: Nova `POST /os-aggregates`, then `add_host` per host.
pub(super) async fn aggregate_create(x: &mut Exec<'_>, name: &str, zone: &Option<String>, hosts: &[String]) -> R<()> {
    use openstack_sdk::api::compute::v2::aggregate::create_21 as m;
    let mut b = m::AggregateBuilder::default();
    b.name(non_empty(name, "name")?);
    if let Some(z) = opt(zone) {
        b.availability_zone(Some(Cow::from(z.to_string())));
    }
    let id = x
        .create(
            m::Request::builder()
                .aggregate(b.build().map_err(build_err)?)
                .build()
                .map_err(build_err)?,
            "aggregate",
        )
        .await?;
    aggregate_hosts(x, &id, hosts, &[]).await?;
    Ok(())
}

/// `AggregateUpdate`: Nova `PUT /os-aggregates/{id}`; no zone sends `null`, which removes it.
pub(super) async fn aggregate_update(x: &mut Exec<'_>, id: &str, name: &str, zone: &Option<String>) -> R<()> {
    use openstack_sdk::api::compute::v2::aggregate::set_21 as m;
    let mut b = m::AggregateBuilder::default();
    b.name(non_empty(name, "name")?);
    // An empty zone is sent as `null`, which removes it (Horizon's edit form).
    b.availability_zone(opt(zone).map(|z| Cow::from(z.to_string())));
    x.send(
        m::Request::builder()
            .id(id)
            .aggregate(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `AggregateSetHosts`: `add_host` / `remove_host` actions, one per host.
pub(super) async fn aggregate_set_hosts(x: &mut Exec<'_>, id: &str, add: &[String], remove: &[String]) -> R<()> {
    if add.is_empty() && remove.is_empty() {
        return Err(AppError::invalid("nothing changed"));
    }
    aggregate_hosts(x, id, add, remove).await?;
    Ok(())
}

/// `AggregateSetMetadata`: one `set_metadata` action; removed keys are sent as `null`.
pub(super) async fn aggregate_set_metadata(
    x: &mut Exec<'_>,
    id: &str,
    set: &[(String, String)],
    unset: &[String],
) -> R<()> {
    use openstack_sdk::api::compute::v2::aggregate::set_metadata as m;
    if set.is_empty() && unset.is_empty() {
        return Err(AppError::invalid("nothing changed"));
    }
    let set = checked_metadata(set)?;
    // Nova removes a key whose value is `null`.
    let md = set
        .into_iter()
        .map(|(k, v)| (k, Some(Cow::from(v))))
        .chain(unset.iter().map(|k| (k.clone(), None)));
    let body = m::SetMetadataBuilder::default()
        .metadata(md)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .set_metadata(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `AggregateDelete`: Nova `DELETE /os-aggregates/{id}`.
pub(super) async fn aggregate_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::aggregate::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `ComputeServiceSet`: Nova `PUT /os-services/{id}` (2.53, services by UUID) to enable or
/// disable it; the reason is sent only when disabling.
pub(super) async fn compute_service_set(x: &mut Exec<'_>, id: &str, enabled: &bool, reason: &Option<String>) -> R<()> {
    use openstack_sdk::api::compute::v2::service::set_253 as m;
    let mut b = m::Request::builder();
    b.id(id).status(if *enabled {
        m::Status::Enabled
    } else {
        m::Status::Disabled
    });
    if let Some(r) = opt(reason).filter(|_| !enabled) {
        b.disabled_reason(r.to_string());
    }
    x.send(b.build().map_err(build_err)?).await?;
    Ok(())
}

/// `ServerMigrate`: cold migration, `POST /servers/{id}/action` with `migrate`; a target
/// host needs 2.56, no host works from 2.1.
pub(super) async fn server_migrate(x: &mut Exec<'_>, id: &str, host: &Option<String>) -> R<()> {
    // Raw: the SDK's `migrate_256` always sends 2.56, which older clouds
    // refuse even without a host; `{"migrate": null}` works from 2.1.
    let r = match opt(host) {
        Some(h) => nova_action(id, json!({"migrate": {"host": h}}), Some("compute 2.56"))?,
        None => nova_action(id, json!({"migrate": null}), None)?,
    };
    x.send(r).await?;
    Ok(())
}

/// `ServerLiveMigrate`: `POST /servers/{id}/action` with `os-migrateLive` at 2.30 (no host
/// lets the scheduler pick; no `block_migration` choice sends `"auto"`).
pub(super) async fn server_live_migrate(
    x: &mut Exec<'_>,
    id: &str,
    host: &Option<String>,
    block_migration: &Option<bool>,
) -> R<()> {
    // Raw: the SDK types `block_migration` as a bool, so it can't send
    // Nova's `"auto"` (2.25: block migration only without shared storage).
    let block = block_migration.map_or(json!("auto"), Value::Bool);
    let body = json!({"os-migrateLive": {"host": opt(host), "block_migration": block}});
    x.send(nova_action(id, body, Some("compute 2.30"))?).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[tokio::test]
    async fn quota_updates_send_only_the_changed_keys() {
        let q = |service, values: &[(&str, i64)]| OsAction::QuotaUpdate {
            service,
            project_id: id("p1"),
            values: values.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        };
        let expect = [
            (
                QuotaService::Compute,
                "compute",
                "/os-quota-sets/p1",
                "quota_set",
                "cores",
            ),
            (
                QuotaService::Volume,
                "block-storage",
                "/os-quota-sets/p1",
                "quota_set",
                "gigabytes",
            ),
            (QuotaService::Network, "network", "/quotas/p1", "quota", "port"),
            (
                QuotaService::LoadBalancer,
                "load-balancer",
                "/lbaas/quotas/p1",
                "quota",
                "member",
            ),
            (
                QuotaService::KeyManager,
                "key-manager",
                "/v1/project-quotas/p1",
                "project_quotas",
                "containers",
            ),
        ];
        for (service, ty, path, wrap, key) in expect {
            let r = dry(q(service, &[(key, 20), (service.keys()[0], -1)])).await;
            assert_eq!(
                (r.len(), r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
                (1, "PUT", path, ty),
                "{service:?}"
            );
            assert_eq!(
                body(&r[0]),
                json!({ wrap: { key: 20, service.keys()[0]: -1 } }),
                "{service:?}"
            );
        }
        assert!(dry_err(q(QuotaService::Compute, &[])).await, "nothing changed");
        assert!(
            dry_err(q(QuotaService::Compute, &[("port", 3)])).await,
            "a Neutron key sent to Nova"
        );
        assert!(dry_err(q(QuotaService::Network, &[("port", -2)])).await, "below -1");
    }

    #[tokio::test]
    async fn flavors_are_created_private_for_projects_and_edited_like_horizon() {
        let spec = |projects: Vec<String>| FlavorSpec {
            name: id(" m1.big "),
            flavor_id: Some(id("auto")),
            vcpus: 4,
            ram_mb: 8192,
            disk_gb: 40,
            ephemeral_gb: 0,
            swap_mb: 0,
            rxtx_factor: None,
            projects,
        };
        let r = dry(OsAction::FlavorCreate(Box::new(spec(vec![id("p1"), id("p2")])))).await;
        assert_eq!(
            body(&r[0]),
            json!({"flavor": {"name": "m1.big", "vcpus": 4, "ram": 8192, "disk": 40, "OS-FLV-EXT-DATA:ephemeral": 0, "swap": 0, "os-flavor-access:is_public": false}}),
            "`auto`: Nova picks the ID"
        );
        assert_eq!(
            calls(&r)[1..],
            [
                (id("POST"), id("/flavors/%3Cnew%20flavor%20id%3E/action")),
                (id("POST"), id("/flavors/%3Cnew%20flavor%20id%3E/action"))
            ],
            "access on the new flavor (the dry-run placeholder, encoded)"
        );
        assert_eq!(body(&r[2]), json!({"addTenantAccess": {"tenant": "p2"}}));
        let r = dry(OsAction::FlavorCreate(Box::new(FlavorSpec {
            flavor_id: Some(id("big")),
            rxtx_factor: Some(2.0),
            ..spec(vec![])
        })))
        .await;
        assert_eq!(
            (
                r.len(),
                &body(&r[0])["flavor"]["os-flavor-access:is_public"],
                &body(&r[0])["flavor"]["id"]
            ),
            (1, &json!(true), &json!("big")),
            "no projects: public"
        );
        assert_eq!(body(&r[0])["flavor"]["rxtx_factor"], "2");
        assert!(
            dry_err(OsAction::FlavorCreate(Box::new(FlavorSpec {
                vcpus: 0,
                ..spec(vec![])
            })))
            .await,
            "at least one VCPU"
        );
        assert_eq!(
            calls(&dry(OsAction::FlavorDelete { id: id("f1") }).await),
            [(id("DELETE"), id("/flavors/f1"))]
        );
        // Edit Flavor (`[admin] flavor_edit`): Horizon's delete and re-create,
        // same ID, then the extra specs and the access back.
        let edited = || {
            Box::new(FlavorSpec {
                flavor_id: Some(id("f1")),
                ..spec(vec![id("p1")])
            })
        };
        let r = dry(OsAction::FlavorReplace {
            spec: edited(),
            extra_specs: vec![(id("hw:cpu_policy"), id("dedicated"))],
        })
        .await;
        assert_eq!(
            calls(&r),
            [
                (id("DELETE"), id("/flavors/f1")),
                (id("POST"), id("/flavors")),
                (id("POST"), id("/flavors/f1/os-extra_specs")),
                (id("POST"), id("/flavors/f1/action"))
            ]
        );
        assert_eq!(
            (
                &body(&r[1])["flavor"]["id"],
                &body(&r[1])["flavor"]["os-flavor-access:is_public"]
            ),
            (&json!("f1"), &json!(false))
        );
        assert_eq!(body(&r[2]), json!({"extra_specs": {"hw:cpu_policy": "dedicated"}}));
        assert!(
            dry_err(OsAction::FlavorReplace {
                spec: Box::new(FlavorSpec { vcpus: 0, ..*edited() }),
                extra_specs: vec![]
            })
            .await,
            "checked before the delete"
        );
        assert!(
            dry_err(OsAction::FlavorReplace {
                spec: Box::new(FlavorSpec {
                    flavor_id: Some(id("auto")),
                    ..*edited()
                }),
                extra_specs: vec![]
            })
            .await,
            "needs the flavor's ID"
        );
        let r = dry(OsAction::FlavorSetExtraSpecs {
            id: id("f1"),
            set: vec![(id("hw:cpu_policy"), id("dedicated"))],
            unset: vec![id("quota:vif")],
        })
        .await;
        assert_eq!(
            calls(&r),
            [
                (id("POST"), id("/flavors/f1/os-extra_specs")),
                (id("DELETE"), id("/flavors/f1/os-extra_specs/quota:vif"))
            ]
        );
        assert_eq!(body(&r[0]), json!({"extra_specs": {"hw:cpu_policy": "dedicated"}}));
        assert!(
            dry_err(OsAction::FlavorSetExtraSpecs {
                id: id("f1"),
                set: vec![],
                unset: vec![]
            })
            .await,
            "nothing changed"
        );
        let r = dry(OsAction::FlavorSetAccess {
            id: id("f1"),
            add: vec![id("p3")],
            remove: vec![id("p1")],
        })
        .await;
        assert_eq!(
            (body(&r[0]), body(&r[1])),
            (
                json!({"addTenantAccess": {"tenant": "p3"}}),
                json!({"removeTenantAccess": {"tenant": "p1"}})
            )
        );
    }

    #[tokio::test]
    async fn aggregates_hosts_and_metadata() {
        let r = dry(OsAction::AggregateCreate {
            name: id("gpu"),
            zone: Some(id("az1")),
            hosts: vec![id("c1")],
        })
        .await;
        assert_eq!(
            calls(&r),
            [
                (id("POST"), id("/os-aggregates")),
                (id("POST"), id("/os-aggregates/%3Cnew%20aggregate%20id%3E/action"))
            ],
            "add_host on the new aggregate (the dry-run placeholder, encoded)"
        );
        assert_eq!(
            (body(&r[0]), body(&r[1])),
            (
                json!({"aggregate": {"name": "gpu", "availability_zone": "az1"}}),
                json!({"add_host": {"host": "c1"}})
            )
        );
        let r = dry(OsAction::AggregateUpdate {
            id: id("7"),
            name: id("gpu"),
            zone: None,
        })
        .await;
        assert_eq!(calls(&r), [(id("PUT"), id("/os-aggregates/7"))]);
        assert_eq!(
            body(&r[0]),
            json!({"aggregate": {"name": "gpu", "availability_zone": null}}),
            "an empty zone clears it"
        );
        let r = dry(OsAction::AggregateSetHosts {
            id: id("7"),
            add: vec![id("c2")],
            remove: vec![id("c1")],
        })
        .await;
        assert_eq!(
            (body(&r[0]), body(&r[1])),
            (
                json!({"add_host": {"host": "c2"}}),
                json!({"remove_host": {"host": "c1"}})
            )
        );
        let r = dry(OsAction::AggregateSetMetadata {
            id: id("7"),
            set: vec![(id("ssd"), id("true"))],
            unset: vec![id("gpu")],
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"set_metadata": {"metadata": {"ssd": "true", "gpu": null}}}),
            "removed keys are null"
        );
        assert_eq!(
            calls(&dry(OsAction::AggregateDelete { id: id("7") }).await),
            [(id("DELETE"), id("/os-aggregates/7"))]
        );
    }

    #[tokio::test]
    async fn metadata_definitions_import_edit_associate_and_delete() {
        let def = r#"{"namespace": "OS::Acme::Gpu", "display_name": "GPU", "visibility": "public", "properties": {"gpu:model": {"type": "string"}}}"#;
        let r = dry(OsAction::MetadefNamespaceImport {
            definition: id(def),
            public: false,
            protected: true,
        })
        .await;
        assert_eq!(calls(&r), [(id("POST"), id("/metadefs/namespaces"))]);
        let b = body(&r[0]);
        assert_eq!(
            (
                b["visibility"].clone(),
                b["protected"].clone(),
                b["properties"]["gpu:model"]["type"].clone()
            ),
            (json!("private"), json!(true), json!("string")),
            "the form decides visibility; the rest is kept"
        );
        assert!(
            dry_err(OsAction::MetadefNamespaceImport {
                definition: id("{not json"),
                public: true,
                protected: false
            })
            .await
        );
        assert!(
            dry_err(OsAction::MetadefNamespaceImport {
                definition: id(r#"{"display_name": "x"}"#),
                public: true,
                protected: false
            })
            .await,
            "a namespace is required"
        );
        assert!(
            dry_err(OsAction::MetadefNamespaceImport {
                definition: id(r#"{"namespace": "a/b"}"#),
                public: true,
                protected: false
            })
            .await,
            "it becomes a path segment"
        );
        let r = dry(OsAction::MetadefNamespaceUpdate {
            id: id("OS::Acme::Gpu"),
            display_name: Some(id("GPU")),
            description: None,
            public: true,
            protected: false,
        })
        .await;
        assert_eq!(calls(&r), [(id("PUT"), id("/metadefs/namespaces/OS::Acme::Gpu"))]);
        assert_eq!(
            body(&r[0]),
            json!({"namespace": "OS::Acme::Gpu", "display_name": "GPU", "visibility": "public", "protected": false})
        );
        assert_eq!(
            calls(
                &dry(OsAction::MetadefNamespaceDelete {
                    id: id("OS::Acme::Gpu")
                })
                .await
            ),
            [(id("DELETE"), id("/metadefs/namespaces/OS::Acme::Gpu"))]
        );
        let r = dry(OsAction::MetadefNamespaceSetTypes {
            id: id("OS::Acme::Gpu"),
            add_type_ids: vec![id("OS::Nova::Flavor")],
            remove_type_ids: vec![id("OS::Glance::Image")],
            prefixes: vec![],
        })
        .await;
        assert_eq!(
            calls(&r),
            [
                (
                    id("DELETE"),
                    id("/metadefs/namespaces/OS::Acme::Gpu/resource_types/OS::Glance::Image")
                ),
                (id("POST"), id("/metadefs/namespaces/OS::Acme::Gpu/resource_types"))
            ]
        );
        assert_eq!(body(&r[1]), json!({"name": "OS::Nova::Flavor"}));
        // A new prefix: the association goes, then comes back with it.
        let flavor = || vec![id("OS::Nova::Flavor")];
        let r = dry(OsAction::MetadefNamespaceSetTypes {
            id: id("OS::Acme::Gpu"),
            add_type_ids: flavor(),
            remove_type_ids: flavor(),
            prefixes: vec![(id("OS::Nova::Flavor"), id(" gpu: "))],
        })
        .await;
        assert_eq!(
            calls(&r).iter().map(|c| c.0.as_str()).collect::<Vec<_>>(),
            ["DELETE", "POST"]
        );
        assert_eq!(body(&r[1]), json!({"name": "OS::Nova::Flavor", "prefix": "gpu:"}));
        assert!(
            dry_err(OsAction::MetadefNamespaceSetTypes {
                id: id("n"),
                add_type_ids: vec![],
                remove_type_ids: vec![],
                prefixes: vec![]
            })
            .await,
            "nothing changed"
        );
    }

    #[tokio::test]
    async fn default_quotas_update_the_default_class() {
        let values = |pairs: &[(&str, i64)]| {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        let r = dry(OsAction::DefaultQuotaUpdate {
            service: QuotaService::Compute,
            values: values(&[("cores", 40), ("instances", -1)]),
        })
        .await;
        assert_eq!(
            (calls(&r), body(&r[0])),
            (
                vec![(id("PUT"), id("/os-quota-class-sets/default"))],
                json!({"quota_class_set": {"cores": 40, "instances": -1}})
            )
        );
        let r = dry(OsAction::DefaultQuotaUpdate {
            service: QuotaService::Volume,
            values: values(&[("gigabytes", 2000)]),
        })
        .await;
        assert_eq!(body(&r[0]), json!({"quota_class_set": {"gigabytes": 2000}}));
        assert!(
            dry_err(OsAction::DefaultQuotaUpdate {
                service: QuotaService::Network,
                values: values(&[("port", 10)])
            })
            .await,
            "Neutron has no quota classes"
        );
        assert!(
            dry_err(OsAction::DefaultQuotaUpdate {
                service: QuotaService::Compute,
                values: values(&[("gigabytes", 1)])
            })
            .await,
            "a Cinder key on Nova"
        );
        assert!(
            dry_err(OsAction::DefaultQuotaUpdate {
                service: QuotaService::Compute,
                values: values(&[])
            })
            .await,
            "nothing changed"
        );
    }

    #[tokio::test]
    async fn compute_services_and_migrations() {
        let r = dry(OsAction::ComputeServiceSet {
            id: id("u1"),
            enabled: false,
            reason: Some(id("maintenance")),
        })
        .await;
        assert_eq!(calls(&r), [(id("PUT"), id("/os-services/u1"))]);
        assert_eq!(
            body(&r[0]),
            json!({"status": "disabled", "disabled_reason": "maintenance"})
        );
        let r = dry(OsAction::ComputeServiceSet {
            id: id("u1"),
            enabled: true,
            reason: Some(id("stale")),
        })
        .await;
        assert_eq!(body(&r[0]), json!({"status": "enabled"}), "no reason when enabling");
        let r = dry(OsAction::ServerMigrate {
            id: id("s1"),
            host: None,
        })
        .await;
        assert_eq!(
            (calls(&r), body(&r[0])),
            (vec![(id("POST"), id("/servers/s1/action"))], json!({"migrate": null}))
        );
        assert_eq!(
            body(
                &dry(OsAction::ServerMigrate {
                    id: id("s1"),
                    host: Some(id("c2"))
                })
                .await[0]
            ),
            json!({"migrate": {"host": "c2"}})
        );
        let r = dry(OsAction::ServerLiveMigrate {
            id: id("s1"),
            host: None,
            block_migration: None,
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"os-migrateLive": {"host": null, "block_migration": "auto"}}),
            "scheduler and auto block migration"
        );
        let r = dry(OsAction::ServerLiveMigrate {
            id: id("s1"),
            host: Some(id("c2")),
            block_migration: Some(true),
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"os-migrateLive": {"host": "c2", "block_migration": true}})
        );
    }
}
