//! Keystone writes (roadmap 57): projects, users, groups, roles, application credentials.

use super::*;

/// Access rules typed as Horizon accepts them: a JSON or YAML list of
/// `{service, method, path}` (YAML parses JSON too). Methods are upper-cased.
pub fn parse_access_rules(text: &str) -> Result<Vec<AccessRule>, String> {
    use yaml_rust2::{Yaml, YamlLoader};
    let docs = YamlLoader::load_from_str(text).map_err(|e| format!("access rules are not valid JSON or YAML: {e}"))?;
    let Some(Yaml::Array(items)) = docs.into_iter().next() else {
        return Err("access rules must be a list of rules".into());
    };
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let get = |k: &str| item[k].as_str().map(str::trim).unwrap_or_default().to_string();
            if !matches!(item, Yaml::Hash(_)) {
                return Err(format!("access rule {}: expected service, method and path", i + 1));
            }
            let rule = AccessRule {
                service: get("service"),
                method: get("method").to_ascii_uppercase(),
                path: get("path"),
            };
            rule.check().map_err(|e| format!("rule {}: {e}", i + 1))?;
            Ok(rule)
        })
        .collect()
}

pub(super) async fn app_credential_create(x: &mut Exec<'_>, spec: &AppCredentialSpec) -> R<()> {
    use openstack_sdk::api::identity::v3::user::application_credential::create as m;
    // Parsed first: the request borrows them until it is built.
    let rules = match opt(&spec.access_rules) {
        Some(text) => parse_access_rules(text).map_err(AppError::invalid)?,
        None => vec![],
    };
    let mut b = m::ApplicationCredentialBuilder::default();
    b.name(non_empty(&spec.name, "name")?)
        .unrestricted(Some(spec.unrestricted));
    if let Some(v) = opt(&spec.description) {
        b.description(Some(Cow::from(v)));
    }
    if let Some(v) = opt(&spec.secret) {
        b.secret(Some(Cow::from(v)));
    }
    if let Some(v) = opt(&spec.expires_at) {
        b.expires_at(Some(Cow::from(v)));
    }
    if !rules.is_empty() {
        let list = rules
            .iter()
            .map(|r| {
                let mut a = m::AccessRulesBuilder::default();
                a.service(r.service.as_str())
                    .path(r.path.as_str())
                    .method(enum_of::<m::Method>(&r.method, "access rule method")?);
                a.build().map_err(build_err)
            })
            .collect::<R<Vec<_>>>()?;
        b.access_rules(list);
    }
    // No roles: Keystone gives the credential all of the user's on the project.
    if !spec.roles.is_empty() {
        let roles = spec
            .roles
            .iter()
            .map(|r| m::RolesBuilder::default().name(r.as_str()).build().map_err(build_err))
            .collect::<R<Vec<_>>>()?;
        b.roles(roles);
    }
    let body = b.build().map_err(build_err)?;
    let ep = m::Request::builder()
        .user_id(non_empty(&spec.user_id, "user")?)
        .application_credential(body)
        .build()
        .map_err(build_err)?;
    // Keystone returns the secret once: shown to the user, never logged.
    if let Some(v) = x.call(ep).await? {
        let get = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let (id, name, secret) = (get("id"), get("name"), get("secret"));
        x.secret = Some(Secret {
            file_name: format!("app-credential-{name}.txt"),
            content: format!("Application credential ID: {id}\nName: {name}\nSecret: {secret}\n"),
            kind: SecretKind::AppCredential,
            fields: vec![("name".into(), name), ("id".into(), id), ("secret".into(), secret)],
            extra: vec![],
        });
    }
    Ok(())
}

