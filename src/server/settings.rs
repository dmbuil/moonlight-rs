//! `moonlight.conf`: TOML files layered the way oslo.config layers Nova's and
//! Neutron's (roadmap 3). Lowest to highest: built-in defaults, the files
//! (`--config-file`, in CLI order, else the first default file found), every
//! `*.conf` of the directories (`--config-dir`, else the first default
//! `moonlight.conf.d/`) sorted by name, and the `MOONLIGHT_*` environment
//! variables (containers, secrets). Later values always win, key by key.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml::Table;

use crate::i18n::Lang;
use crate::model::{Plugin, WriteMode};
use crate::prefs::{ConsolePrefs, ImagePrefs, LaunchPrefs, LoginPrefs, NetworkPrefs, PagedLists, Prefs, SsoChoice, VISIBILITIES};
use crate::theme::Theme;
use crate::server::config::Config;

/// Command line: where to read `moonlight.conf` from (see `sources`).
#[derive(clap::Parser, Debug, Default)]
#[command(version, about = "Moonlight, a web dashboard for OpenStack")]
pub struct Cli {
    /// A configuration file; repeatable, later files override earlier ones.
    #[arg(long, value_name = "PATH")]
    pub config_file: Vec<PathBuf>,
    /// A directory whose `*.conf` files are read in alphabetical order after the files; repeatable.
    #[arg(long, value_name = "DIR")]
    pub config_dir: Vec<PathBuf>,
}

/// Every section of `moonlight.conf`, merged from the files and then the
/// `MOONLIGHT_*` variables; `config()` validates it into [`Config`].
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub keystone: Keystone,
    pub tls: Tls,
    pub session: Session,
    pub writes: Writes,
    pub login: Login,
    pub ui: Ui,
    pub launch: LaunchPrefs,
    pub network: NetworkPrefs,
    pub images: ImagePrefs,
    pub console: ConsolePrefs,
    pub api_access: ApiAccess,
    pub plugins: Plugins,
    pub key_manager: KeyManager,
    pub cache: CacheSettings,
    pub log: Log,
    pub websso: WebSso,
}

/// `[keystone]`: where to sign in.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Keystone {
    /// Public endpoint, without `/v3` (the SDK runs version discovery).
    pub url: Option<String>,
    pub default_domain: String,
    pub default_region: Option<String>,
    /// Catalog interface: `public`, `internal` or `admin` (`OPENSTACK_ENDPOINT_TYPE`).
    pub interface: String,
    /// Several domains in use: Identity › Domains and *Set Domain Context* for
    /// admins (`OPENSTACK_KEYSTONE_MULTIDOMAIN_SUPPORT`, roadmap 57).
    pub multidomain: bool,
}

impl Default for Keystone {
    fn default() -> Self {
        Keystone { url: None, default_domain: "Default".into(), default_region: None, interface: "public".into(), multidomain: false }
    }
}

/// `[tls]`: certificate checks for the cloud's endpoints.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tls {
    /// PEM bundle for clouds signed by a private CA (`OPENSTACK_SSL_CACERT`).
    pub ca_file: Option<String>,
    /// `false` disables certificate checks (`OPENSTACK_SSL_NO_VERIFY`); test clouds only.
    pub verify: bool,
}

impl Default for Tls {
    fn default() -> Self {
        Tls { ca_file: None, verify: true }
    }
}

/// `[session]`: cookie key and flags, timeouts, logout revocation.
#[derive(Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Session {
    /// Base64, at least 64 bytes. Never logged (see `Debug`).
    pub cookie_key: Option<String>,
    /// `Secure` session cookie named `__Host-ml_session`, plus HSTS (roadmap 49).
    /// On by default: turn it off only for plain-HTTP development.
    pub secure_cookies: bool,
    /// On logout, ask Keystone to revoke the session's token (Horizon's default,
    /// `TOKEN_DELETION_DISABLED = False`). Sent through `Exec`, so only with
    /// `[writes] mode = on`; the server forgets the token in every mode.
    pub revoke_on_logout: bool,
    /// Sign-in lasts at most this long, even if the token lives longer (`SESSION_TIMEOUT`).
    pub timeout_minutes: u32,
    /// A token this close to expiring counts as expired (`TOKEN_TIMEOUT_MARGIN`).
    pub token_margin_seconds: u32,
    /// Warn this many days before the password expires; unset = never
    /// (`PASSWORD_EXPIRES_WARNING_THRESHOLD_DAYS`).
    pub password_expiry_warning_days: Option<u32>,
    /// The URL users open, for the same-origin check of writes when a reverse
    /// proxy rewrites `Host` (roadmap 48). Unset: the request's `Host`.
    pub public_url: Option<String>,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            cookie_key: None,
            secure_cookies: true,
            revoke_on_logout: true,
            timeout_minutes: 60,
            token_margin_seconds: 60,
            password_expiry_warning_days: None,
            public_url: None,
        }
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("cookie_key", &self.cookie_key.as_ref().map(|_| "<redacted>"))
            .field("secure_cookies", &self.secure_cookies)
            .field("revoke_on_logout", &self.revoke_on_logout)
            .field("timeout_minutes", &self.timeout_minutes)
            .field("token_margin_seconds", &self.token_margin_seconds)
            .field("password_expiry_warning_days", &self.password_expiry_warning_days)
            .field("public_url", &self.public_url)
            .finish()
    }
}

/// `[writes]`: whether writes are off, previewed or sent.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Writes {
    /// `off`, `dry-run` or `on`.
    pub mode: String,
}

impl Default for Writes {
    fn default() -> Self {
        Writes { mode: "dry-run".into() }
    }
}

/// `[login]`: the login page and sign-in throttling.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Login {
    /// Domains offered as a dropdown (`OPENSTACK_KEYSTONE_DOMAIN_CHOICES`); empty = free text.
    pub domain_choices: Vec<String>,
    /// No domain field: everyone signs in to `[keystone] default_domain`.
    pub domain_hidden: bool,
    /// Let the browser remember the password (`password_autocomplete`).
    pub password_autocomplete: bool,
    /// Show the eye button that reveals the password (`disable_password_reveal`, inverted).
    pub password_reveal: bool,
    /// Text file shown above the form, e.g. a maintenance notice (`MESSAGES_PATH`).
    pub message_file: Option<String>,
    /// Failed sign-ins (per username, and per address with `[log]
    /// client_ip_header`) before `login` stops asking Keystone; 0 = off (roadmap 50).
    pub max_failures: u32,
    /// How long that lasts, counted from the last failure.
    pub lockout_seconds: u32,
}

impl Default for Login {
    fn default() -> Self {
        Login {
            domain_choices: vec![],
            domain_hidden: false,
            password_autocomplete: false,
            password_reveal: true,
            message_file: None,
            max_failures: 5,
            lockout_seconds: 300,
        }
    }
}

