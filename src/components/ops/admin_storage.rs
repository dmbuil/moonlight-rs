//! Admin › Volume and Metadata (roadmap 85): volumes, types, QoS specs, metadefs.

use super::*;

/// Admin › Volumes' and Snapshots' *Update Status*: Horizon's choices less the
/// current status, labelled like the status pills.
fn reset_status_item(
    lang: Lang,
    title: K,
    statuses: &'static [&'static str],
    current: &str,
    name: &str,
    make: impl Fn(String) -> OsAction + Send + Sync + 'static,
) -> MenuItem {
    let choices: Vec<(String, String)> = statuses
        .iter()
        .filter(|s| !s.eq_ignore_ascii_case(current))
        .map(|s| (s.to_string(), crate::i18n::resource_status(lang, s)))
        .collect();
    let (n, make) = (name.to_string(), std::sync::Arc::new(make));
    MenuItem::dialog(t(lang, title), move || {
        let make = make.clone();
        let mut d = Dialog::form(
            t(lang, title),
            t(lang, title),
            vec![
                Field::select("status", t(lang, K::Status), Source::Static(choices.clone()))
                    .required()
                    .help(t(lang, K::ResetStatusHelp)),
            ],
            move |v| Ok(make(req(v, "status", K::Status, lang)?)),
        );
        d.intro = vec![n.clone()];
        d
    })
}

/// Admin › Volumes' row menu, Horizon's admin table: *Update Volume Status*,
/// *Migrate Volume* (from `host`, its current pool), *Update Metadata*, then
/// *Unmanage Volume* and *Delete Volume*, both destructive.
pub fn admin_volume_items(lang: Lang, v: &Volume, host: Option<&str>) -> Vec<MenuItem> {
    let i = v.id.clone();
    let mut items = vec![reset_status_item(
        lang,
        K::UpdateVolumeStatus,
        VOLUME_STATUSES,
        &v.status,
        &v.name,
        move |status| OsAction::VolumeResetStatus { id: i.clone(), status },
    )];
    let (i, n, from) = (v.id.clone(), v.name.clone(), host.unwrap_or("-").to_string());
    items.push(MenuItem::dialog(t(lang, K::MigrateVolume), move || {
        let i = i.clone();
        let mut d = Dialog::form(
            t(lang, K::MigrateVolume),
            t(lang, K::MigrateVolume),
            vec![
                // Cinder refuses migrating to the pool the volume is on.
                Field::select(
                    "host",
                    t(lang, K::DestinationHost),
                    Source::Except(ChoiceSet::CinderPools, from.clone()),
                )
                .required(),
                Field::check("force", t(lang, K::ForceHostCopy), false).help(t(lang, K::ForceHostCopyHelp)),
            ],
            move |v| {
                Ok(OsAction::VolumeMigrate {
                    id: i.clone(),
                    host: req(v, "host", K::DestinationHost, lang)?,
                    force_host_copy: v.flag("force"),
                })
            },
        );
        d.intro = vec![n.clone(), format!("{}: {from}", t(lang, K::CurrentHost))];
        d
    }));
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
    items.push(MenuItem::danger(t(lang, K::UnmanageVolume), move || {
        let mut d = Dialog::confirm(
            lang,
            t(lang, K::UnmanageVolume),
            n.clone(),
            OsAction::VolumeUnmanage { id: i.clone() },
        );
        d.intro.push(t(lang, K::UnmanageHelp).to_string());
        d
    }));
    let (i, n) = (v.id.clone(), v.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteVolume), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteVolume),
            n.clone(),
            OsAction::VolumeDelete { id: i.clone() },
        )
    }));
    items
}

