//! Admin › Compute (roadmap 85): flavors, aggregates, services, migration.

use super::*;

/// A required whole number, named in the error.
fn num_req(v: &Values, k: &str, label: K, lang: Lang) -> Result<i64, String> {
    num_of(v, k, label, lang)?.ok_or_else(|| format!("{}: {}", t(lang, label), t(lang, K::RequiredField)))
}

/// Admin › Flavors' *Create Flavor*, Horizon's two steps: *Flavor Information*
/// and *Flavor Access* (projects picked there make the flavor private).
pub fn create_flavor_dialog(lang: Lang) -> Dialog {
    flavor_form(lang, None)
}

/// *Edit Flavor* (`[admin] flavor_edit`, roadmap 89), Horizon's `UpdateFlavor`:
/// the same steps, prefilled, without the ID; saving deletes the flavor and
/// creates it again with the same ID, extra specs and access.
fn edit_flavor_dialog(lang: Lang, f: &FlavorRow) -> Dialog {
    flavor_form(lang, Some(f))
}

/// *Create Flavor*, or with `old` its *Edit Flavor*.
fn flavor_form(lang: Lang, old: Option<&FlavorRow>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let num = |n: i64| n.to_string();
    let mut fields = vec![
        Field::text("name", tr(K::Name))
            .value(old.map(|f| f.name.clone()).unwrap_or_default())
            .required(),
        Field::text("flavor_id", tr(K::FlavorId))
            .value("auto")
            .help(tr(K::FlavorIdHelp)),
        // Sizes are a few digits each: three a line, two lines (the user found
        // eight full-width one-liners too long a step).
        Field::number("vcpus", tr(K::Vcpus), 1)
            .value(old.map(|f| num(f.vcpus)).unwrap_or_default())
            .required()
            .short(),
        Field::number("ram", tr(K::RamMb), 1)
            .value(old.map(|f| num(f.ram_mb)).unwrap_or_default())
            .required()
            .short(),
        Field::number("disk", tr(K::RootDiskGb), 0)
            .value(old.map_or("0".into(), |f| num(f.disk_gb)))
            .required()
            .short(),
        Field::number("ephemeral", tr(K::EphemeralDiskGb), 0)
            .value(old.map_or("0".into(), |f| num(f.ephemeral_gb)))
            .short(),
        Field::number("swap", tr(K::SwapDiskMb), 0)
            .value(old.map_or("0".into(), |f| num(f.swap_mb)))
            .short(),
        Field::text("rxtx", tr(K::RxtxFactor))
            .value(old.and_then(|f| f.rxtx_factor).map_or("1.0".into(), |r| r.to_string()))
            .short(),
        Field::multi("projects", tr(K::Projects), Source::Set(ChoiceSet::AllProjects)).value(
            old.map(|f| f.access.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>().join(","))
                .unwrap_or_default(),
        ),
    ];
    // The ID stays: Horizon's edit has no ID field.
    let kept = old.map(|f| (f.id.clone(), f.extra_specs.clone()));
    if kept.is_some() {
        fields.retain(|f| f.name != "flavor_id");
    }
    let info: &[&'static str] = if kept.is_some() {
        &["name", "vcpus", "ram", "disk", "ephemeral", "swap", "rxtx"]
    } else {
        &["name", "flavor_id", "vcpus", "ram", "disk", "ephemeral", "swap", "rxtx"]
    };
    let (title, submit) = if kept.is_some() {
        (tr(K::EditFlavor), tr(K::Save))
    } else {
        (tr(K::CreateFlavor), tr(K::CreateFlavor))
    };
    let mut d = Dialog::form(title, submit, fields, move |v| {
        let rxtx_factor = match v.opt("rxtx") {
            None => None,
            Some(x) => Some(
                x.parse::<f64>()
                    .ok()
                    .filter(|f| *f > 0.0)
                    .ok_or_else(|| format!("{}: ?", t(lang, K::RxtxFactor)))?,
            ),
        };
        let spec = FlavorSpec {
            name: req(v, "name", K::Name, lang)?,
            flavor_id: kept.as_ref().map(|(id, _)| id.clone()).or_else(|| v.opt("flavor_id")),
            vcpus: num_req(v, "vcpus", K::Vcpus, lang)?,
            ram_mb: num_req(v, "ram", K::RamMb, lang)?,
            disk_gb: num_req(v, "disk", K::RootDiskGb, lang)?,
            ephemeral_gb: v
                .num("ephemeral")
                .map_err(|_| format!("{}: ?", t(lang, K::EphemeralDiskGb)))?
                .unwrap_or(0),
            swap_mb: v
                .num("swap")
                .map_err(|_| format!("{}: ?", t(lang, K::SwapDiskMb)))?
                .unwrap_or(0),
            rxtx_factor,
            projects: v.list("projects"),
        };
        Ok(match &kept {
            Some((_, extra_specs)) => OsAction::FlavorReplace {
                spec: Box::new(spec),
                extra_specs: extra_specs.clone(),
            },
            None => OsAction::FlavorCreate(Box::new(spec)),
        })
    })
    .wizard(vec![
        Step::new(tr(K::FlavorInfo), info).help(tr(K::FlavorInfoHelp)),
        Step::new(tr(K::FlavorAccessStep), &["projects"]).help(tr(K::FlavorAccessHelp)),
    ]);
    if let Some(f) = old {
        d.intro = vec![f.id.clone(), tr(K::EditFlavorRecreates)];
    }
    d
}

