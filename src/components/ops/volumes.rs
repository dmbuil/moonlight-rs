//! Volumes, snapshots, backups, transfers and volume groups.

use super::*;

/// *Create Volume*; `preset` = `(source, source ID, minimum size in GiB)` when
/// opened from an image, snapshot or volume row.
pub fn create_volume_dialog(lang: Lang, preset: Option<(VolumeSource, String, Option<u32>)>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let (src, src_id, size) = match &preset {
        Some((s, id, size)) => (*s, Some(id.clone()), *size),
        None => (VolumeSource::Empty, None, None),
    };
    let mut fields = vec![
        Field::text("name", tr(K::VolumeName)).required(),
        Field::area("description", tr(K::Description)),
    ];
    if preset.is_none() {
        fields.push(
            Field::select(
                "source",
                tr(K::VolumeSource),
                options(
                    &[
                        ("empty", K::SourceEmpty),
                        ("image", K::BootImage),
                        ("snapshot", K::Snapshots),
                        ("volume", K::Volume),
                    ],
                    lang,
                ),
            )
            .value("empty")
            .required(),
        );
        fields.push(
            Field::select("source_id", tr(K::SourceItem), Source::Set(ChoiceSet::Images)).when("source", "image"),
        );
        fields.push(
            Field::select(
                "source_snap",
                tr(K::SourceItem),
                Source::Set(ChoiceSet::VolumeSnapshots),
            )
            .when("source", "snapshot"),
        );
        fields.push(
            Field::select(
                "source_vol",
                tr(K::SourceItem),
                Source::Set(ChoiceSet::AvailableVolumes),
            )
            .when("source", "volume"),
        );
    }
    fields.push(Field::select(
        "volume_type",
        tr(K::VolumeType),
        Source::Set(ChoiceSet::VolumeTypes),
    ));
    fields.push(
        Field::number("size", tr(K::SizeGb), 1)
            .value(size.unwrap_or(1).to_string())
            .required(),
    );
    fields.push(
        Field::select("az", tr(K::AvailabilityZone), Source::Set(ChoiceSet::VolumeZones))
            .empty_label(tr(K::AnyAvailabilityZone)),
    );

    Dialog::form(tr(K::CreateVolume), tr(K::CreateVolume), fields, move |v| {
        let (source, source_id) = match &src_id {
            Some(id) => (src, Some(id.clone())),
            None => match v.get("source").as_str() {
                "image" => (VolumeSource::Image, v.opt("source_id")),
                "snapshot" => (VolumeSource::Snapshot, v.opt("source_snap")),
                "volume" => (VolumeSource::Volume, v.opt("source_vol")),
                _ => (VolumeSource::Empty, None),
            },
        };
        Ok(OsAction::VolumeCreate {
            name: v.get("name"),
            description: v.opt("description"),
            size_gb: num_of::<u32>(v, "size", K::SizeGb, lang)?.unwrap_or(1),
            volume_type: v.opt("volume_type"),
            availability_zone: v.opt("az"),
            source,
            source_id,
        })
    })
}

pub(super) fn create_volume_item(lang: Lang, src: VolumeSource, id: &str, size: Option<u32>) -> MenuItem {
    let id = id.to_string();
    MenuItem::dialog(t(lang, K::CreateVolume), move || {
        create_volume_dialog(lang, Some((src, id.clone(), size)))
    })
}

