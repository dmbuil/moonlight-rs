//! Server functions: the only bridge between the browser and OpenStack.
//! Bodies run on the server; the browser gets a typed RPC stub.
//!
//! OpenStack calls made from here are GETs, except `run_action` (writes),
//! `run_instance_log` / `run_instance_console` (read-only Nova POSTs, roadmap 31)
//! and `run_stack_validate` / `run_stack_preview` (read-only Heat POSTs, roadmap
//! 62), which all go through `Exec` and `[writes] mode`. Moonlight's own session
//! cookie is written on login, logout and project/region switch.

use leptos::prelude::*;
use leptos::server_fn::codec::{GetUrl, Json};

use crate::model::*;

#[cfg(feature = "ssr")]
use crate::server::{fetch, os, session};

/// What the login page shows (`[login]`); no session needed.
#[server(input = GetUrl)]
pub async fn get_login_prefs() -> Result<crate::prefs::LoginPrefs, AppError> {
    Ok(cfg().login.clone())
}

/// The domain to sign in to: the default when hidden or empty, and one of the
/// configured choices when there is a dropdown (checked here, not only in the form).
pub fn login_domain(prefs: &crate::prefs::LoginPrefs, submitted: &str) -> Result<String, AppError> {
    let d = submitted.trim();
    if prefs.domain_hidden || d.is_empty() {
        return Ok(prefs.default_domain.clone());
    }
    if !prefs.domain_choices.is_empty() && !prefs.domain_choices.iter().any(|c| c == d) {
        return Err(AppError::invalid("domain not offered"));
    }
    Ok(d.to_string())
}

/// Signs in with a password (Keystone `v3password`, unscoped), keeps the token in
/// the encrypted cookie scoped to the first project, and redirects to `/project`.
/// Throttled per username and address (roadmap 50).
#[server]
pub async fn login(username: String, password: String, domain: String) -> Result<(), AppError> {
    let username = username.trim().to_string();
    let domain = login_domain(&cfg().login, &domain)?;
    if username.is_empty() || password.is_empty() {
        return Err(AppError::invalid("missing credentials"));
    }
    // Throttling (roadmap 50): refuse before Keystone sees the attempt; only
    // rejected credentials count, not a Keystone that is down.
    let throttle = &crate::server::config::state().login_throttle;
    let client = session::client_ip();
    let keys = crate::server::throttle::keys(&domain, &username, Some(&client));
    if throttle.blocked(&keys).await {
        tracing::warn!(user = %username, client = %client, "login throttled");
        return Err(AppError::new(ErrorKind::TooManyAttempts, "too many failed sign-ins"));
    }
    let res = match os::login(&username, &password, &domain).await {
        Err(e) if e.kind == ErrorKind::Unauthenticated => {
            throttle.failed(&keys).await;
            return Err(e);
        }
        other => other?,
    };
    throttle.succeeded(&keys[0]).await;
    let sess = os::new_session(res, domain).await?;
    session::store(&sess);
    tracing::info!(user = %sess.user, project = %sess.project_name, client = %session::client_ip(), "login");
    leptos_axum::redirect("/project");
    Ok(())
}

/// Ends the sign-in. The server forgets the token at once, so a copied cookie
/// stops working (`os::forget`); with `[session] revoke_on_logout` (default)
/// Keystone revokes it too, through `Exec`, so only with `[writes] mode = on`.
/// Revocation is best effort: a failure is logged and the logout completes.
/// A WebSSO session then goes to `[websso] logout_url` when set, so the
/// identity provider ends its own session too (roadmap 80); else `/login`.
#[server]
pub async fn logout() -> Result<(), AppError> {
    let mut to = "/login".to_string();
    if let Some(s) = session::current() {
        if s.websso.is_some()
            && let Some(url) = cfg().websso.as_ref().and_then(|w| w.logout_url.clone())
        {
            to = url;
        }
        let config = &cfg();
        if config.revoke_on_logout {
            let revoked = match os::scoped(&s).await {
                Ok(c) => {
                    let mut x = crate::server::actions::Exec::new(config.writes, Some(&c));
                    os::within(os::REQUEST_TIMEOUT, "token revocation", crate::server::actions::revoke_token(&mut x, &s.token)).await.and_then(|r| r)
                }
                Err(e) => Err(e),
            };
            match revoked {
                Ok(()) => tracing::info!(user = %s.user, mode = ?config.writes, client = %session::client_ip(), "logout"),
                Err(e) => tracing::warn!(user = %s.user, client = %session::client_ip(), "logout: token not revoked: {e}"),
            }
        }
        os::forget(&s).await;
    }
    session::clear();
    leptos_axum::redirect(&to);
    Ok(())
}

/// Everything the layout needs: user, projects, regions, write mode, settings
/// and detected capabilities. Falls back to another region when the session's
/// one left the catalog.
#[server(input = GetUrl)]
pub async fn get_session_info() -> Result<SessionInfo, AppError> {
    let mut s = session::require()?;
    let (projects, scoped) = tokio::join!(os::projects(&s), os::scoped(&s));
    let scoped = scoped.ok();
    let regions = scoped.as_ref().map(os::regions).unwrap_or_default();
    let region_fallback = keep_region_valid(&mut s, &regions);
    // After a region fallback the connection points at the lost region: detect next time.
    let mut capabilities = match (&scoped, &region_fallback) {
        (Some(c), None) => os::capabilities(&s, c).await,
        _ => Capabilities::default(),
    };
    capabilities.plugins = cfg().plugins.clone();
    capabilities.multidomain = cfg().multidomain;
    Ok(SessionInfo {
        user: s.user.clone(),
        domain: s.domain.clone(),
        project: ProjectRef { id: s.project_id.clone(), name: s.project_name.clone() },
        projects: projects.unwrap_or_default(),
        region: s.region.clone().unwrap_or_default(),
        regions,
        write_mode: cfg().writes,
        help_url: cfg().help_url.clone(),
        change_password_url: cfg().change_password_url.clone(),
        prefs: cfg().prefs.clone(),
        password_expires_in_days: s.password_warning(chrono::Utc::now(), cfg().password_expiry_warning_days),
        region_fallback,
        capabilities,
    })
}

/// Switches the session to another of the user's projects (checked against
/// Keystone, and only if it can be scoped).
#[server]
pub async fn switch_project(project_id: String) -> Result<(), AppError> {
    let mut s = session::require()?;
    let project = os::projects(&s)
        .await?
        .into_iter()
        .find(|p| p.id == project_id)
        .ok_or_else(|| AppError::new(ErrorKind::Forbidden, "project not available to this user"))?;
    // Fail early (and keep the old session) if the project cannot be scoped.
    os::scoped_to(&s, &project.id, s.region.as_deref()).await?;
    s.project_id = project.id;
    s.project_name = project.name;
    session::store(&s);
    Ok(())
}

/// Switches the session to another region of the catalog.
#[server]
pub async fn switch_region(region: String) -> Result<(), AppError> {
    let mut s = session::require()?;
    let regions = os::regions(&os::scoped(&s).await?);
    if !regions.contains(&region) {
        return Err(AppError::new(ErrorKind::RegionUnavailable, format!("unknown region {region}")));
    }
    s.region = Some(region);
    session::store(&s);
    Ok(())
}