/// Admin › Snapshots' row menu, Horizon's: *Update Status*, *Update Metadata*,
/// *Delete Volume Snapshot*.
pub fn admin_snapshot_items(lang: Lang, s: &Snapshot) -> Vec<MenuItem> {
    let i = s.id.clone();
    let mut items = vec![reset_status_item(
        lang,
        K::UpdateSnapshotStatus,
        SNAPSHOT_STATUSES,
        &s.status,
        &s.name,
        move |status| OsAction::SnapshotResetStatus { id: i.clone(), status },
    )];
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

/// Admin › Volumes' *Manage Volume*, Horizon's form: the back end's identifier
/// (name or ID), its Cinder host, then the new volume's name, description,
/// metadata, type, zone and bootable flag. It lands in the session's project.
pub fn manage_volume_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let id_type = options(&[("source-name", K::BackendName), ("source-id", K::BackendId)], lang);
    let mut d = Dialog::form(
        tr(K::ManageVolume),
        tr(K::ManageVolume),
        vec![
            Field::text("identifier", tr(K::Identifier))
                .required()
                .help(tr(K::IdentifierHelp)),
            Field::select("id_type", tr(K::IdentifierType), id_type)
                .value("source-name")
                .required(),
            Field::text("host", tr(K::Host)).required().help(tr(K::VolumeHostHelp)),
            Field::text("name", tr(K::Name)),
            Field::area("description", tr(K::Description)),
            Field::key_values("metadata", tr(K::Metadata), &[]),
            Field::select("volume_type", tr(K::VolumeType), Source::Set(ChoiceSet::VolumeTypes)),
            Field::select("zone", tr(K::AvailabilityZone), Source::Set(ChoiceSet::VolumeZones)),
            Field::check("bootable", tr(K::Bootable), false),
        ],
        move |v| {
            Ok(OsAction::VolumeManage(Box::new(ManageVolumeSpec {
                identifier: req(v, "identifier", K::Identifier, lang)?,
                by_id: v.get("id_type") == "source-id",
                host: req(v, "host", K::Host, lang)?,
                name: v.opt("name"),
                description: v.opt("description"),
                volume_type: v.opt("volume_type"),
                availability_zone: v.opt("zone"),
                bootable: v.flag("bootable"),
                metadata: validate_metadata(&v.pairs("metadata"))?,
            })))
        },
    );
    d.intro = vec![tr(K::ManageVolumeHelp)];
    d
}

/// *Create Volume Type* / *Create Group Type*: Horizon's name, description
/// and public flag.
fn type_fields(lang: Lang, name: &str, description: &str, public: bool) -> Vec<Field> {
    vec![
        Field::text("name", t(lang, K::Name)).value(name).required(),
        Field::area("description", t(lang, K::Description)).value(description),
        Field::check("public", t(lang, K::Public), public),
    ]
}

/// Admin › Volume Types' *Create Volume Type*.
pub fn create_volume_type_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::CreateVolumeType),
        t(lang, K::CreateVolumeType),
        type_fields(lang, "", "", true),
        move |v| {
            Ok(OsAction::VolumeTypeCreate {
                name: req(v, "name", K::Name, lang)?,
                description: v.opt("description"),
                public: v.flag("public"),
            })
        },
    )
}