/// Admin › Flavors' row menu, Horizon's: *Update Metadata* (extra specs),
/// *Modify Access* (private flavors: Nova can't change `is_public`) and
/// *Delete Flavor*.
pub fn admin_flavor_items(lang: Lang, f: &FlavorRow) -> Vec<MenuItem> {
    // Tracked: the settings arrive after a server-rendered table has built its menus.
    admin_flavor_menu(lang, f, crate::components::dialog::prefs_tracked().admin.flavor_edit)
}

/// [`admin_flavor_items`], with *Edit Flavor* first when `edit` (`[admin]
/// flavor_edit`, Horizon's `ENABLE_FLAVOR_EDIT`).
fn admin_flavor_menu(lang: Lang, f: &FlavorRow, edit: bool) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if edit {
        let row = f.clone();
        items.push(MenuItem::dialog(t(lang, K::EditFlavor), move || {
            edit_flavor_dialog(lang, &row)
        }));
    }
    let i = f.id.clone();
    items.push(metadata_item(
        lang,
        "OS::Nova::Flavor",
        &f.extra_specs,
        move |new, old| {
            let (set, unset) = metadata_changes(old, &new);
            OsAction::FlavorSetExtraSpecs {
                id: i.clone(),
                set,
                unset,
            }
        },
    ));
    if !f.public {
        let (i, current) = (
            f.id.clone(),
            f.access.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
        );
        items.push(MenuItem::dialog(t(lang, K::ModifyAccess), move || {
            let (i, current) = (i.clone(), current.clone());
            Dialog::form(
                t(lang, K::ModifyAccess),
                t(lang, K::Save),
                vec![
                    Field::multi("projects", t(lang, K::Projects), Source::Set(ChoiceSet::AllProjects))
                        .value(current.join(","))
                        .help(t(lang, K::FlavorAccess)),
                ],
                move |v| {
                    let new = v.list("projects");
                    let add: Vec<String> = new.iter().filter(|p| !current.contains(p)).cloned().collect();
                    let remove: Vec<String> = current.iter().filter(|p| !new.contains(p)).cloned().collect();
                    if add.is_empty() && remove.is_empty() {
                        return Err(t(lang, K::NothingChanged).to_string());
                    }
                    Ok(OsAction::FlavorSetAccess {
                        id: i.clone(),
                        add,
                        remove,
                    })
                },
            )
        }));
    }
    let (i, n) = (f.id.clone(), f.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteFlavor), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteFlavor),
            n.clone(),
            OsAction::FlavorDelete { id: i.clone() },
        )
    }));
    items
}

/// Admin › Host Aggregates' *Create Host Aggregate*: name, zone and, as
/// Horizon's second step, its hosts.
pub fn create_aggregate_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::text("name", tr(K::Name)).required(),
        Field::text("zone", tr(K::AvailabilityZone)).help(tr(K::AggregateZoneHelp)),
        Field::multi("hosts", tr(K::Hosts), Source::Set(ChoiceSet::AllComputeHosts)),
    ];
    Dialog::form(tr(K::CreateAggregate), tr(K::CreateAggregate), fields, move |v| {
        Ok(OsAction::AggregateCreate {
            name: req(v, "name", K::Name, lang)?,
            zone: v.opt("zone"),
            hosts: v.list("hosts"),
        })
    })
    .wizard(vec![
        Step::new(tr(K::CreateAggregate), &["name", "zone"]),
        Step::new(tr(K::ManageHosts), &["hosts"]),
    ])
}