/// If the session's region left the catalog, switches (and remembers) the
/// first available one. Returns the region that was lost.
#[cfg(feature = "ssr")]
fn keep_region_valid(s: &mut session::Session, regions: &[String]) -> Option<String> {
    let next = os::fallback_region(s.region.as_deref(), regions)?.clone();
    let lost = s.region.replace(next);
    tracing::warn!(user = %s.user, lost = ?lost, region = ?s.region, "region not in catalog, falling back");
    session::store(s);
    lost
}

/// Shared prologue: session + scoped connection (in a region that exists).
#[cfg(feature = "ssr")]
async fn ctx() -> Result<(session::Session, openstack_sdk::AsyncOpenStack), AppError> {
    let mut s = session::require()?;
    let c = os::scoped(&s).await?;
    let before = s.region.clone();
    keep_region_valid(&mut s, &os::regions(&c));
    if s.region == before {
        return Ok((s, c));
    }
    let c = os::scoped(&s).await?;
    Ok((s, c))
}

/// Runs a server function's body with the session and its scoped connection,
/// bounded by `os::REQUEST_TIMEOUT`; `name` is what a timeout reports. The
/// body's GETs are counted for `Timed::calls` (`fetch::serving`, roadmap 65), and
/// its name lookups cached per session scope (roadmap 66).
#[cfg(feature = "ssr")]
async fn scoped<T, F>(name: &str, body: impl FnOnce(session::Session, openstack_sdk::AsyncOpenStack) -> F) -> Result<T, AppError>
where
    F: std::future::Future<Output = Result<T, AppError>>,
{
    let run = async move {
        // Debug timing (roadmap 83): getting the session and its connection,
        // then the body's requests, per server function and kind of session.
        let started = std::time::Instant::now();
        let (s, c) = ctx().await?;
        let connected = started.elapsed();
        let (user, websso) = (s.user.clone(), s.websso.is_some());
        let scope = os::scoped_key(&s.token, &s.project_id, s.region.as_deref());
        let (out, calls) = fetch::serving_counted(Some(scope), body(s, c)).await;
        tracing::debug!(%user, websso, connect_ms = connected.as_millis() as u64, body_ms = (started.elapsed() - connected).as_millis() as u64, calls, "{name}");
        out
    };
    os::within(os::REQUEST_TIMEOUT, name, run).await?
}

/// `scoped` for a resource named in the URL: checks `id` first, because it is
/// pasted into the request path unencoded (roadmap 48), then hands it to `body`.
#[cfg(feature = "ssr")]
async fn scoped_id<T, F>(name: &str, id: String, body: impl FnOnce(session::Session, openstack_sdk::AsyncOpenStack, String) -> F) -> Result<T, AppError>
where
    F: std::future::Future<Output = Result<T, AppError>>,
{
    check_id(&id)?;
    scoped(name, |s, c| body(s, c, id)).await
}

/// The settings read at start-up (`moonlight.conf`, `MOONLIGHT_*`).
#[cfg(feature = "ssr")]
fn cfg() -> &'static crate::server::config::Config {
    &crate::server::config::state().config
}

/// Overview *Limit Summary*: compute, volume and network quotas with usage.
#[server(input = GetUrl)]
pub async fn overview() -> Result<Timed<Vec<QuotaSection>>, AppError> {
    scoped("overview", |s, c| async move {
        fetch::overview(&c, &s.project_id).await
    })
    .await
}

/// Project › Quotas (roadmap 82): every quota of the current project, per service.
#[server(input = GetUrl)]
pub async fn list_quotas() -> Result<Timed<Vec<QuotaCard>>, AppError> {
    scoped("list_quotas", |s, c| async move {
        // `os::capabilities` has no plugins (configuration, added by `get_session_info`).
        let caps = os::capabilities(&s, &c).await;
        let on = |p: Plugin, service| cfg().plugins.contains(&p) && Requires::Service(service).met(&caps);
        fetch::quotas(&c, &s.project_id, &s.project_id, on(Plugin::LoadBalancer, "load-balancer"), on(Plugin::KeyManager, "key-manager")).await
    })
    .await
}

/// Identity › Projects › *Modify Quotas* (roadmap 82): every quota of project
/// `id`, any project, with its name for the page header. Admins only, checked
/// on the token before any call (the menu is not the gate).
#[server(input = GetUrl)]
pub async fn list_project_quotas(id: String) -> Result<Timed<ProjectQuotas>, AppError> {
    scoped_id("list_project_quotas", id, |s, c, id| async move {
        require_admin(&c)?;
        let caps = os::capabilities(&s, &c).await;
        let on = |p: Plugin, service| cfg().plugins.contains(&p) && Requires::Service(service).met(&caps);
        let (name, t) = tokio::join!(
            fetch::project_name(&c, &id),
            fetch::quotas(&c, &id, &s.project_id, on(Plugin::LoadBalancer, "load-balancer"), on(Plugin::KeyManager, "key-manager"))
        );
        let t = t?;
        Ok(Timed { data: ProjectQuotas { project_id: id.clone(), project_name: name.unwrap_or(id), cards: t.data }, millis: t.millis, calls: t.calls })
    })
    .await
}

/// Overview *Usage Summary*. Empty dates = this month up to today.
#[server(input = GetUrl)]
pub async fn get_usage(start: String, end: String) -> Result<Timed<UsageSummary>, AppError> {
    scoped("get_usage", |s, c| async move {
        let (from, to) = usage_range(&start, &end, chrono::Utc::now().date_naive()).map_err(AppError::invalid)?;
        let stop = to.succ_opt().unwrap_or(to);
        let t = fetch::usage(&c, &s.project_id, &format!("{from}T00:00:00"), &format!("{stop}T00:00:00")).await?;
        Ok(Timed {
            data: UsageSummary { start: from.to_string(), end: to.to_string(), servers: t.data },
            millis: t.millis,
            calls: t.calls,
        })
    })
    .await
}

/// Project › Compute › Instances.
#[server(input = GetUrl)]
pub async fn list_instances() -> Result<Timed<Vec<Instance>>, AppError> {
    scoped("list_instances", |_, c| async move {
        fetch::instances(&c).await
    })
    .await
}

/// Instance detail page.
#[server(input = GetUrl)]
pub async fn get_instance(id: String) -> Result<Timed<InstanceDetail>, AppError> {
    scoped_id("get_instance", id, |s, c, id| async move {
        fetch::instance(&c, &s.project_id, &id).await
    })
    .await
}

/// Project › Compute › Images: every image Glance shows the project, its own first.
#[server(input = GetUrl)]
pub async fn list_images() -> Result<Timed<Vec<Image>>, AppError> {
    scoped("list_images", |s, c| async move {
        let mine = os::projects(&s).await.unwrap_or_default();
        fetch::images(&c, &s.project_id, &mine).await
    })
    .await
}

/// Project › Compute › Key Pairs.
#[server(input = GetUrl)]
pub async fn list_keypairs() -> Result<Timed<Vec<KeyPair>>, AppError> {
    scoped("list_keypairs", |_, c| async move {
        fetch::keypairs(&c).await
    })
    .await
}

/// Project › Compute › Server Groups.
#[server(input = GetUrl)]
pub async fn list_server_groups() -> Result<Timed<Vec<ServerGroup>>, AppError> {
    scoped("list_server_groups", |_, c| async move {
        fetch::server_groups(&c).await
    })
    .await
}

/// Project › Volumes › Volumes.
#[server(input = GetUrl)]
pub async fn list_volumes() -> Result<Timed<Vec<Volume>>, AppError> {
    scoped("list_volumes", |_, c| async move {
        fetch::volumes(&c).await
    })
    .await
}

