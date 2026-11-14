//! Identity (roadmap 57).

use super::*;

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
        // One value, so on one line: date left, time right (roadmap 111).
        Field::date("expiration_date", tr(K::ExpirationDate), true).half(),
        Field::time("expiration_time", tr(K::ExpirationTime)).half().help(tr(K::ExpirationHelp)),
        Field::multi("roles", tr(K::Roles), Source::Set(ChoiceSet::TokenRoles)).help(tr(K::AppCredRolesHelp)),
        // Horizon's examples, in both of the formats it accepts.
        Field::area("access_rules", tr(K::AccessRules)).help(tr(K::AccessRulesHelp)).help_code(
            "[\n  {\"service\": \"compute\",\n   \"method\": \"POST\",\n   \"path\": \"/v2.1/servers\"}\n]\n\n- service: compute\n  method: POST\n  path: /v2.1/servers",
        ),
        Field::check("unrestricted", tr(K::UnrestrictedDangerous), false).help(tr(K::UnrestrictedHelp)).danger(),
    ];
    let mut d = Dialog::form(
        tr(K::CreateAppCredential),
        tr(K::CreateAppCredential),
        fields,
        move |v| {
            let now = chrono::Utc::now().naive_utc();
            let expires_at = app_credential_expiry(&v.get("expiration_date"), &v.get("expiration_time"), now)
                .map_err(|e| invalid(lang, e))?;
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
        },
    )
    .sized(DialogSize::Medium);
    d.intro = vec![tr(K::AppCredHelp)];
    d
}

/// Horizon's *Create Role* (admins): a role name.
pub fn create_role_dialog(lang: Lang) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    Dialog::form(
        tr(K::CreateRole),
        tr(K::CreateRole),
        vec![Field::text("name", tr(K::RoleName)).required()],
        move |v| {
            Ok(OsAction::RoleCreate {
                name: req(v, "name", K::RoleName, lang)?,
            })
        },
    )
}

/// A role's menu, as Horizon's: *Edit* (the name) and *Delete Role*.
pub fn role_items(lang: Lang, r: &IdentityRole) -> Vec<MenuItem> {
    let (i, n) = (r.id.clone(), r.name.clone());
    let (i2, n2) = (i.clone(), n.clone());
    vec![
        MenuItem::dialog(t(lang, K::Edit), move || {
            let id = i.clone();
            let tr = |k| t(lang, k).to_string();
            Dialog::form(
                tr(K::EditRole),
                tr(K::Save),
                vec![Field::text("name", tr(K::RoleName)).value(n.clone()).required()],
                move |v| {
                    Ok(OsAction::RoleUpdate {
                        id: id.clone(),
                        name: req(v, "name", K::RoleName, lang)?,
                    })
                },
            )
        }),
        MenuItem::danger(t(lang, K::DeleteRole), move || {
            let mut d = Dialog::confirm(
                lang,
                t(lang, K::DeleteRole),
                n2.clone(),
                OsAction::RoleDelete { id: i2.clone() },
            );
            d.intro.push(t(lang, K::DeleteRoleWarning).to_string());
            d
        }),
    ]
}

