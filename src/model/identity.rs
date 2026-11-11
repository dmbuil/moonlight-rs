//! Keystone: projects, users, groups, roles, domains, application credentials.

use super::*;

/// A row of Identity › Projects (roadmap 57): every project of the domain for
/// admins, the user's own otherwise (like Horizon).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityProject {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Domain name when known, else its ID.
    pub domain: String,
    pub enabled: bool,
    /// The session's project (*Set as Active Project* is not offered on it).
    pub current: bool,
    /// The user has a role on it, so it can be made the active project.
    pub member: bool,
}

/// A row of Identity › Users: every user of the domain for admins, only the
/// user themself otherwise (Horizon's `get_user` fallback).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityUser {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub email: Option<String>,
    pub enabled: bool,
    pub domain: String,
}

/// Identity › Projects › a project: Horizon's *Overview*, and for admins its
/// *Users* and *Groups* tabs (roadmap 57).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityProjectDetail {
    pub project: IdentityProject,
    pub domain_id: Option<String>,
    /// `None` for non-admins (Keystone won't list the assignments): no tab.
    pub users: Option<Vec<ProjectMember>>,
    pub groups: Option<Vec<ProjectGroup>>,
}

/// A user with roles on a project, directly or through a group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectMember {
    pub id: String,
    pub name: String,
    /// Roles given to the user on the project.
    pub roles: Vec<String>,
    /// Their IDs, for *Manage Members*.
    #[serde(default)]
    pub role_ids: Vec<String>,
    /// `(role, group)` for roles that come from a group's assignment.
    pub group_roles: Vec<(String, String)>,
}

/// A group with roles on a project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectGroup {
    pub id: String,
    pub name: String,
    pub roles: Vec<String>,
    /// Their IDs, for *Modify Groups*.
    #[serde(default)]
    pub role_ids: Vec<String>,
}

/// Identity › Users › a user: Horizon's *Overview* (roadmap 57).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityUserDetail {
    pub user: IdentityUser,
    pub domain_id: Option<String>,
    /// `None`: the password doesn't expire.
    pub password_expires_at: Option<String>,
    /// Keystone's `options.lock_password`: the user can't change their password.
    pub lock_password: bool,
    /// `default_project_id`, named when the project is known.
    pub primary_project: Option<Ref>,
    /// Other attributes Keystone returns (Horizon lists them too), by key.
    pub extra: Vec<(String, String)>,
    /// Admins only (`None` = no tab): the user's role assignments and groups.
    #[serde(default)]
    pub assignments: Option<Vec<RoleAssignmentRow>>,
    #[serde(default)]
    pub groups: Option<Vec<Ref>>,
    /// Keystone's `default_project_id` as an ID (the edit dialog's preset).
    #[serde(default)]
    pub primary_project_id: Option<String>,
}

/// One of a user's role assignments (Horizon's *Role assignments* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoleAssignmentRow {
    pub role: String,
    /// `project`, `domain` or `system`.
    pub scope_kind: String,
    /// The project or domain name (else ID); `all` for the system.
    pub scope: String,
    /// The project's ID, to link it.
    pub project_id: Option<String>,
    /// Through `OS-INHERIT` (inherited to the scope's projects).
    pub inherited: bool,
}

/// Identity › Groups › a group: its overview and members (roadmap 57).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityGroupDetail {
    pub group: IdentityGroup,
    pub domain_id: Option<String>,
    pub users: Vec<IdentityUser>,
}

/// Identity › Application Credentials › one: Horizon's detail page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppCredentialDetail {
    pub credential: AppCredential,
    pub access_rules: Vec<AccessRule>,
}

/// A row of Identity › Domains (multi-domain clouds).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityDomain {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    /// The session's domain context (*Set Domain Context*).
    pub context: bool,
}

/// A row of Identity › Groups (admins).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityGroup {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub domain: String,
}

/// A row of Identity › Roles (admins).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IdentityRole {
    pub id: String,
    pub name: String,
    /// Roles this one implies (`role_inferences`), by name.
    #[serde(default)]
    pub implies: Vec<String>,
}

/// A row of Identity › Application Credentials: the user's own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppCredential {
    pub id: String,
    pub name: String,
    pub project_id: Option<String>,
    pub description: Option<String>,
    /// `None` = never expires.
    pub expires_at: Option<String>,
    pub roles: Vec<String>,
    pub unrestricted: bool,
}

