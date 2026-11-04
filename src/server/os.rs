//! OpenStack connections through gtema's `openstack_sdk`.
//!
//! * Login: `v3password`, unscoped → we keep only the unscoped token.
//! * Every request: `v3token` rescoped to the session's project and region.
//!   The resulting connection is cached in-process for a few minutes.
//!
//! The on-disk auth cache of the SDK is always disabled: this is a multi-user
//! server and tokens must never be written to disk.

use std::collections::BTreeSet;
use std::time::Duration;

use chrono::{DateTime, Utc};
use openstack_sdk::api::QueryAsync;
use openstack_sdk::config::{Auth, CloudConfig};
use openstack_sdk::AsyncOpenStack;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

use super::config::state;
use super::session::Session;
use super::sfe;
use crate::model::{parse_microversion, AppError, Capabilities, ErrorKind, ProjectRef};

pub struct LoginResult {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub user_id: String,
    pub user: String,
    /// The user's domain name (else ID) from the token; federated users get
    /// their identity provider's domain (Keystone ≥ Ocata).
    pub user_domain: Option<String>,
    pub projects: Vec<ProjectRef>,
    pub password_expires_at: Option<DateTime<Utc>>,
}

/// Keystone timestamps: RFC 3339, or naive UTC with microseconds (`2026-11-01T10:00:00.000000`).
pub fn keystone_time(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .ok()
        .or_else(|| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f").ok().map(|n| n.and_utc()))
}

fn base_config(auth_type: &str, auth: Auth, region: Option<String>) -> CloudConfig {
    config_for(&state().config, auth_type, auth, region)
}

/// The SDK configuration for one connection: interface and TLS from `moonlight.conf`.
fn config_for(cfg: &super::config::Config, auth_type: &str, auth: Auth, region: Option<String>) -> CloudConfig {
    CloudConfig {
        auth: Some(auth),
        auth_type: Some(auth_type.into()),
        region_name: region,
        interface: Some(cfg.interface.clone()),
        cacert: cfg.ca_file.clone(),
        verify: Some(cfg.verify_tls),
        name: Some("moonlight".into()),
        auth_cache: Some(false),
        ..Default::default()
    }
}

async fn connect(cfg: &CloudConfig) -> Result<AsyncOpenStack, Box<openstack_sdk::OpenStackError>> {
    AsyncOpenStack::builder(cfg)
        .disable_auth_cache(true)
        .renew_auth(false)
        .connect()
        .await
        .map_err(Box::new)
}

/// Cache key of everything tied to one Keystone token (`projects`, `revoked`):
/// a hash, so the token itself is never a key.
pub fn token_key(token: &str) -> String {
    hash_key(&[token])
}

/// Cache key of a project-scoped connection and what is cached with it
/// (`connections`, `capabilities`, `lookups`).
pub fn scoped_key(token: &str, project_id: &str, region: Option<&str>) -> String {
    hash_key(&[token, project_id, region.unwrap_or("")])
}

/// After logout (roadmap 49): refuse the session's token from now on (see
/// `session::current`) and drop what the caches hold for it. Connections for
/// other projects/regions can't be found by token (their keys are hashes);
/// they become unreachable because `revoked` stops every request first.
pub async fn forget(session: &Session) {
    let st = state();
    let key = token_key(&session.token);
    st.revoked.insert(key.clone(), ()).await;
    st.projects.invalidate(&key).await;
    let scoped = scoped_key(&session.token, &session.project_id, session.region.as_deref());
    st.connections.invalidate(&scoped).await;
    st.capabilities.invalidate(&scoped).await;
    super::fetch::forget_lookups(&scoped).await;
}

/// SHA-256 of `parts`, each followed by a NUL so `["ab", "c"]` and `["a", "bc"]`
/// differ: cache and counter keys that never contain the secret or name itself.
pub fn hash_key(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
        h.update([0u8]);
    }
    hex::encode(h.finalize())
}

