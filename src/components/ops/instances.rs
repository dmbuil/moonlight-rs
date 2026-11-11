//! Instances: the launch wizard, row actions, ports and interfaces.

use super::*;

// Launch wizard rules, following Horizon's *Launch Instance*.

fn image_like(v: &Vals) -> bool {
    matches!(v.get("source").map(String::as_str), Some("image" | "instance_snapshot"))
}

fn creates_volume(v: &Vals) -> bool {
    image_like(v) && v.get("create_volume").map(String::as_str) == Some("true")
}

/// The root disk is a volume (so the flavor's disk does not matter).
fn boots_from_volume(v: &Vals) -> bool {
    !image_like(v) || creates_volume(v)
}

fn source_set(v: &Vals) -> Option<ChoiceSet> {
    match v.get("source").map(String::as_str) {
        Some("image") => Some(ChoiceSet::LaunchImages),
        Some("instance_snapshot") => Some(ChoiceSet::InstanceSnapshots),
        Some("volume") => Some(ChoiceSet::AvailableVolumes),
        Some("volume_snapshot") => Some(ChoiceSet::VolumeSnapshots),
        _ => None,
    }
}

pub(super) fn chosen<'a>(sets: &'a Sets, set: Option<ChoiceSet>, id: Option<&String>) -> Option<&'a Choice> {
    sets.get(&set?)?.iter().find(|c| Some(&c.value) == id)
}

/// Sizing facts of the selected image (none for volume sources).
fn image_facts<'a>(v: &Vals, sets: &'a Sets) -> Option<&'a Facts> {
    image_like(v)
        .then(|| chosen(sets, source_set(v), v.get("source_id")).map(|c| &c.facts))
        .flatten()
}

fn flavor_message(lang: Lang, issue: FlavorIssue) -> String {
    match issue {
        FlavorIssue::Ram(mb) => crate::i18n::flavor_needs_ram(lang, mb),
        FlavorIssue::Disk(gb) => crate::i18n::flavor_needs_disk(lang, gb),
    }
}

/// Horizon's *Launch Instance* wizard (without Scheduler Hints and Metadata),
/// shaped by `[launch]` in `moonlight.conf`.
pub fn launch_dialog(lang: Lang) -> Dialog {
    launch_with(lang, &crate::components::dialog::prefs().launch, None)
}

const BOOT_SOURCES: [(&str, K); 4] = [
    ("image", K::BootImage),
    ("instance_snapshot", K::BootInstanceSnapshot),
    ("volume", K::BootVolume),
    ("volume_snapshot", K::BootVolumeSnapshot),
];

