//! The Identity dashboard: projects, users, groups, roles and
//! application credentials, with their detail pages and, for admins, Horizon's
//! actions (`ops::identity_*`). What each list holds follows the token's role
//! like Horizon (see `api::list_identity_*`); which panels show is
//! `nav::IDENTITY_NAV`.

use leptos::prelude::*;

use std::sync::Arc;

use super::details::Tabs;
use super::details::{Card, DetailHeader, Kv, param};
use super::lists::link;
use crate::api::*;
use crate::components::dialog::{BulkButton, CreateButton, MenuItem, RowActions, caps, data_version};
use crate::components::layout::set_active_project;
use crate::components::ops;
use crate::components::table::{Col, DataTable, Select, col};
use crate::components::widgets::{PageHeader, Skeleton, dash, render, utc_time};
use crate::i18n::{K, Lang, t, use_i18n};
use crate::model::*;

/// *Yes* / *No* in the user's language (same as `details::yes_no`).
fn yes_no(l: Lang, b: bool) -> &'static str {
    t(l, if b { K::Yes } else { K::No })
}

/// *Set as Active Project* on the user's projects other than the current one.
fn project_items(l: Lang, p: &IdentityProject) -> Vec<MenuItem> {
    if !p.member || p.current {
        return vec![];
    }
    let id = p.id.clone();
    vec![MenuItem::action(t(l, K::SetActiveProject), move || {
        set_active_project(id.clone())
    })]
}

/// Identity › Projects' columns: the name links to the project's page only when Keystone
/// lets the user read it (Horizon's `identity:get_project`: admins, or the current project),
/// then description, ID, domain, enabled and each row's menu.
fn project_columns() -> Vec<Col<IdentityProject>> {
    vec![
        // Horizon links the name when `identity:get_project` passes: admins, or
        // the project the user is signed in to.
        col(K::Name, |p: &IdentityProject, _| {
            if p.current || needs::IDENTITY_ADMIN.met(&caps()) {
                link(format!("/identity/projects/{}", p.id), p.name.clone())
            } else {
                view! { <span>{p.name.clone()}</span> }.into_any()
            }
        })
        .sort_text(|p| p.name.clone()),
        col(K::Description, |p: &IdentityProject, _| dash(p.description.clone())),
        col(
            K::ProjectId,
            |p: &IdentityProject, _| view! { <span class="mono">{p.id.clone()}</span> },
        ),
        col(K::DomainName, |p: &IdentityProject, _| p.domain.clone()).sort_text(|p| p.domain.clone()),
        col(K::EnabledCol, |p: &IdentityProject, l| yes_no(l, p.enabled)),
        col(K::Actions, |p: &IdentityProject, l| {
            let mut items = project_items(l, p);
            if needs::IDENTITY_ADMIN.met(&caps()) {
                items.extend(ops::identity_project_items(l, p, None, needs::ADMIN.met(&caps())));
            }
            view! { <RowActions items=items/> }
        })
        .class("actions"),
    ]
}

/// Identity › Projects: the domain's projects for admins (with *Create
/// Project*, edit and delete), the user's own otherwise (*Set as Active
/// Project*).
#[component]
pub fn IdentityProjectsPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_identity_projects());
    let columns: StoredValue<Vec<Col<IdentityProject>>> = StoredValue::new(project_columns());
    let select = Select {
        key: |p: &IdentityProject| (!p.current).then(|| (p.id.clone(), p.name.clone())),
        picked: RwSignal::new(Vec::new()),
    };
    let make: fn(String) -> OsAction = |id| OsAction::ProjectDelete { id };
    let admin = move || needs::IDENTITY_ADMIN.met(&caps());
    view! {
        <PageHeader>
            <Show when=admin>
                <BulkButton label=K::DeleteProjects picked=select.picked
                    open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteProjects, items, make))/>
                <CreateButton label=Signal::derive(move || i18n.t(K::CreateProject).to_string())
                    open=Arc::new(move || ops::create_project_dialog(i18n.lang()))/>
            </Show>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |td| {
                let search = |p: &IdentityProject| format!("{} {} {}", p.name, p.id, p.description.clone().unwrap_or_default());
                if admin() {
                    view! { <DataTable rows=td.data columns=columns.get_value() search=search timing=(td.millis, td.calls) on_refresh=move |_| data.refetch() select=select/> }.into_any()
                } else {
                    view! { <DataTable rows=td.data columns=columns.get_value() search=search timing=(td.millis, td.calls) on_refresh=move |_| data.refetch()/> }.into_any()
                }
            })}
        </Transition>
    }
}