/// *Create Application Credential* (roadmap 57), as Horizon's form. `user_id`
/// is set by `run_action` from the session: users create their own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppCredentialSpec {
    #[serde(default)]
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    /// `None`: Keystone generates one.
    pub secret: Option<String>,
    /// ISO 8601 in UTC (`2026-12-31T00:00:00`); `None` = never expires.
    pub expires_at: Option<String>,
    /// Role names, a subset of the user's on the project; empty = all of them.
    pub roles: Vec<String>,
    /// Lets the credential create other credentials and trusts (Horizon warns: dangerous).
    pub unrestricted: bool,
    /// Access rules as typed (a JSON or YAML list of `{service, method, path}`),
    /// parsed and checked on the server; `None` = no restriction.
    #[serde(default)]
    pub access_rules: Option<String>,
}

/// Horizon's *Create User*: a password typed by the admin, and optionally a
/// primary project with a role on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserSpec {
    /// Set by `run_action` (see [`OsAction::set_domain`]).
    #[serde(default)]
    pub domain_id: String,
    pub name: String,
    pub description: Option<String>,
    pub email: Option<String>,
    pub password: String,
    pub project_id: Option<String>,
    /// Granted on `project_id` once the user exists.
    pub role_id: Option<String>,
    pub enabled: bool,
    pub lock_password: bool,
}

/// One row of *Manage Members* / *Modify Groups*: a user or group and its role
/// IDs on the project (the field's value is a JSON list of these).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssignmentRow {
    pub actor_id: String,
    pub label: String,
    pub role_ids: Vec<String>,
}

/// The grants and revokes that turn `before` into `after` on `project_id`, as
/// `(label, action)` for a batch; `group` says whether actors are groups.
pub fn assignment_changes(
    project_id: &str,
    group: bool,
    before: &[AssignmentRow],
    after: &[AssignmentRow],
) -> Vec<(String, OsAction)> {
    let roles_of = |rows: &[AssignmentRow], id: &str| {
        rows.iter()
            .find(|r| r.actor_id == id)
            .map(|r| r.role_ids.clone())
            .unwrap_or_default()
    };
    let actor = |id: &str| {
        if group {
            (None, Some(id.to_string()))
        } else {
            (Some(id.to_string()), None)
        }
    };
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for row in after.iter().chain(before) {
        if !seen.insert(row.actor_id.as_str()) {
            continue;
        }
        let (old, new) = (roles_of(before, &row.actor_id), roles_of(after, &row.actor_id));
        for r in new.iter().filter(|r| !old.contains(r)) {
            let (user_id, group_id) = actor(&row.actor_id);
            out.push((
                format!("+ {} {r}", row.label),
                OsAction::ProjectRoleGrant {
                    project_id: project_id.into(),
                    user_id,
                    group_id,
                    role_id: r.clone(),
                },
            ));
        }
        for r in old.iter().filter(|r| !new.contains(r)) {
            let (user_id, group_id) = actor(&row.actor_id);
            out.push((
                format!("- {} {r}", row.label),
                OsAction::ProjectRoleRevoke {
                    project_id: project_id.into(),
                    user_id,
                    group_id,
                    role_id: r.clone(),
                },
            ));
        }
    }
    out
}

/// One application credential access rule (Keystone API 3.13).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccessRule {
    pub service: String,
    pub method: String,
    pub path: String,
}

impl AccessRule {
    /// Keystone's schema: a service type, a known HTTP method, a path from `/`.
    pub fn check(&self) -> Result<(), String> {
        if self.service.is_empty() || !self.service.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(format!("access rule: not a service type: {:?}", self.service));
        }
        one_of(
            &self.method,
            &["DELETE", "GET", "HEAD", "PATCH", "POST", "PUT"],
            "access rule method",
        )?;
        if !self.path.starts_with('/') || self.path.len() > 225 {
            return Err(format!("access rule: the path must start with / (got {:?})", self.path));
        }
        Ok(())
    }
}