/// Project › Volumes › Snapshots.
#[server(input = GetUrl)]
pub async fn list_snapshots() -> Result<Timed<Vec<Snapshot>>, AppError> {
    scoped("list_snapshots", |_, c| async move {
        fetch::snapshots(&c).await
    })
    .await
}

/// Project › Network › Networks.
#[server(input = GetUrl)]
pub async fn list_networks() -> Result<Timed<Vec<NetworkRow>>, AppError> {
    scoped("list_networks", |s, c| async move {
        fetch::networks(&c, &s.project_id).await
    })
    .await
}

/// Project › Network › Routers.
#[server(input = GetUrl)]
pub async fn list_routers() -> Result<Timed<Vec<RouterRow>>, AppError> {
    scoped("list_routers", |s, c| async move {
        fetch::routers(&c, &s.project_id).await
    })
    .await
}

/// Project › Network › Security Groups.
#[server(input = GetUrl)]
pub async fn list_security_groups() -> Result<Timed<Vec<SecurityGroupRow>>, AppError> {
    scoped("list_security_groups", |s, c| async move {
        fetch::security_groups(&c, &s.project_id).await
    })
    .await
}

/// Project › Network › Floating IPs.
#[server(input = GetUrl)]
pub async fn list_floating_ips() -> Result<Timed<Vec<FloatingIpRow>>, AppError> {
    scoped("list_floating_ips", |s, c| async move {
        fetch::floating_ips(&c, &s.project_id).await
    })
    .await
}

/// Project › Network › Network Topology.
#[server(input = GetUrl)]
pub async fn get_topology() -> Result<Timed<Topology>, AppError> {
    scoped("get_topology", |s, c| async move {
        fetch::topology(&c, &s.project_id).await
    })
    .await
}

/// Session identity for the API Access page (no secrets; see `Credentials`).
#[server(input = GetUrl)]
pub async fn get_credentials() -> Result<Credentials, AppError> {
    let s = session::require()?;
    let mut c = session_credentials(&s);
    (c.openrc, c.clouds_yaml) = crate::server::downloads::api_access(&c);
    Ok(c)
}

/// Who and where the session is, for the downloads (API Access, application credentials).
#[cfg(feature = "ssr")]
fn session_credentials(s: &session::Session) -> Credentials {
    let url = &cfg().keystone_url;
    let auth_url = if url.ends_with("/v3") { url.clone() } else { format!("{url}/v3") };
    Credentials {
        user_name: s.user.clone(),
        user_id: s.user_id.clone(),
        user_domain: s.domain.clone(),
        federated: s.websso.is_some(),
        identity_provider: s.websso.as_ref().and_then(|w| w.identity_provider.clone()),
        protocol: s.websso.as_ref().map(|w| w.protocol.clone()),
        project_name: s.project_name.clone(),
        project_id: s.project_id.clone(),
        region: s.region.clone().unwrap_or_default(),
        auth_url,
        interface: cfg().interface.clone(),
        cloud_name: cfg().clouds_yaml_name.clone(),
        profile: cfg().clouds_yaml_profile.clone(),
        ..Default::default()
    }
}