/// `[ui]`: user-menu links, defaults and refresh interval.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Ui {
    /// User-menu links to the cloud's own portal; hidden when unset.
    pub help_url: Option<String>,
    pub change_password_url: Option<String>,
    pub bug_url: Option<String>,
    /// `system`, `light` or `dark`, until the user picks one.
    pub default_theme: String,
    /// `en` or `es`; unset = the browser's language.
    pub default_language: Option<String>,
    pub poll_interval_ms: u32,
    /// Send the SDK's full error text (with internal URIs) to the browser.
    pub show_error_details: bool,
    /// Show the REST requests an action sent (or, in dry-run, built) in its
    /// result (roadmap 78).
    pub show_requests: bool,
    /// Bulk actions with this many items or more run in the background (roadmap 87).
    pub background_batch_from: u32,
    /// Rows per page of the paged lists, 1–999; `0` = no paging (roadmap 88).
    pub page_size: u32,
    /// `admin` (default) or `all`: which lists are paged (roadmap 88).
    pub paged_lists: String,
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            help_url: None,
            change_password_url: None,
            bug_url: None,
            default_theme: "system".into(),
            default_language: None,
            poll_interval_ms: Prefs::default().poll_interval_ms,
            show_error_details: false,
            show_requests: false,
            background_batch_from: Prefs::default().background_batch_from,
            page_size: Prefs::default().page_size,
            paged_lists: "admin".into(),
        }
    }
}

/// Largest `[ui] page_size` (roadmap 88). A page asks for `page_size + 1` rows, the extra one
/// only saying whether there is a *Next*; Nova (`max_limit`) and Glance (`api_limit_max`) cap
/// `limit` at 1000 by default, so at 1000 the extra row never came and *Next* never showed.
const MAX_PAGE_SIZE: u32 = 999;

/// `[api_access]`: the `clouds.yaml` download.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ApiAccess {
    /// Entry name in the downloaded `clouds.yaml` (`OPENSTACK_CLOUDS_YAML_NAME`).
    pub clouds_yaml_name: String,
    /// os-client-config vendor profile (`OPENSTACK_CLOUDS_YAML_PROFILE`).
    pub clouds_yaml_profile: Option<String>,
    /// Jinja templates replacing the built-in downloads (roadmap 57): API
    /// Access's (`OPENRC_CUSTOM_TEMPLATE`, `OPENSTACK_CLOUDS_YAML_CUSTOM_TEMPLATE`)
    /// and a new application credential's. Unset = built-in.
    pub openrc_template: Option<String>,
    pub clouds_yaml_template: Option<String>,
    pub app_credential_openrc_template: Option<String>,
    pub app_credential_clouds_yaml_template: Option<String>,
    /// API Access's `clouds.yaml` for WebSSO sessions (`v3websso`, roadmap 80).
    pub websso_clouds_yaml_template: Option<String>,
}

impl Default for ApiAccess {
    fn default() -> Self {
        ApiAccess {
            clouds_yaml_name: "openstack".into(),
            clouds_yaml_profile: None,
            openrc_template: None,
            clouds_yaml_template: None,
            app_credential_openrc_template: None,
            app_credential_clouds_yaml_template: None,
            websso_clouds_yaml_template: None,
        }
    }
}

/// `[plugins]`: opt-in panels.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Plugins {
    /// Opt-in panels (`load-balancer`, `orchestration`); none when unset.
    pub enabled: Vec<String>,
}

/// `[key_manager]`: the Key Manager panels (`key-manager` plugin, roadmap 55).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeyManager {
    /// Offer *Show Payload* on a secret's page: the secret material reaches the
    /// browser (fetched on click, never cached). Off by default.
    pub allow_payload: bool,
}

/// `[cache]`: in-process caches (roadmap 66).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CacheSettings {
    /// How long the lists used only for names (flavors, images, networks,
    /// volumes) are reused per token, project and region; 0 = off.
    pub lookup_seconds: u32,
}

impl Default for CacheSettings {
    fn default() -> Self {
        CacheSettings { lookup_seconds: 30 }
    }
}

/// `[log]`: log filter, where the log goes, and the client-address header.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Log {
    /// A `tracing` filter, like `RUST_LOG`.
    pub filter: String,
    /// Log file; unset or `""` = stdout only. Its directory must exist (roadmap 81).
    pub file: Option<String>,
    /// When a new file is started; rotated files get a date suffix.
    pub rotation: Rotation,
    /// Whether a restart appends to the current file (`true`, the default) or
    /// starts it empty (roadmap 84). A number is the old meaning of `keep`.
    pub keep: KeepSetting,
    /// Log files kept, the current one included; 0 = all. Unset: the old
    /// numeric `keep` if there is one, else 7.
    pub max_files: Option<u32>,
    /// Also write to stdout when `file` is set.
    pub stdout: bool,
    /// Line format of every output.
    pub format: LogFormat,
    /// Header with the client's address behind a proxy, e.g. `X-Forwarded-For`
    /// (`SECURE_PROXY_ADDR_HEADER`); logged with sign-ins and writes.
    pub client_ip_header: Option<String>,
    /// Proxies in front of Moonlight that append to that header: the client is
    /// the entry this many places from the right; 0 = the leftmost, which the
    /// client controls (roadmap 63).
    pub trusted_proxies: u32,
}

impl Default for Log {
    fn default() -> Self {
        Log {
            filter: "moonlight=info,warn".into(),
            file: None,
            rotation: Rotation::Daily,
            keep: KeepSetting::Flag(true),
            max_files: None,
            stdout: false,
            format: LogFormat::Text,
            client_ip_header: None,
            trusted_proxies: 1,
        }
    }
}

impl Log {
    /// Whether a restart keeps the current log file's lines (roadmap 84).
    pub fn keep_current(&self) -> bool {
        match self.keep {
            KeepSetting::Flag(on) => on,
            // The old count never meant "start empty"; that was the bug.
            KeepSetting::Count(_) => true,
        }
    }
    /// Log files kept, the current one included; 0 = all.
    pub fn max_files(&self) -> u32 {
        match (self.max_files, self.keep) {
            (Some(n), _) => n,
            (None, KeepSetting::Count(n)) => n,
            (None, KeepSetting::Flag(_)) => 7,
        }
    }
    /// The old numeric `keep`, for a start-up warning.
    pub fn legacy_keep(&self) -> Option<u32> {
        match self.keep {
            KeepSetting::Count(n) => Some(n),
            KeepSetting::Flag(_) => None,
        }
    }
    /// The log file, if one is set (blank counts as unset).
    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref().map(str::trim).filter(|f| !f.is_empty()).map(Path::new)
    }
}

/// `[log] rotation`. `never` leaves rotation to the system's `logrotate`, which
/// then needs `copytruncate`: the file is not reopened on `SIGHUP`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rotation {
    Never,
    Hourly,
    Daily,
}

/// `[log] keep` (roadmap 84): a yes/no (`true`/`false`, or the strings `yes`,
/// `enabled`, `on` / `no`, `disabled`, `off`), or a number, which is what `keep`
/// meant before (files kept, now `max_files`) and is still read as that.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeepSetting {
    Flag(bool),
    Count(u32),
}

impl<'de> Deserialize<'de> for KeepSetting {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = KeepSetting;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("true/yes/enabled/on or false/no/disabled/off")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<KeepSetting, E> {
                Ok(KeepSetting::Flag(v))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<KeepSetting, E> {
                u32::try_from(v).map(KeepSetting::Count).map_err(|_| E::custom(format!("keep: {v} is not a number of files")))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<KeepSetting, E> {
                u32::try_from(v).map(KeepSetting::Count).map_err(|_| E::custom(format!("keep: {v} is not a number of files")))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<KeepSetting, E> {
                match v.trim().to_ascii_lowercase().as_str() {
                    "true" | "yes" | "enabled" | "on" => Ok(KeepSetting::Flag(true)),
                    "false" | "no" | "disabled" | "off" => Ok(KeepSetting::Flag(false)),
                    _ => Err(E::custom(format!("keep: {v:?} is not one of true/yes/enabled/on or false/no/disabled/off"))),
                }
            }
        }
        d.deserialize_any(V)
    }
}

/// `[log] format`: `text` for people, `json` (one object per line) for log shippers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Text,
    Json,
}