/// Admin › Host Aggregates' row menu, Horizon's: *Edit Host Aggregate*,
/// *Manage Hosts*, *Update Metadata* and *Delete Host Aggregate*.
pub fn aggregate_items(lang: Lang, a: &AggregateRow) -> Vec<MenuItem> {
    let (i, n, z) = (a.id.clone(), a.name.clone(), a.zone.clone().unwrap_or_default());
    let mut items = vec![MenuItem::dialog(t(lang, K::EditAggregate), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditAggregate),
            t(lang, K::Save),
            vec![
                Field::text("name", t(lang, K::Name)).value(n.clone()).required(),
                Field::text("zone", t(lang, K::AvailabilityZone))
                    .value(z.clone())
                    .help(t(lang, K::AggregateZoneHelp)),
            ],
            move |v| {
                Ok(OsAction::AggregateUpdate {
                    id: i.clone(),
                    name: req(v, "name", K::Name, lang)?,
                    zone: v.opt("zone"),
                })
            },
        )
    })];
    let (i, current) = (a.id.clone(), a.hosts.clone());
    items.push(MenuItem::dialog(t(lang, K::ManageHosts), move || {
        let (i, current) = (i.clone(), current.clone());
        Dialog::form(
            t(lang, K::ManageHosts),
            t(lang, K::Save),
            vec![
                Field::multi("hosts", t(lang, K::Hosts), Source::Set(ChoiceSet::AllComputeHosts))
                    .value(current.join(",")),
            ],
            move |v| {
                let new = v.list("hosts");
                let add: Vec<String> = new.iter().filter(|h| !current.contains(h)).cloned().collect();
                let remove: Vec<String> = current.iter().filter(|h| !new.contains(h)).cloned().collect();
                if add.is_empty() && remove.is_empty() {
                    return Err(t(lang, K::NothingChanged).to_string());
                }
                Ok(OsAction::AggregateSetHosts {
                    id: i.clone(),
                    add,
                    remove,
                })
            },
        )
    }));
    // The zone is its own field (*Edit Host Aggregate*), though Nova keeps it as metadata.
    let editable: Vec<_> = a
        .metadata
        .iter()
        .filter(|(k, _)| k != "availability_zone")
        .cloned()
        .collect();
    let i = a.id.clone();
    items.push(metadata_item(
        lang,
        "OS::Nova::Aggregate",
        &editable,
        move |new, old| {
            let (set, unset) = metadata_changes(old, &new);
            OsAction::AggregateSetMetadata {
                id: i.clone(),
                set,
                unset,
            }
        },
    ));
    let (i, n) = (a.id.clone(), a.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteAggregate), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteAggregate),
            n.clone(),
            OsAction::AggregateDelete { id: i.clone() },
        )
    }));
    items
}

/// Hypervisors › Compute Host's row menu, Horizon's: *Disable Service* (with a
/// reason) or *Enable Service*. Needs the service's UUID (compute 2.53).
pub fn compute_service_items(lang: Lang, s: &HostService) -> Vec<MenuItem> {
    if s.id.is_empty() || !needs::SERVICE_TOGGLE.met(&caps()) {
        return Vec::new();
    }
    let (i, host) = (s.id.clone(), s.host.clone());
    if s.enabled {
        vec![MenuItem::danger(t(lang, K::DisableService), move || {
            let i = i.clone();
            let mut d = Dialog::form(
                t(lang, K::DisableService),
                t(lang, K::DisableService),
                vec![Field::text("reason", t(lang, K::DisableReason))],
                move |v| {
                    Ok(OsAction::ComputeServiceSet {
                        id: i.clone(),
                        enabled: false,
                        reason: v.opt("reason"),
                    })
                },
            );
            d.intro = vec![host.clone()];
            d
        })]
    } else {
        vec![MenuItem::dialog(t(lang, K::EnableService), move || {
            Dialog::simple(
                lang,
                t(lang, K::EnableService),
                host.clone(),
                OsAction::ComputeServiceSet {
                    id: i.clone(),
                    enabled: true,
                    reason: None,
                },
            )
        })]
    }
}