/// An application credential's menu (only *Delete*).
pub fn app_credential_items(lang: Lang, a: &AppCredential) -> Vec<MenuItem> {
    let (i, n) = (a.id.clone(), a.name.clone());
    vec![MenuItem::danger(t(lang, K::DeleteAppCredential), move || {
        Dialog::confirm(
            lang,
            t(lang, K::DeleteAppCredential),
            n.clone(),
            OsAction::AppCredentialDelete {
                user_id: String::new(),
                id: i.clone(),
            },
        )
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
        Ok(OsAction::ProjectCreate {
            name: req(v, "name", K::Name, lang)?,
            description: v.opt("description"),
            enabled: v.flag("enabled"),
            domain_id: String::new(),
        })
    })
}

/// A project's menu (admins), as Horizon's: *Manage Members*, *Modify Groups*,
/// *Edit Project*, *Delete Project*. The members dialogs need the project's
/// assignments, so they are offered on the detail page (`detail`).
pub fn identity_project_items(
    lang: Lang,
    p: &IdentityProject,
    detail: Option<&IdentityProjectDetail>,
    admin: bool,
) -> Vec<MenuItem> {
    let tr = move |k| t(lang, k).to_string();
    let mut items = Vec::new();
    if let Some(d) = detail.filter(|d| d.users.is_some()) {
        let (pid, users) = (p.id.clone(), d.users.clone().unwrap_or_default());
        let before: Vec<AssignmentRow> = users
            .iter()
            .filter(|m| !m.role_ids.is_empty())
            .map(|m| AssignmentRow {
                actor_id: m.id.clone(),
                label: m.name.clone(),
                role_ids: m.role_ids.clone(),
            })
            .collect();
        items.push(MenuItem::dialog(tr(K::ManageMembers), move || {
            assignments_dialog(lang, K::ManageMembers, &pid, false, &before)
        }));
        let (pid, groups) = (p.id.clone(), d.groups.clone().unwrap_or_default());
        let before: Vec<AssignmentRow> = groups
            .iter()
            .map(|g| AssignmentRow {
                actor_id: g.id.clone(),
                label: g.name.clone(),
                role_ids: g.role_ids.clone(),
            })
            .collect();
        items.push(MenuItem::dialog(tr(K::ModifyGroups), move || {
            assignments_dialog(lang, K::ModifyGroups, &pid, true, &before)
        }));
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
            Ok(OsAction::ProjectUpdate {
                id: id.clone(),
                name: req(v, "name", K::Name, lang)?,
                description: v.get("description"),
                enabled: v.flag("enabled"),
            })
        })
    }));
    // After *Edit Project*, as Horizon. *Modify Quotas* (roadmap 82): any project's quotas, for the
    // `admin` role only (`needs::ADMIN`); the server checks the role again. The
    // session's own project is edited from Project › Quotas by whoever may.
    if admin {
        items.push(MenuItem::link(
            tr(K::ModifyQuotas),
            format!("/identity/projects/{}/quotas", p.id),
        ));
    }
    let (i, n) = (p.id.clone(), p.name.clone());
    items.push(MenuItem::danger(tr(K::DeleteProject), move || {
        Dialog::confirm(
            lang,
            tr(K::DeleteProject),
            n.clone(),
            OsAction::ProjectDelete { id: i.clone() },
        )
    }));
    items
}