/// Project › API Access: the catalog's endpoints in this region.
#[server(input = GetUrl)]
pub async fn api_access() -> Result<Vec<ApiEndpoint>, AppError> {
    scoped("api_access", |s, c| async move {
        Ok(os::catalog_listing(&c, s.region.as_deref()))
    })
    .await
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

/// Runs (or, in dry-run mode, only builds) a mutating action. JSON, because
/// the default URL encoding drops empty lists (`dns_nameservers: []`) and the
/// server then cannot deserialise the action.
#[server(input = Json)]
pub async fn run_action(action: OsAction) -> Result<ActionOutcome, AppError> {
    use crate::server::actions::{apply, Exec};
    let mode = cfg().writes;
    let s = session::require()?;
    // IDs end up in request paths, unencoded (roadmap 48).
    action.check_ids()?;
    // Users manage their own application credentials (roadmap 57).
    let mut action = action;
    action.set_user(&s.user_id);
    // Projects, users and groups go to the user's domain (or the domain context),
    // never Keystone's default one (roadmap 57). A GET, so also in dry-run.
    if action.needs_domain() {
        let domain = fetch::operation_domain(&os::scoped(&s).await?, &s).await?;
        action.set_domain(&domain);
    }
    // Horizon's identity safety rules (roadmap 57), whatever the dialogs allowed.
    action.identity_safety(&s.user_id, &s.project_id).map_err(AppError::invalid)?;
    action.plugins().into_iter().try_for_each(plugin_enabled)?;
    // Admin-dashboard writes (roadmap 85) and another project's quotas (roadmap
    // 82): the token's role, not only the menu or the services' policy. A
    // rescope, so also in dry-run.
    if action.admin_only() || action.other_projects_quotas(&s.project_id) {
        require_admin(&os::scoped(&s).await?)?;
    }
    // Glance's own properties and `[images] reserved_properties`, whatever the dialog allowed.
    let reserved = &cfg().prefs.images.reserved_properties;
    if let Some(k) = action.image_property_keys().into_iter().find(|k| is_reserved_image_key(k, reserved)) {
        return Err(AppError::invalid(format!("{k} is a reserved image property")));
    }
    let net = &cfg().prefs.network;
    for cidr in action.new_subnet_cidrs() {
        net.check_subnet(cidr).map_err(|e| AppError::invalid(format!("subnet {cidr}: {e}")))?;
    }
    let c = if mode == WriteMode::On { Some(os::scoped(&s).await?) } else { None };
    let mut x = Exec::new(mode, c.as_ref());
    match apply(&mut x, &action).await {
        Ok(()) => {
            // Names may have changed (a renamed network, a new volume): the next
            // page loads fetch the lookups again (roadmap 66). Dry-run changes nothing.
            if mode == WriteMode::On {
                crate::server::fetch::forget_lookups(&os::scoped_key(&s.token, &s.project_id, s.region.as_deref())).await;
            }
            let mut out = x.outcome();
            // A new application credential: Horizon's openrc and clouds.yaml, with the secret.
            if let Some(sec) = out.secret.as_mut().filter(|s| s.kind == SecretKind::AppCredential) {
                let get = |k: &str| sec.fields.iter().find(|(f, _)| f == k).map(|(_, v)| v.clone()).unwrap_or_default();
                let (id, name, secret) = (get("id"), get("name"), get("secret"));
                sec.extra = crate::server::downloads::app_credential(&session_credentials(&s), &id, &name, &secret);
            }
            let failed = out.results.iter().filter(|r| r.error.is_some()).count();
            tracing::info!(user = %s.user, project = %s.project_name, client = %session::client_ip(), ?mode, requests = out.requests.len(), failed, action = %action.kind(), "write");
            Ok(out.shown(cfg().show_requests))
        }
        Err(e) => {
            tracing::warn!(user = %s.user, client = %session::client_ip(), ?mode, action = %action.kind(), "write failed: {e}");
            Err(e)
        }
    }
}

/// *Create Image* › *File* with `[images] upload = direct` (roadmap 45): the URL and a
/// scoped token for one `PUT` of `bytes` straight to Glance. Only with `[writes] mode = on`.
#[server]
pub async fn get_image_upload_ticket(id: String, bytes: u64) -> Result<UploadTicket, AppError> {
    check_id(&id)?;
    let parts = leptos::prelude::use_context::<http::request::Parts>().ok_or_else(|| AppError::internal("no request"))?;
    crate::server::upload::ticket(&parts.headers, &id, bytes).await
}

/// The instance *Log* tab: the last `lines` lines, or the whole log.
#[server]
pub async fn run_instance_log(id: String, lines: Option<u32>) -> Result<ConsoleReply<String>, AppError> {
    scoped_id("run_instance_log", id, |s, c, id| async move {
        let mode = cfg().writes;
        let out = crate::server::console::log(mode, &cfg().prefs.console, Some(&c), &id, lines).await;
        tracing::info!(user = %s.user, project = %s.project_name, client = %session::client_ip(), ?mode, server = %id, ok = out.is_ok(), "instance log");
        out.map(|r| r.shown(cfg().show_requests))
    })
    .await
}

/// The instance *Console* tab: a remote console URL (each call makes a new token).
#[server]
pub async fn run_instance_console(id: String) -> Result<ConsoleReply<RemoteConsole>, AppError> {
    scoped_id("run_instance_console", id, |s, c, id| async move {
        let mode = cfg().writes;
        let remote_consoles = needs::REMOTE_CONSOLE.met(&os::capabilities(&s, &c).await);
        let out = crate::server::console::remote(mode, &cfg().prefs.console, Some(&c), &id, remote_consoles).await;
        tracing::info!(user = %s.user, project = %s.project_name, client = %session::client_ip(), ?mode, server = %id, ok = out.is_ok(), "instance console");
        out.map(|r| r.shown(cfg().show_requests))
    })
    .await
}

/// *Launch Stack* / *Change Stack Template*: the template's parameters (Heat's
/// `POST validate`; read from the text in dry-run). Never logs the template.
// JSON, like `run_action`: the default form encoding drops empty lists
// (`hidden: []`), which then fail to deserialise.
#[server(input = Json)]
pub async fn run_stack_validate(source: TemplateSource) -> Result<ConsoleReply<TemplateInfo>, AppError> {
    plugin_enabled(Plugin::Orchestration)?;
    scoped("run_stack_validate", |s, c| async move {
        let mode = cfg().writes;
        let out = crate::server::heat::validate(mode, Some(&c), &source).await;
        tracing::info!(user = %s.user, project = %s.project_name, client = %session::client_ip(), ?mode, ok = out.is_ok(), "stack validate");
        out.map(|r| r.shown(cfg().show_requests))
    })
    .await
}

/// *Preview Stack*: what the stack would create (Heat's `POST stacks/preview`).
// JSON, like `run_action`: the default form encoding drops empty lists
// (`hidden: []`), which then fail to deserialise.
#[server(input = Json)]
pub async fn run_stack_preview(spec: StackSpec) -> Result<ConsoleReply<Vec<PreviewResource>>, AppError> {
    plugin_enabled(Plugin::Orchestration)?;
    scoped("run_stack_preview", |s, c| async move {
        let mode = cfg().writes;
        let out = crate::server::heat::preview(mode, Some(&c), &spec).await;
        tracing::info!(user = %s.user, project = %s.project_name, client = %session::client_ip(), ?mode, stack = %spec.name, ok = out.is_ok(), "stack preview");
        out.map(|r| r.shown(cfg().show_requests))
    })
    .await
}

/// Options of a dialog field (`ChoiceSet`), GET only.
#[server(input = GetUrl)]
pub async fn get_choices(set: ChoiceSet) -> Result<Vec<Choice>, AppError> {
    if set == ChoiceSet::Secrets {
        plugin_enabled(Plugin::KeyManager)?;
    }
    if set == ChoiceSet::LbCertificates {
        plugin_enabled(Plugin::LoadBalancer)?;
        plugin_enabled(Plugin::KeyManager)?;
    }
    scoped("get_choices", |s, c| async move {
        // Needs the session (domain context, user): answered here.
        if set == ChoiceSet::OperationDomain {
            let (id, name) = fetch::operation_domain_ref(&c, &s).await?;
            return Ok(vec![Choice::new(id, name)]);
        }
        fetch::choices(&c, &s.project_id, set).await
    })
    .await
}

// ---------------------------------------------------------------------------
// Detail views
// ---------------------------------------------------------------------------

/// Network detail page (subnets and ports).
#[server(input = GetUrl)]
pub async fn get_network(id: String) -> Result<Timed<NetworkDetail>, AppError> {
    scoped_id("get_network", id, |s, c, id| async move {
        fetch::network_detail(&c, &s.project_id, &id).await
    })
    .await
}

/// Subnet detail page.
#[server(input = GetUrl)]
pub async fn get_subnet(id: String) -> Result<Timed<SubnetRow>, AppError> {
    scoped_id("get_subnet", id, |s, c, id| async move {
        fetch::subnet_detail(&c, &s.project_id, &id).await
    })
    .await
}

/// Port detail page.
#[server(input = GetUrl)]
pub async fn get_port(id: String) -> Result<Timed<PortRow>, AppError> {
    scoped_id("get_port", id, |s, c, id| async move {
        fetch::port_detail(&c, &s.project_id, &id).await
    })
    .await
}

/// Project › Network › Trunks (`trunk` plugin).
#[server(input = GetUrl)]
pub async fn list_trunks() -> Result<Timed<Vec<TrunkRow>>, AppError> {
    plugin_enabled(Plugin::Trunk)?;
    scoped("list_trunks", |s, c| async move {
        fetch::trunks(&c, &s.project_id).await
    })
    .await
}

/// Trunk detail page (`trunk` plugin).
#[server(input = GetUrl)]
pub async fn get_trunk(id: String) -> Result<Timed<TrunkRow>, AppError> {
    plugin_enabled(Plugin::Trunk)?;
    scoped_id("get_trunk", id, |s, c, id| async move {
        fetch::trunk(&c, &s.project_id, &id).await
    })
    .await
}

/// Project › Network › Network QoS (`qos` plugin).
#[server(input = GetUrl)]
pub async fn list_qos_policies() -> Result<Timed<Vec<QosPolicyRow>>, AppError> {
    plugin_enabled(Plugin::Qos)?;
    scoped("list_qos_policies", |s, c| async move {
        fetch::qos_policies(&c, &s.project_id).await
    })
    .await
}

/// QoS policy detail page (`qos` plugin).
#[server(input = GetUrl)]
pub async fn get_qos_policy(id: String) -> Result<Timed<QosPolicyRow>, AppError> {
    plugin_enabled(Plugin::Qos)?;
    scoped_id("get_qos_policy", id, |s, c, id| async move {
        fetch::qos_policy(&c, &s.project_id, &id).await
    })
    .await
}

/// Key Manager › Secrets (`key-manager` plugin, roadmap 55): metadata only.
#[server(input = GetUrl)]
pub async fn list_secrets() -> Result<Timed<Vec<SecretRow>>, AppError> {
    plugin_enabled(Plugin::KeyManager)?;
    scoped("list_secrets", |_, c| async move {
        fetch::secrets(&c).await
    })
    .await
}

/// Secret detail page (`key-manager` plugin): metadata, ACL, consumers.
#[server(input = GetUrl)]
pub async fn get_secret(id: String) -> Result<Timed<SecretDetail>, AppError> {
    plugin_enabled(Plugin::KeyManager)?;
    let allowed = cfg().key_manager_payload;
    scoped_id("get_secret", id, |s, c, id| async move {
        let lb = cfg().plugins.contains(&Plugin::LoadBalancer).then_some(s.project_id.as_str());
        let mut d = fetch::secret(&c, &id, allowed, lb).await?;
        let roles = os::capabilities(&s, &c).await.roles;
        d.data.acl.can_edit = can_edit_secret_acl(d.data.secret.creator_id.as_deref(), &s.user_id, &roles);
        Ok(d)
    })
    .await
}

/// *Show Payload* (`[key_manager] allow_payload`, roadmap 55): the secret
/// material, on click only. A POST so no cache keeps it; never logged.
#[server]
pub async fn get_secret_payload(id: String) -> Result<SecretPayload, AppError> {
    plugin_enabled(Plugin::KeyManager)?;
    if !cfg().key_manager_payload {
        return Err(AppError::invalid("showing secret payloads is disabled ([key_manager] allow_payload)"));
    }
    if let Some(r) = use_context::<leptos_axum::ResponseOptions>() {
        r.insert_header(http::header::CACHE_CONTROL, http::HeaderValue::from_static("no-store"));
    }
    scoped_id("get_secret_payload", id, |s, c, id| async move {
        let (payload, content_type, base64) = fetch::secret_payload(&c, &id).await?;
        tracing::info!(user = %s.user, project = %s.project_name, client = %session::client_ip(), "secret payload read");
        Ok(SecretPayload { payload: Hidden(payload), content_type, base64 })
    })
    .await
}

/// Key Manager › Containers (`key-manager` plugin).
#[server(input = GetUrl)]
pub async fn list_containers() -> Result<Timed<Vec<ContainerRow>>, AppError> {
    plugin_enabled(Plugin::KeyManager)?;
    scoped("list_containers", |_, c| async move {
        fetch::containers(&c).await
    })
    .await
}

/// Container detail page (`key-manager` plugin): the container and its `read`
/// ACL, editable by its creator or an admin (roadmap 75).
#[server(input = GetUrl)]
pub async fn get_container(id: String) -> Result<Timed<ContainerDetail>, AppError> {
    plugin_enabled(Plugin::KeyManager)?;
    scoped_id("get_container", id, |s, c, id| async move {
        let lb = cfg().plugins.contains(&Plugin::LoadBalancer).then_some(s.project_id.as_str());
        let mut d = fetch::container(&c, &id, lb).await?;
        let roles = os::capabilities(&s, &c).await.roles;
        d.data.acl.can_edit = can_edit_secret_acl(d.data.container.creator_id.as_deref(), &s.user_id, &roles);
        Ok(d)
    })
    .await
}

/// Orchestration › Stacks (`orchestration` plugin, roadmap 62).
#[server(input = GetUrl)]
pub async fn list_stacks() -> Result<Timed<Vec<StackRow>>, AppError> {
    plugin_enabled(Plugin::Orchestration)?;
    scoped("list_stacks", |_, c| async move {
        fetch::stacks(&c).await
    })
    .await
}

/// Stack detail page (`orchestration` plugin): overview, resources, events, template.
#[server(input = GetUrl)]
pub async fn get_stack(id: String) -> Result<Timed<StackDetail>, AppError> {
    plugin_enabled(Plugin::Orchestration)?;
    scoped_id("get_stack", id, |_, c, id| async move {
        fetch::stack(&c, &id).await
    })
    .await
}

/// Orchestration › Resource Types (`orchestration` plugin).
#[server(input = GetUrl)]
pub async fn list_resource_types() -> Result<Timed<Vec<ResourceTypeRow>>, AppError> {
    plugin_enabled(Plugin::Orchestration)?;
    scoped("list_resource_types", |_, c| async move {
        fetch::resource_types(&c).await
    })
    .await
}

/// A resource type's schema (`orchestration` plugin); `id` is the type name.
#[server(input = GetUrl)]
pub async fn get_resource_type(id: String) -> Result<Timed<ResourceTypeDetail>, AppError> {
    plugin_enabled(Plugin::Orchestration)?;
    scoped_id("get_resource_type", id, |_, c, id| async move {
        fetch::resource_type(&c, &id).await
    })
    .await
}

/// Orchestration › Template Versions (`orchestration` plugin).
#[server(input = GetUrl)]
pub async fn list_template_versions() -> Result<Timed<Vec<TemplateVersion>>, AppError> {
    plugin_enabled(Plugin::Orchestration)?;
    scoped("list_template_versions", |_, c| async move {
        fetch::template_versions(&c).await
    })
    .await
}

/// A template version's functions (`orchestration` plugin); `id` is the version.
#[server(input = GetUrl)]
pub async fn get_template_functions(id: String) -> Result<Timed<Vec<TemplateFunction>>, AppError> {
    plugin_enabled(Plugin::Orchestration)?;
    scoped_id("get_template_functions", id, |_, c, id| async move {
        fetch::template_functions(&c, &id).await
    })
    .await
}

/// Monitoring › Alarms (`alarming` plugin, roadmap 79): the project's Aodh alarms.
#[server(input = GetUrl)]
pub async fn list_alarms() -> Result<Timed<Vec<AlarmRow>>, AppError> {
    plugin_enabled(Plugin::Alarming)?;
    scoped("list_alarms", |s, c| async move {
        fetch::alarms(&c, &s.project_id).await
    })
    .await
}

/// Alarm detail page (`alarming` plugin): the alarm and its history.
#[server(input = GetUrl)]
pub async fn get_alarm(id: String) -> Result<Timed<AlarmDetail>, AppError> {
    plugin_enabled(Plugin::Alarming)?;
    scoped_id("get_alarm", id, |_, c, id| async move {
        fetch::alarm(&c, &id).await
    })
    .await
}

/// Plugin panels are opt-in (`[plugins] enabled`): refuse their calls otherwise,
/// so hiding the menu entry is not the only gate.
#[cfg(feature = "ssr")]
fn plugin_enabled(p: Plugin) -> Result<(), AppError> {
    if cfg().plugins.contains(&p) {
        Ok(())
    } else {
        Err(AppError::new(ErrorKind::ServiceUnavailable, format!("the {} plugin is not enabled ([plugins] enabled in moonlight.conf)", p.name())))
    }
}

/// Project › Network › Load Balancers (`load-balancer` plugin).
#[server(input = GetUrl)]
pub async fn list_load_balancers() -> Result<Timed<Vec<LoadBalancerRow>>, AppError> {
    plugin_enabled(Plugin::LoadBalancer)?;
    scoped("list_load_balancers", |s, c| async move {
        fetch::load_balancers(&c, &s.project_id).await
    })
    .await
}

/// Load balancer detail page (`load-balancer` plugin).
#[server(input = GetUrl)]
pub async fn get_load_balancer(id: String) -> Result<Timed<LoadBalancerDetail>, AppError> {
    plugin_enabled(Plugin::LoadBalancer)?;
    scoped_id("get_load_balancer", id, |s, c, id| async move {
        fetch::load_balancer_detail(&c, &s.project_id, &id).await
    })
    .await
}

/// Floating IP detail page, with port forwardings when the extension exists.
#[server(input = GetUrl)]
pub async fn get_floating_ip(id: String) -> Result<Timed<FloatingIpDetail>, AppError> {
    scoped_id("get_floating_ip", id, |s, c, id| async move {
        let forwarding = needs::PORT_FORWARDING.met(&os::capabilities(&s, &c).await);
        fetch::floating_ip_detail(&c, &s.project_id, &id, forwarding).await
    })
    .await
}

/// Router detail page.
#[server(input = GetUrl)]
pub async fn get_router(id: String) -> Result<Timed<RouterDetail>, AppError> {
    scoped_id("get_router", id, |s, c, id| async move {
        fetch::router_detail(&c, &s.project_id, &id).await
    })
    .await
}

/// Security group detail page.
#[server(input = GetUrl)]
pub async fn get_security_group(id: String) -> Result<Timed<SecurityGroupDetail>, AppError> {
    scoped_id("get_security_group", id, |s, c, id| async move {
        fetch::security_group_detail(&c, &s.project_id, &id).await
    })
    .await
}

/// Volume detail page.
#[server(input = GetUrl)]
pub async fn get_volume(id: String) -> Result<Timed<VolumeDetail>, AppError> {
    scoped_id("get_volume", id, |_, c, id| async move {
        fetch::volume_detail(&c, &id).await
    })
    .await
}

/// Volume snapshot detail page.
#[server(input = GetUrl)]
pub async fn get_snapshot(id: String) -> Result<Timed<Snapshot>, AppError> {
    scoped_id("get_snapshot", id, |_, c, id| async move {
        fetch::snapshot_detail(&c, &id).await
    })
    .await
}

/// Project › Volumes › Backups (the `backup` plugin, roadmap 5).
#[server(input = GetUrl)]
pub async fn list_backups() -> Result<Timed<Vec<BackupRow>>, AppError> {
    plugin_enabled(Plugin::Backup)?;
    scoped("list_backups", |_, c| async move {
        fetch::backups(&c).await
    })
    .await
}

/// Backup detail page.
#[server(input = GetUrl)]
pub async fn get_backup(id: String) -> Result<Timed<BackupRow>, AppError> {
    plugin_enabled(Plugin::Backup)?;
    scoped_id("get_backup", id, |_, c, id| async move {
        fetch::backup(&c, &id).await
    })
    .await
}

/// Project › Volumes › Groups (generic volume groups, roadmap 5).
#[server(input = GetUrl)]
pub async fn list_volume_groups() -> Result<Timed<Vec<VolumeGroupRow>>, AppError> {
    scoped("list_volume_groups", |_, c| async move {
        fetch::volume_groups(&c).await
    })
    .await
}

/// Volume group detail page: the group, its volumes and its snapshots.
#[server(input = GetUrl)]
pub async fn get_volume_group(id: String) -> Result<Timed<VolumeGroupDetail>, AppError> {
    scoped_id("get_volume_group", id, |_, c, id| async move {
        fetch::volume_group(&c, &id).await
    })
    .await
}

/// Project › Volumes › Group Snapshots (roadmap 5).
#[server(input = GetUrl)]
pub async fn list_group_snapshots() -> Result<Timed<Vec<GroupSnapshotRow>>, AppError> {
    scoped("list_group_snapshots", |_, c| async move {
        fetch::group_snapshots(&c).await
    })
    .await
}

/// Group snapshot detail page: the snapshot and its volume snapshots.
#[server(input = GetUrl)]
pub async fn get_group_snapshot(id: String) -> Result<Timed<GroupSnapshotDetail>, AppError> {
    scoped_id("get_group_snapshot", id, |_, c, id| async move {
        fetch::group_snapshot(&c, &id).await
    })
    .await
}

/// Image detail page.
#[server(input = GetUrl)]
pub async fn get_image(id: String) -> Result<Timed<ImageDetail>, AppError> {
    scoped_id("get_image", id, |s, c, id| async move {
        let mine = os::projects(&s).await.unwrap_or_default();
        fetch::image_detail(&c, &s.project_id, &mine, &id).await
    })
    .await
}

/// Whether the scoped token has the `admin` role: Horizon's split between the
/// domain-wide Identity lists and the user's own (roadmap 57).
#[cfg(feature = "ssr")]
async fn is_admin(s: &session::Session, c: &openstack_sdk::AsyncOpenStack) -> bool {
    os::capabilities(s, c).await.roles.iter().any(|r| r == "admin")
}

/// The Admin dashboard's gate (roadmap 85): the scoped token must carry the
/// `admin` role, read from the token itself (`needs::ADMIN` only hides the
/// menu, and capability detection fails open). Before any OpenStack call.
#[cfg(feature = "ssr")]
fn require_admin(c: &openstack_sdk::AsyncOpenStack) -> Result<(), AppError> {
    if os::token_is_admin(c) {
        Ok(())
    } else {
        Err(AppError::new(ErrorKind::Forbidden, "the Admin dashboard needs the admin role"))
    }
}

/// Admin › System › System Information: catalog services, compute and block
/// storage services, network agents. Admins only.
#[server(input = GetUrl)]
pub async fn get_system_info() -> Result<Timed<SystemInfo>, AppError> {
    scoped("get_system_info", |s, c| async move {
        require_admin(&c)?;
        let caps = os::capabilities(&s, &c).await;
        fetch::system_info(&c, &caps).await
    })
    .await
}

/// Admin › Overview: every project's usage in the period. Empty dates = this
/// month up to today, as Project › Overview. Admins only.
#[server(input = GetUrl)]
pub async fn get_admin_usage(start: String, end: String) -> Result<Timed<GlobalUsage>, AppError> {
    scoped("get_admin_usage", |_, c| async move {
        require_admin(&c)?;
        let (from, to) = usage_range(&start, &end, chrono::Utc::now().date_naive()).map_err(AppError::invalid)?;
        let stop = to.succ_opt().unwrap_or(to);
        let t = fetch::admin_usage(&c, &format!("{from}T00:00:00"), &format!("{stop}T00:00:00")).await?;
        Ok(Timed { data: GlobalUsage { start: from.to_string(), end: to.to_string(), projects: t.data }, millis: t.millis, calls: t.calls })
    })
    .await
}

/// Admin › System › Defaults: the default quotas per service. Admins only.
#[server(input = GetUrl)]
pub async fn get_default_quotas() -> Result<Timed<Vec<QuotaCard>>, AppError> {
    scoped("get_default_quotas", |s, c| async move {
        require_admin(&c)?;
        let caps = os::capabilities(&s, &c).await;
        fetch::default_quotas(&c, &s.project_id, &caps).await
    })
    .await
}

/// Admin › System › Metadata Definitions: Glance's namespaces. Admins only.
#[server(input = GetUrl)]
pub async fn list_metadef_namespaces() -> Result<Timed<Vec<MetadefNamespace>>, AppError> {
    scoped("list_metadef_namespaces", |_s, c| async move {
        require_admin(&c)?;
        fetch::metadef_namespaces(&c).await
    })
    .await
}

/// *Update Metadata*'s suggestions (roadmap 25): the Glance metadata catalog's properties for
/// `resource_type` (one of [`METADATA_RESOURCE_TYPES`]). Read-only; any project member.
#[server(input = GetUrl)]
pub async fn list_metadata_options(resource_type: String) -> Result<Timed<Vec<MetadataOption>>, AppError> {
    if !METADATA_RESOURCE_TYPES.contains(&resource_type.as_str()) {
        return Err(AppError::invalid("unknown resource type"));
    }
    scoped("list_metadata_options", |_s, c| async move {
        let mut out = fetch::metadata_options(&c, &resource_type).await?;
        // The image editor never offers what it would refuse to send.
        if resource_type == "OS::Glance::Image" {
            let reserved = &cfg().prefs.images.reserved_properties;
            out.data.retain(|o| !is_reserved_image_key(&o.key, reserved));
        }
        Ok(out)
    })
    .await
}

/// Admin › Compute › Hypervisors, with the *Compute Host* tab. Admins only.
#[server(input = GetUrl)]
pub async fn get_admin_hypervisors() -> Result<Timed<Hypervisors>, AppError> {
    scoped("get_admin_hypervisors", |s, c| async move {
        require_admin(&c)?;
        let placement = Requires::Service("placement").met(&os::capabilities(&s, &c).await);
        fetch::admin_hypervisors(&c, placement).await
    })
    .await
}

/// Admin › Hypervisors' *Resource Provider* tab, loaded when the tab opens
/// (Placement, two calls per provider). Admins only.
#[server(input = GetUrl)]
pub async fn get_admin_resource_providers() -> Result<Timed<Vec<ResourceProviderRow>>, AppError> {
    scoped("get_admin_resource_providers", |_s, c| async move {
        require_admin(&c)?;
        fetch::admin_resource_providers(&c).await
    })
    .await
}

/// Admin › Compute › Host Aggregates, with the availability zones. Admins only.
#[server(input = GetUrl)]
pub async fn get_admin_aggregates() -> Result<Timed<Aggregates>, AppError> {
    scoped("get_admin_aggregates", |_s, c| async move {
        require_admin(&c)?;
        fetch::admin_aggregates(&c).await
    })
    .await
}

/// A whole list as a [`Page`] that isn't paged (`[ui] page_size = 0`): the page
/// then shows its client-side table and search box, as before roadmap 88.
#[cfg(feature = "ssr")]
fn whole<T>(all: Timed<Vec<T>>) -> Timed<Page<T>> {
    Timed { millis: all.millis, calls: all.calls, data: Page { rows: all.data, next: None, count: None, paged: false, page_size: 0 } }
}

/// Admin › Compute › Instances: every project's servers, one page of
/// `[ui] page_size` filtered and sorted by `q` (roadmap 88); all of them when
/// paging is off. Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_instances(q: ListQuery) -> Result<Timed<Page<Owned<Instance>>>, AppError> {
    scoped("list_admin_instances", |_s, c| async move {
        require_admin(&c)?;
        match cfg().prefs.page_size_for(true) {
            Some(size) => fetch::admin_instances_page(&c, &q, size).await,
            // `page_size = 0`: every row, as before paging (the page keeps its search box).
            None => Ok(whole(fetch::admin_instances(&c).await?)),
        }
    })
    .await
}

