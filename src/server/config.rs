use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine;
use cookie::Key;
use moka::future::Cache;
use openstack_sdk::AsyncOpenStack;

use crate::model::{ProjectRef, WriteMode};
use crate::server::settings::Settings;

/// Validated settings (see `settings` for where they come from).
#[derive(Clone, Debug)]
pub struct Config {
    pub keystone_url: String,
    pub default_domain: String,
    pub default_region: Option<String>,
    /// `[session] secure_cookies`: `Secure` + `__Host-` cookie, and HSTS.
    pub secure_cookies: bool,
    /// `[session] revoke_on_logout`: Keystone revocation on logout (roadmap 49).
    pub revoke_on_logout: bool,
    /// `[keystone] multidomain`: the Domains panel (roadmap 57).
    pub multidomain: bool,
    /// `[key_manager] allow_payload`: *Show Payload* on secrets (roadmap 55).
    pub key_manager_payload: bool,
    /// `[cache] lookup_seconds`: lifetime of `AppState::lookups`; zero = no cache.
    pub lookup_ttl: Duration,
    /// `[session] public_url` as an origin (`https://host[:port]`), for the
    /// same-origin check of writes (roadmap 48); `None` = compare with `Host`.
    pub public_origin: Option<String>,
    /// `[websso]` when enabled (roadmap 80): where to send the browser to sign
    /// in through the identity provider.
    pub websso: Option<super::websso::WebSso>,
    /// `[writes] mode`: off | dry-run | on. Defaults to dry-run.
    pub writes: WriteMode,
    /// `[ui] help_url`, `change_password_url`: user-menu links (cloud-specific
    /// portals, like Horizon's); hidden when unset.
    pub help_url: Option<String>,
    pub change_password_url: Option<String>,
    /// `[plugins] enabled`: opt-in panels (Octavia, Heat…).
    pub plugins: Vec<crate::model::Plugin>,
    /// `[keystone] interface`: `public`, `internal` or `admin`.
    pub interface: String,
    /// `[tls]`: CA bundle path and certificate verification.
    pub ca_file: Option<String>,
    pub verify_tls: bool,
    /// `[session]`.
    pub session_timeout: chrono::Duration,
    pub token_margin: chrono::Duration,
    pub password_expiry_warning_days: Option<u32>,
    pub login: crate::prefs::LoginPrefs,
    /// `[ui]`: used when the user has not picked one (no cookie).
    pub default_theme: crate::theme::Theme,
    pub default_language: Option<crate::i18n::Lang>,
    /// `[ui] show_error_details`: send the SDK's full error text to the browser
    /// (roadmap 50). Off: the log has it.
    pub show_error_details: bool,
    /// `[ui] show_requests`: keep the REST requests in action results, dry-run
    /// included (roadmap 78). Off: they never reach the browser.
    pub show_requests: bool,
    /// `[ui] max_list_items`: a full list longer than this fails (roadmap 104); 0 = no cap.
    pub max_list_items: u32,
    /// `[log] client_ip_header`.
    pub client_ip_header: Option<http::HeaderName>,
    /// `[log] trusted_proxies`: proxies that append to `client_ip_header`,
    /// counted from the right; 0 = its leftmost entry (roadmap 63).
    pub trusted_proxies: u32,
    /// `[api_access]`.
    pub clouds_yaml_name: String,
    pub clouds_yaml_profile: Option<String>,
    /// Sent to the browser with the session info.
    pub prefs: crate::prefs::Prefs,
}