/// Username/password login against Keystone. Returns an unscoped token and the
/// projects the user can access.
pub async fn login(username: &str, password: &str, domain: &str) -> Result<LoginResult, AppError> {
    let st = state();
    let auth = Auth {
        auth_url: Some(st.config.keystone_url.clone()),
        username: Some(username.into()),
        password: Some(SecretString::from(password.to_string())),
        user_domain_name: Some(domain.into()),
        ..Default::default()
    };
    let os = within(REQUEST_TIMEOUT, "keystone login", connect(&base_config("v3password", auth, None)))
        .await?
        .map_err(sfe("keystone login"))?;
    login_result(&os).await
}

/// WebSSO (roadmap 80): signs in with the unscoped token Keystone posted back
/// after federation, as Horizon does. Authenticating with it (`v3token`,
/// unscoped) is what validates it; Keystone answers with a new unscoped token
/// for the same user, which the session keeps.
pub async fn login_with_token(token: &str) -> Result<LoginResult, AppError> {
    let st = state();
    let auth = Auth {
        auth_url: Some(st.config.keystone_url.clone()),
        token: Some(SecretString::from(token.to_string())),
        ..Default::default()
    };
    let os = within(REQUEST_TIMEOUT, "keystone websso token", connect(&base_config("v3token", auth, None)))
        .await?
        .map_err(sfe("keystone websso token"))?;
    login_result(&os).await
}

/// The token, user and projects of a fresh unscoped connection.
async fn login_result(os: &AsyncOpenStack) -> Result<LoginResult, AppError> {
    let st = state();
    let token = os
        .get_auth_token()
        .ok_or_else(|| AppError::internal("keystone returned no token"))?
        .expose_secret()
        .to_string();
    let info = os
        .get_auth_info()
        .ok_or_else(|| AppError::internal("keystone returned no token info"))?;
    let projects = list_projects_with(os).await?;
    st.projects.insert(token_key(&token), projects.clone()).await;
    Ok(LoginResult {
        token,
        expires_at: info.token.expires_at,
        user_id: info.token.user.id.clone(),
        user: info.token.user.name.clone(),
        // The name, else the ID: the Identity panels pair it with `users/{me}`'s
        // `domain_id`, so a placeholder like `Federated` would match nothing.
        user_domain: info.token.user.domain.as_ref().and_then(|d| d.name.clone().or_else(|| d.id.clone())),
        projects,
        password_expires_at: info.token.user.password_expires_at.as_deref().and_then(keystone_time),
    })
}

/// The session for a sign-in: scoped to the first project, in the default
/// region when the catalog has it, else the first. `Forbidden` without projects.
pub async fn new_session(res: LoginResult, domain: String) -> Result<Session, AppError> {
    let project = res.projects.first().cloned().ok_or_else(|| AppError::new(ErrorKind::Forbidden, "no projects"))?;
    let mut sess = Session {
        token: res.token,
        expires_at: res.expires_at,
        user_id: res.user_id,
        user: res.user,
        domain,
        project_id: project.id,
        project_name: project.name,
        region: state().config.default_region.clone(),
        logged_in_at: Utc::now(),
        password_expires_at: res.password_expires_at,
        domain_context: None,
        websso: None,
    };
    // Validate the region against the catalog; fall back to the first one.
    let regions = regions(&scoped(&sess).await?);
    if let Some(r) = fallback_region(sess.region.as_deref(), &regions) {
        sess.region = Some(r.clone());
    }
    Ok(sess)
}

async fn list_projects_with(os: &AsyncOpenStack) -> Result<Vec<ProjectRef>, AppError> {
    use openstack_sdk::api::identity::v3::auth::project::list;
    let ep = list::Request::builder().build().map_err(sfe("build auth/projects"))?;
    let raw: Vec<serde_json::Value> = ep.query_async(os).await.map_err(sfe("GET auth/projects"))?;
    Ok(super::mapping::projects(&raw))
}