/// Identity › Users' columns: name (linked), description, email, ID, enabled, domain and
/// each row's menu.
fn user_columns() -> Vec<Col<IdentityUser>> {
    vec![
        col(K::UserName, |u: &IdentityUser, _| {
            link(format!("/identity/users/{}", u.id), u.name.clone())
        })
        .sort_text(|u| u.name.clone()),
        col(K::Description, |u: &IdentityUser, _| dash(u.description.clone())),
        col(K::Email, |u: &IdentityUser, _| dash(u.email.clone())),
        col(
            K::UserId,
            |u: &IdentityUser, _| view! { <span class="mono">{u.id.clone()}</span> },
        ),
        col(K::EnabledCol, |u: &IdentityUser, l| yes_no(l, u.enabled)),
        col(K::DomainName, |u: &IdentityUser, _| u.domain.clone()).sort_text(|u| u.domain.clone()),
        col(K::Actions, |u: &IdentityUser, l| {
            let items = if needs::IDENTITY_ADMIN.met(&caps()) {
                ops::user_items(l, u, None)
            } else {
                vec![]
            };
            view! { <RowActions items=items/> }
        })
        .class("actions"),
    ]
}

/// Identity › Users: every user of the domain for admins (with *Create User*,
/// edit, password, enable/disable, delete), the user alone otherwise.
#[component]
pub fn IdentityUsersPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_identity_users());
    let columns: StoredValue<Vec<Col<IdentityUser>>> = StoredValue::new(user_columns());
    let select = Select {
        key: |u: &IdentityUser| Some((u.id.clone(), u.name.clone())),
        picked: RwSignal::new(Vec::new()),
    };
    let make: fn(String) -> OsAction = |id| OsAction::UserDelete { id };
    let admin = move || needs::IDENTITY_ADMIN.met(&caps());
    view! {
        <PageHeader>
            <Show when=admin>
                <BulkButton label=K::DeleteUsers picked=select.picked
                    open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteUsers, items, make))/>
                <CreateButton label=Signal::derive(move || i18n.t(K::CreateUser).to_string())
                    open=Arc::new(move || ops::create_user_dialog(i18n.lang()))/>
            </Show>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |td| {
                let search = |u: &IdentityUser| format!("{} {} {}", u.name, u.id, u.email.clone().unwrap_or_default());
                if admin() {
                    view! { <DataTable rows=td.data columns=columns.get_value() search=search timing=(td.millis, td.calls) on_refresh=move |_| data.refetch() select=select/> }.into_any()
                } else {
                    view! { <DataTable rows=td.data columns=columns.get_value() search=search timing=(td.millis, td.calls) on_refresh=move |_| data.refetch()/> }.into_any()
                }
            })}
        </Transition>
    }
}

/// Identity › Groups' columns: name (linked), description, ID, domain and each row's menu.
fn group_list_columns() -> Vec<Col<IdentityGroup>> {
    vec![
        col(K::Name, |g: &IdentityGroup, _| {
            link(format!("/identity/groups/{}", g.id), g.name.clone())
        })
        .sort_text(|g| g.name.clone()),
        col(K::Description, |g: &IdentityGroup, _| dash(g.description.clone())),
        col(
            K::GroupId,
            |g: &IdentityGroup, _| view! { <span class="mono">{g.id.clone()}</span> },
        ),
        col(K::DomainName, |g: &IdentityGroup, _| g.domain.clone()),
        col(
            K::Actions,
            |g: &IdentityGroup, l| view! { <RowActions items=ops::group_menu(l, g)/> },
        )
        .class("actions"),
    ]
}

