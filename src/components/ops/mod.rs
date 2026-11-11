//! What each resource can do: Horizon's create buttons and row-action menus,
//! expressed as dialogs that build an [`OsAction`]. Shared by list and detail pages.

//! What each resource can do: Horizon's create buttons and row-action menus,
//! expressed as dialogs that build an [`OsAction`]. Shared by list and detail pages.

use crate::components::dialog::{
    Dialog, DialogSize, Field, MenuItem, Sets, Source, Step, Vals, Values, caps, options, options_raw,
};
use crate::components::upload::{Picked, guess_format};
use crate::i18n::{K, Lang, t};
use crate::model::*;
use crate::prefs::ImageUpload;
use std::sync::Arc;

mod admin_compute;
mod admin_storage;
mod alarms;
mod firewall;
mod floating_ips;
mod identity;
mod images;
mod instances;
mod key_manager;
mod networks;
mod octavia;
mod octavia_parts;
mod qos;
mod quotas;
mod routers;
mod security_groups;
mod stacks;
#[cfg(test)]
mod testing;
mod trunks;
mod volumes;

pub use admin_compute::*;
pub use admin_storage::*;
pub use alarms::*;
pub use firewall::*;
pub use floating_ips::*;
pub use identity::*;
pub use images::*;
pub use instances::*;
pub use key_manager::*;
pub use networks::*;
pub use octavia::*;
pub use octavia_parts::*;
pub use qos::*;
pub use quotas::*;
pub use routers::*;
pub use security_groups::*;
pub use stacks::*;
pub use trunks::*;
pub use volumes::*;

fn req(v: &Values, k: &str, label: K, lang: Lang) -> Result<String, String> {
    v.opt(k)
        .ok_or_else(|| format!("{}: {}", t(lang, label), t(lang, K::RequiredField)))
}

fn num_u32(v: &Values, k: &str, label: K, lang: Lang) -> Result<Option<u32>, String> {
    match v.num(k).map_err(|_| format!("{}: ?", t(lang, label)))? {
        None => Ok(None),
        Some(n) if n >= 0 && n <= u32::MAX as i64 => Ok(Some(n as u32)),
        Some(_) => Err(format!("{}: ?", t(lang, label))),
    }
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
        if too_many {
            Err(t(lang, K::TooManySelected).to_string())
        } else {
            Ok(batch.clone())
        }
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

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn bulk_delete_lists_every_name_and_batches_in_order() {
        let items = vec![
            ("v1".to_string(), "data".to_string()),
            ("v2".to_string(), "logs".to_string()),
        ];
        let d = bulk_delete(Lang::EnUs, K::DeleteVolumes, items, |id| OsAction::VolumeDelete { id });
        assert!(d.danger);
        assert_eq!(d.selection, ["data", "logs"], "one bullet per name");
        let OsAction::Batch(b) = (d.build)(&vals(&[])).unwrap() else {
            panic!()
        };
        assert_eq!(b[1], ("logs".to_string(), OsAction::VolumeDelete { id: "v2".into() }));
        let many = (0..=MAX_BATCH).map(|i| (i.to_string(), i.to_string())).collect();
        let d = bulk_delete(Lang::EnUs, K::DeleteVolumes, many, |id| OsAction::VolumeDelete { id });
        assert!((d.build)(&vals(&[])).is_err());
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
        for op in [
            Resize,
            Rebuild,
            Shelve,
            Rescue,
            AttachInterface,
            AssociateIp,
            Pause,
            SoftReboot,
        ] {
            assert!(active.contains(&op), "{op:?}");
        }
        assert!(allowed("RESCUE", Some(false), false).contains(&Unrescue));
        assert!(!active.contains(&Unrescue));
        let shelved = allowed("SHELVED", Some(false), false);
        for op in [Resize, Rebuild, Snapshot, Pause, HardReboot, AttachInterface] {
            assert!(!shelved.contains(&op), "{op:?} on a shelved instance");
        }
        assert_eq!(
            allowed("ACTIVE", Some(true), true),
            [Edit, Unlock, Delete],
            "busy: almost nothing"
        );
        assert_eq!(
            allowed("SHUTOFF", Some(false), false)[0],
            Start,
            "start first when stopped"
        );
        assert_eq!(allowed("ACTIVE", Some(false), false)[0], Snapshot);
        assert_eq!(allowed("BUILD", None, true), [Edit, Lock, Unlock, Delete]);
        for s in ["ACTIVE", "SHUTOFF", "ERROR", "SHELVED", "VERIFY_RESIZE"] {
            assert_eq!(
                allowed(s, Some(false), false).last(),
                Some(&Delete),
                "{s}: delete is last"
            );
        }
    }
}