/// Horizon's expiration rule: a date alone means midnight, a time alone means
/// today, both combine; neither = never. All in UTC, like `now` (the current UTC
/// time). The result must be later than `now`: "today" alone, or a time that
/// already passed, would be in the past. Returns `YYYY-MM-DDTHH:MM:SS`.
pub fn app_credential_expiry(date: &str, time: &str, now: chrono::NaiveDateTime) -> Result<Option<String>, String> {
    let today = now.date();
    let (date, time) = (date.trim(), time.trim());
    if date.is_empty() && time.is_empty() {
        return Ok(None);
    }
    let d = if date.is_empty() {
        today
    } else {
        chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| format!("not a date (YYYY-MM-DD): {date}"))?
    };
    let t = if time.is_empty() {
        chrono::NaiveTime::MIN
    } else {
        chrono::NaiveTime::parse_from_str(time, "%H:%M")
            .or_else(|_| chrono::NaiveTime::parse_from_str(time, "%H:%M:%S"))
            .map_err(|_| format!("not a time (HH:MM): {time}"))?
    };
    let at = d.and_time(t);
    if at <= now {
        return Err(format!(
            "the expiration ({} UTC) must be later than now ({} UTC)",
            at.format("%Y-%m-%d %H:%M"),
            now.format("%Y-%m-%d %H:%M")
        ));
    }
    Ok(Some(at.format("%Y-%m-%dT%H:%M:%S").to_string()))
}

#[cfg(test)]
mod app_credential_tests {
    use super::*;

    #[test]
    fn expiry_follows_horizon() {
        // 20:20 UTC, the moment of the browser test (21:20 in Madrid).
        let now = chrono::NaiveDate::from_ymd_opt(2026, 10, 26)
            .unwrap()
            .and_hms_opt(20, 20, 0)
            .unwrap();
        assert_eq!(app_credential_expiry("", "", now), Ok(None), "never expires");
        assert_eq!(
            app_credential_expiry("2026-12-31", "", now).unwrap().as_deref(),
            Some("2026-12-31T00:00:00"),
            "a date alone: midnight"
        );
        assert_eq!(
            app_credential_expiry(" 2026-10-27 ", "03:15", now).unwrap().as_deref(),
            Some("2026-10-27T03:15:00"),
            "the credential created in the test"
        );
        assert_eq!(
            app_credential_expiry("", "23:30:15", now).unwrap().as_deref(),
            Some("2026-10-26T23:30:15"),
            "a time alone: today (UTC)"
        );
        assert!(
            app_credential_expiry("31/12/2026", "", now)
                .unwrap_err()
                .contains("YYYY-MM-DD")
        );
        assert!(
            app_credential_expiry("2026-12-31", "6pm", now)
                .unwrap_err()
                .contains("HH:MM")
        );
        // Horizon's pitfalls, refused before sending:
        let today_alone = app_credential_expiry("2026-10-26", "", now).unwrap_err();
        assert!(
            today_alone.contains("2026-10-26 00:00 UTC") && today_alone.contains("now (2026-10-26 20:20 UTC)"),
            "{today_alone}"
        );
        assert!(
            app_credential_expiry("", "19:00", now).is_err(),
            "a time alone that already passed today (UTC)"
        );
        assert!(
            app_credential_expiry("", "21:00", now).is_ok(),
            "21:00 UTC is later, though it is 21:20 in Madrid: the form is in UTC"
        );
    }

    #[test]
    fn secret_expiry_is_flagged_30_days_ahead() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 10, 27)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        assert_eq!(expiry(None, now), Expiry::Never);
        assert_eq!(
            expiry(Some("2026-10-27T11:59:00"), now),
            Expiry::Past,
            "Barbican's zoneless UTC"
        );
        assert_eq!(expiry(Some("2026-11-20T00:00:00+00:00"), now), Expiry::Soon);
        assert_eq!(expiry(Some("2027-01-01T00:00:00Z"), now), Expiry::Later);
        assert_eq!(expiry(Some("soon"), now), Expiry::Later, "unreadable: nothing to flag");
    }

    #[test]
    fn identity_actions_are_pointed_at_the_signed_in_user() {
        let spec = AppCredentialSpec {
            user_id: "someone-else".into(),
            name: "ci".into(),
            description: None,
            secret: None,
            expires_at: None,
            roles: vec![],
            unrestricted: false,
            access_rules: None,
        };
        let mut a = OsAction::Batch(vec![
            (
                "a".into(),
                OsAction::AppCredentialDelete {
                    user_id: "someone-else".into(),
                    id: "x".into(),
                },
            ),
            ("b".into(), OsAction::AppCredentialCreate(Box::new(spec))),
        ]);
        a.set_user("me");
        let OsAction::Batch(items) = a else { unreachable!() };
        assert!(matches!(&items[0].1, OsAction::AppCredentialDelete { user_id, .. } if user_id == "me"));
        assert!(
            matches!(&items[1].1, OsAction::AppCredentialCreate(s) if s.user_id == "me"),
            "whatever the browser sent"
        );
    }
}

#[cfg(test)]
mod identity_admin_tests {
    use super::*;

