//! What each resource can do: Horizon's create buttons and row-action menus,
//! expressed as dialogs that build an [`OsAction`]. Shared by list and detail pages.

use crate::components::dialog::{caps, options, options_raw, Dialog, DialogSize, Field, MenuItem, Sets, Source, Step, Vals, Values};
use std::sync::Arc;

use crate::components::upload::{guess_format, Picked};
use crate::i18n::{t, Lang, K};
use crate::prefs::ImageUpload;
use crate::model::*;

fn req(v: &Values, k: &str, label: K, lang: Lang) -> Result<String, String> {
    v.opt(k).ok_or_else(|| format!("{}: {}", t(lang, label), t(lang, K::RequiredField)))
}

fn num_u32(v: &Values, k: &str, label: K, lang: Lang) -> Result<Option<u32>, String> {
    match v.num(k).map_err(|_| format!("{}: ?", t(lang, label)))? {
        None => Ok(None),
        Some(n) if n >= 0 && n <= u32::MAX as i64 => Ok(Some(n as u32)),
        Some(_) => Err(format!("{}: ?", t(lang, label))),
    }
}

// ---------------------------------------------------------------------------
// Instances
// ---------------------------------------------------------------------------

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
fn chosen<'a>(sets: &'a Sets, set: Option<ChoiceSet>, id: Option<&String>) -> Option<&'a Choice> {
    sets.get(&set?)?.iter().find(|c| Some(&c.value) == id)
}
/// Sizing facts of the selected image (none for volume sources).
fn image_facts<'a>(v: &Vals, sets: &'a Sets) -> Option<&'a Facts> {
    image_like(v).then(|| chosen(sets, source_set(v), v.get("source_id")).map(|c| &c.facts)).flatten()
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
    let sources: Vec<(&str, K)> = BOOT_SOURCES.iter().copied().filter(|(s, _)| offered.contains(s) || preset == Some(*s)).collect();
    let first_source = sources.first().map(|(s, _)| *s).unwrap_or("image");
    let key_pair = Field::pick("key_name", tr(K::KeyPair), Source::Set(ChoiceSet::KeyPairs));
    let mut fields = vec![
        // Details
        Field::text("name", tr(K::InstanceName)).required(),
        Field::text("description", tr(K::Description)),
        Field::select("az", tr(K::AvailabilityZone), Source::Set(ChoiceSet::ComputeZones)).empty_label(tr(K::AnyAvailabilityZone)),
        Field::number("count", tr(K::Count), 1).value("1").required(),
        // Source
        Field::select("source", tr(K::BootSource), options(&sources, lang)).value(first_source).required(),
        // `[launch] create_volume`: the production cloud boots from volume, so Yes by default.
        Field::yes_no("create_volume", tr(K::CreateNewVolume), p.create_volume).show_if(image_like),
        Field::number("volume_size", tr(K::VolumeSizeGb), 1).value("1").required().show_if(creates_volume),
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
        Field::pick_many("security_groups", tr(K::SecurityGroups), Source::Set(ChoiceSet::SecurityGroups)).value("default"),
        if p.requires_keypair { key_pair.required() } else { key_pair },
        // Configuration
        Field::area("user_data", tr(K::CustomizationScript)).from_file(MAX_USER_DATA_BYTES),
        Field::select("disk_partition", tr(K::DiskPartition), options(&[("auto", K::Automatic), ("manual", K::Manual)], lang))
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
        Step::new(tr(K::StepDetails), &["name", "description", "az", "count"]).help(tr(K::DetailsHelp)).required(),
        Step::new(tr(K::StepSource), &["source", "create_volume", "volume_size", "delete_on_terminate", "source_id"])
            .help(tr(K::SourceHelp))
            .required(),
        Step::new(tr(K::Flavor), &["flavor"]).help(tr(K::FlavorHelp)).required(),
        Step::new(tr(K::Networks), &["networks"]).help(tr(K::NetworksHelp)).required(),
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
            count: num_u32(v, "count", K::Count, lang)?.unwrap_or(1),
            source,
            source_id: req(v, "source_id", K::SourceItem, lang)?,
            create_volume,
            volume_size_gb: if create_volume { num_u32(v, "volume_size", K::VolumeSizeGb, lang)? } else { None },
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
                let cur = v.get("volume_size").and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0);
                v.insert("volume_size".into(), cur.max(min).to_string());
            }
        }
        _ => {}
    })
    .check(move |v, sets| {
        let mut out = Vec::new();
        if creates_volume(v) {
            let min = image_facts(v, sets).map(min_volume_gb).unwrap_or(1);
            let size = v.get("volume_size").and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0);
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

/// Horizon's instance menu, adapted to the instance state.
/// Entries of an instance's action menu, in Horizon's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    ConfirmResize,
    RevertResize,
    Snapshot,
    AssociateIp,
    AttachInterface,
    DetachInterface,
    Edit,
    /// The server's groups on all its ports at once (roadmap 42).
    EditSecurityGroups,
    UpdateMetadata,
    AttachVolume,
    EditPortSecurityGroups,
    Rescue,
    Unrescue,
    Pause,
    Unpause,
    Suspend,
    Resume,
    Shelve,
    Unshelve,
    Resize,
    Lock,
    Unlock,
    SoftReboot,
    HardReboot,
    ShutOff,
    Start,
    Rebuild,
    Delete,
}

/// Which actions an instance offers, like Horizon's `allowed()` checks:
/// Confirm/Revert first while `VERIFY_RESIZE`, Unshelve only when shelved,
/// Unrescue only in `RESCUE`… While a task runs only Edit, (Un)lock and Delete
/// remain. `locked: None` (unknown) offers both Lock and Unlock.
pub fn allowed(status: &str, locked: Option<bool>, busy: bool) -> Vec<Op> {
    use Op::*;
    const RUNNING: &[&str] = &["ACTIVE", "SHUTOFF", "PAUSED", "SUSPENDED"];
    let s = status.to_ascii_uppercase();
    let is = |list: &[&str]| list.contains(&s.as_str());
    let lock = match locked {
        Some(true) => vec![Unlock],
        Some(false) => vec![Lock],
        None => vec![Lock, Unlock],
    };
    let mut ops = Vec::new();
    if busy {
        ops.push(Edit);
        ops.extend(lock);
        ops.push(Delete);
        return ops;
    }
    if s == "VERIFY_RESIZE" {
        ops.extend([ConfirmResize, RevertResize]);
    }
    if is(RUNNING) {
        ops.extend([Snapshot, AssociateIp]);
    }
    if is(&["ACTIVE", "SHUTOFF", "PAUSED"]) {
        ops.extend([AttachInterface, DetachInterface]);
    }
    ops.push(Edit);
    // A shelved-offloaded server's ports are unbound; Horizon hides it there too.
    if !is(&["SHELVED", "SHELVED_OFFLOADED"]) {
        ops.push(EditSecurityGroups);
    }
    // Next to the instance-wide one, so both ways of editing groups sit together (roadmap 43).
    ops.push(EditPortSecurityGroups);
    ops.push(UpdateMetadata);
    if is(RUNNING) {
        ops.push(AttachVolume);
    }
    if is(&["ACTIVE", "SHUTOFF"]) {
        ops.push(Rescue);
    }
    if s == "RESCUE" {
        ops.push(Unrescue);
    }
    match s.as_str() {
        "ACTIVE" => ops.extend([Pause, Suspend]),
        "PAUSED" => ops.push(Unpause),
        "SUSPENDED" => ops.push(Resume),
        _ => {}
    }
    if is(RUNNING) {
        ops.push(Shelve);
    }
    if is(&["SHELVED", "SHELVED_OFFLOADED"]) {
        ops.push(Unshelve);
    }
    if is(&["ACTIVE", "SHUTOFF"]) {
        ops.push(Resize);
    }
    ops.extend(lock);
    if s == "ACTIVE" {
        ops.extend([SoftReboot, HardReboot, ShutOff]);
    } else if is(&["SHUTOFF", "PAUSED", "SUSPENDED", "ERROR", "RESCUE"]) {
        ops.push(HardReboot);
    }
    if s == "SHUTOFF" {
        ops.push(Start);
    }
    if is(&["ACTIVE", "SHUTOFF"]) {
        ops.push(Rebuild);
    }
    ops.push(Delete);
    // A stopped instance's split button offers *Start Instance* (Horizon's
    // primary action there), not *Create Snapshot*.
    if let Some(i) = ops.iter().position(|o| *o == Start) {
        let start = ops.remove(i);
        ops.insert(0, start);
    }
    ops
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

fn instance_item(lang: Lang, inst: &Instance, op: Op) -> MenuItem {
    let tr = |k| t(lang, k).to_string();
    let (id, name) = (inst.id.clone(), inst.name.clone());
    // One-click actions still confirm, like Horizon.
    let simple = |label: K, a: OsAction| {
        let n = name.clone();
        MenuItem::dialog(tr(label), move || Dialog::simple(lang, t(lang, label), n.clone(), a.clone()))
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
                vec![Field::text("name", t(lang, K::SnapshotName)).value(format!("{name}-snapshot")).required()],
                move |v| Ok(OsAction::ServerSnapshot { id: i.clone(), name: req(v, "name", K::SnapshotName, lang)? }),
            )
        }),
        Op::AssociateIp => MenuItem::dialog(tr(K::AssociateFloatingIp), move || associate_ip_dialog(lang, &i)),
        Op::AttachInterface => MenuItem::dialog(tr(K::AttachInterface), move || attach_interface_dialog(lang, &i)),
        Op::DetachInterface => MenuItem::danger(tr(K::DetachInterface), move || {
            let i = i.clone();
            let mut d = Dialog::form(
                t(lang, K::DetachInterface),
                t(lang, K::DetachInterface),
                vec![Field::select("port", t(lang, K::Port), Source::Of(ChoiceSet::InstancePorts, i.clone())).required()],
                move |v| Ok(OsAction::ServerDetachInterface { id: i.clone(), port_id: req(v, "port", K::Port, lang)? }),
            );
            d.danger = true;
            d
        }),
        Op::UpdateMetadata => {
            let i = i.clone();
            metadata_item(lang, "OS::Nova::Server", &inst.metadata, move |metadata, _| OsAction::ServerSetMetadata { id: i.clone(), metadata })
        }
        Op::Edit => MenuItem::dialog(tr(K::EditInstance), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::EditInstance),
                t(lang, K::Save),
                vec![Field::text("name", t(lang, K::InstanceName)).value(name.clone()).required()],
                move |v| Ok(OsAction::ServerUpdate { id: i.clone(), name: req(v, "name", K::InstanceName, lang)? }),
            )
        }),
        Op::AttachVolume => MenuItem::dialog(tr(K::AttachVolume), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::AttachVolume),
                t(lang, K::AttachVolume),
                vec![Field::select("volume", t(lang, K::Volume), Source::Set(ChoiceSet::AvailableVolumes)).required()],
                move |v| Ok(OsAction::ServerAttachVolume { server_id: i.clone(), volume_id: req(v, "volume", K::Volume, lang)? }),
            )
        }),
        Op::EditSecurityGroups => {
            let current = inst.security_groups.clone();
            MenuItem::dialog(tr(K::EditSecurityGroups), move || security_groups_dialog(lang, &i, &current))
        }
        Op::EditPortSecurityGroups => MenuItem::dialog(tr(K::EditPortSecurityGroups), move || port_security_groups_dialog(lang, &i)),
        Op::Rescue => MenuItem::dialog(tr(K::RescueInstance), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::RescueInstance),
                t(lang, K::RescueInstance),
                vec![Field::select("image", t(lang, K::RescueImage), Source::Set(ChoiceSet::Images)).help(t(lang, K::RescueImageHelp))],
                move |v| Ok(OsAction::ServerRescue { id: i.clone(), image_id: v.opt("image") }),
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
                    vec![Field::select("az", t(lang, K::AvailabilityZone), Source::Set(ChoiceSet::ComputeZones))
                        .empty_label(t(lang, K::AnyAvailabilityZone))
                        .help(t(lang, K::AnyZone))]
                } else {
                    vec![]
                };
                let mut d = Dialog::form(t(lang, K::UnshelveInstance), t(lang, K::UnshelveInstance), fields, move |v| {
                    Ok(OsAction::ServerUnshelve { id: i.clone(), availability_zone: v.opt("az") })
                });
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
                        Ok(OsAction::ServerResize { id: i.clone(), flavor_id })
                    },
                )
            })
        }
        Op::Lock => simple(K::LockInstance, OsAction::ServerLock { id }),
        Op::Unlock => simple(K::UnlockInstance, OsAction::ServerUnlock { id }),
        Op::SoftReboot => simple(K::SoftReboot, OsAction::ServerReboot { id, kind: RebootKind::Soft }),
        Op::HardReboot => simple(K::HardReboot, OsAction::ServerReboot { id, kind: RebootKind::Hard }),
        Op::ShutOff => simple(K::ShutOff, OsAction::ServerStop { id }),
        Op::Start => simple(K::StartInstance, OsAction::ServerStart { id }),
        Op::Rebuild => MenuItem::danger(tr(K::RebuildInstance), move || {
            let i = i.clone();
            let mut d = Dialog::form(
                t(lang, K::RebuildInstance),
                t(lang, K::RebuildInstance),
                vec![Field::select("image", t(lang, K::BootImage), Source::Set(ChoiceSet::Images)).required().help(t(lang, K::RebuildHelp))],
                move |v| Ok(OsAction::ServerRebuild { id: i.clone(), image_id: req(v, "image", K::BootImage, lang)? }),
            );
            d.danger = true;
            d
        }),
        Op::Delete => MenuItem::danger(tr(K::DeleteInstance), move || {
            Dialog::confirm(lang, t(lang, K::DeleteInstance), name.clone(), OsAction::ServerDelete { id: i.clone() })
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
            Field::select("ip", t(lang, K::FloatingIp), Source::Set(ChoiceSet::FreeFloatingIps)).required().when("allocate", "false"),
            Field::select("pool", t(lang, K::Pool), Source::Set(ChoiceSet::ExternalNetworks)).required().when("allocate", "true"),
            Field::select("port", t(lang, K::PortToAssociate), Source::Of(ChoiceSet::AssociablePorts, server_id.to_string())).required(),
        ],
        move |v| {
            let port_id = req(v, "port", K::PortToAssociate, lang)?;
            Ok(if v.flag("allocate") {
                OsAction::FloatingIpAllocate { network_id: req(v, "pool", K::Pool, lang)?, description: None, port_id: Some(port_id), qos_policy_id: None }
            } else {
                OsAction::FloatingIpAssociate { id: req(v, "ip", K::FloatingIp, lang)?, port_id }
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
            Field::select("by", t(lang, K::InterfaceSource), options(&[("network", K::ByNetwork), ("port", K::ByPort)], lang))
                .value("network")
                .required(),
            Field::select("network", t(lang, K::Network), Source::Set(ChoiceSet::Networks)).required().when("by", "network"),
            Field::text("fixed_ip", t(lang, K::FixedIpAddress)).help(t(lang, K::FixedIpHelp)).when("by", "network"),
            Field::select("port", t(lang, K::Port), Source::Set(ChoiceSet::FreePorts)).required().when("by", "port"),
        ],
        move |v| {
            Ok(if v.get("by") == "port" {
                OsAction::ServerAttachInterface { id: id.clone(), network_id: None, port_id: Some(req(v, "port", K::Port, lang)?), fixed_ip: None }
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
        vec![Field::pick_many("groups", t(lang, K::SecurityGroups), Source::Set(ChoiceSet::SecurityGroups)).value(current.join(",")).help(t(lang, K::EditSecurityGroupsHelp))],
        move |v| {
            let (add, remove) = list_diff(&current, &v.list("groups"));
            if add.is_empty() && remove.is_empty() {
                return Err(t(lang, K::NothingChanged).to_string());
            }
            Ok(OsAction::ServerSetSecurityGroups { id: id.clone(), add, remove })
        },
    )
}

/// `(added, removed)` going from `before` to `after`, each in its list's order.
fn list_diff(before: &[String], after: &[String]) -> (Vec<String>, Vec<String>) {
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
            Field::select("port", t(lang, K::Port), Source::Of(ChoiceSet::InstancePorts, server_id.to_string())).required(),
            // Same filtered tables as *Edit Security Groups*; the port's groups are
            // preset by `on_change` as the same comma list (roadmap 43).
            Field::pick_many("groups", t(lang, K::SecurityGroups), Source::Set(ChoiceSet::SecurityGroupIds)),
        ],
        move |v| Ok(OsAction::PortSetSecurityGroups { id: req(v, "port", K::Port, lang)?, security_group_ids: v.list("groups") }),
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
    let out: Vec<_> = CINDER.into_iter().filter(|f| configured.iter().any(|c| c == f)).collect();
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
        item(K::SoftRebootInstances, |id| OsAction::ServerReboot { id, kind: RebootKind::Soft }),
    ]
}

/// Confirmation for a bulk delete: lists every selected name and runs one
/// [`OsAction::Batch`], so the server reports each item separately.
pub fn bulk_delete(lang: Lang, title: K, items: Vec<(String, String)>, make: fn(String) -> OsAction) -> Dialog {
    bulk(lang, title, items, make, true)
}

/// A bulk action over `(id, name)` pairs; destructive ones are red and warn
/// that they cannot be undone.
pub fn bulk(lang: Lang, title: K, items: Vec<(String, String)>, make: fn(String) -> OsAction, danger: bool) -> Dialog {
    let names: Vec<String> = items.iter().map(|(_, n)| n.clone()).collect();
    let too_many = items.len() > MAX_BATCH;
    let batch = OsAction::Batch(items.into_iter().map(|(id, n)| (n, make(id))).collect());
    let title_key = title;
    let title = t(lang, title);
    let mut d = Dialog::form(title, title, vec![], move |_| {
        if too_many { Err(t(lang, K::TooManySelected).to_string()) } else { Ok(batch.clone()) }
    });
    d.intro = vec![t(lang, K::YouSelected).into()];
    d.selection = names;
    if danger {
        d.intro.push(t(lang, K::CannotBeUndone).into());
    }
    // What a Barbican delete takes with it (roadmap 55).
    match title_key {
        K::DeleteSecrets => d.intro.push(t(lang, K::SecretInUse).into()),
        K::DeleteContainers => d.intro.push(t(lang, K::ContainerKeepsSecrets).into()),
        _ => {}
    }
    d.danger = danger;
    d
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
        let mut types: Vec<(String, String)> = VNIC_TYPES.iter().map(|(v, k)| (v.to_string(), t(lang, *k).to_string())).collect();
        // Keep a type Horizon doesn't list (e.g. `accelerator-direct`) selectable.
        if let Some(v) = current.vnic_type.as_ref().filter(|v| !VNIC_TYPES.iter().any(|(x, _)| x == v)) {
            types.push((v.clone(), v.clone()));
        }
        let value = current.vnic_type.clone().unwrap_or_else(|| "normal".into());
        fields.push(Field::select("vnic_type", t(lang, K::VnicType), Source::Static(types)).value(value).required());
        info.push("vnic_type");
    }
    if needs::PORT_SECURITY.met(&caps) {
        fields.push(Field::check("port_security", t(lang, K::PortSecurity), current.port_security));
        info.push("port_security");
    }
    fields.push(
        Field::pick_many("groups", t(lang, K::SecurityGroups), Source::Set(ChoiceSet::SecurityGroupIds))
            .value(current.security_group_ids.join(",")),
    );
    let groups_on: fn(&Vals) -> bool = |v| v.get("port_security").is_none_or(|p| p == "true");
    let steps = vec![
        Step::new(t(lang, K::StepInfo), &info).help(t(lang, K::EditPortInfoHelp)),
        Step::new(t(lang, K::SecurityGroups), &["groups"]).help(t(lang, K::EditPortGroupsHelp)).enabled_if(groups_on),
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
            port_security: if v.0.contains_key("port_security") { v.flag("port_security") } else { before.port_security },
            security_group_ids: if v.0.contains_key("groups") { v.list("groups") } else { before.security_group_ids.clone() },
        };
        before.changes(&id, &after).ok_or_else(|| t(lang, K::NothingChanged).to_string())
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
        items.push(MenuItem::dialog(t(lang, K::EditSecurityGroups), move || edit_port_dialog(lang, &i, &p, true)));
        let (i, p) = (it.port_id.clone(), port.clone());
        items.push(MenuItem::dialog(t(lang, K::EditPort), move || edit_port_dialog(lang, &i, &p, false)));
    }
    items.push(detach_interface_item(lang, server_id, &it.port_id, &it.fixed_ips.join(", ")));
    items
}

/// *Detach Interface* on a row of the instance's Interfaces tab.
pub fn detach_interface_item(lang: Lang, server_id: &str, port_id: &str, label: &str) -> MenuItem {
    let (s, p, l) = (server_id.to_string(), port_id.to_string(), if label.is_empty() { port_id } else { label }.to_string());
    MenuItem::danger(t(lang, K::DetachInterface), move || {
        Dialog::confirm(lang, t(lang, K::DetachInterface), l.clone(), OsAction::ServerDetachInterface { id: s.clone(), port_id: p.clone() })
    })
}

/// *Detach Volume* for one attachment (instance detail, *Volumes* card).
pub fn detach_volume_item(lang: Lang, server_id: &str, volume_id: &str, volume_name: &str) -> MenuItem {
    let (s, v, n) = (server_id.to_string(), volume_id.to_string(), volume_name.to_string());
    MenuItem::danger(t(lang, K::DetachVolume), move || {
        Dialog::confirm(lang, t(lang, K::DetachVolume), n.clone(), OsAction::ServerDetachVolume { server_id: s.clone(), volume_id: v.clone() })
    })
}

// ---------------------------------------------------------------------------
// Images, key pairs, server groups
// ---------------------------------------------------------------------------

/// An image's menu: launch, edit, delete.
pub fn image_items(lang: Lang, im: &Image) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if im.status.eq_ignore_ascii_case("active") {
        let (i, src) = (im.id.clone(), if im.is_snapshot { "instance_snapshot" } else { "image" });
        items.push(MenuItem::dialog(t(lang, K::LaunchFromRow), move || launch_from(lang, src, &i)));
    }
    items.push(create_volume_item(lang, VolumeSource::Image, &im.id, im.size.map(|b| b.div_ceil(1 << 30).max(1) as u32)));
    if im.owned_by_project {
        items.extend(image_owner_items(lang, im, needs::PUBLICIZE_IMAGE.met(&caps())));
    }
    items
}

/// What an image's owner can do: *Edit Image*, *Update Metadata* and *Delete
/// Image*. `publicize`: *Public* is offered in the edit dialog.
fn image_owner_items(lang: Lang, im: &Image, publicize: bool) -> Vec<MenuItem> {
    let im2 = im.clone();
    let formats = crate::components::dialog::prefs().images.disk_formats;
    let mut items = vec![MenuItem::dialog(t(lang, K::EditImage), move || edit_image_dialog(lang, &im2, publicize, &formats))];
    items.push(image_metadata_item(lang, im));
    // Glance refuses deleting a protected image; Horizon hides the item.
    if !im.protected {
        let (i, n) = (im.id.clone(), im.name.clone());
        items.push(MenuItem::danger(t(lang, K::DeleteImage), move || {
            Dialog::confirm(lang, t(lang, K::DeleteImage), n.clone(), OsAction::ImageDelete { id: i.clone() })
        }));
    }
    items
}

/// An image's *Update Metadata*.
fn image_metadata_item(lang: Lang, im: &Image) -> MenuItem {
    // `os_glance_*` properties belong to Glance's import machinery, and `[images]
    // reserved_properties` are the operator's: neither is editable.
    let reserved = crate::components::dialog::prefs().images.reserved_properties;
    let editable: Vec<_> = im.properties.iter().filter(|(k, _)| !is_reserved_image_key(k, &reserved)).cloned().collect();
    let i = im.id.clone();
    metadata_item(lang, "OS::Glance::Image", &editable, move |new, old| {
        let (set, remove) = metadata_changes(old, &new);
        OsAction::ImageSetProperties { id: i.clone(), set, remove }
    })
}

/// Admin › Images' row menu, Horizon's admin table: *Edit Image*, *Update
/// Metadata* and *Delete Image* on every project's image (Glance lets an admin
/// change any, and publicize: `publicize_image` is admin-only by default).
pub fn admin_image_items(lang: Lang, im: &Image) -> Vec<MenuItem> {
    image_owner_items(lang, im, true)
}

/// Custom properties *Edit Image* shows as fields (Horizon's *Image Detail*
/// step); each field is named after its property.
const IMAGE_FIELD_PROPS: [&str; 4] = ["description", "architecture", "kernel_id", "ramdisk_id"];

/// Horizon's *Edit Image*: name, description, requirements (kernel and ramdisk
/// for AMI-style images, architecture, minimum disk and RAM), format while
/// `queued`, visibility (`public` only with `publicize`, or when it already is)
/// and protection. Submits only the changes.
pub fn edit_image_dialog(lang: Lang, im: &Image, publicize: bool, formats: &[String]) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let ami = matches!(im.disk_format.as_deref(), Some("ami" | "aki" | "ari"));
    let queued = im.status.eq_ignore_ascii_case("queued");
    let prop = |k: &str| im.property(k).unwrap_or("").to_string();
    let mut vis: Vec<&str> = vec!["private", "shared", "community"];
    if publicize || im.visibility == "public" {
        vis.push("public");
    }
    let mut fields = vec![
        Field::text("name", tr(K::Name)).value(im.name.clone()).required(),
        Field::area("description", tr(K::Description)).value(prop("description")),
    ];
    if ami {
        fields.push(Field::text("kernel_id", tr(K::KernelId)).value(prop("kernel_id")));
        fields.push(Field::text("ramdisk_id", tr(K::RamdiskId)).value(prop("ramdisk_id")));
    }
    fields.push(Field::text("architecture", tr(K::Architecture)).value(prop("architecture")).help(tr(K::ArchitectureHelp)));
    fields.push(Field::number("min_disk", tr(K::MinDiskGb), 0).value(im.min_disk_gb.unwrap_or(0).to_string()));
    fields.push(Field::number("min_ram", tr(K::MinRamMb), 0).value(im.min_ram_mb.unwrap_or(0).to_string()));
    if queued {
        let opts: Vec<&str> = formats.iter().map(String::as_str).collect();
        fields.push(Field::select("disk_format", tr(K::DiskFormat), options_raw(&opts)).value(im.disk_format.clone().unwrap_or_default()).required());
    }
    fields.push(Field::select("visibility", tr(K::Visibility), options_raw(&vis)).value(im.visibility.clone()).required());
    fields.push(Field::check("protected", tr(K::Protected), im.protected));
    let old = im.clone();
    Dialog::form(tr(K::EditImage), tr(K::Save), fields, move |v| {
        let mut c = ImageChanges::default();
        let name = req(v, "name", K::Name, lang)?;
        c.name = (name != old.name).then_some(name);
        let visibility = req(v, "visibility", K::Visibility, lang)?;
        c.visibility = (visibility != old.visibility).then_some(visibility);
        c.protected = (v.flag("protected") != old.protected).then(|| v.flag("protected"));
        let min_disk = num_u32(v, "min_disk", K::MinDiskGb, lang)?.unwrap_or(0);
        c.min_disk_gb = (i64::from(min_disk) != old.min_disk_gb.unwrap_or(0)).then_some(min_disk);
        let min_ram = num_u32(v, "min_ram", K::MinRamMb, lang)?.unwrap_or(0);
        c.min_ram_mb = (i64::from(min_ram) != old.min_ram_mb.unwrap_or(0)).then_some(min_ram);
        if let Some(f) = v.opt("disk_format").filter(|f| Some(f.as_str()) != old.disk_format.as_deref()) {
            c.disk_format = Some(f);
        }
        for key in IMAGE_FIELD_PROPS {
            // Kernel and ramdisk are only shown (and submitted) for AMI-style images.
            if !v.0.contains_key(key) {
                continue;
            }
            let (new, cur) = (v.get(key), old.property(key));
            match (new.is_empty(), cur) {
                (true, Some(_)) => c.remove.push(key.to_string()),
                (false, c0) if c0 != Some(new.as_str()) => c.set.push((key.to_string(), new)),
                _ => {}
            }
        }
        if c == ImageChanges::default() {
            return Err(t(lang, K::NothingChanged).to_string());
        }
        Ok(OsAction::ImageUpdate { id: old.id.clone(), changes: c })
    })
}

/// Horizon's *Create Image* (roadmap 45): details, source, requirements (kernel and ramdisk
/// for AMI-style formats), sharing and metadata. The *URL* source needs
/// `[images] allow_location` (Horizon's `IMAGES_ALLOW_LOCATION`) and Glance's `web-download`;
/// *File* needs `[images] upload` to be `proxy` or `direct`.
pub fn create_image_dialog(lang: Lang) -> Dialog {
    let img = crate::components::dialog::prefs().images;
    let c = caps();
    create_image_form(lang, &img, needs::PUBLICIZE_IMAGE.met(&c), needs::IMAGE_URL_IMPORT.met(&c))
}

/// [`create_image_dialog`] for given settings: `publicize` (the admin role) offers `public`
/// visibility, `url_import` is whether Glance has `web-download`.
fn create_image_form(lang: Lang, img: &crate::prefs::ImagePrefs, publicize: bool, url_import: bool) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let formats: Vec<&str> = img.disk_formats.iter().map(String::as_str).collect();
    let mut vis: Vec<&str> = vec!["private", "shared", "community"];
    if publicize {
        vis.push("public");
    }
    let default_vis = if vis.contains(&img.default_visibility.as_str()) { img.default_visibility.clone() } else { "private".into() };
    let mut sources = vec![("none".to_string(), tr(K::ImageSourceNone))];
    // *File* (roadmap 45): `[images] upload`, whatever the cloud, since the upload is plain Glance.
    if img.upload != ImageUpload::Off {
        sources.insert(0, ("file".to_string(), tr(K::ImageSourceFile)));
    }
    // Glance rejects `web-download` when the method is off (`GET /v2/info/import`, roadmap 45).
    if img.allow_location && url_import {
        sources.insert(0, ("url".to_string(), tr(K::ImageSourceUrl)));
    }
    let first_source = sources[0].0.clone();
    let fields = vec![
        Field::text("name", tr(K::ImageName)).required(),
        Field::area("description", tr(K::Description)).rows(2),
        // The source and its URL or file share a line (only one of the two shows).
        Field::select("source", tr(K::ImageSource), Source::Static(sources)).value(first_source).required().help(tr(K::ImageSourceNoneHelp)).half(),
        Field::text("url", tr(K::Url)).required().help(tr(K::ImageUrlHelp)).when("source", "url").half(),
        Field::file("file", tr(K::ImageSourceFile)).required().help(tr(K::ImageFileHelp)).when("source", "file").half(),
        // Format and architecture, then minimum disk and RAM, share a line each; the AMI-only
        // kernel and ramdisk (full rows) sit between the two pairs.
        Field::select("disk_format", tr(K::DiskFormat), options_raw(&formats)).value(formats.first().copied().unwrap_or("qcow2")).required().half(),
        // The hint is a tooltip: a help line would make this row taller than its neighbours.
        Field::text("architecture", tr(K::Architecture)).tip(tr(K::ArchitectureHelp)).half(),
        Field::text("kernel_id", tr(K::KernelId)).when("disk_format", "ami"),
        Field::text("ramdisk_id", tr(K::RamdiskId)).when("disk_format", "ami"),
        Field::number("min_disk", tr(K::MinDiskGb), 0).value("0").half(),
        Field::number("min_ram", tr(K::MinRamMb), 0).value("0").half(),
        Field::select("visibility", tr(K::Visibility), options_raw(&vis)).value(default_vis).required().half(),
        Field::check("protected", tr(K::Protected), false).half(),
        Field::key_values("metadata", tr(K::Metadata), &[]).help(tr(K::MetadataHelp)).catalog("OS::Glance::Image").catalog_skip(&IMAGE_FIELD_PROPS),
    ];
    let (max_bytes, offered) = (img.max_upload_bytes(), formats.iter().map(|f| f.to_string()).collect::<Vec<_>>());
    let details: Vec<&'static str> = fields.iter().map(|f| f.name).filter(|n| *n != "metadata").collect();
    let mut dialog = Dialog::form(tr(K::CreateImage), tr(K::CreateImage), fields, move |v| {
        let file = if v.get("source") == "file" {
            let picked = Picked::decode(&v.get("file")).ok_or_else(|| invalid(lang, t(lang, K::ImageSourceFile).to_string()))?;
            if picked.size > max_bytes {
                return Err(too_big(lang, max_bytes));
            }
            Some(picked.size)
        } else {
            None
        };
        let mut properties = validate_metadata(&v.pairs("metadata")).map_err(|e| invalid(lang, e))?;
        // Horizon keeps these as custom properties; kernel and ramdisk only for AMI.
        for key in IMAGE_FIELD_PROPS {
            let value = v.opt(key).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            if let Some(value) = value {
                if properties.iter().any(|(k, _)| k == key) {
                    return Err(invalid(lang, format!("{key}: repeated key")));
                }
                properties.push((key.to_string(), value));
            }
        }
        Ok(OsAction::ImageCreate(ImageSpec {
            name: req(v, "name", K::ImageName, lang)?,
            url: if v.get("source") == "url" { Some(req(v, "url", K::Url, lang)?) } else { None },
            disk_format: req(v, "disk_format", K::DiskFormat, lang)?,
            container_format: None,
            visibility: req(v, "visibility", K::Visibility, lang)?,
            protected: v.flag("protected"),
            min_disk_gb: num_u32(v, "min_disk", K::MinDiskGb, lang)?.filter(|n| *n > 0),
            min_ram_mb: num_u32(v, "min_ram", K::MinRamMb, lang)?.filter(|n| *n > 0),
            properties,
            upload_bytes: file,
        }))
    });
    // Horizon's wizard: the details, then the metadata (item 25 adds the catalog to that step).
    dialog = dialog.wizard(vec![Step::new(tr(K::ImageDetails), &details), Step::new(tr(K::Metadata), &["metadata"])]);
    // Picking a file preselects its disk format (Horizon does), and a file over the limit is
    // refused on the field, before anything is created.
    dialog.on_change = Some(Arc::new(move |name, vals, _| {
        if name == "file"
            && let Some(f) = Picked::decode(vals.get("file").map_or("", String::as_str)).and_then(|p| guess_format(&p.name, &offered))
        {
            vals.insert("disk_format".into(), f);
        }
    }));
    dialog.check = Some(Arc::new(move |vals, _| {
        let size = Picked::decode(vals.get("file").map_or("", String::as_str)).map(|p| p.size);
        if vals.get("source").is_some_and(|s| s == "file") && size.is_some_and(|n| n > max_bytes) {
            vec![("file", too_big(lang, max_bytes))]
        } else {
            vec![]
        }
    }));
    dialog
}

/// `The file is larger than the allowed 10 GiB`.
fn too_big(lang: Lang, max_bytes: u64) -> String {
    format!("{} {} GiB", t(lang, K::ImageFileTooBig), max_bytes >> 30)
}

/// `field` with the catalog's suggestions for `catalog`, when there is a resource type.
fn with_catalog(field: Field, catalog: Option<&'static str>) -> Field {
    catalog.map_or(field.clone(), |rt| field.catalog(rt))
}

/// *Update Metadata*: the current pairs in a key/value editor. `to_action`
/// gets the edited pairs (validated, sorted) and the current ones; nothing
/// is submitted when they are equal.
fn metadata_item(
    lang: Lang,
    catalog: &'static str,
    current: &[(String, String)],
    to_action: impl Fn(Vec<(String, String)>, &[(String, String)]) -> OsAction + Send + Sync + 'static,
) -> MenuItem {
    pairs_item(lang, K::UpdateMetadata, K::Metadata, K::MetadataHelp, Some(catalog), current, to_action)
}

/// A key/value editor entry titled `title` (metadata, extra specs, QoS or
/// group specs): `to_action` gets the new pairs and the current ones; an
/// unchanged list is refused.
fn pairs_item(
    lang: Lang,
    title: K,
    label: K,
    help: K,
    catalog: Option<&'static str>,
    current: &[(String, String)],
    to_action: impl Fn(Vec<(String, String)>, &[(String, String)]) -> OsAction + Send + Sync + 'static,
) -> MenuItem {
    let mut current = current.to_vec();
    current.sort();
    let current = std::sync::Arc::new(current);
    let to_action = std::sync::Arc::new(to_action);
    MenuItem::dialog(t(lang, title), move || {
        let (cur, to_action) = (current.clone(), to_action.clone());
        Dialog::form(
            t(lang, title),
            t(lang, K::Save),
            vec![with_catalog(Field::key_values("metadata", t(lang, label), &cur).help(t(lang, help)), catalog)],
            move |v| {
                let new = validate_metadata(&v.pairs("metadata"))?;
                if new == *cur {
                    return Err(t(lang, K::NothingChanged).to_string());
                }
                Ok(to_action(new, &cur))
            },
        )
        // Policy can still say no (Glance answers 403): a plain "no permission" instead of the raw error.
        .forbidden(t(lang, K::MetadataForbidden))
    })
}

/// *Import Public Key*: name, key type (SSH or X509, roadmap 51) and the key,
/// checked against the type before sending.
pub fn import_keypair_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::ImportPublicKey),
        t(lang, K::ImportPublicKey),
        vec![
            Field::text("name", t(lang, K::Name)).required(),
            Field::select("key_type", t(lang, K::KeyType), Source::Static(vec![("ssh".into(), t(lang, K::SshKey).into()), ("x509".into(), t(lang, K::X509Certificate).into())]))
                .value("ssh")
                .required(),
            Field::area("public_key", t(lang, K::PublicKey)).required().help(t(lang, K::PublicKeyHelp)),
        ],
        move |v| {
            let (public_key, key_type) = (req(v, "public_key", K::PublicKey, lang)?, v.get("key_type"));
            check_public_key(&key_type, &public_key).map_err(|e| invalid(lang, e))?;
            Ok(OsAction::KeyPairImport { name: req(v, "name", K::Name, lang)?, public_key, key_type })
        },
    )
}

/// *Create Key Pair*: an Ed25519 SSH key generated on the server; the private key
/// is shown once.
pub fn generate_keypair_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::CreateKeyPair),
        t(lang, K::CreateKeyPair),
        vec![Field::text("name", t(lang, K::Name)).required().help(t(lang, K::GeneratedKeyHelp))],
        move |v| Ok(OsAction::KeyPairGenerate { name: req(v, "name", K::Name, lang)? }),
    )
}

/// A key pair's menu (only *Delete Key Pair*).
pub fn keypair_items(lang: Lang, name: &str) -> Vec<MenuItem> {
    let n = name.to_string();
    vec![MenuItem::danger(t(lang, K::DeleteKeyPair), move || {
        Dialog::confirm(lang, t(lang, K::DeleteKeyPair), n.clone(), OsAction::KeyPairDelete { name: n.clone() })
    })]
}

/// *Create Server Group*: name and policy.
pub fn create_server_group_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::CreateServerGroup),
        t(lang, K::CreateServerGroup),
        vec![
            Field::text("name", t(lang, K::Name)).required(),
            Field::select("policy", t(lang, K::Policy), options_raw(&["anti-affinity", "affinity", "soft-anti-affinity", "soft-affinity"]))
                .value("anti-affinity")
                .required(),
        ],
        move |v| Ok(OsAction::ServerGroupCreate { name: req(v, "name", K::Name, lang)?, policy: req(v, "policy", K::Policy, lang)? }),
    )
}

/// A server group's menu (only *Delete Server Group*).
pub fn server_group_items(lang: Lang, g: &ServerGroup) -> Vec<MenuItem> {
    group_items(lang, g, None)
}

/// The detail page's menu: the same, going back to the list after deleting.
pub fn server_group_detail_items(lang: Lang, g: &ServerGroup) -> Vec<MenuItem> {
    group_items(lang, g, Some("/project/server_groups"))
}

fn group_items(lang: Lang, g: &ServerGroup, back: Option<&'static str>) -> Vec<MenuItem> {
    let (i, n) = (g.id.clone(), g.name.clone());
    vec![MenuItem::danger(t(lang, K::DeleteServerGroup), move || {
        let d = Dialog::confirm(lang, t(lang, K::DeleteServerGroup), n.clone(), OsAction::ServerGroupDelete { id: i.clone() });
        match back {
            Some(href) => d.then_go(href),
            None => d,
        }
    })]
}

// ---------------------------------------------------------------------------
// Volumes & snapshots
// ---------------------------------------------------------------------------

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
                options(&[("empty", K::SourceEmpty), ("image", K::BootImage), ("snapshot", K::Snapshots), ("volume", K::Volume)], lang),
            )
            .value("empty")
            .required(),
        );
        fields.push(Field::select("source_id", tr(K::SourceItem), Source::Set(ChoiceSet::Images)).when("source", "image"));
        fields.push(Field::select("source_snap", tr(K::SourceItem), Source::Set(ChoiceSet::VolumeSnapshots)).when("source", "snapshot"));
        fields.push(Field::select("source_vol", tr(K::SourceItem), Source::Set(ChoiceSet::AvailableVolumes)).when("source", "volume"));
    }
    fields.push(Field::select("volume_type", tr(K::VolumeType), Source::Set(ChoiceSet::VolumeTypes)));
    fields.push(Field::number("size", tr(K::SizeGb), 1).value(size.unwrap_or(1).to_string()).required());
    fields.push(Field::select("az", tr(K::AvailabilityZone), Source::Set(ChoiceSet::VolumeZones)).empty_label(tr(K::AnyAvailabilityZone)));

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
            size_gb: num_u32(v, "size", K::SizeGb, lang)?.unwrap_or(1),
            volume_type: v.opt("volume_type"),
            availability_zone: v.opt("az"),
            source,
            source_id,
        })
    })
}

fn create_volume_item(lang: Lang, src: VolumeSource, id: &str, size: Option<u32>) -> MenuItem {
    let id = id.to_string();
    MenuItem::dialog(t(lang, K::CreateVolume), move || create_volume_dialog(lang, Some((src, id.clone(), size))))
}

/// A volume's menu: launch, edit, extend, snapshot, change type, upload to image,
/// attach, transfer and delete, as the volume's state allows (like Horizon).
pub fn volume_items(lang: Lang, v: &Volume) -> Vec<MenuItem> {
    let tr = |k| t(lang, k).to_string();
    let mut items = Vec::new();
    if v.bootable && v.status.eq_ignore_ascii_case("available") {
        let i = v.id.clone();
        items.push(MenuItem::dialog(tr(K::LaunchAsInstance), move || launch_from(lang, "volume", &i)));
    }
    let (i, n, d) = (v.id.clone(), v.name.clone(), v.description.clone().unwrap_or_default());
    items.push(MenuItem::dialog(tr(K::EditVolume), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditVolume),
            t(lang, K::Save),
            vec![Field::text("name", t(lang, K::VolumeName)).value(n.clone()), Field::area("description", t(lang, K::Description)).value(d.clone())],
            move |v| Ok(OsAction::VolumeUpdate { id: i.clone(), name: v.get("name"), description: v.opt("description") }),
        )
    }));
    let (i, size) = (v.id.clone(), v.size_gb);
    items.push(MenuItem::dialog(tr(K::ExtendVolume), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::ExtendVolume),
            t(lang, K::ExtendVolume),
            vec![Field::number("size", t(lang, K::NewSizeGb), size + 1).value((size + 1).to_string()).required()],
            move |v| {
                let new = num_u32(v, "size", K::NewSizeGb, lang)?.unwrap_or(0);
                if (new as i64) <= size {
                    return Err(format!("{} > {size}", t(lang, K::NewSizeGb)));
                }
                Ok(OsAction::VolumeExtend { id: i.clone(), new_size_gb: new })
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
                Field::text("name", t(lang, K::SnapshotName)).value(format!("{n}-snapshot")).required(),
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
        items.push(MenuItem::dialog(tr(K::CreateBackup), move || backup_form(lang, Some(i.clone()), format!("{n}-backup"), attached)));
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
                    Ok(OsAction::VolumeRetype { id: i.clone(), new_type, migrate: v.flag("migrate") })
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
            let first = if formats.contains(&"raw") { "raw" } else { formats.first().copied().unwrap_or("raw") };
            let mut fields = vec![
                Field::text("name", t(lang, K::ImageName)).value(n.clone()).required(),
                Field::select("format", t(lang, K::DiskFormat), options_raw(&formats)).value(first).required(),
                Field::select("visibility", t(lang, K::Visibility), options_raw(&crate::prefs::VISIBILITIES))
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
                vec![Field::text("name", t(lang, K::TransferName)).value(n.clone()).help(t(lang, K::TransferHelp))],
                move |v| Ok(OsAction::VolumeTransferCreate { volume_id: i.clone(), name: v.opt("name") }),
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
                move |v| Ok(OsAction::ServerAttachVolume { server_id: req(v, "server", K::Instance, lang)?, volume_id: i.clone() }),
            )
        }));
    }
    for a in &v.attachments {
        items.push(detach_volume_item(lang, &a.server_id, &v.id, &format!("{} → {}", v.name, a.server_name.as_deref().unwrap_or(&a.server_id))));
    }
    let i = v.id.clone();
    items.push(metadata_item(lang, "OS::Cinder::Volume", &v.metadata, move |metadata, _| OsAction::VolumeSetMetadata { id: i.clone(), metadata }));
    let (i, n) = (v.id.clone(), v.name.clone());
    items.push(MenuItem::danger(tr(K::DeleteVolume), move || {
        Dialog::confirm(lang, t(lang, K::DeleteVolume), n.clone(), OsAction::VolumeDelete { id: i.clone() })
    }));
    items
}

/// Volumes header: take ownership of a volume another project transferred.
pub fn accept_transfer_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::AcceptTransfer),
        t(lang, K::AcceptTransfer),
        vec![Field::text("id", t(lang, K::TransferId)).required(), Field::text("key", t(lang, K::AuthKey)).required()],
        move |v| Ok(OsAction::VolumeTransferAccept { transfer_id: req(v, "id", K::TransferId, lang)?, auth_key: req(v, "key", K::AuthKey, lang)? }),
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
                Field::select("volume", tr(K::RestoreToVolume), Source::Set(ChoiceSet::AvailableVolumes)).help(tr(K::RestoreToVolumeHelp)),
                Field::text("name", tr(K::RestoredVolumeName)).value(format!("restore_backup_{n}")),
            ];
            Dialog::form(tr(K::RestoreBackup), tr(K::RestoreBackup), fields, move |v| {
                Ok(OsAction::BackupRestore { id: i.clone(), volume_id: v.opt("volume"), name: v.opt("name") })
            })
        }));
    }
    let (i, n, d) = (b.id.clone(), b.name.clone(), b.description.clone().unwrap_or_default());
    items.push(MenuItem::dialog(tr(K::EditBackup), move || {
        let i = i.clone();
        let fields = vec![Field::text("name", tr(K::Name)).value(n.clone()).required(), Field::area("description", tr(K::Description)).value(d.clone())];
        Dialog::form(tr(K::EditBackup), tr(K::Save), fields, move |v| {
            Ok(OsAction::BackupUpdate { id: i.clone(), name: req(v, "name", K::Name, lang)?, description: v.opt("description") })
        })
    }));
    let (i, n) = (b.id.clone(), b.name.clone());
    items.push(MenuItem::danger(tr(K::DeleteBackup), move || Dialog::confirm(lang, tr(K::DeleteBackup), n.clone(), OsAction::BackupDelete { id: i.clone() })));
    items
}

/// A volume snapshot's menu: launch, edit, delete.
pub fn snapshot_items(lang: Lang, s: &Snapshot) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if s.status.eq_ignore_ascii_case("available") {
        let i = s.id.clone();
        items.push(MenuItem::dialog(t(lang, K::LaunchAsInstance), move || launch_from(lang, "volume_snapshot", &i)));
    }
    items.push(create_volume_item(lang, VolumeSource::Snapshot, &s.id, Some(s.size_gb.max(1) as u32)));
    let (i, n, d) = (s.id.clone(), s.name.clone(), s.description.clone().unwrap_or_default());
    items.push(MenuItem::dialog(t(lang, K::EditSnapshot), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditSnapshot),
            t(lang, K::Save),
            vec![Field::text("name", t(lang, K::Name)).value(n.clone()), Field::area("description", t(lang, K::Description)).value(d.clone())],
            move |v| Ok(OsAction::SnapshotUpdate { id: i.clone(), name: v.get("name"), description: v.opt("description") }),
        )
    }));
    let i = s.id.clone();
    items.push(metadata_item(lang, "OS::Cinder::Snapshot", &s.metadata, move |metadata, _| OsAction::SnapshotSetMetadata { id: i.clone(), metadata }));
    let (i, n) = (s.id.clone(), s.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteSnapshot), move || {
        Dialog::confirm(lang, t(lang, K::DeleteSnapshot), n.clone(), OsAction::SnapshotDelete { id: i.clone() })
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
        Field::select("az", tr(K::AvailabilityZone), Source::Set(ChoiceSet::VolumeZones)).empty_label(tr(K::AnyAvailabilityZone)),
        Field::multi("volume_types", tr(K::VolumeTypes), Source::Set(ChoiceSet::VolumeTypeIds)).required().help(tr(K::VolumeGroupTypesHelp)),
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
fn create_group_from(lang: Lang, title: K, help: K, name: String, source_group_id: Option<String>, group_snapshot_id: Option<String>) -> Dialog {
    let fields = vec![Field::text("name", t(lang, K::Name)).value(name).required().help(t(lang, help)), Field::area("description", t(lang, K::Description))];
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
        let fields = vec![Field::text("name", tr(K::SnapshotName)).value(format!("{n}-snapshot")).required(), Field::area("description", tr(K::Description))];
        Dialog::form(tr(K::CreateSnapshot), tr(K::CreateSnapshot), fields, move |v| {
            Ok(OsAction::GroupSnapshotCreate { group_id: i.clone(), name: req(v, "name", K::SnapshotName, lang)?, description: v.opt("description") })
        })
    });
    let (i, n, d) = (g.id.clone(), g.name.clone(), g.description.clone().unwrap_or_default());
    let edit = MenuItem::dialog(tr(K::EditGroup), move || {
        let i = i.clone();
        let fields = vec![Field::text("name", tr(K::Name)).value(n.clone()).required(), Field::area("description", tr(K::Description)).value(d.clone())];
        Dialog::form(tr(K::EditGroup), tr(K::Save), fields, move |v| {
            Ok(OsAction::VolumeGroupUpdate { id: i.clone(), name: req(v, "name", K::Name, lang)?, description: v.opt("description") })
        })
    });
    let (i, n) = (g.id.clone(), g.name.clone());
    let clone = MenuItem::dialog(tr(K::CloneGroup), move || create_group_from(lang, K::CloneGroup, K::CloneGroupHelp, format!("{n}-clone"), Some(i.clone()), None));
    let (i, n) = (g.id.clone(), g.name.clone());
    let delete = MenuItem::danger(tr(K::DeleteGroup), move || {
        let i = i.clone();
        let mut d = Dialog::confirm(lang, tr(K::DeleteGroup), n.clone(), OsAction::VolumeGroupDelete { id: i.clone(), delete_volumes: false });
        d.fields = vec![Field::check("delete_volumes", tr(K::DeleteGroupVolumes), false).help(tr(K::DeleteGroupVolumesHelp))];
        d.build = std::sync::Arc::new(move |v| Ok(OsAction::VolumeGroupDelete { id: i.clone(), delete_volumes: v.flag("delete_volumes") }));
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
        let field = Field::multi("volumes", tr(K::Volumes), Source::Of(ChoiceSet::GroupVolumes, gid.clone())).value(before.join(",")).help(tr(K::ManageVolumesHelp));
        Dialog::form(tr(K::ManageVolumes), tr(K::Save), vec![field], move |v| {
            let (add_volume_ids, remove_volume_ids) = list_diff(&before, &v.list("volumes"));
            if add_volume_ids.is_empty() && remove_volume_ids.is_empty() {
                return Err(tr(K::NothingChanged));
            }
            Ok(OsAction::VolumeGroupSetVolumes { id: gid.clone(), add_volume_ids, remove_volume_ids })
        })
    });
    items.insert(1, manage);
    if !d.volumes.is_empty() {
        let (gid, ids, names): (String, Vec<String>, Vec<String>) =
            (d.row.id.clone(), d.volumes.iter().map(|v| v.id.clone()).collect(), d.volumes.iter().map(|v| v.name.clone()).collect());
        let remove = MenuItem::dialog(tr(K::RemoveGroupVolumes), move || {
            let action = OsAction::VolumeGroupSetVolumes { id: gid.clone(), add_volume_ids: vec![], remove_volume_ids: ids.clone() };
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
        create_group_from(lang, K::CreateGroup, K::CreateGroupFromSnapshotHelp, n.clone(), None, Some(i.clone()))
    });
    let (i, n) = (s.id.clone(), s.name.clone());
    vec![
        create,
        MenuItem::danger(t(lang, K::DeleteGroupSnapshot), move || {
            Dialog::confirm(lang, t(lang, K::DeleteGroupSnapshot), n.clone(), OsAction::GroupSnapshotDelete { id: i.clone() })
        }),
    ]
}

// ---------------------------------------------------------------------------
// Networks, subnets, ports
// ---------------------------------------------------------------------------

/// Subnet fields, prefilled from `[network]` (DNS servers, IPv6 on or off).
fn subnet_fields(lang: Lang, when: Option<(&'static str, &'static str)>, net: &crate::prefs::NetworkPrefs) -> Vec<Field> {
    let tr = |k| t(lang, k).to_string();
    let versions: &[&str] = if net.enable_ipv6 { &["4", "6"] } else { &["4"] };
    let w = |f: Field| match when {
        Some((a, b)) => f.when(a, b),
        None => f,
    };
    vec![
        w(Field::text("subnet_name", tr(K::SubnetName))),
        w(Field::text("cidr", tr(K::NetworkAddress)).help("192.168.0.0/24").required()),
        w(Field::select("ip_version", tr(K::IpVersion), options_raw(versions)).value("4").required()),
        w(Field::text("gateway", tr(K::GatewayIp)).help(tr(K::GatewayAuto))),
        w(Field::check("no_gateway", tr(K::DisableGateway), false)),
        w(Field::check("dhcp", tr(K::EnableDhcp), true)),
        // Horizon shows the mode only for IPv6 subnets.
        w(Field::select("ipv6_modes", tr(K::Ipv6Modes), ipv6_mode_options(lang)).value(IPV6_MODES[0]).show_if(|v| v.get("ip_version").map(String::as_str) == Some("6"))),
        w(Field::area("pools", tr(K::AllocationPools)).help(tr(K::PoolsHelp))),
        w(Field::area("dns", tr(K::DnsNameServers)).help(tr(K::OnePerLine)).value(net.default_dns_nameservers.join("\n"))),
        w(Field::area("host_routes", tr(K::HostRoutes)).help(tr(K::HostRoutesHelp))),
    ]
}

/// Horizon's five *IPv6 Address Configuration Mode* choices, translated.
fn ipv6_mode_options(lang: Lang) -> Source {
    let keys = [K::Ipv6ModeNone, K::Ipv6ModeSlaacRouter, K::Ipv6ModeSlaacExternal, K::Ipv6ModeStateful, K::Ipv6ModeStateless];
    Source::Static(IPV6_MODES.iter().zip(keys).map(|(v, k)| (v.to_string(), t(lang, k).to_string())).collect())
}

/// `[network] allowed_private_subnet_cidr` / `enable_ipv6`, before submitting
/// (the server checks again). Skipped when the network dialog creates no subnet.
fn subnet_check(lang: Lang, net: crate::prefs::NetworkPrefs) -> impl Fn(&Vals, &Sets) -> Vec<(&'static str, String)> + Send + Sync + 'static {
    move |v, _| {
        let cidr = v.get("cidr").map(|c| c.trim()).unwrap_or_default();
        if v.get("with_subnet").map(String::as_str) == Some("false") || cidr.is_empty() {
            return vec![];
        }
        match net.check_subnet(cidr) {
            Ok(()) => vec![],
            Err(issue) => vec![("cidr", crate::i18n::subnet_issue(lang, &issue))],
        }
    }
}

fn subnet_spec(v: &Values, lang: Lang) -> Result<SubnetSpec, String> {
    let pools = v
        .lines("pools")
        .into_iter()
        .map(|l| match l.split_once(',') {
            Some((a, z)) => Ok((a.trim().to_string(), z.trim().to_string())),
            None => Err(format!("{}: \"{l}\"", t(lang, K::AllocationPools))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let ip_version = if v.get("ip_version") == "6" { 6 } else { 4 };
    let (ipv6_ra_mode, ipv6_address_mode) = if ip_version == 6 { ipv6_modes(&v.opt("ipv6_modes").unwrap_or_else(|| IPV6_MODES[0].into()))? } else { (None, None) };
    let host_routes = v
        .lines("host_routes")
        .into_iter()
        .map(|l| match l.split_once(',') {
            Some((dest, hop)) => validate_route(dest, hop).map_err(|e| format!("{}: {e}", t(lang, K::HostRoutes))),
            None => Err(format!("{}: \"{l}\"", t(lang, K::HostRoutes))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SubnetSpec {
        name: v.get("subnet_name"),
        cidr: req(v, "cidr", K::NetworkAddress, lang)?,
        ip_version,
        gateway_ip: if v.flag("no_gateway") { Some(String::new()) } else { v.opt("gateway") },
        enable_dhcp: v.flag("dhcp"),
        dns_nameservers: v.lines("dns"),
        allocation_pools: pools,
        ipv6_ra_mode,
        ipv6_address_mode,
        host_routes,
    })
}

/// *Create Network*: the network, then an optional subnet (wizard).
pub fn create_network_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let mut fields = vec![
        Field::text("name", tr(K::NetworkName)),
        Field::check("admin_up", tr(K::EnableAdminState), true),
        Field::check("with_subnet", tr(K::WithSubnet), true),
    ];
    let net = crate::components::dialog::prefs().network;
    fields.extend(subnet_fields(lang, Some(("with_subnet", "true")), &net));
    let with_subnet: fn(&Vals) -> bool = |v| v.get("with_subnet").map(String::as_str) == Some("true");
    Dialog::form(tr(K::CreateNetwork), tr(K::CreateNetwork), fields, move |v| {
        Ok(OsAction::NetworkCreate {
            name: v.get("name"),
            admin_up: v.flag("admin_up"),
            subnet: if v.flag("with_subnet") { Some(subnet_spec(v, lang)?) } else { None },
        })
    })
    // Horizon's three steps; the subnet ones switch off without a subnet.
    .wizard(vec![
        Step::new(tr(K::Network), &["name", "admin_up", "with_subnet"]).help(tr(K::NetworkStepHelp)),
        Step::new(tr(K::Subnet), &["subnet_name", "cidr", "ip_version", "gateway", "no_gateway"])
            .help(tr(K::SubnetStepHelp))
            .enabled_if(with_subnet),
        Step::new(tr(K::StepSubnetDetails), &["dhcp", "ipv6_modes", "pools", "dns", "host_routes"]).help(tr(K::SubnetDetailsHelp)).enabled_if(with_subnet),
    ])
    .check(subnet_check(lang, net))
}

/// Admin › Networks' *Create Network* (roadmap 85): Horizon's admin form (the
/// owner project, the provider attributes, *Shared*, *External*), then the same
/// optional subnet steps as Project's.
pub fn admin_create_network_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    // `[network] supported_provider_types` / `segmentation_id_range` (Horizon's).
    let provider_types = crate::components::dialog::prefs().network.provider_types();
    let types: Vec<&str> = provider_types.iter().map(|(t, _)| t.as_str()).collect();
    // Horizon: a physical network for flat and VLAN, a segmentation ID for the
    // tunnelled types and VLAN.
    let physical: fn(&Vals) -> bool = |v| matches!(v.get("provider_type").map(String::as_str), Some("flat" | "vlan"));
    let segmented: fn(&Vals) -> bool = |v| matches!(v.get("provider_type").map(String::as_str), Some("vlan" | "gre" | "vxlan" | "geneve"));
    let mut fields = vec![
        Field::text("name", tr(K::NetworkName)),
        Field::select("project_id", tr(K::Project), Source::Set(ChoiceSet::AllProjects)).required().empty_label(tr(K::SelectProject)),
        // Horizon preselects the first type, *Local*.
        Field::select("provider_type", tr(K::ProviderNetworkType), options_raw(&types)).value(types[0]).help(tr(K::ProviderNetworkTypeHelp)).required(),
        Field::text("physical_network", tr(K::PhysicalNetwork)).help("physnet1").show_if(physical).required(),
        Field::number("segmentation_id", tr(K::SegmentationId), 1).help(tr(K::SegmentationIdHelp)).show_if(segmented),
        Field::check("admin_up", tr(K::EnableAdminState), true),
        Field::check("shared", tr(K::Shared), false),
        Field::check("external", tr(K::ExternalNetwork), false),
        Field::check("with_subnet", tr(K::WithSubnet), true),
    ];
    // Horizon's *Availability Zone Hints*, only where Neutron schedules by zone.
    let zones = needs::NETWORK_AZ.met(&crate::components::dialog::caps());
    if zones {
        fields.push(Field::multi("az_hints", tr(K::AzHints), Source::Set(ChoiceSet::NetworkZones)).help(tr(K::AzHintsHelp)).hide_when_empty());
    }
    fields.push(Field::number("mtu", tr(K::Mtu), 68).help(tr(K::MtuHelp)));
    let net = crate::components::dialog::prefs().network;
    fields.extend(subnet_fields(lang, Some(("with_subnet", "true")), &net));
    let with_subnet: fn(&Vals) -> bool = |v| v.get("with_subnet").map(String::as_str) == Some("true");
    Dialog::form(tr(K::CreateNetwork), tr(K::CreateNetwork), fields, move |v| {
        let mtu = num_u32(v, "mtu", K::Mtu, lang)?;
        let segmentation_id = num_u32(v, "segmentation_id", K::SegmentationId, lang)?;
        let provider = ProviderNetwork { network_type: req(v, "provider_type", K::ProviderNetworkType, lang)?, physical_network: v.opt("physical_network"), segmentation_id };
        provider.validate(&provider_types)?;
        Ok(OsAction::AdminNetworkCreate(Box::new(AdminNetworkSpec {
            name: v.get("name"),
            project_id: req(v, "project_id", K::Project, lang)?,
            provider: Some(provider),
            admin_up: v.flag("admin_up"),
            shared: v.flag("shared"),
            external: v.flag("external"),
            mtu,
            az_hints: v.list("az_hints"),
            subnet: if v.flag("with_subnet") { Some(subnet_spec(v, lang)?) } else { None },
        })))
    })
    .wizard(vec![
        // Horizon's order: the subnet checkbox, then the zone hints and the MTU.
        Step::new(tr(K::Network), &["name", "project_id", "provider_type", "physical_network", "segmentation_id", "admin_up", "shared", "external", "with_subnet", "az_hints", "mtu"])
            .help(tr(K::AdminNetworkStepHelp)),
        Step::new(tr(K::Subnet), &["subnet_name", "cidr", "ip_version", "gateway", "no_gateway"])
            .help(tr(K::SubnetStepHelp))
            .enabled_if(with_subnet),
        Step::new(tr(K::StepSubnetDetails), &["dhcp", "ipv6_modes", "pools", "dns", "host_routes"]).help(tr(K::SubnetDetailsHelp)).enabled_if(with_subnet),
    ])
    .check(subnet_check(lang, net))
}

/// Admin › RBAC Policies' *Create RBAC Policy* (roadmap 85), Horizon's form:
/// *Target Project* (`*` = every project), *Action and Object Type*, then the
/// network or the QoS policy it shares. *Shared QoS Policy* needs Neutron's `qos`.
pub fn create_rbac_policy_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let mut kinds = vec![("shared_network", K::SharedNetwork), ("external_network", K::ExternalNetwork)];
    if Requires::NetExt("qos").met(&crate::components::dialog::caps()) {
        kinds.push(("shared_qos_policy", K::SharedQosPolicy));
    }
    let network: fn(&Vals) -> bool = |v| matches!(v.get("kind").map(String::as_str), Some("shared_network" | "external_network"));
    let fields = vec![
        Field::select("target_project", tr(K::TargetProject), Source::Set(ChoiceSet::RbacTargets)).help(tr(K::RbacTargetHelp)).required().empty_label(tr(K::SelectChoice)),
        Field::select("kind", tr(K::ActionObjectType), options(&kinds, lang)).required().empty_label(tr(K::SelectChoice)),
        Field::select("network_id", tr(K::Network), Source::Set(ChoiceSet::AllNetworks)).show_if(network).required().empty_label(tr(K::SelectChoice)),
        Field::select("qos_policy_id", tr(K::QosPolicy), Source::Set(ChoiceSet::AllQosPolicies)).when("kind", "shared_qos_policy").required().empty_label(tr(K::SelectChoice)),
    ];
    Dialog::form(tr(K::CreateRbacPolicy), tr(K::CreateRbacPolicy), fields, move |v| {
        let kind = req(v, "kind", K::ActionObjectType, lang)?;
        let (object_type, _) = rbac_kind(&kind)?;
        let object_id = if object_type == "qos_policy" { req(v, "qos_policy_id", K::QosPolicy, lang)? } else { req(v, "network_id", K::Network, lang)? };
        Ok(OsAction::RbacPolicyCreate { kind, object_id, target_project: req(v, "target_project", K::TargetProject, lang)? })
    })
}

/// Horizon's *Update RBAC Policy*: only the target project changes (`*` = every
/// project), preselected with the current one.
pub fn edit_rbac_policy_dialog(lang: Lang, r: &RbacPolicyRow) -> Dialog {
    let id = r.id.clone();
    let target = Field::select("target_project", t(lang, K::TargetProject), Source::Set(ChoiceSet::RbacTargets))
        .value(r.target_id.clone())
        .help(t(lang, K::RbacTargetHelp))
        .required();
    Dialog::form(t(lang, K::UpdateRbacPolicy), t(lang, K::SaveChanges), vec![target], move |v| {
        Ok(OsAction::RbacPolicyUpdate { id: id.clone(), target_project: req(v, "target_project", K::TargetProject, lang)? })
    })
}

/// An RBAC policy's row menu, as Horizon's: *Edit Policy*, *Delete RBAC Policy*.
pub fn rbac_policy_items(lang: Lang, r: &RbacPolicyRow) -> Vec<MenuItem> {
    let row = r.clone();
    let (id, label) = (r.id.clone(), format!("{} {} → {}", r.object_type, r.object.name, r.target));
    vec![
        MenuItem::dialog(t(lang, K::EditPolicy), move || edit_rbac_policy_dialog(lang, &row)),
        MenuItem::danger(t(lang, K::DeleteRbacPolicy), move || {
            Dialog::confirm(lang, t(lang, K::DeleteRbacPolicy), label.clone(), OsAction::RbacPolicyDelete { id: id.clone() })
        }),
    ]
}

/// *Create Subnet* on `network_id`.
pub fn create_subnet_dialog(lang: Lang, network_id: &str) -> Dialog {
    let n = network_id.to_string();
    let net = crate::components::dialog::prefs().network;
    Dialog::form(t(lang, K::CreateSubnet), t(lang, K::CreateSubnet), subnet_fields(lang, None, &net), move |v| {
        Ok(OsAction::SubnetCreate { network_id: n.clone(), spec: subnet_spec(v, lang)? })
    })
    .check(subnet_check(lang, net))
}

/// Nothing for other projects' (external, shared) networks, like Horizon.
pub fn network_items(lang: Lang, net: &NetworkRow) -> Vec<MenuItem> {
    if !net.owned {
        return Vec::new();
    }
    network_menu(lang, net, false)
}

/// Admin › Networks' row menu (roadmap 85): every project's network, as
/// Horizon's admin table: *Edit Network*, *Create Subnet*, *Delete Network*.
pub fn admin_network_items(lang: Lang, net: &NetworkRow) -> Vec<MenuItem> {
    network_menu(lang, net, true)
}

/// Horizon's admin *Edit Network*: name, admin state, *Shared*, *External
/// Network*. The provider attributes are shown above the fields, read-only:
/// Horizon doesn't edit them either, and ML2 refuses to change them.
pub fn admin_edit_network_dialog(lang: Lang, net: &NetworkRow) -> Dialog {
    let id = net.id.clone();
    let mut d = Dialog::form(
        t(lang, K::EditNetwork),
        t(lang, K::Save),
        vec![
            Field::text("name", t(lang, K::NetworkName)).value(net.name.clone()),
            Field::check("admin_up", t(lang, K::EnableAdminState), net.admin_up),
            Field::check("shared", t(lang, K::Shared), net.shared),
            Field::check("external", t(lang, K::ExternalNetwork), net.external),
        ],
        move |v| Ok(OsAction::AdminNetworkUpdate { id: id.clone(), name: v.get("name"), admin_up: v.flag("admin_up"), shared: v.flag("shared"), external: v.flag("external") }),
    );
    if let Some(p) = &net.provider {
        d.intro = vec![crate::i18n::provider_summary(lang, p)];
    }
    d
}

/// A network's menu. `admin`: Horizon's admin edit and no *Create Port*
/// (Admin › Networks); else Project's edit and *Create Port*.
fn network_menu(lang: Lang, net: &NetworkRow, admin: bool) -> Vec<MenuItem> {
    let (id, name, admin_up) = (net.id.as_str(), net.name.as_str(), net.admin_up);
    let (i, n) = (id.to_string(), name.to_string());
    let row = net.clone();
    let mut items = vec![MenuItem::dialog(t(lang, K::EditNetwork), move || {
        if admin {
            return admin_edit_network_dialog(lang, &row);
        }
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditNetwork),
            t(lang, K::Save),
            vec![Field::text("name", t(lang, K::NetworkName)).value(n.clone()), Field::check("admin_up", t(lang, K::EnableAdminState), admin_up)],
            move |v| Ok(OsAction::NetworkUpdate { id: i.clone(), name: v.get("name"), admin_up: v.flag("admin_up") }),
        )
    })];
    let i = id.to_string();
    items.push(MenuItem::dialog(t(lang, K::CreateSubnet), move || create_subnet_dialog(lang, &i)));
    if !admin {
        let i = id.to_string();
        items.push(MenuItem::dialog(t(lang, K::CreatePort), move || create_port_dialog(lang, &i)));
    }
    let (i, n) = (id.to_string(), name.to_string());
    items.push(MenuItem::danger(t(lang, K::DeleteNetwork), move || {
        Dialog::confirm(lang, t(lang, K::DeleteNetwork), n.clone(), OsAction::NetworkDelete { id: i.clone() })
    }));
    items
}

/// A subnet's menu (edit, delete).
pub fn subnet_items(lang: Lang, s: &SubnetRow) -> Vec<MenuItem> {
    let (i, n, dhcp, dns) = (s.id.clone(), s.name.clone(), s.enable_dhcp, s.dns_nameservers.join("\n"));
    let mut items = vec![MenuItem::dialog(t(lang, K::EditSubnet), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditSubnet),
            t(lang, K::Save),
            vec![
                Field::text("name", t(lang, K::SubnetName)).value(n.clone()),
                Field::check("dhcp", t(lang, K::EnableDhcp), dhcp),
                Field::area("dns", t(lang, K::DnsNameServers)).value(dns.clone()).help(t(lang, K::OnePerLine)),
            ],
            move |v| Ok(OsAction::SubnetUpdate { id: i.clone(), name: v.get("name"), enable_dhcp: v.flag("dhcp"), dns_nameservers: v.lines("dns") }),
        )
    })];
    let (i, n) = (s.id.clone(), s.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteSubnet), move || {
        Dialog::confirm(lang, t(lang, K::DeleteSubnet), n.clone(), OsAction::SubnetDelete { id: i.clone() })
    }));
    items
}

/// *Create Port* on `network_id`.
pub fn create_port_dialog(lang: Lang, network_id: &str) -> Dialog {
    let n = network_id.to_string();
    Dialog::form(
        t(lang, K::CreatePort),
        t(lang, K::CreatePort),
        vec![
            Field::text("name", t(lang, K::Name)),
            Field::check("admin_up", t(lang, K::EnableAdminState), true),
            Field::text("fixed_ip", t(lang, K::FixedIpOptional)),
        ],
        move |v| Ok(OsAction::PortCreate { network_id: n.clone(), name: v.get("name"), admin_up: v.flag("admin_up"), fixed_ip: v.opt("fixed_ip") }),
    )
}

/// A port's menu (edit, delete).
pub fn port_items(lang: Lang, p: &PortRow) -> Vec<MenuItem> {
    let (i, settings) = (p.id.clone(), p.settings());
    let mut items = vec![MenuItem::dialog(t(lang, K::EditPort), move || edit_port_dialog(lang, &i, &settings, false))];
    let (i, label) = (p.id.clone(), if p.name.is_empty() { p.id.clone() } else { p.name.clone() });
    items.push(MenuItem::danger(t(lang, K::DeletePort), move || {
        Dialog::confirm(lang, t(lang, K::DeletePort), label.clone(), OsAction::PortDelete { id: i.clone() })
    }));
    items
}

// ---------------------------------------------------------------------------
// Routers
// ---------------------------------------------------------------------------

/// Optional *QoS Policy* select (the project's and shared policies).
fn qos_field(lang: Lang) -> Field {
    Field::select("qos", t(lang, K::QosPolicy), Source::Set(ChoiceSet::QosPolicies))
}

/// *Create Router*: name, external network, SNAT (roadmap 30).
pub fn create_router_dialog(lang: Lang) -> Dialog {
    let mut fields = vec![
        Field::text("name", t(lang, K::Name)),
        Field::check("admin_up", t(lang, K::EnableAdminState), true),
        Field::select("external", t(lang, K::ExternalNetwork), Source::Set(ChoiceSet::ExternalNetworks)),
    ];
    // The gateway's policy (`qos` plugin and `qos-gateway-ip`); only with a gateway.
    let gateway: fn(&Vals) -> bool = |v| v.get("external").is_some_and(|e| !e.trim().is_empty());
    if needs::QOS_GATEWAY.met(&caps()) {
        fields.push(qos_field(lang).help(t(lang, K::GatewayQosHelp)).show_if(gateway));
    }
    Dialog::form(t(lang, K::CreateRouter), t(lang, K::CreateRouter), fields, move |v| {
        let external = v.opt("external");
        Ok(OsAction::RouterCreate {
            name: v.get("name"),
            admin_up: v.flag("admin_up"),
            qos_policy_id: v.opt("qos").filter(|_| external.is_some()),
            external_network_id: external,
        })
    })
}

/// *Add Interface* to a router: a subnet, with an optional IP address.
pub fn add_interface_dialog(lang: Lang, router_id: &str) -> Dialog {
    let r = router_id.to_string();
    Dialog::form(
        t(lang, K::AddInterface),
        t(lang, K::AddInterface),
        vec![Field::select("subnet", t(lang, K::Subnet), Source::Set(ChoiceSet::Subnets)).required()],
        move |v| Ok(OsAction::RouterAddInterface { id: r.clone(), subnet_id: req(v, "subnet", K::Subnet, lang)? }),
    )
}

/// A router's menu: set or clear the gateway, edit, add interface, delete.
pub fn router_items(lang: Lang, r: &RouterRow) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if r.external_network.is_some() {
        let (i, n) = (r.id.clone(), r.name.clone());
        items.push(MenuItem::danger(t(lang, K::ClearGateway), move || {
            Dialog::confirm(lang, t(lang, K::ClearGateway), n.clone(), OsAction::RouterClearGateway { id: i.clone() })
        }));
    } else {
        let i = r.id.clone();
        items.push(MenuItem::dialog(t(lang, K::SetGateway), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::SetGateway),
                t(lang, K::SetGateway),
                vec![Field::select("external", t(lang, K::ExternalNetwork), Source::Set(ChoiceSet::ExternalNetworks)).required()],
                move |v| Ok(OsAction::RouterSetGateway { id: i.clone(), network_id: req(v, "external", K::ExternalNetwork, lang)? }),
            )
        }));
    }
    items.push(edit_router_item(lang, r));
    let i = r.id.clone();
    items.push(MenuItem::dialog(t(lang, K::AddInterface), move || add_interface_dialog(lang, &i)));
    items.push(delete_router_item(lang, r));
    items
}

/// *Edit Router*: name, admin state and, with a gateway, SNAT.
fn edit_router_item(lang: Lang, r: &RouterRow) -> MenuItem {
    let (i, n, up) = (r.id.clone(), r.name.clone(), r.admin_up);
    // SNAT is part of the gateway: only with one, and when the token's roles
    // may change it.
    let gw = r.external_network_id.clone().filter(|_| needs::SNAT.met(&caps()));
    let snat = r.snat.unwrap_or(true);
    MenuItem::dialog(t(lang, K::EditRouter), move || {
        let (i, gw) = (i.clone(), gw.clone());
        let mut fields =
            vec![Field::text("name", t(lang, K::Name)).value(n.clone()), Field::check("admin_up", t(lang, K::EnableAdminState), up)];
        if gw.is_some() {
            fields.push(Field::check("snat", t(lang, K::EnableSnat), snat));
        }
        Dialog::form(t(lang, K::EditRouter), t(lang, K::Save), fields, move |v| {
            // Only a change goes in the request, so a rename leaves the gateway alone.
            let snat = gw.clone().map(|net| (net, v.flag("snat"))).filter(|(_, on)| *on != snat);
            Ok(OsAction::RouterUpdate { id: i.clone(), name: v.get("name"), admin_up: v.flag("admin_up"), snat })
        })
    })
}

fn delete_router_item(lang: Lang, r: &RouterRow) -> MenuItem {
    let (i, n) = (r.id.clone(), r.name.clone());
    MenuItem::danger(t(lang, K::DeleteRouter), move || {
        Dialog::confirm(lang, t(lang, K::DeleteRouter), n.clone(), OsAction::RouterDelete { id: i.clone() })
    })
}

/// Admin › Routers' row menu, Horizon's: *Edit Router* and *Delete Router*. No
/// gateway or interface items: their subnets would be the session project's.
pub fn admin_router_items(lang: Lang, r: &RouterRow) -> Vec<MenuItem> {
    vec![edit_router_item(lang, r), delete_router_item(lang, r)]
}

/// Admin › Routers' *Create Router* (roadmap 85), Horizon's admin form: the
/// owner project, then Project's fields, SNAT with a gateway, and *Router Type*,
/// *High Availability Mode* and *Availability Zone Hints* where Neutron has
/// them. *Use Server Default* leaves a field out of the request.
pub fn admin_create_router_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let caps = caps();
    let gateway: fn(&Vals) -> bool = |v| v.get("external").is_some_and(|e| !e.trim().is_empty());
    let mut fields = vec![
        Field::text("name", tr(K::Name)),
        Field::select("project_id", tr(K::Project), Source::Set(ChoiceSet::AllProjects)).required().empty_label(tr(K::SelectProject)),
        Field::check("admin_up", tr(K::EnableAdminState), true),
        Field::select("external", tr(K::ExternalNetwork), Source::Set(ChoiceSet::ExternalNetworks)),
    ];
    if needs::SNAT.met(&caps) {
        // Horizon's default: SNAT on.
        fields.push(Field::check("snat", tr(K::EnableSnat), true).show_if(gateway));
    }
    let tri = |on: K, off: K| options(&[("default", K::UseServerDefault), ("on", on), ("off", off)], lang);
    if needs::DVR.met(&caps) {
        fields.push(Field::select("mode", tr(K::RouterType), tri(K::Distributed, K::Centralized)).value("default"));
    }
    if needs::L3_HA.met(&caps) {
        fields.push(Field::select("ha", tr(K::HaMode), tri(K::HaEnable, K::HaDisable)).value("default"));
    }
    if needs::ROUTER_AZ.met(&caps) {
        fields.push(Field::multi("az_hints", tr(K::AzHints), Source::Set(ChoiceSet::RouterZones)).help(tr(K::RouterAzHintsHelp)).hide_when_empty());
    }
    // `default` (or a hidden field) sends nothing.
    let tri_value = |v: &Values, k: &str| match v.get(k).as_str() {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    };
    Dialog::form(tr(K::CreateRouter), tr(K::CreateRouter), fields, move |v| {
        let external = v.opt("external");
        Ok(OsAction::AdminRouterCreate(Box::new(AdminRouterSpec {
            name: v.get("name"),
            project_id: req(v, "project_id", K::Project, lang)?,
            admin_up: v.flag("admin_up"),
            // A hidden *Enable SNAT* (no gateway, or no `ext-gw-mode`) is empty: not sent.
            enable_snat: v.opt("snat").filter(|_| external.is_some()).map(|s| s == "true"),
            external_network_id: external,
            az_hints: v.list("az_hints"),
            distributed: tri_value(v, "mode"),
            ha: tri_value(v, "ha"),
        })))
    })
}

/// `current`: the router's routes when only plain `extraroute` exists, so the
/// new route is sent with the whole list; `None` uses the atomic call.
pub fn add_route_dialog(lang: Lang, router_id: &str, current: Option<Vec<(String, String)>>) -> Dialog {
    let r = router_id.to_string();
    Dialog::form(
        t(lang, K::AddStaticRoute),
        t(lang, K::AddStaticRoute),
        vec![
            Field::text("destination", t(lang, K::Destination)).required().help("10.9.0.0/16"),
            Field::text("nexthop", t(lang, K::NextHop)).required().help(t(lang, K::NextHopHelp)),
        ],
        move |v| {
            let (destination, nexthop) =
                validate_route(&req(v, "destination", K::Destination, lang)?, &req(v, "nexthop", K::NextHop, lang)?)?;
            Ok(match &current {
                None => OsAction::RouterAddRoute { id: r.clone(), destination, nexthop },
                Some(list) => {
                    let mut routes = list.clone();
                    if !routes.contains(&(destination.clone(), nexthop.clone())) {
                        routes.push((destination, nexthop));
                    }
                    OsAction::RouterSetRoutes { id: r.clone(), routes }
                }
            })
        },
    )
}

/// `current` as in [`add_route_dialog`].
pub fn route_items(lang: Lang, router_id: &str, route: &(String, String), current: Option<&[(String, String)]>) -> Vec<MenuItem> {
    let (r, (dest, hop)) = (router_id.to_string(), route.clone());
    let action = match current {
        None => OsAction::RouterRemoveRoute { id: r, destination: dest.clone(), nexthop: hop.clone() },
        Some(list) => OsAction::RouterSetRoutes { id: r, routes: list.iter().filter(|x| **x != route.clone()).cloned().collect() },
    };
    vec![MenuItem::danger(t(lang, K::DeleteStaticRoute), move || {
        Dialog::confirm(lang, t(lang, K::DeleteStaticRoute), format!("{dest} → {hop}"), action.clone())
    })]
}

/// A router interface's menu (*Delete Interface*).
pub fn interface_items(lang: Lang, router_id: &str, p: &PortRow) -> Vec<MenuItem> {
    let (r, pid, label) = (router_id.to_string(), p.id.clone(), p.fixed_ips.join(", "));
    let is_gw = p.device_owner.as_deref() == Some("network:router_gateway");
    if is_gw {
        return vec![];
    }
    vec![MenuItem::danger(t(lang, K::DeleteInterface), move || {
        Dialog::confirm(lang, t(lang, K::DeleteInterface), label.clone(), OsAction::RouterRemoveInterface { id: r.clone(), port_id: pid.clone() })
    })]
}

// ---------------------------------------------------------------------------
// Security groups
// ---------------------------------------------------------------------------

/// *Create Security Group*: name and description.
pub fn create_security_group_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::CreateSecurityGroup),
        t(lang, K::CreateSecurityGroup),
        vec![Field::text("name", t(lang, K::Name)).required(), Field::area("description", t(lang, K::Description))],
        move |v| Ok(OsAction::SecurityGroupCreate { name: req(v, "name", K::Name, lang)?, description: v.opt("description") }),
    )
}

/// `default` only gets *Manage Rules*: Neutron won't delete it, and renaming it
/// is pointless (Horizon offers neither).
pub fn security_group_items(lang: Lang, id: &str, name: &str, description: Option<&str>, with_manage: bool) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if with_manage {
        items.push(MenuItem::link(t(lang, K::ManageRules), format!("/project/security_groups/{id}")));
    }
    if name == "default" {
        return items;
    }
    let (i, n, d) = (id.to_string(), name.to_string(), description.unwrap_or("").to_string());
    items.push(MenuItem::dialog(t(lang, K::EditSecurityGroup), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditSecurityGroup),
            t(lang, K::Save),
            vec![Field::text("name", t(lang, K::Name)).value(n.clone()).required(), Field::area("description", t(lang, K::Description)).value(d.clone())],
            move |v| Ok(OsAction::SecurityGroupUpdate { id: i.clone(), name: req(v, "name", K::Name, lang)?, description: v.opt("description") }),
        )
    }));
    let (i, n) = (id.to_string(), name.to_string());
    items.push(MenuItem::danger(t(lang, K::DeleteSecurityGroup), move || {
        Dialog::confirm(lang, t(lang, K::DeleteSecurityGroup), n.clone(), OsAction::SecurityGroupDelete { id: i.clone() })
    }));
    items
}

/// *Add Rule* to a security group: Neutron's rule fields, each with a help text
/// under it (roadmap 61) instead of Horizon's side panel. The ports stay visible
/// whatever the protocol; their help says when they don't apply.
pub fn add_rule_dialog(lang: Lang, group_id: &str) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let g = group_id.to_string();
    Dialog::form(
        tr(K::AddRule),
        tr(K::AddRule),
        vec![
            Field::select("direction", tr(K::Direction), options(&[("ingress", K::Ingress), ("egress", K::Egress)], lang)).value("ingress").required().help(tr(K::DirectionHelp)),
            Field::select("ethertype", tr(K::EtherType), options_raw(&["IPv4", "IPv6"])).value("IPv4").required().help(tr(K::EtherTypeHelp)),
            Field::select("protocol", tr(K::IpProtocol), options_raw(&["tcp", "udp", "icmp"])).help(tr(K::IpProtocolHelp)),
            Field::number("port_min", tr(K::PortFrom), 0).help(tr(K::PortFromHelp)),
            Field::number("port_max", tr(K::PortTo), 0).help(tr(K::PortToHelp)),
            Field::text("cidr", tr(K::RemoteCidr)).value("0.0.0.0/0").help(tr(K::RemoteCidrHelp)),
            Field::select("remote_group", tr(K::RemoteGroup), Source::Set(ChoiceSet::SecurityGroupIds)).help(tr(K::RemoteGroupHelp)),
            Field::text("description", tr(K::Description)).help(tr(K::RuleDescriptionHelp)),
        ],
        move |v| {
            let to_i32 = |k: &str, label: K| -> Result<Option<i32>, String> {
                v.num(k).map(|o| o.map(|n| n as i32)).map_err(|_| t(lang, label).to_string())
            };
            Ok(OsAction::RuleCreate(RuleSpec {
                security_group_id: g.clone(),
                direction: req(v, "direction", K::Direction, lang)?,
                ethertype: req(v, "ethertype", K::EtherType, lang)?,
                protocol: v.opt("protocol"),
                port_min: to_i32("port_min", K::PortFrom)?,
                port_max: to_i32("port_max", K::PortTo)?,
                // A remote group wins over the default CIDR.
                remote_ip_prefix: if v.opt("remote_group").is_some() { None } else { v.opt("cidr") },
                remote_group_id: v.opt("remote_group"),
                description: v.opt("description"),
            }))
        },
    )
}

/// A security group rule's menu (*Delete Rule*).
pub fn rule_items(lang: Lang, r: &RuleRow) -> Vec<MenuItem> {
    let (i, label) = (r.id.clone(), format!("{} {} {} {}", r.direction, r.ethertype, r.protocol, r.port_range));
    vec![MenuItem::danger(t(lang, K::DeleteRule), move || {
        Dialog::confirm(lang, t(lang, K::DeleteRule), label.clone(), OsAction::RuleDelete { id: i.clone() })
    })]
}

// ---------------------------------------------------------------------------
// Floating IPs
// ---------------------------------------------------------------------------

/// *Allocate IP To Project*: the external network (pool) and a description.
pub fn allocate_ip_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::AllocateIp),
        t(lang, K::AllocateIp),
        {
            let mut f = vec![
                Field::select("pool", t(lang, K::Pool), Source::Set(ChoiceSet::ExternalNetworks)).required(),
                Field::text("description", t(lang, K::Description)),
            ];
            // `qos` plugin and `qos-fip`.
            if needs::QOS_FIP.met(&caps()) {
                f.push(qos_field(lang));
            }
            f
        },
        move |v| {
            Ok(OsAction::FloatingIpAllocate {
                network_id: req(v, "pool", K::Pool, lang)?,
                description: v.opt("description"),
                port_id: None,
                qos_policy_id: v.opt("qos"),
            })
        },
    )
}

/// Admin › Floating IPs' *Allocate IP To Project* (roadmap 85), Horizon's
/// *Allocate Floating IP*: pool, project, optional address, description.
pub fn admin_allocate_ip_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::select("pool", tr(K::Pool), Source::Set(ChoiceSet::ExternalNetworks)).required(),
        Field::select("project_id", tr(K::Project), Source::Set(ChoiceSet::AllProjects)).required().empty_label(tr(K::SelectProject)),
        Field::text("address", tr(K::FloatingIpAddressOptional)).help(tr(K::FloatingIpAddressHelp)),
        Field::text("description", tr(K::Description)),
    ];
    Dialog::form(tr(K::AllocateFloatingIp), tr(K::AllocateFloatingIp), fields, move |v| {
        Ok(OsAction::AdminFloatingIpAllocate {
            network_id: req(v, "pool", K::Pool, lang)?,
            project_id: req(v, "project_id", K::Project, lang)?,
            address: v.opt("address"),
            description: v.opt("description"),
        })
    })
}

/// Admin › Floating IPs' row menu, Horizon's: *Disassociate* (when mapped) and
/// *Release Floating IP*. No *Associate*: the ports to pick from would be the
/// session project's, not the IP's owner's.
pub fn admin_floating_ip_items(lang: Lang, f: &FloatingIpRow, project_id: Option<&str>) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if f.fixed_ip.is_some() {
        let (i, ip) = (f.id.clone(), f.ip.clone());
        items.push(MenuItem::danger(t(lang, K::Disassociate), move || {
            Dialog::confirm(lang, t(lang, K::Disassociate), ip.clone(), OsAction::FloatingIpDisassociate { id: i.clone() })
        }));
    } else if let Some(owner) = project_id {
        // The owner's instance ports (Neutron needs the port and the floating
        // IP in one project), not the session project's.
        let (i, ip, owner) = (f.id.clone(), f.ip.clone(), owner.to_string());
        items.push(MenuItem::dialog(t(lang, K::Associate), move || {
            let i = i.clone();
            let mut d = Dialog::form(
                t(lang, K::Associate),
                t(lang, K::Associate),
                vec![Field::select("port", t(lang, K::PortToAssociate), Source::Of(ChoiceSet::AllAssociablePorts, owner.clone())).required()],
                move |v| Ok(OsAction::FloatingIpAssociate { id: i.clone(), port_id: req(v, "port", K::PortToAssociate, lang)? }),
            );
            d.intro = vec![ip.clone()];
            d
        }));
    }
    let (i, ip) = (f.id.clone(), f.ip.clone());
    items.push(MenuItem::danger(t(lang, K::ReleaseIp), move || {
        Dialog::confirm(lang, t(lang, K::ReleaseIp), ip.clone(), OsAction::FloatingIpRelease { id: i.clone() })
    }));
    items
}

// ---------------------------------------------------------------------------
// Admin › Compute (roadmap 85)
// ---------------------------------------------------------------------------

/// A required whole number, named in the error.
fn num_req(v: &Values, k: &str, label: K, lang: Lang) -> Result<i64, String> {
    v.num(k).map_err(|_| format!("{}: ?", t(lang, label)))?.ok_or_else(|| format!("{}: {}", t(lang, label), t(lang, K::RequiredField)))
}

/// Admin › Flavors' *Create Flavor*, Horizon's two steps: *Flavor Information*
/// and *Flavor Access* (projects picked there make the flavor private).
pub fn create_flavor_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::text("name", tr(K::Name)).required(),
        Field::text("flavor_id", tr(K::FlavorId)).value("auto").help(tr(K::FlavorIdHelp)),
        // Sizes are a few digits each: three a line, two lines (the user found
        // eight full-width one-liners too long a step).
        Field::number("vcpus", tr(K::Vcpus), 1).required().short(),
        Field::number("ram", tr(K::RamMb), 1).required().short(),
        Field::number("disk", tr(K::RootDiskGb), 0).value("0").required().short(),
        Field::number("ephemeral", tr(K::EphemeralDiskGb), 0).value("0").short(),
        Field::number("swap", tr(K::SwapDiskMb), 0).value("0").short(),
        Field::text("rxtx", tr(K::RxtxFactor)).value("1.0").short(),
        Field::multi("projects", tr(K::Projects), Source::Set(ChoiceSet::AllProjects)),
    ];
    Dialog::form(tr(K::CreateFlavor), tr(K::CreateFlavor), fields, move |v| {
        let rxtx_factor = match v.opt("rxtx") {
            None => None,
            Some(x) => Some(x.parse::<f64>().ok().filter(|f| *f > 0.0).ok_or_else(|| format!("{}: ?", t(lang, K::RxtxFactor)))?),
        };
        Ok(OsAction::FlavorCreate(Box::new(FlavorSpec {
            name: req(v, "name", K::Name, lang)?,
            flavor_id: v.opt("flavor_id"),
            vcpus: num_req(v, "vcpus", K::Vcpus, lang)?,
            ram_mb: num_req(v, "ram", K::RamMb, lang)?,
            disk_gb: num_req(v, "disk", K::RootDiskGb, lang)?,
            ephemeral_gb: v.num("ephemeral").map_err(|_| format!("{}: ?", t(lang, K::EphemeralDiskGb)))?.unwrap_or(0),
            swap_mb: v.num("swap").map_err(|_| format!("{}: ?", t(lang, K::SwapDiskMb)))?.unwrap_or(0),
            rxtx_factor,
            projects: v.list("projects"),
        })))
    })
    .wizard(vec![
        Step::new(tr(K::FlavorInfo), &["name", "flavor_id", "vcpus", "ram", "disk", "ephemeral", "swap", "rxtx"]).help(tr(K::FlavorInfoHelp)),
        Step::new(tr(K::FlavorAccessStep), &["projects"]).help(tr(K::FlavorAccessHelp)),
    ])
}

/// Admin › Flavors' row menu, Horizon's: *Update Metadata* (extra specs),
/// *Modify Access* (private flavors: Nova can't change `is_public`) and
/// *Delete Flavor*.
pub fn admin_flavor_items(lang: Lang, f: &FlavorRow) -> Vec<MenuItem> {
    let i = f.id.clone();
    let mut items = vec![metadata_item(lang, "OS::Nova::Flavor", &f.extra_specs, move |new, old| {
        let (set, unset) = metadata_changes(old, &new);
        OsAction::FlavorSetExtraSpecs { id: i.clone(), set, unset }
    })];
    if !f.public {
        let (i, current) = (f.id.clone(), f.access.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>());
        items.push(MenuItem::dialog(t(lang, K::ModifyAccess), move || {
            let (i, current) = (i.clone(), current.clone());
            Dialog::form(
                t(lang, K::ModifyAccess),
                t(lang, K::Save),
                vec![Field::multi("projects", t(lang, K::Projects), Source::Set(ChoiceSet::AllProjects)).value(current.join(",")).help(t(lang, K::FlavorAccess))],
                move |v| {
                    let new = v.list("projects");
                    let add: Vec<String> = new.iter().filter(|p| !current.contains(p)).cloned().collect();
                    let remove: Vec<String> = current.iter().filter(|p| !new.contains(p)).cloned().collect();
                    if add.is_empty() && remove.is_empty() {
                        return Err(t(lang, K::NothingChanged).to_string());
                    }
                    Ok(OsAction::FlavorSetAccess { id: i.clone(), add, remove })
                },
            )
        }));
    }
    let (i, n) = (f.id.clone(), f.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteFlavor), move || Dialog::confirm(lang, t(lang, K::DeleteFlavor), n.clone(), OsAction::FlavorDelete { id: i.clone() })));
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
        Ok(OsAction::AggregateCreate { name: req(v, "name", K::Name, lang)?, zone: v.opt("zone"), hosts: v.list("hosts") })
    })
    .wizard(vec![Step::new(tr(K::CreateAggregate), &["name", "zone"]), Step::new(tr(K::ManageHosts), &["hosts"])])
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
            vec![Field::text("name", t(lang, K::Name)).value(n.clone()).required(), Field::text("zone", t(lang, K::AvailabilityZone)).value(z.clone()).help(t(lang, K::AggregateZoneHelp))],
            move |v| Ok(OsAction::AggregateUpdate { id: i.clone(), name: req(v, "name", K::Name, lang)?, zone: v.opt("zone") }),
        )
    })];
    let (i, current) = (a.id.clone(), a.hosts.clone());
    items.push(MenuItem::dialog(t(lang, K::ManageHosts), move || {
        let (i, current) = (i.clone(), current.clone());
        Dialog::form(
            t(lang, K::ManageHosts),
            t(lang, K::Save),
            vec![Field::multi("hosts", t(lang, K::Hosts), Source::Set(ChoiceSet::AllComputeHosts)).value(current.join(","))],
            move |v| {
                let new = v.list("hosts");
                let add: Vec<String> = new.iter().filter(|h| !current.contains(h)).cloned().collect();
                let remove: Vec<String> = current.iter().filter(|h| !new.contains(h)).cloned().collect();
                if add.is_empty() && remove.is_empty() {
                    return Err(t(lang, K::NothingChanged).to_string());
                }
                Ok(OsAction::AggregateSetHosts { id: i.clone(), add, remove })
            },
        )
    }));
    // The zone is its own field (*Edit Host Aggregate*), though Nova keeps it as metadata.
    let editable: Vec<_> = a.metadata.iter().filter(|(k, _)| k != "availability_zone").cloned().collect();
    let i = a.id.clone();
    items.push(metadata_item(lang, "OS::Nova::Aggregate", &editable, move |new, old| {
        let (set, unset) = metadata_changes(old, &new);
        OsAction::AggregateSetMetadata { id: i.clone(), set, unset }
    }));
    let (i, n) = (a.id.clone(), a.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteAggregate), move || {
        Dialog::confirm(lang, t(lang, K::DeleteAggregate), n.clone(), OsAction::AggregateDelete { id: i.clone() })
    }));
    items
}

/// Admin › Volumes' and Snapshots' *Update Status*: Horizon's choices less the
/// current status, labelled like the status pills.
fn reset_status_item(lang: Lang, title: K, statuses: &'static [&'static str], current: &str, name: &str, make: impl Fn(String) -> OsAction + Send + Sync + 'static) -> MenuItem {
    let choices: Vec<(String, String)> = statuses.iter().filter(|s| !s.eq_ignore_ascii_case(current)).map(|s| (s.to_string(), crate::i18n::resource_status(lang, s))).collect();
    let (n, make) = (name.to_string(), std::sync::Arc::new(make));
    MenuItem::dialog(t(lang, title), move || {
        let make = make.clone();
        let mut d = Dialog::form(
            t(lang, title),
            t(lang, title),
            vec![Field::select("status", t(lang, K::Status), Source::Static(choices.clone())).required().help(t(lang, K::ResetStatusHelp))],
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
    let mut items = vec![reset_status_item(lang, K::UpdateVolumeStatus, VOLUME_STATUSES, &v.status, &v.name, move |status| OsAction::VolumeResetStatus { id: i.clone(), status })];
    let (i, n, from) = (v.id.clone(), v.name.clone(), host.unwrap_or("-").to_string());
    items.push(MenuItem::dialog(t(lang, K::MigrateVolume), move || {
        let i = i.clone();
        let mut d = Dialog::form(
            t(lang, K::MigrateVolume),
            t(lang, K::MigrateVolume),
            vec![
                // Cinder refuses migrating to the pool the volume is on.
                Field::select("host", t(lang, K::DestinationHost), Source::Except(ChoiceSet::CinderPools, from.clone())).required(),
                Field::check("force", t(lang, K::ForceHostCopy), false).help(t(lang, K::ForceHostCopyHelp)),
            ],
            move |v| Ok(OsAction::VolumeMigrate { id: i.clone(), host: req(v, "host", K::DestinationHost, lang)?, force_host_copy: v.flag("force") }),
        );
        d.intro = vec![n.clone(), format!("{}: {from}", t(lang, K::CurrentHost))];
        d
    }));
    let i = v.id.clone();
    items.push(metadata_item(lang, "OS::Cinder::Volume", &v.metadata, move |metadata, _| OsAction::VolumeSetMetadata { id: i.clone(), metadata }));
    let (i, n) = (v.id.clone(), v.name.clone());
    items.push(MenuItem::danger(t(lang, K::UnmanageVolume), move || {
        let mut d = Dialog::confirm(lang, t(lang, K::UnmanageVolume), n.clone(), OsAction::VolumeUnmanage { id: i.clone() });
        d.intro.push(t(lang, K::UnmanageHelp).to_string());
        d
    }));
    let (i, n) = (v.id.clone(), v.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteVolume), move || Dialog::confirm(lang, t(lang, K::DeleteVolume), n.clone(), OsAction::VolumeDelete { id: i.clone() })));
    items
}

/// Admin › Snapshots' row menu, Horizon's: *Update Status*, *Update Metadata*,
/// *Delete Volume Snapshot*.
pub fn admin_snapshot_items(lang: Lang, s: &Snapshot) -> Vec<MenuItem> {
    let i = s.id.clone();
    let mut items = vec![reset_status_item(lang, K::UpdateSnapshotStatus, SNAPSHOT_STATUSES, &s.status, &s.name, move |status| OsAction::SnapshotResetStatus { id: i.clone(), status })];
    let i = s.id.clone();
    items.push(metadata_item(lang, "OS::Cinder::Snapshot", &s.metadata, move |metadata, _| OsAction::SnapshotSetMetadata { id: i.clone(), metadata }));
    let (i, n) = (s.id.clone(), s.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteSnapshot), move || Dialog::confirm(lang, t(lang, K::DeleteSnapshot), n.clone(), OsAction::SnapshotDelete { id: i.clone() })));
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
            Field::text("identifier", tr(K::Identifier)).required().help(tr(K::IdentifierHelp)),
            Field::select("id_type", tr(K::IdentifierType), id_type).value("source-name").required(),
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
    Dialog::form(t(lang, K::CreateVolumeType), t(lang, K::CreateVolumeType), type_fields(lang, "", "", true), move |v| {
        Ok(OsAction::VolumeTypeCreate { name: req(v, "name", K::Name, lang)?, description: v.opt("description"), public: v.flag("public") })
    })
}

/// Admin › Volume Types' row menu, Horizon's: *View Extra Specs*, *Manage QoS
/// Spec Association*, *Edit Volume Type*, *Create* / *Update Encryption*,
/// then *Delete Encryption* (when encrypted) and *Delete Volume Type*.
pub fn volume_type_items(lang: Lang, ty: &VolumeTypeRow) -> Vec<MenuItem> {
    let i = ty.id.clone();
    let mut items = vec![pairs_item(lang, K::ViewExtraSpecs, K::ExtraSpecs, K::ExtraSpecsHelp, None, &ty.extra_specs, move |new, old| {
        let (set, unset) = metadata_changes(old, &new);
        OsAction::VolumeTypeSetExtraSpecs { id: i.clone(), set, unset }
    })];
    let (i, n, current) = (ty.id.clone(), ty.name.clone(), ty.qos.as_ref().map(|q| q.id.clone()));
    items.push(MenuItem::dialog(t(lang, K::ManageQosAssociation), move || {
        let (i, current) = (i.clone(), current.clone());
        let mut d = Dialog::form(
            t(lang, K::ManageQosAssociation),
            t(lang, K::Save),
            vec![Field::select("qos", t(lang, K::QosSpec), Source::Set(ChoiceSet::QosSpecs)).value(current.clone().unwrap_or_default())],
            move |v| {
                let qos_id = v.opt("qos");
                if qos_id == current {
                    return Err(t(lang, K::NothingChanged).to_string());
                }
                Ok(OsAction::VolumeTypeSetQos { id: i.clone(), qos_id, current_qos_id: current.clone() })
            },
        );
        d.intro = vec![n.clone()];
        d
    }));
    let (i, n, desc, public) = (ty.id.clone(), ty.name.clone(), ty.description.clone().unwrap_or_default(), ty.public);
    items.push(MenuItem::dialog(t(lang, K::EditVolumeType), move || {
        let i = i.clone();
        Dialog::form(t(lang, K::EditVolumeType), t(lang, K::Save), type_fields(lang, &n, &desc, public), move |v| {
            Ok(OsAction::VolumeTypeUpdate { id: i.clone(), name: req(v, "name", K::Name, lang)?, description: v.opt("description"), public: v.flag("public") })
        })
    }));
    let (i, n, enc) = (ty.id.clone(), ty.name.clone(), ty.encryption.clone());
    let title = if enc.is_some() { K::UpdateEncryption } else { K::CreateEncryption };
    items.push(MenuItem::dialog(t(lang, title), move || encryption_dialog(lang, title, &i, &n, enc.as_ref())));
    if ty.encrypted {
        let (i, n) = (ty.id.clone(), ty.name.clone());
        items.push(MenuItem::danger(t(lang, K::DeleteEncryption), move || Dialog::confirm(lang, t(lang, K::DeleteEncryption), n.clone(), OsAction::VolumeTypeEncryptionDelete { id: i.clone() })));
    }
    let (i, n) = (ty.id.clone(), ty.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteVolumeType), move || Dialog::confirm(lang, t(lang, K::DeleteVolumeType), n.clone(), OsAction::VolumeTypeDelete { id: i.clone() })));
    items
}

/// *Create* / *Update Encryption* of a volume type, prefilled from `current`.
fn encryption_dialog(lang: Lang, title: K, id: &str, name: &str, current: Option<&EncryptionSpec>) -> Dialog {
    let (i, update) = (id.to_string(), current.is_some());
    let c = current.cloned().unwrap_or(EncryptionSpec { provider: String::new(), control_location: "front-end".into(), cipher: None, key_size: None });
    let mut d = Dialog::form(
        t(lang, title),
        t(lang, K::Save),
        vec![
            Field::text("provider", t(lang, K::Provider)).value(c.provider).required().help(t(lang, K::EncryptionProviderHelp)),
            Field::select("control_location", t(lang, K::ControlLocation), options_raw(CONTROL_LOCATIONS)).value(c.control_location).required().help(t(lang, K::ControlLocationHelp)),
            Field::text("cipher", t(lang, K::Cipher)).value(c.cipher.unwrap_or_default()),
            Field::number("key_size", t(lang, K::KeySize), 1).value(c.key_size.map(|k| k.to_string()).unwrap_or_default()),
        ],
        move |v| {
            let key_size = num_u32(v, "key_size", K::KeySize, lang)?;
            let spec = EncryptionSpec { provider: req(v, "provider", K::Provider, lang)?, control_location: req(v, "control_location", K::ControlLocation, lang)?, cipher: v.opt("cipher"), key_size };
            Ok(OsAction::VolumeTypeEncryptionSet { id: i.clone(), spec, update })
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
            Field::select("consumer", t(lang, K::Consumer), options_raw(QOS_CONSUMERS)).value("back-end").required().help(t(lang, K::QosConsumerHelp)),
            Field::key_values("specs", t(lang, K::Specs), &[]),
        ],
        move |v| {
            Ok(OsAction::QosSpecCreate { name: req(v, "name", K::Name, lang)?, consumer: req(v, "consumer", K::Consumer, lang)?, specs: validate_metadata(&v.pairs("specs"))? })
        },
    )
}

/// The *QoS Specs* table's row menu, Horizon's: *Manage Specs*, *Edit
/// Consumer*, *Delete QoS Spec*.
pub fn qos_spec_items(lang: Lang, q: &QosSpecRow) -> Vec<MenuItem> {
    let i = q.id.clone();
    let mut items = vec![pairs_item(lang, K::ManageSpecs, K::Specs, K::MetadataHelp, None, &q.specs, move |new, old| {
        let (set, unset) = metadata_changes(old, &new);
        OsAction::QosSpecSetSpecs { id: i.clone(), set, unset }
    })];
    let (i, n, current) = (q.id.clone(), q.name.clone(), q.consumer.clone());
    items.push(MenuItem::dialog(t(lang, K::EditConsumer), move || {
        let (i, current) = (i.clone(), current.clone());
        let mut d = Dialog::form(
            t(lang, K::EditConsumer),
            t(lang, K::Save),
            vec![Field::select("consumer", t(lang, K::Consumer), options_raw(QOS_CONSUMERS)).value(current.clone()).required().help(t(lang, K::QosConsumerHelp))],
            move |v| {
                let consumer = req(v, "consumer", K::Consumer, lang)?;
                if consumer == current {
                    return Err(t(lang, K::NothingChanged).to_string());
                }
                Ok(OsAction::QosSpecSetConsumer { id: i.clone(), consumer })
            },
        );
        d.intro = vec![n.clone()];
        d
    }));
    let (i, n) = (q.id.clone(), q.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteQosSpec), move || Dialog::confirm(lang, t(lang, K::DeleteQosSpec), n.clone(), OsAction::QosSpecDelete { id: i.clone() })));
    items
}

/// Admin › Group Types' *Create Group Type*.
pub fn create_group_type_dialog(lang: Lang) -> Dialog {
    Dialog::form(t(lang, K::CreateGroupType), t(lang, K::CreateGroupType), type_fields(lang, "", "", true), move |v| {
        Ok(OsAction::GroupTypeCreate { name: req(v, "name", K::Name, lang)?, description: v.opt("description"), public: v.flag("public") })
    })
}

/// Admin › Group Types' row menu, Horizon's: *View Specs*, *Edit Group Type*,
/// *Delete Group Type*.
pub fn group_type_items(lang: Lang, g: &GroupTypeRow) -> Vec<MenuItem> {
    let i = g.id.clone();
    let mut items = vec![pairs_item(lang, K::ViewSpecs, K::Specs, K::MetadataHelp, None, &g.specs, move |new, old| {
        let (set, unset) = metadata_changes(old, &new);
        OsAction::GroupTypeSetSpecs { id: i.clone(), set, unset }
    })];
    let (i, n, desc, public) = (g.id.clone(), g.name.clone(), g.description.clone().unwrap_or_default(), g.public);
    items.push(MenuItem::dialog(t(lang, K::EditGroupType), move || {
        let i = i.clone();
        Dialog::form(t(lang, K::EditGroupType), t(lang, K::Save), type_fields(lang, &n, &desc, public), move |v| {
            Ok(OsAction::GroupTypeUpdate { id: i.clone(), name: req(v, "name", K::Name, lang)?, description: v.opt("description"), public: v.flag("public") })
        })
    }));
    let (i, n) = (g.id.clone(), g.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteGroupType), move || Dialog::confirm(lang, t(lang, K::DeleteGroupType), n.clone(), OsAction::GroupTypeDelete { id: i.clone() })));
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
            Field::area("definition", t(lang, K::NamespaceDefinition)).required().from_file(MAX_NAMESPACE_BYTES).help(t(lang, K::NamespaceDefinitionHelp)),
            Field::check("public", t(lang, K::Public), false),
            Field::check("protected", t(lang, K::Protected), false),
        ],
        move |v| {
            let definition = req(v, "definition", K::NamespaceDefinition, lang)?;
            // Checked here too, so a typo is caught before the request.
            serde_json::from_str::<serde_json::Value>(&definition).map_err(|e| format!("{}: {e}", t(lang, K::NamespaceDefinition)))?;
            Ok(OsAction::MetadefNamespaceImport { definition, public: v.flag("public"), protected: v.flag("protected") })
        },
    )
}

/// Admin › Metadata Definitions' row menu: *Edit Namespace*, *Update
/// Associations*, then *Delete Namespace* unless protected (Glance refuses it;
/// Horizon hides it).
pub fn metadef_items(lang: Lang, n: &MetadefNamespace) -> Vec<MenuItem> {
    let ns = n.namespace.clone();
    let display = if n.name != n.namespace { n.name.clone() } else { String::new() };
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
                Ok(OsAction::MetadefNamespaceUpdate { id: id.clone(), display_name: v.opt("display_name"), description: v.opt("description"), public: v.flag("public"), protected: v.flag("protected") })
            },
        );
        d.intro = vec![ns.clone()];
        d
    })];
    let (ns, current) = (n.namespace.clone(), n.resource_types.clone());
    items.push(MenuItem::dialog(t(lang, K::UpdateAssociations), move || {
        let (id, current) = (ns.clone(), current.clone());
        let mut d = Dialog::form(
            t(lang, K::UpdateAssociations),
            t(lang, K::Save),
            vec![Field::multi("types", t(lang, K::ResourceTypes), Source::Set(ChoiceSet::MetadefResourceTypes)).value(current.join(","))],
            move |v| {
                let new = v.list("types");
                let add_type_ids: Vec<String> = new.iter().filter(|x| !current.contains(x)).cloned().collect();
                let remove_type_ids: Vec<String> = current.iter().filter(|x| !new.contains(x)).cloned().collect();
                if add_type_ids.is_empty() && remove_type_ids.is_empty() {
                    return Err(t(lang, K::NothingChanged).to_string());
                }
                Ok(OsAction::MetadefNamespaceSetTypes { id: id.clone(), add_type_ids, remove_type_ids })
            },
        );
        d.intro = vec![ns.clone()];
        d
    }));
    if !n.protected {
        let (i, name) = (n.namespace.clone(), n.name.clone());
        items.push(MenuItem::danger(t(lang, K::DeleteNamespace), move || Dialog::confirm(lang, t(lang, K::DeleteNamespace), name.clone(), OsAction::MetadefNamespaceDelete { id: i.clone() })));
    }
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
                move |v| Ok(OsAction::ComputeServiceSet { id: i.clone(), enabled: false, reason: v.opt("reason") }),
            );
            d.intro = vec![host.clone()];
            d
        })]
    } else {
        vec![MenuItem::dialog(t(lang, K::EnableService), move || {
            Dialog::simple(lang, t(lang, K::EnableService), host.clone(), OsAction::ComputeServiceSet { id: i.clone(), enabled: true, reason: None })
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
            return Dialog::simple(lang, t(lang, K::MigrateInstance), n.clone(), OsAction::ServerMigrate { id: i, host: None });
        }
        let mut d = Dialog::form(
            t(lang, K::MigrateInstance),
            t(lang, K::MigrateInstance),
            vec![Field::select("host", t(lang, K::NewHost), Source::Except(ChoiceSet::ComputeHosts, from.clone())).help(t(lang, K::NewHostHelp))],
            move |v| Ok(OsAction::ServerMigrate { id: i.clone(), host: v.opt("host") }),
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
        let block = options(&[("auto", K::BlockMigrationAuto), ("true", K::Yes), ("false", K::No)], lang);
        let mut d = Dialog::form(
            t(lang, K::LiveMigrateInstance),
            t(lang, K::LiveMigrateInstance),
            vec![
                Field::select("host", t(lang, K::NewHost), Source::Except(ChoiceSet::ComputeHosts, from.clone())).help(t(lang, K::NewHostHelp)),
                Field::select("block", t(lang, K::BlockMigration), block).value("auto"),
            ],
            move |v| {
                let block_migration = match v.get("block").as_str() {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                };
                Ok(OsAction::ServerLiveMigrate { id: i.clone(), host: v.opt("host"), block_migration })
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
                ConfirmResize | RevertResize | Snapshot | Edit | UpdateMetadata | Pause | Unpause | Suspend | Resume | Shelve | Unshelve | Lock | Unlock | SoftReboot | HardReboot | ShutOff | Start | Delete
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

// ---------------------------------------------------------------------------
// Trunks
// ---------------------------------------------------------------------------

/// Sub-port fields; `prefix` keeps the create dialog's optional first sub-port apart.
fn subport_fields(lang: Lang, optional: bool) -> Vec<Field> {
    let tr = |k| t(lang, k).to_string();
    let port = Field::select("subport", tr(K::Subport), Source::Set(ChoiceSet::FreePorts));
    let vlan: fn(&Vals) -> bool = |v| v.get("segmentation_type").map(String::as_str) != Some("inherit") && v.get("subport").is_some_and(|p| !p.is_empty());
    vec![
        if optional { port.help(tr(K::SubportOptionalHelp)) } else { port.required() },
        Field::select("segmentation_type", tr(K::SegmentationType), options_raw(&["vlan", "inherit"])).value("vlan"),
        Field::number("segmentation_id", tr(K::SegmentationId), 1).help(tr(K::VlanHelp)).show_if(vlan),
    ]
}

fn subport_spec(v: &Values, lang: Lang) -> Result<Option<SubportSpec>, String> {
    let Some(port_id) = v.opt("subport") else { return Ok(None) };
    let kind = v.opt("segmentation_type").unwrap_or_else(|| "vlan".into());
    let id = match v.num("segmentation_id").map_err(|_| t(lang, K::VlanHelp).to_string())? {
        Some(n) if kind == "vlan" => Some(u16::try_from(n).map_err(|_| t(lang, K::VlanHelp).to_string())?),
        _ => None,
    };
    let spec = SubportSpec { port_id, segmentation_type: kind, segmentation_id: id };
    spec.body().map_err(|_| t(lang, K::VlanHelp).to_string())?;
    Ok(Some(spec))
}

/// *Create Trunk*: a parent port (unattached) and, optionally, a first sub-port.
pub fn create_trunk_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let mut fields = vec![
        Field::text("name", tr(K::Name)),
        Field::text("description", tr(K::Description)),
        Field::select("parent", tr(K::ParentPort), Source::Set(ChoiceSet::FreePorts)).required(),
        Field::check("admin_up", tr(K::EnableAdminState), true),
    ];
    fields.extend(subport_fields(lang, true));
    Dialog::form(tr(K::CreateTrunk), tr(K::CreateTrunk), fields, move |v| {
        let parent = req(v, "parent", K::ParentPort, lang)?;
        let sub = subport_spec(v, lang)?;
        if sub.as_ref().is_some_and(|s| s.port_id == parent) {
            return Err(t(lang, K::SubportNotParent).to_string());
        }
        Ok(OsAction::TrunkCreate { name: v.get("name"), description: v.opt("description"), port_id: parent, admin_up: v.flag("admin_up"), subports: sub.into_iter().collect() })
    })
}

/// *Add Sub-port* to a trunk.
pub fn add_subport_dialog(lang: Lang, trunk_id: &str) -> Dialog {
    let id = trunk_id.to_string();
    Dialog::form(t(lang, K::AddSubport), t(lang, K::AddSubport), subport_fields(lang, false), move |v| {
        let sub = subport_spec(v, lang)?.ok_or_else(|| format!("{}: {}", t(lang, K::Subport), t(lang, K::RequiredField)))?;
        Ok(OsAction::TrunkAddSubports { id: id.clone(), subports: vec![sub] })
    })
}

/// A trunk's menu: add sub-port, edit, delete.
pub fn trunk_items(lang: Lang, tk: &TrunkRow) -> Vec<MenuItem> {
    let i = tk.id.clone();
    let add = MenuItem::dialog(t(lang, K::AddSubport), move || add_subport_dialog(lang, &i));
    let (i, n, d, up) = (tk.id.clone(), tk.name.clone(), tk.description.clone().unwrap_or_default(), tk.admin_up);
    let edit = MenuItem::dialog(t(lang, K::EditTrunk), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditTrunk),
            t(lang, K::Save),
            vec![
                Field::text("name", t(lang, K::Name)).value(n.clone()),
                Field::text("description", t(lang, K::Description)).value(d.clone()),
                Field::check("admin_up", t(lang, K::EnableAdminState), up),
            ],
            move |v| Ok(OsAction::TrunkUpdate { id: i.clone(), name: v.get("name"), description: v.get("description"), admin_up: v.flag("admin_up") }),
        )
    });
    let (i, n) = (tk.id.clone(), tk.name.clone());
    let delete = MenuItem::danger(t(lang, K::DeleteTrunk), move || Dialog::confirm(lang, t(lang, K::DeleteTrunk), n.clone(), OsAction::TrunkDelete { id: i.clone() }));
    vec![add, edit, delete]
}

/// A trunk sub-port's menu (*Remove Sub-port*).
pub fn subport_items(lang: Lang, trunk_id: &str, sp: &Subport) -> Vec<MenuItem> {
    let (i, port) = (trunk_id.to_string(), sp.port_id.clone());
    let what = format!("{} (VLAN {})", sp.port_name, sp.segmentation_id.map(|n| n.to_string()).unwrap_or_else(|| sp.segmentation_type.clone()));
    vec![MenuItem::danger(t(lang, K::RemoveSubport), move || {
        Dialog::confirm(lang, t(lang, K::RemoveSubport), what.clone(), OsAction::TrunkRemoveSubports { id: i.clone(), port_ids: vec![port.clone()] })
    })]
}

// ---------------------------------------------------------------------------
// Firewall (FWaaS v2, roadmap 93)
// ---------------------------------------------------------------------------

/// Ports apply to TCP and UDP rules only (Neutron refuses them otherwise).
fn firewall_ports_apply(v: &Vals) -> bool {
    matches!(v.get("protocol").map(String::as_str), Some("tcp" | "udp"))
}

/// *Add Rule* (`None`) or *Edit Rule*. No *Shared*: the Project panel never offers or
/// sends it (Neutron's default, not shared, applies); it belongs to the Admin panel
/// (roadmap 94), as for policies and groups.
pub fn firewall_rule_dialog(lang: Lang, rule: Option<&FirewallRuleRow>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let r = rule.cloned();
    let val = |f: fn(&FirewallRuleRow) -> Option<String>| r.as_ref().and_then(f).unwrap_or_default();
    let fields = vec![
        Field::text("name", tr(K::Name)).value(val(|r| Some(r.name.clone()).filter(|n| *n != r.id))),
        Field::text("description", tr(K::Description)).value(val(|r| r.description.clone())),
        // One line: protocol, IP version, action; then source and destination, each
        // an address and (TCP / UDP only) a port on its own line.
        Field::select("protocol", tr(K::Protocol), Source::Static(vec![("any".into(), tr(K::AnyValue)), ("tcp".into(), "TCP".into()), ("udp".into(), "UDP".into()), ("icmp".into(), "ICMP".into())]))
            .value(r.as_ref().map(|r| r.protocol.clone().unwrap_or_else(|| "any".into())).unwrap_or_else(|| "tcp".into()))
            .required()
            .short(),
        Field::select("ip_version", tr(K::IpVersion), options_raw(&["4", "6"])).value(r.as_ref().map(|r| r.ip_version).unwrap_or(4).to_string()).required().short(),
        Field::select("action", tr(K::Action), options(&[("allow", K::FirewallAllow), ("deny", K::FirewallDeny), ("reject", K::FirewallReject)], lang))
            .value(r.as_ref().map(|r| r.action.clone()).unwrap_or_else(|| "allow".into()))
            .required()
            .short(),
        Field::text("source_ip", tr(K::SourceIp)).value(val(|r| r.source_ip.clone())).help(tr(K::FirewallAddressHelp)).half().new_line(),
        Field::text("source_port", tr(K::SourcePort)).value(val(|r| r.source_port.clone())).help(tr(K::FirewallPortHelp)).show_if(firewall_ports_apply).half(),
        Field::text("destination_ip", tr(K::DestinationIp)).value(val(|r| r.destination_ip.clone())).help(tr(K::FirewallAddressHelp)).half().new_line(),
        Field::text("destination_port", tr(K::DestinationPort)).value(val(|r| r.destination_port.clone())).help(tr(K::FirewallPortHelp)).show_if(firewall_ports_apply).half(),
        Field::check("enabled", tr(K::EnabledCol), r.as_ref().is_none_or(|r| r.enabled)),
    ];
    let (title, submit) = if r.is_some() { (K::EditRule, K::Save) } else { (K::AddRule, K::AddRule) };
    let id = r.map(|r| r.id);
    Dialog::form(tr(title), tr(submit), fields, move |v| {
        let spec = FirewallRuleSpec {
            name: v.get("name"),
            description: v.get("description"),
            protocol: v.get("protocol"),
            action: v.get("action"),
            ip_version: if v.get("ip_version") == "6" { 6 } else { 4 },
            source_ip: v.get("source_ip"),
            source_port: v.get("source_port"),
            destination_ip: v.get("destination_ip"),
            destination_port: v.get("destination_port"),
            enabled: v.flag("enabled"),
        };
        // The same check the server makes, so the dialog says what is wrong.
        spec.body()?;
        Ok(match &id {
            Some(id) => OsAction::FirewallRuleUpdate { id: id.clone(), spec },
            None => OsAction::FirewallRuleCreate(spec),
        })
    })
    .sized(DialogSize::Large)
}

/// The page's *Add Rule* button.
pub fn create_firewall_rule_dialog(lang: Lang) -> Dialog {
    firewall_rule_dialog(lang, None)
}

/// A rule's menu; another project's shared rule has none.
pub fn firewall_rule_items(lang: Lang, r: &FirewallRuleRow) -> Vec<MenuItem> {
    if !r.owned {
        return vec![];
    }
    let rr = r.clone();
    let (i, n) = (r.id.clone(), r.name.clone());
    vec![
        MenuItem::dialog(t(lang, K::EditRule), move || firewall_rule_dialog(lang, Some(&rr))),
        MenuItem::danger(t(lang, K::DeleteRule), move || Dialog::confirm(lang, t(lang, K::DeleteRule), n.clone(), OsAction::FirewallRuleDelete { id: i.clone() })),
    ]
}

/// *Add Policy*: its rules picked in evaluation order (*Allocated* order).
pub fn create_firewall_policy_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::text("name", tr(K::Name)).required(),
        Field::text("description", tr(K::Description)),
        Field::pick_many("rules", tr(K::Rules), Source::Set(ChoiceSet::FirewallRules)).help(tr(K::InsertRuleHelp)),
        Field::check("audited", tr(K::Audited), false),
    ];
    Dialog::form(tr(K::AddPolicy), tr(K::AddPolicy), fields, move |v| {
        Ok(OsAction::FirewallPolicyCreate {
            name: req(v, "name", K::Name, lang)?,
            description: v.get("description"),
            rule_ids: v.list("rules"),
            audited: v.flag("audited"),
        })
    })
    .sized(DialogSize::Large)
}

/// *Insert Rule*: a rule the policy doesn't have yet, at the top or after one
/// of its rules (Neutron's `insert_after`; *before* is the same as after the
/// previous one). Defaults to the end.
pub fn insert_firewall_rule_dialog(lang: Lang, p: &FirewallPolicyRow) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let mut places = vec![(String::new(), tr(K::AtTheTop))];
    places.extend(p.rules.iter().map(|r| (r.id.clone(), format!("{} {}", t(lang, K::InsertAfter), r.name))));
    let last = p.rules.last().map(|r| r.id.clone()).unwrap_or_default();
    let have: Vec<String> = p.rules.iter().map(|r| r.id.clone()).collect();
    let id = p.id.clone();
    let fields = vec![
        Field::select("rule", tr(K::Rule), Source::Without(ChoiceSet::FirewallRules, have)).required(),
        Field::select("after", tr(K::Position), Source::Static(places)).value(last).help(tr(K::InsertRuleHelp)),
    ];
    Dialog::form(tr(K::InsertRule), tr(K::InsertRule), fields, move |v| {
        Ok(OsAction::FirewallPolicyInsertRule { id: id.clone(), rule_id: req(v, "rule", K::Rule, lang)?, before_id: None, after_id: v.opt("after") })
    })
}

/// *Remove Rule* of one rule (from the policy page's rule table).
pub fn remove_firewall_rule_dialog(lang: Lang, policy_id: &str, rule: &Ref) -> Dialog {
    let action = OsAction::FirewallPolicyRemoveRule { id: policy_id.to_string(), rule_id: rule.id.clone() };
    Dialog::confirm(lang, t(lang, K::RemoveRule), rule.name.clone(), action)
}

/// A policy's menu; another project's shared policy has none.
pub fn firewall_policy_items(lang: Lang, p: &FirewallPolicyRow) -> Vec<MenuItem> {
    if !p.owned {
        return vec![];
    }
    let tr = move |k| t(lang, k).to_string();
    let mut items = Vec::new();
    let pp = p.clone();
    items.push(MenuItem::dialog(tr(K::InsertRule), move || insert_firewall_rule_dialog(lang, &pp)));
    if !p.rules.is_empty() {
        let pp = p.clone();
        items.push(MenuItem::dialog(tr(K::RemoveRule), move || {
            let id = pp.id.clone();
            let rules = pp.rules.iter().map(|r| (r.id.clone(), r.name.clone())).collect();
            Dialog::form(tr(K::RemoveRule), tr(K::RemoveRule), vec![Field::select("rule", tr(K::Rule), Source::Static(rules)).required()], move |v| {
                Ok(OsAction::FirewallPolicyRemoveRule { id: id.clone(), rule_id: req(v, "rule", K::Rule, lang)? })
            })
        }));
    }
    if p.is_default() {
        // Neutron refuses to edit or (while the default group uses it) delete it.
        return items;
    }
    let pp = p.clone();
    items.push(MenuItem::dialog(tr(K::EditPolicy), move || {
        let id = pp.id.clone();
        let fields = vec![
            Field::text("name", tr(K::Name)).value(pp.name.clone()).required(),
            Field::text("description", tr(K::Description)).value(pp.description.clone().unwrap_or_default()),
            Field::check("audited", tr(K::Audited), pp.audited),
        ];
        Dialog::form(tr(K::EditPolicy), tr(K::Save), fields, move |v| {
            Ok(OsAction::FirewallPolicyUpdate { id: id.clone(), name: req(v, "name", K::Name, lang)?, description: v.get("description"), audited: v.flag("audited") })
        })
    }));
    let (i, n) = (p.id.clone(), p.name.clone());
    items.push(MenuItem::danger(tr(K::DeletePolicy), move || Dialog::confirm(lang, tr(K::DeletePolicy), n.clone(), OsAction::FirewallPolicyDelete { id: i.clone() })));
    items
}

/// The ingress / egress policy selects; empty: no filtering that way. The (i)
/// says which traffic each direction is: FWaaS sees it from the port's device,
/// so on a router interface *ingress* is the subnet's traffic going out.
fn firewall_policy_field(lang: Lang, name: &'static str, label: K, value: Option<&Ref>) -> Field {
    let tip = if label == K::IngressPolicy { K::TipIngressPolicy } else { K::TipEgressPolicy };
    Field::select(name, t(lang, label), Source::Set(ChoiceSet::FirewallPolicies))
        .empty_label(t(lang, K::NoPolicy))
        .value(value.map(|p| p.id.clone()).unwrap_or_default())
        .tip(t(lang, tip))
        .half()
}

/// *Create Firewall Group*: policies, the ports it protects (`FirewallPorts`
/// has only free ones: Neutron refuses a port another group has) and admin state.
pub fn create_firewall_group_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::text("name", tr(K::Name)),
        Field::text("description", tr(K::Description)),
        firewall_policy_field(lang, "ingress", K::IngressPolicy, None),
        firewall_policy_field(lang, "egress", K::EgressPolicy, None),
        Field::check("admin_up", tr(K::AdminStateUp), true),
        Field::pick_many("ports", tr(K::Ports), Source::Set(ChoiceSet::FirewallPorts)),
    ];
    let details = ["name", "description", "ingress", "egress", "admin_up"];
    // Two steps, the group first and then the ports it protects (optional: without
    // ports the group is INACTIVE until *Add Port*).
    let steps = vec![Step::new(tr(K::FirewallGroup), &details).required(), Step::new(tr(K::Ports), &["ports"]).help(tr(K::FirewallPortsHelp))];
    Dialog::form(tr(K::CreateFirewallGroup), tr(K::CreateFirewallGroup), fields, move |v| {
        Ok(OsAction::FirewallGroupCreate {
            name: v.get("name"),
            description: v.get("description"),
            ingress_policy_id: v.opt("ingress"),
            egress_policy_id: v.opt("egress"),
            port_ids: v.list("ports"),
            admin_up: v.flag("admin_up"),
        })
    })
    .wizard(steps)
    .sized(DialogSize::Large)
}

/// *Add Port*: free router interface and instance ports, appended to the
/// group's list (Neutron takes the whole list).
pub fn add_firewall_port_dialog(lang: Lang, g: &FirewallGroupRow) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let (id, have): (String, Vec<String>) = (g.id.clone(), g.ports.iter().map(|p| p.port.id.clone()).collect());
    let fields = vec![Field::pick_many("ports", tr(K::Ports), Source::Set(ChoiceSet::FirewallPorts)).required().help(tr(K::FirewallPortsHelp))];
    Dialog::form(tr(K::AddPort), tr(K::AddPort), fields, move |v| {
        let added = v.list("ports");
        if added.is_empty() {
            return Err(format!("{}: {}", t(lang, K::Ports), t(lang, K::RequiredField)));
        }
        Ok(OsAction::FirewallGroupSetPorts { id: id.clone(), port_ids: have.iter().cloned().chain(added.into_iter().filter(|p| !have.contains(p))).collect() })
    })
}

/// *Remove Port* of one port: the group's list without it.
pub fn remove_firewall_port_dialog(lang: Lang, g: &FirewallGroupRow, port: &FirewallPort) -> Dialog {
    let rest = g.ports.iter().filter(|p| p.port.id != port.port.id).map(|p| p.port.id.clone()).collect();
    let what = firewall_port_label(port);
    Dialog::confirm(lang, t(lang, K::RemovePort), what, OsAction::FirewallGroupSetPorts { id: g.id.clone(), port_ids: rest })
}

/// `device: IPs`, else the port's name.
pub fn firewall_port_label(p: &FirewallPort) -> String {
    let ips = if p.ips.is_empty() { String::new() } else { format!(" ({})", p.ips.join(", ")) };
    match &p.device {
        Some(d) => format!("{}{ips}", d.name),
        None => format!("{}{ips}", p.port.name),
    }
}

/// A firewall group's menu; another project's shared group has none, and the
/// `default` group only *Add Port* / *Remove Port* (Neutron refuses the rest).
pub fn firewall_group_items(lang: Lang, g: &FirewallGroupRow) -> Vec<MenuItem> {
    if !g.owned {
        return vec![];
    }
    let tr = move |k| t(lang, k).to_string();
    let mut items = Vec::new();
    let gg = g.clone();
    let edit = MenuItem::dialog(tr(K::EditFirewallGroup), move || {
        let id = gg.id.clone();
        let fields = vec![
            Field::text("name", tr(K::Name)).value(gg.name.clone()),
            Field::text("description", tr(K::Description)).value(gg.description.clone().unwrap_or_default()),
            firewall_policy_field(lang, "ingress", K::IngressPolicy, gg.ingress.as_ref()),
            firewall_policy_field(lang, "egress", K::EgressPolicy, gg.egress.as_ref()),
            Field::check("admin_up", tr(K::AdminStateUp), gg.admin_up),
        ];
        Dialog::form(tr(K::EditFirewallGroup), tr(K::Save), fields, move |v| {
            Ok(OsAction::FirewallGroupUpdate {
                id: id.clone(),
                name: v.get("name"),
                description: v.get("description"),
                ingress_policy_id: v.opt("ingress"),
                egress_policy_id: v.opt("egress"),
                admin_up: v.flag("admin_up"),
            })
        })
    });
    if !g.is_default() {
        items.push(edit);
    }
    let gg = g.clone();
    items.push(MenuItem::dialog(tr(K::AddPort), move || add_firewall_port_dialog(lang, &gg)));
    if !g.ports.is_empty() {
        let gg = g.clone();
        items.push(MenuItem::dialog(tr(K::RemovePort), move || {
            let id = gg.id.clone();
            let ports: Vec<(String, String)> = gg.ports.iter().map(|p| (p.port.id.clone(), firewall_port_label(p))).collect();
            let all: Vec<String> = ports.iter().map(|(i, _)| i.clone()).collect();
            Dialog::form(tr(K::RemovePort), tr(K::RemovePort), vec![Field::select("port", tr(K::Port), Source::Static(ports)).required()], move |v| {
                let gone = req(v, "port", K::Port, lang)?;
                Ok(OsAction::FirewallGroupSetPorts { id: id.clone(), port_ids: all.iter().filter(|p| **p != gone).cloned().collect() })
            })
        }));
    }
    if g.deletable() {
        let (i, n) = (g.id.clone(), g.name.clone());
        items.push(MenuItem::danger(tr(K::DeleteFirewallGroup), move || Dialog::confirm(lang, tr(K::DeleteFirewallGroup), n.clone(), OsAction::FirewallGroupDelete { id: i.clone() })));
    }
    items
}

// ---------------------------------------------------------------------------
// Network QoS
// ---------------------------------------------------------------------------

/// *Create QoS Policy*: name, description and whether it is shared.
pub fn create_qos_policy_dialog(lang: Lang) -> Dialog {
    Dialog::form(
        t(lang, K::CreatePolicy),
        t(lang, K::CreatePolicy),
        vec![
            Field::text("name", t(lang, K::Name)).required(),
            Field::text("description", t(lang, K::Description)),
            Field::check("shared", t(lang, K::Shared), false),
        ],
        move |v| Ok(OsAction::QosPolicyCreate { name: req(v, "name", K::Name, lang)?, description: v.opt("description"), shared: v.flag("shared") }),
    )
}

/// Only the project's own policies can be changed; shared ones are read-only.
pub fn qos_policy_items(lang: Lang, p: &QosPolicyRow) -> Vec<MenuItem> {
    if !p.owned {
        return vec![];
    }
    let pp = p.clone();
    let (i, n) = (p.id.clone(), p.name.clone());
    vec![
        MenuItem::dialog(t(lang, K::AddRule), move || qos_rule_dialog(lang, &pp.id, None)),
        {
            let pp = p.clone();
            MenuItem::dialog(t(lang, K::EditPolicy), move || {
                let id = pp.id.clone();
                Dialog::form(
                    t(lang, K::EditPolicy),
                    t(lang, K::Save),
                    vec![
                        Field::text("name", t(lang, K::Name)).value(pp.name.clone()).required(),
                        Field::text("description", t(lang, K::Description)).value(pp.description.clone().unwrap_or_default()),
                        Field::check("shared", t(lang, K::Shared), pp.shared),
                    ],
                    move |v| Ok(OsAction::QosPolicyUpdate { id: id.clone(), name: req(v, "name", K::Name, lang)?, description: v.get("description"), shared: v.flag("shared") }),
                )
            })
        },
        MenuItem::danger(t(lang, K::DeletePolicy), move || Dialog::confirm(lang, t(lang, K::DeletePolicy), n.clone(), OsAction::QosPolicyDelete { id: i.clone() })),
    ]
}

fn qos_kind_label(lang: Lang, kind: &str) -> String {
    t(
        lang,
        match kind {
            "bandwidth_limit" => K::BandwidthLimit,
            "dscp_marking" => K::DscpMarking,
            "minimum_bandwidth" => K::MinimumBandwidth,
            _ => K::MinimumPacketRate,
        },
    )
    .to_string()
}

/// *Add Rule* / *Edit Rule*: the fields follow the rule type (Neutron's
/// `bandwidth_limit`, `dscp_marking`, `minimum_bandwidth`, `minimum_packet_rate`).
pub fn qos_rule_dialog(lang: Lang, policy_id: &str, rule: Option<&QosRule>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let kinds = Source::Static(QOS_RULE_TYPES.iter().map(|k| (k.to_string(), qos_kind_label(lang, k))).collect());
    let num = |n: Option<i64>| n.map(|n| n.to_string()).unwrap_or_default();
    let kind = rule.map(|r| r.kind.clone()).unwrap_or_else(|| "bandwidth_limit".into());
    let mut fields = vec![];
    if rule.is_none() {
        fields.push(Field::select("kind", tr(K::Type), kinds).value(kind.clone()).required());
    }
    // On edit the type is fixed: show only its fields.
    let is = |k: &'static str| -> Box<dyn Fn(Field) -> Field> {
        match rule {
            Some(r) if r.kind == k => Box::new(|f| f),
            Some(_) => Box::new(|f: Field| f.show_if(|_| false)),
            None => Box::new(move |f: Field| f.when("kind", k)),
        }
    };
    let directional: fn(&Vals) -> bool = |v| v.get("kind").is_some_and(|k| !QosRuleSpec::directions(k).is_empty());
    let dir_options = |k: &str| options_raw(if k.is_empty() { &["egress", "ingress", "any"] } else { QosRuleSpec::directions(k) });
    let direction = Field::select("direction", tr(K::Direction), dir_options(if rule.is_some() { &kind } else { "" }))
        .value(rule.and_then(|r| r.direction.clone()).unwrap_or_else(|| "egress".into()));
    fields.push(match rule {
        Some(r) if QosRuleSpec::directions(&r.kind).is_empty() => direction.show_if(|_| false),
        Some(_) => direction,
        None => direction.show_if(directional).help(tr(K::AnyOnlyPacketRate)),
    });
    fields.extend([
        is("bandwidth_limit")(Field::number("max_kbps", tr(K::MaxKbps), 1).value(num(rule.and_then(|r| r.max_kbps))).required()),
        is("bandwidth_limit")(Field::number("max_burst_kbps", tr(K::MaxBurstKbps), 0).value(num(rule.and_then(|r| r.max_burst_kbps)))),
        is("dscp_marking")(
            Field::select("dscp_mark", tr(K::DscpMark), options_raw(&DSCP_MARKS.map(|m| m.to_string()).iter().map(String::as_str).collect::<Vec<_>>()))
                .value(num(rule.and_then(|r| r.dscp_mark)))
                .required(),
        ),
        is("minimum_bandwidth")(Field::number("min_kbps", tr(K::MinKbps), 1).value(num(rule.and_then(|r| r.min_kbps))).required()),
        is("minimum_packet_rate")(Field::number("min_kpps", tr(K::MinKpps), 1).value(num(rule.and_then(|r| r.min_kpps))).required()),
    ]);
    let (pid, id) = (policy_id.to_string(), rule.map(|r| r.id.clone()));
    let fixed = rule.map(|r| r.kind.clone());
    let title = tr(if rule.is_some() { K::EditRule } else { K::AddRule });
    Dialog::form(title.clone(), title, fields, move |v| {
        let kind = fixed.clone().unwrap_or_else(|| v.get("kind"));
        let n = |k: &str, label| num_u32(v, k, label, lang);
        let spec = QosRuleSpec {
            direction: v.opt("direction").filter(|_| !QosRuleSpec::directions(&kind).is_empty()),
            max_kbps: n("max_kbps", K::MaxKbps)?.filter(|_| kind == "bandwidth_limit"),
            max_burst_kbps: n("max_burst_kbps", K::MaxBurstKbps)?.filter(|_| kind == "bandwidth_limit"),
            dscp_mark: n("dscp_mark", K::DscpMark)?.and_then(|m| u8::try_from(m).ok()).filter(|_| kind == "dscp_marking"),
            min_kbps: n("min_kbps", K::MinKbps)?.filter(|_| kind == "minimum_bandwidth"),
            min_kpps: n("min_kpps", K::MinKpps)?.filter(|_| kind == "minimum_packet_rate"),
            kind,
        };
        spec.body().map_err(|e| invalid(lang, e))?;
        Ok(match &id {
            Some(id) => OsAction::QosRuleUpdate { policy_id: pid.clone(), id: id.clone(), spec },
            None => OsAction::QosRuleCreate { policy_id: pid.clone(), spec },
        })
    })
}

/// A QoS rule's menu (edit, delete).
pub fn qos_rule_items(lang: Lang, policy_id: &str, r: &QosRule) -> Vec<MenuItem> {
    let (p, rule) = (policy_id.to_string(), r.clone());
    let edit = MenuItem::dialog(t(lang, K::EditRule), move || qos_rule_dialog(lang, &p, Some(&rule)));
    let (p, kind, id) = (policy_id.to_string(), r.kind.clone(), r.id.clone());
    let what = qos_kind_label(lang, &r.kind);
    let delete = MenuItem::danger(t(lang, K::DeleteRule), move || {
        Dialog::confirm(lang, t(lang, K::DeleteRule), what.clone(), OsAction::QosRuleDelete { policy_id: p.clone(), kind: kind.clone(), id: id.clone() })
    });
    vec![edit, delete]
}

/// The localised name of a QoS rule type (`bandwidth_limit` → *Bandwidth Limit*).
pub fn qos_kind(lang: Lang, kind: &str) -> String {
    qos_kind_label(lang, kind)
}

// ---------------------------------------------------------------------------
// Load balancers (Octavia)
// ---------------------------------------------------------------------------

fn u16_of(v: &Values, k: &str, label: K, lang: Lang) -> Result<Option<u16>, String> {
    let bad = || format!("{}: 1-65535", t(lang, label));
    match v.num(k).map_err(|_| bad())? {
        None => Ok(None),
        Some(n) => u16::try_from(n).map(Some).map_err(|_| bad()),
    }
}

fn u32_of(v: &Values, k: &str, label: K, lang: Lang) -> Result<Option<u32>, String> {
    num_u32(v, k, label, lang)
}

/// Fixed choices. Optional selects get their *— None —* entry from the
/// select itself (`Combo`), so it is never added here.
fn static_refs(items: &[(String, String)]) -> Source {
    Source::Static(items.to_vec())
}

/// Shown errors of a spec check: the field the user can fix, then the detail.
fn invalid(lang: Lang, e: String) -> String {
    format!("{}: {e}", t(lang, K::InvalidSettings))
}

/// A listener's *Advanced* options (roadmap 54) behind one check box, as
/// octavia-dashboard has them: each only when the cloud's Octavia has it
/// (`needs::LB_*`). `headers_if`: when *Insert Headers* shows (HTTP listeners),
/// `None` for none. `when`: the condition of the check box itself (the wizard's
/// *Create Listener*). Starts open when `existing` sets any of them.
fn listener_option_fields(
    lang: Lang,
    existing: Option<&LbListenerOptions>,
    headers_if: Option<fn(&Vals) -> bool>,
    ciphers_if: Option<fn(&Vals) -> bool>,
    when: Option<(&'static str, &'static str)>,
) -> Vec<Field> {
    let tr = |k| t(lang, k).to_string();
    let c = caps();
    let o = existing.cloned().unwrap_or_default();
    let set = o != LbListenerOptions { allowed_cidrs: o.allowed_cidrs.clone().filter(|c| !c.is_empty()), ..Default::default() };
    let mut adv = Field::check("lb_advanced", tr(K::LbAdvanced), set).help(tr(K::LbAdvancedHelp));
    if let Some((f, want)) = when {
        adv = adv.when(f, want);
    }
    let mut fields = vec![adv];
    let ms = |x: Option<u32>| x.map(|n| n.to_string()).unwrap_or_default();
    if needs::LB_TIMEOUTS.met(&c) {
        for (name, label, help, value) in [
            // octavia-dashboard's order.
            ("timeout_client_data", K::TimeoutClientData, K::TimeoutClientDataHelp, o.timeout_client_data),
            ("timeout_tcp_inspect", K::TimeoutTcpInspect, K::TimeoutTcpInspectHelp, o.timeout_tcp_inspect),
            ("timeout_member_connect", K::TimeoutMemberConnect, K::TimeoutMemberConnectHelp, o.timeout_member_connect),
            ("timeout_member_data", K::TimeoutMemberData, K::TimeoutMemberDataHelp, o.timeout_member_data),
        ] {
            fields.push(Field::number(name, tr(label), 0).value(ms(value)).help(tr(help)).when("lb_advanced", "true"));
        }
    }
    if let Some(cond) = headers_if {
        fields.push(
            Field::multi("insert_headers", tr(K::InsertHeaders), options_raw(&LB_INSERT_HEADERS))
                .value(o.insert_headers.join(","))
                .help(tr(K::InsertHeadersHelp))
                .when("lb_advanced", "true")
                .show_if(cond),
        );
    }
    // On an edit, only when the listener reports the key (Octavia >= 2.12 there).
    if needs::LB_ALLOWED_CIDRS.met(&c) && (existing.is_none() || o.allowed_cidrs.is_some()) {
        fields.push(
            Field::area("allowed_cidrs", tr(K::AllowedCidrs))
                .value(o.allowed_cidrs.unwrap_or_default().join("\n"))
                .help(tr(K::AllowedCidrsHelp))
                .when("lb_advanced", "true"),
        );
    }
    // `TERMINATED_HTTPS` listeners only (roadmap 54); named apart from the pool's
    // field, both being in the wizard.
    if let Some(cond) = ciphers_if.filter(|_| needs::LB_POOL_CIPHERS.met(&c)) {
        fields.push(
            Field::text("listener_ciphers", tr(K::TlsCiphers))
                .value(o.tls_ciphers.clone().unwrap_or_default())
                .help(tr(K::TlsCiphersHelp))
                .when("lb_advanced", "true")
                .show_if(cond),
        );
    }
    fields
}

/// Listener protocols for a new listener: `TERMINATED_HTTPS` too when the
/// `key-manager` plugin is on and Barbican is in the catalog (roadmap 54).
fn listener_protocols() -> Source {
    let mut p = LB_PROTOCOLS.to_vec();
    if needs::LB_TLS_TERMINATION.met(&caps()) {
        p.push(LB_TLS_PROTOCOL);
    }
    options_raw(&p)
}

/// A `TERMINATED_HTTPS` listener's certificate and SNI certificates (roadmap
/// 54), from `ChoiceSet::LbCertificates`. `tls_if`: when they show (the new
/// listener's protocol); `None` on an edit, which only offers them for such a
/// listener.
fn listener_certificate_fields(lang: Lang, existing: Option<&LbListener>, tls_if: Option<fn(&Vals) -> bool>) -> Vec<Field> {
    let tr = |k| t(lang, k).to_string();
    let mut cert = Field::select("certificate", tr(K::LbCertificate), Source::Set(ChoiceSet::LbCertificates))
        .required()
        .value(existing.and_then(|l| l.default_tls_container_ref.clone()).unwrap_or_default())
        .help(tr(K::LbCertificateHelp));
    let mut sni = Field::multi("sni", tr(K::SniCertificates), Source::Set(ChoiceSet::LbCertificates))
        .value(existing.map(|l| l.sni_container_refs.join(",")).unwrap_or_default())
        .help(tr(K::SniCertificatesHelp));
    if let Some(cond) = tls_if {
        (cert, sni) = (cert.show_if(cond), sni.show_if(cond));
    }
    vec![cert, sni]
}

/// Whether a new listener's protocol (the wizard's or the dialog's field) is
/// `TERMINATED_HTTPS` / inserts headers.
fn wizard_tls(v: &Vals) -> bool {
    v.get("listener_protocol").map(String::as_str) == Some(LB_TLS_PROTOCOL)
}
fn wizard_headers(v: &Vals) -> bool {
    v.get("listener_protocol").is_some_and(|p| lb_inserts_headers(p))
}
fn dialog_tls(v: &Vals) -> bool {
    v.get("protocol").map(String::as_str) == Some(LB_TLS_PROTOCOL)
}
fn dialog_headers(v: &Vals) -> bool {
    v.get("protocol").is_some_and(|p| lb_inserts_headers(p))
}

/// The options typed in [`listener_option_fields`]; `None` when the box is
/// unticked (an edit then leaves them alone, a create keeps Octavia's defaults).
fn listener_options_of(v: &Values, lang: Lang) -> Result<Option<LbListenerOptions>, String> {
    if !v.flag("lb_advanced") {
        return Ok(None);
    }
    Ok(Some(LbListenerOptions {
        timeout_client_data: u32_of(v, "timeout_client_data", K::TimeoutClientData, lang)?,
        timeout_member_connect: u32_of(v, "timeout_member_connect", K::TimeoutMemberConnect, lang)?,
        timeout_member_data: u32_of(v, "timeout_member_data", K::TimeoutMemberData, lang)?,
        timeout_tcp_inspect: u32_of(v, "timeout_tcp_inspect", K::TimeoutTcpInspect, lang)?,
        insert_headers: v.list("insert_headers"),
        allowed_cidrs: v.0.contains_key("allowed_cidrs").then(|| v.list("allowed_cidrs")),
        tls_ciphers: v.opt("listener_ciphers"),
    }))
}

/// *TLS Enabled* and *TLS Cipher String* of a pool (roadmap 54), when the
/// cloud's Octavia has them (2.8, 2.15).
fn pool_tls_fields(lang: Lang, existing: Option<&LbPool>, when: Option<(&'static str, &'static str)>) -> Vec<Field> {
    let tr = |k| t(lang, k).to_string();
    let c = caps();
    if !needs::LB_POOL_TLS.met(&c) {
        return vec![];
    }
    let mut on = Field::check("tls_enabled", tr(K::PoolTlsEnabled), existing.is_some_and(|p| p.tls_enabled)).help(tr(K::PoolTlsEnabledHelp));
    if let Some((f, want)) = when {
        on = on.when(f, want);
    }
    let mut fields = vec![on];
    if needs::LB_POOL_CIPHERS.met(&c) {
        fields.push(
            Field::text("tls_ciphers", tr(K::TlsCiphers))
                .value(existing.and_then(|p| p.tls_ciphers.clone()).unwrap_or_default())
                .help(tr(K::TlsCiphersHelp))
                .when("tls_enabled", "true"),
        );
    }
    fields
}

/// `(tls_enabled, tls_ciphers)` from [`pool_tls_fields`]; `None` when not offered.
fn pool_tls_of(v: &Values) -> Option<(bool, Option<String>)> {
    v.0.contains_key("tls_enabled").then(|| (v.flag("tls_enabled"), v.opt("tls_ciphers").filter(|_| v.flag("tls_enabled"))))
}

/// Moves a listener's port between 80 and 443 when its protocol changes to or
/// from HTTPS / TERMINATED_HTTPS, unless the user typed another port (roadmap 54;
/// octavia-dashboard leaves the port empty instead).
fn suggest_port(changed: &str, v: &mut Vals, protocol: &str, port: &str) {
    if changed != protocol {
        return;
    }
    let tls = matches!(v.get(protocol).map(String::as_str), Some("HTTPS" | LB_TLS_PROTOCOL));
    let next = match (v.get(port).map(String::as_str), tls) {
        (Some("80") | None, true) => "443",
        (Some("443"), false) => "80",
        _ => return,
    };
    v.insert(port.to_string(), next.to_string());
}

/// An *Admin State Up* box of a create form (roadmap 54): on unless shown
/// and unticked, so a form without it creates the resource up.
fn admin_up_of(v: &Values, k: &str) -> bool {
    !v.0.contains_key(k) || v.flag(k)
}

fn persistence_of(v: &Values) -> (Option<String>, Option<String>) {
    let kind = v.opt("persistence");
    let cookie = v.opt("cookie_name").filter(|_| kind.as_deref() == Some("APP_COOKIE"));
    (kind, cookie)
}

fn monitor_spec(v: &Values, lang: Lang, kind: String) -> Result<LbMonitorSpec, String> {
    let http = kind.starts_with("HTTP");
    let retries = |k: &str, label| -> Result<Option<u8>, String> {
        match v.num(k).map_err(|_| format!("{}: 1-10", t(lang, label)))? {
            None => Ok(None),
            Some(n) => u8::try_from(n).map(Some).map_err(|_| format!("{}: 1-10", t(lang, label))),
        }
    };
    Ok(LbMonitorSpec {
        name: v.opt("monitor_name"),
        delay: u32_of(v, "delay", K::DelaySeconds, lang)?.unwrap_or(5),
        timeout: u32_of(v, "timeout", K::TimeoutSeconds, lang)?.unwrap_or(5),
        max_retries: retries("max_retries", K::MaxRetries)?.unwrap_or(3),
        max_retries_down: retries("max_retries_down", K::MaxRetriesDown)?,
        http_method: v.opt("http_method").filter(|_| http),
        url_path: v.opt("url_path").filter(|_| http),
        expected_codes: v.opt("expected_codes").filter(|_| http),
        admin_up: admin_up_of(v, "monitor_admin_up"),
        kind,
    })
}

fn monitor_fields(lang: Lang, when: Option<(&'static str, &'static str)>, m: Option<&LbMonitor>) -> Vec<Field> {
    let tr = |k| t(lang, k).to_string();
    let w = |f: Field| match when {
        Some((a, b)) => f.when(a, b),
        None => f,
    };
    let http: fn(&Vals) -> bool = |v| v.get("monitor_type").is_some_and(|t| t.starts_with("HTTP"));
    let num = |n: Option<i64>, d: &str| n.map(|n| n.to_string()).unwrap_or_else(|| d.to_string());
    let mut out = vec![];
    if m.is_none() {
        out.push(w(Field::select("monitor_type", tr(K::Type), options_raw(&LB_MONITOR_TYPES)).value("HTTP").required()));
    }
    out.extend([
        w(Field::text("monitor_name", tr(K::Name)).value(m.map(|m| m.name.clone()).unwrap_or_default())),
        w(Field::number("delay", tr(K::DelaySeconds), 1).value(num(m.and_then(|m| m.delay), "5")).required()),
        w(Field::number("timeout", tr(K::TimeoutSeconds), 1).value(num(m.and_then(|m| m.timeout), "5")).help(tr(K::TimeoutHelp)).required()),
        w(Field::number("max_retries", tr(K::MaxRetries), 1).value(num(m.and_then(|m| m.max_retries), "3")).required()),
        w(Field::number("max_retries_down", tr(K::MaxRetriesDown), 1).value(num(m.and_then(|m| m.max_retries_down), "3"))),
    ]);
    // Edit: the type is fixed, so the HTTP fields follow the monitor's own type.
    let is_http = m.map(|m| m.kind.starts_with("HTTP"));
    let hf = |f: Field| match is_http {
        Some(true) => w(f),
        Some(false) => f.show_if(|_| false),
        None => w(f).show_if(http),
    };
    out.extend([
        hf(Field::select("http_method", tr(K::HttpMethod), options_raw(&LB_HTTP_METHODS)).value(m.and_then(|m| m.http_method.clone()).unwrap_or_else(|| "GET".into()))),
        hf(Field::text("url_path", tr(K::UrlPath)).value(m.and_then(|m| m.url_path.clone()).unwrap_or_else(|| "/".into()))),
        hf(Field::text("expected_codes", tr(K::ExpectedCodes)).value(m.and_then(|m| m.expected_codes.clone()).unwrap_or_else(|| "200".into())).help(tr(K::ExpectedCodesHelp))),
    ]);
    // A new monitor; an edit has its own *Enable Admin State* box (roadmap 54).
    if m.is_none() {
        out.push(w(Field::check("monitor_admin_up", tr(K::EnableAdminState), true)));
    }
    out
}

/// Horizon's *Create Load Balancer* wizard: details, then an optional listener,
/// its default pool, members and health monitor. Sent as one fully populated
/// request, so Octavia creates everything or nothing.
pub fn create_load_balancer_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let listener: fn(&Vals) -> bool = |v| v.get("with_listener").map(String::as_str) == Some("true");
    let pool: fn(&Vals) -> bool = |v| v.get("with_listener").map(String::as_str) == Some("true") && v.get("with_pool").map(String::as_str) == Some("true");
    let mut fields = vec![
        Field::text("name", tr(K::Name)),
        Field::text("description", tr(K::Description)),
        Field::select("vip_subnet", tr(K::Subnet), Source::Set(ChoiceSet::LbVipSubnets)).required(),
        Field::text("vip_address", tr(K::IpAddress)).help(tr(K::VipAddressHelp)),
        Field::select("flavor", tr(K::Flavor), Source::Set(ChoiceSet::LbFlavors)),
        // octavia-dashboard's details step (roadmap 54); empty = the cloud's default.
        Field::select("lb_az", tr(K::AvailabilityZone), Source::Set(ChoiceSet::LbAvailabilityZones)),
        Field::check("lb_admin_up", tr(K::EnableAdminState), true),
        Field::yes_no("with_listener", tr(K::CreateListener), true),
        Field::select("listener_protocol", tr(K::Protocol), listener_protocols()).value("HTTP").required().when("with_listener", "true"),
        Field::number("listener_port", tr(K::Port), 1).value("80").required().when("with_listener", "true"),
    ];
    // Right after the port, as in octavia-dashboard's listener form (roadmap 54).
    fields.extend(listener_certificate_fields(lang, None, Some(wizard_tls)).into_iter().map(|f| f.when("with_listener", "true")));
    fields.extend([
        Field::text("listener_name", tr(K::Name)).when("with_listener", "true"),
        Field::number("connection_limit", tr(K::ConnectionLimit), 1).help(tr(K::UnlimitedWhenEmpty)).when("with_listener", "true"),
        Field::check("listener_admin_up", tr(K::EnableAdminState), true).when("with_listener", "true"),
    ]);
    fields.extend(listener_option_fields(lang, None, Some(wizard_headers), Some(wizard_tls), Some(("with_listener", "true"))));
    fields.extend([
        Field::yes_no("with_pool", tr(K::CreatePool), true).when("with_listener", "true"),
        // Octavia's single-call create rejects an unnamed default pool.
        Field::text("pool_name", tr(K::Name)).value("pool-1").required().when("with_pool", "true"),
        Field::select("algorithm", tr(K::Algorithm), options_raw(&LB_ALGORITHMS)).value("ROUND_ROBIN").required().when("with_pool", "true"),
        Field::select("persistence", tr(K::SessionPersistence), options_raw(&LB_PERSISTENCE)).when("with_pool", "true"),
        Field::text("cookie_name", tr(K::CookieName)).when("persistence", "APP_COOKIE"),
        Field::check("pool_admin_up", tr(K::EnableAdminState), true).when("with_pool", "true"),
        // One row per member, new rows on the listener's port (roadmap 54).
        Field::member_rows("members", tr(K::Members), &[], Some("listener_port"), vec![]).when("with_pool", "true"),
        Field::yes_no("with_monitor", tr(K::CreateHealthMonitor), true).when("with_pool", "true"),
    ]);
    fields.extend(pool_tls_fields(lang, None, Some(("with_pool", "true"))));
    fields.extend(monitor_fields(lang, Some(("with_monitor", "true")), None));
    let steps = vec![
        Step::new(tr(K::LoadBalancer), &["name", "description", "vip_subnet", "vip_address", "flavor", "lb_az", "lb_admin_up"]).help(tr(K::LbDetailsHelp)).required(),
        Step::new(
            tr(K::Listener),
            &[
                "with_listener",
                "listener_protocol",
                "listener_port",
                "certificate",
                "sni",
                "listener_name",
                "connection_limit",
                "listener_admin_up",
                "lb_advanced",
                "timeout_client_data",
                "timeout_tcp_inspect",
                "timeout_member_connect",
                "timeout_member_data",
                "insert_headers",
                "allowed_cidrs",
                "listener_ciphers",
            ],
        )
        .help(tr(K::ListenerHelp)),
        Step::new(tr(K::Pool), &["with_pool", "pool_name", "algorithm", "persistence", "cookie_name", "pool_admin_up", "tls_enabled", "tls_ciphers"]).help(tr(K::PoolHelp)).enabled_if(listener),
        Step::new(tr(K::Members), &["members"]).help(tr(K::MembersHelp)).enabled_if(pool),
        Step::new(
            tr(K::HealthMonitor),
            &["with_monitor", "monitor_type", "monitor_name", "delay", "timeout", "max_retries", "max_retries_down", "http_method", "url_path", "expected_codes", "monitor_admin_up"],
        )
        .help(tr(K::MonitorHelp))
        .enabled_if(pool),
    ];
    Dialog::form(tr(K::CreateLoadBalancer), tr(K::CreateLoadBalancer), fields, move |v| {
        let with_listener = v.flag("with_listener");
        let with_pool = with_listener && v.flag("with_pool");
        let protocol = v.get("listener_protocol");
        let listener = if with_listener {
            Some(LbListenerSpec {
                name: v.opt("listener_name"),
                description: None,
                protocol: protocol.clone(),
                port: u16_of(v, "listener_port", K::Port, lang)?.ok_or_else(|| format!("{}: {}", t(lang, K::Port), t(lang, K::RequiredField)))?,
                connection_limit: u32_of(v, "connection_limit", K::ConnectionLimit, lang)?,
                options: listener_options_of(v, lang)?.unwrap_or_default(),
                default_tls_container_ref: v.opt("certificate").filter(|_| protocol == LB_TLS_PROTOCOL),
                sni_container_refs: if protocol == LB_TLS_PROTOCOL { v.list("sni") } else { vec![] },
                admin_up: admin_up_of(v, "listener_admin_up"),
            })
        } else {
            None
        };
        let (persistence, cookie) = persistence_of(v);
        let (tls_enabled, tls_ciphers) = pool_tls_of(v).unwrap_or_default();
        let pool = with_pool.then(|| LbPoolSpec {
            name: v.opt("pool_name"),
            description: None,
            protocol: pool_protocol_for(&protocol).to_string(),
            algorithm: v.get("algorithm"),
            session_persistence: persistence,
            cookie_name: cookie,
            tls_enabled,
            tls_ciphers,
            admin_up: admin_up_of(v, "pool_admin_up"),
        });
        let members = if with_pool { member_specs(&v.get("members")).map_err(|e| invalid(lang, e))? } else { vec![] };
        let monitor = if with_pool && v.flag("with_monitor") { Some(monitor_spec(v, lang, v.get("monitor_type"))?) } else { None };
        let spec = LbCreateSpec {
            name: v.opt("name"),
            description: v.opt("description"),
            vip_subnet_id: req(v, "vip_subnet", K::Subnet, lang)?,
            vip_address: v.opt("vip_address"),
            flavor_id: v.opt("flavor"),
            availability_zone: v.opt("lb_az"),
            admin_up: admin_up_of(v, "lb_admin_up"),
            listener,
            pool,
            members,
            monitor,
        };
        spec.check().map_err(|e| invalid(lang, e))?;
        Ok(OsAction::LoadBalancerCreate(Box::new(spec)))
    })
    .on_change(|f, v, _| suggest_port(f, v, "listener_protocol", "listener_port"))
    .wizard(steps)
}

/// Header and row menu of a load balancer. `fip`: its VIP's floating IP
/// `(id, address)`, known on the detail page only.
pub fn load_balancer_items(lang: Lang, l: &LoadBalancerRow) -> Vec<MenuItem> {
    load_balancer_menu(lang, l, None)
}

/// The load balancer detail header's menu.
pub fn load_balancer_detail_items(lang: Lang, d: &LoadBalancerDetail) -> Vec<MenuItem> {
    load_balancer_menu(lang, &d.row, Some(d))
}

fn load_balancer_menu(lang: Lang, l: &LoadBalancerRow, d: Option<&LoadBalancerDetail>) -> Vec<MenuItem> {
    let mut items = Vec::new();
    let (i, n, desc, up) = (l.id.clone(), l.name.clone(), l.description.clone().unwrap_or_default(), l.admin_up);
    items.push(MenuItem::dialog(t(lang, K::EditLoadBalancer), move || {
        let i = i.clone();
        Dialog::form(
            t(lang, K::EditLoadBalancer),
            t(lang, K::Save),
            vec![
                Field::text("name", t(lang, K::Name)).value(n.clone()),
                Field::text("description", t(lang, K::Description)).value(desc.clone()),
                Field::check("admin_up", t(lang, K::EnableAdminState), up),
            ],
            move |v| Ok(OsAction::LoadBalancerUpdate { id: i.clone(), name: v.get("name"), description: v.get("description"), admin_up: v.flag("admin_up") }),
        )
    }));
    if let Some(d) = d {
        match (d.floating_ip_id.clone(), d.vip_port_id.clone()) {
            (Some(fip), _) => {
                let ip = d.floating_ip.clone().unwrap_or_default();
                items.push(MenuItem::danger(t(lang, K::DisassociateFloatingIp), move || {
                    Dialog::confirm(lang, t(lang, K::DisassociateFloatingIp), ip.clone(), OsAction::FloatingIpDisassociate { id: fip.clone() })
                }));
            }
            (None, Some(port)) => {
                items.push(MenuItem::dialog(t(lang, K::AssociateFloatingIp), move || {
                    let port = port.clone();
                    Dialog::form(
                        t(lang, K::AssociateFloatingIp),
                        t(lang, K::Associate),
                        vec![Field::select("fip", t(lang, K::FloatingIp), Source::Set(ChoiceSet::FreeFloatingIps)).required()],
                        move |v| Ok(OsAction::FloatingIpAssociate { id: req(v, "fip", K::FloatingIp, lang)?, port_id: port.clone() }),
                    )
                }));
            }
            _ => {}
        }
    }
    let (i, n) = (l.id.clone(), l.name.clone());
    items.push(MenuItem::danger(t(lang, K::DeleteLoadBalancer), move || {
        let mut d = Dialog::confirm(lang, t(lang, K::DeleteLoadBalancer), n.clone(), OsAction::LoadBalancerDelete { id: i.clone(), cascade: true });
        d.intro.insert(1, t(lang, K::LbCascade).into());
        d
    }));
    items
}

fn refs_of<T>(items: &[T], f: impl Fn(&T) -> (String, String)) -> Vec<(String, String)> {
    items.iter().map(f).collect()
}

/// *Create Listener* / *Edit Listener*; `pools` of this load balancer for the default pool.
pub fn listener_dialog(lang: Lang, d: &LoadBalancerDetail, existing: Option<&LbListener>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let pools = refs_of(&d.pools, |p| (p.id.clone(), p.name.clone()));
    let lb = d.row.id.clone();
    let limit = existing.and_then(|l| l.connection_limit).map(|n| n.to_string()).unwrap_or_default();
    let mut fields = vec![];
    if existing.is_none() {
        fields.push(Field::select("protocol", tr(K::Protocol), listener_protocols()).value("HTTP").required());
        fields.push(Field::number("port", tr(K::Port), 1).value("80").required());
    }
    match existing {
        None => fields.extend(listener_certificate_fields(lang, None, Some(dialog_tls))),
        Some(l) if l.protocol == LB_TLS_PROTOCOL => fields.extend(listener_certificate_fields(lang, Some(l), None)),
        Some(_) => {}
    }
    fields.extend([
        Field::text("name", tr(K::Name)).value(existing.map(|l| l.name.clone()).unwrap_or_default()),
        Field::text("description", tr(K::Description)).value(existing.and_then(|l| l.description.clone()).unwrap_or_default()),
        Field::number("connection_limit", tr(K::ConnectionLimit), 1).value(limit).help(tr(K::UnlimitedWhenEmpty)),
        Field::select("default_pool", tr(K::DefaultPool), static_refs(&pools))
            .value(existing.and_then(|l| l.default_pool.as_ref().map(|p| p.id.clone())).unwrap_or_default()),
    ]);
    // Also on a new listener (roadmap 54).
    fields.push(Field::check("admin_up", tr(K::EnableAdminState), existing.is_none_or(|l| l.admin_up)));
    type Cond = Option<fn(&Vals) -> bool>;
    let always: fn(&Vals) -> bool = |_| true;
    let (headers_if, ciphers_if): (Cond, Cond) = match existing {
        None => (Some(dialog_headers), Some(dialog_tls)),
        Some(l) => (lb_inserts_headers(&l.protocol).then_some(always), (l.protocol == LB_TLS_PROTOCOL).then_some(always)),
    };
    fields.extend(listener_option_fields(lang, existing.map(|l| &l.options), headers_if, ciphers_if, None));
    let id = existing.map(|l| (l.id.clone(), l.protocol.clone()));
    let title = tr(if existing.is_some() { K::EditListener } else { K::CreateListener });
    Dialog::form(title.clone(), title, fields, move |v| {
        let connection_limit = u32_of(v, "connection_limit", K::ConnectionLimit, lang)?;
        let options = listener_options_of(v, lang)?;
        Ok(match &id {
            Some((id, protocol)) => {
                if let Some(o) = &options {
                    o.check(protocol).map_err(|e| invalid(lang, e))?;
                }
                OsAction::ListenerUpdate {
                    id: id.clone(),
                    name: v.get("name"),
                    description: v.get("description"),
                    connection_limit,
                    default_pool_id: v.opt("default_pool"),
                    admin_up: v.flag("admin_up"),
                    protocol: protocol.clone(),
                    options,
                    tls: (protocol == LB_TLS_PROTOCOL).then(|| (v.get("certificate"), v.list("sni"))),
                }
            }
            None => {
                let spec = LbListenerSpec {
                    name: v.opt("name"),
                    description: v.opt("description"),
                    protocol: v.get("protocol"),
                    port: u16_of(v, "port", K::Port, lang)?.unwrap_or(0),
                    connection_limit,
                    options: options.unwrap_or_default(),
                    default_tls_container_ref: v.opt("certificate").filter(|_| v.get("protocol") == LB_TLS_PROTOCOL),
                    sni_container_refs: if v.get("protocol") == LB_TLS_PROTOCOL { v.list("sni") } else { vec![] },
                    admin_up: v.flag("admin_up"),
                };
                spec.check().map_err(|e| invalid(lang, e))?;
                OsAction::ListenerCreate { loadbalancer_id: lb.clone(), spec, default_pool_id: v.opt("default_pool") }
            }
        })
    })
    .on_change(|f, v, _| suggest_port(f, v, "protocol", "port"))
}

/// *Create Pool* / *Edit Pool*. A new pool can become the default pool of a
/// listener that has none.
pub fn pool_dialog(lang: Lang, d: &LoadBalancerDetail, existing: Option<&LbPool>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let free: Vec<_> = d.listeners.iter().filter(|l| l.default_pool.is_none()).map(|l| (l.id.clone(), format!("{} ({} {})", l.name, l.protocol, l.port.unwrap_or(0)))).collect();
    let lb = d.row.id.clone();
    let mut fields = vec![Field::text("name", tr(K::Name)).value(existing.map(|p| p.name.clone()).unwrap_or_default())];
    fields.push(Field::text("description", tr(K::Description)).value(existing.and_then(|p| p.description.clone()).unwrap_or_default()));
    if existing.is_none() {
        fields.push(Field::select("listener", tr(K::Listener), static_refs(&free)).help(tr(K::PoolListenerHelp)));
        fields.push(Field::select("protocol", tr(K::Protocol), options_raw(&LB_PROTOCOLS)).value("HTTP").required());
    }
    fields.extend([
        Field::select("algorithm", tr(K::Algorithm), options_raw(&LB_ALGORITHMS)).value(existing.map_or("ROUND_ROBIN".into(), |p| p.algorithm.clone())).required(),
        Field::select("persistence", tr(K::SessionPersistence), options_raw(&LB_PERSISTENCE)).value(existing.and_then(|p| p.persistence_type.clone()).unwrap_or_default()),
        Field::text("cookie_name", tr(K::CookieName)).value(existing.and_then(|p| p.cookie_name.clone()).unwrap_or_default()).when("persistence", "APP_COOKIE"),
    ]);
    fields.extend(pool_tls_fields(lang, existing, None));
    // Also on a new pool (roadmap 54).
    fields.push(Field::check("admin_up", tr(K::EnableAdminState), existing.is_none_or(|p| p.admin_up)));
    let id = existing.map(|p| (p.id.clone(), p.protocol.clone()));
    let title = tr(if existing.is_some() { K::EditPool } else { K::CreatePool });
    Dialog::form(title.clone(), title, fields, move |v| {
        let (session_persistence, cookie_name) = persistence_of(v);
        let tls = pool_tls_of(v);
        Ok(match &id {
            Some((id, protocol)) => {
                let (on, ciphers) = tls.clone().unwrap_or_default();
                check_pool_tls(protocol, on, ciphers.as_deref()).map_err(|e| invalid(lang, e))?;
                OsAction::PoolUpdate {
                    id: id.clone(),
                    name: v.get("name"),
                    description: v.get("description"),
                    algorithm: v.get("algorithm"),
                    session_persistence,
                    cookie_name,
                    admin_up: v.flag("admin_up"),
                    protocol: protocol.clone(),
                    tls,
                }
            }
            None => {
                let (tls_enabled, tls_ciphers) = tls.unwrap_or_default();
                let spec = LbPoolSpec {
                    name: v.opt("name"),
                    description: v.opt("description"),
                    protocol: v.get("protocol"),
                    algorithm: v.get("algorithm"),
                    session_persistence,
                    cookie_name,
                    tls_enabled,
                    tls_ciphers,
                    admin_up: v.flag("admin_up"),
                };
                spec.check().map_err(|e| invalid(lang, e))?;
                OsAction::PoolCreate { loadbalancer_id: lb.clone(), listener_id: v.opt("listener"), spec }
            }
        })
    })
}

/// *Add Member* (an instance address, or any address on a subnet, into one of
/// this load balancer's pools) / *Edit Member*. Both set the monitor address
/// and port (roadmap 54).
pub fn member_dialog(lang: Lang, d: &LoadBalancerDetail, existing: Option<&LbMember>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let pools = refs_of(&d.pools, |p| (p.id.clone(), p.name.clone()));
    let mut fields = vec![];
    if existing.is_none() {
        fields.push(Field::select("pool", tr(K::Pool), static_refs(&pools)).value(pools.first().map(|p| p.0.clone()).unwrap_or_default()).required());
        let sources = [("instance", K::Instance), ("external", K::ExternalAddress)];
        fields.push(Field::select("source", tr(K::MemberSource), options(&sources, lang)).value("instance").required());
        fields.push(Field::select("target", tr(K::IpAddress), Source::Set(ChoiceSet::LbMemberTargets)).required().when("source", "instance"));
        fields.push(Field::text("address", tr(K::IpAddress)).required().when("source", "external"));
        fields.push(Field::select("subnet", tr(K::Subnet), Source::Set(ChoiceSet::LbMemberSubnets)).required().when("source", "external"));
        fields.push(Field::number("port", tr(K::MemberPort), 1).value("80").required());
    }
    fields.extend([
        Field::text("name", tr(K::Name)).value(existing.map(|m| if m.name == m.id { String::new() } else { m.name.clone() }).unwrap_or_default()),
        Field::number("weight", tr(K::Weight), 0).value(existing.and_then(|m| m.weight).unwrap_or(1).to_string()).required(),
        Field::check("backup", tr(K::Backup), existing.is_some_and(|m| m.backup)),
        Field::text("monitor_address", tr(K::MonitorAddress)).value(existing.and_then(|m| m.monitor_address.clone()).unwrap_or_default()).help(tr(K::MonitorAddressHelp)),
        Field::number("monitor_port", tr(K::MonitorPort), 1).value(existing.and_then(|m| m.monitor_port).map(|p| p.to_string()).unwrap_or_default()),
        Field::check("admin_up", tr(K::EnableAdminState), existing.is_none_or(|m| m.admin_up)),
    ]);
    let ids = existing.map(|m| (m.pool.id.clone(), m.id.clone()));
    let title = tr(if existing.is_some() { K::EditMember } else { K::AddMember });
    Dialog::form(title.clone(), title, fields, move |v| {
        let weight = u16_of(v, "weight", K::Weight, lang)?.unwrap_or(1);
        let monitor_port = u16_of(v, "monitor_port", K::MonitorPort, lang)?;
        Ok(match &ids {
            Some((pool, id)) => OsAction::MemberUpdate {
                pool_id: pool.clone(),
                id: id.clone(),
                name: v.get("name"),
                weight,
                backup: v.flag("backup"),
                admin_up: v.flag("admin_up"),
                monitor_address: v.opt("monitor_address"),
                monitor_port,
            },
            None => {
                let (address, subnet) = if v.get("source") == "external" {
                    (req(v, "address", K::IpAddress, lang)?, v.opt("subnet"))
                } else {
                    let target = req(v, "target", K::IpAddress, lang)?;
                    target.split_once('@').map(|(a, s)| (a.to_string(), Some(s.to_string()))).unwrap_or((target.clone(), None))
                };
                let spec = LbMemberSpec {
                    name: v.opt("name"),
                    address,
                    port: u16_of(v, "port", K::MemberPort, lang)?.unwrap_or(0),
                    subnet_id: subnet,
                    weight: Some(weight),
                    backup: v.flag("backup"),
                    monitor_address: v.opt("monitor_address"),
                    monitor_port,
                    admin_up: v.flag("admin_up"),
                };
                spec.check().map_err(|e| invalid(lang, e))?;
                OsAction::MemberCreate { pool_id: req(v, "pool", K::Pool, lang)?, spec }
            }
        })
    })
}

/// *Edit Members* of a pool: the whole list at once, sent as Octavia's batch
/// update, which deletes the members left out; so it is destructive, and the
/// editor lists the members a save would delete (roadmap 54).
pub fn pool_members_dialog(lang: Lang, d: &LoadBalancerDetail, pool: &LbPool) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let current: Vec<LbMember> = d.members.iter().filter(|m| m.pool.id == pool.id).cloned().collect();
    let rows: Vec<MemberRow> = current.iter().map(MemberRow::from_member).collect();
    let pool_id = pool.id.clone();
    let fields = vec![Field::member_rows("members", tr(K::Members), &rows, None, current).help(tr(K::MembersHelp))];
    let mut dialog = Dialog::form(format!("{}: {}", tr(K::EditMembers), pool.name), tr(K::EditMembers), fields, move |v| {
        let members = member_specs(&v.get("members")).map_err(|e| invalid(lang, e))?;
        Ok(OsAction::PoolSetMembers { pool_id: pool_id.clone(), members })
    })
    .sized(DialogSize::Large);
    dialog.intro = vec![tr(K::EditMembersIntro)];
    dialog.danger = true;
    dialog
}

/// *Create Health Monitor* (for a pool without one) / *Edit Health Monitor*.
pub fn monitor_dialog(lang: Lang, d: &LoadBalancerDetail, existing: Option<&LbMonitor>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let free: Vec<_> = d.pools.iter().filter(|p| p.monitor.is_none()).map(|p| (p.id.clone(), p.name.clone())).collect();
    let mut fields = vec![];
    if existing.is_none() {
        fields.push(Field::select("pool", tr(K::Pool), static_refs(&free)).value(free.first().map(|p| p.0.clone()).unwrap_or_default()).required());
    }
    fields.extend(monitor_fields(lang, None, existing));
    if let Some(m) = existing {
        fields.push(Field::check("admin_up", tr(K::EnableAdminState), m.admin_up));
    }
    let edit = existing.map(|m| (m.id.clone(), m.kind.clone()));
    let title = tr(if existing.is_some() { K::EditHealthMonitor } else { K::CreateHealthMonitor });
    Dialog::form(title.clone(), title, fields, move |v| {
        Ok(match &edit {
            Some((id, kind)) => {
                let spec = monitor_spec(v, lang, kind.clone())?;
                spec.check().map_err(|e| invalid(lang, e))?;
                OsAction::MonitorUpdate { id: id.clone(), spec, admin_up: v.flag("admin_up") }
            }
            None => {
                let spec = monitor_spec(v, lang, v.get("monitor_type"))?;
                spec.check().map_err(|e| invalid(lang, e))?;
                OsAction::MonitorCreate { pool_id: req(v, "pool", K::Pool, lang)?, spec }
            }
        })
    })
}

/// A listener's menu (edit, delete).
pub fn listener_items(lang: Lang, d: &LoadBalancerDetail, l: &LbListener) -> Vec<MenuItem> {
    let (dd, ll) = (d.clone(), l.clone());
    let (i, n) = (l.id.clone(), l.name.clone());
    vec![
        MenuItem::dialog(t(lang, K::EditListener), move || listener_dialog(lang, &dd, Some(&ll))),
        MenuItem::danger(t(lang, K::DeleteListener), move || Dialog::confirm(lang, t(lang, K::DeleteListener), n.clone(), OsAction::ListenerDelete { id: i.clone() })),
    ]
}

/// A pool's menu (edit, delete).
pub fn pool_items(lang: Lang, d: &LoadBalancerDetail, p: &LbPool) -> Vec<MenuItem> {
    let (dd, pp) = (d.clone(), p.clone());
    let (i, n) = (p.id.clone(), p.name.clone());
    vec![
        MenuItem::dialog(t(lang, K::EditPool), move || pool_dialog(lang, &dd, Some(&pp))),
        MenuItem::dialog(t(lang, K::EditMembers), {
            let (dd, pp) = (d.clone(), p.clone());
            move || pool_members_dialog(lang, &dd, &pp)
        }),
        MenuItem::danger(t(lang, K::DeletePool), move || Dialog::confirm(lang, t(lang, K::DeletePool), n.clone(), OsAction::PoolDelete { id: i.clone() })),
    ]
}

/// A member's menu (edit, delete).
pub fn member_items(lang: Lang, d: &LoadBalancerDetail, m: &LbMember) -> Vec<MenuItem> {
    let (dd, mm) = (d.clone(), m.clone());
    let (pool, i) = (m.pool.id.clone(), m.id.clone());
    let what = format!("{}:{}", m.address, m.port.unwrap_or(0));
    vec![
        MenuItem::dialog(t(lang, K::EditMember), move || member_dialog(lang, &dd, Some(&mm))),
        MenuItem::danger(t(lang, K::DeleteMember), move || {
            Dialog::confirm(lang, t(lang, K::DeleteMember), what.clone(), OsAction::MemberDelete { pool_id: pool.clone(), id: i.clone() })
        }),
    ]
}

/// A health monitor's menu (edit, delete).
pub fn monitor_items(lang: Lang, d: &LoadBalancerDetail, m: &LbMonitor) -> Vec<MenuItem> {
    let (dd, mm) = (d.clone(), m.clone());
    let (i, n) = (m.id.clone(), m.name.clone());
    vec![
        MenuItem::dialog(t(lang, K::EditHealthMonitor), move || monitor_dialog(lang, &dd, Some(&mm))),
        MenuItem::danger(t(lang, K::DeleteHealthMonitor), move || {
            Dialog::confirm(lang, t(lang, K::DeleteHealthMonitor), n.clone(), OsAction::MonitorDelete { id: i.clone() })
        }),
    ]
}

/// Listeners that can take L7 policies: Octavia applies them to HTTP and
/// `TERMINATED_HTTPS` listeners only (roadmap 54).
pub fn l7_listeners(d: &LoadBalancerDetail) -> Vec<(String, String)> {
    d.listeners.iter().filter(|l| lb_inserts_headers(&l.protocol)).map(|l| (l.id.clone(), format!("{} ({} {})", l.name, l.protocol, l.port.unwrap_or(0)))).collect()
}

/// *Create L7 Policy* / *Edit L7 Policy* (roadmap 54), as octavia-dashboard
/// has it: the redirect pool or URL shows for its action only.
pub fn l7_policy_dialog(lang: Lang, d: &LoadBalancerDetail, existing: Option<&LbL7Policy>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let pools = refs_of(&d.pools, |p| (p.id.clone(), p.name.clone()));
    let mut fields = vec![];
    if existing.is_none() {
        fields.push(Field::select("listener", tr(K::Listener), static_refs(&l7_listeners(d))).required());
    }
    fields.extend([
        Field::text("name", tr(K::Name)).value(existing.map(|p| p.name.clone()).unwrap_or_default()),
        Field::text("description", tr(K::Description)).value(existing.and_then(|p| p.description.clone()).unwrap_or_default()),
        Field::select("action", tr(K::Action), options_raw(&LB_L7_ACTIONS)).value(existing.map_or("REDIRECT_TO_POOL".into(), |p| p.action.clone())).required(),
        Field::select("redirect_pool", tr(K::RedirectPool), static_refs(&pools))
            .value(existing.and_then(|p| p.redirect_pool.as_ref().map(|r| r.id.clone())).unwrap_or_default())
            .required()
            .when("action", "REDIRECT_TO_POOL"),
        Field::text("redirect_url", tr(K::RedirectUrl))
            .value(existing.and_then(|p| p.redirect_url.clone()).unwrap_or_default())
            .required()
            .when("action", "REDIRECT_TO_URL"),
        Field::number("position", tr(K::Position), 1).value(existing.and_then(|p| p.position).map(|n| n.to_string()).unwrap_or_default()).help(tr(K::L7PositionHelp)),
        Field::check("admin_up", tr(K::EnableAdminState), existing.is_none_or(|p| p.admin_up)),
    ]);
    let id = existing.map(|p| p.id.clone());
    let title = tr(if existing.is_some() { K::EditL7Policy } else { K::CreateL7Policy });
    Dialog::form(title.clone(), title, fields, move |v| {
        let spec = LbL7PolicySpec {
            name: v.opt("name"),
            description: v.opt("description"),
            action: req(v, "action", K::Action, lang)?,
            position: u32_of(v, "position", K::Position, lang)?,
            redirect_pool_id: v.opt("redirect_pool"),
            redirect_url: v.opt("redirect_url"),
            admin_up: v.flag("admin_up"),
        };
        spec.check().map_err(|e| invalid(lang, e))?;
        Ok(match &id {
            Some(id) => OsAction::L7PolicyUpdate { id: id.clone(), spec },
            None => OsAction::L7PolicyCreate { listener_id: req(v, "listener", K::Listener, lang)?, spec },
        })
    })
}

/// *Add L7 Rule* / *Edit L7 Rule* (roadmap 54). `policy`: the policy it goes
/// to (from the policy's menu, or an edit); `None` offers a choice.
pub fn l7_rule_dialog(lang: Lang, d: &LoadBalancerDetail, policy: Option<&LbL7Policy>, existing: Option<&LbL7Rule>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let mut fields = vec![];
    if policy.is_none() {
        let policies = refs_of(&d.l7policies, |p| (p.id.clone(), format!("{} ({})", p.name, p.listener.name)));
        fields.push(Field::select("policy", tr(K::L7Policy), static_refs(&policies)).required());
    }
    let keyed: fn(&Vals) -> bool = |v| matches!(v.get("rule_type").map(String::as_str), Some("HEADER" | "COOKIE"));
    fields.extend([
        Field::select("rule_type", tr(K::RuleType), options_raw(&LB_L7_RULE_TYPES)).value(existing.map_or("PATH".into(), |r| r.rule_type.clone())).required(),
        Field::select("compare_type", tr(K::CompareType), options_raw(&LB_L7_COMPARE_TYPES)).value(existing.map_or("STARTS_WITH".into(), |r| r.compare_type.clone())).required(),
        Field::text("key", tr(K::L7Key)).value(existing.and_then(|r| r.key.clone()).unwrap_or_default()).help(tr(K::L7KeyHelp)).required().show_if(keyed),
        Field::text("value", tr(K::Value)).value(existing.map(|r| r.value.clone()).unwrap_or_default()).required(),
        Field::check("invert", tr(K::Invert), existing.is_some_and(|r| r.invert)).help(tr(K::InvertHelp)),
        Field::check("admin_up", tr(K::EnableAdminState), existing.is_none_or(|r| r.admin_up)),
    ]);
    let (policy_id, id) = (policy.map(|p| p.id.clone()), existing.map(|r| r.id.clone()));
    let title = tr(if existing.is_some() { K::EditL7Rule } else { K::AddL7Rule });
    Dialog::form(title.clone(), title, fields, move |v| {
        let rule_type = req(v, "rule_type", K::RuleType, lang)?;
        let keyed = matches!(rule_type.as_str(), "HEADER" | "COOKIE");
        let spec = LbL7RuleSpec {
            compare_type: req(v, "compare_type", K::CompareType, lang)?,
            key: v.opt("key").filter(|_| keyed),
            value: req(v, "value", K::Value, lang)?,
            invert: v.flag("invert"),
            admin_up: v.flag("admin_up"),
            rule_type,
        };
        spec.check().map_err(|e| invalid(lang, e))?;
        let policy_id = match &policy_id {
            Some(p) => p.clone(),
            None => req(v, "policy", K::L7Policy, lang)?,
        };
        Ok(match &id {
            Some(id) => OsAction::L7RuleUpdate { policy_id, id: id.clone(), spec },
            None => OsAction::L7RuleCreate { policy_id, spec },
        })
    })
}

/// An L7 policy's menu: edit, add a rule, delete (its rules go with it).
pub fn l7_policy_items(lang: Lang, d: &LoadBalancerDetail, p: &LbL7Policy) -> Vec<MenuItem> {
    let (d1, p1, d2, p2) = (d.clone(), p.clone(), d.clone(), p.clone());
    let (i, n) = (p.id.clone(), p.name.clone());
    vec![
        MenuItem::dialog(t(lang, K::EditL7Policy), move || l7_policy_dialog(lang, &d1, Some(&p1))),
        MenuItem::dialog(t(lang, K::AddL7Rule), move || l7_rule_dialog(lang, &d2, Some(&p2), None)),
        MenuItem::danger(t(lang, K::DeleteL7Policy), move || Dialog::confirm(lang, t(lang, K::DeleteL7Policy), n.clone(), OsAction::L7PolicyDelete { id: i.clone() })),
    ]
}

/// An L7 rule's menu (`p`: its policy).
pub fn l7_rule_items(lang: Lang, d: &LoadBalancerDetail, p: &LbL7Policy, r: &LbL7Rule) -> Vec<MenuItem> {
    let (dd, pp, rr) = (d.clone(), p.clone(), r.clone());
    let (pid, id, label) = (p.id.clone(), r.id.clone(), format!("{} {} {}", r.rule_type, r.compare_type, r.value));
    vec![
        MenuItem::dialog(t(lang, K::EditL7Rule), move || l7_rule_dialog(lang, &dd, Some(&pp), Some(&rr))),
        MenuItem::danger(t(lang, K::DeleteL7Rule), move || {
            Dialog::confirm(lang, t(lang, K::DeleteL7Rule), label.clone(), OsAction::L7RuleDelete { policy_id: pid.clone(), id: id.clone() })
        }),
    ]
}

/// *Add Rule* / *Edit Rule* for a floating IP's port forwarding. `ranges`: the
/// cloud accepts `8080:8090` (port-ranges extension).
pub fn port_forwarding_dialog(lang: Lang, floating_ip_id: &str, rule: Option<&PortForwardingRow>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let ranges = needs::PORT_FORWARDING_RANGES.met(&caps());
    let port_help = tr(if ranges { K::PortOrRangeHelp } else { K::PortHelp });
    let fields = vec![
        Field::select("protocol", tr(K::Protocol), options_raw(&["tcp", "udp"])).value(rule.map_or("tcp".into(), |r| r.protocol.clone())).required(),
        Field::select("internal", tr(K::InternalIpAddress), Source::Set(ChoiceSet::ForwardablePorts))
            .value(rule.map(|r| format!("{}/{}", r.internal_port_id, r.internal_ip)).unwrap_or_default())
            .required(),
        Field::text("internal_port", tr(K::InternalPort)).value(rule.map(|r| r.internal.clone()).unwrap_or_default()).help(port_help.clone()).required(),
        Field::text("external_port", tr(K::ExternalPort)).value(rule.map(|r| r.external.clone()).unwrap_or_default()).help(port_help).required(),
        Field::text("description", tr(K::Description)).value(rule.and_then(|r| r.description.clone()).unwrap_or_default()),
    ];
    let (fip, id) = (floating_ip_id.to_string(), rule.map(|r| r.id.clone()));
    let title = tr(if rule.is_some() { K::EditRule } else { K::AddRule });
    Dialog::form(title.clone(), title, fields, move |v| {
        let internal = req(v, "internal", K::InternalIpAddress, lang)?;
        let (port_id, ip) = internal.split_once('/').ok_or_else(|| t(lang, K::InternalIpAddress).to_string())?;
        let spec = PortForwardingSpec {
            floating_ip_id: fip.clone(),
            protocol: req(v, "protocol", K::Protocol, lang)?,
            internal_port_id: port_id.to_string(),
            internal_ip: ip.to_string(),
            internal: req(v, "internal_port", K::InternalPort, lang)?,
            external: req(v, "external_port", K::ExternalPort, lang)?,
            description: v.opt("description"),
            ranges,
        };
        spec.ports().map_err(|_| t(lang, if ranges { K::PortOrRangeHelp } else { K::PortHelp }).to_string())?;
        Ok(match &id {
            Some(id) => OsAction::PortForwardingUpdate { id: id.clone(), spec },
            None => OsAction::PortForwardingCreate(spec),
        })
    })
}

/// A port forwarding's menu (edit, delete).
pub fn port_forwarding_items(lang: Lang, floating_ip_id: &str, r: &PortForwardingRow) -> Vec<MenuItem> {
    let (f, rule) = (floating_ip_id.to_string(), r.clone());
    let edit = MenuItem::dialog(t(lang, K::EditRule), move || port_forwarding_dialog(lang, &f, Some(&rule)));
    let (f, id) = (floating_ip_id.to_string(), r.id.clone());
    let what = format!("{} {} → {}:{}", r.protocol.to_uppercase(), r.external, r.internal_ip, r.internal);
    let delete = MenuItem::danger(t(lang, K::DeleteRule), move || {
        Dialog::confirm(lang, t(lang, K::DeleteRule), what.clone(), OsAction::PortForwardingDelete { floating_ip_id: f.clone(), id: id.clone() })
    });
    vec![edit, delete]
}

/// A floating IP's menu: associate or disassociate, port forwarding rules, release.
pub fn floating_ip_items(lang: Lang, f: &FloatingIpRow) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if f.fixed_ip.is_some() {
        let (i, ip) = (f.id.clone(), f.ip.clone());
        items.push(MenuItem::danger(t(lang, K::Disassociate), move || {
            Dialog::confirm(lang, t(lang, K::Disassociate), ip.clone(), OsAction::FloatingIpDisassociate { id: i.clone() })
        }));
    } else {
        let i = f.id.clone();
        items.push(MenuItem::dialog(t(lang, K::Associate), move || {
            let i = i.clone();
            Dialog::form(
                t(lang, K::Associate),
                t(lang, K::Associate),
                vec![Field::select("port", t(lang, K::PortToAssociate), Source::Set(ChoiceSet::AssociablePorts)).required()],
                move |v| Ok(OsAction::FloatingIpAssociate { id: i.clone(), port_id: req(v, "port", K::PortToAssociate, lang)? }),
            )
        }));
    }
    // In the dropdown: Associate / Disassociate stays the main button.
    if needs::PORT_FORWARDING.met(&caps()) {
        items.push(MenuItem::link(t(lang, K::PortForwardingRules), format!("/project/floating_ips/{}?tab=forwarding", f.id)));
    }
    let (i, ip) = (f.id.clone(), f.ip.clone());
    items.push(MenuItem::danger(t(lang, K::ReleaseIp), move || {
        Dialog::confirm(lang, t(lang, K::ReleaseIp), ip.clone(), OsAction::FloatingIpRelease { id: i.clone() })
    }));
    items
}

// ---------------------------------------------------------------------------
// Project › Quotas (roadmap 82)
// ---------------------------------------------------------------------------

/// A service's card title on Project › Quotas.
pub fn quota_service_label(s: QuotaService) -> K {
    match s {
        QuotaService::Compute => K::Compute,
        QuotaService::Volume => K::VolumesGroup,
        QuotaService::Network => K::NetworkGroup,
        QuotaService::LoadBalancer => K::LoadBalancers,
        QuotaService::KeyManager => K::KeyManager,
    }
}

/// A quota's label (Horizon's *Modify Quotas* wording). Cinder's per-type keys
/// read "Volumes (ceph)"; a key Moonlight doesn't know is shown as the API names it.
pub fn quota_label(lang: Lang, s: QuotaService, key: &str) -> String {
    use QuotaService::*;
    let k = match (s, key) {
        (Compute, "instances") => K::Instances,
        (Compute, "cores") => K::Vcpus,
        (Compute, "ram") => K::Ram,
        (Compute, "key_pairs") => K::KeyPairs,
        (Compute, "server_groups") => K::ServerGroups,
        (Compute, "server_group_members") => K::QServerGroupMembers,
        (Compute, "metadata_items") => K::QMetadataItems,
        (Compute, "injected_files") => K::QInjectedFiles,
        (Compute, "injected_file_content_bytes") => K::QInjectedFileContent,
        (Compute, "injected_file_path_bytes") => K::QInjectedFilePath,
        (Volume, "volumes") => K::Volumes,
        (Volume, "snapshots") => K::VolumeSnapshots,
        (Volume, "gigabytes") => K::QGigabytes,
        (Volume, "backups") => K::QBackups,
        (Volume, "backup_gigabytes") => K::QBackupGigabytes,
        (Volume, "groups") => K::QVolumeGroups,
        (Volume, "per_volume_gigabytes") => K::QPerVolumeGigabytes,
        (Network, "network") => K::Networks,
        (Network, "subnet") => K::QSubnets,
        (Network, "subnetpool") => K::QSubnetPools,
        (Network, "port") => K::Ports,
        (Network, "router") => K::Routers,
        (Network, "floatingip") => K::FloatingIps,
        (Network, "security_group") => K::SecurityGroups,
        (Network, "security_group_rule") => K::SecurityGroupRules,
        (Network, "rbac_policy") => K::QRbacPolicies,
        (Network, "trunk") => K::Trunks,
        (LoadBalancer, "load_balancer") => K::LoadBalancers,
        (LoadBalancer, "listener") => K::Listeners,
        (LoadBalancer, "pool") => K::Pools,
        (LoadBalancer, "member") => K::Members,
        (LoadBalancer, "health_monitor") => K::HealthMonitors,
        (LoadBalancer, "l7policy") => K::L7Policies,
        (LoadBalancer, "l7rule") => K::L7Rules,
        (KeyManager, "secrets") => K::Secrets,
        (KeyManager, "containers") => K::Containers,
        (KeyManager, "orders") => K::QOrders,
        (KeyManager, "consumers") => K::Consumers,
        (KeyManager, "cas") => K::QCertificateAuthorities,
        (Volume, _) => {
            let per_type = [("volumes_", K::Volumes), ("gigabytes_", K::QGigabytes), ("snapshots_", K::VolumeSnapshots)];
            return match per_type.iter().find_map(|(p, k)| key.strip_prefix(p).map(|ty| (k, ty))) {
                Some((k, ty)) => format!("{} ({ty})", t(lang, *k)),
                None => key.to_string(),
            };
        }
        _ => return key.to_string(),
    };
    t(lang, k).to_string()
}

/// *Edit Quotas* of one service: a number per editable quota the service
/// reported, sending only what changed. Shown to everyone: whether the user may
/// is the cloud's policy, unknown beforehand (item 35), so a 403 gets its own
/// message instead.
pub fn edit_quotas_dialog(lang: Lang, card: &QuotaCard) -> Dialog {
    let project_id = card.project_id.clone();
    let title = format!("{}: {}", t(lang, K::EditQuotas), t(lang, quota_service_label(card.service)));
    quotas_form(lang, card, title, move |service, values| OsAction::QuotaUpdate { service, project_id: project_id.clone(), values }).forbidden(t(lang, K::QuotaForbidden))
}

/// Admin › Defaults' *Update Defaults* (roadmap 85): the same form over the
/// `default` quota class of Compute or Volume.
pub fn update_defaults_dialog(lang: Lang, card: &QuotaCard) -> Dialog {
    let title = format!("{}: {}", t(lang, K::UpdateDefaults), t(lang, quota_service_label(card.service)));
    let mut d = quotas_form(lang, card, title, |service, values| OsAction::DefaultQuotaUpdate { service, values });
    // Defaults have no usage: Project's "can't be below what is used" doesn't apply.
    d.intro = vec![t(lang, K::DefaultQuotaHelp).into()];
    d
}

/// One number per limit `card` shows (`-1` unlimited); `make` gets the changed
/// ones only, and an unchanged form is refused.
fn quotas_form(lang: Lang, card: &QuotaCard, title: String, make: impl Fn(QuotaService, std::collections::BTreeMap<String, i64>) -> OsAction + Send + Sync + 'static) -> Dialog {
    let service = card.service;
    let editable: Vec<&'static str> = service.keys().iter().copied().filter(|k| card.rows.iter().any(|r| r.key == *k)).collect();
    let fields = editable
        .iter()
        .map(|k| {
            let row = card.rows.iter().find(|r| r.key == *k).expect("filtered above");
            let mut label = quota_label(lang, service, k);
            if service.unit(k) != QuotaUnit::Count {
                label = format!("{label} ({})", if service.unit(k) == QuotaUnit::Mib { "MiB" } else { "GiB" });
            }
            Field::number(k, label, -1).value(row.limit.unwrap_or(-1).to_string())
        })
        .collect();
    let rows = card.rows.clone();
    let mut d = Dialog::form(title, t(lang, K::Save), fields, move |v| {
        let mut wanted = Vec::new();
        for k in &editable {
            if let Some(n) = v.num(k).map_err(|_| format!("{}: ?", quota_label(lang, service, k)))? {
                wanted.push((k.to_string(), n));
            }
        }
        let values = quota_changes(&rows, &wanted).map_err(|e| {
            // `key: …` → the label the user saw.
            let (k, rest) = e.split_once(':').unwrap_or(("", &e));
            format!("{}:{rest}", quota_label(lang, service, k))
        })?;
        if values.is_empty() {
            return Err(t(lang, K::NothingChanged).into());
        }
        Ok(make(service, values))
    });
    d.intro = vec![t(lang, K::QuotaLimitHelp).into()];
    d
}

// ---------------------------------------------------------------------------
// Alarming (Aodh, roadmap 79)
// ---------------------------------------------------------------------------

/// The selected alarm type, while creating.
fn alarm_type(v: &Vals) -> &str {
    v.get("type").map(String::as_str).unwrap_or("")
}
fn alarm_compares(v: &Vals) -> bool {
    let t = alarm_type(v);
    t == "prometheus" || t.starts_with("gnocchi_")
}
fn alarm_gnocchi(v: &Vals) -> bool {
    alarm_type(v).starts_with("gnocchi_")
}
/// Types with one metric name and a resource type.
fn alarm_metric(v: &Vals) -> bool {
    matches!(alarm_type(v), "gnocchi_resources_threshold" | "gnocchi_aggregation_by_resources_threshold")
}

/// The fields' types: `(field, types it belongs to, visibility while creating)`.
type AlarmFieldTypes = (&'static [&'static str], fn(&Vals) -> bool);
const COMPARES: AlarmFieldTypes = (&["prometheus", "gnocchi_resources_threshold", "gnocchi_aggregation_by_metrics_threshold", "gnocchi_aggregation_by_resources_threshold"], alarm_compares);
const GNOCCHI: AlarmFieldTypes = (&["gnocchi_resources_threshold", "gnocchi_aggregation_by_metrics_threshold", "gnocchi_aggregation_by_resources_threshold"], alarm_gnocchi);
const METRIC: AlarmFieldTypes = (&["gnocchi_resources_threshold", "gnocchi_aggregation_by_resources_threshold"], alarm_metric);

/// *Create Alarm* (`spec` = `None`) and *Edit Alarm*. The fields follow the
/// type; on edit the type is fixed and only its fields are shown. Aodh's PUT
/// replaces the whole alarm, so an edit starts from the alarm's [`AlarmSpec`]
/// (its time constraints, which the form doesn't edit, go back unchanged).
pub fn alarm_dialog(lang: Lang, edit: Option<(&str, &AlarmSpec)>) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let base = edit.map(|(_, s)| s.clone()).unwrap_or_else(|| AlarmSpec {
        alarm_type: "prometheus".into(),
        severity: "low".into(),
        enabled: true,
        comparison: "gt".into(),
        resource_type: "instance".into(),
        aggregation: "mean".into(),
        granularity: Some(300),
        evaluation_periods: Some(1),
        event_type: "*".into(),
        ..Default::default()
    });
    let fixed = edit.map(|(_, s)| s.alarm_type.clone());
    // While creating, a field shows for its types; on edit, it is there only for the alarm's type.
    let of = |(types, show): AlarmFieldTypes, f: Field| -> Option<Field> {
        match &fixed {
            Some(ty) => types.contains(&ty.as_str()).then_some(f),
            None => Some(f.show_if(show)),
        }
    };
    let only = |ty: &'static str, f: Field| -> Option<Field> {
        match &fixed {
            Some(t) => (t == ty).then_some(f),
            None => Some(f.when("type", ty)),
        }
    };
    let num = |n: Option<i64>| n.map(|n| n.to_string()).unwrap_or_default();
    let types = options(
        &[
            ("prometheus", K::AlarmTypePrometheus),
            ("gnocchi_resources_threshold", K::AlarmTypeGnocchiResource),
            ("gnocchi_aggregation_by_metrics_threshold", K::AlarmTypeGnocchiMetrics),
            ("gnocchi_aggregation_by_resources_threshold", K::AlarmTypeGnocchiResources),
            ("event", K::AlarmTypeEvent),
        ],
        lang,
    );
    let comparisons = Source::Static(ALARM_COMPARISONS.iter().map(|c| (c.to_string(), format!("{} ({c})", comparison_symbol(c)))).collect());
    // An aggregation Gnocchi defines but the list doesn't have stays selectable.
    let mut aggregations: Vec<&str> = ALARM_AGGREGATIONS.to_vec();
    if !base.aggregation.is_empty() && !aggregations.contains(&base.aggregation.as_str()) {
        aggregations.push(&base.aggregation);
    }
    let lines = |v: &[String]| v.join("\n");
    let mut fields = vec![
        Field::text("name", tr(K::Name)).value(base.name.clone()).required().help(tr(K::AlarmNameHelp)),
        Field::text("description", tr(K::Description)).value(base.description.clone()).help(tr(K::AlarmDescriptionHelp)),
    ];
    if fixed.is_none() {
        fields.push(Field::select("type", tr(K::Type), types).value(base.alarm_type.clone()).required().help(tr(K::AlarmTypeHelp)).tip(tr(K::TipAlarmType)));
    }
    fields.extend(
        [
            only("prometheus", Field::area("promql", tr(K::Query)).value(base.query.clone()).required().help(tr(K::PromqlHelp)).tip(tr(K::TipPromql))),
            of(METRIC, Field::text("metric", tr(K::Metric)).value(base.metric.clone()).required().help(tr(K::AlarmMetricHelp)).tip(tr(K::TipMetric))),
            only("gnocchi_aggregation_by_metrics_threshold", Field::area("metrics", tr(K::MetricIds)).value(lines(&base.metrics)).required().help(tr(K::MetricIdsHelp)).tip(tr(K::TipMetricIds))),
            of(METRIC, Field::text("resource_type", tr(K::ResourceType)).value(base.resource_type.clone()).required().help(tr(K::AlarmResourceTypeHelp)).tip(tr(K::TipResourceType))),
            only("gnocchi_resources_threshold", Field::text("resource_id", tr(K::ResourceId)).value(base.resource_id.clone()).required().help(tr(K::AlarmResourceIdHelp)).tip(tr(K::TipResourceId))),
            only("gnocchi_aggregation_by_resources_threshold", Field::area("search", tr(K::Query)).value(base.query.clone()).required().help(tr(K::GnocchiSearchHelp)).tip(tr(K::TipGnocchiSearch))),
            of(GNOCCHI, Field::select("aggregation", tr(K::Aggregation), options_raw(&aggregations)).value(base.aggregation.clone()).required().help(tr(K::AggregationHelp)).tip(tr(K::TipAggregation))),
            of(COMPARES, Field::select("comparison", tr(K::Comparison), comparisons).value(base.comparison.clone()).required().help(tr(K::ComparisonHelp))),
            of(COMPARES, Field::text("threshold", tr(K::Threshold)).value(base.threshold.map(|t| t.to_string()).unwrap_or_default()).required().help(tr(K::ThresholdHelp)).tip(tr(K::TipThreshold))),
            of(GNOCCHI, Field::number("granularity", tr(K::GranularitySeconds), 1).value(num(base.granularity)).required().help(tr(K::GranularityHelp)).tip(tr(K::TipGranularity))),
            of(GNOCCHI, Field::number("evaluation_periods", tr(K::EvaluationPeriods), 1).value(num(base.evaluation_periods)).required().help(tr(K::EvaluationPeriodsHelp)).tip(tr(K::TipEvaluationPeriods))),
            only("event", Field::text("event_type", tr(K::EventType)).value(base.event_type.clone()).help(tr(K::EventTypeHelp)).tip(tr(K::TipEventType))),
            only("event", Field::area("event_query", tr(K::Query)).value(lines(&base.event_query)).help(tr(K::EventQueryHelp)).tip(tr(K::TipEventQuery))),
        ]
        .into_iter()
        .flatten(),
    );
    let severities = options(&[("low", K::SeverityLow), ("moderate", K::SeverityModerate), ("critical", K::SeverityCritical)], lang);
    fields.extend([
        Field::select("severity", tr(K::Severity), severities).value(base.severity.clone()).required().help(tr(K::SeverityHelp)),
        Field::check("enabled", tr(K::EnabledCol), base.enabled).help(tr(K::AlarmEnabledHelp)),
        Field::area("alarm_actions", tr(K::OnAlarm)).value(lines(&base.alarm_actions)).help(tr(K::AlarmActionsHelp)).tip(tr(K::TipAlarmActions)),
        Field::area("ok_actions", tr(K::OnOk)).value(lines(&base.ok_actions)).help(tr(K::OkActionsHelp)),
        Field::area("insufficient_data_actions", tr(K::OnInsufficientData)).value(lines(&base.insufficient_data_actions)).help(tr(K::InsufficientDataActionsHelp)),
        Field::check("repeat_actions", tr(K::RepeatActions), base.repeat_actions).help(tr(K::RepeatActionsHelp)),
    ]);
    let id = edit.map(|(id, _)| id.to_string());
    let title = tr(if id.is_some() { K::EditAlarm } else { K::CreateAlarm });
    let submit = if id.is_some() { tr(K::Save) } else { title.clone() };
    Dialog::form(title, submit, fields, move |v| {
        let alarm_type = fixed.clone().unwrap_or_else(|| v.get("type"));
        let bad = |e: String| invalid(lang, e);
        // Spanish users type decimal commas.
        let threshold = match v.opt("threshold") {
            None => None,
            Some(x) => Some(x.replace(',', ".").parse::<f64>().map_err(|_| bad(format!("{}: ?", t(lang, K::Threshold))))?),
        };
        let query = match alarm_type.as_str() {
            "prometheus" => v.get("promql"),
            "gnocchi_aggregation_by_resources_threshold" => v.get("search"),
            _ => String::new(),
        };
        let spec = AlarmSpec {
            name: req(v, "name", K::Name, lang)?,
            description: v.get("description"),
            severity: v.get("severity"),
            enabled: v.flag("enabled"),
            repeat_actions: v.flag("repeat_actions"),
            alarm_actions: v.lines("alarm_actions"),
            ok_actions: v.lines("ok_actions"),
            insufficient_data_actions: v.lines("insufficient_data_actions"),
            time_constraints: base.time_constraints.clone(),
            query,
            comparison: v.get("comparison"),
            threshold,
            metric: v.get("metric"),
            metrics: v.lines("metrics"),
            resource_type: v.get("resource_type"),
            resource_id: v.get("resource_id"),
            aggregation: v.get("aggregation"),
            granularity: v.num("granularity").map_err(bad)?,
            evaluation_periods: v.num("evaluation_periods").map_err(bad)?,
            event_type: v.get("event_type"),
            event_query: v.lines("event_query"),
            alarm_type,
        };
        spec.body().map_err(bad)?;
        Ok(match &id {
            Some(id) => OsAction::AlarmUpdate { id: id.clone(), spec },
            None => OsAction::AlarmCreate(spec),
        })
    })
    .sized(DialogSize::Medium)
}

/// An alarm's menu (list row and detail page): *Edit Alarm*.
pub fn alarm_items(lang: Lang, a: &AlarmRow) -> Vec<MenuItem> {
    let (id, spec) = (a.id.clone(), a.spec.clone());
    vec![MenuItem::dialog(t(lang, K::EditAlarm), move || alarm_dialog(lang, Some((&id, &spec))))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::dialog::Kind;
    use std::collections::HashMap;

    fn vals(pairs: &[(&str, &str)]) -> Values {
        Values(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<HashMap<_, _>>())
    }

    fn source_values(d: &Dialog) -> Vec<String> {
        let f = d.fields.iter().find(|f| f.name == "source").unwrap();
        let Kind::Select(Source::Static(opts)) = &f.kind else { panic!() };
        opts.iter().map(|(v, _)| v.clone()).collect()
    }

    #[test]
    fn create_image_offers_file_only_when_uploads_are_enabled() {
        use crate::prefs::{ImagePrefs, ImageUpload};
        let with = |upload, url: bool| ImagePrefs { upload, allow_location: url, ..Default::default() };
        let d = |p: &ImagePrefs, web_download| create_image_form(Lang::EnUs, p, false, web_download);
        assert_eq!(source_values(&d(&with(ImageUpload::Off, false), true)), ["none"], "nothing changes until an operator enables it");
        assert_eq!(source_values(&d(&with(ImageUpload::Proxy, false), true)), ["file", "none"]);
        assert_eq!(source_values(&d(&with(ImageUpload::Direct, true), true)), ["url", "file", "none"]);
        assert_eq!(source_values(&d(&with(ImageUpload::Proxy, true), false)), ["file", "none"], "Glance has no web-download");
    }

    #[test]
    fn create_image_catalog_leaves_out_the_keys_with_their_own_fields() {
        let d = create_image_form(Lang::EnUs, &Default::default(), false, true);
        let meta = d.fields.iter().find(|f| f.name == "metadata").unwrap();
        assert_eq!(meta.catalog, Some("OS::Glance::Image"));
        for key in ["description", "architecture", "kernel_id", "ramdisk_id"] {
            assert!(meta.catalog_skip.contains(&key), "{key} has its own field: offering it would be a repeated key");
        }
    }

    #[test]
    fn create_image_is_a_wizard_with_metadata_on_its_own_step() {
        let d = create_image_form(Lang::EnUs, &Default::default(), false, true);
        assert_eq!(d.steps.len(), 2);
        assert_eq!(d.steps[1].fields, ["metadata"], "metadata is the second step");
        let mut in_steps: Vec<_> = d.steps.iter().flat_map(|s| s.fields.clone()).collect();
        let mut all: Vec<_> = d.fields.iter().map(|f| f.name).collect();
        in_steps.sort();
        all.sort();
        assert_eq!(in_steps, all, "every field is on exactly one step");
        let half: Vec<_> = d.fields.iter().filter(|f| f.half).map(|f| f.name).collect();
        assert_eq!(half, ["source", "url", "file", "disk_format", "architecture", "min_disk", "min_ram", "visibility", "protected"], "pairs share a line");
    }

    #[test]
    fn create_image_from_a_file_carries_its_size_and_respects_the_limit() {
        use crate::prefs::{ImagePrefs, ImageUpload};
        let prefs = ImagePrefs { upload: ImageUpload::Proxy, max_upload_gb: 1, ..Default::default() };
        let d = create_image_form(Lang::EnUs, &prefs, false, true);
        let picked = |size: u64| Picked { name: "debian.qcow2".into(), size }.encode();
        let base = |file: &str| {
            vec![("name", "Debian".to_string()), ("source", "file".into()), ("file", file.to_string()), ("disk_format", "qcow2".into()), ("visibility", "private".into()), ("min_disk", "0".into()), ("min_ram", "0".into())]
        };
        let build = |file: &str| {
            let owned = base(file);
            let pairs: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
            (d.build)(&vals(&pairs))
        };
        let Ok(OsAction::ImageCreate(spec)) = build(&picked(5 << 20)) else { panic!("a file within the limit builds") };
        assert_eq!((spec.upload_bytes, spec.url), (Some(5 << 20), None));
        assert!(build(&picked((1 << 30) + 1)).unwrap_err().contains("1 GiB"), "over [images] max_upload_gb");
        assert!(build("").is_err(), "no file picked");

        // The field shows the same problem before submitting, and picking a file sets its format.
        let check = d.check.as_ref().unwrap();
        let vals_of = |size: u64| HashMap::from([("source".to_string(), "file".to_string()), ("file".to_string(), picked(size))]);
        assert!(check(&vals_of((1 << 30) + 1), &Sets::new()).iter().any(|(f, _)| *f == "file"));
        assert!(check(&vals_of(1), &Sets::new()).is_empty());
        let mut v = HashMap::from([("file".to_string(), Picked { name: "disk.iso".into(), size: 1 }.encode())]);
        (d.on_change.as_ref().unwrap())("file", &mut v, &Sets::new());
        assert_eq!(v.get("disk_format").map(String::as_str), Some("iso"));
    }

    #[test]
    fn modify_quotas_is_for_admins_and_follows_edit_project() {
        use crate::components::dialog::ItemKind;
        let p = crate::model::IdentityProject {
            id: "p9".into(),
            name: "other".into(),
            description: None,
            domain: "Default".into(),
            enabled: true,
            current: false,
            member: false,
        };
        let labels = |admin| identity_project_items(Lang::EnUs, &p, None, admin).into_iter().map(|m| m.label).collect::<Vec<_>>();
        assert_eq!(labels(true), ["Edit Project", "Modify Quotas", "Delete Project"], "Horizon's order (roadmap 82)");
        assert!(!labels(false).iter().any(|l| l == "Modify Quotas"), "hidden without the admin role");
        let items = identity_project_items(Lang::EnUs, &p, None, true);
        assert!(matches!(&items[1].kind, ItemKind::Link(h) if h == "/identity/projects/p9/quotas"));
    }

    #[test]
    fn update_defaults_sends_the_changes_to_the_default_class() {
        let row = |key: &str, limit| QuotaRow { key: key.into(), used: None, reserved: None, limit };
        let card = QuotaCard { service: QuotaService::Volume, project_id: "p1".into(), rows: vec![row("volumes", Some(10)), row("gigabytes", Some(1000))], error: None };
        let d = update_defaults_dialog(Lang::EnUs, &card);
        assert!(!d.intro.concat().contains("used"), "defaults have no usage to stay above");
        assert!(d.title.starts_with("Update Defaults"), "{}", d.title);
        let a = (d.build)(&vals(&[("volumes", "10"), ("gigabytes", "-1")])).unwrap();
        assert_eq!(a, OsAction::DefaultQuotaUpdate { service: QuotaService::Volume, values: [("gigabytes".to_string(), -1)].into() }, "only what changed");
        assert!((d.build)(&vals(&[("volumes", "10"), ("gigabytes", "1000")])).is_err(), "nothing changed");
    }

    #[test]
    fn edit_quotas_sends_only_changes_and_names_the_quota() {
        let row = |key: &str, used, limit| QuotaRow { key: key.into(), used, reserved: None, limit };
        let card = QuotaCard {
            service: QuotaService::Compute,
            project_id: "p1".into(),
            rows: vec![row("instances", Some(4), Some(10)), row("ram", Some(2048), None), row("unknown_key", None, Some(1))],
            error: None,
        };
        let d = edit_quotas_dialog(Lang::EnUs, &card);
        let names: Vec<_> = d.fields.iter().map(|f| (f.name, f.default.as_str())).collect();
        assert_eq!(names, [("instances", "10"), ("ram", "-1")], "editable keys only; unlimited shown as -1");
        assert_eq!(d.fields[1].label, "RAM (MiB)");
        assert!(d.forbidden.as_deref().is_some_and(|m| m.contains("no permission to edit quotas")));
        let a = (d.build)(&vals(&[("instances", "12"), ("ram", "-1")])).unwrap();
        let OsAction::QuotaUpdate { service, project_id, values } = a else { panic!("{a:?}") };
        assert_eq!((service, project_id.as_str(), values.into_iter().collect::<Vec<_>>()), (QuotaService::Compute, "p1", vec![("instances".to_string(), 12)]));
        assert_eq!((d.build)(&vals(&[("instances", "10")])).unwrap_err(), "Nothing changed.");
        assert!((d.build)(&vals(&[("instances", "3")])).unwrap_err().starts_with("Instances: 3 is below"));
    }

    #[test]
    fn quota_labels_cover_per_type_and_unknown_keys() {
        assert_eq!(quota_label(Lang::EnUs, QuotaService::Volume, "gigabytes_ceph"), "Total Size of Volumes and Snapshots (ceph)");
        assert_eq!(quota_label(Lang::EsEs, QuotaService::Volume, "volumes_ssd"), "Volúmenes (ssd)");
        assert_eq!(quota_label(Lang::EnUs, QuotaService::Network, "new_thing"), "new_thing");
        for s in [QuotaService::Compute, QuotaService::Volume, QuotaService::Network, QuotaService::LoadBalancer, QuotaService::KeyManager] {
            for k in s.keys() {
                assert_ne!(quota_label(Lang::EnUs, s, k), *k, "{s:?} {k} has no label");
            }
        }
    }

    fn volume_group_row() -> VolumeGroupRow {
        VolumeGroupRow {
            id: "g1".into(),
            name: "db".into(),
            description: None,
            status: "available".into(),
            availability_zone: None,
            group_type: Ref { id: "gt".into(), name: "gt".into() },
            volume_types: vec![],
            has_snapshots: false,
            created: String::new(),
        }
    }

    /// The dialog behind the menu item labelled `label`.
    fn item_dialog(items: &[MenuItem], label: &str) -> Dialog {
        let item = items.iter().find(|m| m.label == label).unwrap_or_else(|| panic!("no {label}"));
        let crate::components::dialog::ItemKind::Dialog(open) = &item.kind else { panic!("{label} opens no dialog") };
        open()
    }

    fn backup_row(status: &str) -> BackupRow {
        BackupRow {
            id: "b1".into(),
            name: "nightly".into(),
            description: None,
            status: status.into(),
            size_gb: 5,
            volume: Ref { id: "v1".into(), name: "data".into() },
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
        let labels = |s: &str| backup_items(Lang::EnUs, &backup_row(s)).into_iter().map(|m| m.label).collect::<Vec<_>>();
        assert_eq!(labels("available"), ["Restore Backup", "Edit Backup", "Delete Volume Backup"]);
        assert_eq!(labels("creating"), ["Edit Backup", "Delete Volume Backup"], "nothing to restore yet");
        let items = backup_items(Lang::EnUs, &backup_row("available"));
        let d = item_dialog(&items, "Delete Volume Backup");
        assert!(d.danger && d.selection == ["nightly"]);
        let d = item_dialog(&items, "Restore Backup");
        assert_eq!((d.build)(&vals(&[("volume", ""), ("name", "copy")])).unwrap(), OsAction::BackupRestore { id: "b1".into(), volume_id: None, name: Some("copy".into()) }, "empty volume = new volume");
        assert_eq!((d.build)(&vals(&[("volume", "v9")])).unwrap(), OsAction::BackupRestore { id: "b1".into(), volume_id: Some("v9".into()), name: None });
    }

    #[test]
    fn create_backup_asks_for_the_volume_only_from_the_list() {
        let l = Lang::EnUs;
        let d = create_backup(l);
        assert!((d.build)(&vals(&[("name", "n")])).is_err(), "a volume is required");
        assert_eq!(
            (d.build)(&vals(&[("volume", "v1"), ("name", "n"), ("incremental", "true")])).unwrap(),
            OsAction::BackupCreate { volume_id: "v1".into(), name: "n".into(), description: None, incremental: true, force: false, snapshot_id: None }
        );
        let d = backup_form(l, Some("v2".into()), "x-backup".into(), true);
        assert!(d.fields.iter().all(|f| f.name != "volume"));
        assert_eq!(d.fields.iter().find(|f| f.name == "force").unwrap().default, "true", "attached volumes preselect Force");
    }

    #[test]
    fn deleting_a_volume_group_offers_to_delete_its_volumes() {
        let items = volume_group_items(Lang::EnUs, &volume_group_row());
        let d = item_dialog(&items, "Delete Group");
        assert_eq!(items.last().unwrap().label, "Delete Group", "delete last");
        assert!(d.danger && items.last().unwrap().danger, "destructive: red, with the confirmation tick");
        assert_eq!(d.selection, ["db"]);
        assert_eq!(d.fields[0].default, "false", "volumes are kept unless ticked");
        assert_eq!((d.build)(&vals(&[("delete_volumes", "true")])).unwrap(), OsAction::VolumeGroupDelete { id: "g1".into(), delete_volumes: true });
        assert_eq!((d.build)(&vals(&[])).unwrap(), OsAction::VolumeGroupDelete { id: "g1".into(), delete_volumes: false });
    }

    #[test]
    fn volume_group_dialogs_build_their_requests() {
        let l = Lang::EnUs;
        let d = create_volume_group(l);
        assert!((d.build)(&vals(&[("name", "db"), ("group_type", "gt1")])).is_err(), "at least one volume type");
        assert_eq!(
            (d.build)(&vals(&[("name", "db"), ("group_type", "gt1"), ("volume_types", "vt1,vt2"), ("az", "")])).unwrap(),
            OsAction::VolumeGroupCreate { name: "db".into(), description: None, group_type_id: "gt1".into(), volume_type_ids: vec!["vt1".into(), "vt2".into()], availability_zone: None }
        );

        let labels = |items: &[MenuItem]| items.iter().map(|m| m.label.clone()).collect::<Vec<_>>();
        let row = volume_group_row();
        assert_eq!(labels(&volume_group_items(l, &row)), ["Create Snapshot", "Edit Group", "Clone Group", "Delete Group"]);
        let clone = item_dialog(&volume_group_items(l, &row), "Clone Group");
        assert_eq!(clone.fields[0].default, "db-clone");
        assert_eq!(
            (clone.build)(&vals(&[("name", "db-clone")])).unwrap(),
            OsAction::VolumeGroupCreateFrom { name: "db-clone".into(), description: None, source_group_id: Some("g1".into()), group_snapshot_id: None }
        );

        // The detail page knows the volumes: Manage Volumes saves the difference.
        let vol = |id: &str| Volume { id: id.into(), name: format!("vol-{id}"), ..volume("available") };
        let detail = VolumeGroupDetail { row: row.clone(), volumes: vec![vol("v1"), vol("v2")], snapshots: vec![] };
        let items = volume_group_detail_items(l, &detail);
        assert_eq!(labels(&items), ["Create Snapshot", "Manage Volumes", "Edit Group", "Remove Volumes from Group", "Clone Group", "Delete Group"]);
        let manage = item_dialog(&items, "Manage Volumes");
        assert_eq!(manage.fields[0].default, "v1,v2", "current volumes preselected");
        assert_eq!(
            (manage.build)(&vals(&[("volumes", "v2,v3")])).unwrap(),
            OsAction::VolumeGroupSetVolumes { id: "g1".into(), add_volume_ids: vec!["v3".into()], remove_volume_ids: vec!["v1".into()] }
        );
        assert!((manage.build)(&vals(&[("volumes", "v1,v2")])).is_err(), "nothing changed");
        let remove = item_dialog(&items, "Remove Volumes from Group");
        assert_eq!(remove.selection, ["vol-v1", "vol-v2"]);
        assert!(!remove.danger, "the volumes are kept");
        let empty = VolumeGroupDetail { volumes: vec![], ..detail };
        assert!(!labels(&volume_group_detail_items(l, &empty)).contains(&"Remove Volumes from Group".to_string()));

        let gs = GroupSnapshotRow {
            id: "gs1".into(),
            name: "nightly".into(),
            description: None,
            status: "available".into(),
            group: Ref { id: "g1".into(), name: "db".into() },
            group_type: Ref { id: "gt".into(), name: "gt".into() },
            created: String::new(),
        };
        let from = item_dialog(&group_snapshot_items(l, &gs), "Create Group");
        assert_eq!(
            (from.build)(&vals(&[("name", "restored")])).unwrap(),
            OsAction::VolumeGroupCreateFrom { name: "restored".into(), description: None, source_group_id: None, group_snapshot_id: Some("gs1".into()) }
        );
    }

    #[test]
    fn bulk_delete_lists_every_name_and_batches_in_order() {
        let items = vec![("v1".to_string(), "data".to_string()), ("v2".to_string(), "logs".to_string())];
        let d = bulk_delete(Lang::EnUs, K::DeleteVolumes, items, |id| OsAction::VolumeDelete { id });
        assert!(d.danger);
        assert_eq!(d.selection, ["data", "logs"], "one bullet per name");
        let OsAction::Batch(b) = (d.build)(&vals(&[])).unwrap() else { panic!() };
        assert_eq!(b[1], ("logs".to_string(), OsAction::VolumeDelete { id: "v2".into() }));
        let many = (0..=MAX_BATCH).map(|i| (i.to_string(), i.to_string())).collect();
        let d = bulk_delete(Lang::EnUs, K::DeleteVolumes, many, |id| OsAction::VolumeDelete { id });
        assert!((d.build)(&vals(&[])).is_err());
    }

    #[test]
    fn import_key_pair_sends_the_chosen_type() {
        let d = import_keypair_dialog(Lang::EnUs);
        let pem = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----";
        let a = (d.build)(&vals(&[("name", "web"), ("key_type", "x509"), ("public_key", pem)])).unwrap();
        assert_eq!(a, OsAction::KeyPairImport { name: "web".into(), public_key: pem.into(), key_type: "x509".into() });
        assert!((d.build)(&vals(&[("name", "web"), ("key_type", "ssh"), ("public_key", pem)])).is_err(), "checked before sending");
    }

    #[test]
    fn load_balancer_wizard_builds_one_spec_and_skips_what_is_switched_off() {
        let d = create_load_balancer_dialog(Lang::EnUs);
        assert_eq!(d.steps.len(), 5);
        let full = vals(&[
            ("name", "web-lb"), ("vip_subnet", "sub-1"), ("with_listener", "true"), ("listener_protocol", "HTTPS"),
            ("listener_port", "443"), ("with_pool", "true"), ("pool_name", "web-pool"), ("algorithm", "LEAST_CONNECTIONS"),
            ("persistence", ""),
            ("members", r#"[{"address":"10.0.0.11","subnet_id":"sub-1","label":"web-1: 10.0.0.11","port":"8443","weight":"1"},
                           {"address":"192.0.2.10","subnet_id":"sub-ext","port":"9443","weight":"","backup":true,"monitor_port":"9000"}]"#),
            ("with_monitor", "true"), ("monitor_type", "TCP"), ("delay", "5"), ("timeout", "3"), ("max_retries", "3"),
            ("http_method", "GET"), ("url_path", "/"), ("expected_codes", "200"),
        ]);
        let OsAction::LoadBalancerCreate(spec) = (d.build)(&full).unwrap() else { panic!("not a create") };
        assert_eq!(spec.pool.as_ref().map(|p| p.protocol.as_str()), Some("HTTPS"), "pool protocol follows the listener");
        assert_eq!(spec.pool.as_ref().and_then(|p| p.name.as_deref()), Some("web-pool"));
        let field = |n: &str| d.fields.iter().find(|f| f.name == n).unwrap();
        assert_eq!((field("pool_name").default.as_str(), field("pool_name").required), ("pool-1", true), "Octavia needs a pool name");
        let Kind::Select(Source::Static(persistence)) = &field("persistence").kind else { panic!("not a static select") };
        assert!(persistence.iter().all(|(v, _)| !v.is_empty()), "the select adds *— None —* itself");
        let mut unnamed = full.clone();
        unnamed.0.insert("pool_name".into(), String::new());
        assert!((d.build)(&unnamed).is_err(), "an empty pool name is refused before sending");
        assert_eq!(spec.members.len(), 2);
        assert_eq!((spec.members[0].address.as_str(), spec.members[0].port, spec.members[0].subnet_id.as_deref()), ("10.0.0.11", 8443, Some("sub-1")));
        let ext = &spec.members[1];
        assert_eq!((ext.address.as_str(), ext.port, ext.weight, ext.backup, ext.monitor_port), ("192.0.2.10", 9443, None, true, Some(9000)), "an external member with its own port (roadmap 54)");
        assert!(ext.admin_up, "a row without admin state is enabled");
        let mut twice = full.clone();
        twice.0.insert("members".into(), r#"[{"address":"10.0.0.11","port":"80","weight":"1"},{"address":"10.0.0.11","port":"80","weight":"1"}]"#.into());
        assert!((d.build)(&twice).unwrap_err().contains("listed twice"));
        let m = spec.monitor.unwrap();
        assert_eq!((m.kind.as_str(), m.http_method.clone()), ("TCP", None), "HTTP fields only for HTTP monitors");

        let bare = vals(&[("vip_subnet", "sub-1"), ("with_listener", "false"), ("with_pool", "true"), ("members", r#"[{"address":"10.0.0.11","port":"80","weight":"1"}]"#)]);
        let OsAction::LoadBalancerCreate(spec) = (d.build)(&bare).unwrap() else { panic!("not a create") };
        assert!(spec.listener.is_none() && spec.pool.is_none() && spec.members.is_empty() && spec.monitor.is_none());
        assert!((d.build)(&vals(&[("with_listener", "false")])).is_err(), "a subnet is required");
    }

    fn lb_with_members() -> LoadBalancerDetail {
        let m = |id: &str, address: &str| {
            serde_json::from_value::<LbMember>(serde_json::json!({
                "id": id, "name": id, "pool": {"id": "pool-1", "name": "web"}, "address": address, "port": 80, "weight": 3,
                "subnet": {"id": "sub-1", "name": "s"}, "backup": false, "operating_status": "ONLINE",
                "provisioning_status": "ACTIVE", "admin_up": true, "monitor_address": "10.0.9.9", "monitor_port": 9000
            }))
            .unwrap()
        };
        serde_json::from_value(serde_json::json!({
            "row": {"id": "lb1", "name": "lb", "description": null, "vip_address": null, "availability_zone": null,
                    "operating_status": "ONLINE", "provisioning_status": "ACTIVE", "admin_up": true, "provider": null},
            "project_id": "p", "flavor_id": null, "vip_port_id": null, "vip_network": null, "vip_subnet": null,
            "floating_ip": null, "created": null, "updated": null, "listeners": [],
            "pools": [{"id": "pool-1", "name": "web", "protocol": "HTTP", "algorithm": "ROUND_ROBIN", "session_persistence": null,
                       "listeners": [], "members": 2, "monitor": null, "operating_status": "ONLINE",
                       "provisioning_status": "ACTIVE", "admin_up": true}],
            "members": [m("m1", "10.0.0.5"), m("m2", "10.0.0.6"), serde_json::to_value(LbMember { pool: Ref { id: "other".into(), name: "o".into() }, ..m("m3", "10.0.0.7") }).unwrap()],
            "monitors": []
        }))
        .unwrap()
    }

    #[test]
    fn edit_members_starts_from_the_pool_and_sends_one_batch() {
        let d = lb_with_members();
        let dlg = pool_members_dialog(Lang::EnUs, &d, &d.pools[0]);
        assert!(dlg.danger, "the batch deletes members left out");
        let rows: Vec<MemberRow> = serde_json::from_str(&dlg.fields[0].default).unwrap();
        assert_eq!(rows.iter().map(|r| r.address.as_str()).collect::<Vec<_>>(), ["10.0.0.5", "10.0.0.6"], "only this pool's members");
        assert_eq!((rows[0].weight.as_str(), rows[0].monitor_port.as_str()), ("3", "9000"), "kept, so saving doesn't reset them");
        let Kind::MemberRows { existing, .. } = &dlg.fields[0].kind else { panic!("not a members editor") };
        assert_eq!(existing.len(), 2);
        let keep_one = serde_json::to_string(&rows[..1]).unwrap();
        let OsAction::PoolSetMembers { pool_id, members } = (dlg.build)(&vals(&[("members", &keep_one)])).unwrap() else { panic!("not a batch") };
        assert_eq!((pool_id.as_str(), members.len(), members[0].monitor_address.as_deref()), ("pool-1", 1, Some("10.0.9.9")));
    }

    /// Roadmap 54, slice 2: the *Advanced* box opens on a listener that has
    /// options, *Insert Headers* only for HTTP, and unticked leaves them alone.
    #[test]
    fn listener_and_pool_dialogs_carry_the_advanced_options() {
        let mut d = lb_with_members();
        let listener = |protocol: &str, options: serde_json::Value| -> LbListener {
            serde_json::from_value(serde_json::json!({
                "id": "ls1", "name": "web", "protocol": protocol, "port": 80, "default_pool": null, "connection_limit": null,
                "operating_status": "ONLINE", "provisioning_status": "ACTIVE", "admin_up": true, "options": options
            }))
            .unwrap()
        };
        let http = listener("HTTP", serde_json::json!({"timeout_client_data": 60000, "insert_headers": ["X-Forwarded-For"], "allowed_cidrs": []}));
        d.listeners = vec![http.clone()];
        let dlg = listener_dialog(Lang::EnUs, &d, Some(&http));
        let field = |dlg: &Dialog, n: &str| dlg.fields.iter().find(|f| f.name == n).map(|f| f.default.clone());
        assert_eq!(field(&dlg, "lb_advanced").as_deref(), Some("true"), "options set: open");
        assert_eq!((field(&dlg, "timeout_client_data").as_deref(), field(&dlg, "insert_headers").as_deref()), (Some("60000"), Some("X-Forwarded-For")));
        let base = [("name", "web"), ("description", ""), ("connection_limit", ""), ("default_pool", ""), ("admin_up", "true")];
        let with = |extra: &[(&str, &str)]| vals(&base.iter().chain(extra).copied().collect::<Vec<_>>());
        let OsAction::ListenerUpdate { options, protocol, .. } =
            (dlg.build)(&with(&[("lb_advanced", "true"), ("timeout_client_data", ""), ("insert_headers", "X-Forwarded-Proto"), ("allowed_cidrs", "10.0.0.0/8\n")])).unwrap()
        else {
            panic!("not an update")
        };
        let o = options.expect("ticked: sent");
        assert_eq!((protocol.as_str(), o.timeout_client_data, o.insert_headers, o.allowed_cidrs), ("HTTP", None, vec!["X-Forwarded-Proto".to_string()], Some(vec!["10.0.0.0/8".to_string()])));
        let OsAction::ListenerUpdate { options, .. } = (dlg.build)(&with(&[("lb_advanced", "false")])).unwrap() else { panic!() };
        assert_eq!(options, None, "unticked: left alone");

        let tcp = listener("TCP", serde_json::json!({}));
        let dlg = listener_dialog(Lang::EnUs, &d, Some(&tcp));
        assert_eq!(field(&dlg, "insert_headers"), None, "TCP listeners take no headers");
        assert_eq!(field(&dlg, "allowed_cidrs"), None, "not reported by the listener: Octavia < 2.12");
        assert_eq!(field(&dlg, "lb_advanced").as_deref(), Some("false"));

        let pool = d.pools[0].clone();
        let dlg = pool_dialog(Lang::EnUs, &d, Some(&pool));
        let pool_vals = |extra: &[(&str, &str)]| {
            vals(&[("name", "web"), ("description", ""), ("algorithm", "ROUND_ROBIN"), ("persistence", ""), ("admin_up", "true")].iter().chain(extra).copied().collect::<Vec<_>>())
        };
        let OsAction::PoolUpdate { tls, .. } = (dlg.build)(&pool_vals(&[("tls_enabled", "true"), ("tls_ciphers", "AES256")])).unwrap() else { panic!() };
        assert_eq!(tls, Some((true, Some("AES256".to_string()))));
        assert!((dlg.build)(&pool_vals(&[("tls_enabled", "false"), ("tls_ciphers", "AES256")])).is_ok(), "hidden ciphers are dropped");
    }

    /// Roadmap 54, slice 4: policies only on HTTP listeners; the policy's own
    /// menu presets it on a new rule; the key is dropped for keyless types.
    #[test]
    fn l7_dialogs_build_policies_and_rules() {
        let mut d = lb_with_members();
        let listener = |id: &str, protocol: &str| -> LbListener {
            serde_json::from_value(serde_json::json!({
                "id": id, "name": id, "protocol": protocol, "port": 80, "default_pool": null, "connection_limit": null,
                "operating_status": "ONLINE", "provisioning_status": "ACTIVE", "admin_up": true
            }))
            .unwrap()
        };
        d.listeners = vec![listener("http", "HTTP"), listener("tcp", "TCP"), listener("tls", "TERMINATED_HTTPS")];
        assert_eq!(l7_listeners(&d).iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["http", "tls"]);
        let dlg = l7_policy_dialog(Lang::EnUs, &d, None);
        let OsAction::L7PolicyCreate { listener_id, spec } = (dlg.build)(&vals(&[
            ("listener", "http"), ("name", "api"), ("description", ""), ("action", "REDIRECT_TO_POOL"),
            ("redirect_pool", "pool-1"), ("redirect_url", ""), ("position", "2"), ("admin_up", "true"),
        ]))
        .unwrap()
        else {
            panic!("not a create")
        };
        assert_eq!((listener_id.as_str(), spec.position, spec.redirect_pool_id.as_deref()), ("http", Some(2), Some("pool-1")));

        let policy: LbL7Policy = serde_json::from_value(serde_json::json!({
            "id": "pol-1", "name": "api", "description": null, "listener": {"id": "http", "name": "http"}, "action": "REJECT",
            "position": 1, "redirect_pool": null, "redirect_url": null, "operating_status": "ONLINE",
            "provisioning_status": "ACTIVE", "admin_up": true, "rules": []
        }))
        .unwrap();
        let dlg = l7_rule_dialog(Lang::EnUs, &d, Some(&policy), None);
        assert!(dlg.fields.iter().all(|f| f.name != "policy"), "preset from the policy's menu");
        let OsAction::L7RuleCreate { policy_id, spec } = (dlg.build)(&vals(&[
            ("rule_type", "PATH"), ("compare_type", "STARTS_WITH"), ("key", "stale"), ("value", "/api"), ("invert", "false"), ("admin_up", "true"),
        ]))
        .unwrap()
        else {
            panic!("not a create")
        };
        assert_eq!((policy_id.as_str(), spec.key), ("pol-1", None), "a hidden key isn't sent");
    }

    /// Roadmap 54: TLS listeners move the default port to 443 and back, but a
    /// typed port stays.
    #[test]
    fn the_port_follows_the_protocol_until_typed() {
        let mut v: Vals = [("listener_protocol".to_string(), "TERMINATED_HTTPS".to_string()), ("listener_port".to_string(), "80".to_string())].into();
        suggest_port("listener_protocol", &mut v, "listener_protocol", "listener_port");
        assert_eq!(v["listener_port"], "443");
        v.insert("listener_protocol".into(), "TCP".into());
        suggest_port("listener_protocol", &mut v, "listener_protocol", "listener_port");
        assert_eq!(v["listener_port"], "80");
        v.insert("listener_port".into(), "8443".into());
        v.insert("listener_protocol".into(), "HTTPS".into());
        suggest_port("listener_protocol", &mut v, "listener_protocol", "listener_port");
        assert_eq!(v["listener_port"], "8443", "typed: kept");
        suggest_port("listener_name", &mut v, "listener_protocol", "listener_port");
        assert_eq!(v["listener_port"], "8443");
    }

    #[test]
    fn add_member_takes_an_external_address() {
        let d = lb_with_members();
        let dlg = member_dialog(Lang::EnUs, &d, None);
        let ext = (dlg.build)(&vals(&[
            ("pool", "pool-1"), ("source", "external"), ("address", "192.0.2.10"), ("subnet", "sub-ext"), ("port", "8080"),
            ("weight", "1"), ("monitor_address", ""), ("monitor_port", "9000"), ("admin_up", "true"),
        ]))
        .unwrap();
        let OsAction::MemberCreate { spec, .. } = ext else { panic!("not a create") };
        assert_eq!((spec.address.as_str(), spec.subnet_id.as_deref(), spec.monitor_port, spec.monitor_address), ("192.0.2.10", Some("sub-ext"), Some(9000), None));
        let edit = member_dialog(Lang::EnUs, &d, Some(&d.members[0]));
        let field = |n: &str| edit.fields.iter().find(|f| f.name == n).map(|f| f.default.clone()).unwrap();
        assert_eq!((field("name"), field("monitor_address")), (String::new(), "10.0.9.9".to_string()), "the ID fallback isn't offered as a name");
        let OsAction::MemberUpdate { monitor_address, monitor_port, .. } =
            (edit.build)(&vals(&[("name", ""), ("weight", "3"), ("monitor_address", ""), ("monitor_port", ""), ("admin_up", "true")])).unwrap()
        else {
            panic!("not an update")
        };
        assert_eq!((monitor_address, monitor_port), (None, None), "emptied: back to monitoring the member itself");
    }

    #[test]
    fn identity_create_dialogs_show_where_things_go() {
        for d in [create_project_dialog(Lang::EnUs), create_user_dialog(Lang::EnUs), create_group_dialog(Lang::EnUs)] {
            assert!(matches!(d.fields[0].kind, Kind::Info(ChoiceSet::OperationDomain)), "{}: the domain comes first, read-only", d.title);
        }
    }

    #[test]
    fn firewall_dialogs() {
        let l = Lang::EnUs;
        let d = firewall_rule_dialog(l, None);
        assert!(!d.fields.iter().any(|f| f.name == "shared"), "never offered");
        let a = (d.build)(&vals(&[("protocol", "tcp"), ("action", "deny"), ("ip_version", "6"), ("destination_port", "443"), ("enabled", "true")])).unwrap();
        let OsAction::FirewallRuleCreate(spec) = a else { panic!("not a create") };
        assert_eq!((spec.ip_version, spec.destination_port.as_str()), (6, "443"));
        assert!((d.build)(&vals(&[("protocol", "icmp"), ("action", "allow"), ("source_port", "22")])).is_err(), "the server's check runs in the dialog");
        let rule = FirewallRuleRow {
            id: "r1".into(),
            name: "r1".into(),
            description: None,
            protocol: None,
            ip_version: 4,
            source_ip: None,
            source_port: None,
            destination_ip: None,
            destination_port: None,
            action: "allow".into(),
            enabled: false,
            shared: true,
            policies: vec![],
            owned: true,
        };
        let e = firewall_rule_dialog(l, Some(&rule));
        let field = |n: &str| e.fields.iter().find(|f| f.name == n).unwrap().default.clone();
        assert_eq!((field("name"), field("protocol"), field("enabled")), ("".into(), "any".into(), "false".into()), "unnamed: empty, not the ID");
        assert!(!e.fields.iter().any(|f| f.name == "shared"), "not even for a shared rule");
        assert!(firewall_rule_items(l, &FirewallRuleRow { owned: false, ..rule }).is_empty(), "another project's shared rule");

        let r = |id: &str| Ref { id: id.into(), name: id.into() };
        let p = FirewallPolicyRow { id: "fp1".into(), name: "web".into(), description: None, rules: vec![r("a"), r("b")], audited: false, shared: false, owned: true };
        let ins = insert_firewall_rule_dialog(l, &p);
        assert!(matches!(&ins.fields[0].kind, Kind::Select(Source::Without(ChoiceSet::FirewallRules, have)) if have == &["a", "b"]), "rules it has are not offered");
        assert_eq!(ins.fields[1].default, "b", "at the end by default");
        let a = (ins.build)(&vals(&[("rule", "c"), ("after", "")])).unwrap();
        assert_eq!(a, OsAction::FirewallPolicyInsertRule { id: "fp1".into(), rule_id: "c".into(), before_id: None, after_id: None }, "at the top");
        let labels: Vec<_> = firewall_policy_items(l, &p).into_iter().map(|i| i.label).collect();
        assert_eq!(labels, ["Insert Rule", "Remove Rule", "Edit Policy", "Delete Policy"]);

        let port = |id: &str| FirewallPort { port: r(id), ips: vec!["10.0.0.1".into()], kind: Some("router".into()), device: Some(r("router1")) };
        let g = FirewallGroupRow {
            id: "g1".into(),
            name: "gw".into(),
            description: None,
            ingress: None,
            egress: None,
            ports: vec![port("p1"), port("p2")],
            status: "ACTIVE".into(),
            admin_up: true,
            shared: false,
            owned: true,
        };
        let add = add_firewall_port_dialog(l, &g);
        assert_eq!((add.build)(&vals(&[("ports", "p2,p3")])).unwrap(), OsAction::FirewallGroupSetPorts { id: "g1".into(), port_ids: vec!["p1".into(), "p2".into(), "p3".into()] }, "the whole list, each once");
        assert!(matches!(&add.fields[0].kind, Kind::Pick { src: Source::Set(ChoiceSet::FirewallPorts), .. }));
        let rm = remove_firewall_port_dialog(l, &g, &g.ports[0]);
        assert_eq!((rm.build)(&vals(&[])).unwrap(), OsAction::FirewallGroupSetPorts { id: "g1".into(), port_ids: vec!["p2".into()] });
        assert_eq!(firewall_port_label(&g.ports[0]), "router1 (10.0.0.1)");
        let tips: Vec<_> = create_firewall_group_dialog(l).fields.iter().filter(|f| f.tip.is_some()).map(|f| f.name).collect();
        assert_eq!(tips, ["ingress", "egress"], "the direction of each policy is explained");
        let d = create_firewall_group_dialog(l);
        let steps: Vec<_> = d.steps.iter().map(|s| s.fields.clone()).collect();
        assert_eq!(steps[1], ["ports"], "the port picker is its own step");
        assert!(steps[0].starts_with(&["name", "description", "ingress", "egress", "admin_up"]));

        // Neutron's `default` group and policies (neutron-fwaas matches them by name).
        let labels = |g: &FirewallGroupRow| firewall_group_items(l, g).into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(labels(&g), ["Edit Firewall Group", "Add Port", "Remove Port", "Delete Firewall Group"]);
        let def = FirewallGroupRow { name: "default".into(), ..g.clone() };
        assert_eq!(labels(&def), ["Add Port", "Remove Port"], "only its ports can change");
        assert!(!def.deletable() && g.deletable() && !FirewallGroupRow { owned: false, ..g }.deletable());
        let dp = FirewallPolicyRow { name: "default ingress".into(), ..p.clone() };
        let labels: Vec<_> = firewall_policy_items(l, &dp).into_iter().map(|i| i.label).collect();
        assert_eq!(labels, ["Insert Rule", "Remove Rule"], "its rules can change, the policy can't");
        assert!(!dp.deletable() && p.deletable());
    }

    #[test]
    fn trunk_dialogs_build_subports() {
        let d = create_trunk_dialog(Lang::EnUs);
        let a = (d.build)(&vals(&[("parent", "p0"), ("admin_up", "true"), ("subport", "p1"), ("segmentation_type", "vlan"), ("segmentation_id", "101")])).unwrap();
        let OsAction::TrunkCreate { port_id, subports, .. } = a else { panic!("not a create") };
        assert_eq!((port_id.as_str(), subports[0].segmentation_id), ("p0", Some(101)));
        let none = (d.build)(&vals(&[("parent", "p0")])).unwrap();
        assert!(matches!(none, OsAction::TrunkCreate { ref subports, .. } if subports.is_empty()), "the first sub-port is optional");
        assert!((d.build)(&vals(&[("parent", "p0"), ("subport", "p0"), ("segmentation_id", "5")])).is_err(), "parent is not a sub-port");
        let add = add_subport_dialog(Lang::EnUs, "t1");
        let a = (add.build)(&vals(&[("subport", "p2"), ("segmentation_type", "inherit"), ("segmentation_id", "7")])).unwrap();
        assert!(matches!(a, OsAction::TrunkAddSubports { ref subports, .. } if subports[0].segmentation_id.is_none()), "inherit ignores the ID");
        assert!((add.build)(&vals(&[("subport", "p2"), ("segmentation_type", "vlan"), ("segmentation_id", "5000")])).is_err());
    }

    #[test]
    fn qos_rule_dialog_sends_only_the_fields_of_its_type() {
        let d = qos_rule_dialog(Lang::EnUs, "q1", None);
        let a = (d.build)(&vals(&[("kind", "dscp_marking"), ("dscp_mark", "26"), ("direction", "egress"), ("max_kbps", "5")])).unwrap();
        let OsAction::QosRuleCreate { policy_id, spec } = a else { panic!("not a create") };
        assert_eq!((policy_id.as_str(), spec.dscp_mark, spec.direction.clone(), spec.max_kbps), ("q1", Some(26), None, None));
        assert!((d.build)(&vals(&[("kind", "bandwidth_limit"), ("direction", "egress")])).is_err(), "max_kbps is required");
        let rule = QosRule { id: "r1".into(), kind: "minimum_bandwidth".into(), direction: Some("ingress".into()), max_kbps: None, max_burst_kbps: None, dscp_mark: None, min_kbps: Some(100), min_kpps: None };
        let e = qos_rule_dialog(Lang::EnUs, "q1", Some(&rule));
        assert!(e.fields.iter().all(|f| f.name != "kind"), "the type is fixed on edit");
        let a = (e.build)(&vals(&[("direction", "egress"), ("min_kbps", "200")])).unwrap();
        assert!(matches!(a, OsAction::QosRuleUpdate { ref id, ref spec, .. } if id == "r1" && spec.min_kbps == Some(200) && spec.kind == "minimum_bandwidth"));
        // No `qos` plugin in a test context: the router and floating IP dialogs have no QoS field.
        assert!(create_router_dialog(Lang::EnUs).fields.iter().all(|f| f.name != "qos"));
        assert!(allocate_ip_dialog(Lang::EnUs).fields.iter().all(|f| f.name != "qos"));
    }

    #[test]
    fn floating_ip_menu_keeps_associate_first_and_rules_in_the_dropdown() {
        let row = FloatingIpRow {
            id: "f1".into(),
            ip: "203.0.113.7".into(),
            fixed_ip: None,
            instance_id: None,
            instance_name: None,
            pool: "public".into(),
            status: "DOWN".into(),
            description: None,
            dns_name: None,
            dns_domain: None,
        };
        let labels = |r: &FloatingIpRow| floating_ip_items(Lang::EnUs, r).into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(labels(&row), ["Associate", "Port Forwarding Rules", "Release Floating IP"]);
        let associated = FloatingIpRow { fixed_ip: Some("10.0.0.5".into()), ..row.clone() };
        assert_eq!(labels(&associated)[0], "Disassociate");
        let admin = |r: &FloatingIpRow| admin_floating_ip_items(Lang::EnUs, r, Some("p9")).into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(admin(&row), ["Associate", "Release Floating IP"], "Admin: Associate among the owner's ports");
        let items = admin_floating_ip_items(Lang::EnUs, &row, Some("p9"));
        let d = dialog_of(&items, "Associate");
        assert!(matches!(&d.fields[0].kind, Kind::Select(Source::Of(ChoiceSet::AllAssociablePorts, p)) if p == "p9"), "the owner's ports, not the session's");
        assert_eq!(admin_floating_ip_items(Lang::EnUs, &row, None).len(), 1, "no owner known: no Associate");
        assert_eq!(admin(&associated), ["Disassociate", "Release Floating IP"], "Horizon's admin menu");
    }

    #[test]
    fn create_flavor_is_private_with_projects() {
        let d = create_flavor_dialog(Lang::EnUs);
        let base = [("name", "m1.big"), ("flavor_id", "auto"), ("vcpus", "4"), ("ram", "8192"), ("disk", "40"), ("ephemeral", "0"), ("swap", ""), ("rxtx", "1.0")];
        let with = |extra: &[(&'static str, &'static str)]| {
            let mut p = base.to_vec();
            p.extend_from_slice(extra);
            (d.build)(&vals(&p))
        };
        let OsAction::FlavorCreate(spec) = with(&[("projects", "p1,p2")]).unwrap() else { panic!() };
        assert_eq!((spec.vcpus, spec.ram_mb, spec.disk_gb, spec.swap_mb, spec.rxtx_factor), (4, 8192, 40, 0, Some(1.0)), "empty swap is 0");
        assert_eq!(spec.projects, ["p1", "p2"]);
        assert!(with(&[("rxtx", "fast")]).is_err(), "RX/TX factor must be a number");
        assert!((d.build)(&vals(&[("name", "x"), ("ram", "1"), ("disk", "0")])).is_err(), "VCPUs are required");
    }

    #[test]
    fn modify_access_only_for_private_flavors_and_sends_changes() {
        let f = FlavorRow {
            id: "f1".into(), name: "m1".into(), vcpus: 1, ram_mb: 512, disk_gb: 1, ephemeral_gb: 0, swap_mb: 0, rxtx_factor: None, public: false,
            extra_specs: vec![], access: vec![("p1".into(), "One".into())],
        };
        let labels = |f: &FlavorRow| admin_flavor_items(Lang::EnUs, f).into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(labels(&f), ["Update Metadata", "Modify Access", "Delete Flavor"]);
        assert_eq!(labels(&FlavorRow { public: true, ..f.clone() }), ["Update Metadata", "Delete Flavor"], "Nova can't make a public flavor private");
    }

    #[test]
    fn admin_instance_menu_adds_migrations_and_drops_project_pickers() {
        let labels = |status: &str| admin_instance_items(Lang::EnUs, &instance(status, false), Some("c1")).into_iter().map(|i| i.label).collect::<Vec<_>>();
        let active = labels("ACTIVE");
        for gone in ["Associate Floating IP", "Attach Interface", "Attach Volume", "Edit Security Groups", "Resize Instance", "Rebuild Instance", "Rescue Instance"] {
            assert!(!active.iter().any(|l| l == gone), "{gone}: picks from the session project's resources");
        }
        let at = |l: &str| active.iter().position(|x| x == l).unwrap_or_else(|| panic!("{l} in {active:?}"));
        assert!(at("Migrate Instance") < at("Live Migrate Instance") && at("Live Migrate Instance") < at("Lock Instance"), "{active:?}");
        let off = labels("SHUTOFF");
        assert!(off.iter().any(|l| l == "Migrate Instance") && !off.iter().any(|l| l == "Live Migrate Instance"), "cold only when stopped");
    }

    #[test]
    fn admin_volume_menu_is_horizons_and_status_skips_the_current() {
        let v = volume("in-use");
        let items = admin_volume_items(Lang::EnUs, &v, Some("c1@ceph#fast"));
        let labels: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
        assert_eq!(labels, ["Update Volume Status", "Migrate Volume", "Update Metadata", "Unmanage Volume", "Delete Volume"]);
        assert!(items[3].danger && items[4].danger, "unmanage and delete are destructive");
        let crate::components::dialog::ItemKind::Dialog(open) = &items[0].kind else { panic!() };
        let d = open();
        let Kind::Select(Source::Static(choices)) = &d.fields[0].kind else { panic!("status choices") };
        assert!(!choices.iter().any(|(v, _)| v == "in-use") && choices.iter().any(|(v, _)| v == "available"), "Horizon drops the current status");
        assert_eq!((d.build)(&vals(&[("status", "available")])).unwrap(), OsAction::VolumeResetStatus { id: v.id.clone(), status: "available".into() });
        let crate::components::dialog::ItemKind::Dialog(open) = &items[1].kind else { panic!() };
        let d = open();
        assert!(d.intro.iter().any(|l| l.ends_with("c1@ceph#fast")), "shows the current host: {:?}", d.intro);
        assert!(matches!(&d.fields[0].kind, Kind::Select(Source::Except(ChoiceSet::CinderPools, h)) if h == "c1@ceph#fast"), "Cinder refuses the current pool: not offered");
        assert_eq!((d.build)(&vals(&[("host", "c2@ceph#slow"), ("force", "true")])).unwrap(), OsAction::VolumeMigrate { id: v.id.clone(), host: "c2@ceph#slow".into(), force_host_copy: true });
        let snap = Snapshot { id: "s1".into(), name: "s".into(), description: None, size_gb: 1, status: "error".into(), volume_id: "v".into(), volume_name: None, created: String::new(), metadata: vec![] };
        let labels: Vec<String> = admin_snapshot_items(Lang::EnUs, &snap).into_iter().map(|i| i.label).collect();
        assert_eq!(labels, ["Update Status", "Update Metadata", "Delete Volume Snapshot"]);
    }

    fn dialog_of(items: &[MenuItem], label: &str) -> Dialog {
        let item = items.iter().find(|i| i.label == label).unwrap_or_else(|| panic!("{label}"));
        let crate::components::dialog::ItemKind::Dialog(open) = &item.kind else { panic!("{label} opens no dialog") };
        open()
    }

    #[test]
    fn volume_type_menu_follows_encryption_and_qos() {
        let plain = VolumeTypeRow { id: "t1".into(), name: "ssd".into(), description: None, qos: Some(Ref { id: "q1".into(), name: "gold".into() }), encrypted: false, public: true, extra_specs: vec![("a".into(), "1".into())], encryption: None };
        let labels = |t: &VolumeTypeRow| volume_type_items(Lang::EnUs, t).into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(labels(&plain), ["View Extra Specs", "Manage QoS Spec Association", "Edit Volume Type", "Create Encryption", "Delete Volume Type"]);
        let luks = EncryptionSpec { provider: "luks".into(), control_location: "back-end".into(), cipher: None, key_size: Some(256) };
        let enc = VolumeTypeRow { encrypted: true, encryption: Some(luks.clone()), ..plain.clone() };
        assert_eq!(labels(&enc), ["View Extra Specs", "Manage QoS Spec Association", "Edit Volume Type", "Update Encryption", "Delete Encryption", "Delete Volume Type"]);
        let d = dialog_of(&volume_type_items(Lang::EnUs, &enc), "Update Encryption");
        assert_eq!(d.fields.iter().map(|f| f.default.as_str()).collect::<Vec<_>>(), ["luks", "back-end", "", "256"], "prefilled from the spec");
        assert_eq!((d.build)(&vals(&[("provider", "luks"), ("control_location", "back-end"), ("key_size", "512")])).unwrap(),
            OsAction::VolumeTypeEncryptionSet { id: "t1".into(), spec: EncryptionSpec { key_size: Some(512), ..luks }, update: true });
        let d = dialog_of(&volume_type_items(Lang::EnUs, &plain), "Manage QoS Spec Association");
        assert_eq!(d.fields[0].default, "q1", "the current spec preselected");
        assert!((d.build)(&vals(&[("qos", "q1")])).is_err(), "nothing changed");
        assert_eq!((d.build)(&vals(&[("qos", "")])).unwrap(), OsAction::VolumeTypeSetQos { id: "t1".into(), qos_id: None, current_qos_id: Some("q1".into()) });
        let d = dialog_of(&volume_type_items(Lang::EnUs, &plain), "View Extra Specs");
        assert_eq!((d.build)(&vals(&[("metadata", r#"[["b","2"]]"#)])).unwrap(), OsAction::VolumeTypeSetExtraSpecs { id: "t1".into(), set: vec![("b".into(), "2".into())], unset: vec!["a".into()] });
    }

    #[test]
    fn qos_and_group_type_menus() {
        let q = QosSpecRow { id: "q1".into(), name: "gold".into(), consumer: "back-end".into(), specs: vec![] };
        let items = qos_spec_items(Lang::EnUs, &q);
        assert_eq!(items.iter().map(|i| i.label.clone()).collect::<Vec<_>>(), ["Manage Specs", "Edit Consumer", "Delete QoS Spec"]);
        let d = dialog_of(&items, "Edit Consumer");
        assert!((d.build)(&vals(&[("consumer", "back-end")])).is_err(), "nothing changed");
        assert_eq!((d.build)(&vals(&[("consumer", "both")])).unwrap(), OsAction::QosSpecSetConsumer { id: "q1".into(), consumer: "both".into() });
        let c = create_qos_spec_dialog(Lang::EnUs);
        assert_eq!((c.build)(&vals(&[("name", "gold"), ("consumer", "front-end"), ("specs", r#"[["read_iops_sec","500"]]"#)])).unwrap(),
            OsAction::QosSpecCreate { name: "gold".into(), consumer: "front-end".into(), specs: vec![("read_iops_sec".into(), "500".into())] });
        let g = GroupTypeRow { id: "g1".into(), name: "cg".into(), description: None, public: true, specs: vec![] };
        assert_eq!(group_type_items(Lang::EnUs, &g).into_iter().map(|i| i.label).collect::<Vec<_>>(), ["View Specs", "Edit Group Type", "Delete Group Type"]);
        let c = create_group_type_dialog(Lang::EnUs);
        assert_eq!((c.build)(&vals(&[("name", "cg"), ("public", "false")])).unwrap(), OsAction::GroupTypeCreate { name: "cg".into(), description: None, public: false });
    }

    #[test]
    fn metadef_menu_hides_delete_when_protected_and_sends_association_changes() {
        let n = MetadefNamespace { namespace: "OS::Acme::Gpu".into(), name: "GPU".into(), description: None, resource_types: vec!["OS::Glance::Image".into()], public: true, protected: false };
        let items = metadef_items(Lang::EnUs, &n);
        assert_eq!(items.iter().map(|i| i.label.clone()).collect::<Vec<_>>(), ["Edit Namespace", "Update Associations", "Delete Namespace"]);
        assert_eq!(metadef_items(Lang::EnUs, &MetadefNamespace { protected: true, ..n.clone() }).len(), 2, "Glance refuses deleting a protected namespace");
        let d = dialog_of(&items, "Update Associations");
        assert_eq!((d.build)(&vals(&[("types", "OS::Nova::Flavor")])).unwrap(),
            OsAction::MetadefNamespaceSetTypes { id: "OS::Acme::Gpu".into(), add_type_ids: vec!["OS::Nova::Flavor".into()], remove_type_ids: vec!["OS::Glance::Image".into()] });
        let d = dialog_of(&items, "Edit Namespace");
        assert_eq!(d.fields[0].default, "GPU", "the display name, not the namespace");
        assert!((import_namespace_dialog(Lang::EnUs).build)(&vals(&[("definition", "{oops")])).is_err(), "JSON checked in the dialog");
    }

    #[test]
    fn manage_volume_reads_horizons_form() {
        let d = manage_volume_dialog(Lang::EnUs);
        let a = (d.build)(&vals(&[("identifier", "lun-7"), ("id_type", "source-id"), ("host", "c1@san#p"), ("name", ""), ("bootable", "true"), ("metadata", r#"[["k","v"]]"#)])).unwrap();
        let OsAction::VolumeManage(spec) = a else { panic!() };
        assert_eq!((spec.identifier.as_str(), spec.by_id, spec.host.as_str(), spec.name, spec.bootable), ("lun-7", true, "c1@san#p", None, true));
        assert_eq!(spec.metadata, [("k".to_string(), "v".to_string())]);
        assert!((d.build)(&vals(&[("identifier", "lun-7"), ("id_type", "source-name")])).is_err(), "the host is required");
    }

    #[test]
    fn live_migrate_sends_auto_unless_chosen() {
        let item = live_migrate_item(Lang::EnUs, "s1", "web", "c1");
        let crate::components::dialog::ItemKind::Dialog(open) = &item.kind else { panic!("a dialog item") };
        let d = open();
        assert!(matches!(&d.fields[0].kind, Kind::Select(Source::Except(ChoiceSet::ComputeHosts, h)) if h == "c1"), "Nova refuses the current host: not offered");
        assert_eq!((d.build)(&vals(&[("host", ""), ("block", "auto")])).unwrap(), OsAction::ServerLiveMigrate { id: "s1".into(), host: None, block_migration: None });
        assert_eq!((d.build)(&vals(&[("host", "c2"), ("block", "false")])).unwrap(), OsAction::ServerLiveMigrate { id: "s1".into(), host: Some("c2".into()), block_migration: Some(false) });
    }

    #[test]
    fn aggregate_hosts_and_compute_services() {
        let a = AggregateRow { id: "7".into(), name: "gpu".into(), zone: Some("az1".into()), hosts: vec!["c1".into()], metadata: vec![("availability_zone".into(), "az1".into())] };
        let items = aggregate_items(Lang::EnUs, &a);
        assert_eq!(items.iter().map(|i| i.label.clone()).collect::<Vec<_>>(), ["Edit Host Aggregate", "Manage Hosts", "Update Metadata", "Delete Host Aggregate"]);
        let crate::components::dialog::ItemKind::Dialog(open) = &items[1].kind else { panic!() };
        let d = open();
        assert_eq!((d.build)(&vals(&[("hosts", "c2")])).unwrap(), OsAction::AggregateSetHosts { id: "7".into(), add: vec!["c2".into()], remove: vec!["c1".into()] });
        assert!((d.build)(&vals(&[("hosts", "c1")])).is_err(), "nothing changed");
        let svc = HostService { id: "u1".into(), binary: "nova-compute".into(), host: "c1".into(), zone: None, enabled: true, up: true, updated_at: None, disabled_reason: None };
        assert_eq!(compute_service_items(Lang::EnUs, &svc)[0].label, "Disable Service");
        assert_eq!(compute_service_items(Lang::EnUs, &HostService { enabled: false, ..svc.clone() })[0].label, "Enable Service");
        assert!(compute_service_items(Lang::EnUs, &HostService { id: String::new(), ..svc }).is_empty(), "no UUID (before 2.53): no action");
    }

    #[test]
    fn admin_create_router_sends_server_defaults_as_nothing() {
        let d = admin_create_router_dialog(Lang::EnUs);
        let names: Vec<&str> = d.fields.iter().map(|f| f.name).collect();
        assert_eq!(names, ["name", "project_id", "admin_up", "external", "snat", "mode", "ha", "az_hints"], "unknown capabilities show every field");
        let a = (d.build)(&vals(&[("name", "edge"), ("project_id", "p2"), ("admin_up", "true"), ("external", "ext"), ("snat", "false"), ("mode", "off"), ("ha", "default"), ("az_hints", "az1,az2")])).unwrap();
        let OsAction::AdminRouterCreate(spec) = a else { panic!() };
        assert_eq!(
            *spec,
            AdminRouterSpec {
                name: "edge".into(),
                project_id: "p2".into(),
                admin_up: true,
                external_network_id: Some("ext".into()),
                enable_snat: Some(false),
                az_hints: vec!["az1".into(), "az2".into()],
                distributed: Some(false),
                ha: None,
            }
        );
        // No gateway: *Enable SNAT* is hidden (empty) and not sent.
        let OsAction::AdminRouterCreate(spec) = (d.build)(&vals(&[("project_id", "p2"), ("mode", "on")])).unwrap() else { panic!() };
        assert_eq!((spec.enable_snat, spec.external_network_id, spec.distributed), (None, None, Some(true)));
        assert!((d.build)(&vals(&[("name", "edge")])).is_err(), "the project is required");
    }

    #[test]
    fn admin_router_menu_is_horizons_edit_and_delete() {
        let r = RouterRow {
            id: "r1".into(),
            name: "edge".into(),
            status: "ACTIVE".into(),
            external_network: Some("public".into()),
            external_network_id: Some("ext".into()),
            snat: Some(true),
            admin_up: true,
            availability_zones: vec![],
        };
        let labels = |items: Vec<MenuItem>| items.into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(labels(admin_router_items(Lang::EnUs, &r)), ["Edit Router", "Delete Router"], "no gateway or interface items: their choices are the session project's");
        assert_eq!(labels(router_items(Lang::EnUs, &r)), ["Clear Gateway", "Edit Router", "Add Interface", "Delete Router"], "Project's menu unchanged");
    }

    #[test]
    fn admin_allocate_names_the_project_and_optional_address() {
        let d = admin_allocate_ip_dialog(Lang::EnUs);
        let a = (d.build)(&vals(&[("pool", "ext"), ("project_id", "p2"), ("address", ""), ("description", "vpn")])).unwrap();
        assert_eq!(a, OsAction::AdminFloatingIpAllocate { network_id: "ext".into(), project_id: "p2".into(), address: None, description: Some("vpn".into()) });
        assert!((d.build)(&vals(&[("pool", "ext")])).is_err(), "the project is required");
    }

    #[test]
    fn upload_formats_are_the_configured_ones_cinder_supports() {
        let def = crate::prefs::ImagePrefs::default().disk_formats;
        assert_eq!(upload_formats(&def), ["raw", "qcow2", "vmdk", "vdi", "vhd", "ploop"]);
        assert_eq!(upload_formats(&["qcow2".into(), "iso".into()]), ["qcow2"]);
        assert_eq!(upload_formats(&["iso".into()]), ["raw"], "nothing usable: raw");
    }

    #[test]
    fn subnet_dialogs_follow_the_network_settings() {
        use crate::prefs::{AllowedCidrs, NetworkPrefs};
        let field = |fs: &[Field], n: &str| fs.iter().find(|f| f.name == n).cloned().unwrap();
        let def = subnet_fields(Lang::EnUs, None, &NetworkPrefs::default());
        assert_eq!(field(&def, "dns").default, "", "no DNS servers by default");
        assert!(matches!(field(&def, "ip_version").kind, Kind::Select(Source::Static(o)) if o.len() == 2));
        let net = NetworkPrefs {
            default_dns_nameservers: vec!["10.0.0.2".into(), "10.0.0.3".into()],
            enable_ipv6: false,
            allowed_private_subnet_cidr: AllowedCidrs { ipv4: vec!["10.0.0.0/8".into()], ipv6: vec![] },
            ..Default::default()
        };
        let fs = subnet_fields(Lang::EnUs, None, &net);
        assert_eq!(field(&fs, "dns").default, "10.0.0.2\n10.0.0.3");
        assert!(matches!(field(&fs, "ip_version").kind, Kind::Select(Source::Static(o)) if o.len() == 1));
        let check = subnet_check(Lang::EnUs, net);
        let vals = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vals>();
        let sets = Sets::default();
        assert!(check(&vals(&[("cidr", "10.1.0.0/24")]), &sets).is_empty());
        assert_eq!(check(&vals(&[("cidr", "172.16.0.0/24")]), &sets)[0].0, "cidr");
        assert!(check(&vals(&[("with_subnet", "false"), ("cidr", "172.16.0.0/24")]), &sets).is_empty(), "no subnet: no check");
        let a = OsAction::Batch(vec![("s".into(), OsAction::SubnetCreate { network_id: "n".into(), spec: SubnetSpec { cidr: "10.0.0.0/24".into(), ..sample_subnet() } })]);
        assert_eq!(a.new_subnet_cidrs(), ["10.0.0.0/24"]);
    }

    fn sample_subnet() -> SubnetSpec {
        SubnetSpec {
            name: String::new(), cidr: String::new(), ip_version: 4, gateway_ip: None, enable_dhcp: true, dns_nameservers: vec![], allocation_pools: vec![],
            ipv6_ra_mode: None, ipv6_address_mode: None, host_routes: vec![],
        }
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
        assert_eq!(field(&def, "create_volume").unwrap().default, "true", "default: boot from a new volume");
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
        assert_eq!((field(&d, "create_volume").unwrap().default.as_str(), field(&d, "config_drive").unwrap().default.as_str()), ("false", "true"));
        assert!(field(&d, "key_name").unwrap().required);
        assert!(matches!(field(&d, "admin_pass").unwrap().kind, Kind::Password));
        let from_image = launch_with(Lang::EnUs, &p, Some("image"));
        assert_eq!(sources(&from_image), ["image", "volume", "volume_snapshot"], "a row's own source stays");
    }

    #[test]
    fn launch_form_builds_a_launch_spec() {
        let d = launch_dialog(Lang::EnUs);
        let a = (d.build)(&vals(&[
            ("name", "web"), ("count", "2"), ("source", "image"), ("source_id", "img"),
            ("create_volume", "true"), ("volume_size", "30"), ("flavor", "f"),
            ("networks", "n1,n2"), ("security_groups", "default"),
        ]))
        .unwrap();
        let OsAction::ServerLaunch(s) = a else { panic!() };
        assert_eq!((s.count, s.volume_size_gb, s.network_ids.len()), (2, Some(30), 2));
        assert!(s.create_volume);
        // missing flavor is reported
        assert!((d.build)(&vals(&[("name", "x"), ("source_id", "i")])).is_err());
    }

    #[test]
    fn allowed_actions_follow_horizon() {
        use Op::*;
        let a = allowed("VERIFY_RESIZE", Some(false), false);
        assert_eq!(a[..2], [ConfirmResize, RevertResize], "confirm/revert come first");
        assert!(!allowed("ACTIVE", Some(false), false).contains(&ConfirmResize));
        assert!(allowed("SHELVED_OFFLOADED", Some(false), false).contains(&Unshelve));
        assert!(!allowed("ACTIVE", Some(false), false).contains(&Unshelve));
        let active = allowed("ACTIVE", Some(false), false);
        for op in [Resize, Rebuild, Shelve, Rescue, AttachInterface, AssociateIp, Pause, SoftReboot] {
            assert!(active.contains(&op), "{op:?}");
        }
        assert!(allowed("RESCUE", Some(false), false).contains(&Unrescue));
        assert!(!active.contains(&Unrescue));
        let shelved = allowed("SHELVED", Some(false), false);
        for op in [Resize, Rebuild, Snapshot, Pause, HardReboot, AttachInterface] {
            assert!(!shelved.contains(&op), "{op:?} on a shelved instance");
        }
        assert_eq!(allowed("ACTIVE", Some(true), true), [Edit, Unlock, Delete], "busy: almost nothing");
        assert_eq!(allowed("SHUTOFF", Some(false), false)[0], Start, "start first when stopped");
        assert_eq!(allowed("ACTIVE", Some(false), false)[0], Snapshot);
        assert_eq!(allowed("BUILD", None, true), [Edit, Lock, Unlock, Delete]);
        for s in ["ACTIVE", "SHUTOFF", "ERROR", "SHELVED", "VERIFY_RESIZE"] {
            assert_eq!(allowed(s, Some(false), false).last(), Some(&Delete), "{s}: delete is last");
        }
    }

    fn instance(status: &str, locked: bool) -> Instance {
        Instance {
            id: "i".into(),
            name: "n".into(),
            image_name: None,
            addresses: vec![],
            flavor_name: "small".into(),
            flavor: FlavorInfo { id: "f1".into(), name: "small".into(), vcpus: None, ram_mb: None, disk_gb: None, ephemeral_gb: None, swap_mb: None },
            image_from_volume: false,
            locked,
            key_name: None,
            status: status.into(),
            availability_zone: None,
            task_state: None,
            power_state: 1,
            created: String::new(),
            metadata: vec![],
            security_groups: vec!["default".into()],
            boot_volume_ids: vec![],
        }
    }

    #[test]
    fn edit_security_groups_sends_the_difference_and_needs_ports() {
        let mut inst = instance("ACTIVE", false);
        let has = |inst: &Instance| instance_items(Lang::EnUs, inst).iter().any(|m| m.label == "Edit Security Groups");
        assert!(!has(&inst), "no ports: Nova would answer 400");
        inst.addresses = vec![Address { network: "net".into(), addr: "10.0.0.5".into(), floating: false, mac: None }];
        assert!(has(&inst));
        assert!(!allowed("SHELVED_OFFLOADED", Some(false), false).contains(&Op::EditSecurityGroups));
        let d = security_groups_dialog(Lang::EnUs, "i", &inst.security_groups);
        assert_eq!(
            (d.build)(&vals(&[("groups", "web,ssh")])).unwrap(),
            OsAction::ServerSetSecurityGroups { id: "i".into(), add: vec!["web".into(), "ssh".into()], remove: vec!["default".into()] }
        );
        assert!((d.build)(&vals(&[("groups", "default")])).is_err(), "nothing changed");
    }

    #[test]
    fn security_group_editors_are_filtered_tables_next_to_each_other() {
        use crate::components::dialog::Kind;
        let ops = allowed("ACTIVE", Some(false), false);
        let at = |op| ops.iter().position(|o| *o == op).unwrap();
        assert_eq!(at(Op::EditPortSecurityGroups), at(Op::EditSecurityGroups) + 1, "the two items sit together");
        assert!(allowed("SHELVED_OFFLOADED", Some(false), false).contains(&Op::EditPortSecurityGroups), "per port stays available");
        for d in [security_groups_dialog(Lang::EnUs, "i", &[]), port_security_groups_dialog(Lang::EnUs, "i")] {
            let f = d.fields.iter().find(|f| f.name == "groups").unwrap();
            assert!(matches!(f.kind, Kind::Pick { multi: true, .. }), "{}: Allocated / Available with a filter", d.title);
        }
    }

    #[test]
    fn resize_rejects_the_current_flavor_and_launch_presets_the_source() {
        let items = instance_items(Lang::EnUs, &instance("ACTIVE", false));
        let resize = items.iter().find(|m| m.label == "Resize Instance").unwrap();
        let crate::components::dialog::ItemKind::Dialog(open) = &resize.kind else { panic!() };
        let d = open();
        assert!((d.build)(&vals(&[("flavor", "f1")])).is_err());
        assert!(matches!((d.build)(&vals(&[("flavor", "f2")])), Ok(OsAction::ServerResize { .. })));
        let d = launch_from(Lang::EnUs, "volume", "v1");
        let get = |n: &str| d.fields.iter().find(|f| f.name == n).unwrap().default.clone();
        assert_eq!((get("source"), get("source_id")), ("volume".to_string(), "v1".to_string()));
    }

    fn volume(status: &str) -> Volume {
        Volume {
            id: "v".into(),
            name: "data".into(),
            description: None,
            size_gb: 10,
            status: status.into(),
            volume_type: Some("std".into()),
            attachments: vec![],
            availability_zone: None,
            bootable: false,
            encrypted: false,
            metadata: vec![],
        }
    }

    #[test]
    fn volume_menu_follows_status_and_retype_needs_a_new_type() {
        let labels = |st| volume_items(Lang::EnUs, &volume(st)).into_iter().map(|m| m.label).collect::<Vec<_>>();
        let avail = labels("available");
        for l in ["Change Volume Type", "Upload to Image", "Create Transfer"] {
            assert!(avail.contains(&l.to_string()), "{l}");
        }
        let in_use = labels("in-use");
        assert!(in_use.contains(&"Upload to Image".to_string()) && !in_use.contains(&"Create Transfer".to_string()));
        assert!(!labels("error").contains(&"Change Volume Type".to_string()));
        let items = volume_items(Lang::EnUs, &volume("available"));
        let crate::components::dialog::ItemKind::Dialog(open) = &items.iter().find(|m| m.label == "Change Volume Type").unwrap().kind else { panic!() };
        let d = open();
        assert!((d.build)(&vals(&[("type", "std")])).is_err());
        assert_eq!(
            (d.build)(&vals(&[("type", "fast"), ("migrate", "true")])).unwrap(),
            OsAction::VolumeRetype { id: "v".into(), new_type: "fast".into(), migrate: true }
        );
    }

    #[test]
    fn routes_use_the_atomic_call_or_the_whole_list() {
        let cur = vec![("10.1.0.0/16".to_string(), "10.0.0.1".to_string())];
        let new = vals(&[("destination", "10.2.0.0/16"), ("nexthop", "10.0.0.2")]);
        let a = (add_route_dialog(Lang::EnUs, "r", None).build)(&new).unwrap();
        assert!(matches!(a, OsAction::RouterAddRoute { .. }));
        let OsAction::RouterSetRoutes { routes, .. } = (add_route_dialog(Lang::EnUs, "r", Some(cur.clone())).build)(&new).unwrap() else { panic!() };
        assert_eq!(routes.len(), 2);
        let items = route_items(Lang::EnUs, "r", &cur[0], Some(&cur));
        let crate::components::dialog::ItemKind::Dialog(open) = &items[0].kind else { panic!() };
        assert_eq!((open().build)(&vals(&[])).unwrap(), OsAction::RouterSetRoutes { id: "r".into(), routes: vec![] });
    }

    #[test]
    fn associate_ip_picks_an_existing_ip_or_allocates_one() {
        let d = associate_ip_dialog(Lang::EnUs, "s1");
        let a = (d.build)(&vals(&[("allocate", "false"), ("ip", "f1"), ("port", "p1")])).unwrap();
        assert_eq!(a, OsAction::FloatingIpAssociate { id: "f1".into(), port_id: "p1".into() });
        let a = (d.build)(&vals(&[("allocate", "true"), ("pool", "ext"), ("port", "p1")])).unwrap();
        assert_eq!(a, OsAction::FloatingIpAllocate { network_id: "ext".into(), description: None, port_id: Some("p1".into()), qos_policy_id: None });
    }

    #[test]
    fn instance_menu_depends_on_state() {
        let labels = |status, locked| -> Vec<String> {
            instance_items(Lang::EnUs, &instance(status, locked)).into_iter().map(|m| m.label).collect()
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
    fn admin_edit_network_sends_shared_and_external_and_shows_the_provider() {
        let net = NetworkRow {
            id: "n1".into(), name: "ext".into(), subnets: vec![], shared: false, external: true, status: "ACTIVE".into(), admin_up: true,
            availability_zones: vec![], owned: false, dhcp_agents: None,
            provider: Some(ProviderNetwork { network_type: "vlan".into(), physical_network: Some("physnet1".into()), segmentation_id: Some(120) }),
        };
        let d = admin_edit_network_dialog(Lang::EnUs, &net);
        assert_eq!(d.fields.iter().map(|f| f.name).collect::<Vec<_>>(), ["name", "admin_up", "shared", "external"], "Horizon's four fields");
        assert!(d.intro[0].contains("vlan") && d.intro[0].contains("physnet1") && d.intro[0].contains("120"), "{:?}", d.intro);
        let a = (d.build)(&vals(&[("name", "ext"), ("admin_up", "true"), ("shared", "true"), ("external", "false")])).unwrap();
        assert_eq!(a, OsAction::AdminNetworkUpdate { id: "n1".into(), name: "ext".into(), admin_up: true, shared: true, external: false });
    }

    #[test]
    fn admin_network_form_builds_provider_and_owner() {
        let d = admin_create_network_dialog(Lang::EnUs);
        let base = [("name", "ext"), ("project_id", "p1"), ("admin_up", "true"), ("shared", "true"), ("external", "true"), ("with_subnet", "false")];
        let with = |extra: &[(&'static str, &'static str)]| vals(&[&base[..], extra].concat());
        let a = (d.build)(&with(&[("provider_type", "vlan"), ("physical_network", "physnet1"), ("segmentation_id", "120")])).unwrap();
        let OsAction::AdminNetworkCreate(spec) = a else { panic!() };
        assert_eq!((spec.project_id.as_str(), spec.shared, spec.external, spec.subnet.is_none()), ("p1", true, true, true));
        assert_eq!(spec.provider, Some(ProviderNetwork { network_type: "vlan".into(), physical_network: Some("physnet1".into()), segmentation_id: Some(120) }));
        assert!((d.build)(&with(&[("provider_type", "vlan"), ("segmentation_id", "120")])).is_err(), "vlan without a physical network");
        assert!((d.build)(&with(&[("provider_type", "vxlan"), ("segmentation_id", "99999999")])).is_err(), "VNI out of range");
        assert!((d.build)(&vals(&[("provider_type", "vxlan")])).is_err(), "the project is required");
        let a = (d.build)(&with(&[("provider_type", "local"), ("mtu", "1450"), ("az_hints", "az1,az2")])).unwrap();
        let OsAction::AdminNetworkCreate(spec) = a else { panic!() };
        assert_eq!((spec.mtu, spec.az_hints.clone()), (Some(1450), vec!["az1".to_string(), "az2".to_string()]));
        assert_eq!(d.fields.iter().find(|f| f.name == "provider_type").map(|f| f.default.as_str()), Some("local"), "Horizon preselects Local");
    }

    #[test]
    fn rbac_edit_changes_only_the_target_and_starts_from_the_current_one() {
        let r = RbacPolicyRow {
            id: "r1".into(), project: "admin".into(), object_type: "network".into(), object: Ref { id: "n1".into(), name: "public".into() },
            target: "demo".into(), target_id: "p2".into(), action: "access_as_shared".into(),
        };
        let labels: Vec<String> = rbac_policy_items(Lang::EnUs, &r).into_iter().map(|m| m.label).collect();
        assert_eq!(labels, ["Edit Policy", "Delete RBAC Policy"], "Horizon's row menu, delete last");
        let d = edit_rbac_policy_dialog(Lang::EnUs, &r);
        assert_eq!((d.fields.len(), d.fields[0].default.as_str()), (1, "p2"), "one field, preselected with the target's ID");
        let a = (d.build)(&vals(&[("target_project", "*")])).unwrap();
        assert_eq!(a, OsAction::RbacPolicyUpdate { id: "r1".into(), target_project: "*".into() });
    }

    #[test]
    fn rbac_form_picks_the_object_by_kind() {
        let d = create_rbac_policy_dialog(Lang::EnUs);
        let a = (d.build)(&vals(&[("target_project", "*"), ("kind", "external_network"), ("network_id", "n1"), ("qos_policy_id", "q1")])).unwrap();
        assert_eq!(a, OsAction::RbacPolicyCreate { kind: "external_network".into(), object_id: "n1".into(), target_project: "*".into() });
        let a = (d.build)(&vals(&[("target_project", "p2"), ("kind", "shared_qos_policy"), ("qos_policy_id", "q1")])).unwrap();
        assert_eq!(a, OsAction::RbacPolicyCreate { kind: "shared_qos_policy".into(), object_id: "q1".into(), target_project: "p2".into() });
        assert!((d.build)(&vals(&[("target_project", "p2"), ("kind", "shared_network")])).is_err(), "the network is required");
    }

    #[test]
    fn subnet_steps_read_ipv6_modes_and_host_routes() {
        let d = create_network_dialog(Lang::EnUs);
        let base = [("with_subnet", "true"), ("dhcp", "true")];
        let build = |extra: &[(&'static str, &'static str)]| (d.build)(&vals(&[&base[..], extra].concat()));
        let subnet = |a: OsAction| match a {
            OsAction::NetworkCreate { subnet: Some(s), .. } => s,
            other => panic!("{other:?}"),
        };
        let s = subnet(build(&[("cidr", "2001:db8::/64"), ("ip_version", "6"), ("ipv6_modes", "none/slaac"), ("host_routes", "2001:db8:1::/64,2001:db8::1")]).unwrap());
        assert_eq!((s.ipv6_ra_mode, s.ipv6_address_mode.as_deref()), (None, Some("slaac")));
        assert_eq!(s.host_routes, vec![("2001:db8:1::/64".to_string(), "2001:db8::1".to_string())]);
        let s = subnet(build(&[("cidr", "10.0.0.0/24"), ("ip_version", "4"), ("ipv6_modes", "slaac/slaac")]).unwrap());
        assert_eq!(s.ipv6_address_mode, None, "IPv4 ignores a leftover IPv6 mode");
        assert!(build(&[("cidr", "10.0.0.0/24"), ("ip_version", "4"), ("host_routes", "10.1.0.0/24")]).is_err(), "a route needs its next hop");
    }

    #[test]
    fn network_form_with_and_without_subnet() {
        let d = create_network_dialog(Lang::EnUs);
        let a = (d.build)(&vals(&[("name", "n"), ("admin_up", "true"), ("with_subnet", "false")])).unwrap();
        assert!(matches!(a, OsAction::NetworkCreate { subnet: None, .. }));
        let a = (d.build)(&vals(&[
            ("with_subnet", "true"), ("cidr", "10.0.0.0/24"), ("ip_version", "4"),
            ("no_gateway", "true"), ("dhcp", "true"), ("pools", "10.0.0.10,10.0.0.20"), ("dns", "1.1.1.1\n8.8.8.8"),
        ]))
        .unwrap();
        let OsAction::NetworkCreate { subnet: Some(s), .. } = a else { panic!() };
        assert_eq!(s.gateway_ip.as_deref(), Some(""));
        assert_eq!(s.allocation_pools, vec![("10.0.0.10".into(), "10.0.0.20".into())]);
        assert_eq!(s.dns_nameservers.len(), 2);
        assert!((d.build)(&vals(&[("with_subnet", "true"), ("cidr", "x"), ("pools", "nocomma")])).is_err());
    }

    #[test]
    fn foreign_networks_and_the_default_group_are_read_only() {
        let net = |owned| NetworkRow {
            id: "n".into(), name: "public".into(), subnets: vec![], shared: false, external: true,
            status: "ACTIVE".into(), admin_up: true, availability_zones: vec![], owned, dhcp_agents: None, provider: None,
        };
        assert!(network_items(Lang::EnUs, &net(false)).is_empty());
        let labels = |items: Vec<MenuItem>| items.into_iter().map(|m| m.label).collect::<Vec<_>>();
        assert_eq!(labels(network_items(Lang::EnUs, &net(true))), ["Edit Network", "Create Subnet", "Create Port", "Delete Network"]);
        assert_eq!(labels(admin_network_items(Lang::EnUs, &net(false))), ["Edit Network", "Create Subnet", "Delete Network"], "Admin: any project's network, Horizon's admin menu");
        assert_eq!(labels(security_group_items(Lang::EnUs, "g", "default", None, true)), ["Manage Rules"]);
        assert!(security_group_items(Lang::EnUs, "g", "default", None, false).is_empty());
        assert_eq!(security_group_items(Lang::EnUs, "g", "web", None, true).len(), 3);
    }

    #[test]
    fn edit_image_sends_only_the_changes() {
        let im = Image {
            id: "i".into(), name: "deb".into(), is_snapshot: false, status: "active".into(), visibility: "private".into(),
            protected: false, disk_format: Some("qcow2".into()), size: None, owned_by_project: true, owner_id: None,
            owner_name: None, properties: vec![("description".into(), "old".into())], min_disk_gb: Some(5), min_ram_mb: None,
            import_failed: None,
        };
        let formats = vec!["raw".to_string(), "qcow2".to_string()];
        let d = edit_image_dialog(Lang::EnUs, &im, false, &formats);
        let names: Vec<_> = d.fields.iter().map(|f| f.name).collect();
        assert_eq!(names, ["name", "description", "architecture", "min_disk", "min_ram", "visibility", "protected"], "no kernel/ramdisk/format");
        let Kind::Select(Source::Static(vis)) = &d.fields[5].kind else { panic!() };
        assert!(!vis.iter().any(|(v, _)| v == "public"), "public needs the admin role");
        let base = [("name", "deb"), ("description", "old"), ("architecture", ""), ("min_disk", "5"), ("min_ram", "0"), ("visibility", "private"), ("protected", "false")];
        let with = |changes: &[(&'static str, &'static str)]| {
            let mut all: Vec<(&str, &str)> = base.to_vec();
            for (k, v) in changes {
                all.retain(|(a, _)| a != k);
                all.push((k, v));
            }
            (d.build)(&vals(&all))
        };
        assert!(with(&[]).is_err(), "nothing changed");
        let OsAction::ImageUpdate { changes, .. } = with(&[("description", ""), ("architecture", "x86_64"), ("min_disk", "10")]).unwrap() else { panic!() };
        assert_eq!(changes, ImageChanges {
            min_disk_gb: Some(10),
            set: vec![("architecture".into(), "x86_64".into())],
            remove: vec!["description".into()],
            ..Default::default()
        });
        let ami = Image { disk_format: Some("ami".into()), status: "queued".into(), visibility: "public".into(), ..im };
        let d = edit_image_dialog(Lang::EnUs, &ami, false, &formats);
        for f in ["kernel_id", "ramdisk_id", "disk_format"] {
            assert!(d.fields.iter().any(|x| x.name == f), "{f}");
        }
        let Kind::Select(Source::Static(vis)) = &d.fields.iter().find(|f| f.name == "visibility").unwrap().kind else { panic!() };
        assert!(vis.iter().any(|(v, _)| v == "public"), "already public: kept");
    }

    #[test]
    fn admin_edits_any_projects_image_and_may_publicize() {
        let im = Image {
            id: "i".into(), name: "deb".into(), is_snapshot: false, status: "active".into(), visibility: "private".into(),
            protected: false, disk_format: Some("qcow2".into()), size: None, owned_by_project: false, owner_id: Some("p9".into()),
            owner_name: None, properties: vec![], min_disk_gb: None, min_ram_mb: None, import_failed: None,
        };
        assert!(image_items(Lang::EnUs, &im).iter().all(|i| i.label != "Edit Image" && i.label != "Update Metadata"), "Project: another project's image is read-only");
        let own = image_items(Lang::EnUs, &Image { owned_by_project: true, ..im.clone() });
        assert!(dialog_of(&own, "Update Metadata").forbidden.is_some(), "a 403 shows the no-permission text");
        let items = admin_image_items(Lang::EnUs, &im);
        assert_eq!(items.iter().map(|i| i.label.clone()).collect::<Vec<_>>(), ["Edit Image", "Update Metadata", "Delete Image"]);
        let d = dialog_of(&items, "Edit Image");
        let Kind::Select(Source::Static(vis)) = &d.fields.iter().find(|f| f.name == "visibility").unwrap().kind else { panic!() };
        assert!(vis.iter().any(|(v, _)| v == "public"), "an admin may publicize");
        let locked = admin_image_items(Lang::EnUs, &Image { protected: true, ..im });
        assert!(locked.iter().all(|i| i.label != "Delete Image"), "Glance refuses deleting a protected image (Horizon hides it)");
    }

    #[test]
    fn zone_selectors_offer_any_zone() {
        let d = launch_from(Lang::EnUs, "image", "img");
        let az = d.fields.iter().find(|f| f.name == "az").unwrap();
        assert_eq!(az.empty_label.as_deref(), Some("Any Availability Zone"));
    }

    #[test]
    fn update_metadata_submits_only_changes() {
        let kv = |p: &[(&str, &str)]| p.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>();
        let json = |p: &[(&str, &str)]| serde_json::to_string(&kv(p)).unwrap();
        let v = Volume { metadata: kv(&[("env", "dev")]), ..volume("available") };
        let items = volume_items(Lang::EnUs, &v);
        let item = items.iter().find(|m| m.label == "Update Metadata").expect("volume menu has it");
        let crate::components::dialog::ItemKind::Dialog(open) = &item.kind else { panic!() };
        let d = open();
        assert_eq!(d.fields[0].default, json(&[("env", "dev")]), "prefilled with the current pairs");
        assert!((d.build)(&vals(&[("metadata", &json(&[(" env ", "dev")]))])).is_err(), "nothing changed");
        assert!((d.build)(&vals(&[("metadata", &json(&[("a", "1"), ("a", "2")]))])).is_err(), "repeated key");
        let ok = (d.build)(&vals(&[("metadata", &json(&[]))])).unwrap();
        assert_eq!(ok, OsAction::VolumeSetMetadata { id: "v".into(), metadata: vec![] });
        let item = metadata_item(Lang::EnUs, "OS::Glance::Image", &kv(&[("a", "1"), ("b", "2")]), |new, old| {
            let (set, remove) = metadata_changes(old, &new);
            OsAction::ImageSetProperties { id: "i".into(), set, remove }
        });
        let crate::components::dialog::ItemKind::Dialog(open) = &item.kind else { panic!() };
        let d = open();
        let out = (d.build)(&vals(&[("metadata", &json(&[("a", "10"), ("c", "3")]))])).unwrap();
        assert_eq!(out, OsAction::ImageSetProperties { id: "i".into(), set: kv(&[("a", "10"), ("c", "3")]), remove: vec!["b".into()] });
        assert!(instance_items(Lang::EnUs, &instance("ACTIVE", false)).iter().any(|m| m.label == "Update Metadata"));
    }

    #[test]
    fn extend_must_grow() {
        let v = Volume {
            id: "v".into(), name: "d".into(), description: None, size_gb: 10, status: "available".into(),
            volume_type: None, attachments: vec![], availability_zone: None, bootable: false, encrypted: false,
            metadata: vec![],
        };
        let items = volume_items(Lang::EnUs, &v);
        let crate::components::dialog::ItemKind::Dialog(open) = &items[1].kind else { panic!() };
        let d = open();
        assert!((d.build)(&vals(&[("size", "10")])).is_err());
        assert!(matches!((d.build)(&vals(&[("size", "11")])), Ok(OsAction::VolumeExtend { new_size_gb: 11, .. })));
    }

    #[test]
    fn rule_form_prefers_remote_group_over_cidr() {
        let d = add_rule_dialog(Lang::EnUs, "sg");
        let a = (d.build)(&vals(&[("direction", "ingress"), ("ethertype", "IPv4"), ("protocol", "tcp"), ("port_min", "22"), ("cidr", "0.0.0.0/0")])).unwrap();
        let OsAction::RuleCreate(r) = a else { panic!() };
        assert_eq!((r.port_min, r.remote_ip_prefix.as_deref()), (Some(22), Some("0.0.0.0/0")));
        let a = (d.build)(&vals(&[("direction", "egress"), ("ethertype", "IPv6"), ("remote_group", "other"), ("cidr", "0.0.0.0/0")])).unwrap();
        let OsAction::RuleCreate(r) = a else { panic!() };
        assert_eq!(r.remote_ip_prefix, None);
        assert_eq!(r.remote_group_id.as_deref(), Some("other"));
    }

    #[test]
    fn every_rule_field_has_help_and_the_to_port_label_has_no_hint() {
        let d = add_rule_dialog(Lang::EnUs, "sg");
        let missing: Vec<_> = d.fields.iter().filter(|f| f.help.is_none()).map(|f| f.name).collect();
        assert!(missing.is_empty(), "fields without help (roadmap 61): {missing:?}");
        let to = d.fields.iter().find(|f| f.name == "port_max").unwrap();
        assert_eq!(to.label, "To Port", "the \"empty = same\" hint moved to the help text");
    }

    #[test]
    fn alarm_dialog_follows_the_type_and_keeps_what_it_does_not_edit() {
        let create = alarm_dialog(Lang::EsEs, None);
        assert!(create.fields.iter().any(|f| f.name == "type"), "the type is chosen on create");
        let a = (create.build)(&vals(&[("name", "cpu"), ("type", "prometheus"), ("promql", "up"), ("comparison", "lt"), ("threshold", "0,5"), ("severity", "low"), ("enabled", "true")])).unwrap();
        let OsAction::AlarmCreate(spec) = a else { panic!("{a:?}") };
        assert_eq!((spec.query.as_str(), spec.threshold), ("up", Some(0.5)), "decimal comma");
        assert!((create.build)(&vals(&[("name", "cpu"), ("type", "prometheus"), ("promql", "up"), ("comparison", "lt"), ("severity", "low")])).is_err(), "no threshold");
        let existing = AlarmSpec {
            name: "deleted".into(),
            alarm_type: "event".into(),
            severity: "moderate".into(),
            event_type: "compute.instance.delete.end".into(),
            event_query: vec!["traits.instance_id = srv1".into()],
            time_constraints: vec![AlarmTimeConstraint { name: "office".into(), start: "0 9 * * *".into(), duration: Some(3600), timezone: None, description: None }],
            ..Default::default()
        };
        let edit = alarm_dialog(Lang::EnUs, Some(("a1", &existing)));
        let names: Vec<_> = edit.fields.iter().map(|f| f.name).collect();
        assert!(!names.contains(&"type") && !names.contains(&"threshold") && names.contains(&"event_query"), "only the fixed type's fields: {names:?}");
        let event_query = edit.fields.iter().find(|f| f.name == "event_query").unwrap();
        assert_eq!(event_query.default, "traits.instance_id = srv1");
        let a = (edit.build)(&vals(&[("name", "deleted"), ("event_type", "compute.instance.delete.end"), ("event_query", "traits.instance_id = srv1"), ("severity", "critical")])).unwrap();
        let OsAction::AlarmUpdate { id, spec } = a else { panic!("{a:?}") };
        assert_eq!((id.as_str(), spec.alarm_type.as_str(), spec.severity.as_str()), ("a1", "event", "critical"));
        assert_eq!(spec.time_constraints, existing.time_constraints, "Aodh's PUT replaces the alarm: constraints go back");
        for d in [&create, &edit] {
            let bare: Vec<_> = d.fields.iter().filter(|f| f.help.is_none()).map(|f| f.name).collect();
            assert!(bare.is_empty(), "every alarm field has a help text: {bare:?}");
        }
        let tipped = |d: &Dialog| d.fields.iter().filter(|f| f.tip.is_some()).map(|f| f.name).collect::<Vec<_>>();
        assert_eq!(
            tipped(&create),
            ["type", "promql", "metric", "metrics", "resource_type", "resource_id", "search", "aggregation", "threshold", "granularity", "evaluation_periods", "event_type", "event_query", "alarm_actions"],
            "the complex fields explain themselves behind an (i)"
        );
        // Markup that doesn't pair up would show as raw `*` or backticks.
        use crate::components::widgets::{tip_blocks, tip_inline, TipBlock, TipInline};
        fn raw(parts: &[TipInline]) -> bool {
            parts.iter().any(|p| match p {
                TipInline::Text(t) => t.contains('`') || t.contains('*'),
                TipInline::Bold(i) | TipInline::Italic(i) => raw(i),
                TipInline::Code(_) => false,
            })
        }
        for lang in Lang::ALL {
            for f in alarm_dialog(lang, None).fields.iter().filter_map(|f| f.tip.as_deref()) {
                for b in tip_blocks(f) {
                    let lines = match b {
                        TipBlock::Para(p) => vec![p],
                        TipBlock::List(items) => items,
                    };
                    for l in lines {
                        assert!(!raw(&tip_inline(&l)), "{lang:?}: unpaired markup in {l:?}");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod launch_tests {
    use super::*;

    fn sets() -> Sets {
        let image = |id: &str, bytes: u64, min_disk: i64, min_ram: i64| Choice {
            value: id.into(),
            label: id.into(),
            facts: Facts { size_bytes: Some(bytes), min_disk_gb: Some(min_disk), min_ram_mb: Some(min_ram), ..Default::default() },
            ..Default::default()
        };
        let flavor = |id: &str, ram: i64, disk: i64| Choice {
            value: id.into(),
            label: id.into(),
            facts: Facts { ram_mb: Some(ram), disk_gb: Some(disk), vcpus: Some(1), ..Default::default() },
            ..Default::default()
        };
        let mut s = Sets::new();
        s.insert(ChoiceSet::LaunchImages, vec![image("rhel", 7_800_000_000, 20, 2048), image("debian", 428_690_000, 0, 0)]);
        s.insert(ChoiceSet::Flavors, vec![flavor("small", 1024, 10), flavor("big", 4096, 50)]);
        s
    }

    fn launch_values() -> Vals {
        [("source", "image"), ("create_volume", "true"), ("volume_size", "1"), ("source_id", ""), ("networks", "n1")]
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
        assert!(p.iter().any(|(f, m)| *f == "volume_size" && m.contains("at least 20 GB")));
        assert!(p.iter().any(|(f, _)| *f == "networks"));

        let small = &s[&ChoiceSet::Flavors][0];
        let reason = block("flavor", small, &v, &s).unwrap();
        assert!(reason.contains("2048 MB of RAM"), "{reason}");
        assert_eq!(block("flavor", &s[&ChoiceSet::Flavors][1], &v, &s), None);
        // Booting straight from the image also needs the root disk.
        v.insert("create_volume".into(), "false".into());
        v.insert("source_id".into(), "debian".into());
        assert_eq!(block("flavor", small, &v, &s), None, "debian needs 1 GB and no RAM minimum");
        v.insert("ports".into(), "p1".into());
        assert!(check(&v, &s).iter().all(|(f, _)| *f != "networks"), "a port is enough");
    }
}

// ---------------------------------------------------------------------------
// Key Manager (Barbican, roadmap 55)
// ---------------------------------------------------------------------------

const SECRET_TYPES: &[&str] = &["opaque", "passphrase", "symmetric", "certificate", "private", "public"];

/// The role fields of *Create Container*, per type: (field, role). A role in
/// two types gets a field for each, since a field shows for one value only.
const CONTAINER_ROLE_FIELDS: &[(&str, &str, &str)] = &[
    ("certificate", "cert_certificate", "certificate"),
    ("certificate", "cert_private_key", "private_key"),
    ("certificate", "cert_passphrase", "private_key_passphrase"),
    ("certificate", "cert_intermediates", "intermediates"),
    ("rsa", "rsa_public_key", "public_key"),
    ("rsa", "rsa_private_key", "private_key"),
    ("rsa", "rsa_passphrase", "private_key_passphrase"),
];

fn km_options(lang: Lang, items: &[&str]) -> Source {
    Source::Static(items.iter().map(|v| (v.to_string(), crate::i18n::key_manager_term(lang, v))).collect())
}

/// *Store Secret* (Barbican `POST /v1/secrets`): name, type, payload (text, or
/// base64 for binary; empty = metadata only), algorithm, bit length, mode and
/// expiration (UTC). A file picker for binary payloads is not offered yet.
pub fn store_secret_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::text("name", tr(K::Name)).help(tr(K::SecretNameHelp)),
        Field::select("secret_type", tr(K::SecretType), km_options(lang, SECRET_TYPES)).value("opaque").required().help(tr(K::SecretTypeHelp)),
        Field::select("format", tr(K::PayloadFormat), options(&[("text/plain", K::PayloadText), ("application/octet-stream", K::PayloadBase64)], lang))
            .value("text/plain")
            .help(tr(K::PayloadFormatHelp)),
        Field::area("payload", tr(K::Payload)).help(tr(K::PayloadHelp)),
        Field::text("algorithm", tr(K::Algorithm)).help(tr(K::AlgorithmHelp)),
        Field::number("bit_length", tr(K::BitLength), 1).help(tr(K::BitLengthHelp)),
        Field::text("mode", tr(K::CipherMode)).help(tr(K::CipherModeHelp)),
        Field::date("expiration_date", tr(K::ExpirationDate), true),
        Field::time("expiration_time", tr(K::ExpirationTime)).help(tr(K::SecretExpirationHelp)),
    ];
    Dialog::form(tr(K::StoreSecret), tr(K::StoreSecret), fields, move |v| {
        let now = chrono::Utc::now().naive_utc();
        let expiration = app_credential_expiry(&v.get("expiration_date"), &v.get("expiration_time"), now).map_err(|e| invalid(lang, e))?;
        Ok(OsAction::SecretCreate(SecretSpec {
            name: v.opt("name"),
            secret_type: req(v, "secret_type", K::SecretType, lang)?,
            // Not trimmed: whitespace can be part of a secret.
            payload: Some(v.get("payload")).filter(|p| !p.is_empty()).map(Hidden),
            payload_content_type: v.get("format"),
            algorithm: v.opt("algorithm"),
            bit_length: v.num("bit_length").map_err(|e| invalid(lang, e))?,
            mode: v.opt("mode"),
            expiration,
        }))
    })
    .sized(DialogSize::Medium)
}

/// A secret row's menu: *Delete Secret* (its consumers are only known on the
/// detail page, so the dialog warns about them in general).
pub fn secret_items(lang: Lang, x: &SecretRow) -> Vec<MenuItem> {
    vec![delete_secret(lang, x, None)]
}

/// The detail page's menu: *Edit Access* and *Reset Access* (roadmap 75), then
/// *Delete Secret*, listing the secret's consumers.
pub fn secret_detail_items(lang: Lang, d: &SecretDetail) -> Vec<MenuItem> {
    let id = d.secret.id.clone();
    let target = move |project_access, users| OsAction::SecretAclSet { id: id.clone(), project_access, users };
    let reset = OsAction::SecretAclDelete { id: d.secret.id.clone() };
    let mut items = acl_items(lang, &d.secret.name, &d.acl, target, reset, None);
    items.push(delete_secret(lang, &d.secret, d.consumers.clone()));
    items
}

/// The container detail page's menu: *Edit Access* and *Reset Access* (roadmap
/// 75; the dialog reminds that the secrets inside have their own ACLs), then
/// *Delete Container*.
pub fn container_detail_items(lang: Lang, d: &ContainerDetail) -> Vec<MenuItem> {
    let c = &d.container;
    let id = c.id.clone();
    let target = move |project_access, users| OsAction::ContainerAclSet { id: id.clone(), project_access, users };
    let reset = OsAction::ContainerAclDelete { id: c.id.clone() };
    let mut items = acl_items(lang, &c.name, &d.acl, target, reset, Some(K::ContainerAclSecrets));
    items.extend(container_items(lang, c));
    items
}

/// *Edit Access* and *Reset Access* for whoever may change the ACL (the
/// creator or an admin, `KmAcl::can_edit`); *Reset* only when an ACL is set.
/// Both warn first when something uses the resource ([`KmAcl::in_use`]).
fn acl_items(
    lang: Lang,
    name: &str,
    acl: &KmAcl,
    set: impl Fn(bool, Vec<String>) -> OsAction + Send + Sync + 'static,
    reset: OsAction,
    note: Option<K>,
) -> Vec<MenuItem> {
    if !acl.can_edit {
        return vec![];
    }
    let warning = in_use_warning(lang, acl);
    let mut items = vec![edit_access(lang, acl, set, warning.clone(), note)];
    if acl.is_set() {
        items.push(reset_access(lang, name, acl, reset, warning));
    }
    items
}

/// When something uses the resource (the *Consumers* section of its page lists
/// what): removing a service's user (Octavia's) from the ACL breaks it.
fn in_use_warning(lang: Lang, acl: &KmAcl) -> Option<String> {
    acl.in_use().then(|| format!("{} {}", t(lang, K::AclInUse), t(lang, K::AclInUseWarning)))
}

/// *Edit Access*: whether every project member can read it, and which other
/// users can. Sent whole (`PUT …/acl`); *Nothing changed* when untouched.
fn edit_access(
    lang: Lang,
    acl: &KmAcl,
    set: impl Fn(bool, Vec<String>) -> OsAction + Send + Sync + 'static,
    warning: Option<String>,
    note: Option<K>,
) -> MenuItem {
    let (before_access, before_users) = (acl.project_access.unwrap_or(true), acl.users.clone());
    let set = std::sync::Arc::new(set);
    MenuItem::dialog(t(lang, K::EditAccess), move || {
        let tr = |k| t(lang, k).to_string();
        let fields = vec![
            Field::check("project_access", tr(K::ProjectAccessField), before_access),
            Field::area("users", tr(K::AclUsersField)).value(before_users.join("\n")).help(tr(K::AclUsersHelp)),
        ];
        let (set, before_users) = (set.clone(), before_users.clone());
        let mut d = Dialog::form(tr(K::EditAccess), tr(K::Save), fields, move |v| {
            let (project_access, users) = (v.flag("project_access"), parse_user_ids(&v.opt("users").unwrap_or_default()));
            if project_access == before_access && users == before_users {
                return Err(t(lang, K::NothingChanged).to_string());
            }
            Ok(set(project_access, users))
        });
        d.intro = warning.iter().cloned().chain(note.map(tr)).collect();
        d
    })
}

/// *Reset Access*: Barbican's default again; lists the users who lose access.
fn reset_access(lang: Lang, name: &str, acl: &KmAcl, reset: OsAction, warning: Option<String>) -> MenuItem {
    let users: Vec<String> = acl.users.iter().map(|u| acl.user_label(u)).collect();
    let name = name.to_string();
    MenuItem::danger(t(lang, K::ResetAccess), move || {
        let mut d = Dialog::confirm(lang, t(lang, K::ResetAccess), name.clone(), reset.clone());
        // "You have selected:" stays first: the dialog lists the selection under it.
        d.intro = vec![t(lang, K::YouSelected).into(), t(lang, K::ResetAccessIntro).into()];
        if !users.is_empty() {
            d.intro.push(format!("{}: {}", t(lang, K::AclUsersLoseAccess), users.join(", ")));
        }
        d.intro.extend(warning.clone());
        d
    })
}

fn delete_secret(lang: Lang, x: &SecretRow, consumers: Option<Vec<SecretConsumer>>) -> MenuItem {
    let (i, n) = (x.id.clone(), x.name.clone());
    MenuItem::danger(t(lang, K::DeleteSecret), move || {
        let mut d = Dialog::confirm(lang, t(lang, K::DeleteSecret), n.clone(), OsAction::SecretDelete { id: i.clone() });
        match consumers.as_deref() {
            Some([]) => {}
            Some(cs) => {
                let list: Vec<String> = cs.iter().map(|c| format!("{} {} {}", c.service, c.resource_type, c.resource_id)).collect();
                d.intro.push(format!("{}: {}", t(lang, K::SecretConsumersOf), list.join(", ")));
            }
            None => d.intro.push(t(lang, K::SecretInUse).into()),
        }
        d
    })
}

/// The (role, `secret_ref`) pairs of *Create Container*: one per role field of
/// the type, or, for `generic`, each picked secret as `secret_1`, `secret_2`…
/// (Barbican's generic roles are free names).
pub fn container_pairs(container_type: &str, get: impl Fn(&str) -> Option<String>, generic: Vec<String>) -> Vec<(String, String)> {
    if container_type == "generic" {
        return generic.into_iter().enumerate().map(|(n, r)| (format!("secret_{}", n + 1), r)).collect();
    }
    CONTAINER_ROLE_FIELDS
        .iter()
        .filter(|(ty, _, _)| *ty == container_type)
        .filter_map(|(_, field, role)| get(field).map(|r| (role.to_string(), r)))
        .collect()
}

/// *Create Container*: a name, a type, and secrets per role (the server checks
/// the roles the type needs).
pub fn create_container_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let mut fields = vec![
        Field::text("name", tr(K::Name)),
        Field::select("container_type", tr(K::ContainerType), km_options(lang, &["certificate", "rsa", "generic"])).value("certificate").required(),
        Field::multi("generic", tr(K::Secrets), Source::Set(ChoiceSet::Secrets)).when("container_type", "generic"),
    ];
    for &(ty, field, role) in CONTAINER_ROLE_FIELDS {
        let f = Field::select(field, crate::i18n::key_manager_term(lang, role), Source::Set(ChoiceSet::Secrets)).when("container_type", ty);
        fields.push(if role == "certificate" || (ty == "rsa" && role != "private_key_passphrase") { f.required() } else { f });
    }
    let mut d = Dialog::form(tr(K::CreateContainer), tr(K::CreateContainer), fields, move |v| {
        let ty = req(v, "container_type", K::ContainerType, lang)?;
        Ok(OsAction::ContainerCreate { name: v.opt("name"), secrets: container_pairs(&ty, |f| v.opt(f), v.list("generic")), container_type: ty })
    })
    .sized(DialogSize::Medium);
    d.intro = vec![tr(K::ContainerSecretsHelp)];
    d
}

/// A container's menu: *Delete Container* (its secrets stay).
pub fn container_items(lang: Lang, c: &ContainerRow) -> Vec<MenuItem> {
    let (i, n) = (c.id.clone(), c.name.clone());
    vec![MenuItem::danger(t(lang, K::DeleteContainer), move || {
        let mut d = Dialog::confirm(lang, t(lang, K::DeleteContainer), n.clone(), OsAction::ContainerDelete { id: i.clone() });
        d.intro.push(t(lang, K::ContainerKeepsSecrets).into());
        d
    })]
}

// --- Orchestration (Heat, roadmap 62) -----------------------------------------

/// A stack's menu, row and detail header alike: *Change Stack Template* (a page),
/// *Check*, *Suspend* or *Resume* (whichever applies), *Delete*. Heat refuses actions while it works on the
/// stack (`*_IN_PROGRESS`), so only *Delete* is offered then.
pub fn stack_items(lang: Lang, x: &StackRow) -> Vec<MenuItem> {
    let (name, id) = (x.name.clone(), x.id.clone());
    let simple = |label: K, make: fn(String, String) -> OsAction| {
        let (n, i) = (name.clone(), id.clone());
        MenuItem::dialog(t(lang, label), move || Dialog::simple(lang, t(lang, label), n.clone(), make(n.clone(), i.clone())))
    };
    let mut items = vec![];
    if !stack_busy(&x.status) {
        items.push(MenuItem::link(t(lang, K::ChangeStackTemplate), format!("/project/stacks/{}/change", x.id)));
        items.push(simple(K::CheckStack, |name, id| OsAction::StackCheck { name, id }));
        if stack_suspended(&x.status) {
            items.push(simple(K::ResumeStack, |name, id| OsAction::StackResume { name, id }));
        } else {
            items.push(simple(K::SuspendStack, |name, id| OsAction::StackSuspend { name, id }));
        }
    }
    items.push(MenuItem::danger(t(lang, K::DeleteStack), move || {
        let mut d = Dialog::confirm(lang, t(lang, K::DeleteStack), name.clone(), OsAction::StackDelete { name: name.clone(), id: id.clone() });
        d.intro.push(t(lang, K::StackDeletesResources).into());
        d
    }));
    items
}

/// The *Stacks* list's bulk menu over the selection (keys from [`stack_key`]).
pub fn bulk_stack_items(lang: Lang, picked: Vec<(String, String)>) -> Vec<MenuItem> {
    let item = |label: K, make: fn(String) -> OsAction| {
        let p = picked.clone();
        MenuItem::dialog(t(lang, label), move || bulk(lang, label, p.clone(), make, false))
    };
    vec![
        item(K::CheckStacks, |k| {
            let (name, id) = split_stack_key(&k);
            OsAction::StackCheck { name, id }
        }),
        item(K::SuspendStacks, |k| {
            let (name, id) = split_stack_key(&k);
            OsAction::StackSuspend { name, id }
        }),
        item(K::ResumeStacks, |k| {
            let (name, id) = split_stack_key(&k);
            OsAction::StackResume { name, id }
        }),
    ]
}

#[cfg(test)]
mod stack_tests {
    use super::*;

    fn row(status: &str) -> StackRow {
        StackRow {
            id: "s1".into(),
            name: "web".into(),
            description: None,
            status: status.into(),
            status_reason: None,
            created: String::new(),
            updated: None,
        }
    }

    #[test]
    fn stack_menu_follows_the_status() {
        let l = Lang::EnUs;
        let labels = |s: &str| stack_items(l, &row(s)).into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(labels("CREATE_COMPLETE"), ["Change Stack Template", "Check Stack", "Suspend Stack", "Delete Stack"]);
        assert_eq!(labels("SUSPEND_COMPLETE"), ["Change Stack Template", "Check Stack", "Resume Stack", "Delete Stack"]);
        assert_eq!(labels("UPDATE_IN_PROGRESS"), ["Delete Stack"], "Heat refuses actions while busy");
    }

    #[test]
    fn stack_keys_round_trip() {
        assert_eq!(split_stack_key(&stack_key("web", "s1")), ("web".into(), "s1".into()));
        assert_eq!(split_stack_key("broken"), (String::new(), String::new()));
    }
}

#[cfg(test)]
mod key_manager_tests {
    use super::*;

    fn secret(can_edit: bool, project_access: Option<bool>, users: &[&str]) -> SecretDetail {
        SecretDetail {
            secret: SecretRow {
                id: "s1".into(),
                name: "db".into(),
                secret_type: "opaque".into(),
                algorithm: None,
                bit_length: None,
                mode: None,
                content_type: None,
                status: "ACTIVE".into(),
                expiration: None,
                created: String::new(),
                updated: None,
                creator_id: Some("u0".into()),
                secret_ref: "https://kms/v1/secrets/s1".into(),
            },
            acl: KmAcl {
                users: users.iter().map(|u| u.to_string()).collect(),
                project_access,
                user_names: vec![("u1".into(), "octavia".into())],
                can_edit,
                in_use: vec![],
                octavia: vec![],
            },
            consumers: Some(vec![]),
            payload_allowed: false,
        }
    }

    #[test]
    fn secret_access_items_follow_policy_and_acl() {
        let labels = |d: SecretDetail| secret_detail_items(Lang::EnUs, &d).into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(labels(secret(false, Some(false), &["u1"])), ["Delete Secret"], "neither creator nor admin");
        assert_eq!(labels(secret(true, Some(true), &[])), ["Edit Access", "Delete Secret"], "Barbican's default: nothing to reset");
        assert_eq!(labels(secret(true, Some(true), &["u1"])), ["Edit Access", "Reset Access", "Delete Secret"]);
        assert_eq!(labels(secret(true, Some(false), &[])), ["Edit Access", "Reset Access", "Delete Secret"], "private");
        assert_eq!(secret(true, None, &[]).acl.user_label("u1"), "octavia (u1)");
        assert_eq!(secret(true, None, &[]).acl.user_label("u9"), "u9");
    }

    #[test]
    fn access_dialogs_warn_when_something_uses_it() {
        let mut d = secret(true, Some(true), &["u1"]);
        assert_eq!(in_use_warning(Lang::EnUs, &d.acl), None);
        d.acl.octavia = vec![OctaviaUse { lb_id: Some("lb1".into()), lb_name: "web".into(), kind: "listener".into(), name: "https".into(), via: None }];
        let w = in_use_warning(Lang::EnUs, &d.acl).unwrap();
        assert!(w.contains("Consumers"), "points at the page's list: {w}");
        let items = secret_detail_items(Lang::EnUs, &d);
        let crate::components::dialog::ItemKind::Dialog(open) = &items[1].kind else { panic!("Reset Access opens a dialog") };
        assert!(open().intro.contains(&w), "Reset Access shows the warning");
        let crate::components::dialog::ItemKind::Dialog(open) = &items[0].kind else { panic!("Edit Access opens a dialog") };
        assert!(open().intro.contains(&w), "Edit Access too");
    }

    #[test]
    fn container_pairs_follow_the_type() {
        let picked = |f: &str| match f {
            "cert_certificate" => Some("https://kms/v1/secrets/c".to_string()),
            "cert_private_key" => Some("https://kms/v1/secrets/k".to_string()),
            "rsa_public_key" => Some("https://kms/v1/secrets/p".to_string()),
            _ => None,
        };
        let refs = |v: Vec<(String, String)>| v.into_iter().map(|(r, s)| format!("{r}={s}")).collect::<Vec<_>>();
        assert_eq!(refs(container_pairs("certificate", picked, vec![])), ["certificate=https://kms/v1/secrets/c", "private_key=https://kms/v1/secrets/k"], "fields of another type are ignored");
        assert_eq!(refs(container_pairs("rsa", picked, vec![])), ["public_key=https://kms/v1/secrets/p"]);
        assert_eq!(refs(container_pairs("generic", picked, vec!["a".into(), "b".into()])), ["secret_1=a", "secret_2=b"]);
    }
}

// ---------------------------------------------------------------------------
// Identity (roadmap 57)
// ---------------------------------------------------------------------------

/// Horizon's *Create Application Credential*: name, description, secret (empty
/// = generated), expiration date and time (UTC; a time alone means today),
/// roles (a subset of the user's in the current project; none = all), access
/// rules (JSON or YAML, parsed on the server) and *Unrestricted*. The
/// Kubernetes namespace (Horizon's `KUBECONFIG_ENABLED`) is not offered.
/// The secret comes back once, with `openrc` and `clouds.yaml`.
pub fn create_app_credential_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::text("name", tr(K::Name)).required(),
        Field::text("description", tr(K::Description)),
        Field::password("secret", tr(K::Secret)).help(tr(K::SecretHelp)),
        Field::date("expiration_date", tr(K::ExpirationDate), true),
        Field::time("expiration_time", tr(K::ExpirationTime)).help(tr(K::ExpirationHelp)),
        Field::multi("roles", tr(K::Roles), Source::Set(ChoiceSet::TokenRoles)).help(tr(K::AppCredRolesHelp)),
        // Horizon's examples, in both of the formats it accepts.
        Field::area("access_rules", tr(K::AccessRules)).help(tr(K::AccessRulesHelp)).help_code(
            "[\n  {\"service\": \"compute\",\n   \"method\": \"POST\",\n   \"path\": \"/v2.1/servers\"}\n]\n\n- service: compute\n  method: POST\n  path: /v2.1/servers",
        ),
        Field::check("unrestricted", tr(K::UnrestrictedDangerous), false).help(tr(K::UnrestrictedHelp)).danger(),
    ];
    let mut d = Dialog::form(tr(K::CreateAppCredential), tr(K::CreateAppCredential), fields, move |v| {
        let now = chrono::Utc::now().naive_utc();
        let expires_at = app_credential_expiry(&v.get("expiration_date"), &v.get("expiration_time"), now).map_err(|e| invalid(lang, e))?;
        Ok(OsAction::AppCredentialCreate(Box::new(AppCredentialSpec {
            // Set from the session by `run_action`.
            user_id: String::new(),
            name: req(v, "name", K::Name, lang)?,
            description: v.opt("description"),
            secret: v.opt("secret"),
            expires_at,
            roles: v.list("roles"),
            unrestricted: v.flag("unrestricted"),
            access_rules: v.opt("access_rules"),
        })))
    })
    .sized(DialogSize::Medium);
    d.intro = vec![tr(K::AppCredHelp)];
    d
}

/// Horizon's *Create Role* (admins): a role name.
pub fn create_role_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    Dialog::form(tr(K::CreateRole), tr(K::CreateRole), vec![Field::text("name", tr(K::RoleName)).required()], move |v| {
        Ok(OsAction::RoleCreate { name: req(v, "name", K::RoleName, lang)? })
    })
}

/// A role's menu, as Horizon's: *Edit* (the name) and *Delete Role*.
pub fn role_items(lang: Lang, r: &IdentityRole) -> Vec<MenuItem> {
    let (i, n) = (r.id.clone(), r.name.clone());
    let (i2, n2) = (i.clone(), n.clone());
    vec![
        MenuItem::dialog(t(lang, K::Edit), move || {
            let id = i.clone();
            let tr = |k| t(lang, k).to_string();
            Dialog::form(tr(K::EditRole), tr(K::Save), vec![Field::text("name", tr(K::RoleName)).value(n.clone()).required()], move |v| {
                Ok(OsAction::RoleUpdate { id: id.clone(), name: req(v, "name", K::RoleName, lang)? })
            })
        }),
        MenuItem::danger(t(lang, K::DeleteRole), move || {
            let mut d = Dialog::confirm(lang, t(lang, K::DeleteRole), n2.clone(), OsAction::RoleDelete { id: i2.clone() });
            d.intro.push(t(lang, K::DeleteRoleWarning).to_string());
            d
        }),
    ]
}

/// An application credential's menu (only *Delete*).
pub fn app_credential_items(lang: Lang, a: &AppCredential) -> Vec<MenuItem> {
    let (i, n) = (a.id.clone(), a.name.clone());
    vec![MenuItem::danger(t(lang, K::DeleteAppCredential), move || {
        Dialog::confirm(lang, t(lang, K::DeleteAppCredential), n.clone(), OsAction::AppCredentialDelete { user_id: String::new(), id: i.clone() })
    })]
}

/// Horizon's *Create Project* (admins): name, description, enabled; created in
/// the token's domain.
pub fn create_project_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        // Where it goes, as Horizon shows it (the same domain `run_action` sets).
        Field::info("domain", tr(K::Domain), ChoiceSet::OperationDomain),
        Field::text("name", tr(K::Name)).required(),
        Field::area("description", tr(K::Description)),
        Field::check("enabled", tr(K::EnabledCol), true),
    ];
    Dialog::form(tr(K::CreateProject), tr(K::CreateProject), fields, move |v| {
        // `domain_id` is set from the session by `run_action`.
        Ok(OsAction::ProjectCreate { name: req(v, "name", K::Name, lang)?, description: v.opt("description"), enabled: v.flag("enabled"), domain_id: String::new() })
    })
}

/// A project's menu (admins), as Horizon's: *Manage Members*, *Modify Groups*,
/// *Edit Project*, *Delete Project*. The members dialogs need the project's
/// assignments, so they are offered on the detail page (`detail`).
pub fn identity_project_items(lang: Lang, p: &IdentityProject, detail: Option<&IdentityProjectDetail>, admin: bool) -> Vec<MenuItem> {
    let tr = move |k| t(lang, k).to_string();
    let mut items = Vec::new();
    if let Some(d) = detail.filter(|d| d.users.is_some()) {
        let (pid, users) = (p.id.clone(), d.users.clone().unwrap_or_default());
        let before: Vec<AssignmentRow> = users
            .iter()
            .filter(|m| !m.role_ids.is_empty())
            .map(|m| AssignmentRow { actor_id: m.id.clone(), label: m.name.clone(), role_ids: m.role_ids.clone() })
            .collect();
        items.push(MenuItem::dialog(tr(K::ManageMembers), move || assignments_dialog(lang, K::ManageMembers, &pid, false, &before)));
        let (pid, groups) = (p.id.clone(), d.groups.clone().unwrap_or_default());
        let before: Vec<AssignmentRow> = groups.iter().map(|g| AssignmentRow { actor_id: g.id.clone(), label: g.name.clone(), role_ids: g.role_ids.clone() }).collect();
        items.push(MenuItem::dialog(tr(K::ModifyGroups), move || assignments_dialog(lang, K::ModifyGroups, &pid, true, &before)));
    }
    let p2 = p.clone();
    items.push(MenuItem::dialog(tr(K::EditProject), move || {
        let id = p2.id.clone();
        let fields = vec![
            Field::text("name", tr(K::Name)).value(p2.name.clone()).required(),
            Field::area("description", tr(K::Description)).value(p2.description.clone().unwrap_or_default()),
            Field::check("enabled", tr(K::EnabledCol), p2.enabled),
        ];
        Dialog::form(tr(K::EditProject), tr(K::Save), fields, move |v| {
            Ok(OsAction::ProjectUpdate { id: id.clone(), name: req(v, "name", K::Name, lang)?, description: v.get("description"), enabled: v.flag("enabled") })
        })
    }));
    // After *Edit Project*, as Horizon. *Modify Quotas* (roadmap 82): any project's quotas, for the
    // `admin` role only (`needs::ADMIN`); the server checks the role again. The
    // session's own project is edited from Project › Quotas by whoever may.
    if admin {
        items.push(MenuItem::link(tr(K::ModifyQuotas), format!("/identity/projects/{}/quotas", p.id)));
    }
    let (i, n) = (p.id.clone(), p.name.clone());
    items.push(MenuItem::danger(tr(K::DeleteProject), move || Dialog::confirm(lang, tr(K::DeleteProject), n.clone(), OsAction::ProjectDelete { id: i.clone() })));
    items
}

/// *Manage Members* (users) / *Modify Groups* (groups): the project's direct
/// assignments edited at once, sent as the grants and revokes that changed.
fn assignments_dialog(lang: Lang, title: K, project_id: &str, group: bool, before: &[AssignmentRow]) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let actors = if group { ChoiceSet::IdentityGroups } else { ChoiceSet::IdentityUsers };
    let label = if group { K::Groups } else { K::Users };
    let (pid, before) = (project_id.to_string(), before.to_vec());
    let before2 = before.clone();
    Dialog::form(tr(title), tr(K::Save), vec![Field::assignments("rows", tr(label), &before, actors)], move |v| {
        let after: Vec<AssignmentRow> = serde_json::from_str(&v.get("rows")).unwrap_or_default();
        let changes = assignment_changes(&pid, group, &before2, &after);
        if changes.is_empty() {
            return Err(t(lang, K::NothingChanged).to_string());
        }
        Ok(OsAction::Batch(changes))
    })
    .sized(DialogSize::Large)
}

/// Horizon's *Create User* (admins): name, description, email, password twice,
/// primary project and a role on it, enabled, lock password.
pub fn create_user_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::info("domain", tr(K::Domain), ChoiceSet::OperationDomain),
        Field::text("name", tr(K::UserName)).required(),
        Field::area("description", tr(K::Description)),
        Field::text("email", tr(K::Email)),
        Field::password("password", tr(K::Password)).required(),
        Field::password("confirm", tr(K::ConfirmPassword)).required(),
        Field::select("project", tr(K::PrimaryProject), Source::Set(ChoiceSet::IdentityProjects)),
        Field::select("role", tr(K::Role), Source::Set(ChoiceSet::IdentityRoles)).show_if(|v| v.get("project").is_some_and(|p| !p.is_empty())),
        Field::check("enabled", tr(K::EnabledCol), true),
        Field::check("lock_password", tr(K::LockPassword), false),
    ];
    Dialog::form(tr(K::CreateUser), tr(K::CreateUser), fields, move |v| {
        let password = v.get("password");
        if password != v.get("confirm") {
            return Err(t(lang, K::PasswordsDiffer).to_string());
        }
        let project_id = v.opt("project");
        Ok(OsAction::UserCreate(Box::new(UserSpec {
            // Set from the session by `run_action`.
            domain_id: String::new(),
            name: req(v, "name", K::UserName, lang)?,
            description: v.opt("description"),
            email: v.opt("email"),
            password,
            role_id: project_id.as_ref().and(v.opt("role")),
            project_id,
            enabled: v.flag("enabled"),
            lock_password: v.flag("lock_password"),
        })))
    })
}

/// A user's menu (admins), as Horizon's: *Edit*, *Change Password*,
/// *Enable/Disable User*, *Delete User*. `run_action` refuses disabling or
/// deleting oneself.
pub fn identity_user_items(lang: Lang, d: &IdentityUserDetail) -> Vec<MenuItem> {
    if d.assignments.is_none() {
        // Not an admin: Horizon offers no actions on the user's own row either.
        return vec![];
    }
    user_items(lang, &d.user, d.primary_project_id.clone())
}

/// The same menu for a row of the Users list (admins).
pub fn user_items(lang: Lang, u: &IdentityUser, primary: Option<String>) -> Vec<MenuItem> {
    let tr = move |k| t(lang, k).to_string();
    let u = u.clone();
    let mut items = Vec::new();
    let u2 = u.clone();
    items.push(MenuItem::dialog(tr(K::Edit), move || {
        let id = u2.id.clone();
        let fields = vec![
            Field::text("name", tr(K::UserName)).value(u2.name.clone()).required(),
            Field::text("email", tr(K::Email)).value(u2.email.clone().unwrap_or_default()),
            Field::area("description", tr(K::Description)).value(u2.description.clone().unwrap_or_default()),
            Field::select("project", tr(K::PrimaryProject), Source::Set(ChoiceSet::IdentityProjects)).value(primary.clone().unwrap_or_default()),
        ];
        Dialog::form(tr(K::EditUser), tr(K::Save), fields, move |v| {
            Ok(OsAction::UserUpdate { id: id.clone(), name: req(v, "name", K::UserName, lang)?, email: v.get("email"), description: v.get("description"), project_id: v.opt("project") })
        })
    }));
    let id = u.id.clone();
    items.push(MenuItem::dialog(tr(K::ChangePassword), move || {
        let id = id.clone();
        let fields = vec![Field::password("password", tr(K::Password)).required(), Field::password("confirm", tr(K::ConfirmPassword)).required()];
        Dialog::form(tr(K::ChangePassword), tr(K::Save), fields, move |v| {
            let password = v.get("password");
            if password != v.get("confirm") {
                return Err(t(lang, K::PasswordsDiffer).to_string());
            }
            Ok(OsAction::UserSetPassword { id: id.clone(), password })
        })
    }));
    let (id, name, enabled) = (u.id.clone(), u.name.clone(), u.enabled);
    let label = tr(if enabled { K::DisableUser } else { K::EnableUser });
    items.push(MenuItem::dialog(label.clone(), move || Dialog::simple(lang, label.clone(), name.clone(), OsAction::UserSetEnabled { id: id.clone(), enabled: !enabled })));
    let (i, n) = (u.id.clone(), u.name.clone());
    items.push(MenuItem::danger(tr(K::DeleteUser), move || Dialog::confirm(lang, tr(K::DeleteUser), n.clone(), OsAction::UserDelete { id: i.clone() })));
    items
}

/// Horizon's *Create Group* (admins).
pub fn create_group_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let fields = vec![
        Field::info("domain", tr(K::Domain), ChoiceSet::OperationDomain),
        Field::text("name", tr(K::Name)).required(),
        Field::area("description", tr(K::Description)),
    ];
    Dialog::form(tr(K::CreateGroup), tr(K::CreateGroup), fields, move |v| {
        // `domain_id` is set from the session by `run_action`.
        Ok(OsAction::GroupCreate { name: req(v, "name", K::Name, lang)?, description: v.opt("description"), domain_id: String::new() })
    })
}

/// A group's menu (admins): *Manage Members* (on the detail page, which knows
/// the members), *Edit Group*, *Delete Group*.
pub fn identity_group_items(lang: Lang, d: &IdentityGroupDetail) -> Vec<MenuItem> {
    let mut items = group_menu(lang, &d.group);
    let (gid, before): (String, Vec<String>) = (d.group.id.clone(), d.users.iter().map(|u| u.id.clone()).collect());
    let tr = move |k| t(lang, k).to_string();
    items.insert(
        0,
        MenuItem::dialog(tr(K::ManageMembers), move || {
            let (gid, before) = (gid.clone(), before.clone());
            let field = Field::pick_many("users", tr(K::Users), Source::Set(ChoiceSet::IdentityUsers)).value(before.join(","));
            Dialog::form(tr(K::ManageMembers), tr(K::Save), vec![field], move |v| {
                let after = v.list("users");
                let mut changes: Vec<(String, OsAction)> = after
                    .iter()
                    .filter(|u| !before.contains(u))
                    .map(|u| (format!("+ {u}"), OsAction::GroupAddUser { group_id: gid.clone(), user_id: u.clone() }))
                    .collect();
                changes.extend(before.iter().filter(|u| !after.contains(u)).map(|u| (format!("- {u}"), OsAction::GroupRemoveUser { group_id: gid.clone(), user_id: u.clone() })));
                if changes.is_empty() {
                    return Err(t(lang, K::NothingChanged).to_string());
                }
                Ok(OsAction::Batch(changes))
            })
            .sized(DialogSize::Large)
        }),
    );
    items
}

/// A group row's menu: *Edit Group*, *Delete Group*.
pub fn group_menu(lang: Lang, g: &IdentityGroup) -> Vec<MenuItem> {
    let tr = move |k| t(lang, k).to_string();
    let g2 = g.clone();
    let (i, n) = (g.id.clone(), g.name.clone());
    vec![
        MenuItem::dialog(tr(K::EditGroup), move || {
            let id = g2.id.clone();
            let fields = vec![
                Field::text("name", tr(K::Name)).value(g2.name.clone()).required(),
                Field::area("description", tr(K::Description)).value(g2.description.clone().unwrap_or_default()),
            ];
            Dialog::form(tr(K::EditGroup), tr(K::Save), fields, move |v| {
                Ok(OsAction::GroupUpdate { id: id.clone(), name: req(v, "name", K::Name, lang)?, description: v.get("description") })
            })
        }),
        MenuItem::danger(tr(K::DeleteGroup), move || Dialog::confirm(lang, tr(K::DeleteGroup), n.clone(), OsAction::GroupDelete { id: i.clone() })),
    ]
}
