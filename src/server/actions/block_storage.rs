//! Cinder writes: volumes, snapshots, transfers, groups, backups.

use super::*;

/// *Delete Group*. A JSON request because the SDK's `group::delete_313` posts
/// to `groups/action`, without the group's ID. Cinder needs volume 3.13 for
/// group actions, and a JSON request negotiates no microversion: set it here.
fn volume_group_delete(id: &str, delete_volumes: bool) -> R<JsonRequest> {
    let mut r = JsonRequest::new(
        http::Method::POST,
        openstack_sdk::types::ServiceType::BlockStorage,
        format!("groups/{}/action", non_empty(id, "group")?),
        Some(json!({"delete": {"delete-volumes": delete_volumes}})),
    );
    let mut h = http::HeaderMap::new();
    h.insert("openstack-api-version", http::HeaderValue::from_static("volume 3.13"));
    r.headers = Some(h);
    Ok(r)
}

/// `VolumeCreate`: `POST /volumes` (3.0), empty or from an image, a snapshot or another volume
/// (`source_id` is then required). Fails on a size of 0 GiB.
#[allow(clippy::too_many_arguments)] // one parameter per field of the variant
pub(super) async fn volume_create(
    x: &mut Exec<'_>,
    name: &str,
    description: &Option<String>,
    size_gb: &u32,
    volume_type: &Option<String>,
    availability_zone: &Option<String>,
    source: &VolumeSource,
    source_id: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::volume::create_30 as m;
    if *size_gb == 0 {
        return Err(AppError::invalid("size must be at least 1 GiB"));
    }
    let mut b = m::VolumeBuilder::default();
    b.name(Some(Cow::from(name.trim().to_string())))
        .size(Some(*size_gb as i32));
    if let Some(d) = opt(description) {
        b.description(Some(Cow::from(d.to_string())));
    }
    if let Some(t) = opt(volume_type) {
        b.volume_type(Some(Cow::from(t.to_string())));
    }
    if let Some(z) = opt(availability_zone) {
        b.availability_zone(Some(Cow::from(z.to_string())));
    }
    let src = opt(source_id).map(|s| Cow::from(s.to_string()));
    match (source, src) {
        (VolumeSource::Empty, _) => {}
        (VolumeSource::Image, Some(s)) => {
            b.image_ref(Some(s));
        }
        (VolumeSource::Snapshot, Some(s)) => {
            b.snapshot_id(Some(s));
        }
        (VolumeSource::Volume, Some(s)) => {
            b.source_volid(Some(s));
        }
        (_, None) => return Err(AppError::invalid("volume source is required")),
    }
    let body = b.build().map_err(build_err)?;
    x.create(m::Request::builder().volume(body).build().map_err(build_err)?, "volume")
        .await?;
    Ok(())
}