/// Projects visible to the session's unscoped token (cached 5 min).
pub async fn projects(session: &Session) -> Result<Vec<ProjectRef>, AppError> {
    let st = state();
    let key = token_key(&session.token);
    if let Some(p) = st.projects.get(&key).await {
        return Ok(p);
    }
    let auth = Auth {
        auth_url: Some(st.config.keystone_url.clone()),
        token: Some(SecretString::from(session.token.clone())),
        ..Default::default()
    };
    let os = within(REQUEST_TIMEOUT, "keystone token", connect(&base_config("v3token", auth, None)))
        .await?
        .map_err(sfe("keystone token"))?;
    let p = within(REQUEST_TIMEOUT, "keystone projects", list_projects_with(&os)).await??;
    st.projects.insert(key, p.clone()).await;
    Ok(p)
}

/// A connection scoped to the session's project and region.
pub async fn scoped(session: &Session) -> Result<AsyncOpenStack, AppError> {
    scoped_to(session, &session.project_id, session.region.as_deref()).await
}

pub async fn scoped_to(
    session: &Session,
    project_id: &str,
    region: Option<&str>,
) -> Result<AsyncOpenStack, AppError> {
    let st = state();
    let key = scoped_key(&session.token, project_id, region);
    if let Some(c) = st.connections.get(&key).await {
        tracing::debug!(user = %session.user, websso = session.websso.is_some(), "scoped connection: cached");
        return Ok(c);
    }
    // Debug timing of a cache miss (roadmap 83): rescope and discovery are the
    // expensive part of a request that has to build its connection.
    let started = std::time::Instant::now();
    let auth = Auth {
        auth_url: Some(st.config.keystone_url.clone()),
        token: Some(SecretString::from(session.token.clone())),
        project_id: Some(project_id.to_string()),
        ..Default::default()
    };
    let cfg = base_config("v3token", auth, region.map(str::to_string));
    let os = within(REQUEST_TIMEOUT, "keystone (scoped token)", connect(&cfg))
        .await?
        .map_err(sfe("scoped connection"))?;
    let rescoped = started.elapsed();
    // Only cache a fully discovered connection: if a region is down, the next
    // request retries discovery and recovers as soon as the region is back.
    let discovered = discover(&os).await;
    if discovered {
        st.connections.insert(key, os.clone()).await;
    }
    tracing::debug!(
        user = %session.user,
        websso = session.websso.is_some(),
        rescope_ms = rescoped.as_millis() as u64,
        discovery_ms = (started.elapsed() - rescoped).as_millis() as u64,
        cached = discovered,
        "scoped connection: built"
    );
    Ok(os)
}

/// Upper bound for one server function (Keystone + every OpenStack call it
/// makes). The SDK's HTTP client has no timeout of its own: without this, an
/// unreachable region keeps the page loading until the OS gives up (~75 s).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Upper bound for the version discovery of one service. Short, so that the
/// header (regions, projects) still loads when a region is down.
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);

/// Runs `fut` with a deadline; on expiry, a `Timeout` error naming `what`.
///
/// The future is boxed: server fns wrap their whole body (`tokio::join!` of many
/// SDK calls), and kept inline that overflows a worker's 2 MiB stack in debug builds.
pub async fn within<T>(limit: Duration, what: &str, fut: impl Future<Output = T>) -> Result<T, AppError> {
    tokio::time::timeout(limit, Box::pin(fut)).await.map_err(|_| {
        tracing::warn!("{what}: no answer within {} s", limit.as_secs());
        AppError::new(ErrorKind::Timeout, format!("{what}: no answer within {} s", limit.as_secs()))
    })
}

