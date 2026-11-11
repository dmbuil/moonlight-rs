//! Keystone: projects, users, groups, roles, domains, application credentials.

use super::*;

/// Keystone `auth/projects` → the projects the user can switch to, sorted by name.
pub fn projects(raw: &[Value]) -> Vec<ProjectRef> {
    let mut out: Vec<_> = raw
        .iter()
        .filter(|p| p.get("enabled").and_then(Value::as_bool).unwrap_or(true))
        .filter_map(|p| {
            Some(ProjectRef {
                id: s(p, "id")?,
                name: name_or_id(p),
            })
        })
        .collect();
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

/// Keystone's `enabled` is optional and means true when absent.
pub(super) fn enabled(v: &Value) -> bool {
    v.get("enabled").and_then(Value::as_bool).unwrap_or(true)
}

/// `domain_id` → name when `domains` has it, else the ID.
fn domain_of(v: &Value, domains: &HashMap<String, String>) -> String {
    let id = s(v, "domain_id").unwrap_or_default();
    domains.get(&id).cloned().unwrap_or(id)
}

/// Identity › Projects, by name. `mine`: the user's projects (they can switch to them).
pub fn identity_projects(
    raw: &[Value],
    domains: &HashMap<String, String>,
    current: &str,
    mine: &[String],
) -> Vec<IdentityProject> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|p| {
            let id = s(p, "id")?;
            Some(IdentityProject {
                name: name_or_id(p),
                description: s_ne(p, "description"),
                domain: domain_of(p, domains),
                enabled: enabled(p),
                current: id == current,
                member: mine.contains(&id),
                id,
            })
        })
        .collect();
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

/// Identity › Users, by name.
pub fn identity_users(raw: &[Value], domains: &HashMap<String, String>) -> Vec<IdentityUser> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|u| unwrap(u, "user"))
        .filter_map(|u| {
            Some(IdentityUser {
                id: s(u, "id")?,
                name: name_or_id(u),
                description: s_ne(u, "description"),
                email: s_ne(u, "email"),
                enabled: enabled(u),
                domain: domain_of(u, domains),
            })
        })
        .collect();
    out.sort_by_key(|u| u.name.to_lowercase());
    out
}

/// A project's members from its role assignments (`include_names`): users and
/// groups with their roles, and each group's users (`group_users`, by group ID)
/// with the roles they get through it, like Horizon's *Users* tab.
pub fn project_members(
    assignments: &[Value],
    group_users: &[(String, Vec<Value>)],
) -> (Vec<ProjectMember>, Vec<ProjectGroup>) {
    let mut users: Vec<ProjectMember> = Vec::new();
    let mut groups: Vec<ProjectGroup> = Vec::new();
    let role_of = |a: &Value| {
        a.get("role")
            .map(|r| s_ne(r, "name").or_else(|| s(r, "id")).unwrap_or_default())
            .unwrap_or_default()
    };
    let role_id = |a: &Value| a.get("role").and_then(|r| s(r, "id")).unwrap_or_default();
    for a in assignments {
        let (role, rid) = (role_of(a), role_id(a));
        if let Some(u) = a.get("user") {
            let id = s(u, "id").unwrap_or_default();
            match users.iter_mut().find(|m| m.id == id) {
                Some(m) => {
                    m.roles.push(role);
                    m.role_ids.push(rid);
                }
                None => users.push(ProjectMember {
                    name: name_or_id(u),
                    id,
                    roles: vec![role],
                    role_ids: vec![rid],
                    group_roles: vec![],
                }),
            }
        } else if let Some(g) = a.get("group") {
            let id = s(g, "id").unwrap_or_default();
            match groups.iter_mut().find(|x| x.id == id) {
                Some(x) => {
                    x.roles.push(role);
                    x.role_ids.push(rid);
                }
                None => groups.push(ProjectGroup {
                    name: name_or_id(g),
                    id,
                    roles: vec![role],
                    role_ids: vec![rid],
                }),
            }
        }
    }
    for (group_id, members) in group_users {
        let Some(g) = groups.iter().find(|g| &g.id == group_id).cloned() else {
            continue;
        };
        for u in members {
            let id = s(u, "id").unwrap_or_default();
            let from_group = g.roles.iter().map(|r| (r.clone(), g.name.clone()));
            match users.iter_mut().find(|m| m.id == id) {
                Some(m) => m.group_roles.extend(from_group),
                None => users.push(ProjectMember {
                    name: name_or_id(u),
                    id,
                    roles: vec![],
                    role_ids: vec![],
                    group_roles: from_group.collect(),
                }),
            }
        }
    }
    for m in &mut users {
        m.roles.sort();
        m.roles.dedup();
        m.role_ids.sort();
        m.role_ids.dedup();
        m.group_roles.sort();
        m.group_roles.dedup();
    }
    for g in &mut groups {
        g.roles.sort();
        g.roles.dedup();
        g.role_ids.sort();
        g.role_ids.dedup();
    }
    users.sort_by_key(|m| m.name.to_lowercase());
    groups.sort_by_key(|g| g.name.to_lowercase());
    (users, groups)
}