/// The images of a paged Admin › Instances page's boot-from-volume rows, as
/// `(server ID, image name)`, from the volumes in their `boot_volume_ids`
/// (roadmap 88). Called after the table shows; at most
/// `fetch::MAX_BOOT_VOLUMES` IDs. Admins only.
#[server(input = GetUrl)]
pub async fn get_admin_boot_images(volume_ids: Vec<String>) -> Result<Timed<Vec<(String, String)>>, AppError> {
    scoped("get_admin_boot_images", |_s, c| async move {
        require_admin(&c)?;
        let mut ids = volume_ids;
        ids.sort();
        ids.dedup();
        fetch::admin_boot_images(&c, ids).await
    })
    .await
}

/// Admin › Compute › Flavors: public and private. Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_flavors() -> Result<Timed<Vec<FlavorRow>>, AppError> {
    scoped("list_admin_flavors", |_s, c| async move {
        require_admin(&c)?;
        fetch::admin_flavors(&c).await
    })
    .await
}

/// Admin › Compute › Images: every project's images. Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_images() -> Result<Timed<Vec<Owned<Image>>>, AppError> {
    scoped("list_admin_images", |_s, c| async move {
        require_admin(&c)?;
        fetch::admin_images(&c).await
    })
    .await
}

/// Admin › Volume › Volumes: every project's volumes, one page of `[ui] page_size`
/// filtered and sorted by `q` (roadmap 88), with the total at volume 3.45; all of
/// them when paging is off. Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_volumes(q: ListQuery) -> Result<Timed<Page<Owned<Volume>>>, AppError> {
    scoped("list_admin_volumes", |s, c| async move {
        require_admin(&c)?;
        match cfg().prefs.page_size_for(true) {
            Some(size) => {
                let mv = os::capabilities(&s, &c).await.volume_mv;
                fetch::admin_volumes_page(&c, &q, size, mv).await
            }
            None => Ok(whole(fetch::admin_volumes(&c).await?)),
        }
    })
    .await
}