/// `preset`: the source of a *Launch as Instance* row, offered even when
/// `[launch]` disables it for the wizard.
fn launch_with(lang: Lang, p: &crate::prefs::LaunchPrefs, preset: Option<&str>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let offered = p.sources();
    let sources: Vec<(&str, K)> = BOOT_SOURCES
        .iter()
        .copied()
        .filter(|(s, _)| offered.contains(s) || preset == Some(*s))
        .collect();
    let first_source = sources.first().map(|(s, _)| *s).unwrap_or("image");
    let key_pair = Field::pick("key_name", tr(K::KeyPair), Source::Set(ChoiceSet::KeyPairs));
    let mut fields = vec![
        // Details
        Field::text("name", tr(K::InstanceName)).required(),
        Field::text("description", tr(K::Description)),
        Field::select("az", tr(K::AvailabilityZone), Source::Set(ChoiceSet::ComputeZones))
            .empty_label(tr(K::AnyAvailabilityZone)),
        Field::number("count", tr(K::Count), 1).value("1").required(),
        // Source
        Field::select("source", tr(K::BootSource), options(&sources, lang))
            .value(first_source)
            .required(),
        // `[launch] create_volume`: the production cloud boots from volume, so Yes by default.
        Field::yes_no("create_volume", tr(K::CreateNewVolume), p.create_volume).show_if(image_like),
        Field::number("volume_size", tr(K::VolumeSizeGb), 1)
            .value("1")
            .required()
            .show_if(creates_volume),
        Field::yes_no("delete_on_terminate", tr(K::DeleteOnTerminate), false).show_if(boots_from_volume),
        Field::pick(
            "source_id",
            tr(K::SourceItem),
            Source::Switch(
                "source",
                vec![
                    ("image", ChoiceSet::LaunchImages),
                    ("instance_snapshot", ChoiceSet::InstanceSnapshots),
                    ("volume", ChoiceSet::AvailableVolumes),
                    ("volume_snapshot", ChoiceSet::VolumeSnapshots),
                ],
            ),
        )
        .required(),
        Field::pick("flavor", tr(K::Flavor), Source::Set(ChoiceSet::Flavors)).required(),
        Field::pick_many("networks", tr(K::Networks), Source::Set(ChoiceSet::Networks)),
        Field::pick_many("ports", tr(K::Ports), Source::Set(ChoiceSet::FreePorts)),
        Field::pick_many(
            "security_groups",
            tr(K::SecurityGroups),
            Source::Set(ChoiceSet::SecurityGroups),
        )
        .value("default"),
        if p.requires_keypair {
            key_pair.required()
        } else {
            key_pair
        },
        // Configuration
        Field::area("user_data", tr(K::CustomizationScript)).from_file(MAX_USER_DATA_BYTES),
        Field::select(
            "disk_partition",
            tr(K::DiskPartition),
            options(&[("auto", K::Automatic), ("manual", K::Manual)], lang),
        )
        .value("auto")
        .required(),
        Field::check("config_drive", tr(K::ConfigDrive), p.config_drive),
        Field::pick("server_group", tr(K::ServerGroup), Source::Set(ChoiceSet::ServerGroups)),
    ];
    let mut config_step = vec!["user_data", "disk_partition", "config_drive"];
    if p.can_set_password {
        fields.push(Field::password("admin_pass", tr(K::AdminPassword)).help(tr(K::AdminPasswordHelp)));
        config_step.push("admin_pass");
    }
    let steps = vec![
        Step::new(tr(K::StepDetails), &["name", "description", "az", "count"])
            .help(tr(K::DetailsHelp))
            .required(),
        Step::new(
            tr(K::StepSource),
            &[
                "source",
                "create_volume",
                "volume_size",
                "delete_on_terminate",
                "source_id",
            ],
        )
        .help(tr(K::SourceHelp))
        .required(),
        Step::new(tr(K::Flavor), &["flavor"]).help(tr(K::FlavorHelp)).required(),
        Step::new(tr(K::Networks), &["networks"])
            .help(tr(K::NetworksHelp))
            .required(),
        Step::new(tr(K::StepNetworkPorts), &["ports"]).help(tr(K::PortsHelp)),
        Step::new(tr(K::SecurityGroups), &["security_groups"]).help(tr(K::SecurityGroupsHelp)),
        {
            let s = Step::new(tr(K::KeyPair), &["key_name"]).help(tr(K::KeyPairHelp));
            if p.requires_keypair { s.required() } else { s }
        },
        Step::new(tr(K::StepConfiguration), &config_step).help(tr(K::ConfigurationHelp)),
        Step::new(tr(K::StepServerGroups), &["server_group"]).help(tr(K::ServerGroupsHelp)),
    ];
    Dialog::form(tr(K::LaunchInstance), tr(K::LaunchInstance), fields, move |v| {
        let source = match v.get("source").as_str() {
            "volume" => BootSource::Volume,
            "volume_snapshot" => BootSource::VolumeSnapshot,
            // An instance snapshot is an image.
            _ => BootSource::Image,
        };
        let create_volume = source == BootSource::Image && v.flag("create_volume");
        Ok(OsAction::ServerLaunch(LaunchSpec {
            name: req(v, "name", K::InstanceName, lang)?,
            availability_zone: v.opt("az"),
            count: num_of::<u32>(v, "count", K::Count, lang)?.unwrap_or(1),
            source,
            source_id: req(v, "source_id", K::SourceItem, lang)?,
            create_volume,
            volume_size_gb: if create_volume {
                num_of::<u32>(v, "volume_size", K::VolumeSizeGb, lang)?
            } else {
                None
            },
            delete_volume_on_terminate: v.flag("delete_on_terminate"),
            flavor_id: req(v, "flavor", K::Flavor, lang)?,
            network_ids: v.list("networks"),
            port_ids: v.list("ports"),
            security_groups: v.list("security_groups"),
            key_name: v.opt("key_name"),
            server_group_id: v.opt("server_group"),
            description: v.opt("description"),
            user_data: Some(v.0.get("user_data").cloned().unwrap_or_default()).filter(|u| !u.trim().is_empty()),
            config_drive: v.flag("config_drive"),
            manual_disk_partition: v.get("disk_partition") == "manual",
            admin_pass: Some(v.0.get("admin_pass").cloned().unwrap_or_default()).filter(|p| !p.is_empty()),
        }))
    })
    .wizard(steps)
    .on_change(|name, v, sets| match name {
        // Like Horizon: a new source type clears the allocated item…
        "source" => {
            v.insert("source_id".into(), String::new());
        }
        // …and allocating an image raises the volume size to its minimum.
        "source_id" => {
            if let Some(min) = image_facts(v, sets).map(min_volume_gb) {
                let cur = v
                    .get("volume_size")
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .unwrap_or(0);
                v.insert("volume_size".into(), cur.max(min).to_string());
            }
        }
        _ => {}
    })
    .check(move |v, sets| {
        let mut out = Vec::new();
        if creates_volume(v) {
            let min = image_facts(v, sets).map(min_volume_gb).unwrap_or(1);
            let size = v
                .get("volume_size")
                .and_then(|s| s.trim().parse::<u32>().ok())
                .unwrap_or(0);
            if size < min {
                out.push(("volume_size", crate::i18n::volume_min(lang, min)));
            }
        }
        if let Some(f) = chosen(sets, Some(ChoiceSet::Flavors), v.get("flavor"))
            && let Some(issue) = flavor_issue(&f.facts, image_facts(v, sets), boots_from_volume(v))
        {
            out.push(("flavor", flavor_message(lang, issue)));
        }
        let none = |k: &str| v.get(k).is_none_or(|x| x.trim().is_empty());
        if none("networks") && none("ports") {
            out.push(("networks", t(lang, K::NetworkOrPort).to_string()));
        }
        if v.get("user_data").is_some_and(|u| u.len() > MAX_USER_DATA_BYTES) {
            out.push(("user_data", t(lang, K::UserDataTooBig).to_string()));
        }
        out
    })
    .block(move |name, c, v, sets| {
        (name == "flavor")
            .then(|| flavor_issue(&c.facts, image_facts(v, sets), boots_from_volume(v)))
            .flatten()
            .map(|i| flavor_message(lang, i))
    })
}

/// The instance's menu (row and detail header): Horizon's actions allowed in its
/// state, minus those the cloud is too old for or that can't apply (no ports).
pub fn instance_items(lang: Lang, inst: &Instance) -> Vec<MenuItem> {
    let busy = crate::components::widgets::is_busy(inst.task_state.as_deref(), &inst.status);
    let c = caps();
    allowed(&inst.status, Some(inst.locked), busy)
        .into_iter()
        // Version-dependent actions hide on clouds that are too old (roadmap 19).
        .filter(|op| *op != Op::Rebuild || needs::REBUILD.met(&c))
        // Without ports Nova has nowhere to put a group and answers 400 (roadmap 42).
        .filter(|op| *op != Op::EditSecurityGroups || !inst.addresses.is_empty())
        .map(|op| instance_item(lang, inst, op))
        .collect()
}

