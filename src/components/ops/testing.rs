//! Helpers shared by the ops tests.

use super::*;
use std::collections::HashMap;

pub(super) fn vals(pairs: &[(&str, &str)]) -> Values {
    Values(
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<HashMap<_, _>>(),
    )
}

pub(super) fn dialog_of(items: &[MenuItem], label: &str) -> Dialog {
    let item = items
        .iter()
        .find(|i| i.label == label)
        .unwrap_or_else(|| panic!("{label}"));
    let crate::components::dialog::ItemKind::Dialog(open) = &item.kind else {
        panic!("{label} opens no dialog")
    };
    open()
}

pub(super) fn instance(status: &str, locked: bool) -> Instance {
    Instance {
        id: "i".into(),
        name: "n".into(),
        image_name: None,
        addresses: vec![],
        flavor_name: "small".into(),
        flavor: FlavorInfo {
            id: "f1".into(),
            name: "small".into(),
            vcpus: None,
            ram_mb: None,
            disk_gb: None,
            ephemeral_gb: None,
            swap_mb: None,
        },
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

pub(super) fn volume(status: &str) -> Volume {
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