/// Admin › Volume Types' row menu, Horizon's: *View Extra Specs*, *Manage QoS
/// Spec Association*, *Edit Volume Type*, *Create* / *Update Encryption*,
/// then *Delete Encryption* (when encrypted) and *Delete Volume Type*.
pub fn volume_type_items(lang: Lang, ty: &VolumeTypeRow) -> Vec<MenuItem> {
    let i = ty.id.clone();
    let mut items = vec![pairs_item(
        lang,
        K::ViewExtraSpecs,
        K::ExtraSpecs,
        K::ExtraSpecsHelp,
        None,
        &ty.extra_specs,
        move |new, old| {
            let (set, unset) = metadata_changes(old, &new);
            OsAction::VolumeTypeSetExtraSpecs {
                id: i.clone(),
                set,
                unset,
            }
        },
    )];
    let (i, n, current) = (ty.id.clone(), ty.name.clone(), ty.qos.as_ref().map(|q| q.id.clone()));
    items.push(MenuItem::dialog(t(lang, K::ManageQosAssociation), move || {
        let (i, current) = (i.clone(), current.clone());
        let mut d = Dialog::form(
            t(lang, K::ManageQosAssociation),
            t(lang, K::Save),
            vec![
                Field::select("qos", t(lang, K::QosSpec), Source::Set(ChoiceSet::QosSpecs))
                    .value(current.clone().unwrap_or_default()),
            ],
            move |v| {
                let qos_id = v.opt("qos");
                if qos_id == current {
                    return Err(t(lang, K::NothingChanged).to_string());
                }
                Ok(OsAction::VolumeTypeSetQos {
                    id: i.clone(),
                    qos_id,
                    current_qos_id: current.clone(),
                })
            },
        );
        d.intro = vec![n.clone()];
        d
    }));
    let (i, n, desc, public) = (
        ty.id.clone(),
        ty.name.clone(),
        ty.description.clone().unwrap_or_default(),
        ty.public,
    );
    items.push(MenuItem::dialog(t(lang, K::EditVolumeType), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditVolumeType),
            t(lang, K::Save),
            type_fields(lang, &n, &desc, public),
            move |v| {
                Ok(OsAction::VolumeTypeUpdate {
                    id: i.clone(),
                    name: req(v, "name", K::Name, lang)?,
                    description: v.opt("description"),
                    public: v.flag("public"),
                })
            },
        )
    }));
    let (i, n, enc) = (ty.id.clone(), ty.name.clone(), ty.encryption.clone());
    let title = if enc.is_some() {
        K::UpdateEncryption
    } else {
        K::CreateEncryption
    };
    items.push(MenuItem::dialog(t(lang, title), move || {
        encryption_dialog(lang, title, &i, &n, enc.as_ref())
    }));
    if ty.encrypted {
        let (i, n) = (ty.id.clone(), ty.name.clone());
        items.push(MenuItem::danger(t(lang, K::DeleteEncryption), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeleteEncryption),
                n.clone(),
                OsAction::VolumeTypeEncryptionDelete { id: i.clone() },
            )
        }));
    }
    let (i, n) = (ty.id.clone(), ty.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteVolumeType), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteVolumeType),
            n.clone(),
            OsAction::VolumeTypeDelete { id: i.clone() },
        )
    }));
    items
}

/// *Create* / *Update Encryption* of a volume type, prefilled from `current`.
fn encryption_dialog(lang: Lang, title: K, id: &str, name: &str, current: Option<&EncryptionSpec>) -> Dialog {
    let (i, update) = (id.to_string(), current.is_some());
    let c = current.cloned().unwrap_or(EncryptionSpec {
        provider: String::new(),
        control_location: "front-end".into(),
        cipher: None,
        key_size: None,
    });
    let mut d = Dialog::form(
        t(lang, title),
        t(lang, K::Save),
        vec![
            Field::text("provider", t(lang, K::Provider))
                .value(c.provider)
                .required()
                .help(t(lang, K::EncryptionProviderHelp)),
            Field::select(
                "control_location",
                t(lang, K::ControlLocation),
                options_raw(CONTROL_LOCATIONS),
            )
            .value(c.control_location)
            .required()
            .help(t(lang, K::ControlLocationHelp)),
            Field::text("cipher", t(lang, K::Cipher)).value(c.cipher.unwrap_or_default()),
            Field::number("key_size", t(lang, K::KeySize), 1)
                .value(c.key_size.map(|k| k.to_string()).unwrap_or_default()),
        ],
        move |v| {
            let key_size = num_u32(v, "key_size", K::KeySize, lang)?;
            let spec = EncryptionSpec {
                provider: req(v, "provider", K::Provider, lang)?,
                control_location: req(v, "control_location", K::ControlLocation, lang)?,
                cipher: v.opt("cipher"),
                key_size,
            };
            Ok(OsAction::VolumeTypeEncryptionSet {
                id: i.clone(),
                spec,
                update,
            })
        },
    );
    d.intro = vec![name.to_string()];
    d
}

/// Admin › Volume Types' *Create QoS Spec*: name, consumer and the specs.
pub fn create_qos_spec_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::CreateQosSpec),
        t(lang, K::CreateQosSpec),
        vec![
            Field::text("name", t(lang, K::Name)).required(),
            Field::select("consumer", t(lang, K::Consumer), options_raw(QOS_CONSUMERS))
                .value("back-end")
                .required()
                .help(t(lang, K::QosConsumerHelp)),
            Field::key_values("specs", t(lang, K::Specs), &[]),
        ],
        move |v| {
            Ok(OsAction::QosSpecCreate {
                name: req(v, "name", K::Name, lang)?,
                consumer: req(v, "consumer", K::Consumer, lang)?,
                specs: validate_metadata(&v.pairs("specs"))?,
            })
        },
    )
}