/// Identity › Groups (admins): *Create Group*, edit and delete.
#[component]
pub fn IdentityGroupsPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_identity_groups());
    let columns: StoredValue<Vec<Col<IdentityGroup>>> = StoredValue::new(group_list_columns());
    let select = Select {
        key: |g: &IdentityGroup| Some((g.id.clone(), g.name.clone())),
        picked: RwSignal::new(Vec::new()),
    };
    let make: fn(String) -> OsAction = |id| OsAction::GroupDelete { id };
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteGroups picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteGroups, items, make))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateGroup).to_string())
                open=Arc::new(move || ops::create_group_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |td| view! {
                <DataTable rows=td.data columns=columns.get_value() search=|g: &IdentityGroup| format!("{} {}", g.name, g.id)
                    timing=(td.millis, td.calls) on_refresh=move |_| data.refetch() select=select/>
            })}
        </Transition>
    }
}

/// Identity › Roles' columns: name, ID, the roles it implies and each row's menu.
fn role_columns() -> Vec<Col<IdentityRole>> {
    vec![
        col(
            K::RoleName,
            |r: &IdentityRole, _| view! { <span class="strong">{r.name.clone()}</span> },
        )
        .sort_text(|r| r.name.clone()),
        col(
            K::RoleId,
            |r: &IdentityRole, _| view! { <span class="mono">{r.id.clone()}</span> },
        ),
        col(K::ImpliedRoles, |r: &IdentityRole, _| dash(Some(r.implies.join(", ")))),
        col(
            K::Actions,
            |r: &IdentityRole, l| view! { <RowActions items=ops::role_items(l, r)/> },
        )
        .class("actions"),
    ]
}

/// Identity › Roles (admins): *Create Role*, and per row *Edit* and *Delete
/// Role* (also several at once), as Horizon.
#[component]
pub fn IdentityRolesPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_identity_roles());
    // Built once, outside the view: inline in `view!` closures the column types
    // made type-checking crawl (AGENTS.md §6).
    let columns: StoredValue<Vec<Col<IdentityRole>>> = StoredValue::new(role_columns());
    let select = Select {
        key: |r: &IdentityRole| Some((r.id.clone(), r.name.clone())),
        picked: RwSignal::new(Vec::new()),
    };
    let make: fn(String) -> OsAction = |id| OsAction::RoleDelete { id };
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteRoles picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteRoles, items, make))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateRole).to_string())
                open=Arc::new(move || ops::create_role_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |td| view! {
                <DataTable rows=td.data columns=columns.get_value() search=|r: &IdentityRole| format!("{} {}", r.name, r.id)
                    timing=(td.millis, td.calls) on_refresh=move |_| data.refetch() select=select/>
            })}
        </Transition>
    }
}

/// Identity › Application Credentials' columns: name (linked), project, description,
/// expiration (*Never* without one), ID, roles and each row's menu.
fn app_credential_columns() -> Vec<Col<AppCredential>> {
    vec![
        col(K::Name, |a: &AppCredential, _| {
            link(format!("/identity/application_credentials/{}", a.id), a.name.clone())
        })
        .sort_text(|a| a.name.clone()),
        col(
            K::ProjectId,
            |a: &AppCredential, _| view! { <span class="mono">{dash(a.project_id.clone())}</span> },
        ),
        col(K::Description, |a: &AppCredential, _| dash(a.description.clone())),
        col(K::Expiration, |a: &AppCredential, l| {
            a.expires_at
                .as_deref()
                .map(utc_time)
                .unwrap_or_else(|| t(l, K::Never).to_string())
        })
        .sort_text(|a| a.expires_at.clone().unwrap_or_default()),
        col(
            K::Id,
            |a: &AppCredential, _| view! { <span class="mono">{a.id.clone()}</span> },
        ),
        col(K::Roles, |a: &AppCredential, _| a.roles.join(", ")),
        col(
            K::Actions,
            |a: &AppCredential, l| view! { <RowActions items=ops::app_credential_items(l, a)/> },
        )
        .class("actions"),
    ]
}

