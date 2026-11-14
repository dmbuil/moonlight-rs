//! Nova writes: instances (and the launch), key pairs, server groups.

use super::*;

/// `ServerLaunch`: `POST /servers` (2.19, for `description`), following Horizon's launch wizard.
/// The boot source becomes `imageRef` or a `block_device_mapping_v2` entry (a new volume from an
/// image, an existing volume or a volume snapshot); networks and ports go in `networks`,
/// security groups by name, the server group as a scheduler hint. Fails on an empty name, flavor
/// or source, a count outside 1–100 or user data over 16 KB.
pub(super) async fn launch(x: &mut Exec<'_>, s: &LaunchSpec) -> R<()> {
    // 2.19 adds `description`; everything else is as in 2.1.
    use openstack_sdk::api::compute::v2::server::create_219 as m;
    let name = non_empty(&s.name, "name")?;
    let source_id = non_empty(&s.source_id, "boot source")?;
    if s.count == 0 || s.count > 100 {
        return Err(AppError::invalid("count must be between 1 and 100"));
    }
    let mut b = m::ServerBuilder::default();
    b.name(name).flavor_ref(non_empty(&s.flavor_id, "flavor")?);
    if s.count > 1 {
        b.min_count(s.count as i32).max_count(s.count as i32);
    }
    if let Some(z) = opt(&s.availability_zone) {
        b.availability_zone(z.to_string());
    }
    if let Some(k) = opt(&s.key_name) {
        b.key_name(k.to_string());
    }
    if let Some(d) = opt(&s.description) {
        b.description(Some(Cow::from(d.to_string())));
    }
    if let Some(u) = s.user_data.as_deref().filter(|u| !u.trim().is_empty()) {
        if u.len() > MAX_USER_DATA_BYTES {
            return Err(AppError::invalid("the customization script is larger than 16 KB"));
        }
        use base64::Engine;
        b.user_data(base64::engine::general_purpose::STANDARD.encode(u));
    }
    if s.config_drive {
        b.config_drive(true);
    }
    if let Some(p) = s.admin_pass.as_deref().filter(|p| !p.is_empty()) {
        b.admin_pass(p);
    }
    b.os_dcf_disk_config(if s.manual_disk_partition {
        m::OsDcfDiskConfig::Manual
    } else {
        m::OsDcfDiskConfig::Auto
    });
    // Boot source, following Horizon's launch wizard.
    let boot_volume = |src: m::SourceType, size: Option<u32>, delete: bool| {
        let mut bd = m::BlockDeviceMappingV2Builder::default();
        bd.boot_index(Some(Cow::from("0")))
            .uuid(source_id.to_string())
            .source_type(src)
            .destination_type(m::DestinationType::Volume)
            .delete_on_termination(delete);
        if let Some(gb) = size {
            bd.volume_size(gb as i32);
        }
        bd.build().map_err(build_err)
    };
    match s.source {
        BootSource::Image if s.create_volume => {
            let size = s
                .volume_size_gb
                .filter(|g| *g > 0)
                .ok_or_else(|| AppError::invalid("volume size is required"))?;
            b.block_device_mapping_v2(vec![boot_volume(
                m::SourceType::Image,
                Some(size),
                s.delete_volume_on_terminate,
            )?]);
        }
        BootSource::Image => {
            b.image_ref(source_id.to_string());
        }
        BootSource::Volume => {
            b.block_device_mapping_v2(vec![boot_volume(
                m::SourceType::Volume,
                None,
                s.delete_volume_on_terminate,
            )?]);
        }
        BootSource::VolumeSnapshot => {
            b.block_device_mapping_v2(vec![boot_volume(
                m::SourceType::Snapshot,
                s.volume_size_gb.filter(|g| *g > 0),
                s.delete_volume_on_terminate,
            )?]);
        }
    }
    let nets = s
        .network_ids
        .iter()
        .filter(|n| !n.trim().is_empty())
        .map(|n| {
            m::NetworksBuilder::default()
                .uuid(n.trim().to_string())
                .build()
                .map_err(build_err)
        })
        .chain(s.port_ids.iter().filter(|p| !p.trim().is_empty()).map(|p| {
            m::NetworksBuilder::default()
                .port(Some(Cow::from(p.trim().to_string())))
                .build()
                .map_err(build_err)
        }))
        .collect::<R<Vec<_>>>()?;
    if !nets.is_empty() {
        b.networks(nets);
    }
    let sgs = s
        .security_groups
        .iter()
        .filter(|g| !g.trim().is_empty())
        .map(|g| {
            m::SecurityGroupsBuilder::default()
                .name(g.trim().to_string())
                .build()
                .map_err(build_err)
        })
        .collect::<R<Vec<_>>>()?;
    if !sgs.is_empty() {
        b.security_groups(sgs);
    }
    let mut req = m::Request::builder();
    req.server(b.build().map_err(build_err)?);
    if let Some(g) = opt(&s.server_group_id) {
        let hints = m::OsSchedulerHintsBuilder::default()
            .group(vec![Cow::from(g.to_string())])
            .build()
            .map_err(build_err)?;
        req.os_scheduler_hints(hints);
    }
    x.create(req.build().map_err(build_err)?, "server").await?;
    Ok(())
}

