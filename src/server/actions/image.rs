//! Glance writes: images, their properties and uploads.

use super::*;

/// Glance JSON-Patch for [`ImageChanges`]: `replace` for core fields, `add`
/// (adds or replaces) and `remove` for custom properties. Nothing changed is
/// an error, so no empty PATCH is sent.
fn image_patch(c: &ImageChanges) -> R<Vec<Value>> {
    let mut ops = Vec::new();
    let mut replace = |path: &str, value: Value| ops.push(json!({"op": "replace", "path": path, "value": value}));
    if let Some(n) = &c.name {
        replace("/name", json!(non_empty(n, "name")?));
    }
    if let Some(v) = &c.visibility {
        if !["public", "private", "shared", "community"].contains(&v.as_str()) {
            return Err(AppError::invalid("invalid visibility"));
        }
        replace("/visibility", json!(v));
    }
    if let Some(p) = c.protected {
        replace("/protected", json!(p));
    }
    if let Some(d) = c.min_disk_gb {
        replace("/min_disk", json!(d));
    }
    if let Some(r) = c.min_ram_mb {
        replace("/min_ram", json!(r));
    }
    if let Some(f) = &c.disk_format {
        replace("/disk_format", json!(non_empty(f, "disk format")?));
    }
    let set = checked_metadata(&c.set)?;
    for k in c.remove.iter().map(|k| k.trim()) {
        if k.is_empty() || set.iter().any(|(s, _)| s == k) {
            return Err(AppError::invalid(format!("cannot both set and remove {k:?}")));
        }
        ops.push(json!({"op": "remove", "path": format!("/{}", pointer_escape(k)?)}));
    }
    for (k, v) in &set {
        ops.push(json!({"op": "add", "path": format!("/{}", pointer_escape(k)?), "value": v}));
    }
    if ops.is_empty() {
        return Err(AppError::invalid("nothing changed"));
    }
    Ok(ops)
}

/// A custom image property as a JSON-Pointer segment (RFC 6901); Glance's own
/// fields can't be touched through the metadata editor.
fn pointer_escape(key: &str) -> R<String> {
    if crate::server::mapping::is_image_core_key(key) {
        return Err(AppError::invalid(format!("{key} is not a custom property")));
    }
    Ok(key.replace('~', "~0").replace('/', "~1"))
}

/// One of the SDK's image enums (disk or container format, visibility) from its Glance name.
fn image_enum<T: serde::de::DeserializeOwned>(what: &str, v: &str) -> R<T> {
    serde_json::from_value(json!(v.trim())).map_err(|_| AppError::invalid(format!("unknown {what} {v:?}")))
}

/// The `uri` of a `web-download` import: `http(s)` only (Glance's default policy refuses
/// other schemes and private addresses anyway), without whitespace.
fn image_source_url(url: &str) -> R<&str> {
    let u = url.trim();
    let host = u
        .strip_prefix("https://")
        .or_else(|| u.strip_prefix("http://"))
        .unwrap_or("");
    if host.is_empty() || host.starts_with('/') || u.chars().any(char::is_whitespace) {
        return Err(AppError::invalid("the image URL must be http:// or https://"));
    }
    Ok(u)
}

pub(super) async fn image_update(x: &mut Exec<'_>, id: &str, changes: &ImageChanges) -> R<()> {
    use openstack_sdk::api::image::v2::image::patch as m;
    let patch: json_patch::Patch = serde_json::from_value(Value::Array(image_patch(changes)?)).map_err(build_err)?;
    x.send(m::Request::builder().id(id).patch(patch).build().map_err(build_err)?)
        .await?;
    Ok(())
}

pub(super) async fn image_set_properties(
    x: &mut Exec<'_>,
    id: &str,
    set: &[(String, String)],
    remove: &[String],
) -> R<()> {
    use openstack_sdk::api::image::v2::image::patch as m;
    let changes = ImageChanges {
        set: set.to_vec(),
        remove: remove.to_vec(),
        ..Default::default()
    };
    let patch: json_patch::Patch = serde_json::from_value(Value::Array(image_patch(&changes)?)).map_err(build_err)?;
    x.send(m::Request::builder().id(id).patch(patch).build().map_err(build_err)?)
        .await?;
    Ok(())
}

pub(super) async fn image_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::image::v2::image::delete as m;
    x.send(m::Request::builder().id(id).build().map_err(build_err)?).await?;
    Ok(())
}