/// Admin › Volume › Snapshots: every project's volume snapshots, one page as
/// [`list_admin_volumes`]. Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_snapshots(q: ListQuery) -> Result<Timed<Page<Owned<Snapshot>>>, AppError> {
    scoped("list_admin_snapshots", |s, c| async move {
        require_admin(&c)?;
        match cfg().prefs.page_size_for(true) {
            Some(size) => {
                let mv = os::capabilities(&s, &c).await.volume_mv;
                fetch::admin_snapshots_page(&c, &q, size, mv).await
            }
            None => Ok(whole(fetch::admin_snapshots(&c).await?)),
        }
    })
    .await
}

/// The names of servers or volumes a paged Admin page points at (a volume's
/// attachments, a floating IP's instance, a snapshot's volume and host), read after
/// the table shows (roadmap 88): at most `fetch::MAX_LATE_NAMES` IDs, sent in
/// chunks of [`BOOT_IMAGE_CHUNK`]. A resource that can't be read is left out.
/// Admins only.
#[server(input = GetUrl)]
pub async fn get_admin_names(kind: NameKind, ids: Vec<String>) -> Result<Timed<Vec<LateName>>, AppError> {
    scoped("get_admin_names", |_s, c| async move {
        require_admin(&c)?;
        let mut ids = ids;
        ids.sort();
        ids.dedup();
        fetch::admin_names(&c, kind, ids).await
    })
    .await
}