pub(super) fn instance_item(lang: Lang, inst: &Instance, op: Op) -> MenuItem {
    let tr = |k| t(lang, k).to_string();
    let (id, name) = (inst.id.clone(), inst.name.clone());
    // One-click actions still confirm, like Horizon.
    let simple = |label: K, a: OsAction| {
        let n = name.clone();
        MenuItem::dialog(tr(label), move || {
            Dialog::simple(lang, t(lang, label), n.clone(), a.clone())
        })
    };
    let i = id.clone();
    match op {
        Op::ConfirmResize => simple(K::ConfirmResize, OsAction::ServerConfirmResize { id }),
        Op::RevertResize => simple(K::RevertResize, OsAction::ServerRevertResize { id }),
        Op::Snapshot => MenuItem::dialog(tr(K::CreateSnapshot), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::CreateSnapshot),
                t(lang, K::CreateSnapshot),
                vec![
                    Field::text("name", t(lang, K::SnapshotName))
                        .value(format!("{name}-snapshot"))
                        .required(),
                ],
                move |v| {
                    Ok(OsAction::ServerSnapshot {
                        id: i.clone(),
                        name: req(v, "name", K::SnapshotName, lang)?,
                    })
                },
            )
        }),
        Op::AssociateIp => MenuItem::dialog(tr(K::AssociateFloatingIp), move || associate_ip_dialog(lang, &i)),
        Op::AttachInterface => MenuItem::dialog(tr(K::AttachInterface), move || attach_interface_dialog(lang, &i)),
        Op::DetachInterface => MenuItem::danger(tr(K::DetachInterface), move || {
            let i = i.clone();
            let mut d = Dialog::form(
                t(lang, K::DetachInterface),
                t(lang, K::DetachInterface),
                vec![
                    Field::select(
                        "port",
                        t(lang, K::Port),
                        Source::Of(ChoiceSet::InstancePorts, i.clone()),
                    )
                    .required(),
                ],
                move |v| {
                    Ok(OsAction::ServerDetachInterface {
                        id: i.clone(),
                        port_id: req(v, "port", K::Port, lang)?,
                    })
                },
            );
            d.danger = true;
            d
        }),
        Op::UpdateMetadata => {
            let i = i.clone();
            metadata_item(lang, "OS::Nova::Server", &inst.metadata, move |metadata, _| {
                OsAction::ServerSetMetadata {
                    id: i.clone(),
                    metadata,
                }
            })
        }
        Op::Edit => MenuItem::dialog(tr(K::EditInstance), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::EditInstance),
                t(lang, K::Save),
                vec![
                    Field::text("name", t(lang, K::InstanceName))
                        .value(name.clone())
                        .required(),
                ],
                move |v| {
                    Ok(OsAction::ServerUpdate {
                        id: i.clone(),
                        name: req(v, "name", K::InstanceName, lang)?,
                    })
                },
            )
        }),
        Op::AttachVolume => MenuItem::dialog(tr(K::AttachVolume), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::AttachVolume),
                t(lang, K::AttachVolume),
                vec![Field::select("volume", t(lang, K::Volume), Source::Set(ChoiceSet::AvailableVolumes)).required()],
                move |v| {
                    Ok(OsAction::ServerAttachVolume {
                        server_id: i.clone(),
                        volume_id: req(v, "volume", K::Volume, lang)?,
                    })
                },
            )
        }),
        Op::EditSecurityGroups => {
            let current = inst.security_groups.clone();
            MenuItem::dialog(tr(K::EditSecurityGroups), move || {
                security_groups_dialog(lang, &i, &current)
            })
        }
        Op::EditPortSecurityGroups => MenuItem::dialog(tr(K::EditPortSecurityGroups), move || {
            port_security_groups_dialog(lang, &i)
        }),
        Op::Rescue => MenuItem::dialog(tr(K::RescueInstance), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::RescueInstance),
                t(lang, K::RescueInstance),
                vec![
                    Field::select("image", t(lang, K::RescueImage), Source::Set(ChoiceSet::Images))
                        .help(t(lang, K::RescueImageHelp)),
                ],
                move |v| {
                    Ok(OsAction::ServerRescue {
                        id: i.clone(),
                        image_id: v.opt("image"),
                    })
                },
            )
        }),
        Op::Unrescue => simple(K::UnrescueInstance, OsAction::ServerUnrescue { id }),
        Op::Pause => simple(K::PauseInstance, OsAction::ServerPause { id }),
        Op::Unpause => simple(K::UnpauseInstance, OsAction::ServerUnpause { id }),
        Op::Suspend => simple(K::SuspendInstance, OsAction::ServerSuspend { id }),
        Op::Resume => simple(K::ResumeInstance, OsAction::ServerResume { id }),
        Op::Shelve => simple(K::ShelveInstance, OsAction::ServerShelve { id }),
        Op::Unshelve => {
            let to_zone = needs::UNSHELVE_TO_ZONE.met(&caps());
            MenuItem::dialog(tr(K::UnshelveInstance), move || {
                let i = i.clone();
                let fields = if to_zone {
                    vec![
                        Field::select("az", t(lang, K::AvailabilityZone), Source::Set(ChoiceSet::ComputeZones))
                            .empty_label(t(lang, K::AnyAvailabilityZone))
                            .help(t(lang, K::AnyZone)),
                    ]
                } else {
                    vec![]
                };
                let mut d = Dialog::form(
                    t(lang, K::UnshelveInstance),
                    t(lang, K::UnshelveInstance),
                    fields,
                    move |v| {
                        Ok(OsAction::ServerUnshelve {
                            id: i.clone(),
                            availability_zone: v.opt("az"),
                        })
                    },
                );
                if !to_zone {
                    d.intro = vec![t(lang, K::YouSelected).into()];
                    d.selection = vec![name.clone()];
                }
                d
            })
        }
        Op::Resize => {
            let current = inst.flavor.id.clone();
            MenuItem::dialog(tr(K::ResizeInstance), move || {
                let (i, current) = (i.clone(), current.clone());
                Dialog::form(
                    t(lang, K::ResizeInstance),
                    t(lang, K::ResizeInstance),
                    vec![Field::select("flavor", t(lang, K::NewFlavor), Source::Set(ChoiceSet::Flavors)).required()],
                    move |v| {
                        let flavor_id = req(v, "flavor", K::NewFlavor, lang)?;
                        if flavor_id == current {
                            return Err(t(lang, K::SameFlavor).to_string());
                        }
                        Ok(OsAction::ServerResize {
                            id: i.clone(),
                            flavor_id,
                        })
                    },
                )
            })
        }
        Op::Lock => simple(K::LockInstance, OsAction::ServerLock { id }),
        Op::Unlock => simple(K::UnlockInstance, OsAction::ServerUnlock { id }),
        Op::SoftReboot => simple(
            K::SoftReboot,
            OsAction::ServerReboot {
                id,
                kind: RebootKind::Soft,
            },
        ),
        Op::HardReboot => simple(
            K::HardReboot,
            OsAction::ServerReboot {
                id,
                kind: RebootKind::Hard,
            },
        ),
        Op::ShutOff => simple(K::ShutOff, OsAction::ServerStop { id }),
        Op::Start => simple(K::StartInstance, OsAction::ServerStart { id }),
        Op::Rebuild => MenuItem::danger(tr(K::RebuildInstance), move || {
            let i = i.clone();
            let mut d = Dialog::form(
                t(lang, K::RebuildInstance),
                t(lang, K::RebuildInstance),
                vec![
                    Field::select("image", t(lang, K::BootImage), Source::Set(ChoiceSet::Images))
                        .required()
                        .help(t(lang, K::RebuildHelp)),
                ],
                move |v| {
                    Ok(OsAction::ServerRebuild {
                        id: i.clone(),
                        image_id: req(v, "image", K::BootImage, lang)?,
                    })
                },
            );
            d.danger = true;
            d
        }),
        Op::Delete => MenuItem::danger(tr(K::DeleteInstance), move || {
            Dialog::confirm(
                lang,
                t(lang, K::DeleteInstance),
                name.clone(),
                OsAction::ServerDelete { id: i.clone() },
            )
        }),
    }
}