/// A volume's menu: launch, edit, extend, snapshot, change type, upload to image,
/// attach, transfer and delete, as the volume's state allows (like Horizon).
pub fn volume_items(lang: Lang, v: &Volume) -> Vec<MenuItem> {
    let tr = |k| t(lang, k).to_string();
    let mut items = Vec::new();
    if v.bootable && v.status.eq_ignore_ascii_case("available") {
        let i = v.id.clone();
        items.push(MenuItem::dialog(tr(K::LaunchAsInstance), move || {
            launch_from(lang, "volume", &i)
        }));
    }
    let (i, n, d) = (v.id.clone(), v.name.clone(), v.description.clone().unwrap_or_default());
    items.push(MenuItem::dialog(tr(K::EditVolume), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditVolume),
            t(lang, K::Save),
            vec![
                Field::text("name", t(lang, K::VolumeName)).value(n.clone()),
                Field::area("description", t(lang, K::Description)).value(d.clone()),
            ],
            move |v| {
                Ok(OsAction::VolumeUpdate {
                    id: i.clone(),
                    name: v.get("name"),
                    description: v.opt("description"),
                })
            },
        )
    }));
    let (i, size) = (v.id.clone(), v.size_gb);
    items.push(MenuItem::dialog(tr(K::ExtendVolume), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::ExtendVolume),
            t(lang, K::ExtendVolume),
            vec![
                Field::number("size", t(lang, K::NewSizeGb), size + 1)
                    .value((size + 1).to_string())
                    .required(),
            ],
            move |v| {
                let new = num_of::<u32>(v, "size", K::NewSizeGb, lang)?.unwrap_or(0);
                if (new as i64) <= size {
                    return Err(format!("{} > {size}", t(lang, K::NewSizeGb)));
                }
                Ok(OsAction::VolumeExtend {
                    id: i.clone(),
                    new_size_gb: new,
                })
            },
        )
    }));
    let (i, n, attached) = (v.id.clone(), v.name.clone(), !v.attachments.is_empty());
    items.push(MenuItem::dialog(tr(K::CreateSnapshot), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::CreateSnapshot),
            t(lang, K::CreateSnapshot),
            vec![
                Field::text("name", t(lang, K::SnapshotName))
                    .value(format!("{n}-snapshot"))
                    .required(),
                Field::area("description", t(lang, K::Description)),
                Field::check("force", t(lang, K::ForceSnapshot), attached),
            ],
            move |v| {
                Ok(OsAction::VolumeSnapshotCreate {
                    volume_id: i.clone(),
                    name: req(v, "name", K::SnapshotName, lang)?,
                    description: v.opt("description"),
                    force: v.flag("force"),
                })
            },
        )
    }));
    if needs::BACKUP.met(&caps()) {
        let (i, n, attached) = (v.id.clone(), v.name.clone(), !v.attachments.is_empty());
        items.push(MenuItem::dialog(tr(K::CreateBackup), move || {
            backup_form(lang, Some(i.clone()), format!("{n}-backup"), attached)
        }));
    }
    let status = v.status.to_ascii_lowercase();
    // Horizon: retype and upload for available or in-use volumes, transfer only when available.
    let c = caps();
    if status == "available" || status == "in-use" {
        let (i, current) = (v.id.clone(), v.volume_type.clone().unwrap_or_default());
        items.push(MenuItem::dialog(tr(K::ChangeVolumeType), move || {
            let (i, current) = (i.clone(), current.clone());
            Dialog::form(
                t(lang, K::ChangeVolumeType),
                t(lang, K::ChangeVolumeType),
                vec![
                    Field::select("type", t(lang, K::NewVolumeType), Source::Set(ChoiceSet::VolumeTypes)).required(),
                    Field::check("migrate", t(lang, K::MigrateOnDemand), false).help(t(lang, K::MigrateHelp)),
                ],
                move |v| {
                    let new_type = req(v, "type", K::NewVolumeType, lang)?;
                    if new_type == current {
                        return Err(t(lang, K::SameVolumeType).to_string());
                    }
                    Ok(OsAction::VolumeRetype {
                        id: i.clone(),
                        new_type,
                        migrate: v.flag("migrate"),
                    })
                },
            )
        }));
    }
    if (status == "available" || status == "in-use") && needs::UPLOAD_TO_IMAGE.met(&c) {
        let (i, n, attached) = (v.id.clone(), v.name.clone(), status == "in-use");
        items.push(MenuItem::dialog(tr(K::UploadToImage), move || {
            let i = i.clone();
            let img = crate::components::dialog::prefs().images;
            let formats = upload_formats(&img.disk_formats);
            let first = if formats.contains(&"raw") {
                "raw"
            } else {
                formats.first().copied().unwrap_or("raw")
            };
            let mut fields = vec![
                Field::text("name", t(lang, K::ImageName)).value(n.clone()).required(),
                Field::select("format", t(lang, K::DiskFormat), options_raw(&formats))
                    .value(first)
                    .required(),
                Field::select(
                    "visibility",
                    t(lang, K::Visibility),
                    options_raw(&crate::prefs::VISIBILITIES),
                )
                .value(img.default_visibility.clone())
                .required(),
            ];
            if attached {
                fields.push(Field::check("force", t(lang, K::ForceUpload), false));
            }
            Dialog::form(t(lang, K::UploadToImage), t(lang, K::UploadToImage), fields, move |v| {
                Ok(OsAction::VolumeUploadImage {
                    id: i.clone(),
                    image_name: req(v, "name", K::ImageName, lang)?,
                    disk_format: req(v, "format", K::DiskFormat, lang)?,
                    force: v.flag("force"),
                    visibility: v.opt("visibility"),
                })
            })
        }));
    }
    if status == "available" {
        let (i, n) = (v.id.clone(), v.name.clone());
        items.push(MenuItem::dialog(tr(K::CreateTransfer), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::CreateTransfer),
                t(lang, K::CreateTransfer),
                vec![
                    Field::text("name", t(lang, K::TransferName))
                        .value(n.clone())
                        .help(t(lang, K::TransferHelp)),
                ],
                move |v| {
                    Ok(OsAction::VolumeTransferCreate {
                        volume_id: i.clone(),
                        name: v.opt("name"),
                    })
                },
            )
        }));
    }
    if v.attachments.is_empty() {
        let i = v.id.clone();
        items.push(MenuItem::dialog(tr(K::AttachVolume), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::AttachVolume),
                t(lang, K::AttachVolume),
                vec![Field::select("server", t(lang, K::Instance), Source::Set(ChoiceSet::Servers)).required()],
                move |v| {
                    Ok(OsAction::ServerAttachVolume {
                        server_id: req(v, "server", K::Instance, lang)?,
                        volume_id: i.clone(),
                    })
                },
            )
        }));
    }
    for a in &v.attachments {
        items.push(detach_volume_item(
            lang,
            &a.server_id,
            &v.id,
            &format!("{} → {}", v.name, a.server_name.as_deref().unwrap_or(&a.server_id)),
        ));
    }
    let i = v.id.clone();
    items.push(metadata_item(
        lang,
        "OS::Cinder::Volume",
        &v.metadata,
        move |metadata, _| OsAction::VolumeSetMetadata {
            id: i.clone(),
            metadata,
        },
    ));
    let (i, n) = (v.id.clone(), v.name.clone());
    items.push(MenuItem::danger(tr(K::DeleteVolume), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteVolume),
            n.clone(),
            OsAction::VolumeDelete { id: i.clone() },
        )
    }));
    items
}

