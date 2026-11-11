//! Cinder: volumes, snapshots, backups, groups, messages.

use super::*;

/// Volumes with their attachments' server names.
pub fn volumes(raw: &[Value], servers: &[Value]) -> Vec<Volume> {
    let mut out = volumes_page(raw, servers);
    out.sort_by_key(|v| v.name.to_lowercase());
    out
}

/// [`volumes`] in Cinder's order, for a paged list (roadmap 88): the next page's
/// marker is the last row. `servers` may be empty: the attachments then carry only
/// the server IDs, named later (`api::get_late_names`).
pub fn volumes_page(raw: &[Value], servers: &[Value]) -> Vec<Volume> {
    let server_names = names_by_id(servers);
    raw.iter()
        .map(|v| Volume {
            id: s(v, "id").unwrap_or_default(),
            name: name_or_id(v),
            description: s_ne(v, "description"),
            size_gb: i(v, "size").unwrap_or(0),
            status: s(v, "status").unwrap_or_default(),
            volume_type: s_ne(v, "volume_type"),
            attachments: arr(v, "attachments")
                .iter()
                .filter_map(|a| {
                    let server_id = s(a, "server_id")?;
                    Some(VolumeAttachment {
                        server_name: server_names.get(&server_id).cloned(),
                        server_id,
                        device: s_ne(a, "device"),
                    })
                })
                .collect(),
            availability_zone: s_ne(v, "availability_zone"),
            bootable: b(v, "bootable"),
            encrypted: b(v, "encrypted"),
            metadata: kv_pairs(v.get("metadata")),
        })
        .collect()
}