/// *Migrate Instance* (cold): the scheduler picks the host, or (compute 2.56)
/// the admin does, among the hosts other than `from` (Nova refuses the
/// current one; Horizon leaves it out).
fn migrate_item(lang: Lang, id: &str, name: &str, from: &str) -> MenuItem {
    let (i, n, from) = (id.to_string(), name.to_string(), from.to_string());
    MenuItem::dialog(t(lang, K::MigrateInstance), move || {
        let i = i.clone();
        if !needs::MIGRATE_HOST.met(&caps()) {
            return Dialog::simple(
                lang,
                t(lang, K::MigrateInstance),
                n.clone(),
                OsAction::ServerMigrate { id: i, host: None },
            );
        }
        let mut d = Dialog::form(
            t(lang, K::MigrateInstance),
            t(lang, K::MigrateInstance),
            vec![
                Field::select(
                    "host",
                    t(lang, K::NewHost),
                    Source::Except(ChoiceSet::ComputeHosts, from.clone()),
                )
                .help(t(lang, K::NewHostHelp)),
            ],
            move |v| {
                Ok(OsAction::ServerMigrate {
                    id: i.clone(),
                    host: v.opt("host"),
                })
            },
        );
        d.intro = vec![n.clone()];
        d
    })
}

/// *Live Migrate Instance* (compute 2.30): *New Host* (empty: the scheduler;
/// never `from`, the current one) and *Block Migration*, automatic by default.
fn live_migrate_item(lang: Lang, id: &str, name: &str, from: &str) -> MenuItem {
    let (i, n, from) = (id.to_string(), name.to_string(), from.to_string());
    MenuItem::dialog(t(lang, K::LiveMigrateInstance), move || {
        let i = i.clone();
        let block = options(
            &[("auto", K::BlockMigrationAuto), ("true", K::Yes), ("false", K::No)],
            lang,
        );
        let mut d = Dialog::form(
            t(lang, K::LiveMigrateInstance),
            t(lang, K::LiveMigrateInstance),
            vec![
                Field::select(
                    "host",
                    t(lang, K::NewHost),
                    Source::Except(ChoiceSet::ComputeHosts, from.clone()),
                )
                .help(t(lang, K::NewHostHelp)),
                Field::select("block", t(lang, K::BlockMigration), block).value("auto"),
            ],
            move |v| {
                let block_migration = match v.get("block").as_str() {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                };
                Ok(OsAction::ServerLiveMigrate {
                    id: i.clone(),
                    host: v.opt("host"),
                    block_migration,
                })
            },
        );
        d.intro = vec![n.clone()];
        d
    })
}

