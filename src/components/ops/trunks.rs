//! Trunks and subports.

use super::*;

/// Sub-port fields; `prefix` keeps the create dialog's optional first sub-port apart.
fn subport_fields(lang: Lang, optional: bool) -> Vec<Field> {
    let tr = |k| t(lang, k).to_string();
    let port = Field::select("subport", tr(K::Subport), Source::Set(ChoiceSet::FreePorts));
    let vlan: fn(&Vals) -> bool = |v| {
        v.get("segmentation_type").map(String::as_str) != Some("inherit")
            && v.get("subport").is_some_and(|p| !p.is_empty())
    };
    vec![
        if optional {
            port.help(tr(K::SubportOptionalHelp))
        } else {
            port.required()
        },
        Field::select(
            "segmentation_type",
            tr(K::SegmentationType),
            options_raw(&["vlan", "inherit"]),
        )
        .value("vlan"),
        Field::number("segmentation_id", tr(K::SegmentationId), 1)
            .help(tr(K::VlanHelp))
            .show_if(vlan),
    ]
}

/// The sub-port of *Create Trunk* / *Add Subports*, `None` when no port is picked. A VLAN needs an
/// ID of 1-4094 (checked by `SubportSpec::body`, which the action sends); `inherit` takes none.
fn subport_spec(v: &Values, lang: Lang) -> Result<Option<SubportSpec>, String> {
    let Some(port_id) = v.opt("subport") else {
        return Ok(None);
    };
    let kind = v.opt("segmentation_type").unwrap_or_else(|| "vlan".into());
    let id = match v.num("segmentation_id").map_err(|_| t(lang, K::VlanHelp).to_string())? {
        Some(n) if kind == "vlan" => Some(u16::try_from(n).map_err(|_| t(lang, K::VlanHelp).to_string())?),
        _ => None,
    };
    let spec = SubportSpec {
        port_id,
        segmentation_type: kind,
        segmentation_id: id,
    };
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
        Ok(OsAction::TrunkCreate {
            name: v.get("name"),
            description: v.opt("description"),
            port_id: parent,
            admin_up: v.flag("admin_up"),
            subports: sub.into_iter().collect(),
        })
    })
}

/// *Add Sub-port* to a trunk.
pub fn add_subport_dialog(lang: Lang, trunk_id: &str) -> Dialog {
    let id = trunk_id.to_string();
    Dialog::form(
        t(lang, K::AddSubport),
        t(lang, K::AddSubport),
        subport_fields(lang, false),
        move |v| {
            let sub = subport_spec(v, lang)?
                .ok_or_else(|| format!("{}: {}", t(lang, K::Subport), t(lang, K::RequiredField)))?;
            Ok(OsAction::TrunkAddSubports {
                id: id.clone(),
                subports: vec![sub],
            })
        },
    )
}

/// A trunk's menu: add sub-port, edit, delete.
pub fn trunk_items(lang: Lang, tk: &TrunkRow) -> Vec<MenuItem> {
    let i = tk.id.clone();
    let add = MenuItem::dialog(t(lang, K::AddSubport), move || add_subport_dialog(lang, &i));
    let (i, n, d, up) = (
        tk.id.clone(),
        tk.name.clone(),
        tk.description.clone().unwrap_or_default(),
        tk.admin_up,
    );
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
            move |v| {
                Ok(OsAction::TrunkUpdate {
                    id: i.clone(),
                    name: v.get("name"),
                    description: v.get("description"),
                    admin_up: v.flag("admin_up"),
                })
            },
        )
    });
    let (i, n) = (tk.id.clone(), tk.name.clone());
    let delete = MenuItem::danger(t(lang, K::DeleteTrunk), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteTrunk),
            n.clone(),
            OsAction::TrunkDelete { id: i.clone() },
        )
    });
    vec![add, edit, delete]
}

/// A trunk sub-port's menu (*Remove Sub-port*).
pub fn subport_items(lang: Lang, trunk_id: &str, sp: &Subport) -> Vec<MenuItem> {
    let (i, port) = (trunk_id.to_string(), sp.port_id.clone());
    let what = format!(
        "{} (VLAN {})",
        sp.port_name,
        sp.segmentation_id
            .map(|n| n.to_string())
            .unwrap_or_else(|| sp.segmentation_type.clone())
    );
    vec![MenuItem::danger(t(lang, K::RemoveSubport), move || {
        Dialog::confirm(
            lang,
            t(lang, K::RemoveSubport),
            what.clone(),
            OsAction::TrunkRemoveSubports {
                id: i.clone(),
                port_ids: vec![port.clone()],
            },
        )
    })]
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[test]
    fn trunk_dialogs_build_subports() {
        let d = create_trunk_dialog(Lang::EnUs);
        let a = (d.build)(&vals(&[
            ("parent", "p0"),
            ("admin_up", "true"),
            ("subport", "p1"),
            ("segmentation_type", "vlan"),
            ("segmentation_id", "101"),
        ]))
        .unwrap();
        let OsAction::TrunkCreate { port_id, subports, .. } = a else {
            panic!("not a create")
        };
        assert_eq!((port_id.as_str(), subports[0].segmentation_id), ("p0", Some(101)));
        let none = (d.build)(&vals(&[("parent", "p0")])).unwrap();
        assert!(
            matches!(none, OsAction::TrunkCreate { ref subports, .. } if subports.is_empty()),
            "the first sub-port is optional"
        );
        assert!(
            (d.build)(&vals(&[("parent", "p0"), ("subport", "p0"), ("segmentation_id", "5")])).is_err(),
            "parent is not a sub-port"
        );
        let add = add_subport_dialog(Lang::EnUs, "t1");
        let a = (add.build)(&vals(&[
            ("subport", "p2"),
            ("segmentation_type", "inherit"),
            ("segmentation_id", "7"),
        ]))
        .unwrap();
        assert!(
            matches!(a, OsAction::TrunkAddSubports { ref subports, .. } if subports[0].segmentation_id.is_none()),
            "inherit ignores the ID"
        );
        assert!(
            (add.build)(&vals(&[
                ("subport", "p2"),
                ("segmentation_type", "vlan"),
                ("segmentation_id", "5000")
            ]))
            .is_err()
        );
    }
}