pub(super) async fn image_create(x: &mut Exec<'_>, spec: &ImageSpec) -> R<()> {
    use openstack_sdk::api::image::v2::image::{create as m, import::create as imp};
    let url = spec.url.as_deref().map(image_source_url).transpose()?;
    if spec.upload_bytes.is_some() && (url.is_some() || spec.upload_bytes == Some(0)) {
        return Err(AppError::invalid(
            "an image comes from a URL or from a non-empty file, not both",
        ));
    }
    let disk: m::DiskFormat = image_enum("disk format", &spec.disk_format)?;
    // Horizon: AMI-style images keep their format as container, the rest are `bare`.
    let container_name = match spec
        .container_format
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        Some(c) => c,
        None if matches!(spec.disk_format.trim(), "ami" | "aki" | "ari") => spec.disk_format.trim(),
        None => "bare",
    };
    let container: m::ContainerFormat = image_enum("container format", container_name)?;
    let visibility: m::Visibility = image_enum("visibility", &spec.visibility)?;
    let props = checked_metadata(&spec.properties)?;
    if let Some((k, _)) = props.iter().find(|(k, _)| crate::server::mapping::is_image_core_key(k)) {
        return Err(AppError::invalid(format!("{k} is not a custom property")));
    }
    let mut b = m::Request::builder();
    b.name(Some(Cow::from(non_empty(&spec.name, "name")?.to_string())))
        .disk_format(disk)
        .container_format(container)
        .visibility(visibility)
        .protected(spec.protected)
        .properties(props.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    if let Some(d) = spec.min_disk_gb {
        b.min_disk(d as i32);
    }
    if let Some(r) = spec.min_ram_mb {
        b.min_ram(r as i32);
    }
    if let Some(owner) = spec.owner.as_deref().map(str::trim).filter(|o| !o.is_empty()) {
        b.owner(Some(Cow::from(owner.to_string())));
    }
    let image = x.create(b.build().map_err(build_err)?, "image").await?;
    if let Some(bytes) = spec.upload_bytes {
        x.note_upload(&image, bytes);
    }
    if let Some(url) = url {
        let method = imp::MethodBuilder::default()
            .name("web-download")
            .uri(url)
            .build()
            .map_err(build_err)?;
        x.send(
            imp::Request::builder()
                .image_id(image)
                .method(method)
                .build()
                .map_err(build_err)?,
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[tokio::test]
    async fn image_create_records_then_imports_the_url() {
        let spec = ImageSpec {
            name: id(" Debian "),
            url: Some(id("https://cloud.example/debian.qcow2")),
            disk_format: id("qcow2"),
            visibility: id("private"),
            min_disk_gb: Some(10),
            properties: vec![(id("architecture"), id("x86_64"))],
            ..Default::default()
        };
        let r = dry(OsAction::ImageCreate(spec.clone())).await;
        assert_eq!(
            calls(&r),
            [
                (id("POST"), id("/images")),
                (id("POST"), id("/images/%3Cnew%20image%20id%3E/import"))
            ]
        );
        let b = body(&r[0]);
        assert_eq!(
            (
                b["name"].as_str(),
                b["disk_format"].as_str(),
                b["container_format"].as_str()
            ),
            (Some("Debian"), Some("qcow2"), Some("bare"))
        );
        assert_eq!(
            (
                b["visibility"].as_str(),
                b["protected"].as_bool(),
                b["min_disk"].as_i64(),
                b["architecture"].as_str()
            ),
            (Some("private"), Some(false), Some(10), Some("x86_64"))
        );
        assert_eq!(
            body(&r[1]),
            json!({"method": {"name": "web-download", "uri": "https://cloud.example/debian.qcow2"}})
        );
        let bare = dry(OsAction::ImageCreate(ImageSpec {
            url: None,
            ..spec.clone()
        }))
        .await;
        assert_eq!(calls(&bare), [(id("POST"), id("/images"))], "no URL: only the record");
        // A file: the record, then the streamed PUT is only previewed (the browser sends it).
        let file = dry(OsAction::ImageCreate(ImageSpec {
            url: None,
            upload_bytes: Some(2048),
            ..spec.clone()
        }))
        .await;
        assert_eq!(
            calls(&file),
            [
                (id("POST"), id("/images")),
                (id("PUT"), id("/images/<new image id>/file"))
            ]
        );
        assert!(
            file[1].body.as_deref().unwrap_or_default().contains("2048 bytes"),
            "{:?}",
            file[1].body
        );
        let ami = dry(OsAction::ImageCreate(ImageSpec {
            disk_format: id("ami"),
            ..spec.clone()
        }))
        .await;
        assert_eq!(
            body(&ami[0])["container_format"],
            "ami",
            "AMI-style images are their own container"
        );
        assert!(body(&r[0]).get("owner").is_none(), "no owner: the session's project");
        let owned = dry(OsAction::ImageCreate(ImageSpec {
            owner: Some(id("p9")),
            ..spec.clone()
        }))
        .await;
        assert_eq!(body(&owned[0])["owner"], "p9", "admin Create Image (roadmap 89)");
        for bad in [
            ImageSpec {
                url: Some(id("ftp://x/y")),
                ..spec.clone()
            },
            ImageSpec {
                url: Some(id("https://")),
                ..spec.clone()
            },
            ImageSpec {
                url: Some(id("https://a b")),
                ..spec.clone()
            },
            ImageSpec {
                upload_bytes: Some(1),
                ..spec.clone()
            },
            ImageSpec {
                url: None,
                upload_bytes: Some(0),
                ..spec.clone()
            },
            ImageSpec {
                name: id(" "),
                ..spec.clone()
            },
            ImageSpec {
                disk_format: id("exe"),
                ..spec.clone()
            },
            ImageSpec {
                visibility: id("everyone"),
                ..spec.clone()
            },
            ImageSpec {
                properties: vec![(id("status"), id("active"))],
                ..spec.clone()
            },
        ] {
            assert!(dry_err(OsAction::ImageCreate(bad.clone())).await, "{bad:?}");
        }
    }

    #[tokio::test]
    async fn image_update_is_a_json_patch() {
        let rename = ImageChanges {
            name: Some(id(" n ")),
            ..Default::default()
        };
        let r = dry(OsAction::ImageUpdate {
            id: id("i"),
            changes: rename,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
            ("PATCH", "/images/i", "image")
        );
        assert_eq!(
            body(&r[0]),
            json!([{"op": "replace", "path": "/name", "value": "n"}]),
            "only what changed"
        );
        let all = ImageChanges {
            visibility: Some(id("shared")),
            protected: Some(true),
            min_disk_gb: Some(10),
            min_ram_mb: Some(512),
            disk_format: Some(id("qcow2")),
            set: vec![(id("architecture"), id("x86_64"))],
            remove: vec![id("description")],
            ..Default::default()
        };
        let r = dry(OsAction::ImageUpdate {
            id: id("i"),
            changes: all,
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!([
                {"op": "replace", "path": "/visibility", "value": "shared"},
                {"op": "replace", "path": "/protected", "value": true},
                {"op": "replace", "path": "/min_disk", "value": 10},
                {"op": "replace", "path": "/min_ram", "value": 512},
                {"op": "replace", "path": "/disk_format", "value": "qcow2"},
                {"op": "remove", "path": "/description"},
                {"op": "add", "path": "/architecture", "value": "x86_64"},
            ]),
            "integers stay integers, properties are added or removed"
        );
        for bad in [
            ImageChanges {
                visibility: Some(id("everyone")),
                ..Default::default()
            },
            ImageChanges {
                name: Some(id(" ")),
                ..Default::default()
            },
            ImageChanges::default(),
        ] {
            let mut x = Exec::new(WriteMode::DryRun, None);
            assert!(
                apply(
                    &mut x,
                    &OsAction::ImageUpdate {
                        id: id("i"),
                        changes: bad.clone()
                    }
                )
                .await
                .is_err(),
                "{bad:?}"
            );
        }
    }

    #[tokio::test]
    async fn metadata_is_replaced_or_patched() {
        let md = vec![(id(" role "), id("web")), (id("env"), id("dev"))];
        let r = dry(OsAction::ServerSetMetadata {
            id: id("s"),
            metadata: md.clone(),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
            ("PUT", "/servers/s/metadata", "compute")
        );
        assert_eq!(body(&r[0]), json!({"metadata": {"env": "dev", "role": "web"}}));
        let r = dry(OsAction::VolumeSetMetadata {
            id: id("v"),
            metadata: vec![],
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/volumes/v/metadata")
        );
        assert_eq!(body(&r[0]), json!({"metadata": {}}), "empty removes everything");
        let r = dry(OsAction::SnapshotSetMetadata {
            id: id("sn"),
            metadata: md,
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("PUT", "/snapshots/sn/metadata")
        );
        let r = dry(OsAction::ImageSetProperties {
            id: id("i"),
            set: vec![(id("os_distro"), id("debian")), (id("a/b"), id("1"))],
            remove: vec![id("old")],
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].service.as_str()), ("PATCH", "image"));
        assert_eq!(
            body(&r[0]),
            json!([
                {"op": "remove", "path": "/old"},
                {"op": "add", "path": "/a~1b", "value": "1"},
                {"op": "add", "path": "/os_distro", "value": "debian"},
            ])
        );
        for bad in [
            OsAction::ServerSetMetadata {
                id: id("s"),
                metadata: vec![(id("k"), id("1")), (id("k"), id("2"))],
            },
            OsAction::ImageSetProperties {
                id: id("i"),
                set: vec![(id("min_disk"), id("10"))],
                remove: vec![],
            },
            OsAction::ImageSetProperties {
                id: id("i"),
                set: vec![],
                remove: vec![id("visibility")],
            },
            OsAction::ImageSetProperties {
                id: id("i"),
                set: vec![],
                remove: vec![],
            },
        ] {
            let mut x = Exec::new(WriteMode::DryRun, None);
            assert!(apply(&mut x, &bad).await.is_err(), "{bad:?}");
        }
    }
}