/// A user's detail; `projects` names the primary project when known.
pub fn identity_user_detail(
    u: &Value,
    domains: &HashMap<String, String>,
    projects: &HashMap<String, String>,
) -> IdentityUserDetail {
    let u = unwrap(u, "user");
    // Keystone's own attributes; anything else is shown as extra, like Horizon.
    const KNOWN: [&str; 11] = [
        "id",
        "name",
        "description",
        "email",
        "enabled",
        "domain_id",
        "default_project_id",
        "password_expires_at",
        "options",
        "links",
        "federated",
    ];
    let mut extra: Vec<(String, String)> = u
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, v)| !KNOWN.contains(&k.as_str()) && !v.is_null())
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    extra.sort();
    IdentityUserDetail {
        user: identity_users(std::slice::from_ref(u), domains)
            .pop()
            .unwrap_or(IdentityUser {
                id: String::new(),
                name: String::new(),
                description: None,
                email: None,
                enabled: true,
                domain: String::new(),
            }),
        domain_id: s_ne(u, "domain_id"),
        password_expires_at: s_ne(u, "password_expires_at"),
        lock_password: u.get("options").map(|o| b(o, "lock_password")).unwrap_or(false),
        primary_project: s_ne(u, "default_project_id").map(|id| named(id, projects)),
        primary_project_id: s_ne(u, "default_project_id"),
        extra,
        assignments: None,
        groups: None,
    }
}

/// Identity › Groups, by name.
pub fn identity_groups(raw: &[Value], domains: &HashMap<String, String>) -> Vec<IdentityGroup> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|g| {
            Some(IdentityGroup {
                id: s(g, "id")?,
                name: name_or_id(g),
                description: s_ne(g, "description"),
                domain: domain_of(g, domains),
            })
        })
        .collect();
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

/// Identity › Roles, by name, with the roles each implies (`inferences`:
/// Keystone's `role_inferences`, `{prior_role, implies: [...]}`).
pub fn identity_roles(raw: &[Value], inferences: &[Value]) -> Vec<IdentityRole> {
    let implied = |id: &str| -> Vec<String> {
        let mut v: Vec<String> = inferences
            .iter()
            .filter(|i| i.get("prior_role").and_then(|p| s(p, "id")).as_deref() == Some(id))
            .flat_map(|i| arr(i, "implies").iter().map(name_or_id).collect::<Vec<_>>())
            .collect();
        v.sort();
        v
    };
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|r| {
            let id = s(r, "id")?;
            Some(IdentityRole {
                implies: implied(&id),
                name: name_or_id(r),
                id,
            })
        })
        .collect();
    out.sort_by_key(|r| r.name.to_lowercase());
    out
}

/// A user's role assignments from `role_assignments?user.id&include_names`.
pub fn role_assignment_rows(raw: &[Value]) -> Vec<RoleAssignmentRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|a| {
            let role = a
                .get("role")
                .map(|r| s_ne(r, "name").or_else(|| s(r, "id")).unwrap_or_default())
                .unwrap_or_default();
            let scope = a.get("scope");
            let inherited = scope.is_some_and(|sc| sc.get("OS-INHERIT:inherited_to").is_some());
            let (scope_kind, name, project_id) = match scope {
                Some(sc) if sc.get("project").is_some() => {
                    ("project", name_or_id(&sc["project"]), s(&sc["project"], "id"))
                }
                Some(sc) if sc.get("domain").is_some() => ("domain", name_or_id(&sc["domain"]), None),
                Some(sc) if sc.get("system").is_some() => ("system", "all".to_string(), None),
                _ => ("", String::new(), None),
            };
            RoleAssignmentRow {
                role,
                scope_kind: scope_kind.into(),
                scope: name,
                project_id,
                inherited,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        (a.scope_kind.as_str(), a.scope.to_lowercase(), a.role.as_str()).cmp(&(
            b.scope_kind.as_str(),
            b.scope.to_lowercase(),
            b.role.as_str(),
        ))
    });
    out
}