/// Admin › Volume › Volume Types, with the QoS specs. Admins only.
#[server(input = GetUrl)]
pub async fn get_admin_volume_types() -> Result<Timed<VolumeTypes>, AppError> {
    scoped("get_admin_volume_types", |_s, c| async move {
        require_admin(&c)?;
        fetch::admin_volume_types(&c).await
    })
    .await
}

/// Admin › Volume › Groups: every project's volume groups. Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_volume_groups() -> Result<Timed<Vec<Owned<VolumeGroupRow>>>, AppError> {
    scoped("list_admin_volume_groups", |_s, c| async move {
        require_admin(&c)?;
        fetch::admin_volume_groups(&c).await
    })
    .await
}

/// Admin › Volume › Group Snapshots: every project's. Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_group_snapshots() -> Result<Timed<Vec<Owned<GroupSnapshotRow>>>, AppError> {
    scoped("list_admin_group_snapshots", |_s, c| async move {
        require_admin(&c)?;
        fetch::admin_group_snapshots(&c).await
    })
    .await
}

/// Admin › Volume › Group Types. Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_group_types() -> Result<Timed<Vec<GroupTypeRow>>, AppError> {
    scoped("list_admin_group_types", |_s, c| async move {
        require_admin(&c)?;
        fetch::admin_group_types(&c).await
    })
    .await
}

/// Admin › Network › Networks: every network. Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_networks() -> Result<Timed<Vec<Owned<NetworkRow>>>, AppError> {
    scoped("list_admin_networks", |s, c| async move {
        require_admin(&c)?;
        let dhcp = needs::DHCP_AGENTS.met(&os::capabilities(&s, &c).await);
        fetch::admin_networks(&c, dhcp).await
    })
    .await
}

/// Admin › Network › Routers: every project's routers. Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_routers() -> Result<Timed<Vec<Owned<RouterRow>>>, AppError> {
    scoped("list_admin_routers", |_s, c| async move {
        require_admin(&c)?;
        fetch::admin_routers(&c).await
    })
    .await
}