/// `[websso]`: sign-in through Keystone federation (OIDC, SAML), after
/// Horizon's `WEBSSO_*` settings (roadmap 80).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WebSso {
    /// The *Authenticate using* select on the login page (`WEBSSO_ENABLED`).
    pub enabled: bool,
    /// Its options, in order (`WEBSSO_CHOICES`): `credentials` is the password
    /// form, any other ID a federation protocol or a key of `idp_mapping`.
    pub choices: Vec<SsoChoice>,
    /// Preselected choice; unset = the first (`WEBSSO_INITIAL_CHOICE`).
    pub initial_choice: Option<String>,
    /// Choice ID → identity provider and protocol, for Keystone's
    /// per-provider WebSSO URL (`WEBSSO_IDP_MAPPING`).
    pub idp_mapping: std::collections::BTreeMap<String, IdpProtocol>,
    /// Keystone as the browser reaches it, when not `[keystone] url`
    /// (`WEBSSO_KEYSTONE_URL`).
    pub keystone_url: Option<String>,
    /// Where the browser goes after logout, e.g. the IdP's logout page
    /// (`WEBSSO_DEFAULT_REDIRECT_LOGOUT`).
    pub logout_url: Option<String>,
}

/// A value of `[websso] idp_mapping`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdpProtocol {
    pub idp: String,
    pub protocol: String,
}

/// Environment variable → `(section, key)`. `RUST_LOG` keeps its usual meaning.
const ENV: &[(&str, &str, &str)] = &[
    ("MOONLIGHT_KEYSTONE_URL", "keystone", "url"),
    ("MOONLIGHT_DEFAULT_DOMAIN", "keystone", "default_domain"),
    ("MOONLIGHT_DEFAULT_REGION", "keystone", "default_region"),
    ("MOONLIGHT_MULTIDOMAIN", "keystone", "multidomain"),
    ("MOONLIGHT_COOKIE_KEY", "session", "cookie_key"),
    ("MOONLIGHT_SECURE_COOKIES", "session", "secure_cookies"),
    ("MOONLIGHT_PUBLIC_URL", "session", "public_url"),
    ("MOONLIGHT_WRITES", "writes", "mode"),
    ("MOONLIGHT_HELP_URL", "ui", "help_url"),
    ("MOONLIGHT_CHANGE_PASSWORD_URL", "ui", "change_password_url"),
    ("MOONLIGHT_PLUGINS", "plugins", "enabled"),
    ("MOONLIGHT_TRUSTED_PROXIES", "log", "trusted_proxies"),
    ("MOONLIGHT_LOG_FILE", "log", "file"),
    ("RUST_LOG", "log", "filter"),
];

/// oslo.config's search list (`find_config_files` / `find_config_dirs`), in
/// priority order: only the first one that exists is used.
pub fn default_candidates(home: Option<&Path>) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(h) = home {
        dirs.push(h.join(".moonlight"));
        dirs.push(h.to_path_buf());
    }
    dirs.push("/etc/moonlight".into());
    dirs.push("/etc".into());
    (dirs.iter().map(|d| d.join("moonlight.conf")).collect(), dirs.iter().map(|d| d.join("moonlight.conf.d")).collect())
}

/// The files to read, lowest priority first. Paths given on the command line
/// must exist; the defaults are only looked for when none were given.
pub fn sources(cli: &Cli, defaults: &(Vec<PathBuf>, Vec<PathBuf>)) -> Result<Vec<PathBuf>, String> {
    let pick = |given: &[PathBuf], default: &[PathBuf], is: fn(&Path) -> bool, what: &str| -> Result<Vec<PathBuf>, String> {
        if given.is_empty() {
            return Ok(default.iter().find(|p| is(p)).cloned().into_iter().collect());
        }
        match given.iter().find(|p| !is(p)) {
            Some(p) => Err(format!("{what} {} not found", p.display())),
            None => Ok(given.to_vec()),
        }
    };
    let mut out = pick(&cli.config_file, &defaults.0, Path::is_file, "--config-file")?;
    for dir in pick(&cli.config_dir, &defaults.1, Path::is_dir, "--config-dir")? {
        let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut confs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "conf"))
            .collect();
        confs.sort();
        out.extend(confs);
    }
    Ok(out)
}

/// Deep merge: tables are merged key by key, anything else is replaced.
pub fn merge(base: &mut Table, over: Table) {
    for (k, v) in over {
        match (base.get_mut(&k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            (_, v) => {
                base.insert(k, v);
            }
        }
    }
}

/// The environment as one more layer. Empty variables count as unset.
fn env_layer(get: &dyn Fn(&str) -> Option<String>) -> Table {
    let mut t = Table::new();
    for &(var, section, key) in ENV {
        let Some(v) = get(var).map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) else { continue };
        let v = match key {
            "secure_cookies" | "multidomain" => toml::Value::Boolean(v == "true" || v == "1"),
            // Not a number: left a string, so loading names the setting.
            "trusted_proxies" => v.parse().map_or(toml::Value::String(v), toml::Value::Integer),
            // Comma-separated: `load-balancer,orchestration`.
            "enabled" => toml::Value::Array(
                v.split(',').map(str::trim).filter(|p| !p.is_empty()).map(|p| toml::Value::String(p.into())).collect(),
            ),
            _ => toml::Value::String(v),
        };
        let entry = t.entry(section).or_insert_with(|| toml::Value::Table(Table::new()));
        if let toml::Value::Table(s) = entry {
            s.insert(key.into(), v);
        }
    }
    t
}

/// Reads `files` in order, then the environment, and deserialises the result.
/// Each file is checked on its own first, so an unknown key names its file.
pub fn load(files: &[PathBuf], env: &dyn Fn(&str) -> Option<String>) -> Result<Settings, String> {
    let mut all = Table::new();
    for f in files {
        let text = std::fs::read_to_string(f).map_err(|e| format!("{}: {e}", f.display()))?;
        toml::from_str::<Settings>(&text).map_err(|e| format!("{}: {e}", f.display()))?;
        merge(&mut all, toml::from_str(&text).map_err(|e| format!("{}: {e}", f.display()))?);
    }
    merge(&mut all, env_layer(env));
    toml::Value::Table(all).try_into().map_err(|e| format!("configuration: {e}"))
}

/// An optional `http(s)` URL setting: empty → `None`, anything else must parse.
fn url_setting(name: &str, value: Option<&str>) -> Result<Option<String>, String> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) if (v.starts_with("https://") || v.starts_with("http://")) && !v.contains(char::is_whitespace) => Ok(Some(v.into())),
        Some(v) => Err(format!("{name} must be an http(s) URL (got {v:?})")),
    }
}

/// `scheme://host[:port]` of a URL, as browsers send it in `Origin`: lowercase,
/// without path, user info or a default port (`:443` kept, it never matched,
/// roadmap 63).
fn origin_of(name: &str, url: &str) -> Result<String, String> {
    let u = url::Url::parse(url).map_err(|e| format!("{name}: {e} (got {url:?})"))?;
    Ok(u.origin().ascii_serialization())
}

