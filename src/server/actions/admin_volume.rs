//! Admin › Volume writes: volume and snapshot status, migration, manage, volume
//! types and encryption, QoS specs, group types.

use super::*;

/// A specs edit (`set` pairs, `unset` keys): refused when empty, `set`
/// checked like metadata.
fn spec_changes(set: &[(String, String)], unset: &[String]) -> R<Vec<(String, String)>> {
    if set.is_empty() && unset.is_empty() {
        return Err(AppError::invalid("nothing changed"));
    }
    checked_metadata(set)
}

/// `status` if it is one of `allowed` (Horizon's choices): Cinder stores any
/// string `os-reset_status` is given, so a typo would strand the resource.
/// Also vets a QoS spec's `consumer` against `model::QOS_CONSUMERS`.
fn known_status<'s>(status: &'s str, allowed: &[&str]) -> R<&'s str> {
    let s = status.trim();
    if allowed.contains(&s) {
        Ok(s)
    } else {
        Err(AppError::invalid(format!("unknown status {s}")))
    }
}

/// `VolumeResetStatus`: `POST /volumes/{id}/action` with `os-reset_status`, to one of
/// `model::VOLUME_STATUSES`. Changes only Cinder's record, not the back end.
pub(super) async fn volume_reset_status(x: &mut Exec<'_>, id: &str, status: &str) -> R<()> {
    use openstack_sdk::api::block_storage::v3::volume::os_reset_status as m;
    let status = known_status(status, crate::model::VOLUME_STATUSES)?;
    let body = m::OsResetStatusBuilder::default()
        .status(Some(Cow::from(status)))
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .os_reset_status(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `VolumeMigrate`: `POST /volumes/{id}/action` with `os-migrate_volume` to the back-end `host`
/// (`host@backend#pool`); `force_host_copy` skips the driver's own migration.
pub(super) async fn volume_migrate(x: &mut Exec<'_>, id: &str, host: &str, force_host_copy: &bool) -> R<()> {
    use openstack_sdk::api::block_storage::v3::volume::os_migrate_volume_30 as m;
    let body = m::OsMigrateVolumeBuilder::default()
        .host(non_empty(host, "destination host")?)
        .force_host_copy(*force_host_copy)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .os_migrate_volume(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `VolumeManage`: `POST /manageable_volumes` (3.0), adopts a back-end volume, found by ID
/// (`source-id`) or by name (`source-name`), into Cinder.
pub(super) async fn volume_manage(x: &mut Exec<'_>, spec: &ManageVolumeSpec) -> R<()> {
    use openstack_sdk::api::block_storage::v3::volume_manage::create_30 as m;
    let key = if spec.by_id { "source-id" } else { "source-name" };
    let mut b = m::VolumeBuilder::default();
    b._ref(json!({ key: non_empty(&spec.identifier, "identifier")? }))
        .host(Some(Cow::from(non_empty(&spec.host, "host")?.to_string())))
        .bootable(spec.bootable);
    // Empty fields are left out: Cinder names it after nothing, uses its
    // default type and the back end's zone.
    let some = |v: &Option<String>| opt(v).map(|v| Some(Cow::from(v.to_string())));
    if let Some(v) = some(&spec.name) {
        b.name(v);
    }
    if let Some(v) = some(&spec.description) {
        b.description(v);
    }
    if let Some(v) = some(&spec.volume_type) {
        b.volume_type(v);
    }
    if let Some(v) = some(&spec.availability_zone) {
        b.availability_zone(v);
    }
    let md = checked_metadata(&spec.metadata)?;
    if !md.is_empty() {
        b.metadata(md.into_iter());
    }
    x.create(
        m::Request::builder()
            .volume(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "volume",
    )
    .await?;
    Ok(())
}

/// `VolumeUnmanage`: `POST /volumes/{id}/action` with `os-unmanage`; Cinder forgets the volume
/// but leaves its data on the back end.
pub(super) async fn volume_unmanage(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::block_storage::v3::volume::os_unmanage as m;
    x.send(
        m::Request::builder()
            .id(id)
            .os_unmanage(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `VolumeTypeCreate`: `POST /types` as a raw JSON request, because the SDK's create posts to
/// `types/{id}/action` (the dry-run test guards it).
pub(super) async fn volume_type_create(
    x: &mut Exec<'_>,
    name: &str,
    description: &Option<String>,
    public: &bool,
) -> R<()> {
    let mut t = json!({"name": non_empty(name, "name")?, "os-volume-type-access:is_public": public});
    if let Some(d) = opt(description) {
        t["description"] = json!(d);
    }
    x.create(
        JsonRequest::new(
            http::Method::POST,
            openstack_sdk::types::ServiceType::BlockStorage,
            "types",
            Some(json!({ "volume_type": t })),
        ),
        "volume type",
    )
    .await?;
    Ok(())
}

/// `VolumeTypeUpdate`: `PUT /types/{id}` with the name, description (`""` clears it) and
/// `is_public`.
pub(super) async fn volume_type_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    description: &Option<String>,
    public: &bool,
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::r#type::set as m;
    let body = m::VolumeTypeBuilder::default()
        .name(Some(Cow::from(non_empty(name, "name")?)))
        .description(Some(Cow::from(opt(description).unwrap_or(""))))
        .is_public(*public)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .volume_type(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `VolumeTypeDelete`: `DELETE /types/{id}`.
pub(super) async fn volume_type_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::block_storage::v3::r#type::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `VolumeTypeSetExtraSpecs`: one `POST /types/{id}/extra_specs` for every `set` pair, then a
/// `DELETE /types/{id}/extra_specs/{key}` per `unset` key.
pub(super) async fn volume_type_set_extra_specs(
    x: &mut Exec<'_>,
    id: &str,
    set: &[(String, String)],
    unset: &[String],
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::r#type::extra_spec::{create as c, delete as d};
    let set = spec_changes(set, unset)?;
    if !set.is_empty() {
        x.send(
            c::Request::builder()
                .type_id(id)
                .extra_specs(set.into_iter().map(|(k, v)| (k, Some(Cow::from(v)))))
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    for key in unset {
        x.send(
            d::Request::builder()
                .type_id(id)
                .id(path_key(key)?)
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    Ok(())
}

/// `VolumeTypeSetQos`: moves the type from `current_qos_id` to `qos_id` (either may be `None`):
/// `GET /qos-specs/{old}/disassociate`, then `GET /qos-specs/{new}/associate`, each with
/// `vol_type_id`. Fails when nothing changes.
pub(super) async fn volume_type_set_qos(
    x: &mut Exec<'_>,
    id: &str,
    qos_id: &Option<String>,
    current_qos_id: &Option<String>,
) -> R<()> {
    let (new, old) = (opt(qos_id), opt(current_qos_id));
    if new == old {
        return Err(AppError::invalid("nothing changed"));
    }
    // Raw: the SDK's associate/disassociate lack `vol_type_id`. They
    // are GETs that change state (Cinder's API), hence sent as writes.
    let qos_call = |qos: &str, verb: &str| {
        let mut r = JsonRequest::new(
            http::Method::GET,
            openstack_sdk::types::ServiceType::BlockStorage,
            format!("qos-specs/{qos}/{verb}"),
            None,
        );
        r.query.push(("vol_type_id", id.to_string()));
        r
    };
    // `qos_id` / `current_qos_id` are path segments: `check_ids` vets them.
    if let Some(q) = old {
        x.send(qos_call(q, "disassociate")).await?;
    }
    if let Some(q) = new {
        x.send(qos_call(q, "associate")).await?;
    }
    Ok(())
}

/// `VolumeTypeEncryptionSet`: `POST /types/{id}/encryption`, or with `update`
/// `PUT /types/{id}/encryption/provider`, as raw JSON (see the comment inside). Fails on a
/// control location outside `model::CONTROL_LOCATIONS`.
pub(super) async fn volume_type_encryption_set(
    x: &mut Exec<'_>,
    id: &str,
    spec: &EncryptionSpec,
    update: &bool,
) -> R<()> {
    if !crate::model::CONTROL_LOCATIONS.contains(&spec.control_location.as_str()) {
        return Err(AppError::invalid(format!(
            "unknown control location {}",
            spec.control_location
        )));
    }
    let mut e = json!({"provider": non_empty(&spec.provider, "provider")?, "control_location": spec.control_location});
    if let Some(c) = opt(&spec.cipher) {
        e["cipher"] = json!(c);
    }
    if let Some(k) = spec.key_size {
        e["key_size"] = json!(k);
    }
    // Raw: the SDK's create also requires `control_location` as an enum
    // and its update needs an encryption ID Cinder ignores (cinderclient
    // sends `provider`).
    let (method, path) = if *update {
        (http::Method::PUT, format!("types/{id}/encryption/provider"))
    } else {
        (http::Method::POST, format!("types/{id}/encryption"))
    };
    x.send(JsonRequest::new(
        method,
        openstack_sdk::types::ServiceType::BlockStorage,
        path,
        Some(json!({ "encryption": e })),
    ))
    .await?;
    Ok(())
}

/// `VolumeTypeEncryptionDelete`: `DELETE /types/{id}/encryption/provider`, as cinderclient does
/// (Cinder ignores the encryption ID in that path).
pub(super) async fn volume_type_encryption_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    x.send(JsonRequest::new(
        http::Method::DELETE,
        openstack_sdk::types::ServiceType::BlockStorage,
        format!("types/{id}/encryption/provider"),
        None,
    ))
    .await?;
    Ok(())
}

/// `QosSpecCreate`: `POST /qos-specs` with `consumer` (one of `model::QOS_CONSUMERS`) and the
/// specs as properties.
pub(super) async fn qos_spec_create(x: &mut Exec<'_>, name: &str, consumer: &str, specs: &[(String, String)]) -> R<()> {
    use openstack_sdk::api::block_storage::v3::qos_spec::create as m;
    let consumer = known_status(consumer, crate::model::QOS_CONSUMERS)?;
    let specs = checked_metadata(specs)?;
    let mut b = m::QosSpecsBuilder::default();
    b.name(non_empty(name, "name")?).properties(
        std::iter::once(("consumer".to_string(), json!(consumer))).chain(specs.into_iter().map(|(k, v)| (k, json!(v)))),
    );
    x.create(
        m::Request::builder()
            .qos_specs(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "QoS spec",
    )
    .await?;
    Ok(())
}

/// `QosSpecSetConsumer`: `PUT /qos-specs/{id}` with only `consumer`.
pub(super) async fn qos_spec_set_consumer(x: &mut Exec<'_>, id: &str, consumer: &str) -> R<()> {
    use openstack_sdk::api::block_storage::v3::qos_spec::set as m;
    let consumer = known_status(consumer, crate::model::QOS_CONSUMERS)?;
    x.send(
        m::Request::builder()
            .id(id)
            .qos_specs(std::iter::once(("consumer", Some(Cow::from(consumer)))))
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `QosSpecSetSpecs`: `PUT /qos-specs/{id}` with the `set` pairs, then one
/// `PUT /qos-specs/{id}/delete_keys` with every `unset` key.
pub(super) async fn qos_spec_set_specs(
    x: &mut Exec<'_>,
    id: &str,
    set: &[(String, String)],
    unset: &[String],
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::qos_spec::{delete_keys as d, set as m};
    let set = spec_changes(set, unset)?;
    if !set.is_empty() {
        x.send(
            m::Request::builder()
                .id(id)
                .qos_specs(set.into_iter().map(|(k, v)| (k, Some(Cow::from(v)))))
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    if !unset.is_empty() {
        x.send(
            d::Request::builder()
                .id(id)
                .keys(unset.iter().map(|k| Cow::from(k.as_str())).collect::<Vec<_>>())
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    Ok(())
}

/// `QosSpecDelete`: `DELETE /qos-specs/{id}`.
pub(super) async fn qos_spec_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::block_storage::v3::qos_spec::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `GroupTypeCreate`: `POST /group_types` (3.11).
pub(super) async fn group_type_create(
    x: &mut Exec<'_>,
    name: &str,
    description: &Option<String>,
    public: &bool,
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::group_type::create_311 as m;
    let mut b = m::GroupTypeBuilder::default();
    b.name(non_empty(name, "name")?).is_public(*public);
    if let Some(d) = opt(description) {
        b.description(Some(Cow::from(d)));
    }
    x.create(
        m::Request::builder()
            .group_type(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "group type",
    )
    .await?;
    Ok(())
}

/// `GroupTypeUpdate`: `PUT /group_types/{id}` (3.11) with the name, description and `is_public`.
pub(super) async fn group_type_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    description: &Option<String>,
    public: &bool,
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::group_type::set_311 as m;
    let body = m::GroupTypeBuilder::default()
        .name(Some(Cow::from(non_empty(name, "name")?)))
        .description(Some(Cow::from(opt(description).unwrap_or(""))))
        .is_public(*public)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .group_type(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `GroupTypeDelete`: `DELETE /group_types/{id}` (3.11).
pub(super) async fn group_type_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::block_storage::v3::group_type::delete_311 as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `GroupTypeSetSpecs`: one `POST /group_types/{id}/group_specs` (3.11) for the `set` pairs,
/// then a `DELETE /group_types/{id}/group_specs/{key}` per `unset` key.
pub(super) async fn group_type_set_specs(
    x: &mut Exec<'_>,
    id: &str,
    set: &[(String, String)],
    unset: &[String],
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::group_type::group_spec::{create_311 as c, delete_311 as d};
    let set = spec_changes(set, unset)?;
    if !set.is_empty() {
        x.send(
            c::Request::builder()
                .group_type_id(id)
                .group_specs(set.into_iter().map(|(k, v)| (k, Some(Cow::from(v)))))
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    for key in unset {
        x.send(
            d::Request::builder()
                .group_type_id(id)
                .id(path_key(key)?)
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    Ok(())
}

/// `SnapshotResetStatus`: `POST /snapshots/{id}/action` with `os-reset_status`, to one of
/// `model::SNAPSHOT_STATUSES`.
pub(super) async fn snapshot_reset_status(x: &mut Exec<'_>, id: &str, status: &str) -> R<()> {
    use openstack_sdk::api::block_storage::v3::snapshot::os_reset_status as m;
    let status = known_status(status, crate::model::SNAPSHOT_STATUSES)?;
    let body = m::OsResetStatusBuilder::default()
        .status(status)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .os_reset_status(body)
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
    use crate::model::EncryptionSpec;

    #[tokio::test]
    async fn volume_types_specs_qos_and_encryption() {
        let r = dry(OsAction::VolumeTypeCreate {
            name: id("ssd"),
            description: None,
            public: false,
        })
        .await;
        assert_eq!(
            (calls(&r), body(&r[0])),
            (
                vec![(id("POST"), id("/types"))],
                json!({"volume_type": {"name": "ssd", "os-volume-type-access:is_public": false}})
            ),
            "not the SDK's types/{{id}}/action"
        );
        let r = dry(OsAction::VolumeTypeUpdate {
            id: id("t1"),
            name: id("ssd"),
            description: None,
            public: true,
        })
        .await;
        assert_eq!(
            (calls(&r), body(&r[0])),
            (
                vec![(id("PUT"), id("/types/t1"))],
                json!({"volume_type": {"name": "ssd", "description": "", "is_public": true}})
            ),
            "an empty description clears it"
        );
        assert_eq!(
            calls(&dry(OsAction::VolumeTypeDelete { id: id("t1") }).await),
            [(id("DELETE"), id("/types/t1"))]
        );
        let r = dry(OsAction::VolumeTypeSetExtraSpecs {
            id: id("t1"),
            set: vec![(id("volume_backend_name"), id("ceph"))],
            unset: vec![id("old")],
        })
        .await;
        assert_eq!(
            calls(&r),
            [
                (id("POST"), id("/types/t1/extra_specs")),
                (id("DELETE"), id("/types/t1/extra_specs/old"))
            ]
        );
        assert_eq!(body(&r[0]), json!({"extra_specs": {"volume_backend_name": "ceph"}}));
        assert!(
            dry_err(OsAction::VolumeTypeSetExtraSpecs {
                id: id("t1"),
                set: vec![],
                unset: vec![]
            })
            .await,
            "nothing changed"
        );
        let r = dry(OsAction::VolumeTypeSetQos {
            id: id("t1"),
            qos_id: Some(id("q2")),
            current_qos_id: Some(id("q1")),
        })
        .await;
        assert_eq!(
            calls(&r),
            [
                (id("GET"), id("/qos-specs/q1/disassociate?vol_type_id=t1")),
                (id("GET"), id("/qos-specs/q2/associate?vol_type_id=t1"))
            ],
            "the old one first"
        );
        let r = dry(OsAction::VolumeTypeSetQos {
            id: id("t1"),
            qos_id: None,
            current_qos_id: Some(id("q1")),
        })
        .await;
        assert_eq!(calls(&r).len(), 1, "None only disassociates");
        assert!(
            dry_err(OsAction::VolumeTypeSetQos {
                id: id("t1"),
                qos_id: Some(id("q1")),
                current_qos_id: Some(id("q1"))
            })
            .await,
            "nothing changed"
        );
        let spec = EncryptionSpec {
            provider: id("luks"),
            control_location: id("front-end"),
            cipher: Some(id("aes-xts-plain64")),
            key_size: Some(256),
        };
        let r = dry(OsAction::VolumeTypeEncryptionSet {
            id: id("t1"),
            spec: spec.clone(),
            update: false,
        })
        .await;
        assert_eq!(calls(&r), [(id("POST"), id("/types/t1/encryption"))]);
        assert_eq!(
            body(&r[0]),
            json!({"encryption": {"provider": "luks", "control_location": "front-end", "cipher": "aes-xts-plain64", "key_size": 256}})
        );
        let r = dry(OsAction::VolumeTypeEncryptionSet {
            id: id("t1"),
            spec: EncryptionSpec {
                cipher: None,
                key_size: None,
                ..spec.clone()
            },
            update: true,
        })
        .await;
        assert_eq!(
            (calls(&r), body(&r[0])),
            (
                vec![(id("PUT"), id("/types/t1/encryption/provider"))],
                json!({"encryption": {"provider": "luks", "control_location": "front-end"}})
            ),
            "defaults left out"
        );
        assert!(
            dry_err(OsAction::VolumeTypeEncryptionSet {
                id: id("t1"),
                spec: EncryptionSpec {
                    control_location: id("middle"),
                    ..spec
                },
                update: false
            })
            .await
        );
        assert_eq!(
            calls(&dry(OsAction::VolumeTypeEncryptionDelete { id: id("t1") }).await),
            [(id("DELETE"), id("/types/t1/encryption/provider"))]
        );
        let r = dry(OsAction::QosSpecCreate {
            name: id("gold"),
            consumer: id("back-end"),
            specs: vec![(id("total_iops_sec"), id("1000"))],
        })
        .await;
        assert_eq!(
            (calls(&r), body(&r[0])),
            (
                vec![(id("POST"), id("/qos-specs"))],
                json!({"qos_specs": {"name": "gold", "consumer": "back-end", "total_iops_sec": "1000"}})
            )
        );
        assert!(
            dry_err(OsAction::QosSpecCreate {
                name: id("gold"),
                consumer: id("anywhere"),
                specs: vec![]
            })
            .await,
            "Horizon's consumers only"
        );
        let r = dry(OsAction::QosSpecSetConsumer {
            id: id("q1"),
            consumer: id("both"),
        })
        .await;
        assert_eq!(
            (calls(&r), body(&r[0])),
            (
                vec![(id("PUT"), id("/qos-specs/q1"))],
                json!({"qos_specs": {"consumer": "both"}})
            )
        );
        let r = dry(OsAction::QosSpecSetSpecs {
            id: id("q1"),
            set: vec![(id("read_iops_sec"), id("500"))],
            unset: vec![id("total_iops_sec")],
        })
        .await;
        assert_eq!(
            calls(&r),
            [
                (id("PUT"), id("/qos-specs/q1")),
                (id("PUT"), id("/qos-specs/q1/delete_keys"))
            ]
        );
        assert_eq!(
            (body(&r[0]), body(&r[1])),
            (
                json!({"qos_specs": {"read_iops_sec": "500"}}),
                json!({"keys": ["total_iops_sec"]})
            )
        );
        assert_eq!(
            calls(&dry(OsAction::QosSpecDelete { id: id("q1") }).await),
            [(id("DELETE"), id("/qos-specs/q1"))]
        );
    }

    #[tokio::test]
    async fn group_types_and_their_specs() {
        let r = dry(OsAction::GroupTypeCreate {
            name: id("cg"),
            description: Some(id("consistent")),
            public: true,
        })
        .await;
        assert_eq!(
            (calls(&r), body(&r[0])),
            (
                vec![(id("POST"), id("/group_types"))],
                json!({"group_type": {"name": "cg", "description": "consistent", "is_public": true}})
            )
        );
        let r = dry(OsAction::GroupTypeUpdate {
            id: id("g1"),
            name: id("cg"),
            description: None,
            public: false,
        })
        .await;
        assert_eq!(
            (calls(&r), body(&r[0])),
            (
                vec![(id("PUT"), id("/group_types/g1"))],
                json!({"group_type": {"name": "cg", "description": "", "is_public": false}})
            )
        );
        assert_eq!(
            calls(&dry(OsAction::GroupTypeDelete { id: id("g1") }).await),
            [(id("DELETE"), id("/group_types/g1"))]
        );
        let r = dry(OsAction::GroupTypeSetSpecs {
            id: id("g1"),
            set: vec![(id("consistent_group_snapshot_enabled"), id("<is> True"))],
            unset: vec![id("x")],
        })
        .await;
        assert_eq!(
            calls(&r),
            [
                (id("POST"), id("/group_types/g1/group_specs")),
                (id("DELETE"), id("/group_types/g1/group_specs/x"))
            ]
        );
        assert_eq!(
            body(&r[0]),
            json!({"group_specs": {"consistent_group_snapshot_enabled": "<is> True"}})
        );
    }

    #[tokio::test]
    async fn admin_volume_status_migrate_manage_and_unmanage() {
        let r = dry(OsAction::VolumeResetStatus {
            id: id("v1"),
            status: id("available"),
        })
        .await;
        assert_eq!(calls(&r), [(id("POST"), id("/volumes/v1/action"))]);
        assert_eq!(
            body(&r[0]),
            json!({"os-reset_status": {"status": "available"}}),
            "status only, as Horizon"
        );
        assert!(
            dry_err(OsAction::VolumeResetStatus {
                id: id("v1"),
                status: id("fixed")
            })
            .await,
            "Horizon's statuses only"
        );
        let r = dry(OsAction::VolumeMigrate {
            id: id("v1"),
            host: id("cinder@ceph#fast"),
            force_host_copy: true,
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"os-migrate_volume": {"host": "cinder@ceph#fast", "force_host_copy": true}})
        );
        assert!(
            dry_err(OsAction::VolumeMigrate {
                id: id("v1"),
                host: id(" "),
                force_host_copy: false
            })
            .await,
            "a destination is required"
        );
        let spec = ManageVolumeSpec {
            identifier: id("vol-0042"),
            by_id: false,
            host: id("cinder@ceph#fast"),
            name: Some(id("legacy")),
            description: None,
            volume_type: Some(id("ssd")),
            availability_zone: Some(String::new()),
            bootable: true,
            metadata: vec![(id("origin"), id("san"))],
        };
        let r = dry(OsAction::VolumeManage(Box::new(spec.clone()))).await;
        assert_eq!(calls(&r), [(id("POST"), id("/os-volume-manage"))]);
        assert_eq!(
            body(&r[0]),
            json!({"volume": {"ref": {"source-name": "vol-0042"}, "host": "cinder@ceph#fast", "name": "legacy", "volume_type": "ssd", "bootable": true, "metadata": {"origin": "san"}}}),
            "empty zone and description left out"
        );
        let r = dry(OsAction::VolumeManage(Box::new(ManageVolumeSpec {
            by_id: true,
            ..spec
        })))
        .await;
        assert_eq!(body(&r[0])["volume"]["ref"], json!({"source-id": "vol-0042"}));
        let r = dry(OsAction::VolumeUnmanage { id: id("v1") }).await;
        assert_eq!(
            (calls(&r), body(&r[0])),
            (
                vec![(id("POST"), id("/volumes/v1/action"))],
                json!({"os-unmanage": null})
            )
        );
        let r = dry(OsAction::SnapshotResetStatus {
            id: id("s1"),
            status: id("error"),
        })
        .await;
        assert_eq!(
            (calls(&r), body(&r[0])),
            (
                vec![(id("POST"), id("/snapshots/s1/action"))],
                json!({"os-reset_status": {"status": "error"}})
            )
        );
        assert!(
            dry_err(OsAction::SnapshotResetStatus {
                id: id("s1"),
                status: id("in-use")
            })
            .await,
            "not a snapshot status"
        );
    }
}