/// `VolumeUpdate`: `PUT /volumes/{id}` with the name and description; an empty description is
/// sent as `""`, which clears it.
pub(super) async fn volume_update(x: &mut Exec<'_>, id: &str, name: &str, description: &Option<String>) -> R<()> {
    use openstack_sdk::api::block_storage::v3::volume::set_30 as m;
    let body = m::VolumeBuilder::default()
        .name(Some(Cow::from(name.trim().to_string())))
        .description(Some(Cow::from(opt(description).unwrap_or("").to_string())))
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).volume(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `VolumeExtend`: `POST /volumes/{id}/action` with `os-extend` to `new_size_gb` (Cinder only
/// grows a volume).
pub(super) async fn volume_extend(x: &mut Exec<'_>, id: &str, new_size_gb: &u32) -> R<()> {
    use openstack_sdk::api::block_storage::v3::volume::os_extend as m;
    let body = m::OsExtendBuilder::default()
        .new_size(*new_size_gb as i32)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .os_extend(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `VolumeDelete`: `DELETE /volumes/{id}`.
pub(super) async fn volume_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::block_storage::v3::volume::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `VolumeSetMetadata`: replaces all the volume's metadata, `PUT /volumes/{id}/metadata`.
pub(super) async fn volume_set_metadata(x: &mut Exec<'_>, id: &str, metadata: &[(String, String)]) -> R<()> {
    use openstack_sdk::api::block_storage::v3::volume::metadata::replace as m;
    let md = checked_metadata(metadata)?;
    x.send(
        m::Request::builder()
            .volume_id(id)
            .metadata(md.into_iter())
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `VolumeRetype`: `POST /volumes/{id}/action` with `os-retype`; `migrate` maps to the migration
/// policy (`on-demand`, else `never`, which fails if the new type needs another backend).
pub(super) async fn volume_retype(x: &mut Exec<'_>, id: &str, new_type: &str, migrate: &bool) -> R<()> {
    use openstack_sdk::api::block_storage::v3::volume::os_retype as m;
    let policy = if *migrate {
        m::MigrationPolicy::OnDemand
    } else {
        m::MigrationPolicy::Never
    };
    let body = m::OsRetypeBuilder::default()
        .new_type(non_empty(new_type, "volume type")?)
        .migration_policy(policy)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .os_retype(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `VolumeUploadImage`: `POST /volumes/{id}/action` with `os-volume_upload_image` (3.1, for
/// `visibility`), container format `bare`. `force` uploads an attached volume. Fails on a disk
/// format or visibility Cinder doesn't know.
pub(super) async fn volume_upload_image(
    x: &mut Exec<'_>,
    id: &str,
    image_name: &str,
    disk_format: &str,
    force: &bool,
    visibility: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::volume::os_volume_upload_image_31 as m;
    let format = match disk_format {
        "raw" => m::DiskFormat::Raw,
        "qcow2" => m::DiskFormat::Qcow2,
        "vmdk" => m::DiskFormat::Vmdk,
        "vdi" => m::DiskFormat::Vdi,
        "vhd" => m::DiskFormat::Vhd,
        "vhdx" => m::DiskFormat::Vhdx,
        "ploop" => m::DiskFormat::Ploop,
        other => return Err(AppError::invalid(format!("unknown disk format {other}"))),
    };
    let mut b = m::OsVolumeUploadImageBuilder::default();
    b.image_name(non_empty(image_name, "image name")?)
        .disk_format(format)
        .container_format(Some(Cow::from("bare")))
        .force(*force);
    match opt(visibility) {
        None => {}
        Some("private") => {
            b.visibility(m::Visibility::Private);
        }
        Some("shared") => {
            b.visibility(m::Visibility::Shared);
        }
        Some("community") => {
            b.visibility(m::Visibility::Community);
        }
        Some("public") => {
            b.visibility(m::Visibility::Public);
        }
        Some(other) => return Err(AppError::invalid(format!("unknown visibility {other}"))),
    }
    let body = b.build().map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .os_volume_upload_image(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `VolumeTransferCreate`: `POST /os-volume-transfer`. Cinder returns the transfer ID and its
/// authorization key once: both go back as a `Secret` for the receiving project.
pub(super) async fn volume_transfer_create(x: &mut Exec<'_>, volume_id: &str, name: &Option<String>) -> R<()> {
    use openstack_sdk::api::block_storage::v3::os_volume_transfer::create as m;
    let mut b = m::TransferBuilder::default();
    b.volume_id(volume_id);
    if let Some(n) = opt(name) {
        b.name(Some(Cow::from(n.to_string())));
    }
    let body = b.build().map_err(build_err)?;
    let resp = x
        .call(m::Request::builder().transfer(body).build().map_err(build_err)?)
        .await?;
    // The receiving project needs both; Cinder never shows the key again.
    if let Some(v) = resp {
        let get = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        x.secret = Some(Secret {
            file_name: format!("transfer-{volume_id}.txt"),
            content: format!("Transfer ID: {}\nAuthorization key: {}\n", get("id"), get("auth_key")),
            kind: SecretKind::Transfer,
            fields: vec![],
            extra: vec![],
        });
    }
    Ok(())
}

/// `VolumeTransferAccept`: `POST /os-volume-transfer/{transfer_id}/accept` with the key, which
/// moves the volume into the current project.
pub(super) async fn volume_transfer_accept(x: &mut Exec<'_>, transfer_id: &str, auth_key: &str) -> R<()> {
    use openstack_sdk::api::block_storage::v3::os_volume_transfer::accept as m;
    let body = m::AcceptBuilder::default()
        .auth_key(non_empty(auth_key, "authorization key")?)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(non_empty(transfer_id, "transfer ID")?)
            .accept(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `VolumeSnapshotCreate`: `POST /snapshots`; `force` snapshots an attached (`in-use`) volume.
pub(super) async fn volume_snapshot_create(
    x: &mut Exec<'_>,
    volume_id: &str,
    name: &str,
    description: &Option<String>,
    force: &bool,
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::snapshot::create as m;
    let mut b = m::SnapshotBuilder::default();
    b.volume_id(volume_id)
        .name(Some(Cow::from(non_empty(name, "name")?.to_string())));
    if let Some(d) = opt(description) {
        b.description(Some(Cow::from(d.to_string())));
    }
    if *force {
        b.force(true);
    }
    let body = b.build().map_err(build_err)?;
    x.create(
        m::Request::builder().snapshot(body).build().map_err(build_err)?,
        "snapshot",
    )
    .await?;
    Ok(())
}

/// `SnapshotUpdate`: `PUT /snapshots/{id}` with the name and description (`""` clears it).
pub(super) async fn snapshot_update(x: &mut Exec<'_>, id: &str, name: &str, description: &Option<String>) -> R<()> {
    use openstack_sdk::api::block_storage::v3::snapshot::set as m;
    let body = m::SnapshotBuilder::default()
        .name(Some(Cow::from(name.trim().to_string())))
        .description(Some(Cow::from(opt(description).unwrap_or("").to_string())))
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).snapshot(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `SnapshotDelete`: `DELETE /snapshots/{id}`.
pub(super) async fn snapshot_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::block_storage::v3::snapshot::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `SnapshotSetMetadata`: replaces all the snapshot's metadata, `PUT /snapshots/{id}/metadata`.
pub(super) async fn snapshot_set_metadata(x: &mut Exec<'_>, id: &str, metadata: &[(String, String)]) -> R<()> {
    use openstack_sdk::api::block_storage::v3::snapshot::metadata::replace as m;
    let md = checked_metadata(metadata)?;
    x.send(
        m::Request::builder()
            .snapshot_id(id)
            .metadata(md.into_iter())
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `VolumeGroupDelete`: sends [`volume_group_delete`], optionally with the group's volumes.
pub(super) async fn delete_volume_group(x: &mut Exec<'_>, id: &str, delete_volumes: &bool) -> R<()> {
    x.send(volume_group_delete(id, *delete_volumes)?).await?;
    Ok(())
}

/// `VolumeGroupCreate`: `POST /groups` (3.13) with a group type and at least one volume type.
pub(super) async fn volume_group_create(
    x: &mut Exec<'_>,
    name: &str,
    description: &Option<String>,
    group_type_id: &str,
    volume_type_ids: &[String],
    availability_zone: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::group::create_313 as m;
    let types: Vec<Cow<str>> = volume_type_ids
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(Cow::from)
        .collect();
    if types.is_empty() {
        return Err(AppError::invalid("a group needs at least one volume type"));
    }
    let mut b = m::GroupBuilder::default();
    b.name(Some(Cow::from(non_empty(name, "name")?)))
        .group_type(non_empty(group_type_id, "group type")?)
        .volume_types(types);
    if let Some(d) = opt(description) {
        b.description(Some(Cow::from(d)));
    }
    if let Some(z) = opt(availability_zone) {
        b.availability_zone(Some(Cow::from(z)));
    }
    x.create(
        m::Request::builder()
            .group(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "group",
    )
    .await?;
    Ok(())
}

/// `VolumeGroupCreateFrom`: `POST /groups/action` with `create-from-src` (3.14), from either a
/// source group or a group snapshot, never both.
pub(super) async fn volume_group_create_from(
    x: &mut Exec<'_>,
    name: &str,
    description: &Option<String>,
    source_group_id: &Option<String>,
    group_snapshot_id: &Option<String>,
) -> R<()> {
    // Cinder answers `{"group": {...}}` here (no `response_key` in the
    // SDK), so the new ID isn't read back: `send`, not `create`.
    use openstack_sdk::api::block_storage::v3::group::create_from_src_314 as m;
    let mut b = m::CreateFromSrcBuilder::default();
    b.name(Some(Cow::from(non_empty(name, "name")?)));
    match (opt(source_group_id), opt(group_snapshot_id)) {
        (Some(g), None) => b.source_group_id(g),
        (None, Some(s)) => b.group_snapshot_id(s),
        _ => {
            return Err(AppError::invalid(
                "a group is created from either a group or a group snapshot",
            ));
        }
    };
    if let Some(d) = opt(description) {
        b.description(Some(Cow::from(d)));
    }
    x.send(
        m::Request::builder()
            .create_from_src(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `VolumeGroupUpdate`: `PUT /groups/{id}` (3.13) with the name and description.
pub(super) async fn volume_group_update(x: &mut Exec<'_>, id: &str, name: &str, description: &Option<String>) -> R<()> {
    use openstack_sdk::api::block_storage::v3::group::set_313 as m;
    // An empty description is sent as `""` so clearing it works.
    let body = m::GroupBuilder::default()
        .name(Some(Cow::from(non_empty(name, "name")?)))
        .description(Some(Cow::from(opt(description).unwrap_or(""))))
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(non_empty(id, "group")?)
            .group(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `VolumeGroupSetVolumes`: `PUT /groups/{id}` (3.13) with `add_volumes` / `remove_volumes`,
/// each one comma-separated string of volume IDs.
pub(super) async fn volume_group_set_volumes(
    x: &mut Exec<'_>,
    id: &str,
    add_volume_ids: &[String],
    remove_volume_ids: &[String],
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::group::set_313 as m;
    // Cinder takes both lists as one comma-separated string.
    let list = |ids: &[String]| {
        Some(
            ids.iter()
                .map(|i| i.trim())
                .filter(|i| !i.is_empty())
                .collect::<Vec<_>>()
                .join(","),
        )
        .filter(|s| !s.is_empty())
    };
    let (add, remove) = (list(add_volume_ids), list(remove_volume_ids));
    if add.is_none() && remove.is_none() {
        return Err(AppError::invalid("no volumes to add or remove"));
    }
    let mut b = m::GroupBuilder::default();
    if let Some(a) = add {
        b.add_volumes(Some(Cow::from(a)));
    }
    if let Some(r) = remove {
        b.remove_volumes(Some(Cow::from(r)));
    }
    x.send(
        m::Request::builder()
            .id(non_empty(id, "group")?)
            .group(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `GroupSnapshotCreate`: `POST /group_snapshots` (3.14), a consistent snapshot of every volume in
/// the group.
pub(super) async fn group_snapshot_create(
    x: &mut Exec<'_>,
    group_id: &str,
    name: &str,
    description: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::group_snapshot::create_314 as m;
    let mut b = m::GroupSnapshotBuilder::default();
    b.group_id(non_empty(group_id, "group")?)
        .name(Some(Cow::from(non_empty(name, "name")?)));
    if let Some(d) = opt(description) {
        b.description(Some(Cow::from(d)));
    }
    x.create(
        m::Request::builder()
            .group_snapshot(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "group snapshot",
    )
    .await?;
    Ok(())
}

/// `GroupSnapshotDelete`: `DELETE /group_snapshots/{id}` (3.14).
pub(super) async fn group_snapshot_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::block_storage::v3::group_snapshot::delete_314 as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `BackupCreate`: `POST /backups` (3.51), optionally incremental, forced (attached volume) or
/// from a snapshot. `incremental` and `force` are only sent when set.
pub(super) async fn backup_create(
    x: &mut Exec<'_>,
    volume_id: &str,
    name: &str,
    description: &Option<String>,
    incremental: &bool,
    force: &bool,
    snapshot_id: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::backup::create_351 as m;
    let mut b = m::BackupBuilder::default();
    b.volume_id(non_empty(volume_id, "volume")?)
        .name(Some(Cow::from(non_empty(name, "name")?)));
    if let Some(d) = opt(description) {
        b.description(Some(Cow::from(d)));
    }
    // Only sent when set: Cinder rejects `incremental` on a first backup
    // of the volume, and `force` is only needed for attached volumes.
    if *incremental {
        b.incremental(true);
    }
    if *force {
        b.force(true);
    }
    if let Some(sn) = opt(snapshot_id) {
        b.snapshot_id(Some(Cow::from(sn)));
    }
    x.create(
        m::Request::builder()
            .backup(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "backup",
    )
    .await?;
    Ok(())
}

/// `BackupUpdate`: `PUT /backups/{id}` (3.9, the first that allows it) with the name and
/// description.
pub(super) async fn backup_update(x: &mut Exec<'_>, id: &str, name: &str, description: &Option<String>) -> R<()> {
    use openstack_sdk::api::block_storage::v3::backup::set_39 as m;
    // An empty description is sent as `""` so clearing it works.
    let body = m::BackupBuilder::default()
        .name(Some(Cow::from(non_empty(name, "name")?)))
        .description(Some(Cow::from(opt(description).unwrap_or(""))))
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(non_empty(id, "backup")?)
            .backup(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `BackupRestore`: `POST /backups/{id}/restore`, over `volume_id` when given (its data is
/// replaced), else into a new volume named `name`.
pub(super) async fn backup_restore(
    x: &mut Exec<'_>,
    id: &str,
    volume_id: &Option<String>,
    name: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::block_storage::v3::backup::restore::create as m;
    let mut b = m::RestoreBuilder::default();
    // `volume_id` restores over that volume; else Cinder makes a new one (`name`).
    if let Some(v) = opt(volume_id) {
        b.volume_id(Some(Cow::from(v)));
    } else if let Some(n) = opt(name) {
        b.name(Some(Cow::from(n)));
    }
    x.send(
        m::Request::builder()
            .id(non_empty(id, "backup")?)
            .restore(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `BackupDelete`: `DELETE /backups/{id}`.
pub(super) async fn backup_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::block_storage::v3::backup::delete as m;
    x.send(
        m::Request::builder()
            .id(non_empty(id, "backup")?)
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

    #[tokio::test]
    async fn volumes_and_snapshots() {
        let r = dry(OsAction::VolumeCreate {
            name: id("data"),
            description: None,
            size_gb: 10,
            volume_type: Some(id("ssd")),
            availability_zone: None,
            source: VolumeSource::Image,
            source_id: Some(id("img")),
        })
        .await;
        let v = &body(&r[0])["volume"];
        assert_eq!(
            (v["size"].clone(), v["imageRef"].clone(), v["volume_type"].clone()),
            (json!(10), json!("img"), json!("ssd"))
        );
        assert!(v.get("availability_zone").is_none());

        let mut x = Exec::new(WriteMode::DryRun, None);
        let no_src = OsAction::VolumeCreate {
            name: id("x"),
            description: None,
            size_gb: 1,
            volume_type: None,
            availability_zone: None,
            source: VolumeSource::Snapshot,
            source_id: None,
        };
        assert!(apply(&mut x, &no_src).await.is_err());

        let r = dry(OsAction::VolumeExtend {
            id: id("v"),
            new_size_gb: 20,
        })
        .await;
        assert_eq!(body(&r[0]), json!({"os-extend": {"new_size": 20}}));
        let r = dry(OsAction::VolumeSnapshotCreate {
            volume_id: id("v"),
            name: id("s"),
            description: None,
            force: true,
        })
        .await;
        assert_eq!(body(&r[0])["snapshot"]["force"], true);
        let r = dry(OsAction::SnapshotDelete { id: id("s") }).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
            ("DELETE", "/snapshots/s", "block-storage")
        );
    }

    #[tokio::test]
    async fn volume_backups_are_created_edited_restored_and_deleted() {
        let r = dry(OsAction::BackupCreate {
            volume_id: id("v1"),
            name: id("nightly"),
            description: None,
            incremental: false,
            force: false,
            snapshot_id: None,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
            ("POST", "/backups", "block-storage")
        );
        assert_eq!(
            body(&r[0]),
            json!({"backup": {"name": "nightly", "volume_id": "v1"}}),
            "flags are left out unless set"
        );
        let r = dry(OsAction::BackupCreate {
            volume_id: id("v1"),
            name: id("n"),
            description: Some("d".into()),
            incremental: true,
            force: true,
            snapshot_id: Some("s1".into()),
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"backup": {"name": "n", "description": "d", "incremental": true, "force": true, "snapshot_id": "s1", "volume_id": "v1"}})
        );
        let r = dry(OsAction::BackupUpdate {
            id: id("b1"),
            name: id("n2"),
            description: None,
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/backups/b1"));
        assert_eq!(
            body(&r[0]),
            json!({"backup": {"name": "n2", "description": ""}}),
            "an empty description clears it"
        );
        let r = dry(OsAction::BackupRestore {
            id: id("b1"),
            volume_id: None,
            name: Some("copy".into()),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("POST", "/backups/b1/restore")
        );
        assert_eq!(body(&r[0]), json!({"restore": {"name": "copy"}}));
        let r = dry(OsAction::BackupRestore {
            id: id("b1"),
            volume_id: Some("v9".into()),
            name: Some("ignored".into()),
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"restore": {"volume_id": "v9"}}),
            "restoring over a volume sends no name"
        );
        let r = dry(OsAction::BackupDelete { id: id("b1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/backups/b1"));
        assert_eq!(OsAction::BackupDelete { id: id("b1") }.plugins(), [Plugin::Backup]);
    }

    #[tokio::test]
    async fn volume_groups_and_group_snapshots_are_deleted() {
        let r = dry(OsAction::VolumeGroupDelete {
            id: id("g1"),
            delete_volumes: true,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
            ("POST", "/groups/g1/action", "block-storage"),
            "with the group's ID, unlike the SDK's delete_313"
        );
        assert_eq!(body(&r[0]), json!({"delete": {"delete-volumes": true}}));
        let r = dry(OsAction::VolumeGroupDelete {
            id: id("g1"),
            delete_volumes: false,
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"delete": {"delete-volumes": false}}),
            "sent explicitly: Cinder then refuses a group with volumes"
        );
        let req = volume_group_delete("g1", false).unwrap();
        assert_eq!(
            req.headers.as_ref().unwrap()["openstack-api-version"],
            "volume 3.13",
            "group actions need 3.13"
        );
        let r = dry(OsAction::GroupSnapshotDelete { id: id("gs1") }).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/group_snapshots/gs1")
        );
    }

    #[tokio::test]
    async fn volume_group_writes() {
        let create = |types: &[&str], az: Option<&str>| OsAction::VolumeGroupCreate {
            name: id(" db "),
            description: Some(id("")),
            group_type_id: id("gt1"),
            volume_type_ids: types.iter().map(|t| t.to_string()).collect(),
            availability_zone: az.map(str::to_string),
        };
        let r = dry(create(&["vt1", "vt2"], Some("nova"))).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/groups"));
        assert_eq!(
            body(&r[0]),
            json!({"group": {"name": "db", "group_type": "gt1", "volume_types": ["vt1", "vt2"], "availability_zone": "nova"}})
        );
        assert!(
            body(&dry(create(&["vt1"], None)).await[0])["group"]
                .get("availability_zone")
                .is_none(),
            "no zone: Cinder picks"
        );
        assert!(
            dry_err(create(&[], None)).await,
            "Cinder needs at least one volume type"
        );

        let from = |g: Option<&str>, s: Option<&str>| OsAction::VolumeGroupCreateFrom {
            name: id("copy"),
            description: Some(id("clone")),
            source_group_id: g.map(str::to_string),
            group_snapshot_id: s.map(str::to_string),
        };
        let r = dry(from(Some("g1"), None)).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/groups/action"));
        assert_eq!(
            body(&r[0]),
            json!({"create-from-src": {"name": "copy", "description": "clone", "source_group_id": "g1"}})
        );
        assert_eq!(
            body(&dry(from(None, Some("gs1"))).await[0]),
            json!({"create-from-src": {"name": "copy", "description": "clone", "group_snapshot_id": "gs1"}})
        );
        assert!(
            dry_err(from(None, None)).await && dry_err(from(Some("g1"), Some("gs1"))).await,
            "exactly one source"
        );

        let r = dry(OsAction::VolumeGroupUpdate {
            id: id("g1"),
            name: id("db2"),
            description: None,
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/groups/g1"));
        assert_eq!(
            body(&r[0]),
            json!({"group": {"name": "db2", "description": ""}}),
            "an empty description clears it"
        );

        let set = |add: &[&str], remove: &[&str]| OsAction::VolumeGroupSetVolumes {
            id: id("g1"),
            add_volume_ids: add.iter().map(|t| t.to_string()).collect(),
            remove_volume_ids: remove.iter().map(|t| t.to_string()).collect(),
        };
        let r = dry(set(&["v1", "v2"], &["v3"])).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PUT", "/groups/g1"));
        assert_eq!(
            body(&r[0]),
            json!({"group": {"add_volumes": "v1,v2", "remove_volumes": "v3"}}),
            "Cinder's comma-separated strings"
        );
        assert_eq!(
            body(&dry(set(&[], &["v3"])).await[0]),
            json!({"group": {"remove_volumes": "v3"}}),
            "an empty list is left out"
        );
        assert!(dry_err(set(&[], &[])).await);

        let r = dry(OsAction::GroupSnapshotCreate {
            group_id: id("g1"),
            name: id("before-upgrade"),
            description: None,
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/group_snapshots"));
        assert_eq!(
            body(&r[0]),
            json!({"group_snapshot": {"group_id": "g1", "name": "before-upgrade"}})
        );
    }

    #[tokio::test]
    async fn volume_actions_parity() {
        let r = dry(OsAction::VolumeRetype {
            id: id("v"),
            new_type: id("fast"),
            migrate: true,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("POST", "/volumes/v/action")
        );
        assert_eq!(
            body(&r[0]),
            json!({"os-retype": {"new_type": "fast", "migration_policy": "on-demand"}})
        );
        let r = dry(OsAction::VolumeRetype {
            id: id("v"),
            new_type: id("fast"),
            migrate: false,
        })
        .await;
        assert_eq!(body(&r[0])["os-retype"]["migration_policy"], "never");
        let up = |format: &str, visibility: Option<&str>| OsAction::VolumeUploadImage {
            id: id("v"),
            image_name: id("img"),
            disk_format: id(format),
            force: true,
            visibility: visibility.map(id),
        };
        let r = dry(up("qcow2", None)).await;
        assert_eq!(
            body(&r[0]),
            json!({"os-volume_upload_image": {"image_name": "img", "disk_format": "qcow2", "container_format": "bare", "force": true}})
        );
        let r = dry(up("raw", Some("community"))).await;
        assert_eq!(body(&r[0])["os-volume_upload_image"]["visibility"], "community");
        assert!(dry_err(up("iso", None)).await);
        assert!(dry_err(up("raw", Some("everyone"))).await);
        assert!(
            dry_err(OsAction::VolumeRetype {
                id: id("v"),
                new_type: id(" "),
                migrate: false
            })
            .await
        );
    }

    #[tokio::test]
    async fn volume_transfers() {
        let mut x = Exec::new(WriteMode::DryRun, None);
        apply(
            &mut x,
            &OsAction::VolumeTransferCreate {
                volume_id: id("v"),
                name: Some(id("to ops")),
            },
        )
        .await
        .unwrap();
        let out = x.outcome();
        assert_eq!(
            (out.requests[0].method.as_str(), out.requests[0].path.as_str()),
            ("POST", "/os-volume-transfer")
        );
        assert_eq!(
            body(&out.requests[0]),
            json!({"transfer": {"volume_id": "v", "name": "to ops"}})
        );
        assert!(out.secret.is_none(), "dry-run creates no key");
        let r = dry(OsAction::VolumeTransferAccept {
            transfer_id: id("t"),
            auth_key: id("k3y"),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("POST", "/os-volume-transfer/t/accept")
        );
        assert_eq!(body(&r[0]), json!({"accept": {"auth_key": "k3y"}}));
        assert!(
            dry_err(OsAction::VolumeTransferAccept {
                transfer_id: id("t"),
                auth_key: id("")
            })
            .await
        );
    }
}