/// Identity › Application Credentials: the user's own, with *Create* (the
/// secret is shown once) and delete, one or several, as Horizon.
#[component]
pub fn AppCredentialsPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_app_credentials());
    // Built once, outside the view: inline in `view!` closures the column types
    // made type-checking crawl (AGENTS.md §6).
    let columns: StoredValue<Vec<Col<AppCredential>>> = StoredValue::new(app_credential_columns());
    let select = Select {
        key: |a: &AppCredential| Some((a.id.clone(), a.name.clone())),
        picked: RwSignal::new(Vec::new()),
    };
    let make: fn(String) -> OsAction = |id| OsAction::AppCredentialDelete {
        user_id: String::new(),
        id,
    };
    view! {
        <PageHeader>
            <BulkButton label=K::DeleteAppCredentials picked=select.picked
                open=Arc::new(move |l, items| ops::bulk_delete(l, K::DeleteAppCredentials, items, make))/>
            <CreateButton label=Signal::derive(move || i18n.t(K::CreateAppCredential).to_string())
                open=Arc::new(move || ops::create_app_credential_dialog(i18n.lang()))/>
        </PageHeader>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |td| view! {
                <DataTable rows=td.data columns=columns.get_value() search=|a: &AppCredential| format!("{} {} {}", a.name, a.id, a.roles.join(" "))
                    timing=(td.millis, td.calls) on_refresh=move |_| data.refetch() select=select/>
            })}
        </Transition>
    }
}

/// Identity › Projects › a project: Horizon's *Overview*, and for admins its
/// *Users* (direct roles and roles through groups) and *Groups* tabs.
#[component]
pub fn IdentityProjectDetailPage() -> impl IntoView {
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(i, _)| get_identity_project(i));
    let tab = RwSignal::new(0usize);
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let detail = StoredValue::new(d.clone());
                let (users, groups) = (d.users.clone(), d.groups.clone());
                let has_tabs = users.is_some();
                view! {
                    <DetailHeader group=K::Identity panel=K::Projects href="/identity/projects" name=d.project.name.clone()
                        timing=(td.millis, td.calls) items=Signal::derive(move || detail.with_value(|d| {
                            if d.users.is_some() { ops::identity_project_items(use_i18n().lang(), &d.project, Some(d), needs::ADMIN.met(&caps())) } else { vec![] }
                        }))/>
                    {if has_tabs {
                        let users = StoredValue::new(users.unwrap_or_default());
                        let groups = StoredValue::new(groups.unwrap_or_default());
                        view! {
                            <Tabs labels=vec![K::Overview, K::Users, K::Groups] active=tab/>
                            {move || match tab.get() {
                                1 => view! { <DataTable rows=users.get_value() columns=member_columns() search=|m: &ProjectMember| format!("{} {}", m.name, m.id)/> }.into_any(),
                                2 => view! { <DataTable rows=groups.get_value() columns=group_columns() search=|g: &ProjectGroup| format!("{} {}", g.name, g.id)/> }.into_any(),
                                _ => detail.with_value(project_overview),
                            }}
                        }.into_any()
                    } else {
                        detail.with_value(project_overview)
                    }}
                }
            })}
        </Transition>
    }
}

/// The *Overview* tab (Horizon's fields).
fn project_overview(d: &IdentityProjectDetail) -> AnyView {
    let p = d.project.clone();
    let (enabled, current) = (p.enabled, p.current);
    let quotas_href = if current {
        Some("/project/quotas".to_string())
    } else {
        needs::ADMIN
            .met(&caps())
            .then(|| format!("/identity/projects/{}/quotas", p.id))
    };
    let domain_id = dash(d.domain_id.clone());
    view! {
        <div class="cards">
            <Card title=K::Overview>
                <Kv label=K::Name>{p.name.clone()}</Kv>
                <Kv label=K::Id><span class="mono">{p.id.clone()}</span></Kv>
                <Kv label=K::DomainName>{p.domain.clone()}</Kv>
                <Kv label=K::DomainId><span class="mono">{domain_id}</span></Kv>
                <Kv label=K::EnabledCol>{move || yes_no(use_i18n().lang(), enabled)}</Kv>
                <Kv label=K::Description>{dash(p.description.clone())}</Kv>
                // The current project's quotas are on Project › Quotas; another's, for
                // admins, on its own Quotas page.
                {quotas_href.map(|href| view! { <Kv label=K::Quotas><a href=href>{move || use_i18n().t(K::ViewQuotas)}</a></Kv> })}
            </Card>
        </div>
    }
    .into_any()
}