/// Admin › Network › Floating IPs: every project's, one page of `[ui] page_size`
/// filtered and sorted by `q` (roadmap 88); all of them when paging is off.
/// Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_floating_ips(q: ListQuery) -> Result<Timed<Page<Owned<FloatingIpRow>>>, AppError> {
    scoped("list_admin_floating_ips", |_s, c| async move {
        require_admin(&c)?;
        match cfg().prefs.page_size_for(true) {
            Some(size) => fetch::admin_floating_ips_page(&c, &q, size).await,
            None => Ok(whole(fetch::admin_floating_ips(&c).await?)),
        }
    })
    .await
}

/// Admin › Network › Trunks: every project's (`trunk` plugin). Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_trunks() -> Result<Timed<Vec<Owned<TrunkRow>>>, AppError> {
    plugin_enabled(Plugin::Trunk)?;
    scoped("list_admin_trunks", |_s, c| async move {
        require_admin(&c)?;
        fetch::admin_trunks(&c).await
    })
    .await
}

/// Admin › Network › RBAC Policies. Admins only.
#[server(input = GetUrl)]
pub async fn list_admin_rbac_policies() -> Result<Timed<Vec<RbacPolicyRow>>, AppError> {
    scoped("list_admin_rbac_policies", |_s, c| async move {
        require_admin(&c)?;
        fetch::admin_rbac_policies(&c).await
    })
    .await
}

/// Identity › Projects: the domain's projects for admins, the user's own otherwise.
#[server(input = GetUrl)]
pub async fn list_identity_projects() -> Result<Timed<Vec<IdentityProject>>, AppError> {
    scoped("list_identity_projects", |s, c| async move {
        let admin = is_admin(&s, &c).await;
        fetch::identity_projects(&c, &s, admin).await
    })
    .await
}

/// Identity › Users: the domain's users for admins, only the user otherwise.
#[server(input = GetUrl)]
pub async fn list_identity_users() -> Result<Timed<Vec<IdentityUser>>, AppError> {
    scoped("list_identity_users", |s, c| async move {
        let admin = is_admin(&s, &c).await;
        fetch::identity_users(&c, &s, admin).await
    })
    .await
}

/// Identity › Projects › a project: overview, and members for admins.
#[server(input = GetUrl)]
pub async fn get_identity_project(id: String) -> Result<Timed<IdentityProjectDetail>, AppError> {
    scoped_id("get_identity_project", id, |s, c, id| async move {
        let admin = is_admin(&s, &c).await;
        fetch::identity_project(&c, &s, admin, &id).await
    })
    .await
}

/// Identity › Groups › a group and its members (admins).
#[server(input = GetUrl)]
pub async fn get_identity_group(id: String) -> Result<Timed<IdentityGroupDetail>, AppError> {
    scoped_id("get_identity_group", id, |s, c, id| async move {
        fetch::identity_group(&c, &s, &id).await
    })
    .await
}

/// Identity › Application Credentials › one of the user's.
#[server(input = GetUrl)]
pub async fn get_app_credential(id: String) -> Result<Timed<AppCredentialDetail>, AppError> {
    scoped_id("get_app_credential", id, |s, c, id| async move {
        fetch::app_credential(&c, &s, &id).await
    })
    .await
}

/// Identity › Users › a user: Horizon's overview.
#[server(input = GetUrl)]
pub async fn get_identity_user(id: String) -> Result<Timed<IdentityUserDetail>, AppError> {
    scoped_id("get_identity_user", id, |s, c, id| async move {
        let mine = os::projects(&s).await.unwrap_or_default();
        let admin = is_admin(&s, &c).await;
        fetch::identity_user(&c, &s, &mine, admin, &id).await
    })
    .await
}

/// Identity › Domains (multi-domain clouds, admins): refused unless
/// `[keystone] multidomain` is on.
#[server(input = GetUrl)]
pub async fn list_identity_domains() -> Result<Timed<Vec<IdentityDomain>>, AppError> {
    if !cfg().multidomain {
        return Err(AppError::invalid("[keystone] multidomain is off"));
    }
    scoped("list_identity_domains", |s, c| async move {
        fetch::identity_domain_list(&c, &s).await
    })
    .await
}

/// *Set Domain Context* (`Some(id)`) or *Clear Domain Context* (`None`): stored
/// in the session cookie; it scopes the identity lists, like Horizon's.
#[server]
pub async fn set_domain_context(id: Option<String>) -> Result<(), AppError> {
    if !cfg().multidomain {
        return Err(AppError::invalid("[keystone] multidomain is off"));
    }
    let mut s = session::require()?;
    s.domain_context = match id {
        None => None,
        Some(id) => {
            // Pasted into the request path unencoded (roadmap 48).
            check_id(&id)?;
            let c = os::scoped(&s).await?;
            Some(fetch::identity_domain_ref(&c, &id).await?)
        }
    };
    tracing::info!(user = %s.user, client = %session::client_ip(), context = ?s.domain_context.as_ref().map(|d| &d.1), "domain context");
    session::store(&s);
    Ok(())
}

/// Identity › Groups (admins; Keystone answers others with 403).
#[server(input = GetUrl)]
pub async fn list_identity_groups() -> Result<Timed<Vec<IdentityGroup>>, AppError> {
    scoped("list_identity_groups", |s, c| async move {
        fetch::identity_groups(&c, &s).await
    })
    .await
}

/// Identity › Roles (admins).
#[server(input = GetUrl)]
pub async fn list_identity_roles() -> Result<Timed<Vec<IdentityRole>>, AppError> {
    scoped("list_identity_roles", |_, c| async move {
        fetch::identity_roles(&c).await
    })
    .await
}

/// Identity › Application Credentials: the user's own.
#[server(input = GetUrl)]
pub async fn list_app_credentials() -> Result<Timed<Vec<AppCredential>>, AppError> {
    scoped("list_app_credentials", |s, c| async move {
        fetch::app_credentials(&c, &s).await
    })
    .await
}

/// Server group detail page: policy, rules and members (roadmap 52).
#[server(input = GetUrl)]
pub async fn get_server_group(id: String) -> Result<Timed<ServerGroupDetail>, AppError> {
    scoped_id("get_server_group", id, |_, c, id| async move {
        fetch::server_group_detail(&c, &id).await
    })
    .await
}

/// Key pair detail page (key pairs are named, not numbered).
#[server(input = GetUrl)]
pub async fn get_keypair(name: String) -> Result<Timed<KeyPairDetail>, AppError> {
    scoped_id("get_keypair", name, |_, c, name| async move {
        fetch::keypair_detail(&c, &name).await
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::login_domain;
    use crate::prefs::LoginPrefs;

    #[test]
    fn login_domain_follows_the_login_settings() {
        let free = LoginPrefs::default();
        assert_eq!(login_domain(&free, " Corp ").unwrap(), "Corp");
        assert_eq!(login_domain(&free, "").unwrap(), "Default", "empty: the default domain");
        let hidden = LoginPrefs { domain_hidden: true, ..Default::default() };
        assert_eq!(login_domain(&hidden, "Other").unwrap(), "Default", "hidden: always the default");
        let choices = LoginPrefs { domain_choices: vec!["Default".into(), "Corp".into()], ..Default::default() };
        assert_eq!(login_domain(&choices, "Corp").unwrap(), "Corp");
        assert!(login_domain(&choices, "Evil").is_err(), "only the offered domains");
    }
}
