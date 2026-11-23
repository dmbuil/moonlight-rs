//! Glance: images, their properties and detail.

use super::*;

/// Whether a Glance image is an instance snapshot. Nova tags a snapshot of an ephemeral instance
/// with `image_type=snapshot`, but a snapshot of a volume-backed instance has no `image_type`: it
/// is a zero-size image whose `block_device_mapping` property (a JSON string) boots from a Cinder
/// snapshot (`boot_index` 0, `source_type` snapshot, `destination_type` volume). A malformed
/// mapping counts as not a snapshot.
pub fn is_instance_snapshot(im: &Value) -> bool {
    if s(im, "image_type").as_deref() == Some("snapshot") {
        return true;
    }
    let parsed;
    let bdm = match im.get("block_device_mapping") {
        Some(Value::String(raw)) => match serde_json::from_str::<Value>(raw) {
            Ok(v) => {
                parsed = v;
                &parsed
            }
            Err(_) => return false,
        },
        Some(v) => v,
        None => return false,
    };
    bdm.as_array().is_some_and(|entries| {
        entries.iter().any(|e| {
            e.get("boot_index")
                .is_some_and(|b| b.as_i64() == Some(0) || b.as_str() == Some("0"))
                && s(e, "source_type").as_deref() == Some("snapshot")
                && s(e, "destination_type").as_deref() == Some("volume")
        })
    })
}