/// A group's overview and its members.
pub fn identity_group_detail(g: &Value, users: &[Value], domains: &HashMap<String, String>) -> IdentityGroupDetail {
    let g = unwrap(g, "group");
    IdentityGroupDetail {
        group: identity_groups(std::slice::from_ref(g), domains)
            .pop()
            .unwrap_or(IdentityGroup {
                id: String::new(),
                name: String::new(),
                description: None,
                domain: String::new(),
            }),
        domain_id: s_ne(g, "domain_id"),
        users: identity_users(users, domains),
    }
}

/// One application credential, with its access rules.
pub fn app_credential_detail(v: &Value) -> AppCredentialDetail {
    let v = unwrap(v, "application_credential");
    AppCredentialDetail {
        credential: app_credentials(std::slice::from_ref(v)).pop().unwrap_or(AppCredential {
            id: String::new(),
            name: String::new(),
            project_id: None,
            description: None,
            expires_at: None,
            roles: vec![],
            unrestricted: false,
        }),
        access_rules: arr(v, "access_rules")
            .iter()
            .map(|r| AccessRule {
                service: s(r, "service").unwrap_or_default(),
                method: s(r, "method").unwrap_or_default(),
                path: s(r, "path").unwrap_or_default(),
            })
            .collect(),
    }
}