/// Volumes header: take ownership of a volume another project transferred.
pub fn accept_transfer_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::AcceptTransfer),
        t(lang, K::AcceptTransfer),
        vec![
            Field::text("id", t(lang, K::TransferId)).required(),
            Field::text("key", t(lang, K::AuthKey)).required(),
        ],
        move |v| {
            Ok(OsAction::VolumeTransferAccept {
                transfer_id: req(v, "id", K::TransferId, lang)?,
                auth_key: req(v, "key", K::AuthKey, lang)?,
            })
        },
    )
}

/// *Create Volume Backup* (roadmap 5, `backup` plugin). From a volume's menu `volume_id`
/// is fixed; from the Backups list the volume is picked (available ones: an attached
/// volume is backed up from its own menu, which preselects *Force*).
pub fn backup_form(lang: Lang, volume_id: Option<String>, name: String, attached: bool) -> Dialog {
    let tr = move |k| t(lang, k).to_string();
    let mut fields = Vec::new();
    if volume_id.is_none() {
        fields.push(Field::select("volume", tr(K::Volume), Source::Set(ChoiceSet::AvailableVolumes)).required());
    }
    fields.push(Field::text("name", tr(K::BackupName)).value(name).required());
    fields.push(Field::area("description", tr(K::Description)));
    fields.push(Field::check("incremental", tr(K::IncrementalBackup), false).help(tr(K::IncrementalBackupHelp)));
    fields.push(Field::check("force", tr(K::ForceBackup), attached));
    Dialog::form(tr(K::CreateBackup), tr(K::CreateBackup), fields, move |v| {
        let volume_id = match &volume_id {
            Some(i) => i.clone(),
            None => req(v, "volume", K::Volume, lang)?,
        };
        Ok(OsAction::BackupCreate {
            volume_id,
            name: req(v, "name", K::BackupName, lang)?,
            description: v.opt("description"),
            incremental: v.flag("incremental"),
            force: v.flag("force"),
            snapshot_id: None,
        })
    })
}

/// Volumes › Backups' *Create Volume Backup* button.
pub fn create_backup(lang: Lang) -> Dialog {
    backup_form(lang, None, String::new(), false)
}

/// A volume backup's menu: restore (only when `available`), edit, delete.
pub fn backup_items(lang: Lang, b: &BackupRow) -> Vec<MenuItem> {
    let tr = move |k| t(lang, k).to_string();
    let mut items = Vec::new();
    if b.status.eq_ignore_ascii_case("available") {
        let (i, n) = (b.id.clone(), b.name.clone());
        items.push(MenuItem::dialog(tr(K::RestoreBackup), move || {
            let i = i.clone();
            let fields = vec![
                Field::select(
                    "volume",
                    tr(K::RestoreToVolume),
                    Source::Set(ChoiceSet::AvailableVolumes),
                )
                .help(tr(K::RestoreToVolumeHelp)),
                Field::text("name", tr(K::RestoredVolumeName)).value(format!("restore_backup_{n}")),
            ];
            Dialog::form(tr(K::RestoreBackup), tr(K::RestoreBackup), fields, move |v| {
                Ok(OsAction::BackupRestore {
                    id: i.clone(),
                    volume_id: v.opt("volume"),
                    name: v.opt("name"),
                })
            })
        }));
    }
    let (i, n, d) = (b.id.clone(), b.name.clone(), b.description.clone().unwrap_or_default());
    items.push(MenuItem::dialog(tr(K::EditBackup), move || {
        let i = i.clone();
        let fields = vec![
            Field::text("name", tr(K::Name)).value(n.clone()).required(),
            Field::area("description", tr(K::Description)).value(d.clone()),
        ];
        Dialog::form(tr(K::EditBackup), tr(K::Save), fields, move |v| {
            Ok(OsAction::BackupUpdate {
                id: i.clone(),
                name: req(v, "name", K::Name, lang)?,
                description: v.opt("description"),
            })
        })
    }));
    let (i, n) = (b.id.clone(), b.name.clone());
    items.push(MenuItem::danger(tr(K::DeleteBackup), move || {
        Dialog::confirm(
            lang,
            tr(K::DeleteBackup),
            n.clone(),
            OsAction::BackupDelete { id: i.clone() },
        )
    }));
    items
}