/// Characters allowed in a `[websso] choices` ID. They are Keystone protocol IDs
/// (`openid`, `saml2`, `mapped`) and go into a query string and a URL path
/// unescaped, so nothing that needs escaping.
fn is_choice_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// Validates `[websso]`: the choices, the mapping, and the URLs Keystone needs.
/// `None` when disabled (the rest of the section is then ignored).
fn websso(w: &WebSso, keystone_url: &str, public_origin: Option<&str>) -> Result<Option<(super::websso::WebSso, Vec<SsoChoice>, String)>, String> {
    if !w.enabled {
        return Ok(None);
    }
    // Keystone posts the token back to an `origin` it must list in
    // `[federation] trusted_dashboard`, byte for byte: guessing it from `Host`
    // would break behind a proxy.
    let origin = public_origin.ok_or("[websso] enabled needs [session] public_url (Keystone sends the token back to it)")?;
    let mut choices: Vec<SsoChoice> = Vec::new();
    for c in &w.choices {
        let id = c.id.trim();
        if !is_choice_id(id) {
            return Err(format!("[websso] choices: {id:?} is not a valid ID (letters, digits, '.', '_', '-')"));
        }
        if choices.iter().any(|x| x.id == id) {
            return Err(format!("[websso] choices: {id:?} appears twice"));
        }
        choices.push(SsoChoice { id: id.into(), label: c.label.trim().into() });
    }
    let federated: Vec<String> = choices.iter().map(|c| c.id.clone()).filter(|id| id != SsoChoice::CREDENTIALS).collect();
    if federated.is_empty() {
        return Err("[websso] choices needs at least one federation protocol besides \"credentials\"".into());
    }
    let initial = match w.initial_choice.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(i) if choices.iter().any(|c| c.id == i) => i.to_string(),
        Some(i) => return Err(format!("[websso] initial_choice {i:?} is not one of the choices")),
        None => choices[0].id.clone(),
    };
    let mut idp_mapping = std::collections::BTreeMap::new();
    for (id, m) in &w.idp_mapping {
        if !federated.contains(id) {
            return Err(format!("[websso] idp_mapping: {id:?} is not a federation choice"));
        }
        let (idp, protocol) = (m.idp.trim(), m.protocol.trim());
        if idp.is_empty() || protocol.is_empty() {
            return Err(format!("[websso] idp_mapping.{id}: idp and protocol cannot be empty"));
        }
        idp_mapping.insert(id.clone(), (idp.to_string(), protocol.to_string()));
    }
    let keystone = url_setting("[websso] keystone_url", w.keystone_url.as_deref())?.unwrap_or_else(|| keystone_url.to_string());
    let ws = super::websso::WebSso::new(&keystone, origin, federated, idp_mapping, url_setting("[websso] logout_url", w.logout_url.as_deref())?)
        .map_err(|e| format!("[websso] keystone_url: {e}"))?;
    Ok(Some((ws, choices, initial)))
}

/// Known plugin names, each once; anything else is a typo worth stopping for.
fn plugins(names: &[String]) -> Result<Vec<Plugin>, String> {
    let mut out = Vec::new();
    for n in names {
        let p = Plugin::parse(n).ok_or_else(|| {
            let known: Vec<_> = Plugin::ALL.iter().map(|p| p.name()).collect();
            format!("[plugins] enabled: unknown plugin {n:?} (known: {})", known.join(", "))
        })?;
        if !out.contains(&p) {
            out.push(p);
        }
    }
    Ok(out)
}