/// Process-wide state. Nothing here is required for correctness: losing it
/// only costs one extra Keystone round-trip per user.
pub struct AppState {
    pub config: Config,
    pub cookie_key: Key,
    /// Project-scoped SDK connections, keyed by hash(token, project, region).
    pub connections: Cache<String, AsyncOpenStack>,
    /// Projects visible to an unscoped token, keyed by hash(token).
    pub projects: Cache<String, Vec<ProjectRef>>,
    /// What a project + region offers, keyed like `connections`.
    pub capabilities: Cache<String, crate::model::Capabilities>,
    /// Tokens of sessions that logged out, keyed by hash(token) (roadmap 49):
    /// `session::current` refuses them, so a copied cookie stops working even
    /// though `connections` may still hold a scoped connection for it. Per
    /// process; with several replicas, Keystone revocation covers the rest.
    pub revoked: Cache<String, ()>,
    /// Lists used only for names (`fetch::lookup`, roadmap 66), keyed by
    /// hash(token, project, region) + `/` + the list's name, so a user never
    /// sees another's (visibility differs per role). Emptied for the session
    /// after each write and at logout. Weighed by item count.
    pub lookups: Cache<String, std::sync::Arc<Vec<serde_json::Value>>>,
    /// Failed sign-ins (`[login] max_failures`, `lockout_seconds`; roadmap 50).
    pub login_throttle: super::throttle::Throttle,
    /// WebSSO nonce cookies already used, keyed by hash(cookie value), so a
    /// replayed one is refused (roadmap 80). Per process: the browser deletes
    /// the cookie on use anyway.
    pub websso_used: Cache<String, ()>,
}

/// Items `AppState::lookups` holds at most, over every session.
const LOOKUP_ITEMS: u64 = 200_000;

static STATE: OnceLock<AppState> = OnceLock::new();

pub fn init(settings: &Settings) -> Result<&'static AppState, String> {
    let config = settings.config()?;
    let a = &settings.api_access;
    super::downloads::init(&[
        (super::downloads::OPENRC, a.openrc_template.clone()),
        (super::downloads::CLOUDS_YAML, a.clouds_yaml_template.clone()),
        (super::downloads::APP_OPENRC, a.app_credential_openrc_template.clone()),
        (
            super::downloads::APP_CLOUDS_YAML,
            a.app_credential_clouds_yaml_template.clone(),
        ),
        (
            super::downloads::WEBSSO_CLOUDS_YAML,
            a.websso_clouds_yaml_template.clone(),
        ),
    ])?;
    let cookie_key = match settings
        .session
        .cookie_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(b64) => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| format!("[session] cookie_key is not valid base64: {e}"))?;
            Key::try_from(bytes.as_slice())
                .map_err(|_| "[session] cookie_key must decode to at least 64 bytes".to_string())?
        }
        None => {
            tracing::warn!("[session] cookie_key not set: using a random key, sessions end on restart");
            Key::generate()
        }
    };
    let lookup_ttl = config.lookup_ttl.max(Duration::from_secs(1));
    let state = AppState {
        config,
        cookie_key,
        connections: Cache::builder()
            .max_capacity(2_000)
            .time_to_idle(Duration::from_secs(15 * 60))
            .build(),
        projects: Cache::builder()
            .max_capacity(2_000)
            .time_to_live(Duration::from_secs(5 * 60))
            .build(),
        // Extensions and versions change on upgrades only: 10 min is plenty.
        capabilities: Cache::builder()
            .max_capacity(2_000)
            .time_to_live(Duration::from_secs(10 * 60))
            .build(),
        // Keystone tokens live at most 24 h by default; after that the
        // token is dead anyway.
        revoked: Cache::builder()
            .max_capacity(100_000)
            .time_to_live(Duration::from_secs(24 * 60 * 60))
            .build(),
        // Images can number thousands on public clouds: the bound is in items
        // (a few hundred bytes to a few kB each), not entries.
        lookups: Cache::builder()
            .max_capacity(LOOKUP_ITEMS)
            .weigher(|_, v: &std::sync::Arc<Vec<serde_json::Value>>| u32::try_from(v.len()).unwrap_or(u32::MAX).max(1))
            .time_to_live(lookup_ttl)
            .build(),
        login_throttle: super::throttle::Throttle::new(
            settings.login.max_failures,
            Duration::from_secs(u64::from(settings.login.lockout_seconds)),
        ),
        // A nonce is refused after its own lifetime anyway.
        websso_used: Cache::builder()
            .max_capacity(100_000)
            .time_to_live(Duration::from_secs(super::websso::NONCE_SECONDS.unsigned_abs()))
            .build(),
    };
    Ok(STATE.get_or_init(|| state))
}

/// The state, or `None` before `init` (unit tests).
pub fn try_state() -> Option<&'static AppState> {
    STATE.get()
}

pub fn state() -> &'static AppState {
    STATE
        .get()
        .expect("server::config::init() must run before serving requests")
}