/// Admin › Instances' row menu, Horizon's admin table: Project's actions that
/// don't pick from the session project's resources (no interfaces, volumes,
/// floating IPs, security groups, rescue, resize or rebuild), plus *Migrate*
/// (ACTIVE, SHUTOFF) and *Live Migrate* (ACTIVE), before the lock items.
/// `host` is the instance's current one (`OS-EXT-SRV-ATTR:host`), left out of
/// the migration targets.
pub fn admin_instance_items(lang: Lang, inst: &Instance, host: Option<&str>) -> Vec<MenuItem> {
    use Op::*;
    let busy = crate::components::widgets::is_busy(inst.task_state.as_deref(), &inst.status);
    let ops: Vec<Op> = allowed(&inst.status, Some(inst.locked), busy)
        .into_iter()
        .filter(|op| {
            matches!(
                op,
                ConfirmResize
                    | RevertResize
                    | Snapshot
                    | Edit
                    | UpdateMetadata
                    | Pause
                    | Unpause
                    | Suspend
                    | Resume
                    | Shelve
                    | Unshelve
                    | Lock
                    | Unlock
                    | SoftReboot
                    | HardReboot
                    | ShutOff
                    | Start
                    | Delete
            )
        })
        .collect();
    let at = ops.iter().position(|o| matches!(o, Lock | Unlock)).unwrap_or(ops.len());
    let mut items: Vec<MenuItem> = ops.into_iter().map(|op| instance_item(lang, inst, op)).collect();
    if !busy {
        let mut migrate = Vec::new();
        if matches!(inst.status.as_str(), "ACTIVE" | "SHUTOFF") {
            migrate.push(migrate_item(lang, &inst.id, &inst.name, host.unwrap_or_default()));
        }
        if inst.status == "ACTIVE" && needs::LIVE_MIGRATE.met(&caps()) {
            migrate.push(live_migrate_item(lang, &inst.id, &inst.name, host.unwrap_or_default()));
        }
        items.splice(at..at, migrate);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::components::dialog::Kind;

    #[test]
    fn create_flavor_is_private_with_projects() {
        let d = create_flavor_dialog(Lang::EnUs);
        let base = [
            ("name", "m1.big"),
            ("flavor_id", "auto"),
            ("vcpus", "4"),
            ("ram", "8192"),
            ("disk", "40"),
            ("ephemeral", "0"),
            ("swap", ""),
            ("rxtx", "1.0"),
        ];
        let with = |extra: &[(&'static str, &'static str)]| {
            let mut p = base.to_vec();
            p.extend_from_slice(extra);
            (d.build)(&vals(&p))
        };
        let OsAction::FlavorCreate(spec) = with(&[("projects", "p1,p2")]).unwrap() else {
            panic!()
        };
        assert_eq!(
            (spec.vcpus, spec.ram_mb, spec.disk_gb, spec.swap_mb, spec.rxtx_factor),
            (4, 8192, 40, 0, Some(1.0)),
            "empty swap is 0"
        );
        assert_eq!(spec.projects, ["p1", "p2"]);
        assert!(with(&[("rxtx", "fast")]).is_err(), "RX/TX factor must be a number");
        assert!(
            (d.build)(&vals(&[("name", "x"), ("ram", "1"), ("disk", "0")])).is_err(),
            "VCPUs are required"
        );
    }

    #[test]
    fn modify_access_only_for_private_flavors_and_sends_changes() {
        let f = FlavorRow {
            id: "f1".into(),
            name: "m1".into(),
            vcpus: 1,
            ram_mb: 512,
            disk_gb: 1,
            ephemeral_gb: 0,
            swap_mb: 0,
            rxtx_factor: None,
            public: false,
            extra_specs: vec![],
            access: vec![("p1".into(), "One".into())],
        };
        let labels = |f: &FlavorRow| {
            admin_flavor_items(Lang::EnUs, f)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(labels(&f), ["Update Metadata", "Modify Access", "Delete Flavor"]);
        assert_eq!(
            labels(&FlavorRow {
                public: true,
                ..f.clone()
            }),
            ["Update Metadata", "Delete Flavor"],
            "Nova can't make a public flavor private"
        );
    }

    #[test]
    fn edit_flavor_only_when_enabled_and_recreates_with_the_same_id() {
        let f = FlavorRow {
            id: "f1".into(),
            name: "m1".into(),
            vcpus: 2,
            ram_mb: 2048,
            disk_gb: 20,
            ephemeral_gb: 0,
            swap_mb: 0,
            rxtx_factor: None,
            public: false,
            extra_specs: vec![("hw:cpu_policy".into(), "dedicated".into())],
            access: vec![("p1".into(), "One".into())],
        };
        let labels = |edit| {
            admin_flavor_menu(Lang::EnUs, &f, edit)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(false),
            ["Update Metadata", "Modify Access", "Delete Flavor"],
            "off by default, as Horizon's ENABLE_FLAVOR_EDIT"
        );
        assert_eq!(labels(true)[0], "Edit Flavor");
        let d = dialog_of(&admin_flavor_menu(Lang::EnUs, &f, true), "Edit Flavor");
        let field = |n: &str| d.fields.iter().find(|x| x.name == n).map(|x| x.default.clone());
        assert_eq!(
            (field("name"), field("vcpus"), field("ram"), field("projects")),
            (
                Some("m1".into()),
                Some("2".into()),
                Some("2048".into()),
                Some("p1".into())
            )
        );
        assert!(field("flavor_id").is_none(), "the ID stays");
        assert!(!d.steps[0].fields.contains(&"flavor_id"));
        let v = vals(&[
            ("name", "m1"),
            ("vcpus", "4"),
            ("ram", "4096"),
            ("disk", "20"),
            ("ephemeral", "0"),
            ("swap", "0"),
            ("rxtx", "1.0"),
            ("projects", "p1"),
        ]);
        let OsAction::FlavorReplace { spec, extra_specs } = (d.build)(&v).unwrap() else {
            panic!()
        };
        assert_eq!(
            (spec.flavor_id.as_deref(), spec.vcpus, spec.projects.clone()),
            (Some("f1"), 4, vec!["p1".to_string()])
        );
        assert_eq!(extra_specs, f.extra_specs, "the extra specs come back");
    }

    #[test]
    fn admin_instance_menu_adds_migrations_and_drops_project_pickers() {
        let labels = |status: &str| {
            admin_instance_items(Lang::EnUs, &instance(status, false), Some("c1"))
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let active = labels("ACTIVE");
        for gone in [
            "Associate Floating IP",
            "Attach Interface",
            "Attach Volume",
            "Edit Security Groups",
            "Resize Instance",
            "Rebuild Instance",
            "Rescue Instance",
        ] {
            assert!(
                !active.iter().any(|l| l == gone),
                "{gone}: picks from the session project's resources"
            );
        }
        let at = |l: &str| {
            active
                .iter()
                .position(|x| x == l)
                .unwrap_or_else(|| panic!("{l} in {active:?}"))
        };
        assert!(
            at("Migrate Instance") < at("Live Migrate Instance") && at("Live Migrate Instance") < at("Lock Instance"),
            "{active:?}"
        );
        let off = labels("SHUTOFF");
        assert!(
            off.iter().any(|l| l == "Migrate Instance") && !off.iter().any(|l| l == "Live Migrate Instance"),
            "cold only when stopped"
        );
    }

    #[test]
    fn live_migrate_sends_auto_unless_chosen() {
        let item = live_migrate_item(Lang::EnUs, "s1", "web", "c1");
        let crate::components::dialog::ItemKind::Dialog(open) = &item.kind else {
            panic!("a dialog item")
        };
        let d = open();
        assert!(
            matches!(&d.fields[0].kind, Kind::Select(Source::Except(ChoiceSet::ComputeHosts, h)) if h == "c1"),
            "Nova refuses the current host: not offered"
        );
        assert_eq!(
            (d.build)(&vals(&[("host", ""), ("block", "auto")])).unwrap(),
            OsAction::ServerLiveMigrate {
                id: "s1".into(),
                host: None,
                block_migration: None
            }
        );
        assert_eq!(
            (d.build)(&vals(&[("host", "c2"), ("block", "false")])).unwrap(),
            OsAction::ServerLiveMigrate {
                id: "s1".into(),
                host: Some("c2".into()),
                block_migration: Some(false)
            }
        );
    }

    #[test]
    fn aggregate_hosts_and_compute_services() {
        let a = AggregateRow {
            id: "7".into(),
            name: "gpu".into(),
            zone: Some("az1".into()),
            hosts: vec!["c1".into()],
            metadata: vec![("availability_zone".into(), "az1".into())],
        };
        let items = aggregate_items(Lang::EnUs, &a);
        assert_eq!(
            items.iter().map(|i| i.label.clone()).collect::<Vec<_>>(),
            [
                "Edit Host Aggregate",
                "Manage Hosts",
                "Update Metadata",
                "Delete Host Aggregate"
            ]
        );
        let crate::components::dialog::ItemKind::Dialog(open) = &items[1].kind else {
            panic!()
        };
        let d = open();
        assert_eq!(
            (d.build)(&vals(&[("hosts", "c2")])).unwrap(),
            OsAction::AggregateSetHosts {
                id: "7".into(),
                add: vec!["c2".into()],
                remove: vec!["c1".into()]
            }
        );
        assert!((d.build)(&vals(&[("hosts", "c1")])).is_err(), "nothing changed");
        let svc = HostService {
            id: "u1".into(),
            binary: "nova-compute".into(),
            host: "c1".into(),
            zone: None,
            enabled: true,
            up: true,
            updated_at: None,
            disabled_reason: None,
        };
        assert_eq!(compute_service_items(Lang::EnUs, &svc)[0].label, "Disable Service");
        assert_eq!(
            compute_service_items(
                Lang::EnUs,
                &HostService {
                    enabled: false,
                    ..svc.clone()
                }
            )[0]
            .label,
            "Enable Service"
        );
        assert!(
            compute_service_items(
                Lang::EnUs,
                &HostService {
                    id: String::new(),
                    ..svc
                }
            )
            .is_empty(),
            "no UUID (before 2.53): no action"
        );
    }
}