/// Horizon's *Associate Floating IP* from an instance: an unassociated IP, or
/// a new one from a pool (allocated and associated in one request), to one of
/// the instance's ports.
pub fn associate_ip_dialog(lang: Lang, server_id: &str) -> Dialog {
    Dialog::form(
        t(lang, K::AssociateFloatingIp),
        t(lang, K::Associate),
        vec![
            Field::check("allocate", t(lang, K::AllocateNewIp), false),
            Field::select("ip", t(lang, K::FloatingIp), Source::Set(ChoiceSet::FreeFloatingIps))
                .required()
                .when("allocate", "false"),
            Field::select("pool", t(lang, K::Pool), Source::Set(ChoiceSet::ExternalNetworks))
                .required()
                .when("allocate", "true"),
            Field::select(
                "port",
                t(lang, K::PortToAssociate),
                Source::Of(ChoiceSet::AssociablePorts, server_id.to_string()),
            )
            .required(),
        ],
        move |v| {
            let port_id = req(v, "port", K::PortToAssociate, lang)?;
            Ok(if v.flag("allocate") {
                OsAction::FloatingIpAllocate {
                    network_id: req(v, "pool", K::Pool, lang)?,
                    description: None,
                    port_id: Some(port_id),
                    qos_policy_id: None,
                }
            } else {
                OsAction::FloatingIpAssociate {
                    id: req(v, "ip", K::FloatingIp, lang)?,
                    port_id,
                }
            })
        },
    )
}

/// *Attach Interface*: a network (with an optional fixed IP) or an existing free port.
pub fn attach_interface_dialog(lang: Lang, server_id: &str) -> Dialog {
    let id = server_id.to_string();
    Dialog::form(
        t(lang, K::AttachInterface),
        t(lang, K::AttachInterface),
        vec![
            Field::select(
                "by",
                t(lang, K::InterfaceSource),
                options(&[("network", K::ByNetwork), ("port", K::ByPort)], lang),
            )
            .value("network")
            .required(),
            Field::select("network", t(lang, K::Network), Source::Set(ChoiceSet::Networks))
                .required()
                .when("by", "network"),
            Field::text("fixed_ip", t(lang, K::FixedIpAddress))
                .help(t(lang, K::FixedIpHelp))
                .when("by", "network"),
            Field::select("port", t(lang, K::Port), Source::Set(ChoiceSet::FreePorts))
                .required()
                .when("by", "port"),
        ],
        move |v| {
            Ok(if v.get("by") == "port" {
                OsAction::ServerAttachInterface {
                    id: id.clone(),
                    network_id: None,
                    port_id: Some(req(v, "port", K::Port, lang)?),
                    fixed_ip: None,
                }
            } else {
                OsAction::ServerAttachInterface {
                    id: id.clone(),
                    network_id: Some(req(v, "network", K::Network, lang)?),
                    port_id: None,
                    fixed_ip: v.opt("fixed_ip"),
                }
            })
        },
    )
}