/// Identity › Application Credentials, by name; role names (else IDs).
pub fn app_credentials(raw: &[Value]) -> Vec<AppCredential> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|a| {
            Some(AppCredential {
                id: s(a, "id")?,
                name: name_or_id(a),
                project_id: s_ne(a, "project_id"),
                description: s_ne(a, "description"),
                expires_at: s_ne(a, "expires_at"),
                roles: arr(a, "roles")
                    .iter()
                    .filter_map(|r| s_ne(r, "name").or_else(|| s(r, "id")))
                    .collect(),
                unrestricted: a.get("unrestricted").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Identity › Domains, by name; `context` marks the session's domain context.
pub fn identity_domains(raw: &[Value], context: Option<&str>) -> Vec<IdentityDomain> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|d| {
            let id = s(d, "id")?;
            Some(IdentityDomain {
                name: name_or_id(d),
                description: s_ne(d, "description"),
                enabled: enabled(d),
                context: context == Some(id.as_str()),
                id,
            })
        })
        .collect();
    out.sort_by_key(|d| d.name.to_lowercase());
    out
}

/// Domain ID → name, from a domain list (admins) plus the user's own domain.
pub fn domain_names(raw: &[Value], own: Option<(String, String)>) -> HashMap<String, String> {
    let mut m = names_by_id(raw);
    if let Some((id, name)) = own {
        m.entry(id).or_insert(name);
    }
    m
}

/// The group (`(id, name)`) whose members include `server_id`, if any.
pub fn group_of(groups: &[Value], server_id: &str) -> Option<(String, String)> {
    groups
        .iter()
        .find(|g| strs(g, "members").iter().any(|m| m == server_id))
        .map(|g| (s(g, "id").unwrap_or_default(), name_or_id(g)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn domains_mark_the_context() {
        let d = identity_domains(
            &[
                json!({"id": "d2", "name": "Zeta", "enabled": false}),
                json!({"id": "d1", "name": "ttcp-spain"}),
            ],
            Some("d2"),
        );
        assert_eq!(
            (d[0].name.as_str(), d[0].enabled, d[0].context),
            ("ttcp-spain", true, false)
        );
        assert_eq!((d[1].name.as_str(), d[1].enabled, d[1].context), ("Zeta", false, true));
    }

    #[test]
    fn role_assignments_name_their_scope() {
        let rows = role_assignment_rows(&[
            json!({"role": {"id": "r1", "name": "member"}, "user": {"id": "u"}, "scope": {"project": {"id": "p1", "name": "pokemon"}}}),
            json!({"role": {"id": "r2", "name": "admin"}, "user": {"id": "u"}, "scope": {"domain": {"id": "d1", "name": "ttcp-spain"}, "OS-INHERIT:inherited_to": "projects"}}),
            json!({"role": {"id": "r3", "name": "reader"}, "user": {"id": "u"}, "scope": {"system": {"all": true}}}),
        ]);
        assert_eq!(
            (rows[0].scope_kind.as_str(), rows[0].scope.as_str(), rows[0].inherited),
            ("domain", "ttcp-spain", true)
        );
        assert_eq!(
            (rows[1].scope.as_str(), rows[1].project_id.as_deref()),
            ("pokemon", Some("p1"))
        );
        assert_eq!((rows[2].scope_kind.as_str(), rows[2].scope.as_str()), ("system", "all"));
        let a = app_credential_detail(
            &json!({"application_credential": {"id": "a", "name": "ci", "roles": [], "unrestricted": true,
            "access_rules": [{"id": "x", "service": "compute", "method": "GET", "path": "/v2.1/servers"}]}}),
        );
        assert_eq!(
            (a.credential.unrestricted, a.access_rules[0].path.as_str()),
            (true, "/v2.1/servers")
        );
        let g = identity_group_detail(
            &json!({"group": {"id": "g", "name": "ops", "domain_id": "d1"}}),
            &[json!({"id": "u1", "name": "ana"})],
            &HashMap::new(),
        );
        assert_eq!(
            (g.group.name.as_str(), g.domain_id.as_deref(), g.users[0].name.as_str()),
            ("ops", Some("d1"), "ana")
        );
    }

    #[test]
    fn project_members_join_direct_and_group_roles() {
        let assignments = [
            json!({"role": {"id": "r1", "name": "member"}, "user": {"id": "u1", "name": "ana"}, "scope": {"project": {"id": "p"}}}),
            json!({"role": {"id": "r2", "name": "reader"}, "user": {"id": "u1", "name": "ana"}, "scope": {"project": {"id": "p"}}}),
            json!({"role": {"id": "r3", "name": "admin"}, "group": {"id": "g1", "name": "ops"}, "scope": {"project": {"id": "p"}}}),
            json!({"role": {"id": "r4"}, "group": {"id": "g1", "name": "ops"}, "scope": {"project": {"id": "p"}}}),
        ];
        let (users, groups) = project_members(
            &assignments,
            &[(
                "g1".into(),
                vec![json!({"id": "u1", "name": "ana"}), json!({"id": "u2", "name": "bo"})],
            )],
        );
        assert_eq!(
            groups,
            [ProjectGroup {
                id: "g1".into(),
                name: "ops".into(),
                roles: vec!["admin".into(), "r4".into()],
                role_ids: vec!["r3".into(), "r4".into()]
            }],
            "a nameless role keeps its ID"
        );
        assert_eq!(users[0].role_ids, ["r1", "r2"], "IDs kept for Manage Members");
        assert_eq!(
            (users[0].name.as_str(), users[0].roles.clone()),
            ("ana", vec!["member".to_string(), "reader".into()])
        );
        assert_eq!(
            users[0].group_roles,
            [("admin".to_string(), "ops".to_string()), ("r4".into(), "ops".into())],
            "also through the group"
        );
        assert_eq!(
            (users[1].name.as_str(), users[1].roles.len(), users[1].group_roles.len()),
            ("bo", 0, 2),
            "only through the group"
        );
    }

    #[test]
    fn user_detail_reads_options_primary_project_and_extras() {
        let domains = domain_names(&[], Some(("d1".into(), "ttcp-spain".into())));
        let projects = HashMap::from([("p1".to_string(), "admin2".to_string())]);
        let d = identity_user_detail(
            &json!({"user": {"id": "u1", "name": "admin2", "domain_id": "d1", "enabled": true, "default_project_id": "p1",
                "password_expires_at": null, "options": {"lock_password": true}, "links": {"self": "x"}, "department": "ops", "phone": null}}),
            &domains,
            &projects,
        );
        assert_eq!(
            (d.user.name.as_str(), d.user.domain.as_str(), d.domain_id.as_deref()),
            ("admin2", "ttcp-spain", Some("d1"))
        );
        assert_eq!(
            (d.password_expires_at.clone(), d.lock_password),
            (None, true),
            "null = never expires"
        );
        assert_eq!(d.primary_project.map(|p| p.name), Some("admin2".into()));
        assert_eq!(
            d.extra,
            [("department".to_string(), "ops".to_string())],
            "unknown attributes shown; nulls and links not"
        );
        let other = identity_user_detail(
            &json!({"id": "u2", "name": "x", "default_project_id": "p9"}),
            &domains,
            &projects,
        );
        assert_eq!(
            (other.primary_project.map(|p| p.name), other.lock_password),
            (Some("p9".into()), false),
            "an unknown project keeps its ID"
        );
    }

    #[test]
    fn identity_lists_follow_keystone_quirks() {
        let domains = domain_names(&[], Some(("d1".into(), "ttcp-spain".into())));
        let p = identity_projects(
            &[
                json!({"id": "p2", "name": "pokemon", "description": "", "domain_id": "d1", "enabled": true}),
                json!({"id": "p1", "name": "admin2", "description": "admin Project", "domain_id": "d9"}),
                json!({"id": "p3", "name": "Zeta", "domain_id": "d1", "enabled": false}),
            ],
            &domains,
            "p2",
            &["p1".into(), "p2".into()],
        );
        assert_eq!(
            p.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["admin2", "pokemon", "Zeta"]
        );
        assert_eq!(
            (p[0].enabled, p[0].domain.as_str(), p[0].member, p[0].current),
            (true, "d9", true, false),
            "`enabled` absent = true; unknown domain keeps its ID"
        );
        assert_eq!(
            (p[1].description.clone(), p[1].domain.as_str(), p[1].current),
            (None, "ttcp-spain", true),
            "empty description = none"
        );
        assert!(
            !p[2].enabled && !p[2].member,
            "not the user's: no *Set as Active Project*"
        );

        let u = identity_users(
            &[
                json!({"user": {"id": "u1", "name": "zc01001", "domain_id": "d1", "password_expires_at": null}}),
                json!({"id": "u2", "name": "", "email": "a@example.com", "enabled": false}),
            ],
            &domains,
        );
        assert_eq!(
            (u[0].name.as_str(), u[0].email.clone()),
            ("u2", Some("a@example.com".into())),
            "a nameless user falls back to its ID"
        );
        assert_eq!(
            (
                u[1].name.as_str(),
                u[1].email.clone(),
                u[1].domain.as_str(),
                u[1].enabled
            ),
            ("zc01001", None, "ttcp-spain", true),
            "`user::get` comes wrapped"
        );

        let a = app_credentials(&[
            json!({"id": "a1", "name": "poke", "project_id": "p2", "expires_at": null,
            "roles": [{"id": "r1", "name": "power_user"}, {"id": "r2"}], "unrestricted": false}),
        ]);
        assert_eq!(
            (a[0].expires_at.clone(), a[0].roles.clone()),
            (None, vec!["power_user".to_string(), "r2".into()]),
            "never expires; a role without a name keeps its ID"
        );
        let roles = identity_roles(
            &[
                json!({"id": "r", "name": "reader"}),
                json!({"id": "a", "name": "admin"}),
                json!({"id": "m", "name": "member"}),
            ],
            &[
                json!({"prior_role": {"id": "a", "name": "admin"}, "implies": [{"id": "m", "name": "member"}]}),
                json!({"prior_role": {"id": "m", "name": "member"}, "implies": [{"id": "r", "name": "reader"}]}),
            ],
        );
        assert_eq!(
            (roles[0].name.as_str(), roles[0].implies.clone()),
            ("admin", vec!["member".to_string()]),
            "Keystone's default chain"
        );
        assert!(roles[2].implies.is_empty(), "reader implies nothing");
        assert_eq!(
            identity_groups(&[json!({"id": "g", "name": "ops", "domain_id": "d1"})], &domains)[0].domain,
            "ttcp-spain"
        );
    }

    #[test]
    fn projects_skip_disabled_and_sort() {
        let p = projects(&[
            json!({"id": "2", "name": "beta", "enabled": true}),
            json!({"id": "3", "name": "gone", "enabled": false}),
            json!({"id": "1", "name": "Alpha"}),
        ]);
        assert_eq!(
            p.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
            vec!["Alpha", "beta"]
        );
    }
}