/// A project's *Users* tab: each member with their direct roles and the roles they get
/// through groups.
fn member_columns() -> Vec<Col<ProjectMember>> {
    vec![
        col(K::UserName, |m: &ProjectMember, _| {
            link(format!("/identity/users/{}", m.id), m.name.clone())
        })
        .sort_text(|m| m.name.clone()),
        col(
            K::UserId,
            |m: &ProjectMember, _| view! { <span class="mono">{m.id.clone()}</span> },
        ),
        col(K::Roles, |m: &ProjectMember, _| dash(Some(m.roles.join(", ")))),
        col(K::RolesFromGroups, |m: &ProjectMember, _| {
            dash(Some(
                m.group_roles
                    .iter()
                    .map(|(r, g)| format!("{r} ({g})"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ))
        }),
    ]
}

/// A project's *Groups* tab: each group with a role on the project, and its roles.
fn group_columns() -> Vec<Col<ProjectGroup>> {
    vec![
        col(
            K::Name,
            |g: &ProjectGroup, _| view! { <span class="strong">{g.name.clone()}</span> },
        )
        .sort_text(|g| g.name.clone()),
        col(
            K::GroupId,
            |g: &ProjectGroup, _| view! { <span class="mono">{g.id.clone()}</span> },
        ),
        col(K::Roles, |g: &ProjectGroup, _| g.roles.join(", ")),
    ]
}

/// Identity › Users › a user: Horizon's overview (members can open only themselves).
#[component]
pub fn IdentityUserDetailPage() -> impl IntoView {
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(i, _)| get_identity_user(i));
    let tab = RwSignal::new(0usize);
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let name = d.user.name.clone();
                let detail = StoredValue::new(d.clone());
                let has_tabs = d.assignments.is_some();
                let items = Signal::derive(move || detail.with_value(|d| ops::identity_user_items(use_i18n().lang(), d)));
                view! {
                    <DetailHeader group=K::Identity panel=K::Users href="/identity/users" name=name
                        timing=(td.millis, td.calls) items=items/>
                    {if has_tabs {
                        let assignments = StoredValue::new(d.assignments.clone().unwrap_or_default());
                        let groups = StoredValue::new(d.groups.clone().unwrap_or_default());
                        view! {
                            <Tabs labels=vec![K::Overview, K::RoleAssignments, K::Groups] active=tab/>
                            {move || match tab.get() {
                                1 => view! { <DataTable rows=assignments.get_value() columns=assignment_columns()
                                    search=|a: &RoleAssignmentRow| format!("{} {}", a.role, a.scope)/> }.into_any(),
                                2 => view! { <DataTable rows=groups.get_value() columns=ref_group_columns()
                                    search=|g: &Ref| format!("{} {}", g.name, g.id)/> }.into_any(),
                                _ => detail.with_value(user_overview),
                            }}
                        }.into_any()
                    } else {
                        detail.with_value(user_overview)
                    }}
                }
            })}
        </Transition>
    }
}

/// A user's *Overview* tab (Horizon's fields).
fn user_overview(d: &IdentityUserDetail) -> AnyView {
    let u = d.user.clone();
    let (enabled, locked) = (u.enabled, d.lock_password);
    let domain_id = dash(d.domain_id.clone());
    let expires = d
        .password_expires_at
        .as_deref()
        .map(utc_time)
        .unwrap_or_else(|| "-".into());
    let primary = match &d.primary_project {
        Some(p) => link(format!("/identity/projects/{}", p.id), p.name.clone()),
        None => "-".into_any(),
    };
    let extra = d.extra.clone();
    view! {
        <div class="cards">
            <Card title=K::Overview>
                <Kv label=K::Name>{u.name.clone()}</Kv>
                <Kv label=K::Id><span class="mono">{u.id.clone()}</span></Kv>
                <Kv label=K::DomainName>{u.domain.clone()}</Kv>
                <Kv label=K::DomainId><span class="mono">{domain_id}</span></Kv>
                <Kv label=K::Description>{dash(u.description.clone())}</Kv>
                <Kv label=K::Email>{dash(u.email.clone())}</Kv>
                <Kv label=K::EnabledCol>{move || yes_no(use_i18n().lang(), enabled)}</Kv>
                <Kv label=K::PasswordExpiresAt>{expires}</Kv>
                <Kv label=K::LockPassword>{move || yes_no(use_i18n().lang(), locked)}</Kv>
                <Kv label=K::PrimaryProject>{primary}</Kv>
                {extra.into_iter().map(|(k, v)| view! { <dt class="mono">{k}</dt><dd>{v}</dd> }).collect_view()}
            </Card>
        </div>
    }
    .into_any()
}