/// A volume snapshot's menu: launch, edit, delete.
pub fn snapshot_items(lang: Lang, s: &Snapshot) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if s.status.eq_ignore_ascii_case("available") {
        let i = s.id.clone();
        items.push(MenuItem::dialog(t(lang, K::LaunchAsInstance), move || {
            launch_from(lang, "volume_snapshot", &i)
        }));
    }
    items.push(create_volume_item(
        lang,
        VolumeSource::Snapshot,
        &s.id,
        Some(s.size_gb.max(1) as u32),
    ));
    let (i, n, d) = (s.id.clone(), s.name.clone(), s.description.clone().unwrap_or_default());
    items.push(MenuItem::dialog(t(lang, K::EditSnapshot), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditSnapshot),
            t(lang, K::Save),
            vec![
                Field::text("name", t(lang, K::Name)).value(n.clone()),
                Field::area("description", t(lang, K::Description)).value(d.clone()),
            ],
            move |v| {
                Ok(OsAction::SnapshotUpdate {
                    id: i.clone(),
                    name: v.get("name"),
                    description: v.opt("description"),
                })
            },
        )
    }));
    let i = s.id.clone();
    items.push(metadata_item(
        lang,
        "OS::Cinder::Snapshot",
        &s.metadata,
        move |metadata, _| OsAction::SnapshotSetMetadata {
            id: i.clone(),
            metadata,
        },
    ));
    let (i, n) = (s.id.clone(), s.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteSnapshot), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteSnapshot),
            n.clone(),
            OsAction::SnapshotDelete { id: i.clone() },
        )
    }));
    items
}

/// Horizon's *Create Group* (Volumes › Groups): name, description, group type,
/// zone and the volume types it accepts, in one form (Horizon uses two steps).
pub fn create_volume_group(lang: Lang) -> Dialog {
    let tr = move |k| t(lang, k).to_string();
    let fields = vec![
        Field::text("name", tr(K::Name)).required(),
        Field::area("description", tr(K::Description)),
        Field::select("group_type", tr(K::GroupType), Source::Set(ChoiceSet::GroupTypes)).required(),
        Field::select("az", tr(K::AvailabilityZone), Source::Set(ChoiceSet::VolumeZones))
            .empty_label(tr(K::AnyAvailabilityZone)),
        Field::multi(
            "volume_types",
            tr(K::VolumeTypes),
            Source::Set(ChoiceSet::VolumeTypeIds),
        )
        .required()
        .help(tr(K::VolumeGroupTypesHelp)),
    ];
    Dialog::form(tr(K::CreateGroup), tr(K::CreateGroup), fields, move |v| {
        let volume_type_ids = v.list("volume_types");
        if volume_type_ids.is_empty() {
            return Err(format!("{}: {}", tr(K::VolumeTypes), tr(K::RequiredField)));
        }
        Ok(OsAction::VolumeGroupCreate {
            name: req(v, "name", K::Name, lang)?,
            description: v.opt("description"),
            group_type_id: req(v, "group_type", K::GroupType, lang)?,
            volume_type_ids,
            availability_zone: v.opt("az"),
        })
    })
}

/// A name and description form that creates a group from a source: *Clone
/// Group* (a group) or *Create Group* (a group snapshot).
fn create_group_from(
    lang: Lang,
    title: K,
    help: K,
    name: String,
    source_group_id: Option<String>,
    group_snapshot_id: Option<String>,
) -> Dialog {
    let fields = vec![
        Field::text("name", t(lang, K::Name))
            .value(name)
            .required()
            .help(t(lang, help)),
        Field::area("description", t(lang, K::Description)),
    ];
    Dialog::form(t(lang, title), t(lang, title), fields, move |v| {
        Ok(OsAction::VolumeGroupCreateFrom {
            name: req(v, "name", K::Name, lang)?,
            description: v.opt("description"),
            source_group_id: source_group_id.clone(),
            group_snapshot_id: group_snapshot_id.clone(),
        })
    })
}