/// *Edit Security Groups* (Horizon: *Edit Instance* → *Security Groups*): the
/// server's current groups preselected; the change sent is the difference,
/// applied by Nova on every port of the server. Groups go by name, as Nova wants.
pub fn security_groups_dialog(lang: Lang, server_id: &str, current: &[String]) -> Dialog {
    let (id, current) = (server_id.to_string(), current.to_vec());
    Dialog::form(
        t(lang, K::EditSecurityGroups),
        t(lang, K::Save),
        // *Allocated / Available* tables with a filter (name and description), for
        // projects with many groups (roadmap 43).
        vec![
            Field::pick_many(
                "groups",
                t(lang, K::SecurityGroups),
                Source::Set(ChoiceSet::SecurityGroups),
            )
            .value(current.join(","))
            .help(t(lang, K::EditSecurityGroupsHelp)),
        ],
        move |v| {
            let (add, remove) = list_diff(&current, &v.list("groups"));
            if add.is_empty() && remove.is_empty() {
                return Err(t(lang, K::NothingChanged).to_string());
            }
            Ok(OsAction::ServerSetSecurityGroups {
                id: id.clone(),
                add,
                remove,
            })
        },
    )
}

/// `(added, removed)` going from `before` to `after`, each in its list's order.
pub(super) fn list_diff(before: &[String], after: &[String]) -> (Vec<String>, Vec<String>) {
    let add = after.iter().filter(|x| !before.contains(x)).cloned().collect();
    let remove = before.iter().filter(|x| !after.contains(x)).cloned().collect();
    (add, remove)
}

/// Horizon's newer *Edit Port Security Groups*: pick a port of the instance,
/// its current groups are preselected, and the port gets exactly the chosen set.
pub fn port_security_groups_dialog(lang: Lang, server_id: &str) -> Dialog {
    Dialog::form(
        t(lang, K::EditPortSecurityGroups),
        t(lang, K::Save),
        vec![
            Field::select(
                "port",
                t(lang, K::Port),
                Source::Of(ChoiceSet::InstancePorts, server_id.to_string()),
            )
            .required(),
            // Same filtered tables as *Edit Security Groups*; the port's groups are
            // preset by `on_change` as the same comma list (roadmap 43).
            Field::pick_many(
                "groups",
                t(lang, K::SecurityGroups),
                Source::Set(ChoiceSet::SecurityGroupIds),
            ),
        ],
        move |v| {
            Ok(OsAction::PortSetSecurityGroups {
                id: req(v, "port", K::Port, lang)?,
                security_group_ids: v.list("groups"),
            })
        },
    )
    .on_change(|field, vals, sets| {
        if field == "port" {
            let port = vals.get("port").cloned().unwrap_or_default();
            let groups = sets
                .get(&ChoiceSet::InstancePorts)
                .and_then(|l| l.iter().find(|c| c.value == port))
                .and_then(|c| c.cells.get(1).cloned())
                .unwrap_or_default();
            vals.insert("groups".into(), groups);
        }
    })
}

/// *Launch* / *Launch as Instance* on image, volume and snapshot rows: the
/// launch wizard with the source preselected.
pub fn launch_from(lang: Lang, source: &'static str, source_id: &str) -> Dialog {
    let mut d = launch_with(lang, &crate::components::dialog::prefs().launch, Some(source));
    for f in &mut d.fields {
        match f.name {
            "source" => f.default = source.to_string(),
            "source_id" => f.default = source_id.to_string(),
            _ => {}
        }
    }
    d
}

/// Formats Cinder can upload a volume as, among `[images] disk_formats`
/// (raw when the list leaves none).
pub fn upload_formats(configured: &[String]) -> Vec<&'static str> {
    const CINDER: [&str; 7] = ["raw", "qcow2", "vmdk", "vdi", "vhd", "vhdx", "ploop"];
    let out: Vec<_> = CINDER
        .into_iter()
        .filter(|f| configured.iter().any(|c| c == f))
        .collect();
    if out.is_empty() { vec!["raw"] } else { out }
}

/// Header split button for the selected instances (Horizon's *More Actions*).
pub fn bulk_instance_items(lang: Lang, picked: Vec<(String, String)>) -> Vec<MenuItem> {
    let item = |label: K, make: fn(String) -> OsAction| {
        let p = picked.clone();
        MenuItem::dialog(t(lang, label), move || bulk(lang, label, p.clone(), make, false))
    };
    vec![
        item(K::StartInstances, |id| OsAction::ServerStart { id }),
        item(K::ShutOffInstances, |id| OsAction::ServerStop { id }),
        item(K::SoftRebootInstances, |id| OsAction::ServerReboot {
            id,
            kind: RebootKind::Soft,
        }),
    ]
}

/// `binding:vnic_type` values Horizon offers in *Edit Port*.
const VNIC_TYPES: &[(&str, K)] = &[
    ("normal", K::VnicNormal),
    ("direct", K::VnicDirect),
    ("direct-physical", K::VnicDirectPhysical),
    ("macvtap", K::VnicMacvtap),
    ("baremetal", K::VnicBaremetal),
    ("virtio-forwarder", K::VnicVirtioForwarder),
    ("vdpa", K::VnicVdpa),
    ("smart-nic", K::VnicSmartNic),
    ("remote-managed", K::VnicRemoteManaged),
];