/// Version discovery for the services Moonlight uses. Catalogs often list
/// unversioned endpoints (`https://neutron…/`, `https://glance…/`); without
/// discovery the SDK would call `/networks` instead of `/v2.0/networks`.
/// Runs in parallel, each bounded by `DISCOVERY_TIMEOUT`; failures are only
/// logged. Returns whether every service was discovered.
async fn discover(os: &AsyncOpenStack) -> bool {
    use openstack_sdk::types::ServiceType as S;
    let one = |name: &'static str, ty: S| async move {
        match within(DISCOVERY_TIMEOUT, name, os.discover_service_endpoint(&ty)).await {
            Ok(Ok(())) => true,
            Ok(Err(e)) => {
                tracing::warn!("discovery of {name} failed: {e}");
                false
            }
            Err(_) => false,
        }
    };
    // Optional services (skipped by the SDK when the catalog lacks them) do not
    // decide caching: a broken Octavia must not slow every other page down.
    let (c, n, b, i, _lb) = tokio::join!(
        one("compute", S::Compute),
        one("network", S::Network),
        one("block-storage", S::BlockStorage),
        one("image", S::Image),
        one("load-balancer", S::LoadBalancer),
    );
    c && n && b && i
}

/// What the session's project and region offer (roadmap 19): catalog services,
/// Neutron extensions and the highest compute/volume microversions. GET only,
/// cached like the connection. Nothing is cached when detection fails, and the
/// result then has `known: false`, so the UI hides nothing.
pub async fn capabilities(session: &Session, os: &AsyncOpenStack) -> Capabilities {
    use openstack_sdk::api::AsyncClient;
    use openstack_sdk::types::ServiceType as S;
    let st = state();
    let key = scoped_key(&session.token, &session.project_id, session.region.as_deref());
    if let Some(c) = st.capabilities.get(&key).await {
        return c;
    }
    let services: Vec<String> = catalog(os, session.region.as_deref()).into_iter().map(|e| e.service_type).collect();
    let has_network = services.iter().any(|s| s == "network");
    let max_mv = |ty: S| async move {
        let ep = within(DISCOVERY_TIMEOUT, "version range", os.get_service_endpoint(&ty, None)).await.ok()?.ok()?;
        // Nova and Cinder put their highest microversion in the version
        // document's `version` ("2.95"), which the SDK keeps as the endpoint's
        // version; `max_version` is only set by some deployments.
        let v = ep.version();
        ep.max_version().as_deref().and_then(parse_microversion).or((v.minor > 0).then_some((v.major as u16, v.minor as u16)))
    };
    let extensions = async {
        if !has_network {
            return Some(Vec::new());
        }
        use openstack_sdk::api::network::v2::extension::list as m;
        let ep = m::Request::builder().build().ok()?;
        let v: Result<Vec<serde_json::Value>, _> = within(DISCOVERY_TIMEOUT, "network extensions", ep.query_async(os)).await.ok()?;
        match v {
            Ok(list) => Some(list.iter().filter_map(|e| e.get("alias").and_then(|a| a.as_str()).map(str::to_string)).collect()),
            Err(e) => {
                tracing::warn!("network extensions unavailable: {e}");
                None
            }
        }
    };
    // Octavia has no microversions: discovery picks its newest version (`v2.27`),
    // which reads the same way (roadmap 54).
    let has_lb = services.iter().any(|s| s == "load-balancer" || s == "octavia");
    let lb_mv = async {
        if has_lb { max_mv(S::LoadBalancer).await } else { None }
    };
    // Glance's enabled import methods; `None` (fail open) when the image service is missing
    // or the call fails, so the *URL* source is never hidden by a detection hiccup.
    let has_image = services.iter().any(|s| s == "image");
    let image_import = async {
        if !has_image {
            return None;
        }
        use openstack_sdk::api::image::v2::info::import::get as m;
        let ep = m::Request::builder().build().ok()?;
        let v: Result<serde_json::Value, _> = within(DISCOVERY_TIMEOUT, "image import methods", ep.query_async(os)).await.ok()?;
        match v {
            Ok(v) => Some(import_methods(&v)),
            Err(e) => {
                tracing::warn!("image import methods unavailable: {e}");
                None
            }
        }
    };
    let (compute_mv, volume_mv, lb_mv, extensions, image_import) = tokio::join!(max_mv(S::Compute), max_mv(S::BlockStorage), lb_mv, extensions, image_import);
    let Some(network_extensions) = extensions else {
        return Capabilities::default();
    };
    // `plugins` comes from the configuration, added per request (`get_session_info`).
    let caps = Capabilities { known: true, services, network_extensions, compute_mv, volume_mv, lb_mv, plugins: vec![], roles: token_roles(os), multidomain: false, image_import };
    st.capabilities.insert(key, caps.clone()).await;
    caps
}