/// A volume group's menu (roadmap 5), in Horizon's order: *Create Snapshot*,
/// *Edit Group*, *Clone Group*, *Delete Group* (with Horizon's *Delete Volumes*
/// option). *Manage Volumes* needs the group's volumes: see
/// [`volume_group_detail_items`].
pub fn volume_group_items(lang: Lang, g: &VolumeGroupRow) -> Vec<MenuItem> {
    let tr = move |k| t(lang, k).to_string();
    let (i, n) = (g.id.clone(), g.name.clone());
    let snapshot = MenuItem::dialog(tr(K::CreateSnapshot), move || {
        let i = i.clone();
        let fields = vec![
            Field::text("name", tr(K::SnapshotName))
                .value(format!("{n}-snapshot"))
                .required(),
            Field::area("description", tr(K::Description)),
        ];
        Dialog::form(tr(K::CreateSnapshot), tr(K::CreateSnapshot), fields, move |v| {
            Ok(OsAction::GroupSnapshotCreate {
                group_id: i.clone(),
                name: req(v, "name", K::SnapshotName, lang)?,
                description: v.opt("description"),
            })
        })
    });
    let (i, n, d) = (g.id.clone(), g.name.clone(), g.description.clone().unwrap_or_default());
    let edit = MenuItem::dialog(tr(K::EditGroup), move || {
        let i = i.clone();
        let fields = vec![
            Field::text("name", tr(K::Name)).value(n.clone()).required(),
            Field::area("description", tr(K::Description)).value(d.clone()),
        ];
        Dialog::form(tr(K::EditGroup), tr(K::Save), fields, move |v| {
            Ok(OsAction::VolumeGroupUpdate {
                id: i.clone(),
                name: req(v, "name", K::Name, lang)?,
                description: v.opt("description"),
            })
        })
    });
    let (i, n) = (g.id.clone(), g.name.clone());
    let clone = MenuItem::dialog(tr(K::CloneGroup), move || {
        create_group_from(
            lang,
            K::CloneGroup,
            K::CloneGroupHelp,
            format!("{n}-clone"),
            Some(i.clone()),
            None,
        )
    });
    let (i, n) = (g.id.clone(), g.name.clone());
    let delete = MenuItem::danger(tr(K::DeleteGroup), move || {
        let i = i.clone();
        let mut d = Dialog::confirm(
            lang,
            tr(K::DeleteGroup),
            n.clone(),
            OsAction::VolumeGroupDelete {
                id: i.clone(),
                delete_volumes: false,
            },
        );
        d.fields =
            vec![Field::check("delete_volumes", tr(K::DeleteGroupVolumes), false).help(tr(K::DeleteGroupVolumesHelp))];
        d.build = std::sync::Arc::new(move |v| {
            Ok(OsAction::VolumeGroupDelete {
                id: i.clone(),
                delete_volumes: v.flag("delete_volumes"),
            })
        });
        d
    });
    vec![snapshot, edit, clone, delete]
}

/// The group detail page's menu: the row's, plus *Manage Volumes* (its current
/// volumes preselected; the save adds and removes the difference) and, when it
/// has volumes, *Remove Volumes from Group*, both before *Clone Group* like Horizon.
pub fn volume_group_detail_items(lang: Lang, d: &VolumeGroupDetail) -> Vec<MenuItem> {
    let tr = move |k| t(lang, k).to_string();
    let mut items = volume_group_items(lang, &d.row);
    let (gid, before): (String, Vec<String>) = (d.row.id.clone(), d.volumes.iter().map(|v| v.id.clone()).collect());
    let manage = MenuItem::dialog(tr(K::ManageVolumes), move || {
        let (gid, before) = (gid.clone(), before.clone());
        let field = Field::multi(
            "volumes",
            tr(K::Volumes),
            Source::Of(ChoiceSet::GroupVolumes, gid.clone()),
        )
        .value(before.join(","))
        .help(tr(K::ManageVolumesHelp));
        Dialog::form(tr(K::ManageVolumes), tr(K::Save), vec![field], move |v| {
            let (add_volume_ids, remove_volume_ids) = list_diff(&before, &v.list("volumes"));
            if add_volume_ids.is_empty() && remove_volume_ids.is_empty() {
                return Err(tr(K::NothingChanged));
            }
            Ok(OsAction::VolumeGroupSetVolumes {
                id: gid.clone(),
                add_volume_ids,
                remove_volume_ids,
            })
        })
    });
    items.insert(1, manage);
    if !d.volumes.is_empty() {
        let (gid, ids, names): (String, Vec<String>, Vec<String>) = (
            d.row.id.clone(),
            d.volumes.iter().map(|v| v.id.clone()).collect(),
            d.volumes.iter().map(|v| v.name.clone()).collect(),
        );
        let remove = MenuItem::dialog(tr(K::RemoveGroupVolumes), move || {
            let action = OsAction::VolumeGroupSetVolumes {
                id: gid.clone(),
                add_volume_ids: vec![],
                remove_volume_ids: ids.clone(),
            };
            let mut dlg = Dialog::simple(lang, tr(K::RemoveGroupVolumes), String::new(), action);
            dlg.selection = names.clone();
            dlg.intro.push(tr(K::RemoveGroupVolumesHelp));
            dlg
        });
        items.insert(3, remove);
    }
    items
}