/// Images with visibility and owner (`project_names` for owners other than
/// `project_id`): the project's own first, then by name.
pub fn images(
    raw: &[Value],
    project_id: &str,
    project_names: &HashMap<String, String>,
    pills: &[String],
) -> Vec<Image> {
    let mut out = images_page(raw, project_id, project_names, pills);
    // Project images first, then alphabetical (Horizon groups them by tab)
    out.sort_by(|a, b| {
        b.owned_by_project
            .cmp(&a.owned_by_project)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

/// [`images`] in Glance's order, for a paged list: the next page's
/// marker is the last row. `pills`: the `[images] pills` keys each row carries (`&[]`: none).
pub fn images_page(
    raw: &[Value],
    project_id: &str,
    project_names: &HashMap<String, String>,
    pills: &[String],
) -> Vec<Image> {
    raw.iter()
        .map(|im| Image {
            id: s(im, "id").unwrap_or_default(),
            name: name_or_id(im),
            is_snapshot: is_instance_snapshot(im),
            status: s(im, "status").unwrap_or_default(),
            visibility: s(im, "visibility").unwrap_or_default(),
            protected: b(im, "protected"),
            disk_format: s_ne(im, "disk_format"),
            size: im.get("size").and_then(Value::as_u64),
            owned_by_project: s(im, "owner").as_deref() == Some(project_id),
            owner_name: s(im, "owner").and_then(|o| project_names.get(&o).cloned()),
            owner_id: s_ne(im, "owner"),
            properties: image_properties(im),
            min_disk_gb: i(im, "min_disk"),
            min_ram_mb: i(im, "min_ram"),
            import_failed: s_ne(im, "os_glance_failed_import"),
            pills: Vec::new(),
        })
        .map(|mut i| {
            i.pills = i.pills_for(pills);
            i
        })
        .collect()
}

/// The keys Glance defines on an image; every other key is a custom property.
const IMAGE_CORE_KEYS: &[&str] = &[
    "id",
    "name",
    "status",
    "visibility",
    "protected",
    "disk_format",
    "container_format",
    "size",
    "virtual_size",
    "checksum",
    "owner",
    "min_disk",
    "min_ram",
    "created_at",
    "updated_at",
    "tags",
    "self",
    "file",
    "schema",
    "locations",
    "direct_url",
    "os_hash_algo",
    "os_hash_value",
    "os_hidden",
    "stores",
];

/// Whether Glance owns this key (not a custom property).
pub fn is_image_core_key(k: &str) -> bool {
    IMAGE_CORE_KEYS.contains(&k)
}

/// The image's custom properties (non-core keys), sorted, non-string values as JSON text.
fn image_properties(im: &Value) -> Vec<(String, String)> {
    let mut properties: Vec<(String, String)> = im
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| !is_image_core_key(k))
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    properties.sort();
    properties
}

/// The image detail page, with the owner's project name.
pub fn image_detail(im: &Value, project_id: &str, project_names: &HashMap<String, String>) -> ImageDetail {
    let image = images(std::slice::from_ref(im), project_id, project_names, &[]).remove(0);
    let properties = image.properties.clone();
    ImageDetail {
        owner: s_ne(im, "owner"),
        checksum: s_ne(im, "checksum"),
        container_format: s_ne(im, "container_format"),
        virtual_size: im.get("virtual_size").and_then(Value::as_u64),
        tags: strs(im, "tags"),
        created: s(im, "created_at"),
        updated: s(im, "updated_at"),
        properties,
        image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rows_carry_the_configured_pills() {
        let raw = [
            json!({"id": "a", "os_type": "linux", "disk_format": "raw"}),
            json!({"id": "b"}),
        ];
        let keys = ["os_type", "disk_format"].map(String::from);
        let out = images_page(&raw, "p", &HashMap::new(), &keys);
        assert_eq!(
            out[0].pills,
            vec![("os_type".to_string(), "linux".to_string())],
            "core fields are not metadata"
        );
        assert!(out[1].pills.is_empty() && images_page(&raw, "p", &HashMap::new(), &[])[0].pills.is_empty());
    }

    #[test]
    fn failed_import_reason_comes_from_os_glance_failed_import() {
        let ok = json!({"id": "a", "status": "queued", "os_glance_failed_import": ""});
        let bad = json!({"id": "b", "status": "queued", "os_glance_failed_import": "Unable to download: 404"});
        let out = images(&[ok, bad], "p", &HashMap::new(), &[]);
        let by = |id: &str| out.iter().find(|i| i.id == id).unwrap().import_failed.clone();
        assert_eq!(by("a"), None, "an empty value is not a failure");
        assert_eq!(by("b").as_deref(), Some("Unable to download: 404"));
    }

    #[test]
    fn images_put_project_ones_first_and_detect_snapshots() {
        let raw = vec![
            json!({"id": "1", "name": "ubuntu", "owner": "other", "visibility": "public", "size": 1024}),
            json!({"id": "2", "name": "backup", "owner": "p1", "visibility": "private", "image_type": "snapshot", "protected": true}),
        ];
        let out = images(&raw, "p1", &HashMap::new(), &[]);
        assert_eq!(out[0].name, "backup");
        assert!(out[0].is_snapshot && out[0].protected && out[0].owned_by_project);
        assert_eq!(out[1].size, Some(1024));
    }

    #[test]
    fn volume_backed_snapshots_are_instance_snapshots_without_image_type() {
        let bdm = r#"[{"source_type": "snapshot", "destination_type": "volume", "boot_index": 0, "snapshot_id": "s", "volume_size": 20}]"#;
        let backed = json!({"id": "v", "size": 0, "bdm_v2": "True", "block_device_mapping": bdm});
        let ephemeral = json!({"id": "e", "image_type": "snapshot"});
        let from_image = json!({"id": "i", "block_device_mapping":
            r#"[{"source_type": "image", "destination_type": "volume", "boot_index": 0}]"#});
        let data_only = json!({"id": "d", "block_device_mapping":
            r#"[{"source_type": "snapshot", "destination_type": "volume", "boot_index": 1}]"#});
        let as_array = json!({"id": "a", "block_device_mapping":
            [{"source_type": "snapshot", "destination_type": "volume", "boot_index": 0}]});
        let malformed = json!({"id": "m", "block_device_mapping": "not json ["});
        let plain = json!({"id": "p", "name": "ubuntu"});
        let flags: Vec<bool> = [backed, ephemeral, from_image, data_only, as_array, malformed, plain]
            .iter()
            .map(is_instance_snapshot)
            .collect();
        assert_eq!(flags, [true, true, false, false, true, false, false]);
    }

    #[test]
    fn image_detail_separates_custom_properties() {
        let im = json!({"id": "i", "name": "rhel", "owner": "p", "min_disk": 10, "hw_disk_bus": "scsi", "os_distro": "rhel", "size": 5});
        let d = image_detail(&im, "p", &HashMap::new());
        assert_eq!(
            d.properties,
            vec![
                ("hw_disk_bus".into(), "scsi".into()),
                ("os_distro".into(), "rhel".into())
            ]
        );
        assert_eq!((d.image.min_disk_gb, d.image.min_ram_mb), (Some(10), None));
        assert_eq!(d.image.property("os_distro"), Some("rhel"));
    }

    #[test]
    fn image_owner_resolves_to_project_name() {
        let names = HashMap::from([("p1".to_string(), "Proyecto A".to_string())]);
        let out = images(
            &[
                json!({"id": "1", "name": "a", "owner": "p1"}),
                json!({"id": "2", "name": "b", "owner": "zz"}),
            ],
            "p1",
            &names,
            &[],
        );
        assert_eq!(out[0].owner_name.as_deref(), Some("Proyecto A"));
        assert_eq!(
            (out[1].owner_name.clone(), out[1].owner_id.as_deref()),
            (None, Some("zz"))
        );
    }
}