/// The *QoS Specs* table's row menu, Horizon's: *Manage Specs*, *Edit
/// Consumer*, *Delete QoS Spec*.
pub fn qos_spec_items(lang: Lang, q: &QosSpecRow) -> Vec<MenuItem> {
    let i = q.id.clone();
    let mut items = vec![pairs_item(
        lang,
        K::ManageSpecs,
        K::Specs,
        K::MetadataHelp,
        None,
        &q.specs,
        move |new, old| {
            let (set, unset) = metadata_changes(old, &new);
            OsAction::QosSpecSetSpecs {
                id: i.clone(),
                set,
                unset,
            }
        },
    )];
    let (i, n, current) = (q.id.clone(), q.name.clone(), q.consumer.clone());
    items.push(MenuItem::dialog(t(lang, K::EditConsumer), move || {
        let (i, current) = (i.clone(), current.clone());
        let mut d = Dialog::form(
            t(lang, K::EditConsumer),
            t(lang, K::Save),
            vec![
                Field::select("consumer", t(lang, K::Consumer), options_raw(QOS_CONSUMERS))
                    .value(current.clone())
                    .required()
                    .help(t(lang, K::QosConsumerHelp)),
            ],
            move |v| {
                let consumer = req(v, "consumer", K::Consumer, lang)?;
                if consumer == current {
                    return Err(t(lang, K::NothingChanged).to_string());
                }
                Ok(OsAction::QosSpecSetConsumer {
                    id: i.clone(),
                    consumer,
                })
            },
        );
        d.intro = vec![n.clone()];
        d
    }));
    let (i, n) = (q.id.clone(), q.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteQosSpec), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteQosSpec),
            n.clone(),
            OsAction::QosSpecDelete { id: i.clone() },
        )
    }));
    items
}

/// Admin › Group Types' *Create Group Type*.
pub fn create_group_type_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::CreateGroupType),
        t(lang, K::CreateGroupType),
        type_fields(lang, "", "", true),
        move |v| {
            Ok(OsAction::GroupTypeCreate {
                name: req(v, "name", K::Name, lang)?,
                description: v.opt("description"),
                public: v.flag("public"),
            })
        },
    )
}

/// Admin › Group Types' row menu, Horizon's: *View Specs*, *Edit Group Type*,
/// *Delete Group Type*.
pub fn group_type_items(lang: Lang, g: &GroupTypeRow) -> Vec<MenuItem> {
    let i = g.id.clone();
    let mut items = vec![pairs_item(
        lang,
        K::ViewSpecs,
        K::Specs,
        K::MetadataHelp,
        None,
        &g.specs,
        move |new, old| {
            let (set, unset) = metadata_changes(old, &new);
            OsAction::GroupTypeSetSpecs {
                id: i.clone(),
                set,
                unset,
            }
        },
    )];
    let (i, n, desc, public) = (
        g.id.clone(),
        g.name.clone(),
        g.description.clone().unwrap_or_default(),
        g.public,
    );
    items.push(MenuItem::dialog(t(lang, K::EditGroupType), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditGroupType),
            t(lang, K::Save),
            type_fields(lang, &n, &desc, public),
            move |v| {
                Ok(OsAction::GroupTypeUpdate {
                    id: i.clone(),
                    name: req(v, "name", K::Name, lang)?,
                    description: v.opt("description"),
                    public: v.flag("public"),
                })
            },
        )
    }));
    let (i, n) = (g.id.clone(), g.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteGroupType), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteGroupType),
            n.clone(),
            OsAction::GroupTypeDelete { id: i.clone() },
        )
    }));
    items
}

/// Largest namespace definition *Import Namespace* reads from a file
/// (Glance's own definitions are under 30 KB).
const MAX_NAMESPACE_BYTES: usize = 256 * 1024;