/// Volume snapshots with their volume's name.
pub fn snapshots(raw: &[Value], volumes: &[Value]) -> Vec<Snapshot> {
    let mut out = snapshots_page(raw, volumes);
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

/// [`snapshots`] in Cinder's order, for a paged list (roadmap 88). `volumes` may be
/// empty: the volume is then named later (`api::get_late_names`).
pub fn snapshots_page(raw: &[Value], volumes: &[Value]) -> Vec<Snapshot> {
    let vol_names: HashMap<String, String> = volumes
        .iter()
        .filter_map(|v| Some((s(v, "id")?, s_ne(v, "name")?)))
        .collect();
    raw.iter()
        .map(|sn| {
            let volume_id = s(sn, "volume_id").unwrap_or_default();
            Snapshot {
                id: s(sn, "id").unwrap_or_default(),
                name: name_or_id(sn),
                description: s_ne(sn, "description"),
                size_gb: i(sn, "size").unwrap_or(0),
                status: s(sn, "status").unwrap_or_default(),
                volume_name: vol_names.get(&volume_id).cloned(),
                volume_id,
                created: s(sn, "created_at").unwrap_or_default(),
                metadata: kv_pairs(sn.get("metadata")),
            }
        })
        .collect()
}

/// Volumes › Backups (roadmap 5), newest first, with the names of their volumes
/// (`volumes` may be empty: the ID is shown then). `is_incremental` and
/// `fail_reason` are absent on older Cinders; a backup of a deleted volume
/// keeps its `volume_id`, so the link may point nowhere.
pub fn backups(raw: &[Value], volumes: &[Value]) -> Vec<BackupRow> {
    let names = names_by_id(volumes);
    let mut out: Vec<_> = raw
        .iter()
        .map(|bk| BackupRow {
            id: s(bk, "id").unwrap_or_default(),
            name: name_or_id(bk),
            description: s_ne(bk, "description"),
            status: s(bk, "status").unwrap_or_default(),
            size_gb: i(bk, "size").unwrap_or(0),
            volume: named_ref(s(bk, "volume_id").unwrap_or_default(), &names),
            incremental: b(bk, "is_incremental"),
            container: s_ne(bk, "container"),
            snapshot_id: s_ne(bk, "snapshot_id"),
            availability_zone: s_ne(bk, "availability_zone"),
            fail_reason: s_ne(bk, "fail_reason"),
            created: s(bk, "created_at").unwrap_or_default(),
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

/// A `Ref` to `id`, named from `names` (the ID when unknown, like Horizon).
pub(super) fn named_ref(id: String, names: &HashMap<String, String>) -> Ref {
    Ref {
        name: names.get(&id).cloned().unwrap_or_else(|| id.clone()),
        id,
    }
}

/// One generic volume group (roadmap 5). `group_snapshots` only tells whether
/// it has any (Horizon's *Has Snapshots*).
fn volume_group(
    g: &Value,
    group_types: &HashMap<String, String>,
    volume_types: &HashMap<String, String>,
    group_snapshots: &[Value],
) -> VolumeGroupRow {
    let id = s(g, "id").unwrap_or_default();
    VolumeGroupRow {
        name: name_or_id(g),
        description: s_ne(g, "description"),
        status: s(g, "status").unwrap_or_default(),
        availability_zone: s_ne(g, "availability_zone"),
        group_type: named_ref(s(g, "group_type").unwrap_or_default(), group_types),
        volume_types: strs(g, "volume_types")
            .into_iter()
            .map(|t| named_ref(t, volume_types))
            .collect(),
        has_snapshots: group_snapshots
            .iter()
            .any(|x| s(x, "group_id").as_deref() == Some(id.as_str())),
        created: s(g, "created_at").unwrap_or_default(),
        id,
    }
}

/// Volumes › Groups, sorted by name. Type names come from the group type and
/// volume type lists; either may be empty (IDs shown instead).
pub fn volume_groups(
    raw: &[Value],
    group_types: &[Value],
    volume_types: &[Value],
    group_snapshots: &[Value],
) -> Vec<VolumeGroupRow> {
    let mut out = volume_groups_page(raw, group_types, volume_types, group_snapshots);
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

/// [`volume_groups`] in Cinder's order, for a paged list (roadmap 88).
pub fn volume_groups_page(
    raw: &[Value],
    group_types: &[Value],
    volume_types: &[Value],
    group_snapshots: &[Value],
) -> Vec<VolumeGroupRow> {
    let (gt, vt) = (names_by_id(group_types), names_by_id(volume_types));
    raw.iter().map(|g| volume_group(g, &gt, &vt, group_snapshots)).collect()
}

/// The volume group detail: `volumes` and `group_snapshots` may hold the whole
/// project's; only those with this `group_id` are kept. Volumes carry
/// `group_id` from volume API 3.13, which `fetch` asks for.
pub fn volume_group_detail(
    g: &Value,
    group_types: &[Value],
    volume_types: &[Value],
    group_snapshots: &[Value],
    volumes_raw: &[Value],
) -> VolumeGroupDetail {
    let row = volume_group(
        g,
        &names_by_id(group_types),
        &names_by_id(volume_types),
        group_snapshots,
    );
    let in_group = |x: &&Value| s(x, "group_id").as_deref() == Some(row.id.as_str());
    let mine: Vec<Value> = volumes_raw.iter().filter(in_group).cloned().collect();
    let snaps: Vec<Value> = group_snapshots.iter().filter(in_group).cloned().collect();
    let groups = [g.clone()];
    VolumeGroupDetail {
        volumes: volumes(&mine, &[]),
        snapshots: group_snapshots_rows(&snaps, &groups, group_types),
        row,
    }
}

/// Volumes › Group Snapshots, newest first, with their group's name.
pub fn group_snapshots_rows(raw: &[Value], groups: &[Value], group_types: &[Value]) -> Vec<GroupSnapshotRow> {
    let mut out = group_snapshots_page(raw, groups, group_types);
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

/// [`group_snapshots_rows`] in Cinder's order, for a paged list (roadmap 88);
/// `groups` need only be the page's ([`group_ids`]).
pub fn group_snapshots_page(raw: &[Value], groups: &[Value], group_types: &[Value]) -> Vec<GroupSnapshotRow> {
    let (gn, gt) = (names_by_id(groups), names_by_id(group_types));
    raw.iter()
        .map(|x| GroupSnapshotRow {
            id: s(x, "id").unwrap_or_default(),
            name: name_or_id(x),
            description: s_ne(x, "description"),
            status: s(x, "status").unwrap_or_default(),
            group: named_ref(s(x, "group_id").unwrap_or_default(), &gn),
            group_type: named_ref(s(x, "group_type_id").unwrap_or_default(), &gt),
            created: s(x, "created_at").unwrap_or_default(),
        })
        .collect()
}

/// The groups a page of group snapshots belongs to, each once (roadmap 88).
pub fn group_ids(raw: &[Value]) -> Vec<String> {
    distinct(raw.iter().filter_map(|x| s(x, "group_id")))
}

/// *Manage Volumes* choices: per group, its own volumes and those Cinder lets
/// it take (like Horizon's form: no group yet, one of the group's volume types,
/// `available` or `in-use`), each with the group's ID as first cell. Groups list
/// volume types by ID while volumes name theirs (`volume_type_id` only from
/// 3.61), so both are matched. Volumes need `group_id` (volume 3.13).
pub fn group_volume_choices(groups: &[Value], volumes_raw: &[Value], volume_types: &[Value]) -> Vec<Choice> {
    let type_names = names_by_id(volume_types);
    let mut out = Vec::new();
    for g in groups {
        let Some(gid) = s(g, "id") else { continue };
        let ids = strs(g, "volume_types");
        let accepted: Vec<&str> = ids
            .iter()
            .map(String::as_str)
            .chain(ids.iter().filter_map(|t| type_names.get(t).map(String::as_str)))
            .collect();
        let fits = |v: &Value| {
            s(v, "group_id").is_none()
                && matches!(s(v, "status").as_deref(), Some("available" | "in-use"))
                && [s(v, "volume_type"), s(v, "volume_type_id")]
                    .iter()
                    .flatten()
                    .any(|t| accepted.contains(&t.as_str()))
        };
        let mine = volumes_raw
            .iter()
            .filter(|v| s(v, "group_id").as_deref() == Some(gid.as_str()) || fits(v));
        out.extend(sorted(
            mine.filter_map(|v| {
                Some(Choice {
                    value: s(v, "id")?,
                    label: label_volume(v),
                    cells: vec![gid.clone()],
                    ..Default::default()
                })
            })
            .collect(),
        ));
    }
    out
}

/// The group snapshot detail: the volume snapshots taken with it (those with
/// its `group_snapshot_id`, volume API 3.14), named after their volumes.
pub fn group_snapshot_detail(
    x: &Value,
    groups: &[Value],
    group_types: &[Value],
    snapshots_raw: &[Value],
    volumes_raw: &[Value],
) -> GroupSnapshotDetail {
    let row = group_snapshots_rows(std::slice::from_ref(x), groups, group_types).remove(0);
    let mine: Vec<Value> = snapshots_raw
        .iter()
        .filter(|sn| s(sn, "group_snapshot_id").as_deref() == Some(row.id.as_str()))
        .cloned()
        .collect();
    GroupSnapshotDetail {
        snapshots: snapshots(&mine, volumes_raw),
        row,
    }
}

/// Cinder user messages about one volume, newest first.
pub fn volume_messages(msgs: &[Value], volume_id: &str) -> Vec<VolumeMessage> {
    let mut out: Vec<VolumeMessage> = msgs
        .iter()
        .filter(|m| s(m, "resource_uuid").as_deref() == Some(volume_id))
        .filter_map(|m| {
            Some(VolumeMessage {
                id: s(m, "id")?,
                level: s(m, "message_level").unwrap_or_default(),
                message: s_ne(m, "user_message").unwrap_or_else(|| s(m, "event_id").unwrap_or_default()),
                created: s(m, "created_at").unwrap_or_default(),
                request_id: s_ne(m, "request_id"),
            })
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

/// The volume detail page, with attached servers' names and its snapshots.
pub fn volume_detail(v: &Value, servers: &[Value], snaps: &[Value]) -> VolumeDetail {
    let volume = volumes(std::slice::from_ref(v), servers).remove(0);
    let image_md = v.get("volume_image_metadata");
    let snapshots = snapshots(
        &snaps
            .iter()
            .filter(|x| s(x, "volume_id").as_deref() == Some(volume.id.as_str()))
            .cloned()
            .collect::<Vec<_>>(),
        std::slice::from_ref(v),
    );
    VolumeDetail {
        created: s(v, "created_at").unwrap_or_default(),
        source_image: image_md.and_then(|m| s(m, "image_name").or_else(|| s(m, "image_id"))),
        source_snapshot_id: s_ne(v, "snapshot_id"),
        multiattach: b(v, "multiattach"),
        metadata: kv_pairs(v.get("metadata")),
        image_metadata: kv_pairs(image_md),
        snapshots,
        volume,
        messages: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::*;
    use super::*;
    use serde_json::json;

    #[test]
    fn volumes_resolve_server_names_and_string_booleans() {
        let raw = vec![
            json!({"id": "v1", "name": "", "size": 10, "status": "in-use", "bootable": "true",
            "encrypted": false, "attachments": [{"server_id": "s1", "device": "/dev/vda"}]}),
        ];
        let servers = vec![json!({"id": "s1", "name": "web"})];
        let v = &volumes(&raw, &servers)[0];
        assert_eq!(v.name, "v1");
        assert!(v.bootable && !v.encrypted);
        assert_eq!(v.attachments[0].server_name.as_deref(), Some("web"));
    }

    #[test]
    fn backups_name_their_volume_and_survive_missing_fields() {
        let raw = [
            json!({"id": "b1", "name": "", "status": "available", "size": 5, "volume_id": "v1", "is_incremental": true, "container": "volumebackups", "created_at": "2026-01-01T00:00:00", "description": ""}),
            json!({"id": "b2", "name": "new", "status": "error", "size": 1, "volume_id": "gone", "fail_reason": "no space", "snapshot_id": null, "created_at": "2026-02-01T00:00:00"}),
        ];
        let rows = backups(&raw, &[json!({"id": "v1", "name": "data"})]);
        assert_eq!(rows[0].id, "b2", "newest first");
        assert_eq!(
            (
                rows[0].volume.name.as_str(),
                rows[0].incremental,
                rows[0].fail_reason.as_deref()
            ),
            ("gone", false, Some("no space")),
            "deleted volume: ID shown"
        );
        assert_eq!(
            (
                rows[1].name.as_str(),
                rows[1].volume.name.as_str(),
                rows[1].description.clone()
            ),
            ("b1", "data", None),
            "unnamed: ID; \"\" is no description"
        );
        assert!(rows[1].incremental);
        assert_eq!(backups(&raw, &[])[1].volume.name, "v1");
    }

    #[test]
    fn volume_groups_resolve_type_names_and_snapshots() {
        let groups = [
            json!({"id": "g1", "name": "db", "description": "", "status": "available", "availability_zone": "nova",
                   "group_type": "gt1", "volume_types": ["vt1", "vt-gone"], "created_at": "2026-10-01T10:00:00.000000"}),
            json!({"id": "g2", "name": null, "status": "creating", "group_type": "gt-gone", "volume_types": []}),
        ];
        let types = [json!({"id": "gt1", "name": "default"})];
        let vtypes = [json!({"id": "vt1", "name": "ceph"})];
        let snaps = [
            json!({"id": "gs1", "name": "nightly", "group_id": "g1", "group_type_id": "gt1", "status": "available", "created_at": "2026-10-02T10:00:00.000000"}),
        ];
        let rows = volume_groups(&groups, &types, &vtypes, &snaps);
        assert_eq!(
            rows.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(),
            ["db", "g2"],
            "unnamed: the ID, sorted by name"
        );
        let db = &rows[0];
        assert_eq!(
            (db.description.as_deref(), db.group_type.name.as_str()),
            (None, "default"),
            "\"\" description is none"
        );
        assert_eq!(
            db.volume_types.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["ceph", "vt-gone"],
            "unknown type: its ID"
        );
        assert!(db.has_snapshots && !rows[1].has_snapshots);
        assert_eq!(rows[1].group_type.name, "gt-gone");
        // Without the type lists (a user Cinder refuses them to) every name is an ID.
        assert_eq!(volume_groups(&groups, &[], &[], &[])[0].group_type.name, "gt1");
    }

    #[test]
    fn volume_group_detail_keeps_only_its_volumes_and_snapshots() {
        let g = json!({"id": "g1", "name": "db", "status": "available", "group_type": "gt1", "volume_types": ["vt1"]});
        let vols = [
            json!({"id": "v1", "name": "data", "size": 10, "status": "in-use", "group_id": "g1", "bootable": "false"}),
            json!({"id": "v2", "name": "other", "size": 1, "status": "available", "group_id": null}),
        ];
        let snaps = [
            json!({"id": "gs1", "group_id": "g1", "group_type_id": "gt1", "status": "available"}),
            json!({"id": "gs2", "group_id": "g9", "status": "available"}),
        ];
        let d = volume_group_detail(&g, &[], &[], &snaps, &vols);
        assert_eq!(d.volumes.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["v1"]);
        assert_eq!(
            d.snapshots
                .iter()
                .map(|s| (s.id.as_str(), s.group.name.as_str()))
                .collect::<Vec<_>>(),
            [("gs1", "db")]
        );
        assert!(d.row.has_snapshots);
    }

    #[test]
    fn group_snapshots_name_their_group_and_list_their_volume_snapshots() {
        let gs = json!({"id": "gs1", "name": "", "description": "before upgrade", "status": "available", "group_id": "g1",
                        "group_type_id": "gt1", "created_at": "2026-10-02T10:00:00.000000"});
        let groups = [json!({"id": "g1", "name": "db"})];
        let rows = group_snapshots_rows(
            &[
                gs.clone(),
                json!({"id": "gs2", "group_id": "g-gone", "status": "error"}),
            ],
            &groups,
            &[],
        );
        assert_eq!(
            rows.iter().map(|r| r.group.name.as_str()).collect::<Vec<_>>(),
            ["db", "g-gone"],
            "newest first; a deleted group: its ID"
        );
        assert_eq!(
            (rows[0].name.as_str(), rows[0].group_type.name.as_str()),
            ("gs1", "gt1"),
            "\"\" name: the ID"
        );
        let snaps = [
            json!({"id": "s1", "name": "data-snap", "volume_id": "v1", "size": 10, "status": "available", "group_snapshot_id": "gs1"}),
            json!({"id": "s2", "volume_id": "v1", "size": 10, "status": "available"}),
        ];
        let d = group_snapshot_detail(&gs, &groups, &[], &snaps, &[json!({"id": "v1", "name": "data"})]);
        assert_eq!(
            d.snapshots
                .iter()
                .map(|s| (s.id.as_str(), s.volume_name.as_deref()))
                .collect::<Vec<_>>(),
            [("s1", Some("data"))]
        );
    }

    #[test]
    fn group_volume_choices_offer_members_and_volumes_the_group_can_take() {
        let groups = [
            json!({"id": "g1", "volume_types": ["vt-ssd"]}),
            json!({"id": "g2", "volume_types": ["vt-hdd"]}),
        ];
        let types = [
            json!({"id": "vt-ssd", "name": "ssd"}),
            json!({"id": "vt-hdd", "name": "hdd"}),
        ];
        let vols = [
            json!({"id": "v1", "name": "in-g1", "size": 10, "status": "in-use", "volume_type": "hdd", "group_id": "g1"}),
            json!({"id": "v2", "name": "free-ssd", "size": 5, "status": "available", "volume_type": "ssd", "group_id": null}),
            json!({"id": "v3", "name": "busy-ssd", "size": 5, "status": "creating", "volume_type": "ssd"}),
            json!({"id": "v4", "name": "in-g2", "size": 5, "status": "available", "volume_type": "ssd", "group_id": "g2"}),
            // From 3.61 volumes also carry the type's ID.
            json!({"id": "v5", "name": "by-id", "size": 1, "status": "available", "volume_type": "x", "volume_type_id": "vt-hdd"}),
        ];
        let c = group_volume_choices(&groups, &vols, &types);
        let of = |g: &str| {
            c.iter()
                .filter(|x| x.cells == [g])
                .map(|x| x.value.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            of("g1"),
            ["v2", "v1"],
            "a member stays even with another type; sorted by label"
        );
        assert_eq!(
            of("g2"),
            ["v5", "v4"],
            "type matched by ID; not v3 (creating) nor g1's volumes"
        );
        assert_eq!(c.iter().find(|x| x.value == "v2").unwrap().label, "free-ssd (5 GiB)");
    }

    #[test]
    fn snapshots_resolve_volume_names() {
        let s = snapshots(
            &[json!({"id": "s1", "name": "snap", "size": 5, "volume_id": "v1", "created_at": "2026-01-01"})],
            &[json!({"id": "v1", "name": "data"})],
        );
        assert_eq!(s[0].volume_name.as_deref(), Some("data"));
    }

    #[test]
    fn volume_detail_lists_its_snapshots_and_source() {
        let v = json!({"id": "v", "name": "data", "size": 10, "bootable": "true", "created_at": "2026-01-01T00:00:00",
            "volume_image_metadata": {"image_name": "rhel-9.4", "image_id": "img"}});
        let snaps = vec![
            json!({"id": "s1", "volume_id": "v", "name": "a"}),
            json!({"id": "s2", "volume_id": "other"}),
        ];
        let d = volume_detail(&v, &[], &snaps);
        assert_eq!(d.source_image.as_deref(), Some("rhel-9.4"));
        assert_eq!(d.snapshots.len(), 1);
        assert_eq!(d.snapshots[0].volume_name.as_deref(), Some("data"));
    }

    #[test]
    fn boot_from_volume_takes_the_image_from_the_boot_volume() {
        let mut sv = server_fixture();
        sv["image"] = json!("");
        sv["os-extended-volumes:volumes_attached"] = json!([{"id": "data"}, {"id": "root"}]);
        let id = sv["id"].clone();
        let volumes = vec![
            json!({"id": "data", "bootable": "false", "attachments": [{"server_id": id, "device": "/dev/vdb"}]}),
            json!({"id": "root", "bootable": "true", "attachments": [{"server_id": id, "device": "/dev/vda"}],
                   "volume_image_metadata": {"image_name": "Ubuntu-24.04LTS", "image_id": "img-u"}}),
        ];
        let vm = &instances(&[sv.clone()], &flavors(), &[], &volumes)[0];
        assert_eq!(vm.image_name.as_deref(), Some("Ubuntu-24.04LTS"));
        assert!(vm.image_from_volume);

        // root_device_name (admin field) wins over conventions
        sv["OS-EXT-SRV-ATTR:root_device_name"] = json!("/dev/vdb");
        let volumes2 = vec![
            json!({"id": "data", "attachments": [{"server_id": id, "device": "/dev/vdb"}],
                   "volume_image_metadata": {"image_id": "img-r"}}),
            volumes[1].clone(),
        ];
        let vm = &instances(
            &[sv.clone()],
            &flavors(),
            &[json!({"id": "img-r", "name": "rhel"})],
            &volumes2,
        )[0];
        assert_eq!(
            vm.image_name.as_deref(),
            Some("rhel"),
            "image id resolved through Glance names"
        );

        // no volume metadata at all → still None, not a crash
        let vm = &instances(&[sv], &flavors(), &[], &[])[0];
        assert_eq!(vm.image_name, None);
        assert!(!vm.image_from_volume);
    }

    #[test]
    fn volume_messages_keep_this_volume_newest_first() {
        let msgs = vec![
            json!({"id": "m1", "resource_uuid": "v1", "message_level": "ERROR", "created_at": "2026-10-01T10:00:00",
                   "user_message": "create volume: Could not find any available weighted backend.", "request_id": "req-1"}),
            json!({"id": "m2", "resource_uuid": "v2", "message_level": "ERROR", "created_at": "2026-10-02T10:00:00", "user_message": "x"}),
            json!({"id": "m3", "resource_uuid": "v1", "message_level": "ERROR", "created_at": "2026-10-03T10:00:00",
                   "user_message": "", "event_id": "VOLUME_000002"}),
        ];
        let out = volume_messages(&msgs, "v1");
        assert_eq!(out.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["m3", "m1"]);
        assert_eq!(
            out[0].message, "VOLUME_000002",
            "empty user message falls back to the event"
        );
        assert_eq!(out[1].request_id.as_deref(), Some("req-1"));
    }

    #[test]
    fn boot_images_are_found_from_the_volumes_alone() {
        let volumes = vec![
            // c: a data volume first, then the root one.
            json!({"id": "data", "bootable": "false", "attachments": [{"server_id": "c", "device": "/dev/vdb"}]}),
            json!({"id": "root", "bootable": "true", "attachments": [{"server_id": "c", "device": "/dev/vda"}],
                   "volume_image_metadata": {"image_name": "rhel-9.4", "image_id": "img-r"}}),
            // d: no conventional device, the bootable volume decides; only an image ID.
            json!({"id": "v2", "bootable": "true", "attachments": [{"server_id": "d", "device": "/dev/xyz"}],
                   "volume_image_metadata": {"image_id": "img-u"}}),
            // e: a volume without image metadata names nothing.
            json!({"id": "v3", "bootable": "true", "attachments": [{"server_id": "e", "device": "/dev/vda"}]}),
        ];
        assert_eq!(
            boot_images(&volumes),
            [
                ("c".to_string(), "rhel-9.4".to_string()),
                ("d".to_string(), "img-u".to_string())
            ]
        );
        assert!(boot_images(&[]).is_empty());
    }

    #[test]
    fn volume_snapshot_and_floating_ip_pages_keep_the_apis_order() {
        let vols = vec![
            json!({"id": "v2", "name": "zeta", "attachments": [{"server_id": "s1", "device": "/dev/vdb"}]}),
            json!({"id": "v1", "name": "alpha"}),
        ];
        let page = volumes_page(&vols, &[]);
        assert_eq!(
            page.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
            ["v2", "v1"],
            "Cinder's order: the marker is the last row"
        );
        assert_eq!(
            (
                page[0].attachments[0].server_id.as_str(),
                page[0].attachments[0].server_name.as_deref()
            ),
            ("s1", None),
            "named later"
        );
        let snaps = vec![
            json!({"id": "a", "created_at": "2020-01-01", "volume_id": "v1"}),
            json!({"id": "b", "created_at": "2026-01-01", "volume_id": "v1"}),
        ];
        assert_eq!(
            snapshots_page(&snaps, &[])
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        let fips = vec![
            json!({"id": "f2", "floating_ip_address": "10.0.0.9", "floating_network_id": "ext", "port_details": {"device_id": "s1", "device_owner": "compute:nova"}}),
            json!({"id": "f1", "floating_ip_address": "10.0.0.1", "floating_network_id": "ext"}),
        ];
        let page = floating_ips_page(&fips, &[json!({"id": "ext", "name": "public"})], &[]);
        assert_eq!(page.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), ["f2", "f1"]);
        assert_eq!(
            (
                page[0].pool.as_str(),
                page[0].instance_id.as_deref(),
                page[0].instance_name.as_deref()
            ),
            ("public", Some("s1"), None)
        );
        assert_eq!(pool_ids(&fips), ["ext"], "one network lookup for the whole page");
    }
}