impl Settings {
    /// Validates the settings the server runs with (the cookie key is decoded by `config::init`).
    pub fn config(&self) -> Result<Config, String> {
        let url = self.keystone.url.as_deref().map(str::trim).filter(|s| !s.is_empty()).ok_or(
            "[keystone] url is not set (moonlight.conf or MOONLIGHT_KEYSTONE_URL; see moonlight.conf.example)",
        )?;
        let mode = self.writes.mode.trim();
        let interface = self.keystone.interface.trim();
        if !["public", "internal", "admin"].contains(&interface) {
            return Err(format!("[keystone] interface must be public, internal or admin (got {interface:?})"));
        }
        let ca_file = self.tls.ca_file.as_deref().map(str::trim).filter(|s| !s.is_empty());
        if let Some(f) = ca_file
            && !Path::new(f).is_file()
        {
            return Err(format!("[tls] ca_file {f} not found"));
        }
        if self.session.timeout_minutes == 0 {
            return Err("[session] timeout_minutes must be at least 1".into());
        }
        let l = &self.login;
        let domain_choices: Vec<String> = l.domain_choices.iter().map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).collect();
        if l.domain_hidden && !domain_choices.is_empty() {
            return Err("[login] domain_hidden and domain_choices cannot be used together".into());
        }
        let message = match l.message_file.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(f) => Some(std::fs::read_to_string(f).map_err(|e| format!("[login] message_file {f}: {e}"))?.trim().to_string()).filter(|m| !m.is_empty()),
            None => None,
        };
        let theme = self.ui.default_theme.trim();
        let default_theme = Theme::parse(theme).ok_or_else(|| format!("[ui] default_theme must be system, light or dark (got {theme:?})"))?;
        let default_language = match self.ui.default_language.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(l) => Some(Lang::parse(l).ok_or_else(|| format!("[ui] default_language must be en or es (got {l:?})"))?),
            None => None,
        };
        if self.ui.poll_interval_ms < 1000 {
            return Err(format!("[ui] poll_interval_ms must be at least 1000 (got {})", self.ui.poll_interval_ms));
        }
        if self.ui.page_size > MAX_PAGE_SIZE {
            return Err(format!("[ui] page_size must be 0 (no paging) or 1-{MAX_PAGE_SIZE}, the APIs' usual cap (got {})", self.ui.page_size));
        }
        let paged_lists = match self.ui.paged_lists.trim() {
            "admin" => PagedLists::Admin,
            "all" => PagedLists::All,
            other => return Err(format!("[ui] paged_lists must be admin or all (got {other:?})")),
        };
        if self.launch.sources().is_empty() {
            return Err("[launch] at least one boot source must stay enabled".into());
        }
        let net = &self.network;
        for (version, list) in [("ipv4", &net.allowed_private_subnet_cidr.ipv4), ("ipv6", &net.allowed_private_subnet_cidr.ipv6)] {
            for c in list {
                let cidr = crate::prefs::Cidr::parse(c).map_err(|e| format!("[network] allowed_private_subnet_cidr.{version}: {e}"))?;
                if cidr.addr.is_ipv4() != (version == "ipv4") {
                    return Err(format!("[network] allowed_private_subnet_cidr.{version}: {c} is not {version}"));
                }
            }
        }
        net.check_provider_types()?;
        for d in &net.default_dns_nameservers {
            d.trim().parse::<std::net::IpAddr>().map_err(|_| format!("[network] default_dns_nameservers: {d:?} is not an IP address"))?;
        }
        let img = &self.images;
        if img.disk_formats.iter().all(|f| f.trim().is_empty()) {
            return Err("[images] disk_formats cannot be empty".into());
        }
        if !VISIBILITIES.contains(&img.default_visibility.as_str()) {
            return Err(format!("[images] default_visibility must be one of {} (got {:?})", VISIBILITIES.join(", "), img.default_visibility));
        }
        if img.max_upload_gb == 0 {
            return Err("[images] max_upload_gb must be at least 1".into());
        }
        if self.console.log_length == 0 {
            return Err("[console] log_length must be at least 1".into());
        }
        let client_ip_header = match self.log.client_ip_header.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(h) => Some(http::HeaderName::from_bytes(h.as_bytes()).map_err(|_| format!("[log] client_ip_header {h:?} is not a header name"))?),
            None => None,
        };
        let yaml_name = self.api_access.clouds_yaml_name.trim();
        if yaml_name.is_empty() {
            return Err("[api_access] clouds_yaml_name cannot be empty".into());
        }
        let keystone_url = url.trim_end_matches('/').to_string();
        let public_origin = url_setting("[session] public_url", self.session.public_url.as_deref())?.map(|u| origin_of("[session] public_url", &u)).transpose()?;
        let (websso, websso_choices, websso_initial) = match websso(&self.websso, &keystone_url, public_origin.as_deref())? {
            Some((w, choices, initial)) => (Some(w), choices, initial),
            None => (None, vec![], String::new()),
        };
        Ok(Config {
            keystone_url,
            default_domain: self.keystone.default_domain.clone(),
            default_region: self.keystone.default_region.clone().filter(|s| !s.is_empty()),
            multidomain: self.keystone.multidomain,
            key_manager_payload: self.key_manager.allow_payload,
            lookup_ttl: std::time::Duration::from_secs(u64::from(self.cache.lookup_seconds)),
            secure_cookies: self.session.secure_cookies,
            revoke_on_logout: self.session.revoke_on_logout,
            public_origin,
            websso,
            show_error_details: self.ui.show_error_details,
            show_requests: self.ui.show_requests,
            writes: WriteMode::parse(mode).ok_or_else(|| format!("[writes] mode must be off, dry-run or on (got {mode:?})"))?,
            help_url: url_setting("[ui] help_url", self.ui.help_url.as_deref())?,
            change_password_url: url_setting("[ui] change_password_url", self.ui.change_password_url.as_deref())?,
            plugins: plugins(&self.plugins.enabled)?,
            interface: interface.to_string(),
            ca_file: ca_file.map(str::to_string),
            verify_tls: self.tls.verify,
            session_timeout: chrono::Duration::minutes(i64::from(self.session.timeout_minutes)),
            token_margin: chrono::Duration::seconds(i64::from(self.session.token_margin_seconds)),
            password_expiry_warning_days: self.session.password_expiry_warning_days,
            login: LoginPrefs {
                default_domain: self.keystone.default_domain.clone(),
                domain_choices,
                domain_hidden: l.domain_hidden,
                password_autocomplete: l.password_autocomplete,
                password_reveal: l.password_reveal,
                message,
                websso_choices,
                websso_initial,
            },
            default_theme,
            default_language,
            client_ip_header,
            trusted_proxies: self.log.trusted_proxies,
            clouds_yaml_name: yaml_name.to_string(),
            clouds_yaml_profile: self.api_access.clouds_yaml_profile.clone().filter(|p| !p.trim().is_empty()),
            prefs: Prefs {
                poll_interval_ms: self.ui.poll_interval_ms,
                bug_url: url_setting("[ui] bug_url", self.ui.bug_url.as_deref())?,
                background_batch_from: self.ui.background_batch_from,
                page_size: self.ui.page_size,
                paged_lists,
                launch: self.launch.clone(),
                network: NetworkPrefs {
                    default_dns_nameservers: net.default_dns_nameservers.iter().map(|d| d.trim().to_string()).collect(),
                    ..net.clone()
                },
                images: ImagePrefs {
                    disk_formats: img.disk_formats.iter().map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect(),
                    reserved_properties: img.reserved_properties.iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect(),
                    ..img.clone()
                },
                console: self.console.clone(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A fresh directory under the system temp dir, removed on drop.
    struct Tmp(PathBuf);
    impl Tmp {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("moonlight-settings-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
        fn file(&self, rel: &str, text: &str) -> PathBuf {
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, text).unwrap();
            p
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn defaults_follow_oslo_search_order() {
        let (files, dirs) = default_candidates(Some(Path::new("/home/u")));
        let s = |v: &[PathBuf]| v.iter().map(|p| p.display().to_string()).collect::<Vec<_>>();
        assert_eq!(
            s(&files),
            ["/home/u/.moonlight/moonlight.conf", "/home/u/moonlight.conf", "/etc/moonlight/moonlight.conf", "/etc/moonlight.conf"]
        );
        assert_eq!(s(&dirs)[3], "/etc/moonlight.conf.d");
        assert_eq!(default_candidates(None).0.len(), 2);
    }

    #[test]
    fn dir_files_are_read_alphabetically_after_the_files_and_later_wins() {
        let t = Tmp::new("order");
        let main = t.file("moonlight.conf", "[keystone]\nurl = \"https://main\"\ndefault_region = \"r1\"\n");
        t.file("d/20-site.conf", "[keystone]\nurl = \"https://site\"\n");
        t.file("d/10-base.conf", "[keystone]\nurl = \"https://base\"\ndefault_domain = \"Corp\"\n");
        t.file("d/README", "not toml");
        let cli = Cli { config_file: vec![main.clone()], config_dir: vec![t.0.join("d")] };
        let files = sources(&cli, &(vec![], vec![])).unwrap();
        let names: Vec<_> = files.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect();
        assert_eq!(names, ["moonlight.conf", "10-base.conf", "20-site.conf"]);
        let s = load(&files, &no_env).unwrap();
        assert_eq!(s.keystone.url.as_deref(), Some("https://site"));
        assert_eq!(s.keystone.default_domain, "Corp", "partial override keeps other keys");
        assert_eq!(s.keystone.default_region.as_deref(), Some("r1"));
        assert_eq!(s.writes.mode, "dry-run", "built-in default");
    }

    #[test]
    fn defaults_are_only_used_without_cli_paths_and_only_the_first_found() {
        let t = Tmp::new("defaults");
        let home = t.file(".moonlight/moonlight.conf", "");
        let etc = t.file("etc/moonlight.conf", "");
        let missing = t.0.join("nope/moonlight.conf");
        let defaults = (vec![missing.clone(), home.clone(), etc.clone()], vec![t.0.join("nope.d")]);
        assert_eq!(sources(&Cli::default(), &defaults).unwrap(), [home]);
        let cli = Cli { config_file: vec![etc.clone()], ..Default::default() };
        assert_eq!(sources(&cli, &defaults).unwrap(), [etc]);
        let cli = Cli { config_file: vec![missing], ..Default::default() };
        assert!(sources(&cli, &defaults).unwrap_err().contains("not found"));
    }

    #[test]
    fn environment_overrides_files() {
        let t = Tmp::new("env");
        let f = t.file("m.conf", "[writes]\nmode = \"off\"\n[session]\nsecure_cookies = true\n[keystone]\nurl = \"https://file\"\n");
        let env: HashMap<&str, &str> =
            [("MOONLIGHT_WRITES", "on"), ("MOONLIGHT_SECURE_COOKIES", "0"), ("MOONLIGHT_KEYSTONE_URL", " "), ("RUST_LOG", "debug"), ("MOONLIGHT_TRUSTED_PROXIES", "0")]
                .into();
        let s = load(&[f], &|k| env.get(k).map(|v| v.to_string())).unwrap();
        assert_eq!(s.writes.mode, "on");
        assert!(!s.session.secure_cookies);
        assert_eq!(s.keystone.url.as_deref(), Some("https://file"), "empty variables are ignored");
        assert_eq!((s.log.filter.as_str(), s.log.trusted_proxies), ("debug", 0));
        let bad: HashMap<&str, &str> = [("MOONLIGHT_TRUSTED_PROXIES", "two")].into();
        assert!(load(&[], &|k| bad.get(k).map(|v| v.to_string())).unwrap_err().contains("trusted_proxies"));
    }

    #[test]
    fn the_log_goes_to_stdout_unless_a_file_is_set() {
        let d = Log::default();
        assert_eq!((d.file(), d.rotation, d.keep_current(), d.max_files(), d.stdout, d.format), (None, Rotation::Daily, true, 7, false, LogFormat::Text));
        let log = |t: &str| toml::from_str::<Settings>(t).map(|s| s.log).map_err(|e| e.to_string());
        assert_eq!(log("[log]\nfile = \" \"\n").unwrap().file(), None, "blank = stdout only");
        let full = log("[log]\nfile = \"/var/log/moonlight/moonlight.log\"\nrotation = \"hourly\"\nkeep = \"no\"\nmax_files = 24\nstdout = true\nformat = \"json\"\n").unwrap();
        assert_eq!(full.file(), Some(Path::new("/var/log/moonlight/moonlight.log")));
        assert_eq!((full.rotation, full.keep_current(), full.max_files(), full.stdout, full.format), (Rotation::Hourly, false, 24, true, LogFormat::Json));
        assert!(log("[log]\nrotation = \"weekly\"\n").unwrap_err().contains("rotation"));
        // `keep` (roadmap 84): a yes/no in any of its spellings, or the old count.
        for (v, on) in [("true", true), ("\"yes\"", true), ("\"Enabled\"", true), ("\"on\"", true), ("false", false), ("\"no\"", false), ("\"disabled\"", false), ("\"off\"", false)] {
            let l = log(&format!("[log]\nkeep = {v}\n")).unwrap();
            assert_eq!((l.keep_current(), l.max_files(), l.legacy_keep()), (on, 7, None), "keep = {v}");
        }
        let old = log("[log]\nkeep = 3\n").unwrap();
        assert_eq!((old.keep_current(), old.max_files(), old.legacy_keep()), (true, 3, Some(3)), "the old count is max_files and keeps the current file");
        assert_eq!(log("[log]\nkeep = 3\nmax_files = 5\n").unwrap().max_files(), 5, "max_files wins");
        assert!(log("[log]\nkeep = \"maybe\"\n").unwrap_err().contains("keep"));
        assert!(log("[log]\nkeep = -1\n").unwrap_err().contains("keep"));
        assert!(log("[log]\nformat = \"xml\"\n").unwrap_err().contains("format"));
        let env: HashMap<&str, &str> = [("MOONLIGHT_LOG_FILE", "/tmp/m.log")].into();
        let s = load(&[], &|k| env.get(k).map(|v| v.to_string())).unwrap();
        assert_eq!(s.log.file(), Some(Path::new("/tmp/m.log")));
    }

    #[test]
    fn invalid_values_are_rejected_with_their_origin() {
        let t = Tmp::new("invalid");
        let typo = t.file("typo.conf", "[writes]\nmodee = \"on\"\n");
        assert!(load(&[typo], &no_env).unwrap_err().starts_with(&t.0.join("typo.conf").display().to_string()));
        let bad = t.file("bad.conf", "[keystone]\nurl = \"https://k/\"\n[writes]\nmode = \"yes\"\n");
        let s = load(&[bad], &no_env).unwrap();
        assert!(s.config().unwrap_err().contains("[writes] mode"));
        assert!(Settings::default().config().unwrap_err().contains("[keystone] url"));
        let ok = Settings { keystone: Keystone { url: Some("https://k/".into()), ..Default::default() }, ..Default::default() };
        let c = ok.config().unwrap();
        assert_eq!((c.keystone_url.as_str(), c.writes), ("https://k", WriteMode::DryRun));
    }

    fn parse(text: &str) -> Result<Config, String> {
        toml::from_str::<Settings>(&format!("[keystone]\nurl = \"https://k\"\n{text}")).map_err(|e| e.to_string())?.config()
    }

    #[test]
    fn page_size_and_paged_lists_are_validated() {
        assert_eq!(parse("[ui]\npage_size = 999").unwrap().prefs.page_size, 999);
        assert_eq!(parse("[ui]\npage_size = 0").unwrap().prefs.page_size_for(true), None, "0 = no paging");
        assert!(parse("[ui]\npage_size = 1000").unwrap_err().contains("page_size"), "limit = page_size + 1 must stay within the APIs' default cap of 1000");
        assert_eq!(parse("[ui]\npaged_lists = \"all\"").unwrap().prefs.paged_lists, PagedLists::All);
        assert!(parse("[ui]\npaged_lists = \"some\"").unwrap_err().contains("paged_lists"));
        let admin = parse("").unwrap().prefs;
        assert_eq!((admin.page_size_for(true), admin.page_size_for(false)), (Some(100), None), "Project panels stay whole by default");
        let all = parse("[ui]\npaged_lists = \"all\"").unwrap().prefs;
        assert_eq!(all.page_size_for(false), Some(100));
    }

    #[test]
    fn a_file_with_only_the_url_gets_every_default() {
        let c = parse("").unwrap();
        assert_eq!((c.default_domain.as_str(), c.interface.as_str(), c.writes), ("Default", "public", WriteMode::DryRun));
        assert_eq!((c.ca_file.clone(), c.verify_tls, c.secure_cookies, c.revoke_on_logout), (None, true, true, true), "safe by default");
        assert!(!c.show_error_details, "internal URIs stay in the log by default");
        assert!(!c.show_requests, "results without the REST requests by default (roadmap 78)");
        assert!(parse("[ui]\nshow_requests = true").unwrap().show_requests);
        assert_eq!(parse("").unwrap().prefs.background_batch_from, 10, "bulk actions of 10 items or more run in the background (roadmap 87)");
        assert_eq!(parse("[ui]\nbackground_batch_from = 0").unwrap().prefs.background_batch_from, 0);
        assert_eq!((c.prefs.page_size, c.prefs.paged_lists), (100, PagedLists::Admin), "Admin lists, 100 rows a page (roadmap 88)");
        assert_eq!((c.session_timeout.num_minutes(), c.token_margin.num_seconds(), c.password_expiry_warning_days), (60, 60, None));
        assert_eq!(c.login, LoginPrefs::default());
        assert_eq!((c.default_theme, c.default_language), (Theme::System, None));
        assert_eq!((c.help_url.clone(), c.change_password_url.clone(), c.client_ip_header.clone()), (None, None, None));
        assert_eq!(c.trusted_proxies, 1, "the address the nearest proxy saw, not the client's claim");
        assert_eq!((c.clouds_yaml_name.as_str(), c.clouds_yaml_profile.clone()), ("openstack", None));
        assert_eq!(c.prefs, Prefs::default());
        assert!(c.plugins.is_empty());
        assert!(!c.key_manager_payload, "secret payloads stay out of the browser by default");
        // An empty section is the same as a missing one.
        let empty = parse("[tls]\n[session]\n[login]\n[ui]\n[launch]\n[network]\n[network.allowed_private_subnet_cidr]\n[images]\n[console]\n[api_access]\n[log]\n").unwrap();
        assert_eq!((empty.prefs, empty.login, empty.session_timeout), (c.prefs, c.login, c.session_timeout));
    }

    #[test]
    fn horizon_style_settings_are_read() {
        let c = parse(
            "interface = \"internal\"\n[session]\ntimeout_minutes = 30\ntoken_margin_seconds = 0\npassword_expiry_warning_days = 7\n\
             [login]\ndomain_choices = [\"Default\", \"Corp\"]\npassword_reveal = false\n\
             [ui]\ndefault_theme = \"dark\"\ndefault_language = \"es\"\npoll_interval_ms = 2500\nbug_url = \"https://bugs.example\"\n\
             [launch]\nrequires_keypair = true\nflavor_sort = \"vcpus\"\n\
             [network]\ndefault_dns_nameservers = [\"10.0.0.2\"]\nallowed_private_subnet_cidr = { ipv4 = [\"10.0.0.0/8\"] }\n\
             [images]\ndefault_visibility = \"shared\"\n[console]\nenabled = true\ntype = \"spice\"\n[api_access]\nclouds_yaml_name = \"acme\"\n[log]\nclient_ip_header = \"X-Forwarded-For\"\ntrusted_proxies = 2\n",
        )
        .unwrap();
        assert_eq!((c.interface.as_str(), c.session_timeout.num_minutes(), c.token_margin.num_seconds()), ("internal", 30, 0));
        assert_eq!(c.password_expiry_warning_days, Some(7));
        assert_eq!((c.login.domain_choices.len(), c.login.password_reveal), (2, false));
        assert_eq!((c.default_theme, c.default_language), (Theme::Dark, Some(Lang::EsEs)));
        assert_eq!((c.prefs.poll_interval_ms, c.prefs.bug_url.as_deref()), (2500, Some("https://bugs.example")));
        assert!(c.prefs.launch.requires_keypair && c.prefs.launch.create_volume, "unset keys keep their default");
        assert_eq!(c.prefs.launch.flavor_sort, crate::prefs::FlavorSort::Vcpus);
        assert_eq!(c.prefs.network.allowed_private_subnet_cidr.ipv4, ["10.0.0.0/8"]);
        assert_eq!((c.prefs.images.default_visibility.as_str(), c.clouds_yaml_name.as_str()), ("shared", "acme"));
        assert_eq!((c.client_ip_header.unwrap().as_str(), c.trusted_proxies), ("x-forwarded-for", 2));
        let console = &c.prefs.console;
        assert_eq!((console.enabled, console.kind, console.log_length), (true, crate::prefs::ConsoleType::Spice, 35));
        assert!(parse("[key_manager]\nallow_payload = true\n").unwrap().key_manager_payload);
        assert_eq!(parse("").unwrap().lookup_ttl.as_secs(), 30, "name lookups cached 30 s by default (roadmap 66)");
        assert!(parse("[cache]\nlookup_seconds = 0\n").unwrap().lookup_ttl.is_zero(), "0 turns the cache off");
    }

    #[test]
    fn invalid_horizon_style_settings_stop_start_up() {
        let err = |t: &str| parse(t).unwrap_err();
        assert!(err("interface = \"private\"\n").contains("[keystone] interface"));
        assert!(err("[tls]\nca_file = \"/nonexistent/ca.pem\"\n").contains("[tls] ca_file"));
        assert!(err("[session]\ntimeout_minutes = 0\n").contains("timeout_minutes"));
        assert!(err("[login]\ndomain_hidden = true\ndomain_choices = [\"A\"]\n").contains("cannot be used together"));
        assert!(err("[login]\nmessage_file = \"/nonexistent/motd\"\n").contains("[login] message_file"));
        assert!(err("[ui]\ndefault_theme = \"blue\"\n").contains("default_theme"));
        assert!(err("[ui]\ndefault_language = \"fr\"\n").contains("default_language"));
        assert!(err("[ui]\npoll_interval_ms = 10\n").contains("poll_interval_ms"));
        assert!(err("[launch]\ndisable_image = true\ndisable_instance_snapshot = true\ndisable_volume = true\ndisable_volume_snapshot = true\n").contains("boot source"));
        assert!(err("[launch]\nflavor_sort = \"price\"\n").contains("flavor_sort"), "unknown enum value");
        assert!(err("[network]\nallowed_private_subnet_cidr = { ipv4 = [\"fd00::/8\"] }\n").contains("is not ipv4"));
        assert!(err("[network]\ndefault_dns_nameservers = [\"dns.example\"]\n").contains("not an IP address"));
        assert!(err("[network]\nipv6 = false\n").contains("unknown field"));
        assert!(err("[images]\ndisk_formats = []\n").contains("disk_formats"));
        assert!(err("[images]\nreserved_properties = 1\n").contains("reserved_properties"), "must be a list");
        assert!(err("[images]\nmax_upload_gb = 0\n").contains("max_upload_gb"));
        assert!(err("[images]\nupload = \"ftp\"\n").contains("upload"), "only off, proxy or direct");
        assert!(err("[console]\nlog_length = 0\n").contains("log_length"));
        assert!(err("[console]\ntype = \"rdp\"\n").contains("rdp"));
        assert!(err("[images]\ndefault_visibility = \"everyone\"\n").contains("default_visibility"));
        assert!(err("[log]\nclient_ip_header = \"bad header\"\n").contains("client_ip_header"));
        assert!(err("[ui]\nbug_url = \"ftp://x\"\n").contains("bug_url"));
    }

    #[test]
    fn the_login_message_is_read_from_its_file() {
        let t = Tmp::new("motd");
        let f = t.file("motd.txt", "  Maintenance on Sunday 02:00-04:00 UTC.\n");
        let c = parse(&format!("[login]\nmessage_file = {:?}\n", f.display().to_string())).unwrap();
        assert_eq!(c.login.message.as_deref(), Some("Maintenance on Sunday 02:00-04:00 UTC."));
    }

    #[test]
    fn the_shipped_example_is_valid_and_lists_the_defaults() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/moonlight.conf.example")).unwrap();
        let c = toml::from_str::<Settings>(&text).unwrap().config().unwrap();
        // Everything but the cloud-specific values equals a file with only the URL.
        let d = parse("").unwrap();
        assert_eq!(c.prefs, d.prefs);
        assert_eq!(c.login, d.login);
        assert_eq!((c.interface, c.ca_file, c.verify_tls), (d.interface, d.ca_file, d.verify_tls));
        assert_eq!((c.session_timeout, c.token_margin, c.password_expiry_warning_days), (d.session_timeout, d.token_margin, d.password_expiry_warning_days));
        assert_eq!((c.default_theme, c.default_language, c.client_ip_header), (d.default_theme, d.default_language, d.client_ip_header));
        assert_eq!((c.help_url, c.change_password_url, c.clouds_yaml_name, c.clouds_yaml_profile), (d.help_url, d.change_password_url, d.clouds_yaml_name, d.clouds_yaml_profile));
        assert_eq!(c.writes, WriteMode::DryRun);
    }

    #[test]
    fn provider_network_types_and_ranges_are_horizons_settings() {
        let c = parse("[network]\nsupported_provider_types = [\"vxlan\", \"vlan\"]\n[network.segmentation_id_range]\nvlan = [1000, 1999]\n").unwrap();
        assert_eq!(c.prefs.network.provider_types(), [("vxlan".to_string(), Some((1, (1 << 24) - 1))), ("vlan".to_string(), Some((1000, 1999)))], "listed order, narrowed range");
        assert_eq!(parse("").unwrap().prefs.network.provider_types().len(), crate::model::PROVIDER_NETWORK_TYPES.len(), "\"*\" by default");
        for (bad, why) in [
            ("[network]\nsupported_provider_types = [\"opflex\"]\n", "unknown type"),
            ("[network]\nsupported_provider_types = []\n", "cannot be empty"),
            ("[network.segmentation_id_range]\nflat = [1, 2]\n", "has no segmentation ID"),
            ("[network.segmentation_id_range]\nvlan = [10, 5000]\n", "within 1–4094"),
            ("[network.segmentation_id_range]\nvlan = [20, 10]\n", "ordered"),
        ] {
            let e = parse(bad).unwrap_err();
            assert!(e.contains(why), "{bad}: {e}");
        }
    }

    #[test]
    fn plugins_are_opt_in_known_and_settable_from_the_environment() {
        let ok = |enabled: &[&str]| Settings {
            keystone: Keystone { url: Some("https://k".into()), ..Default::default() },
            plugins: Plugins { enabled: enabled.iter().map(|s| s.to_string()).collect() },
            ..Default::default()
        };
        assert!(ok(&[]).config().unwrap().plugins.is_empty(), "none unless listed");
        assert_eq!(ok(&["orchestration", "load-balancer", "orchestration"]).config().unwrap().plugins, [Plugin::Orchestration, Plugin::LoadBalancer]);
        assert!(ok(&["loadbalancer"]).config().unwrap_err().contains("unknown plugin \"loadbalancer\""));
        let t = Tmp::new("plugins");
        let f = t.file("m.conf", "[plugins]\nenabled = [\"orchestration\"]\n");
        let env: HashMap<&str, &str> = [("MOONLIGHT_PLUGINS", " load-balancer, ,orchestration ")].into();
        let s = load(std::slice::from_ref(&f), &|k| env.get(k).map(|v| v.to_string())).unwrap();
        assert_eq!(s.plugins.enabled, ["load-balancer", "orchestration"], "the environment replaces the list");
        assert_eq!(load(&[f], &no_env).unwrap().plugins.enabled, ["orchestration"]);
    }

    #[test]
    fn websso_is_off_by_default_and_validated_when_on() {
        let c = parse("").unwrap();
        assert_eq!((c.websso, c.login.websso_choices.len()), (None, 0), "password sign-in only by default");
        let off = parse("[websso]\nchoices = [{ id = \"bad id\" }]\n").unwrap();
        assert_eq!(off.websso, None, "the section is ignored while disabled");
        let on = |rest: &str| parse(&format!("[session]\npublic_url = \"https://cloud.example.com/x\"\n[websso]\nenabled = true\n{rest}"));
        let full = on(concat!(
            "choices = [{ id = \"credentials\" }, { id = \"oidc\", label = \" OpenID Connect \" }, { id = \"acme\", label = \"ACME\" }]\n",
            "initial_choice = \"oidc\"\n",
            "keystone_url = \"https://keystone.example.com:5000/v3\"\n",
            "logout_url = \"https://idp.example.com/logout\"\n",
            "[websso.idp_mapping]\nacme = { idp = \"acme\", protocol = \"openid\" }\n",
        ))
        .unwrap();
        let ids: Vec<_> = full.login.websso_choices.iter().map(|c| (c.id.as_str(), c.label.as_str())).collect();
        assert_eq!(ids, [("credentials", ""), ("oidc", "OpenID Connect"), ("acme", "ACME")]);
        assert_eq!(full.login.websso_initial, "oidc");
        let ws = full.websso.unwrap();
        assert_eq!(ws.logout_url.as_deref(), Some("https://idp.example.com/logout"));
        assert_eq!(
            ws.start_url("acme").unwrap(),
            "https://keystone.example.com:5000/v3/auth/OS-FEDERATION/identity_providers/acme/protocols/openid/websso?origin=https%3A%2F%2Fcloud.example.com%2Fauth%2Fwebsso",
            "origin = public_url's origin + /auth/websso; [websso] keystone_url wins"
        );
        let short = on("choices = [{ id = \"saml2\" }]\n").unwrap();
        assert_eq!(short.login.websso_initial, "saml2", "the first choice when initial_choice is unset");
        assert!(short.websso.unwrap().start_url("saml2").unwrap().starts_with("https://k/v3/auth/OS-FEDERATION/websso/saml2?"), "[keystone] url by default");
        let err = |rest: &str| on(rest).unwrap_err();
        assert!(parse("[websso]\nenabled = true\nchoices = [{ id = \"oidc\" }]\n").unwrap_err().contains("public_url"), "no origin to send the token back to");
        assert!(err("").contains("at least one federation protocol"));
        assert!(err("choices = [{ id = \"credentials\" }]\n").contains("at least one federation protocol"));
        assert!(err("choices = [{ id = \"oidc\" }, { id = \"oidc\" }]\n").contains("twice"));
        assert!(err("choices = [{ id = \"o/idc\" }]\n").contains("not a valid ID"));
        assert!(err("choices = [{ id = \"oidc\" }]\ninitial_choice = \"saml2\"\n").contains("initial_choice"));
        assert!(err("choices = [{ id = \"oidc\" }]\nkeystone_url = \"ftp://k\"\n").contains("keystone_url"));
        assert!(err("choices = [{ id = \"oidc\" }]\n[websso.idp_mapping]\ncredentials = { idp = \"a\", protocol = \"b\" }\n").contains("not a federation choice"));
        assert!(err("choices = [{ id = \"oidc\" }]\n[websso.idp_mapping]\noidc = { idp = \" \", protocol = \"b\" }\n").contains("cannot be empty"));
    }

    #[test]
    fn url_settings_are_optional_but_must_be_http() {
        assert_eq!(url_setting("X", None), Ok(None));
        assert_eq!(url_setting("X", Some("  ")), Ok(None));
        assert_eq!(url_setting("X", Some(" https://help.example/ ")), Ok(Some("https://help.example/".into())));
        assert!(url_setting("X", Some("javascript:alert(1)")).is_err());
        assert!(url_setting("X", Some("https://a b")).is_err());
    }

    #[test]
    fn the_cookie_key_is_never_printed() {
        let s = Session { cookie_key: Some("c2VjcmV0".into()), ..Default::default() };
        assert!(!format!("{s:?}").contains("c2VjcmV0"));
    }

    #[test]
    fn public_url_becomes_an_origin() {
        let o = |u| super::origin_of("u", u).unwrap();
        assert_eq!(o("https://Cloud.Example.com/dashboard/?x=1"), "https://cloud.example.com");
        assert_eq!(o("http://10.0.0.5:3000"), "http://10.0.0.5:3000");
        assert_eq!(o("https://cloud.example:443/"), "https://cloud.example", "default port dropped, as browsers do");
        assert_eq!(o("http://cloud.example:80"), "http://cloud.example");
        assert_eq!(o("https://admin:pw@CLOUD.example:8443/x"), "https://cloud.example:8443", "user info dropped");
        assert!(super::origin_of("u", "https://").unwrap_err().starts_with("u:"));
    }

}