/// Horizon's *Edit Port*, two tabs: *Info* (name, admin state, VNIC type, port
/// security) and *Security Groups* (an *Allocated / Available* picker, switched
/// off while port security is off). `groups_first` opens it on the second tab,
/// as Horizon's *Edit Security Groups* does. Sends only what changed
/// ([`PortSettings::changes`]). VNIC type needs `binding`, port security
/// `port-security` (roadmap 44).
pub fn edit_port_dialog(lang: Lang, port_id: &str, current: &PortSettings, groups_first: bool) -> Dialog {
    let (id, before) = (port_id.to_string(), current.clone());
    let caps = caps();
    let mut fields = vec![
        Field::text("name", t(lang, K::Name)).value(current.name.clone()),
        Field::check("admin_up", t(lang, K::EnableAdminState), current.admin_up),
    ];
    let mut info = vec!["name", "admin_up"];
    if needs::PORT_BINDING.met(&caps) {
        let mut types: Vec<(String, String)> = VNIC_TYPES
            .iter()
            .map(|(v, k)| (v.to_string(), t(lang, *k).to_string()))
            .collect();
        // Keep a type Horizon doesn't list (e.g. `accelerator-direct`) selectable.
        if let Some(v) = current
            .vnic_type
            .as_ref()
            .filter(|v| !VNIC_TYPES.iter().any(|(x, _)| x == v))
        {
            types.push((v.clone(), v.clone()));
        }
        let value = current.vnic_type.clone().unwrap_or_else(|| "normal".into());
        fields.push(
            Field::select("vnic_type", t(lang, K::VnicType), Source::Static(types))
                .value(value)
                .required(),
        );
        info.push("vnic_type");
    }
    if needs::PORT_SECURITY.met(&caps) {
        fields.push(Field::check(
            "port_security",
            t(lang, K::PortSecurity),
            current.port_security,
        ));
        info.push("port_security");
    }
    fields.push(
        Field::pick_many(
            "groups",
            t(lang, K::SecurityGroups),
            Source::Set(ChoiceSet::SecurityGroupIds),
        )
        .value(current.security_group_ids.join(",")),
    );
    let groups_on: fn(&Vals) -> bool = |v| v.get("port_security").is_none_or(|p| p == "true");
    let steps = vec![
        Step::new(t(lang, K::StepInfo), &info).help(t(lang, K::EditPortInfoHelp)),
        Step::new(t(lang, K::SecurityGroups), &["groups"])
            .help(t(lang, K::EditPortGroupsHelp))
            .enabled_if(groups_on),
    ];
    // A disabled step can't be shown: with port security off, start on *Info*.
    let first = usize::from(groups_first && current.port_security);
    Dialog::form(t(lang, K::EditPort), t(lang, K::Save), fields, move |v| {
        // Fields the cloud doesn't support (or a switched-off step) aren't submitted:
        // keep their current value.
        let after = PortSettings {
            name: v.get("name"),
            admin_up: v.flag("admin_up"),
            vnic_type: v.opt("vnic_type").or_else(|| before.vnic_type.clone()),
            port_security: if v.0.contains_key("port_security") {
                v.flag("port_security")
            } else {
                before.port_security
            },
            security_group_ids: if v.0.contains_key("groups") {
                v.list("groups")
            } else {
                before.security_group_ids.clone()
            },
        };
        before
            .changes(&id, &after)
            .ok_or_else(|| t(lang, K::NothingChanged).to_string())
    })
    .wizard(steps)
    .starting_at(first)
    .sized(DialogSize::Medium)
}

/// The row actions of the instance's *Interfaces* tab, as Horizon: *Edit
/// Security Groups* (the button), *Edit Port*, then *Detach Interface*. The edits
/// need the Neutron port (`Interface::port`); without it only *Detach* remains.
pub fn instance_interface_items(lang: Lang, server_id: &str, it: &Interface) -> Vec<MenuItem> {
    let mut items = vec![];
    if let Some(port) = &it.port {
        let (i, p) = (it.port_id.clone(), port.clone());
        items.push(MenuItem::dialog(t(lang, K::EditSecurityGroups), move || {
            edit_port_dialog(lang, &i, &p, true)
        }));
        let (i, p) = (it.port_id.clone(), port.clone());
        items.push(MenuItem::dialog(t(lang, K::EditPort), move || {
            edit_port_dialog(lang, &i, &p, false)
        }));
    }
    items.push(detach_interface_item(
        lang,
        server_id,
        &it.port_id,
        &it.fixed_ips.join(", "),
    ));
    items
}

/// *Detach Interface* on a row of the instance's Interfaces tab.
pub fn detach_interface_item(lang: Lang, server_id: &str, port_id: &str, label: &str) -> MenuItem {
    let (s, p, l) = (
        server_id.to_string(),
        port_id.to_string(),
        if label.is_empty() { port_id } else { label }.to_string(),
    );
    MenuItem::danger(t(lang, K::DetachInterface), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DetachInterface),
            l.clone(),
            OsAction::ServerDetachInterface {
                id: s.clone(),
                port_id: p.clone(),
            },
        )
    })
}

/// *Detach Volume* for one attachment (instance detail, *Volumes* card).
pub fn detach_volume_item(lang: Lang, server_id: &str, volume_id: &str, volume_name: &str) -> MenuItem {
    let (s, v, n) = (server_id.to_string(), volume_id.to_string(), volume_name.to_string());
    MenuItem::danger(t(lang, K::DetachVolume), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DetachVolume),
            n.clone(),
            OsAction::ServerDetachVolume {
                server_id: s.clone(),
                volume_id: v.clone(),
            },
        )
    })
}

#[cfg(test)]
mod launch_tests {
    use super::*;