/// A user's *Role Assignments* tab: scope (project, domain or system, the project linked),
/// role, and whether it is inherited (`OS-INHERIT`).
fn assignment_columns() -> Vec<Col<RoleAssignmentRow>> {
    vec![
        col(K::ProjectOrDomain, |a: &RoleAssignmentRow, l| {
            let kind = t(
                l,
                match a.scope_kind.as_str() {
                    "domain" => K::Domain,
                    "system" => K::System,
                    _ => K::Project,
                },
            );
            let label = format!("{kind}: {}", a.scope);
            match &a.project_id {
                Some(p) => link(format!("/identity/projects/{p}"), label),
                None => view! { <span>{label}</span> }.into_any(),
            }
        })
        .sort_text(|a| a.scope.clone()),
        col(K::Role, |a: &RoleAssignmentRow, _| a.role.clone()).sort_text(|a| a.role.clone()),
        col(K::Inherited, |a: &RoleAssignmentRow, l| yes_no(l, a.inherited)),
    ]
}

/// A user's *Groups* tab: each group linked, with its ID.
fn ref_group_columns() -> Vec<Col<Ref>> {
    vec![
        col(K::Name, |g: &Ref, _| {
            link(format!("/identity/groups/{}", g.id), g.name.clone())
        })
        .sort_text(|g| g.name.clone()),
        col(
            K::GroupId,
            |g: &Ref, _| view! { <span class="mono">{g.id.clone()}</span> },
        ),
    ]
}

/// Identity › Groups › a group: overview and members (Horizon's *Group
/// Management* page).
#[component]
pub fn IdentityGroupDetailPage() -> impl IntoView {
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(i, _)| get_identity_group(i));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let g = d.group.clone();
                let detail = StoredValue::new(d.clone());
                let items = Signal::derive(move || detail.with_value(|d| ops::identity_group_items(use_i18n().lang(), d)));
                let domain_id = dash(d.domain_id.clone());
                let users = d.users.clone();
                view! {
                    <DetailHeader group=K::Identity panel=K::Groups href="/identity/groups" name=g.name.clone()
                        timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Overview>
                            <Kv label=K::Name>{g.name.clone()}</Kv>
                            <Kv label=K::Id><span class="mono">{g.id.clone()}</span></Kv>
                            <Kv label=K::Description>{dash(g.description.clone())}</Kv>
                            <Kv label=K::DomainName>{g.domain.clone()}</Kv>
                            <Kv label=K::DomainId><span class="mono">{domain_id}</span></Kv>
                        </Card>
                    </div>
                    <h2 class="section-title">{move || use_i18n().t(K::GroupMembers)}</h2>
                    <DataTable rows=users columns=group_member_columns() search=|u: &IdentityUser| format!("{} {}", u.name, u.id)/>
                }
            })}
        </Transition>
    }
}

/// A group's *Members* table: name (linked), email, ID and enabled.
fn group_member_columns() -> Vec<Col<IdentityUser>> {
    vec![
        col(K::UserName, |u: &IdentityUser, _| {
            link(format!("/identity/users/{}", u.id), u.name.clone())
        })
        .sort_text(|u| u.name.clone()),
        col(K::Email, |u: &IdentityUser, _| dash(u.email.clone())),
        col(
            K::UserId,
            |u: &IdentityUser, _| view! { <span class="mono">{u.id.clone()}</span> },
        ),
        col(K::EnabledCol, |u: &IdentityUser, l| yes_no(l, u.enabled)),
    ]
}