/// The method names in `GET /v2/info/import`. The SDK unwraps `import-methods`, whose value is
/// `{"description": …, "type": "array", "value": ["web-download", …]}`; a bare array is
/// accepted too.
fn import_methods(v: &serde_json::Value) -> Vec<String> {
    let list = v.get("value").unwrap_or(v);
    list.as_array().into_iter().flatten().filter_map(|m| m.as_str().map(str::to_string)).collect()
}

/// The scoped token's role names as Keystone spells them, sorted (application
/// credential roles are sent by name, roadmap 57). Keystone already adds implied
/// roles (`admin` → `member` → `reader`) to the token.
pub fn token_role_names(os: &AsyncOpenStack) -> Vec<String> {
    let roles = os.get_auth_info().and_then(|i| i.token.roles).unwrap_or_default();
    let mut names: Vec<String> = roles.into_iter().map(|r| r.name).collect();
    names.sort();
    names
}

/// [`token_role_names`], lowercase, for comparisons.
fn token_roles(os: &AsyncOpenStack) -> Vec<String> {
    token_role_names(os).into_iter().map(|r| r.to_lowercase()).collect()
}

/// Whether the scoped token has the `admin` role: Neutron then shows other
/// projects' resources (AGENTS.md §6), which callers filter or, like
/// `fetch::net_lookups`, rely on.
pub fn token_is_admin(os: &AsyncOpenStack) -> bool {
    token_roles(os).iter().any(|r| r == "admin")
}

/// Regions present in the scoped token's catalog, sorted.
pub fn regions(os: &AsyncOpenStack) -> Vec<String> {
    os.get_available_regions()
        .unwrap_or_default()
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The region to switch to when `current` is not in the catalog any more
/// (or none is set): the first available one. `None` = keep `current`.
pub fn fallback_region<'a>(current: Option<&str>, regions: &'a [String]) -> Option<&'a String> {
    match current {
        Some(r) if regions.iter().any(|x| x == r) => None,
        _ => regions.first(),
    }
}

/// Endpoints of the configured interface (`[keystone] interface`) in the
/// current region, for the "API Access" page and service detection.
pub fn catalog(os: &AsyncOpenStack, region: Option<&str>) -> Vec<crate::model::ApiEndpoint> {
    catalog_entries(os.get_token_catalog().unwrap_or_default(), &state().config.interface, region, false)
}

/// Project › API Access: like [`catalog`], plus the services without an
/// endpoint here (empty `url`), which Horizon lists with `-`.
pub fn catalog_listing(os: &AsyncOpenStack, region: Option<&str>) -> Vec<crate::model::ApiEndpoint> {
    catalog_entries(os.get_token_catalog().unwrap_or_default(), &state().config.interface, region, true)
}

/// Admin › System Information › *Services*: the token's catalog, one row per
/// service and region with all its endpoints (none for a service without any),
/// as Horizon builds it (from the catalog, not Keystone's `services` API: no
/// extra call).
pub fn catalog_services(os: &AsyncOpenStack) -> Vec<crate::model::CatalogService> {
    catalog_services_of(os.get_token_catalog().unwrap_or_default())
}