    fn sets() -> Sets {
        let image = |id: &str, bytes: u64, min_disk: i64, min_ram: i64| Choice {
            value: id.into(),
            label: id.into(),
            facts: Facts {
                size_bytes: Some(bytes),
                min_disk_gb: Some(min_disk),
                min_ram_mb: Some(min_ram),
                ..Default::default()
            },
            ..Default::default()
        };
        let flavor = |id: &str, ram: i64, disk: i64| Choice {
            value: id.into(),
            label: id.into(),
            facts: Facts {
                ram_mb: Some(ram),
                disk_gb: Some(disk),
                vcpus: Some(1),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut s = Sets::new();
        s.insert(
            ChoiceSet::LaunchImages,
            vec![
                image("rhel", 7_800_000_000, 20, 2048),
                image("debian", 428_690_000, 0, 0),
            ],
        );
        s.insert(
            ChoiceSet::Flavors,
            vec![flavor("small", 1024, 10), flavor("big", 4096, 50)],
        );
        s
    }

    fn launch_values() -> Vals {
        [
            ("source", "image"),
            ("create_volume", "true"),
            ("volume_size", "1"),
            ("source_id", ""),
            ("networks", "n1"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    }

    #[test]
    fn allocating_an_image_raises_the_volume_size_like_horizon() {
        let d = launch_dialog(Lang::EnUs);
        let (hook, s) = (d.on_change.clone().unwrap(), sets());
        let mut v = launch_values();
        v.insert("source_id".into(), "rhel".into());
        hook("source_id", &mut v, &s);
        assert_eq!(v["volume_size"], "20");
        // Never lowered: a smaller image keeps the 20 GB.
        v.insert("source_id".into(), "debian".into());
        hook("source_id", &mut v, &s);
        assert_eq!(v["volume_size"], "20");
        // A new source type clears the allocation.
        hook("source", &mut v, &s);
        assert_eq!(v["source_id"], "");
    }

    #[test]
    fn launch_checks_volume_size_flavor_and_networks() {
        let d = launch_dialog(Lang::EnUs);
        let (check, block, s) = (d.check.clone().unwrap(), d.block.clone().unwrap(), sets());
        let mut v = launch_values();
        v.insert("source_id".into(), "rhel".into());
        v.insert("volume_size".into(), "10".into());
        v.insert("networks".into(), String::new());
        let p = check(&v, &s);
        assert!(
            p.iter()
                .any(|(f, m)| *f == "volume_size" && m.contains("at least 20 GB"))
        );
        assert!(p.iter().any(|(f, _)| *f == "networks"));

        let small = &s[&ChoiceSet::Flavors][0];
        let reason = block("flavor", small, &v, &s).unwrap();
        assert!(reason.contains("2048 MB of RAM"), "{reason}");
        assert_eq!(block("flavor", &s[&ChoiceSet::Flavors][1], &v, &s), None);
        // Booting straight from the image also needs the root disk.
        v.insert("create_volume".into(), "false".into());
        v.insert("source_id".into(), "debian".into());
        assert_eq!(
            block("flavor", small, &v, &s),
            None,
            "debian needs 1 GB and no RAM minimum"
        );
        v.insert("ports".into(), "p1".into());
        assert!(check(&v, &s).iter().all(|(f, _)| *f != "networks"), "a port is enough");
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::components::dialog::Kind;

    #[test]
    fn upload_formats_are_the_configured_ones_cinder_supports() {
        let def = crate::prefs::ImagePrefs::default().disk_formats;
        assert_eq!(upload_formats(&def), ["raw", "qcow2", "vmdk", "vdi", "vhd", "ploop"]);
        assert_eq!(upload_formats(&["qcow2".into(), "iso".into()]), ["qcow2"]);
        assert_eq!(upload_formats(&["iso".into()]), ["raw"], "nothing usable: raw");
    }

    #[test]
    fn launch_wizard_follows_the_launch_settings() {
        let field = |d: &Dialog, n: &str| d.fields.iter().find(|f| f.name == n).cloned();
        let sources = |d: &Dialog| match field(d, "source").map(|f| f.kind) {
            Some(Kind::Select(Source::Static(o))) => o.into_iter().map(|(v, _)| v).collect::<Vec<_>>(),
            _ => vec![],
        };
        let def = launch_with(Lang::EnUs, &crate::prefs::LaunchPrefs::default(), None);
        assert_eq!(sources(&def).len(), 4);
        assert_eq!(
            field(&def, "create_volume").unwrap().default,
            "true",
            "default: boot from a new volume"
        );
        assert!(!field(&def, "key_name").unwrap().required && field(&def, "admin_pass").is_none());

        let p = crate::prefs::LaunchPrefs {
            disable_image: true,
            disable_instance_snapshot: true,
            create_volume: false,
            config_drive: true,
            requires_keypair: true,
            can_set_password: true,
            ..Default::default()
        };
        let d = launch_with(Lang::EnUs, &p, None);
        assert_eq!(sources(&d), ["volume", "volume_snapshot"]);
        assert_eq!(field(&d, "source").unwrap().default, "volume", "first offered source");
        assert_eq!(
            (
                field(&d, "create_volume").unwrap().default.as_str(),
                field(&d, "config_drive").unwrap().default.as_str()
            ),
            ("false", "true")
        );
        assert!(field(&d, "key_name").unwrap().required);
        assert!(matches!(field(&d, "admin_pass").unwrap().kind, Kind::Password));
        let from_image = launch_with(Lang::EnUs, &p, Some("image"));
        assert_eq!(
            sources(&from_image),
            ["image", "volume", "volume_snapshot"],
            "a row's own source stays"
        );
    }

    #[test]
    fn launch_form_builds_a_launch_spec() {
        let d = launch_dialog(Lang::EnUs);
        let a = (d.build)(&vals(&[
            ("name", "web"),
            ("count", "2"),
            ("source", "image"),
            ("source_id", "img"),
            ("create_volume", "true"),
            ("volume_size", "30"),
            ("flavor", "f"),
            ("networks", "n1,n2"),
            ("security_groups", "default"),
        ]))
        .unwrap();
        let OsAction::ServerLaunch(s) = a else { panic!() };
        assert_eq!((s.count, s.volume_size_gb, s.network_ids.len()), (2, Some(30), 2));
        assert!(s.create_volume);
        // missing flavor is reported
        assert!((d.build)(&vals(&[("name", "x"), ("source_id", "i")])).is_err());
    }

    #[test]
    fn edit_security_groups_sends_the_difference_and_needs_ports() {
        let mut inst = instance("ACTIVE", false);
        let has = |inst: &Instance| {
            instance_items(Lang::EnUs, inst)
                .iter()
                .any(|m| m.label == "Edit Security Groups")
        };
        assert!(!has(&inst), "no ports: Nova would answer 400");
        inst.addresses = vec![Address {
            network: "net".into(),
            addr: "10.0.0.5".into(),
            floating: false,
            mac: None,
        }];
        assert!(has(&inst));
        assert!(!allowed("SHELVED_OFFLOADED", Some(false), false).contains(&Op::EditSecurityGroups));
        let d = security_groups_dialog(Lang::EnUs, "i", &inst.security_groups);
        assert_eq!(
            (d.build)(&vals(&[("groups", "web,ssh")])).unwrap(),
            OsAction::ServerSetSecurityGroups {
                id: "i".into(),
                add: vec!["web".into(), "ssh".into()],
                remove: vec!["default".into()]
            }
        );
        assert!((d.build)(&vals(&[("groups", "default")])).is_err(), "nothing changed");
    }

    #[test]
    fn security_group_editors_are_filtered_tables_next_to_each_other() {
        use crate::components::dialog::Kind;
        let ops = allowed("ACTIVE", Some(false), false);
        let at = |op| ops.iter().position(|o| *o == op).unwrap();
        assert_eq!(
            at(Op::EditPortSecurityGroups),
            at(Op::EditSecurityGroups) + 1,
            "the two items sit together"
        );
        assert!(
            allowed("SHELVED_OFFLOADED", Some(false), false).contains(&Op::EditPortSecurityGroups),
            "per port stays available"
        );
        for d in [
            security_groups_dialog(Lang::EnUs, "i", &[]),
            port_security_groups_dialog(Lang::EnUs, "i"),
        ] {
            let f = d.fields.iter().find(|f| f.name == "groups").unwrap();
            assert!(
                matches!(f.kind, Kind::Pick { multi: true, .. }),
                "{}: Allocated / Available with a filter",
                d.title
            );
        }
    }

    #[test]
    fn resize_rejects_the_current_flavor_and_launch_presets_the_source() {
        let items = instance_items(Lang::EnUs, &instance("ACTIVE", false));
        let resize = items.iter().find(|m| m.label == "Resize Instance").unwrap();
        let crate::components::dialog::ItemKind::Dialog(open) = &resize.kind else {
            panic!()
        };
        let d = open();
        assert!((d.build)(&vals(&[("flavor", "f1")])).is_err());
        assert!(matches!(
            (d.build)(&vals(&[("flavor", "f2")])),
            Ok(OsAction::ServerResize { .. })
        ));
        let d = launch_from(Lang::EnUs, "volume", "v1");
        let get = |n: &str| d.fields.iter().find(|f| f.name == n).unwrap().default.clone();
        assert_eq!(
            (get("source"), get("source_id")),
            ("volume".to_string(), "v1".to_string())
        );
    }

    #[test]
    fn associate_ip_picks_an_existing_ip_or_allocates_one() {
        let d = associate_ip_dialog(Lang::EnUs, "s1");
        let a = (d.build)(&vals(&[("allocate", "false"), ("ip", "f1"), ("port", "p1")])).unwrap();
        assert_eq!(
            a,
            OsAction::FloatingIpAssociate {
                id: "f1".into(),
                port_id: "p1".into()
            }
        );
        let a = (d.build)(&vals(&[("allocate", "true"), ("pool", "ext"), ("port", "p1")])).unwrap();
        assert_eq!(
            a,
            OsAction::FloatingIpAllocate {
                network_id: "ext".into(),
                description: None,
                port_id: Some("p1".into()),
                qos_policy_id: None
            }
        );
    }

    #[test]
    fn instance_menu_depends_on_state() {
        let labels = |status, locked| -> Vec<String> {
            instance_items(Lang::EnUs, &instance(status, locked))
                .into_iter()
                .map(|m| m.label)
                .collect()
        };
        let active = labels("ACTIVE", false);
        assert!(active.contains(&"Shut Off Instance".into()));
        assert!(active.contains(&"Lock Instance".into()));
        assert!(!active.contains(&"Start Instance".into()));
        let off = labels("SHUTOFF", true);
        assert!(off.contains(&"Start Instance".into()));
        assert!(off.contains(&"Unlock Instance".into()));
        assert!(!off.contains(&"Soft Reboot Instance".into()));
        assert_eq!(active.last().unwrap(), "Delete Instance", "delete is always last");
    }

    #[test]
    fn zone_selectors_offer_any_zone() {
        let d = launch_from(Lang::EnUs, "image", "img");
        let az = d.fields.iter().find(|f| f.name == "az").unwrap();
        assert_eq!(az.empty_label.as_deref(), Some("Any Availability Zone"));
    }
}