/// Identity › Application Credentials › one: Horizon's detail page.
#[component]
pub fn AppCredentialDetailPage() -> impl IntoView {
    let id = param("id");
    let data = Resource::new(move || (id(), data_version()), |(i, _)| get_app_credential(i));
    view! {
        <Transition fallback=|| view! { <Skeleton rows=6/> }>
            {move || render(data.get(), move |td| {
                let d = td.data;
                let a = d.credential.clone();
                let items = {
                    let a = a.clone();
                    Signal::derive(move || ops::app_credential_items(use_i18n().lang(), &a))
                };
                let unrestricted = a.unrestricted;
                let expires = a.expires_at.clone();
                let rules = d.access_rules.clone();
                view! {
                    <DetailHeader group=K::Identity panel=K::ApplicationCredentials href="/identity/application_credentials"
                        name=a.name.clone() timing=(td.millis, td.calls) items=items/>
                    <div class="cards">
                        <Card title=K::Overview>
                            <Kv label=K::Name>{a.name.clone()}</Kv>
                            <Kv label=K::Id><span class="mono">{a.id.clone()}</span></Kv>
                            <Kv label=K::Description>{dash(a.description.clone())}</Kv>
                            <Kv label=K::ProjectId><span class="mono">{dash(a.project_id.clone())}</span></Kv>
                            <Kv label=K::Roles>{dash(Some(a.roles.join(", ")))}</Kv>
                            <Kv label=K::Expiration>{move || expires.as_deref().map(utc_time).unwrap_or_else(|| use_i18n().t(K::Never).to_string())}</Kv>
                            <Kv label=K::UnrestrictedDangerous>{move || yes_no(use_i18n().lang(), unrestricted)}</Kv>
                        </Card>
                    </div>
                    <h2 class="section-title">{move || use_i18n().t(K::AccessRules)}</h2>
                    <DataTable rows=rules columns=access_rule_columns() search=|r: &AccessRule| format!("{} {} {}", r.service, r.method, r.path)/>
                }
            })}
        </Transition>
    }
}

/// An application credential's *Access Rules*: service, HTTP method and path it may call.
fn access_rule_columns() -> Vec<Col<AccessRule>> {
    vec![
        col(K::Service, |r: &AccessRule, _| r.service.clone()),
        col(
            K::Method,
            |r: &AccessRule, _| view! { <span class="mono">{r.method.clone()}</span> },
        ),
        col(
            K::Path,
            |r: &AccessRule, _| view! { <span class="mono">{r.path.clone()}</span> },
        ),
    ]
}

/// Identity › Domains' columns: the context domain's name in bold, description, ID, enabled
/// and each row's *Set Domain Context*.
fn domain_columns() -> Vec<Col<IdentityDomain>> {
    vec![
        col(
            K::Name,
            |d: &IdentityDomain, _| view! { <span class:strong=d.context>{d.name.clone()}</span> },
        )
        .sort_text(|d| d.name.clone()),
        col(K::Description, |d: &IdentityDomain, _| dash(d.description.clone())),
        col(
            K::DomainId,
            |d: &IdentityDomain, _| view! { <span class="mono">{d.id.clone()}</span> },
        ),
        col(K::EnabledCol, |d: &IdentityDomain, l| yes_no(l, d.enabled)),
        col(K::Actions, |d: &IdentityDomain, l| {
            let items = if d.context {
                vec![]
            } else {
                let id = d.id.clone();
                vec![MenuItem::action(t(l, K::SetDomainContext), move || {
                    switch_domain_context(Some(id.clone()))
                })]
            };
            view! { <RowActions items=items/> }
        })
        .class("actions"),
    ]
}

/// Sets or clears the domain context, then reloads so every list follows it.
fn switch_domain_context(id: Option<String>) {
    leptos::task::spawn_local(async move {
        if set_domain_context(id).await.is_ok() {
            crate::components::layout::hard_reload(None);
        }
    });
}

/// Identity › Domains (multi-domain clouds, admins): Horizon's list with *Set
/// Domain Context*, which scopes Projects, Users and Groups to that domain.
#[component]
pub fn IdentityDomainsPage() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(data_version, |_| list_identity_domains());
    let columns: StoredValue<Vec<Col<IdentityDomain>>> = StoredValue::new(domain_columns());
    view! {
        <PageHeader/>
        <Transition fallback=|| view! { <Skeleton/> }>
            {move || render(data.get(), move |td| {
                let current = td.data.iter().find(|d| d.context).map(|d| d.name.clone());
                view! {
                    {current.map(|name| view! {
                        <div class="alert alert-ok domain-context">
                            <span><strong>{move || i18n.t(K::DomainContextIs)}</strong>" "{name}". "{move || i18n.t(K::DomainContextHelp)}</span>
                            <button type="button" class="btn btn-sm" on:click=move |_| switch_domain_context(None)>{move || i18n.t(K::ClearDomainContext)}</button>
                        </div>
                    })}
                    <DataTable rows=td.data columns=columns.get_value() search=|d: &IdentityDomain| format!("{} {}", d.name, d.id)
                        timing=(td.millis, td.calls) on_refresh=move |_| data.refetch()/>
                }
            })}
        </Transition>
    }
}