fn catalog_services_of(services: Vec<openstack_sdk::types::identity::v3::ServiceEndpoints>) -> Vec<crate::model::CatalogService> {
    let mut out = Vec::new();
    for svc in services {
        // A service without endpoints (ttcp-dev's `ceilometer`) gets no row, as in Horizon.
        let regions: BTreeSet<String> = svc.endpoints.iter().map(|e| e.region.clone().unwrap_or_else(|| "-".into())).collect();
        for region in regions {
            let mut endpoints: Vec<(String, String)> = svc
                .endpoints
                .iter()
                .filter(|e| e.region.as_deref().unwrap_or("-") == region)
                .map(|e| (e.interface.clone(), e.url.clone()))
                .collect();
            endpoints.sort();
            out.push(crate::model::CatalogService {
                name: Some(svc.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| svc.service_type.clone()),
                service_type: svc.service_type.clone(),
                region,
                endpoints,
            });
        }
    }
    out.sort_by(|a, b| (&a.name, &a.region).cmp(&(&b.name, &b.region)));
    out
}

fn catalog_entries(
    services: Vec<openstack_sdk::types::identity::v3::ServiceEndpoints>,
    interface: &str,
    region: Option<&str>,
    keep_unreachable: bool,
) -> Vec<crate::model::ApiEndpoint> {
    let mut out: Vec<_> = services
        .into_iter()
        .filter_map(|svc| {
            let ep = svc.endpoints.iter().find(|e| {
                e.interface == interface
                    && match (region, e.region.as_deref()) {
                        (Some(want), Some(have)) => want == have,
                        (Some(_), None) => true,
                        (None, _) => true,
                    }
            });
            if ep.is_none() && !keep_unreachable {
                return None;
            }
            Some(crate::model::ApiEndpoint {
                service_type: svc.service_type.clone(),
                name: Some(svc.name.clone()).filter(|n| !n.is_empty()),
                url: ep.map(|e| e.url.clone()).unwrap_or_default(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.service_type.cmp(&b.service_type));
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn import_methods_read_glances_wrapped_list() {
        use serde_json::json;
        let wrapped = json!({"description": "Import methods available.", "type": "array", "value": ["glance-direct", "web-download"]});
        assert_eq!(super::import_methods(&wrapped), ["glance-direct", "web-download"]);
        assert_eq!(super::import_methods(&json!(["web-download"])), ["web-download"], "bare array");
        assert!(super::import_methods(&json!({"value": []})).is_empty(), "import disabled");
    }

    use super::*;

    #[test]
    fn system_information_lists_each_service_per_region_with_every_endpoint() {
        let svc: Vec<openstack_sdk::types::identity::v3::ServiceEndpoints> = serde_json::from_value(serde_json::json!([
            {"type": "load-balancer", "name": "octavia", "endpoints": [
                {"id": "1", "interface": "public", "region": "r1", "url": "https://octavia"},
                {"id": "2", "interface": "internal", "region": "r1", "url": "https://octavia-internal"},
                {"id": "3", "interface": "public", "region": "r2", "url": "https://octavia.r2"}]},
            {"type": "identity", "name": "", "endpoints": [{"id": "4", "interface": "public", "url": "https://keystone"}]},
            {"type": "Ceilometer", "name": "ceilometer", "endpoints": []}
        ]))
        .unwrap();
        let out = catalog_services_of(svc);
        let rows: Vec<_> = out.iter().map(|s| (s.name.as_str(), s.region.as_str(), s.endpoints.len())).collect();
        assert_eq!(rows, [("identity", "-", 1), ("octavia", "r1", 2), ("octavia", "r2", 1)], "unnamed → type; no region → `-`; one row per region; none without endpoints (Horizon)");
        assert_eq!(out[1].endpoints[0], ("internal".to_string(), "https://octavia-internal".to_string()), "sorted by interface");
    }

    #[test]
    fn api_access_also_lists_services_without_an_endpoint_here() {
        let svc: Vec<openstack_sdk::types::identity::v3::ServiceEndpoints> = serde_json::from_value(serde_json::json!([
            {"type": "compute", "name": "nova", "endpoints": [{"id": "1", "interface": "public", "region": "r1", "url": "https://nova"}]},
            {"type": "metering", "name": "ceilometer", "endpoints": [{"id": "2", "interface": "internal", "region": "r1", "url": "https://x"}]}
        ]))
        .unwrap();
        let types = |v: Vec<crate::model::ApiEndpoint>| v.into_iter().map(|e| (e.service_type, e.url)).collect::<Vec<_>>();
        assert_eq!(types(catalog_entries(svc.clone(), "public", Some("r1"), false)), [("compute".into(), "https://nova".into())]);
        assert_eq!(
            types(catalog_entries(svc, "public", Some("r1"), true)),
            [("compute".to_string(), "https://nova".to_string()), ("metering".into(), String::new())],
            "listed with an empty URL (shown as -), never used for detection"
        );
    }

    #[test]
    fn cache_keys_depend_on_every_part_and_hide_the_token() {
        let a = hash_key(&["tok", "p1", "r1"]);
        assert_ne!(a, hash_key(&["tok", "p2", "r1"]));
        assert_ne!(a, hash_key(&["tok", "p1", "r2"]));
        // separators prevent ambiguous concatenation
        assert_ne!(hash_key(&["ab", "c"]), hash_key(&["a", "bc"]));
        assert!(!a.contains("tok"));
        assert_eq!(a.len(), 64);
    }

    #[tokio::test]
    async fn a_hanging_call_becomes_a_timeout_error() {
        let start = std::time::Instant::now();
        let e = within(Duration::from_millis(50), "GET servers", std::future::pending::<()>()).await.unwrap_err();
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(e.kind, ErrorKind::Timeout);
        assert!(e.retryable());
        assert!(e.detail.starts_with("GET servers: no answer within"), "{}", e.detail);
        // A fast call is untouched.
        assert_eq!(within(Duration::from_secs(1), "x", async { 7 }).await.unwrap(), 7);
    }

    #[test]
    fn parses_keystone_timestamps() {
        assert!(keystone_time("2026-11-01T10:00:00.000000").is_some());
        assert!(keystone_time("2026-11-01T10:00:00Z").is_some());
        assert_eq!(keystone_time("never"), None);
    }

    #[test]
    fn region_falls_back_to_the_first_available() {
        let regions = vec!["es-central-1".to_string(), "es-north-1".to_string()];
        assert_eq!(fallback_region(Some("es-north-1"), &regions), None);
        assert_eq!(fallback_region(Some("gone-1"), &regions), Some(&regions[0]));
        assert_eq!(fallback_region(None, &regions), Some(&regions[0]));
        // An empty catalog offers nothing to switch to.
        assert_eq!(fallback_region(Some("gone-1"), &[]), None);
    }

    fn settings(extra: &str) -> super::super::config::Config {
        let text = format!("[keystone]\nurl = \"https://k\"\n{extra}");
        toml::from_str::<crate::server::settings::Settings>(&text).unwrap().config().unwrap()
    }

    #[test]
    fn sdk_config_never_caches_auth_to_disk() {
        let cfg = config_for(&settings(""), "v3token", Auth::default(), Some("r".into()));
        assert_eq!(cfg.auth_cache, Some(false));
        assert_eq!((cfg.interface.as_deref(), cfg.cacert.as_deref(), cfg.verify), (Some("public"), None, Some(true)), "defaults");
    }

    #[test]
    fn sdk_config_follows_interface_and_tls_settings() {
        let cfg = config_for(&settings("interface = \"internal\"\n[tls]\nverify = false\n"), "v3token", Auth::default(), None);
        assert_eq!((cfg.interface.as_deref(), cfg.verify), (Some("internal"), Some(false)));
    }
}