/// A group snapshot's menu: *Create Group* from it, *Delete Group Snapshot*.
pub fn group_snapshot_items(lang: Lang, s: &GroupSnapshotRow) -> Vec<MenuItem> {
    let (i, n) = (s.id.clone(), s.name.clone());
    let create = MenuItem::dialog(t(lang, K::CreateGroup), move || {
        create_group_from(
            lang,
            K::CreateGroup,
            K::CreateGroupFromSnapshotHelp,
            n.clone(),
            None,
            Some(i.clone()),
        )
    });
    let (i, n) = (s.id.clone(), s.name.clone());
    vec![
        create,
        MenuItem::danger(t(lang, K::DeleteGroupSnapshot), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeleteGroupSnapshot),
                n.clone(),
                OsAction::GroupSnapshotDelete { id: i.clone() },
            )
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    fn volume_group_row() -> VolumeGroupRow {
        VolumeGroupRow {
            id: "g1".into(),
            name: "db".into(),
            description: None,
            status: "available".into(),
            availability_zone: None,
            group_type: Ref {
                id: "gt".into(),
                name: "gt".into(),
            },
            volume_types: vec![],
            has_snapshots: false,
            created: String::new(),
        }
    }

    /// The dialog behind the menu item labelled `label`.
    fn item_dialog(items: &[MenuItem], label: &str) -> Dialog {
        let item = items
            .iter()
            .find(|m| m.label == label)
            .unwrap_or_else(|| panic!("no {label}"));
        let crate::components::dialog::ItemKind::Dialog(open) = &item.kind else {
            panic!("{label} opens no dialog")
        };
        open()
    }

    fn backup_row(status: &str) -> BackupRow {
        BackupRow {
            id: "b1".into(),
            name: "nightly".into(),
            description: None,
            status: status.into(),
            size_gb: 5,
            volume: Ref {
                id: "v1".into(),
                name: "data".into(),
            },
            incremental: false,
            container: None,
            snapshot_id: None,
            availability_zone: None,
            fail_reason: None,
            created: String::new(),
        }
    }

    #[test]
    fn backup_menu_restores_only_available_backups_and_deletes_last() {
        let labels = |s: &str| {
            backup_items(Lang::EnUs, &backup_row(s))
                .into_iter()
                .map(|m| m.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels("available"),
            ["Restore Backup", "Edit Backup", "Delete Volume Backup"]
        );
        assert_eq!(
            labels("creating"),
            ["Edit Backup", "Delete Volume Backup"],
            "nothing to restore yet"
        );
        let items = backup_items(Lang::EnUs, &backup_row("available"));
        let d = item_dialog(&items, "Delete Volume Backup");
        assert!(d.danger && d.selection == ["nightly"]);
        let d = item_dialog(&items, "Restore Backup");
        assert_eq!(
            (d.build)(&vals(&[("volume", ""), ("name", "copy")])).unwrap(),
            OsAction::BackupRestore {
                id: "b1".into(),
                volume_id: None,
                name: Some("copy".into())
            },
            "empty volume = new volume"
        );
        assert_eq!(
            (d.build)(&vals(&[("volume", "v9")])).unwrap(),
            OsAction::BackupRestore {
                id: "b1".into(),
                volume_id: Some("v9".into()),
                name: None
            }
        );
    }

    #[test]
    fn create_backup_asks_for_the_volume_only_from_the_list() {
        let l = Lang::EnUs;
        let d = create_backup(l);
        assert!((d.build)(&vals(&[("name", "n")])).is_err(), "a volume is required");
        assert_eq!(
            (d.build)(&vals(&[("volume", "v1"), ("name", "n"), ("incremental", "true")])).unwrap(),
            OsAction::BackupCreate {
                volume_id: "v1".into(),
                name: "n".into(),
                description: None,
                incremental: true,
                force: false,
                snapshot_id: None
            }
        );
        let d = backup_form(l, Some("v2".into()), "x-backup".into(), true);
        assert!(d.fields.iter().all(|f| f.name != "volume"));
        assert_eq!(
            d.fields.iter().find(|f| f.name == "force").unwrap().default,
            "true",
            "attached volumes preselect Force"
        );
    }

    #[test]
    fn deleting_a_volume_group_offers_to_delete_its_volumes() {
        let items = volume_group_items(Lang::EnUs, &volume_group_row());
        let d = item_dialog(&items, "Delete Group");
        assert_eq!(items.last().unwrap().label, "Delete Group", "delete last");
        assert!(
            d.danger && items.last().unwrap().danger,
            "destructive: red, with the confirmation tick"
        );
        assert_eq!(d.selection, ["db"]);
        assert_eq!(d.fields[0].default, "false", "volumes are kept unless ticked");
        assert_eq!(
            (d.build)(&vals(&[("delete_volumes", "true")])).unwrap(),
            OsAction::VolumeGroupDelete {
                id: "g1".into(),
                delete_volumes: true
            }
        );
        assert_eq!(
            (d.build)(&vals(&[])).unwrap(),
            OsAction::VolumeGroupDelete {
                id: "g1".into(),
                delete_volumes: false
            }
        );
    }

    #[test]
    fn volume_group_dialogs_build_their_requests() {
        let l = Lang::EnUs;
        let d = create_volume_group(l);
        assert!(
            (d.build)(&vals(&[("name", "db"), ("group_type", "gt1")])).is_err(),
            "at least one volume type"
        );
        assert_eq!(
            (d.build)(&vals(&[
                ("name", "db"),
                ("group_type", "gt1"),
                ("volume_types", "vt1,vt2"),
                ("az", "")
            ]))
            .unwrap(),
            OsAction::VolumeGroupCreate {
                name: "db".into(),
                description: None,
                group_type_id: "gt1".into(),
                volume_type_ids: vec!["vt1".into(), "vt2".into()],
                availability_zone: None
            }
        );

        let labels = |items: &[MenuItem]| items.iter().map(|m| m.label.clone()).collect::<Vec<_>>();
        let row = volume_group_row();
        assert_eq!(
            labels(&volume_group_items(l, &row)),
            ["Create Snapshot", "Edit Group", "Clone Group", "Delete Group"]
        );
        let clone = item_dialog(&volume_group_items(l, &row), "Clone Group");
        assert_eq!(clone.fields[0].default, "db-clone");
        assert_eq!(
            (clone.build)(&vals(&[("name", "db-clone")])).unwrap(),
            OsAction::VolumeGroupCreateFrom {
                name: "db-clone".into(),
                description: None,
                source_group_id: Some("g1".into()),
                group_snapshot_id: None
            }
        );

        // The detail page knows the volumes: Manage Volumes saves the difference.
        let vol = |id: &str| Volume {
            id: id.into(),
            name: format!("vol-{id}"),
            ..volume("available")
        };
        let detail = VolumeGroupDetail {
            row: row.clone(),
            volumes: vec![vol("v1"), vol("v2")],
            snapshots: vec![],
        };
        let items = volume_group_detail_items(l, &detail);
        assert_eq!(
            labels(&items),
            [
                "Create Snapshot",
                "Manage Volumes",
                "Edit Group",
                "Remove Volumes from Group",
                "Clone Group",
                "Delete Group"
            ]
        );
        let manage = item_dialog(&items, "Manage Volumes");
        assert_eq!(manage.fields[0].default, "v1,v2", "current volumes preselected");
        assert_eq!(
            (manage.build)(&vals(&[("volumes", "v2,v3")])).unwrap(),
            OsAction::VolumeGroupSetVolumes {
                id: "g1".into(),
                add_volume_ids: vec!["v3".into()],
                remove_volume_ids: vec!["v1".into()]
            }
        );
        assert!(
            (manage.build)(&vals(&[("volumes", "v1,v2")])).is_err(),
            "nothing changed"
        );
        let remove = item_dialog(&items, "Remove Volumes from Group");
        assert_eq!(remove.selection, ["vol-v1", "vol-v2"]);
        assert!(!remove.danger, "the volumes are kept");
        let empty = VolumeGroupDetail {
            volumes: vec![],
            ..detail
        };
        assert!(!labels(&volume_group_detail_items(l, &empty)).contains(&"Remove Volumes from Group".to_string()));

        let gs = GroupSnapshotRow {
            id: "gs1".into(),
            name: "nightly".into(),
            description: None,
            status: "available".into(),
            group: Ref {
                id: "g1".into(),
                name: "db".into(),
            },
            group_type: Ref {
                id: "gt".into(),
                name: "gt".into(),
            },
            created: String::new(),
        };
        let from = item_dialog(&group_snapshot_items(l, &gs), "Create Group");
        assert_eq!(
            (from.build)(&vals(&[("name", "restored")])).unwrap(),
            OsAction::VolumeGroupCreateFrom {
                name: "restored".into(),
                description: None,
                source_group_id: None,
                group_snapshot_id: Some("gs1".into())
            }
        );
    }

    #[test]
    fn volume_menu_follows_status_and_retype_needs_a_new_type() {
        let labels = |st| {
            volume_items(Lang::EnUs, &volume(st))
                .into_iter()
                .map(|m| m.label)
                .collect::<Vec<_>>()
        };
        let avail = labels("available");
        for l in ["Change Volume Type", "Upload to Image", "Create Transfer"] {
            assert!(avail.contains(&l.to_string()), "{l}");
        }
        let in_use = labels("in-use");
        assert!(in_use.contains(&"Upload to Image".to_string()) && !in_use.contains(&"Create Transfer".to_string()));
        assert!(!labels("error").contains(&"Change Volume Type".to_string()));
        let items = volume_items(Lang::EnUs, &volume("available"));
        let crate::components::dialog::ItemKind::Dialog(open) =
            &items.iter().find(|m| m.label == "Change Volume Type").unwrap().kind
        else {
            panic!()
        };
        let d = open();
        assert!((d.build)(&vals(&[("type", "std")])).is_err());
        assert_eq!(
            (d.build)(&vals(&[("type", "fast"), ("migrate", "true")])).unwrap(),
            OsAction::VolumeRetype {
                id: "v".into(),
                new_type: "fast".into(),
                migrate: true
            }
        );
    }

    #[test]
    fn update_metadata_submits_only_changes() {
        let kv = |p: &[(&str, &str)]| {
            p.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
        };
        let json = |p: &[(&str, &str)]| serde_json::to_string(&kv(p)).unwrap();
        let v = Volume {
            metadata: kv(&[("env", "dev")]),
            ..volume("available")
        };
        let items = volume_items(Lang::EnUs, &v);
        let item = items
            .iter()
            .find(|m| m.label == "Update Metadata")
            .expect("volume menu has it");
        let crate::components::dialog::ItemKind::Dialog(open) = &item.kind else {
            panic!()
        };
        let d = open();
        assert_eq!(
            d.fields[0].default,
            json(&[("env", "dev")]),
            "prefilled with the current pairs"
        );
        assert!(
            (d.build)(&vals(&[("metadata", &json(&[(" env ", "dev")]))])).is_err(),
            "nothing changed"
        );
        assert!(
            (d.build)(&vals(&[("metadata", &json(&[("a", "1"), ("a", "2")]))])).is_err(),
            "repeated key"
        );
        let ok = (d.build)(&vals(&[("metadata", &json(&[]))])).unwrap();
        assert_eq!(
            ok,
            OsAction::VolumeSetMetadata {
                id: "v".into(),
                metadata: vec![]
            }
        );
        let item = metadata_item(
            Lang::EnUs,
            "OS::Glance::Image",
            &kv(&[("a", "1"), ("b", "2")]),
            |new, old| {
                let (set, remove) = metadata_changes(old, &new);
                OsAction::ImageSetProperties {
                    id: "i".into(),
                    set,
                    remove,
                }
            },
        );
        let crate::components::dialog::ItemKind::Dialog(open) = &item.kind else {
            panic!()
        };
        let d = open();
        let out = (d.build)(&vals(&[("metadata", &json(&[("a", "10"), ("c", "3")]))])).unwrap();
        assert_eq!(
            out,
            OsAction::ImageSetProperties {
                id: "i".into(),
                set: kv(&[("a", "10"), ("c", "3")]),
                remove: vec!["b".into()]
            }
        );
        assert!(
            instance_items(Lang::EnUs, &instance("ACTIVE", false))
                .iter()
                .any(|m| m.label == "Update Metadata")
        );
    }

    #[test]
    fn extend_must_grow() {
        let v = Volume {
            id: "v".into(),
            name: "d".into(),
            description: None,
            size_gb: 10,
            status: "available".into(),
            volume_type: None,
            attachments: vec![],
            availability_zone: None,
            bootable: false,
            encrypted: false,
            metadata: vec![],
        };
        let items = volume_items(Lang::EnUs, &v);
        let crate::components::dialog::ItemKind::Dialog(open) = &items[1].kind else {
            panic!()
        };
        let d = open();
        assert!((d.build)(&vals(&[("size", "10")])).is_err());
        assert!(matches!(
            (d.build)(&vals(&[("size", "11")])),
            Ok(OsAction::VolumeExtend { new_size_gb: 11, .. })
        ));
    }
}