pub(super) async fn project_create(
    x: &mut Exec<'_>,
    name: &str,
    description: &Option<String>,
    enabled: &bool,
    domain_id: &str,
) -> R<()> {
    use openstack_sdk::api::identity::v3::project::create as m;
    let mut b = m::ProjectBuilder::default();
    // Always explicit: without it Keystone uses its default domain (roadmap 57).
    b.name(non_empty(name, "project name")?)
        .enabled(*enabled)
        .domain_id(Some(Cow::from(non_empty(domain_id, "domain")?)));
    if let Some(d) = opt(description) {
        b.description(Some(Cow::from(d)));
    }
    x.create(
        m::Request::builder()
            .project(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "project",
    )
    .await?;
    Ok(())
}

pub(super) async fn project_update(x: &mut Exec<'_>, id: &str, name: &str, description: &str, enabled: &bool) -> R<()> {
    use openstack_sdk::api::identity::v3::project::set as m;
    let body = m::ProjectBuilder::default()
        .name(non_empty(name, "project name")?)
        .description(Some(Cow::from(description.trim())))
        .enabled(*enabled)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(non_empty(id, "project")?)
            .project(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

pub(super) async fn project_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::identity::v3::project::delete as m;
    x.send(
        m::Request::builder()
            .id(non_empty(id, "project")?)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

pub(super) async fn project_role_grant_or_revoke(
    x: &mut Exec<'_>,
    project_id: &str,
    user_id: &Option<String>,
    group_id: &Option<String>,
    role_id: &str,
    action: &OsAction,
) -> R<()> {
    use OsAction::*;
    let grant = matches!(action, ProjectRoleGrant { .. });
    let actor = match (opt(user_id), opt(group_id)) {
        (Some(u), None) => format!("users/{u}"),
        (None, Some(g)) => format!("groups/{g}"),
        _ => return Err(AppError::invalid("a role assignment needs a user or a group")),
    };
    let path = format!(
        "projects/{}/{actor}/roles/{}",
        non_empty(project_id, "project")?,
        non_empty(role_id, "role")?
    );
    let method = if grant { http::Method::PUT } else { http::Method::DELETE };
    x.send(JsonRequest::new(
        method,
        openstack_sdk::types::ServiceType::Identity,
        path,
        None,
    ))
    .await?;
    Ok(())
}

pub(super) async fn user_create(x: &mut Exec<'_>, spec: &UserSpec) -> R<()> {
    let mut user = serde_json::Map::new();
    user.insert("name".into(), json!(non_empty(&spec.name, "user name")?));
    // Always explicit: without it Keystone uses its default domain (roadmap 57).
    user.insert("domain_id".into(), json!(non_empty(&spec.domain_id, "domain")?));
    if spec.password.is_empty() {
        return Err(AppError::invalid("a password is required"));
    }
    user.insert("password".into(), json!(spec.password));
    user.insert("enabled".into(), json!(spec.enabled));
    if let Some(v) = opt(&spec.description) {
        user.insert("description".into(), json!(v));
    }
    if let Some(v) = opt(&spec.email) {
        user.insert("email".into(), json!(v));
    }
    if let Some(v) = opt(&spec.project_id) {
        user.insert("default_project_id".into(), json!(v));
    }
    if spec.lock_password {
        user.insert("options".into(), json!({"lock_password": true}));
    }
    // JSON, not `user::create`: Keystone's `email` is an extra attribute the builder lacks.
    let ep = JsonRequest::new(
        http::Method::POST,
        openstack_sdk::types::ServiceType::Identity,
        "users",
        Some(json!({"user": user})),
    );
    let new_id = x.create(ep, "user").await?;
    if let (Some(p), Some(r)) = (opt(&spec.project_id), opt(&spec.role_id)) {
        let path = format!("projects/{p}/users/{new_id}/roles/{r}");
        x.send(JsonRequest::new(
            http::Method::PUT,
            openstack_sdk::types::ServiceType::Identity,
            path,
            None,
        ))
        .await?;
    }
    Ok(())
}

pub(super) async fn user_update(
    x: &mut Exec<'_>,
    id: &str,
    name: &str,
    email: &str,
    description: &str,
    project_id: &Option<String>,
) -> R<()> {
    let body = json!({"user": {
        "name": non_empty(name, "user name")?,
        "email": email.trim(),
        "description": description.trim(),
        "default_project_id": opt(project_id),
    }});
    x.send(JsonRequest::new(
        http::Method::PATCH,
        openstack_sdk::types::ServiceType::Identity,
        format!("users/{}", non_empty(id, "user")?),
        Some(body),
    ))
    .await?;
    Ok(())
}

pub(super) async fn user_set_enabled(x: &mut Exec<'_>, id: &str, enabled: &bool) -> R<()> {
    let body = json!({"user": {"enabled": enabled}});
    x.send(JsonRequest::new(
        http::Method::PATCH,
        openstack_sdk::types::ServiceType::Identity,
        format!("users/{}", non_empty(id, "user")?),
        Some(body),
    ))
    .await?;
    Ok(())
}

pub(super) async fn user_set_password(x: &mut Exec<'_>, id: &str, password: &str) -> R<()> {
    if password.is_empty() {
        return Err(AppError::invalid("a password is required"));
    }
    let body = json!({"user": {"password": password}});
    x.send(JsonRequest::new(
        http::Method::PATCH,
        openstack_sdk::types::ServiceType::Identity,
        format!("users/{}", non_empty(id, "user")?),
        Some(body),
    ))
    .await?;
    Ok(())
}

pub(super) async fn user_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::identity::v3::user::delete as m;
    x.send(
        m::Request::builder()
            .id(non_empty(id, "user")?)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

pub(super) async fn group_create(x: &mut Exec<'_>, name: &str, description: &Option<String>, domain_id: &str) -> R<()> {
    use openstack_sdk::api::identity::v3::group::create as m;
    let mut b = m::GroupBuilder::default();
    // Always explicit: without it Keystone uses its default domain (roadmap 57).
    b.name(non_empty(name, "group name")?)
        .domain_id(non_empty(domain_id, "domain")?);
    if let Some(d) = opt(description) {
        b.description(Some(Cow::from(d)));
    }
    x.create(
        m::Request::builder()
            .group(b.build().map_err(build_err)?)
            .build()
            .map_err(build_err)?,
        "group",
    )
    .await?;
    Ok(())
}

pub(super) async fn group_update(x: &mut Exec<'_>, id: &str, name: &str, description: &str) -> R<()> {
    use openstack_sdk::api::identity::v3::group::set as m;
    let body = m::GroupBuilder::default()
        .name(non_empty(name, "group name")?)
        .description(Some(Cow::from(description.trim())))
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(non_empty(id, "group")?)
            .group(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

pub(super) async fn group_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::identity::v3::group::delete as m;
    x.send(
        m::Request::builder()
            .id(non_empty(id, "group")?)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

pub(super) async fn group_add_user(x: &mut Exec<'_>, group_id: &str, user_id: &str) -> R<()> {
    use openstack_sdk::api::identity::v3::group::user::set as m;
    x.send(
        m::Request::builder()
            .group_id(non_empty(group_id, "group")?)
            .id(non_empty(user_id, "user")?)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

pub(super) async fn group_remove_user(x: &mut Exec<'_>, group_id: &str, user_id: &str) -> R<()> {
    use openstack_sdk::api::identity::v3::group::user::delete as m;
    x.send(
        m::Request::builder()
            .group_id(non_empty(group_id, "group")?)
            .id(non_empty(user_id, "user")?)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

pub(super) async fn role_create(x: &mut Exec<'_>, name: &str) -> R<()> {
    use openstack_sdk::api::identity::v3::role::create as m;
    let body = m::RoleBuilder::default()
        .name(non_empty(name, "role name")?)
        .build()
        .map_err(build_err)?;
    x.create(m::Request::builder().role(body).build().map_err(build_err)?, "role")
        .await?;
    Ok(())
}

pub(super) async fn role_update(x: &mut Exec<'_>, id: &str, name: &str) -> R<()> {
    use openstack_sdk::api::identity::v3::role::set as m;
    let body = m::RoleBuilder::default()
        .name(non_empty(name, "role name")?)
        .build()
        .map_err(build_err)?;
    x.send(
        m::Request::builder()
            .id(non_empty(id, "role")?)
            .role(body)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

pub(super) async fn role_delete(x: &mut Exec<'_>, id: &str) -> R<()> {
    use openstack_sdk::api::identity::v3::role::delete as m;
    x.send(
        m::Request::builder()
            .id(non_empty(id, "role")?)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

pub(super) async fn app_credential_delete(x: &mut Exec<'_>, user_id: &str, id: &str) -> R<()> {
    use openstack_sdk::api::identity::v3::user::application_credential::delete as m;
    x.send(
        m::Request::builder()
            .user_id(non_empty(user_id, "user")?)
            .id(non_empty(id, "application credential")?)
            .build()
            .map_err(build_err)?,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[tokio::test]
    async fn application_credentials_are_created_with_the_secret_masked_and_deleted() {
        let spec = AppCredentialSpec {
            user_id: id("u1"),
            name: id("ci"),
            description: Some(id("pipeline")),
            secret: Some(id("s3cret")),
            expires_at: Some(id("2026-12-31T00:00:00")),
            roles: vec![id("power_user")],
            unrestricted: false,
            access_rules: None,
        };
        let r = dry(OsAction::AppCredentialCreate(Box::new(spec.clone()))).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
            ("POST", "/users/u1/application_credentials", "identity")
        );
        assert_eq!(
            body(&r[0]),
            json!({"application_credential": {"name": "ci", "description": "pipeline", "secret": "********",
                "expires_at": "2026-12-31T00:00:00", "roles": [{"name": "power_user"}], "unrestricted": false}}),
            "the chosen secret never shows in the preview"
        );
        let bare = AppCredentialSpec {
            description: None,
            secret: None,
            expires_at: None,
            roles: vec![],
            ..spec.clone()
        };
        assert_eq!(
            body(&dry(OsAction::AppCredentialCreate(Box::new(bare))).await[0]),
            json!({"application_credential": {"name": "ci", "unrestricted": false}}),
            "no roles = all of the user's"
        );
        assert!(
            dry_err(OsAction::AppCredentialCreate(Box::new(AppCredentialSpec {
                user_id: String::new(),
                ..spec
            })))
            .await,
            "run_action sets the user"
        );
        let r = dry(OsAction::AppCredentialDelete {
            user_id: id("u1"),
            id: id("a1"),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/users/u1/application_credentials/a1")
        );
    }

    #[tokio::test]
    async fn access_rules_are_read_as_json_or_yaml_like_horizon() {
        let json = r#"[{"service": "compute", "method": "POST", "path": "/v2.1/servers"}]"#;
        let yaml = "- service: compute\n  method: post\n  path: /v2.1/servers\n";
        let want = vec![AccessRule {
            service: id("compute"),
            method: id("POST"),
            path: id("/v2.1/servers"),
        }];
        assert_eq!(parse_access_rules(json).unwrap(), want, "Horizon's JSON example");
        assert_eq!(
            parse_access_rules(yaml).unwrap(),
            want,
            "Horizon's YAML example; the method upper-cased"
        );
        assert!(
            parse_access_rules("service: compute")
                .unwrap_err()
                .contains("must be a list")
        );
        assert!(
            parse_access_rules("- service: nova compute\n  method: GET\n  path: /")
                .unwrap_err()
                .contains("rule 1")
        );
        assert!(
            parse_access_rules("- service: compute\n  method: FETCH\n  path: /")
                .unwrap_err()
                .contains("method")
        );
        assert!(
            parse_access_rules("- service: compute\n  method: GET\n  path: v2.1")
                .unwrap_err()
                .contains("start with /")
        );
        assert!(parse_access_rules("[unclosed").is_err());

        let spec = AppCredentialSpec {
            user_id: id("u1"),
            name: id("ci"),
            description: None,
            secret: None,
            expires_at: None,
            roles: vec![],
            unrestricted: false,
            access_rules: Some(id(yaml)),
        };
        let r = dry(OsAction::AppCredentialCreate(Box::new(spec.clone()))).await;
        assert_eq!(
            body(&r[0])["application_credential"]["access_rules"],
            json!([{"service": "compute", "method": "POST", "path": "/v2.1/servers"}])
        );
        assert!(
            dry_err(OsAction::AppCredentialCreate(Box::new(AppCredentialSpec {
                access_rules: Some(id("nope")),
                ..spec
            })))
            .await,
            "refused before sending"
        );
    }

    #[tokio::test]
    async fn roles_are_created_renamed_and_deleted() {
        let r = dry(OsAction::RoleCreate { name: id(" auditor ") }).await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str(), r[0].service.as_str()),
            ("POST", "/roles", "identity")
        );
        assert_eq!(body(&r[0]), json!({"role": {"name": "auditor"}}), "trimmed");
        let r = dry(OsAction::RoleUpdate {
            id: id("r1"),
            name: id("auditors"),
        })
        .await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("PATCH", "/roles/r1"));
        assert_eq!(body(&r[0]), json!({"role": {"name": "auditors"}}));
        let r = dry(OsAction::RoleDelete { id: id("r1") }).await;
        assert_eq!((r[0].method.as_str(), r[0].path.as_str()), ("DELETE", "/roles/r1"));
        assert!(
            dry_err(OsAction::RoleCreate { name: id(" ") }).await,
            "a name is required"
        );
    }

    #[tokio::test]
    async fn identity_admin_requests_are_horizons() {
        let idy = |r: &RequestPreview| (r.method.clone(), r.path.clone());
        let r = dry(OsAction::ProjectCreate {
            name: id("web"),
            description: Some(id("Web team")),
            enabled: true,
            domain_id: id("d-spain"),
        })
        .await;
        assert_eq!(
            (idy(&r[0]), body(&r[0])),
            (
                (id("POST"), id("/projects")),
                json!({"project": {"name": "web", "description": "Web team", "enabled": true, "domain_id": "d-spain"}})
            ),
            "the domain is always explicit: Keystone would use Default"
        );
        assert!(
            dry_err(OsAction::ProjectCreate {
                name: id("web"),
                description: None,
                enabled: true,
                domain_id: String::new()
            })
            .await,
            "run_action sets the domain"
        );
        let r = dry(OsAction::ProjectUpdate {
            id: id("p1"),
            name: id("web"),
            description: String::new(),
            enabled: false,
        })
        .await;
        assert_eq!(
            (idy(&r[0]), body(&r[0])),
            (
                (id("PATCH"), id("/projects/p1")),
                json!({"project": {"name": "web", "description": "", "enabled": false}})
            )
        );
        assert_eq!(
            idy(&dry(OsAction::ProjectDelete { id: id("p1") }).await[0]),
            (id("DELETE"), id("/projects/p1"))
        );
        let grant = OsAction::ProjectRoleGrant {
            project_id: id("p1"),
            user_id: Some(id("u1")),
            group_id: None,
            role_id: id("r1"),
        };
        assert_eq!(
            idy(&dry(grant).await[0]),
            (id("PUT"), id("/projects/p1/users/u1/roles/r1"))
        );
        let revoke = OsAction::ProjectRoleRevoke {
            project_id: id("p1"),
            user_id: None,
            group_id: Some(id("g1")),
            role_id: id("r1"),
        };
        assert_eq!(
            idy(&dry(revoke).await[0]),
            (id("DELETE"), id("/projects/p1/groups/g1/roles/r1"))
        );
        assert!(
            dry_err(OsAction::ProjectRoleGrant {
                project_id: id("p1"),
                user_id: None,
                group_id: None,
                role_id: id("r1")
            })
            .await,
            "a user or a group"
        );

        let spec = UserSpec {
            domain_id: id("d-spain"),
            name: id("ana"),
            description: None,
            email: Some(id("ana@example.com")),
            password: id("Secr3t!"),
            project_id: Some(id("p1")),
            role_id: Some(id("r1")),
            enabled: true,
            lock_password: true,
        };
        let r = dry(OsAction::UserCreate(Box::new(spec.clone()))).await;
        assert_eq!(idy(&r[0]), (id("POST"), id("/users")));
        assert_eq!(
            body(&r[0]),
            json!({"user": {"name": "ana", "domain_id": "d-spain", "password": "********", "enabled": true, "email": "ana@example.com", "default_project_id": "p1", "options": {"lock_password": true}}}),
            "the password is masked in the preview"
        );
        assert_eq!(
            idy(&r[1]),
            (id("PUT"), id("/projects/p1/users/%3Cnew%20user%20id%3E/roles/r1")),
            "then the role on the primary project (the dry-run placeholder, encoded)"
        );
        assert!(
            dry_err(OsAction::UserCreate(Box::new(UserSpec {
                password: String::new(),
                ..spec
            })))
            .await
        );
        let r = dry(OsAction::UserUpdate {
            id: id("u1"),
            name: id("ana"),
            email: String::new(),
            description: id("ops"),
            project_id: None,
        })
        .await;
        assert_eq!(
            body(&r[0]),
            json!({"user": {"name": "ana", "email": "", "description": "ops", "default_project_id": null}}),
            "null clears the primary project"
        );
        let r = dry(OsAction::UserSetEnabled {
            id: id("u1"),
            enabled: false,
        })
        .await;
        assert_eq!(
            (idy(&r[0]), body(&r[0])),
            ((id("PATCH"), id("/users/u1")), json!({"user": {"enabled": false}}))
        );
        let r = dry(OsAction::UserSetPassword {
            id: id("u1"),
            password: id("n3w"),
        })
        .await;
        assert_eq!(body(&r[0]), json!({"user": {"password": "********"}}));
        assert_eq!(
            idy(&dry(OsAction::UserDelete { id: id("u1") }).await[0]),
            (id("DELETE"), id("/users/u1"))
        );

        let r = dry(OsAction::GroupCreate {
            name: id("ops"),
            description: None,
            domain_id: id("d-spain"),
        })
        .await;
        assert_eq!(
            (idy(&r[0]), body(&r[0])),
            (
                (id("POST"), id("/groups")),
                json!({"group": {"name": "ops", "domain_id": "d-spain"}})
            )
        );
        let r = dry(OsAction::GroupUpdate {
            id: id("g1"),
            name: id("ops"),
            description: id("Operators"),
        })
        .await;
        assert_eq!(
            (idy(&r[0]), body(&r[0])),
            (
                (id("PATCH"), id("/groups/g1")),
                json!({"group": {"name": "ops", "description": "Operators"}})
            )
        );
        assert_eq!(
            idy(&dry(OsAction::GroupDelete { id: id("g1") }).await[0]),
            (id("DELETE"), id("/groups/g1"))
        );
        assert_eq!(
            idy(&dry(OsAction::GroupAddUser {
                group_id: id("g1"),
                user_id: id("u1")
            })
            .await[0]),
            (id("PUT"), id("/groups/g1/users/u1"))
        );
        assert_eq!(
            idy(&dry(OsAction::GroupRemoveUser {
                group_id: id("g1"),
                user_id: id("u1")
            })
            .await[0]),
            (id("DELETE"), id("/groups/g1/users/u1"))
        );
    }

    #[tokio::test]
    async fn app_credential_delete_path() {
        let r = dry(OsAction::AppCredentialDelete {
            user_id: id("u1"),
            id: id("a1"),
        })
        .await;
        assert_eq!(
            (r[0].method.as_str(), r[0].path.as_str()),
            ("DELETE", "/users/u1/application_credentials/a1")
        );
    }
}