/// *Manage Members* (users) / *Modify Groups* (groups): the project's direct
/// assignments edited at once, sent as the grants and revokes that changed.
fn assignments_dialog(lang: Lang, title: K, project_id: &str, group: bool, before: &[AssignmentRow]) -> Dialog {
    let tr = |k| t(lang, k).to_string();
    let actors = if group {
        ChoiceSet::IdentityGroups
    } else {
        ChoiceSet::IdentityUsers
    };
    let label = if group { K::Groups } else { K::Users };
    let (pid, before) = (project_id.to_string(), before.to_vec());
    let before2 = before.clone();
    Dialog::form(
        tr(title),
        tr(K::Save),
        vec![Field::assignments("rows", tr(label), &before, actors)],
        move |v| {
            let after: Vec<AssignmentRow> = serde_json::from_str(&v.get("rows")).unwrap_or_default();
            let changes = assignment_changes(&pid, group, &before2, &after);
            if changes.is_empty() {
                return Err(t(lang, K::NothingChanged).to_string());
            }
            Ok(OsAction::Batch(changes))
        },
    )
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
        Field::select(
            "project",
            tr(K::PrimaryProject),
            Source::Set(ChoiceSet::IdentityProjects),
        ),
        Field::select("role", tr(K::Role), Source::Set(ChoiceSet::IdentityRoles))
            .show_if(|v| v.get("project").is_some_and(|p| !p.is_empty())),
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
            Field::select(
                "project",
                tr(K::PrimaryProject),
                Source::Set(ChoiceSet::IdentityProjects),
            )
            .value(primary.clone().unwrap_or_default()),
        ];
        Dialog::form(tr(K::EditUser), tr(K::Save), fields, move |v| {
            Ok(OsAction::UserUpdate {
                id: id.clone(),
                name: req(v, "name", K::UserName, lang)?,
                email: v.get("email"),
                description: v.get("description"),
                project_id: v.opt("project"),
            })
        })
    }));
    let id = u.id.clone();
    items.push(MenuItem::dialog(tr(K::ChangePassword), move || {
        let id = id.clone();
        let fields = vec![
            Field::password("password", tr(K::Password)).required(),
            Field::password("confirm", tr(K::ConfirmPassword)).required(),
        ];
        Dialog::form(tr(K::ChangePassword), tr(K::Save), fields, move |v| {
            let password = v.get("password");
            if password != v.get("confirm") {
                return Err(t(lang, K::PasswordsDiffer).to_string());
            }
            Ok(OsAction::UserSetPassword {
                id: id.clone(),
                password,
            })
        })
    }));
    let (id, name, enabled) = (u.id.clone(), u.name.clone(), u.enabled);
    let label = tr(if enabled { K::DisableUser } else { K::EnableUser });
    items.push(MenuItem::dialog(label.clone(), move || {
        Dialog::simple(
            lang,
            label.clone(),
            name.clone(),
            OsAction::UserSetEnabled {
                id: id.clone(),
                enabled: !enabled,
            },
        )
    }));
    let (i, n) = (u.id.clone(), u.name.clone());
    items.push(MenuItem::danger(tr(K::DeleteUser), move || {
        Dialog::confirm(
            lang,
            tr(K::DeleteUser),
            n.clone(),
            OsAction::UserDelete { id: i.clone() },
        )
    }));
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
        Ok(OsAction::GroupCreate {
            name: req(v, "name", K::Name, lang)?,
            description: v.opt("description"),
            domain_id: String::new(),
        })
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
            let field =
                Field::pick_many("users", tr(K::Users), Source::Set(ChoiceSet::IdentityUsers)).value(before.join(","));
            Dialog::form(tr(K::ManageMembers), tr(K::Save), vec![field], move |v| {
                let after = v.list("users");
                let mut changes: Vec<(String, OsAction)> = after
                    .iter()
                    .filter(|u| !before.contains(u))
                    .map(|u| {
                        (
                            format!("+ {u}"),
                            OsAction::GroupAddUser {
                                group_id: gid.clone(),
                                user_id: u.clone(),
                            },
                        )
                    })
                    .collect();
                changes.extend(before.iter().filter(|u| !after.contains(u)).map(|u| {
                    (
                        format!("- {u}"),
                        OsAction::GroupRemoveUser {
                            group_id: gid.clone(),
                            user_id: u.clone(),
                        },
                    )
                }));
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
                Ok(OsAction::GroupUpdate {
                    id: id.clone(),
                    name: req(v, "name", K::Name, lang)?,
                    description: v.get("description"),
                })
            })
        }),
        MenuItem::danger(tr(K::DeleteGroup), move || {
            Dialog::confirm(
                lang,
                tr(K::DeleteGroup),
                n.clone(),
                OsAction::GroupDelete { id: i.clone() },
            )
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::dialog::Kind;

    #[test]
    fn expiration_date_and_time_share_a_line() {
        // Roadmap 111: the two halves of one value, side by side (Store Secret too).
        for d in [
            create_app_credential_dialog(Lang::EnUs),
            super::super::store_secret_dialog(Lang::EnUs),
        ] {
            let f = |n| d.fields.iter().find(|f| f.name == n).unwrap();
            assert!(f("expiration_date").half && f("expiration_time").half, "{}", d.title);
        }
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
        let labels = |admin| {
            identity_project_items(Lang::EnUs, &p, None, admin)
                .into_iter()
                .map(|m| m.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(true),
            ["Edit Project", "Modify Quotas", "Delete Project"],
            "Horizon's order (roadmap 82)"
        );
        assert!(
            !labels(false).iter().any(|l| l == "Modify Quotas"),
            "hidden without the admin role"
        );
        let items = identity_project_items(Lang::EnUs, &p, None, true);
        assert!(matches!(&items[1].kind, ItemKind::Link(h) if h == "/identity/projects/p9/quotas"));
    }

    #[test]
    fn identity_create_dialogs_show_where_things_go() {
        for d in [
            create_project_dialog(Lang::EnUs),
            create_user_dialog(Lang::EnUs),
            create_group_dialog(Lang::EnUs),
        ] {
            assert!(
                matches!(d.fields[0].kind, Kind::Info(ChoiceSet::OperationDomain)),
                "{}: the domain comes first, read-only",
                d.title
            );
        }
    }
}