    #[test]
    fn nobody_locks_themselves_out() {
        let (me, here) = ("u-me", "p-here");
        let no = |a: OsAction| a.identity_safety(me, here).is_err();
        assert!(no(OsAction::UserDelete { id: me.into() }));
        assert!(no(OsAction::UserSetEnabled {
            id: me.into(),
            enabled: false
        }));
        assert!(
            !no(OsAction::UserSetEnabled {
                id: me.into(),
                enabled: true
            }),
            "enabling is fine"
        );
        assert!(!no(OsAction::UserDelete { id: "u-other".into() }));
        assert!(no(OsAction::ProjectDelete { id: here.into() }));
        assert!(no(OsAction::ProjectUpdate {
            id: here.into(),
            name: "x".into(),
            description: String::new(),
            enabled: false
        }));
        assert!(
            !no(OsAction::ProjectUpdate {
                id: here.into(),
                name: "renamed".into(),
                description: String::new(),
                enabled: true
            }),
            "renaming is fine"
        );
        let revoke_me = OsAction::ProjectRoleRevoke {
            project_id: here.into(),
            user_id: Some(me.into()),
            group_id: None,
            role_id: "r".into(),
        };
        assert!(no(revoke_me.clone()));
        assert!(
            !no(OsAction::ProjectRoleRevoke {
                project_id: "p-elsewhere".into(),
                user_id: Some(me.into()),
                group_id: None,
                role_id: "r".into()
            }),
            "another project"
        );
        assert!(
            no(OsAction::Batch(vec![
                ("ok".into(), OsAction::UserDelete { id: "u-other".into() }),
                ("bad".into(), revoke_me)
            ])),
            "inside a batch"
        );
    }

    #[test]
    fn creations_are_put_in_the_users_domain() {
        let spec = UserSpec {
            domain_id: "Default".into(),
            name: "ana".into(),
            description: None,
            email: None,
            password: "p".into(),
            project_id: None,
            role_id: None,
            enabled: true,
            lock_password: false,
        };
        let mut a = OsAction::Batch(vec![
            (
                "p".into(),
                OsAction::ProjectCreate {
                    name: "MyTest".into(),
                    description: None,
                    enabled: true,
                    domain_id: String::new(),
                },
            ),
            ("u".into(), OsAction::UserCreate(Box::new(spec))),
            (
                "g".into(),
                OsAction::GroupCreate {
                    name: "ops".into(),
                    description: None,
                    domain_id: String::new(),
                },
            ),
        ]);
        assert!(a.needs_domain());
        a.set_domain("d-spain");
        let OsAction::Batch(items) = a else { unreachable!() };
        assert!(matches!(&items[0].1, OsAction::ProjectCreate { domain_id, .. } if domain_id == "d-spain"));
        assert!(
            matches!(&items[1].1, OsAction::UserCreate(s) if s.domain_id == "d-spain"),
            "whatever the browser sent"
        );
        assert!(matches!(&items[2].1, OsAction::GroupCreate { domain_id, .. } if domain_id == "d-spain"));
        assert!(
            !OsAction::RoleCreate { name: "x".into() }.needs_domain(),
            "roles are global"
        );
    }

    #[test]
    fn member_changes_are_only_what_changed() {
        let row = |id: &str, roles: &[&str]| AssignmentRow {
            actor_id: id.into(),
            label: format!("user-{id}"),
            role_ids: roles.iter().map(|r| r.to_string()).collect(),
        };
        let before = [row("a", &["member"]), row("b", &["member", "reader"])];
        let after = [row("a", &["member", "admin"]), row("c", &["reader"])];
        let changes = assignment_changes("p", false, &before, &after);
        let kinds: Vec<_> = changes
            .iter()
            .map(|(_, a)| match a {
                OsAction::ProjectRoleGrant { user_id, role_id, .. } => {
                    format!("+{}:{role_id}", user_id.clone().unwrap())
                }
                OsAction::ProjectRoleRevoke { user_id, role_id, .. } => {
                    format!("-{}:{role_id}", user_id.clone().unwrap())
                }
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(
            kinds,
            ["+a:admin", "+c:reader", "-b:member", "-b:reader"],
            "a gains admin, c is added, b is removed"
        );
        assert!(assignment_changes("p", false, &before, &before).is_empty());
        let g = assignment_changes("p", true, &[], &[row("g", &["member"])]);
        assert!(
            matches!(&g[0].1, OsAction::ProjectRoleGrant { group_id: Some(gid), user_id: None, .. } if gid == "g"),
            "groups use the group path"
        );
    }
}