/// `ServerUpdate`: renames the instance, `PUT /servers/{id}`.
pub(super) async fn server_update(x: &mut Exec<'_>, id: &str, name: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::set_21 as m;
    let body = m::ServerBuilder::default()
        .name(non_empty(name, "name")?)
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).server(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `ServerStart`: `POST /servers/{id}/action` with `os-start`.
pub(super) async fn server_start(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::os_start as m;
    x.send(
        m::Request::builder()
            .id(id)
            .os_start(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerStop`: `POST /servers/{id}/action` with `os-stop`.
pub(super) async fn server_stop(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::os_stop as m;
    x.send(
        m::Request::builder()
            .id(id)
            .os_stop(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerReboot`: `POST /servers/{id}/action` with `reboot`, soft or hard.
pub(super) async fn server_reboot(x: &mut Exec<'_>, id: &str, kind: &RebootKind) -> R<()> {
    use openstack_sdk::api::compute::v2::server::reboot as m;
    let t = match kind {
        RebootKind::Soft => m::Type::Soft,
        RebootKind::Hard => m::Type::Hard,
    };
    let body = m::RebootBuilder::default()._type(t).build().map_err(build_err)?;
    x.send(m::Request::builder().id(id).reboot(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `ServerPause`: `POST /servers/{id}/action` with `pause`.
pub(super) async fn server_pause(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::pause as m;
    x.send(
        m::Request::builder()
            .id(id)
            .pause(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerUnpause`: `POST /servers/{id}/action` with `unpause`.
pub(super) async fn server_unpause(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::unpause as m;
    x.send(
        m::Request::builder()
            .id(id)
            .unpause(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerSuspend`: `POST /servers/{id}/action` with `suspend`.
pub(super) async fn server_suspend(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::suspend as m;
    x.send(
        m::Request::builder()
            .id(id)
            .suspend(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerResume`: `POST /servers/{id}/action` with `resume`.
pub(super) async fn server_resume(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::resume as m;
    x.send(
        m::Request::builder()
            .id(id)
            .resume(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerLock`: `POST /servers/{id}/action` with `lock`.
pub(super) async fn server_lock(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::lock_21 as m;
    x.send(
        m::Request::builder()
            .id(id)
            .lock(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerUnlock`: `POST /servers/{id}/action` with `unlock`.
pub(super) async fn server_unlock(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::unlock_21 as m;
    x.send(
        m::Request::builder()
            .id(id)
            .unlock(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerSnapshot`: `POST /servers/{id}/action` with `createImage`; Nova makes the Glance image
/// (or, for a volume-backed instance, volume snapshots behind it).
pub(super) async fn server_snapshot(x: &mut Exec<'_>, id: &str, name: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::create_image_21 as m;
    let body = m::CreateImageBuilder::default()
        .name(non_empty(name, "name")?)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(id)
            .create_image(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerDelete`: `DELETE /servers/{id}`.
pub(super) async fn server_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

/// `ServerSetMetadata`: replaces all the instance's metadata, `PUT /servers/{id}/metadata`; keys
/// and values are checked by `checked_metadata` first.
pub(super) async fn server_set_metadata(x: &mut Exec<'_>, id: &str, metadata: &[(String, String)]) -> R<()> {
    use openstack_sdk::api::compute::v2::server::metadata::replace as m;
    let md = checked_metadata(metadata)?;
    x.send(
        m::Request::builder()
            .server_id(id)
            .metadata(md.into_iter())
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerSetSecurityGroups`: one `addSecurityGroup` / `removeSecurityGroup` action per group
/// (`POST /servers/{id}/action`, by name), each recorded as an `ItemResult` labelled `+ name` or
/// `- name`, so one failure doesn't hide the others. Fails if both lists are empty.
pub(super) async fn server_set_security_groups(x: &mut Exec<'_>, id: &str, add: &[String], remove: &[String]) -> R<()> {
    use openstack_sdk::api::compute::v2::server::{add_security_group as a, remove_security_group as r};
    if add.is_empty() && remove.is_empty() {
        return Err(AppError::invalid("nothing changed"));
    }
    // One Nova action per group, each recorded like a batch item, so a
    // failure (e.g. a port with port security off) names its group.
    for name in add {
        let body = a::AddSecurityGroupBuilder::default()
            .name(non_empty(name, "security group")?)
            .build()
            .map_err(build_err)?;
        let res = x
            .send(
                a::Request::builder()
                    .id(id)
                    .add_security_group(body)
                    .build()
                    .map_err(build_err)?,
            )
            .await;
        x.results.push(ItemResult {
            label: format!("+ {name}"),
            error: res.err().map(|e| e.to_string()),
        });
    }
    for name in remove {
        let body = r::RemoveSecurityGroupBuilder::default()
            .name(non_empty(name, "security group")?)
            .build()
            .map_err(build_err)?;
        let res = x
            .send(
                r::Request::builder()
                    .id(id)
                    .remove_security_group(body)
                    .build()
                    .map_err(build_err)?,
            )
            .await;
        x.results.push(ItemResult {
            label: format!("- {name}"),
            error: res.err().map(|e| e.to_string()),
        });
    }
    Ok(())
}

/// `ServerAttachVolume`: `POST /servers/{server_id}/os-volume_attachments`; Nova picks the device.
pub(super) async fn server_attach_volume(x: &mut Exec<'_>, server_id: &str, volume_id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::volume_attachment::create_20 as m;
    let body = m::VolumeAttachmentBuilder::default()
        .volume_id(volume_id)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .server_id(server_id)
            .volume_attachment(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerDetachVolume`: `DELETE /servers/{server_id}/os-volume_attachments/{volume_id}`.
pub(super) async fn server_detach_volume(x: &mut Exec<'_>, server_id: &str, volume_id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::volume_attachment::delete as m;
    x.send(
        m::Request::builder()
            .server_id(server_id)
            .id(volume_id)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerResize`: `POST /servers/{id}/action` with `resize`; the instance then waits in
/// `VERIFY_RESIZE` for [`server_confirm_resize`] or [`server_revert_resize`].
pub(super) async fn server_resize(x: &mut Exec<'_>, id: &str, flavor_id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::resize as m;
    let body = m::ResizeBuilder::default()
        .flavor_ref(non_empty(flavor_id, "flavor")?)
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).resize(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `ServerConfirmResize`: `POST /servers/{id}/action` with `confirmResize`.
pub(super) async fn server_confirm_resize(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::confirm_resize as m;
    x.send(
        m::Request::builder()
            .id(id)
            .confirm_resize(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerRevertResize`: `POST /servers/{id}/action` with `revertResize`.
pub(super) async fn server_revert_resize(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::revert_resize as m;
    x.send(
        m::Request::builder()
            .id(id)
            .revert_resize(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerRebuild`: `POST /servers/{id}/action` with `rebuild` (2.94; from 2.93 Nova also rebuilds a
/// volume-backed instance) from `image_id`. Destroys the root disk's data.
pub(super) async fn server_rebuild(x: &mut Exec<'_>, id: &str, image_id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::rebuild_294 as m;
    let body = m::RebuildBuilder::default()
        .image_ref(non_empty(image_id, "image")?)
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().id(id).rebuild(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `ServerShelve`: `POST /servers/{id}/action` with `shelve`. Unshelve is built inline in
/// `apply` (its body depends on the microversion).
pub(super) async fn server_shelve(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::shelve as m;
    x.send(
        m::Request::builder()
            .id(id)
            .shelve(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerRescue`: `POST /servers/{id}/action` with `rescue`, optionally from another image.
/// Nova generates the rescue password and returns it once: it is handed back as a `Secret`
/// (`SecretKind::Password`) to show, like a generated key.
pub(super) async fn server_rescue(x: &mut Exec<'_>, id: &str, image_id: &Option<String>) -> R<()> {
    use openstack_sdk::api::compute::v2::server::rescue as m;
    let mut b = m::RescueBuilder::default();
    if let Some(img) = opt(image_id) {
        b.rescue_image_ref(img);
    }
    let body = b.build().map_err(build_err)?;
    let resp = x
        .call(m::Request::builder().id(id).rescue(body).build().map_err(build_err)?)
        .await?;
    // Nova generates the password; shown once (like a generated key).
    if let Some(pass) = resp.as_ref().and_then(|v| v.get("adminPass")).and_then(Value::as_str) {
        x.secret = Some(Secret {
            file_name: format!("rescue-{id}.txt"),
            content: pass.to_string(),
            kind: SecretKind::Password,
            fields: vec![],
            extra: vec![],
        });
    }
    Ok(())
}

/// `ServerUnrescue`: `POST /servers/{id}/action` with `unrescue`.
pub(super) async fn server_unrescue(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::unrescue as m;
    x.send(
        m::Request::builder()
            .id(id)
            .unrescue(Value::Null)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerAttachInterface`: `POST /servers/{id}/os-interface` with either a network (and an
/// optional fixed IP) or an existing port, never both.
pub(super) async fn server_attach_interface(
    x: &mut Exec<'_>,
    id: &str,
    network_id: &Option<String>,
    port_id: &Option<String>,
    fixed_ip: &Option<String>,
) -> R<()> {
    use openstack_sdk::api::compute::v2::server::interface::create_20 as m;
    let mut b = m::InterfaceAttachmentBuilder::default();
    match (opt(network_id), opt(port_id)) {
        (Some(net), None) => {
            b.net_id(net);
            if let Some(ip) = opt(fixed_ip) {
                b.fixed_ips(vec![
                    m::FixedIpsBuilder::default()
                        .ip_address(ip)
                        .build()
                        .map_err(build_err)?,
                ]);
            }
        }
        (None, Some(port)) => {
            b.port_id(port);
        }
        _ => return Err(AppError::invalid("choose either a network or a port")),
    }
    let body = b.build().map_err(build_err)?;
    x.send(
        m::Request::builder()
            .server_id(id)
            .interface_attachment(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `ServerDetachInterface`: `DELETE /servers/{id}/os-interface/{port_id}`.
pub(super) async fn server_detach_interface(x: &mut Exec<'_>, id: &str, port_id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server::interface::delete as m;
    x.send(
        m::Request::builder()
            .server_id(id)
            .id(port_id)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

/// `KeyPairImport`: `POST /os-keypairs` (2.10, for `type`) with the user's public key, checked by
/// `model::check_public_key` first. `key_type` is `ssh` or `x509`.
pub(super) async fn key_pair_import(x: &mut Exec<'_>, name: &str, public_key: &str, key_type: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::keypair::create_210 as m;
    let public_key = non_empty(public_key, "public key")?;
    crate::model::check_public_key(key_type, public_key).map_err(AppError::invalid)?;
    let body = m::KeypairBuilder::default()
        .name(non_empty(name, "name")?)
        .public_key(public_key)
        ._type(if key_type == "x509" {
            m::Type::X509
        } else {
            m::Type::Ssh
        })
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().keypair(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `KeyPairGenerate`: makes an Ed25519 key here and sends only its public half, `POST
/// /os-keypairs` (2.10), as Nova stopped generating keys in 2.92. The private key is returned
/// once as a `Secret` (`{name}.pem`), and only when the write really happened (not in dry-run).
pub(super) async fn key_pair_generate(x: &mut Exec<'_>, name: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::keypair::create_210 as m;
    use ssh_key::{Algorithm, LineEnding, PrivateKey, rand_core::OsRng};
    let name = non_empty(name, "name")?;
    let key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).map_err(sfe("generate key"))?;
    let public_key = key.public_key().to_openssh().map_err(sfe("encode public key"))?;
    let body = m::KeypairBuilder::default()
        .name(name)
        .public_key(public_key.as_str())
        ._type(m::Type::Ssh)
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().keypair(body).build().map_err(build_err)?)
        .await?;
    // Only once Nova accepted the public key, and never in dry-run.
    if x.mode == WriteMode::On {
        let pem = key.to_openssh(LineEnding::LF).map_err(sfe("encode private key"))?;
        x.secret = Some(Secret {
            file_name: format!("{name}.pem"),
            content: pem.to_string(),
            kind: SecretKind::PrivateKey,
            fields: vec![],
            extra: vec![],
        });
    }
    Ok(())
}

/// `KeyPairDelete`: `DELETE /os-keypairs/{name}` (key pairs are identified by name).
pub(super) async fn key_pair_delete(x: &mut Exec<'_>, name: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::keypair::delete_20 as m;
    x.send(m::Request::builder().id(name).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `ServerGroupCreate`: `POST /os-server-groups` (2.64, one `policy` string rather than a list).
/// `policy` is `affinity`, `anti-affinity`, `soft-affinity` or `soft-anti-affinity`.
pub(super) async fn server_group_create(x: &mut Exec<'_>, name: &str, policy: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server_group::create_264 as m;
    let p = match policy {
        "affinity" => m::Policy::Affinity,
        "anti-affinity" => m::Policy::AntiAffinity,
        "soft-affinity" => m::Policy::SoftAffinity,
        "soft-anti-affinity" => m::Policy::SoftAntiAffinity,
        _ => return Err(AppError::invalid("invalid policy")),
    };
    let body = m::ServerGroupBuilder::default()
        .name(non_empty(name, "name")?)
        .policy(p)
        .build()
        .map_err(build_err)?;
    x.send(m::Request::builder().server_group(body).build().map_err(build_err)?)
        .await?;
    Ok(())
}

/// `ServerGroupDelete`: `DELETE /os-server-groups/{id}`.
pub(super) async fn server_group_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::compute::v2::server_group::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[tokio::test]
    async fn off_mode_rejects_without_building() {
        let mut x = Exec::new(WriteMode::Off, None);
        assert!(apply(&mut x, &OsAction::ServerDelete { id: id("s1") }).await.is_err());
        assert!(x.outcome().requests.is_empty());
    }

    #[tokio::test]
    async fn dry_run_never_needs_a_client() {
        // `os` is None: if dry-run tried to send, it would fail with "no OpenStack connection".
        let r = dry(OsAction::ServerDelete { id: id("s1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/servers/s1"));
        assert_eq!(r[0].service, "compute");
    }

    #[tokio::test]
    async fn server_power_actions() {
        let cases = [
            (OsAction::ServerStart { id: id("s") }, "os-start"),
            (OsAction::ServerStop { id: id("s") }, "os-stop"),
            (OsAction::ServerPause { id: id("s") }, "pause"),
            (OsAction::ServerUnpause { id: id("s") }, "unpause"),
            (OsAction::ServerSuspend { id: id("s") }, "suspend"),
            (OsAction::ServerResume { id: id("s") }, "resume"),
            (OsAction::ServerLock { id: id("s") }, "lock"),
            (OsAction::ServerUnlock { id: id("s") }, "unlock"),
        ];
        for (action, key) in cases {
            let r = dry(action).await;
            assert_eq!(
                (r[0].method.as_str(), r[0].path.as_str()),
                ("POST", "/servers/s/action")
            );
            assert!(body(&r[0]).get(key).is_some(), "{key}: {:?}", r[0].body);
        }
        let r = dry(OsAction::ServerReboot {
            id: id("s"),
            kind: RebootKind::Hard,
        })
        .await;
        assert_eq!(body(&r[0]), json!({"reboot": {"type": "HARD"}}));
    }

    #[tokio::test]
    async fn server_rename_snapshot_and_volumes() {
        let r = dry(OsAction::ServerUpdate {
            id: id("s"),
            name: id(" web "),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), body(&r[0])),
            ("PUT", json!({"server": {"name": "web"}}))
        );
        let r = dry(OsAction::ServerSnapshot {
            id: id("s"),
            name: id("snap"),
        })
        .await;
        assert_eq!(body(&r[0])["createImage"]["name"], "snap");
        let r = dry(OsAction::ServerSetSecurityGroups {
            id: id("s"),
            add: vec![id("web")],
            remove: vec![id("default")],
        })
        .await;
        assert_eq!(
            (r.len(), r[0].path.as_str(), r[1].path.as_str()),
            (2, "/servers/s/action", "/servers/s/action")
        );
        assert_eq!(
            body(&r[0]),
            json!({"addSecurityGroup": {"name": "web"}}),
            "Nova takes groups by name"
        );
        assert_eq!(body(&r[1]), json!({"removeSecurityGroup": {"name": "default"}}));
        assert!(
            dry_err(OsAction::ServerSetSecurityGroups {
                id: id("s"),
                add: vec![],
                remove: vec![]
            })
            .await,
            "an empty change is refused"
        );
        let r = dry(OsAction::ServerAttachVolume {
            server_id: id("s"),
            volume_id: id("v"),
        })
        .await;
        assert_eq!(r[0].path, "/servers/s/os-volume_attachments");
        assert_eq!(body(&r[0])["volumeAttachment"]["volumeId"], "v");
        let r = dry(OsAction::ServerDetachVolume {
            server_id: id("s"),
            volume_id: id("v"),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/servers/s/os-volume_attachments/v")
        );
    }

    fn launch_spec() -> LaunchSpec {
        LaunchSpec {
            name: "web".into(),
            availability_zone: Some("alh1".into()),
            count: 2,
            source: BootSource::Image,
            source_id: "img".into(),
            create_volume: true,
            volume_size_gb: Some(50),
            delete_volume_on_terminate: true,
            flavor_id: "f1".into(),
            network_ids: vec!["n1".into(), "".into()],
            security_groups: vec!["default".into()],
            key_name: Some("ops".into()),
            server_group_id: Some("g1".into()),
            port_ids: vec![],
            description: None,
            user_data: None,
            config_drive: false,
            manual_disk_partition: false,
            admin_pass: None,
        }
    }

    #[tokio::test]
    async fn launch_admin_password_is_sent_but_never_previewed() {
        let spec = LaunchSpec {
            admin_pass: Some("s3cr3t-Pa55".into()),
            ..launch_spec()
        };
        let r = dry(OsAction::ServerLaunch(spec)).await;
        let preview = r[0].body.as_deref().unwrap();
        assert!(preview.contains("\"adminPass\": \"********\"") && !preview.contains("s3cr3t"));
        let none = dry(OsAction::ServerLaunch(launch_spec())).await;
        assert!(
            !none[0].body.as_deref().unwrap().contains("adminPass"),
            "not sent by default"
        );
    }

    #[tokio::test]
    async fn launch_boot_from_new_volume() {
        let r = dry(OsAction::ServerLaunch(launch_spec())).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("POST", "/servers"));
        let b = body(&r[0]);
        let s = &b["server"];
        assert_eq!(s["name"], "web");
        assert_eq!(s["flavorRef"], "f1");
        assert_eq!((s["min_count"].clone(), s["max_count"].clone()), (json!(2), json!(2)));
        assert!(s.get("imageRef").is_none(), "boot from volume must not set imageRef");
        let bdm = &s["block_device_mapping_v2"][0];
        assert_eq!(bdm["source_type"], "image");
        assert_eq!(bdm["destination_type"], "volume");
        assert_eq!(bdm["volume_size"], 50);
        assert_eq!(bdm["boot_index"], "0");
        assert_eq!(s["networks"], json!([{"uuid": "n1"}]), "empty network ids are dropped");
        assert_eq!(s["security_groups"], json!([{"name": "default"}]));
        assert_eq!(s["key_name"], "ops");
        assert_eq!(b["os:scheduler_hints"]["group"], json!(["g1"]));
        assert_eq!(s["OS-DCF:diskConfig"], "AUTO");
        assert!(s.get("user_data").is_none() && s.get("description").is_none());
    }

    #[tokio::test]
    async fn launch_with_ports_description_and_customization_script() {
        let mut spec = launch_spec();
        spec.port_ids = vec!["p1".into()];
        spec.description = Some(" web tier ".into());
        spec.user_data = Some("#cloud-config\n".into());
        spec.config_drive = true;
        spec.manual_disk_partition = true;
        let s = body(&dry(OsAction::ServerLaunch(spec.clone())).await[0])["server"].clone();
        assert_eq!(
            s["networks"],
            json!([{"uuid": "n1"}, {"port": "p1"}]),
            "networks first, then ports"
        );
        assert_eq!(s["description"], "web tier");
        assert_eq!(s["user_data"], "I2Nsb3VkLWNvbmZpZwo=");
        assert_eq!(
            (s["config_drive"].clone(), s["OS-DCF:diskConfig"].clone()),
            (json!(true), json!("MANUAL"))
        );

        spec.user_data = Some("x".repeat(MAX_USER_DATA_BYTES + 1));
        let mut x = Exec::new(WriteMode::DryRun, None);
        assert!(apply(&mut x, &OsAction::ServerLaunch(spec)).await.is_err());
    }

    #[tokio::test]
    async fn launch_from_image_without_volume_and_validation() {
        let mut spec = launch_spec();
        spec.create_volume = false;
        spec.count = 1;
        spec.server_group_id = None;
        let b = body(&dry(OsAction::ServerLaunch(spec.clone())).await[0]);
        assert_eq!(b["server"]["imageRef"], "img");
        assert!(b["server"].get("block_device_mapping_v2").is_none());
        assert!(b["server"].get("min_count").is_none());

        let mut x = Exec::new(WriteMode::DryRun, None);
        spec.name = "  ".into();
        assert!(apply(&mut x, &OsAction::ServerLaunch(spec.clone())).await.is_err());
        spec.name = "ok".into();
        spec.count = 0;
        assert!(apply(&mut x, &OsAction::ServerLaunch(spec)).await.is_err());
    }

    #[tokio::test]
    async fn keypairs_and_server_groups() {
        let import = |key: &str, kind: &str| OsAction::KeyPairImport {
            name: id("k"),
            public_key: id(key),
            key_type: id(kind),
        };
        let r = dry(import("ssh-ed25519 AAAA", "ssh")).await;
        assert_eq!(
            body(&r[0]),
            json!({"keypair": {"name": "k", "public_key": "ssh-ed25519 AAAA", "type": "ssh"}})
        );
        let pem = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----";
        let r = dry(import(pem, "x509")).await;
        assert_eq!(
            body(&r[0]),
            json!({"keypair": {"name": "k", "public_key": pem, "type": "x509"}})
        );
        assert!(dry_err(import(pem, "ssh")).await, "a certificate is not an SSH key");
        assert!(
            dry_err(import("ssh-ed25519 AAAA", "x509")).await,
            "an SSH key is not a certificate"
        );
        assert!(dry_err(import("ssh-ed25519 AAAA", "rsa")).await, "unknown type");
        let mut x = Exec::new(WriteMode::DryRun, None);
        apply(&mut x, &OsAction::KeyPairGenerate { name: id(" k ") })
            .await
            .unwrap();
        let out = x.outcome();
        assert!(out.secret.is_none(), "dry-run never generates a usable private key");
        let kp = &body(&out.requests[0])["keypair"];
        assert_eq!((kp["name"].as_str(), kp["type"].as_str()), (Some("k"), Some("ssh")));
        assert!(kp["public_key"].as_str().unwrap().starts_with("ssh-ed25519 "));
        assert!(!format!("{out:?}").contains("PRIVATE KEY"));
        let r = dry(OsAction::KeyPairDelete { name: id("k") }).await;
        assert_eq!(r[0].path, "/os-keypairs/k");
        let r = dry(OsAction::ServerGroupCreate {
            name: id("g"),
            policy: id("soft-anti-affinity"),
        })
        .await;
        assert_eq!(body(&r[0])["server_group"]["policy"], "soft-anti-affinity");
    }

    #[tokio::test]
    async fn batch_keeps_going_after_a_failing_item() {
        let mut x = Exec::new(WriteMode::DryRun, None);
        let action = OsAction::Batch(vec![
            (
                "bad".into(),
                OsAction::ServerUpdate {
                    id: id("s1"),
                    name: "  ".into(),
                },
            ),
            ("good".into(), OsAction::ServerDelete { id: id("s2") }),
        ]);
        apply(&mut x, &action).await.unwrap();
        let out = x.outcome();
        assert!(out.results[0].error.as_deref().unwrap().contains("name"));
        assert_eq!(out.results[1].error, None);
        assert_eq!(out.requests.last().unwrap().path, "/servers/s2");
    }

    #[tokio::test]
    async fn server_lifecycle_actions_parity() {
        let one = |a| async move {
            let r = dry(a).await;
            assert_eq!(
                (r[0].method.as_str(), r[0].path.as_str()),
                ("POST", "/servers/s/action")
            );
            body(&r[0])
        };
        assert_eq!(
            one(OsAction::ServerResize {
                id: id("s"),
                flavor_id: id("f2")
            })
            .await,
            json!({"resize": {"flavorRef": "f2"}})
        );
        assert_eq!(
            one(OsAction::ServerConfirmResize { id: id("s") }).await,
            json!({"confirmResize": null})
        );
        assert_eq!(
            one(OsAction::ServerRevertResize { id: id("s") }).await,
            json!({"revertResize": null})
        );
        assert_eq!(
            one(OsAction::ServerRebuild {
                id: id("s"),
                image_id: id("img")
            })
            .await,
            json!({"rebuild": {"imageRef": "img"}})
        );
        assert_eq!(
            one(OsAction::ServerShelve { id: id("s") }).await,
            json!({"shelve": null})
        );
        assert_eq!(
            one(OsAction::ServerUnshelve {
                id: id("s"),
                availability_zone: None
            })
            .await,
            json!({"unshelve": null})
        );
        assert_eq!(
            one(OsAction::ServerUnshelve {
                id: id("s"),
                availability_zone: Some(id("az1"))
            })
            .await,
            json!({"unshelve": {"availability_zone": "az1"}})
        );
        assert_eq!(
            one(OsAction::ServerRescue {
                id: id("s"),
                image_id: None
            })
            .await,
            json!({"rescue": {}})
        );
        assert_eq!(
            one(OsAction::ServerRescue {
                id: id("s"),
                image_id: Some(id("r"))
            })
            .await,
            json!({"rescue": {"rescue_image_ref": "r"}})
        );
        assert_eq!(
            one(OsAction::ServerUnrescue { id: id("s") }).await,
            json!({"unrescue": null})
        );
        assert!(
            dry_err(OsAction::ServerResize {
                id: id("s"),
                flavor_id: id(" ")
            })
            .await
        );
        assert!(
            dry_err(OsAction::ServerRebuild {
                id: id("s"),
                image_id: id("")
            })
            .await
        );
    }

    #[tokio::test]
    async fn rescue_in_dry_run_has_no_secret() {
        let mut x = Exec::new(WriteMode::DryRun, None);
        apply(
            &mut x,
            &OsAction::ServerRescue {
                id: id("s"),
                image_id: None,
            },
        )
        .await
        .unwrap();
        assert!(x.outcome().secret.is_none());
    }

    #[tokio::test]
    async fn interfaces_attach_by_network_or_port_and_detach() {
        let r = dry(OsAction::ServerAttachInterface {
            id: id("s"),
            network_id: Some(id("n")),
            port_id: None,
            fixed_ip: Some(id("10.0.0.9")),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("POST", "/servers/s/os-interface")
        );
        assert_eq!(
            body(&r[0]),
            json!({"interfaceAttachment": {"net_id": "n", "fixed_ips": [{"ip_address": "10.0.0.9"}]}})
        );
        let r = dry(OsAction::ServerAttachInterface {
            id: id("s"),
            network_id: None,
            port_id: Some(id("p")),
            fixed_ip: Some(id("ignored")),
        })
        .await;
        assert_eq!(body(&r[0]), json!({"interfaceAttachment": {"port_id": "p"}}));
        for (net, port) in [(None, None), (Some(id("n")), Some(id("p")))] {
            assert!(
                dry_err(OsAction::ServerAttachInterface {
                    id: id("s"),
                    network_id: net,
                    port_id: port,
                    fixed_ip: None
                })
                .await
            );
        }
        let r = dry(OsAction::ServerDetachInterface {
            id: id("s"),
            port_id: id("p"),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/servers/s/os-interface/p")
        );
    }
}