/// Admin › Metadata Definitions' *Import Namespace*, Horizon's form: the JSON
/// (typed or loaded from a file), *Public* and *Protected*.
pub fn import_namespace_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::ImportNamespace),
        t(lang, K::ImportNamespace),
        vec![
            Field::area("definition", t(lang, K::NamespaceDefinition))
                .required()
                .from_file(MAX_NAMESPACE_BYTES)
                .help(t(lang, K::NamespaceDefinitionHelp)),
            Field::check("public", t(lang, K::Public), false),
            Field::check("protected", t(lang, K::Protected), false),
        ],
        move |v| {
            let definition = req(v, "definition", K::NamespaceDefinition, lang)?;
            // Checked here too, so a typo is caught before the request.
            serde_json::from_str::<serde_json::Value>(&definition)
                .map_err(|e| format!("{}: {e}", t(lang, K::NamespaceDefinition)))?;
            Ok(OsAction::MetadefNamespaceImport {
                definition,
                public: v.flag("public"),
                protected: v.flag("protected"),
            })
        },
    )
}

/// Admin › Metadata Definitions' row menu: *Edit Namespace*, *Update
/// Associations*, then *Delete Namespace* unless protected (Glance refuses it;
/// Horizon hides it).
pub fn metadef_items(lang: Lang, n: &MetadefNamespace) -> Vec<MenuItem> {
    let ns = n.namespace.clone();
    let display = if n.name != n.namespace {
        n.name.clone()
    } else {
        String::new()
    };
    let (desc, public, protected) = (n.description.clone().unwrap_or_default(), n.public, n.protected);
    let mut items = vec![MenuItem::dialog(t(lang, K::EditNamespace), move || {
        let id = ns.clone();
        let mut d = Dialog::form(
            t(lang, K::EditNamespace),
            t(lang, K::Save),
            vec![
                Field::text("display_name", t(lang, K::DisplayName)).value(display.clone()),
                Field::area("description", t(lang, K::Description)).value(desc.clone()),
                Field::check("public", t(lang, K::Public), public),
                Field::check("protected", t(lang, K::Protected), protected),
            ],
            move |v| {
                Ok(OsAction::MetadefNamespaceUpdate {
                    id: id.clone(),
                    display_name: v.opt("display_name"),
                    description: v.opt("description"),
                    public: v.flag("public"),
                    protected: v.flag("protected"),
                })
            },
        );
        d.intro = vec![ns.clone()];
        d
    })];
    let (ns, current, old_prefixes) = (n.namespace.clone(), n.resource_types.clone(), n.prefixes.clone());
    items.push(MenuItem::dialog(t(lang, K::UpdateAssociations), move || {
        let (id, current, old) = (ns.clone(), current.clone(), old_prefixes.clone());
        let mut d = Dialog::form(
            t(lang, K::UpdateAssociations),
            t(lang, K::Save),
            vec![
                Field::multi(
                    "types",
                    t(lang, K::ResourceTypes),
                    Source::Set(ChoiceSet::MetadefResourceTypes),
                )
                .value(current.join(",")),
                // Horizon's per-type *Prefix* (roadmap 89): resource type → prefix.
                Field::key_values("prefixes", t(lang, K::Prefixes), &old).help(t(lang, K::PrefixesHelp)),
            ],
            move |v| {
                let (new, prefixes) = (v.list("types"), v.pairs("prefixes"));
                let prefix_of = |list: &[(String, String)], ty: &str| {
                    list.iter()
                        .find(|(t, _)| t == ty)
                        .map(|(_, p)| p.trim().to_string())
                        .unwrap_or_default()
                };
                if let Some((ty, _)) = prefixes.iter().find(|(ty, _)| !new.contains(ty)) {
                    return Err(format!("{}: {ty}", t(lang, K::PrefixNotAssociated)));
                }
                // A kept type whose prefix changed is removed and added again.
                let changed = |ty: &&String| current.contains(ty) && prefix_of(&prefixes, ty) != prefix_of(&old, ty);
                let add_type_ids: Vec<String> = new
                    .iter()
                    .filter(|x| !current.contains(x) || changed(x))
                    .cloned()
                    .collect();
                let remove_type_ids: Vec<String> = current
                    .iter()
                    .filter(|x| !new.contains(x) || changed(x))
                    .cloned()
                    .collect();
                if add_type_ids.is_empty() && remove_type_ids.is_empty() {
                    return Err(t(lang, K::NothingChanged).to_string());
                }
                Ok(OsAction::MetadefNamespaceSetTypes {
                    id: id.clone(),
                    add_type_ids,
                    remove_type_ids,
                    prefixes,
                })
            },
        );
        d.intro = vec![ns.clone()];
        d
    }));
    if !n.protected {
        let (i, name) = (n.namespace.clone(), n.name.clone());
        items.push(MenuItem::danger(t(lang, K::DeleteNamespace), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeleteNamespace),
                name.clone(),
                OsAction::MetadefNamespaceDelete { id: i.clone() },
            )
        }));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::components::dialog::Kind;

    #[test]
    fn admin_volume_menu_is_horizons_and_status_skips_the_current() {
        let v = volume("in-use");
        let items = admin_volume_items(Lang::EnUs, &v, Some("c1@ceph#fast"));
        let labels: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
        assert_eq!(
            labels,
            [
                "Update Volume Status",
                "Migrate Volume",
                "Update Metadata",
                "Unmanage Volume",
                "Delete Volume"
            ]
        );
        assert!(
            items[3].danger && items[4].danger,
            "unmanage and delete are destructive"
        );
        let crate::components::dialog::ItemKind::Dialog(open) = &items[0].kind else {
            panic!()
        };
        let d = open();
        let Kind::Select(Source::Static(choices)) = &d.fields[0].kind else {
            panic!("status choices")
        };
        assert!(
            !choices.iter().any(|(v, _)| v == "in-use") && choices.iter().any(|(v, _)| v == "available"),
            "Horizon drops the current status"
        );
        assert_eq!(
            (d.build)(&vals(&[("status", "available")])).unwrap(),
            OsAction::VolumeResetStatus {
                id: v.id.clone(),
                status: "available".into()
            }
        );
        let crate::components::dialog::ItemKind::Dialog(open) = &items[1].kind else {
            panic!()
        };
        let d = open();
        assert!(
            d.intro.iter().any(|l| l.ends_with("c1@ceph#fast")),
            "shows the current host: {:?}",
            d.intro
        );
        assert!(
            matches!(&d.fields[0].kind, Kind::Select(Source::Except(ChoiceSet::CinderPools, h)) if h == "c1@ceph#fast"),
            "Cinder refuses the current pool: not offered"
        );
        assert_eq!(
            (d.build)(&vals(&[("host", "c2@ceph#slow"), ("force", "true")])).unwrap(),
            OsAction::VolumeMigrate {
                id: v.id.clone(),
                host: "c2@ceph#slow".into(),
                force_host_copy: true
            }
        );
        let snap = Snapshot {
            id: "s1".into(),
            name: "s".into(),
            description: None,
            size_gb: 1,
            status: "error".into(),
            volume_id: "v".into(),
            volume_name: None,
            created: String::new(),
            metadata: vec![],
        };
        let labels: Vec<String> = admin_snapshot_items(Lang::EnUs, &snap)
            .into_iter()
            .map(|i| i.label)
            .collect();
        assert_eq!(labels, ["Update Status", "Update Metadata", "Delete Volume Snapshot"]);
    }

    #[test]
    fn volume_type_menu_follows_encryption_and_qos() {
        let plain = VolumeTypeRow {
            id: "t1".into(),
            name: "ssd".into(),
            description: None,
            qos: Some(Ref {
                id: "q1".into(),
                name: "gold".into(),
            }),
            encrypted: false,
            public: true,
            extra_specs: vec![("a".into(), "1".into())],
            encryption: None,
        };
        let labels = |t: &VolumeTypeRow| {
            volume_type_items(Lang::EnUs, t)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(&plain),
            [
                "View Extra Specs",
                "Manage QoS Spec Association",
                "Edit Volume Type",
                "Create Encryption",
                "Delete Volume Type"
            ]
        );
        let luks = EncryptionSpec {
            provider: "luks".into(),
            control_location: "back-end".into(),
            cipher: None,
            key_size: Some(256),
        };
        let enc = VolumeTypeRow {
            encrypted: true,
            encryption: Some(luks.clone()),
            ..plain.clone()
        };
        assert_eq!(
            labels(&enc),
            [
                "View Extra Specs",
                "Manage QoS Spec Association",
                "Edit Volume Type",
                "Update Encryption",
                "Delete Encryption",
                "Delete Volume Type"
            ]
        );
        let d = dialog_of(&volume_type_items(Lang::EnUs, &enc), "Update Encryption");
        assert_eq!(
            d.fields.iter().map(|f| f.default.as_str()).collect::<Vec<_>>(),
            ["luks", "back-end", "", "256"],
            "prefilled from the spec"
        );
        assert_eq!(
            (d.build)(&vals(&[
                ("provider", "luks"),
                ("control_location", "back-end"),
                ("key_size", "512")
            ]))
            .unwrap(),
            OsAction::VolumeTypeEncryptionSet {
                id: "t1".into(),
                spec: EncryptionSpec {
                    key_size: Some(512),
                    ..luks
                },
                update: true
            }
        );
        let d = dialog_of(&volume_type_items(Lang::EnUs, &plain), "Manage QoS Spec Association");
        assert_eq!(d.fields[0].default, "q1", "the current spec preselected");
        assert!((d.build)(&vals(&[("qos", "q1")])).is_err(), "nothing changed");
        assert_eq!(
            (d.build)(&vals(&[("qos", "")])).unwrap(),
            OsAction::VolumeTypeSetQos {
                id: "t1".into(),
                qos_id: None,
                current_qos_id: Some("q1".into())
            }
        );
        let d = dialog_of(&volume_type_items(Lang::EnUs, &plain), "View Extra Specs");
        assert_eq!(
            (d.build)(&vals(&[("metadata", r#"[["b","2"]]"#)])).unwrap(),
            OsAction::VolumeTypeSetExtraSpecs {
                id: "t1".into(),
                set: vec![("b".into(), "2".into())],
                unset: vec!["a".into()]
            }
        );
    }

    #[test]
    fn qos_and_group_type_menus() {
        let q = QosSpecRow {
            id: "q1".into(),
            name: "gold".into(),
            consumer: "back-end".into(),
            specs: vec![],
        };
        let items = qos_spec_items(Lang::EnUs, &q);
        assert_eq!(
            items.iter().map(|i| i.label.clone()).collect::<Vec<_>>(),
            ["Manage Specs", "Edit Consumer", "Delete QoS Spec"]
        );
        let d = dialog_of(&items, "Edit Consumer");
        assert!(
            (d.build)(&vals(&[("consumer", "back-end")])).is_err(),
            "nothing changed"
        );
        assert_eq!(
            (d.build)(&vals(&[("consumer", "both")])).unwrap(),
            OsAction::QosSpecSetConsumer {
                id: "q1".into(),
                consumer: "both".into()
            }
        );
        let c = create_qos_spec_dialog(Lang::EnUs);
        assert_eq!(
            (c.build)(&vals(&[
                ("name", "gold"),
                ("consumer", "front-end"),
                ("specs", r#"[["read_iops_sec","500"]]"#)
            ]))
            .unwrap(),
            OsAction::QosSpecCreate {
                name: "gold".into(),
                consumer: "front-end".into(),
                specs: vec![("read_iops_sec".into(), "500".into())]
            }
        );
        let g = GroupTypeRow {
            id: "g1".into(),
            name: "cg".into(),
            description: None,
            public: true,
            specs: vec![],
        };
        assert_eq!(
            group_type_items(Lang::EnUs, &g)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>(),
            ["View Specs", "Edit Group Type", "Delete Group Type"]
        );
        let c = create_group_type_dialog(Lang::EnUs);
        assert_eq!(
            (c.build)(&vals(&[("name", "cg"), ("public", "false")])).unwrap(),
            OsAction::GroupTypeCreate {
                name: "cg".into(),
                description: None,
                public: false
            }
        );
    }

    #[test]
    fn metadef_menu_hides_delete_when_protected_and_sends_association_changes() {
        let n = MetadefNamespace {
            namespace: "OS::Acme::Gpu".into(),
            name: "GPU".into(),
            description: None,
            resource_types: vec!["OS::Glance::Image".into()],
            public: true,
            protected: false,
            prefixes: vec![],
        };
        let items = metadef_items(Lang::EnUs, &n);
        assert_eq!(
            items.iter().map(|i| i.label.clone()).collect::<Vec<_>>(),
            ["Edit Namespace", "Update Associations", "Delete Namespace"]
        );
        assert_eq!(
            metadef_items(
                Lang::EnUs,
                &MetadefNamespace {
                    protected: true,
                    ..n.clone()
                }
            )
            .len(),
            2,
            "Glance refuses deleting a protected namespace"
        );
        let d = dialog_of(&items, "Update Associations");
        assert_eq!(
            (d.build)(&vals(&[("types", "OS::Nova::Flavor")])).unwrap(),
            OsAction::MetadefNamespaceSetTypes {
                id: "OS::Acme::Gpu".into(),
                add_type_ids: vec!["OS::Nova::Flavor".into()],
                remove_type_ids: vec!["OS::Glance::Image".into()],
                prefixes: vec![]
            }
        );
        // Prefixes (roadmap 89): prefilled, a change re-adds the association, and one
        // for a type that isn't ticked is refused.
        let with = MetadefNamespace {
            prefixes: vec![("OS::Glance::Image".into(), "hw_".into())],
            ..n.clone()
        };
        let d = dialog_of(&metadef_items(Lang::EnUs, &with), "Update Associations");
        assert_eq!(d.fields[1].default, r#"[["OS::Glance::Image","hw_"]]"#);
        let img = || vec!["OS::Glance::Image".to_string()];
        assert!(
            (d.build)(&vals(&[
                ("types", "OS::Glance::Image"),
                ("prefixes", r#"[["OS::Glance::Image","hw_"]]"#)
            ]))
            .is_err(),
            "nothing changed"
        );
        assert_eq!(
            (d.build)(&vals(&[
                ("types", "OS::Glance::Image"),
                ("prefixes", r#"[["OS::Glance::Image","gpu_"]]"#)
            ]))
            .unwrap(),
            OsAction::MetadefNamespaceSetTypes {
                id: "OS::Acme::Gpu".into(),
                add_type_ids: img(),
                remove_type_ids: img(),
                prefixes: vec![("OS::Glance::Image".into(), "gpu_".into())]
            }
        );
        assert!(
            (d.build)(&vals(&[
                ("types", "OS::Glance::Image"),
                ("prefixes", r#"[["OS::Nova::Flavor","hw:"]]"#)
            ]))
            .is_err(),
            "not associated"
        );
        let d = dialog_of(&items, "Edit Namespace");
        assert_eq!(d.fields[0].default, "GPU", "the display name, not the namespace");
        assert!(
            (import_namespace_dialog(Lang::EnUs).build)(&vals(&[("definition", "{oops")])).is_err(),
            "JSON checked in the dialog"
        );
    }

    #[test]
    fn manage_volume_reads_horizons_form() {
        let d = manage_volume_dialog(Lang::EnUs);
        let a = (d.build)(&vals(&[
            ("identifier", "lun-7"),
            ("id_type", "source-id"),
            ("host", "c1@san#p"),
            ("name", ""),
            ("bootable", "true"),
            ("metadata", r#"[["k","v"]]"#),
        ]))
        .unwrap();
        let OsAction::VolumeManage(spec) = a else { panic!() };
        assert_eq!(
            (
                spec.identifier.as_str(),
                spec.by_id,
                spec.host.as_str(),
                spec.name,
                spec.bootable
            ),
            ("lun-7", true, "c1@san#p", None, true)
        );
        assert_eq!(spec.metadata, [("k".to_string(), "v".to_string())]);
        assert!(
            (d.build)(&vals(&[("identifier", "lun-7"), ("id_type", "source-name")])).is_err(),
            "the host is required"
        );
    }
}
