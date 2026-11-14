//! Octavia: load balancers, listeners, pools, members, monitors, L7 policies.

use super::*;

/// An Octavia load balancer, as on Horizon's *Load Balancers* list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoadBalancerRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub vip_address: Option<String>,
    pub availability_zone: Option<String>,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
    pub provider: Option<String>,
}

/// Listener and pool protocols offered. `HTTPS` passes TLS through to the members.
pub const LB_PROTOCOLS: [&str; 4] = ["HTTP", "HTTPS", "TCP", "UDP"];
/// HTTPS ended at the load balancer with a Barbican certificate:
/// listeners only (the pool is `HTTP`), offered with `needs::LB_TLS_TERMINATION`.
pub const LB_TLS_PROTOCOL: &str = "TERMINATED_HTTPS";
/// Octavia `lb_algorithm` values offered in the dialogs.
pub const LB_ALGORITHMS: [&str; 3] = ["ROUND_ROBIN", "LEAST_CONNECTIONS", "SOURCE_IP"];
/// Octavia `session_persistence.type` values; `APP_COOKIE` needs a cookie name.
pub const LB_PERSISTENCE: [&str; 3] = ["SOURCE_IP", "HTTP_COOKIE", "APP_COOKIE"];
/// Octavia health monitor `type` values; the HTTP fields apply to `HTTP`/`HTTPS` only.
pub const LB_MONITOR_TYPES: [&str; 6] = ["HTTP", "HTTPS", "PING", "TCP", "TLS-HELLO", "UDP-CONNECT"];
/// HTTP methods a health monitor may use.
pub const LB_HTTP_METHODS: [&str; 3] = ["GET", "HEAD", "POST"];

/// The pool protocol that goes with a listener's (Horizon pairs them the same way).
pub fn pool_protocol_for(listener: &str) -> &'static str {
    match listener {
        "HTTPS" => "HTTPS",
        "TCP" => "TCP",
        "UDP" => "UDP",
        _ => "HTTP",
    }
}

/// Rejects a value outside a fixed set (an Octavia protocol, an Aodh severity, a key type, …),
/// naming `what` and the allowed values: the server re-checks what the dialog's select offered.
/// Shared by the other `model` specs.
pub(super) fn one_of(v: &str, allowed: &[&str], what: &str) -> Result<(), String> {
    if allowed.contains(&v) {
        Ok(())
    } else {
        Err(format!("{what} must be one of {} (got {v})", allowed.join(", ")))
    }
}

/// A listener to create (wizard step or *Create Listener*).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbListenerSpec {
    pub name: Option<String>,
    pub description: Option<String>,
    pub protocol: String,
    pub port: u16,
    /// `None` = unlimited.
    pub connection_limit: Option<u32>,
    /// The *Advanced* fields.
    #[serde(default)]
    pub options: LbListenerOptions,
    /// `TERMINATED_HTTPS` only: the Barbican container or secret URL of the
    /// certificate, required.
    #[serde(default)]
    pub default_tls_container_ref: Option<String>,
    /// `TERMINATED_HTTPS` only: more certificates, chosen by the name the client
    /// asks for (SNI).
    #[serde(default)]
    pub sni_container_refs: Vec<String>,
    /// *Admin State Up*; only `false` is sent.
    #[serde(default = "yes")]
    pub admin_up: bool,
}

/// Headers Octavia can add to requests sent to the members (`insert_headers`),
/// as octavia-dashboard offers them; HTTP listeners only.
pub const LB_INSERT_HEADERS: [&str; 3] = ["X-Forwarded-For", "X-Forwarded-Port", "X-Forwarded-Proto"];
/// Largest timeout, ms: Octavia takes up to a year, but the SDK's builders
/// take an `i32` (about 24.8 days).
pub const LB_TIMEOUT_MAX: u32 = i32::MAX as u32;

/// A listener's *Advanced* options, as octavia-dashboard has them.
/// `None` / empty = Octavia's default: not sent on create, and on edit sent as
/// `null` / `[]` / `{}`, which Octavia takes as "back to the default".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LbListenerOptions {
    /// Milliseconds (Octavia API 2.1; defaults 50 000, 0, 5 000 and 50 000).
    pub timeout_client_data: Option<u32>,
    pub timeout_tcp_inspect: Option<u32>,
    pub timeout_member_connect: Option<u32>,
    pub timeout_member_data: Option<u32>,
    /// Some of [`LB_INSERT_HEADERS`], each sent as `"true"`.
    pub insert_headers: Vec<String>,
    /// Client networks allowed in (Octavia 2.12); empty = everyone. `None`: not
    /// offered (older Octavia, which would refuse the key), so never sent.
    pub allowed_cidrs: Option<Vec<String>>,
    /// OpenSSL cipher string of a `TERMINATED_HTTPS` listener (Octavia 2.15);
    /// `None` = the cloud's default.
    pub tls_ciphers: Option<String>,
}

impl LbListenerOptions {
    /// Timeouts up to 7 days, headers known and for HTTP listeners only, CIDRs
    /// that parse.
    pub fn check(&self, protocol: &str) -> Result<(), String> {
        let timeouts = [
            self.timeout_client_data,
            self.timeout_member_connect,
            self.timeout_member_data,
            self.timeout_tcp_inspect,
        ];
        if timeouts.iter().flatten().any(|&t| t > LB_TIMEOUT_MAX) {
            return Err(format!("timeouts are at most {LB_TIMEOUT_MAX} ms"));
        }
        if !self.insert_headers.is_empty() && !lb_inserts_headers(protocol) {
            return Err("headers can only be inserted by HTTP and TERMINATED_HTTPS listeners".into());
        }
        if self.tls_ciphers.as_deref().is_some_and(|c| !c.trim().is_empty()) && protocol != LB_TLS_PROTOCOL {
            return Err("TLS ciphers are for TERMINATED_HTTPS listeners".into());
        }
        self.insert_headers
            .iter()
            .try_for_each(|h| one_of(h, &LB_INSERT_HEADERS, "header"))?;
        for c in self.allowed_cidrs.iter().flatten() {
            crate::prefs::Cidr::parse(c)?;
        }
        Ok(())
    }

    /// The cipher string, if one was typed.
    pub fn ciphers(&self) -> Option<&str> {
        self.tls_ciphers.as_deref().map(str::trim).filter(|c| !c.is_empty())
    }

    /// `insert_headers` as Octavia wants it: `{"X-Forwarded-For": "true", …}`.
    pub fn headers_map(&self) -> serde_json::Map<String, serde_json::Value> {
        self.insert_headers
            .iter()
            .map(|h| (h.clone(), serde_json::Value::String("true".into())))
            .collect()
    }
}

/// A pool to create; in the wizard the listener's default pool, which must be named
///.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbPoolSpec {
    pub name: Option<String>,
    pub description: Option<String>,
    pub protocol: String,
    pub algorithm: String,
    /// `SOURCE_IP`, `HTTP_COOKIE` or `APP_COOKIE` (with `cookie_name`).
    pub session_persistence: Option<String>,
    pub cookie_name: Option<String>,
    /// Re-encrypt to the members (Octavia 2.8).
    #[serde(default)]
    pub tls_enabled: bool,
    /// OpenSSL cipher string for `tls_enabled` pools (Octavia 2.15); `None` = the
    /// cloud's default.
    #[serde(default)]
    pub tls_ciphers: Option<String>,
    /// *Admin State Up*; only `false` is sent.
    #[serde(default = "yes")]
    pub admin_up: bool,
}

/// A pool member to create: an address on a subnet, its port and weight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbMemberSpec {
    pub name: Option<String>,
    pub address: String,
    pub port: u16,
    pub subnet_id: Option<String>,
    /// 0-256; `None` = Octavia's default (1).
    pub weight: Option<u16>,
    pub backup: bool,
    /// Health-check this address/port instead of the member's own.
    #[serde(default)]
    pub monitor_address: Option<String>,
    #[serde(default)]
    pub monitor_port: Option<u16>,
    #[serde(default = "yes")]
    pub admin_up: bool,
}

/// Serde default of every spec's `admin_up`: a spec without it is created up, as Octavia does.
fn yes() -> bool {
    true
}

/// One row of the members editor, as typed: numbers stay text
/// until submit, so a half-typed port isn't lost. The field's value is a JSON
/// list of these.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MemberRow {
    pub address: String,
    /// Empty = Octavia uses the load balancer's VIP subnet.
    #[serde(default)]
    pub subnet_id: String,
    /// Shown for rows whose address isn't typed (instance addresses, existing members).
    #[serde(default)]
    pub subnet_name: String,
    /// The instance the address belongs to, when picked from *Available Instances*.
    #[serde(default)]
    pub label: Option<String>,
    /// An *external member*: address and subnet are typed in the row. Instance
    /// addresses and existing members keep theirs (changing an existing member's
    /// address would replace it).
    #[serde(default)]
    pub typed: bool,
    #[serde(default)]
    pub name: String,
    pub port: String,
    /// Empty = Octavia's default (1).
    #[serde(default)]
    pub weight: String,
    #[serde(default)]
    pub backup: bool,
    #[serde(default = "yes")]
    pub admin_up: bool,
    #[serde(default)]
    pub monitor_address: String,
    #[serde(default)]
    pub monitor_port: String,
}

impl MemberRow {
    /// A row for an existing member, so *Edit Members* sends it back unchanged.
    pub fn from_member(m: &LbMember) -> Self {
        MemberRow {
            address: m.address.clone(),
            subnet_id: m.subnet.as_ref().map(|s| s.id.clone()).unwrap_or_default(),
            subnet_name: m.subnet.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
            label: None,
            typed: false,
            // `LbMember.name` falls back to the ID for display; don't send that as a name.
            name: if m.name == m.id { String::new() } else { m.name.clone() },
            port: m.port.map(|p| p.to_string()).unwrap_or_default(),
            weight: m.weight.map(|w| w.to_string()).unwrap_or_else(|| "1".into()),
            backup: m.backup,
            admin_up: m.admin_up,
            monitor_address: m.monitor_address.clone().unwrap_or_default(),
            monitor_port: m.monitor_port.map(|p| p.to_string()).unwrap_or_default(),
        }
    }

    /// The member to send, checked ([`LbMemberSpec::check`]).
    pub fn spec(&self) -> Result<LbMemberSpec, String> {
        let address = self.address.trim();
        let port_of = |s: &str, what: &str| -> Result<Option<u16>, String> {
            let s = s.trim();
            if s.is_empty() {
                return Ok(None);
            }
            s.parse::<u16>()
                .map(Some)
                .map_err(|_| format!("{address}: the {what} must be 1-65535"))
        };
        let weight = match self.weight.trim() {
            "" => None,
            w => Some(
                w.parse::<u16>()
                    .map_err(|_| format!("{address}: the weight must be 0-256"))?,
            ),
        };
        let nonempty = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
        // Horizon requires a subnet for an external member; ours too.
        if self.typed && self.subnet_id.trim().is_empty() {
            return Err(format!("{address}: pick the member's subnet"));
        }
        let spec = LbMemberSpec {
            name: nonempty(&self.name),
            address: address.to_string(),
            port: port_of(&self.port, "port")?.ok_or_else(|| format!("{address}: the port is required"))?,
            subnet_id: nonempty(&self.subnet_id),
            weight,
            backup: self.backup,
            monitor_address: nonempty(&self.monitor_address),
            monitor_port: port_of(&self.monitor_port, "monitor port")?,
            admin_up: self.admin_up,
        };
        spec.check()?;
        Ok(spec)
    }
}

/// The members editor's value as members to send: every row checked, and no
/// address/port twice (Octavia's key for a member).
pub fn member_specs(json: &str) -> Result<Vec<LbMemberSpec>, String> {
    let rows: Vec<MemberRow> = if json.trim().is_empty() {
        vec![]
    } else {
        serde_json::from_str(json).map_err(|e| e.to_string())?
    };
    let specs = rows.iter().map(MemberRow::spec).collect::<Result<Vec<_>, _>>()?;
    unique_members(&specs)?;
    Ok(specs)
}

/// Octavia rejects two members with the same address and port in one pool; catch it before the
/// request, naming the duplicate.
fn unique_members(specs: &[LbMemberSpec]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    match specs.iter().find(|m| !seen.insert((m.address.as_str(), m.port))) {
        Some(m) => Err(format!("{}:{} is listed twice", m.address, m.port)),
        None => Ok(()),
    }
}

/// Existing members that a batch update with `keep` deletes: Octavia matches
/// members by address and port, and removes those left out.
pub fn removed_members<'a>(existing: &'a [LbMember], keep: &[LbMemberSpec]) -> Vec<&'a LbMember> {
    existing
        .iter()
        .filter(|m| {
            !keep
                .iter()
                .any(|k| k.address == m.address && m.port == Some(i64::from(k.port)))
        })
        .collect()
}

/// A health monitor to create; Octavia's rules are in [`LbMonitorSpec::check`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbMonitorSpec {
    pub name: Option<String>,
    /// One of [`LB_MONITOR_TYPES`]; can't change after create.
    pub kind: String,
    /// Seconds between checks.
    pub delay: u32,
    /// Seconds a check may take, at most `delay`.
    pub timeout: u32,
    /// Successful checks before a member is `ONLINE` again (1-10).
    pub max_retries: u8,
    /// Failed checks before it is `ERROR` (1-10); `None` = Octavia's default (3).
    pub max_retries_down: Option<u8>,
    /// HTTP(S) monitors only.
    pub http_method: Option<String>,
    pub url_path: Option<String>,
    pub expected_codes: Option<String>,
    /// *Admin State Up*; only `false` is sent on create.
    #[serde(default = "yes")]
    pub admin_up: bool,
}

/// Horizon's *Create Load Balancer* wizard, sent as one fully populated request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbCreateSpec {
    pub name: Option<String>,
    pub description: Option<String>,
    pub vip_subnet_id: String,
    pub vip_address: Option<String>,
    pub flavor_id: Option<String>,
    /// An Octavia availability zone (`ChoiceSet::LbAvailabilityZones`); `None` = any.
    pub availability_zone: Option<String>,
    /// *Admin State Up* of the load balancer; only `false` is sent.
    #[serde(default = "yes")]
    pub admin_up: bool,
    pub listener: Option<LbListenerSpec>,
    /// Needs a listener.
    pub pool: Option<LbPoolSpec>,
    /// Need a pool.
    pub members: Vec<LbMemberSpec>,
    pub monitor: Option<LbMonitorSpec>,
}

impl LbListenerSpec {
    /// Octavia's rules for a listener: known protocol, port 1-65535, its
    /// options (`LbListenerOptions::check`) and certificates (`check_listener_tls`).
    pub fn check(&self) -> Result<(), String> {
        if self.protocol != LB_TLS_PROTOCOL {
            one_of(&self.protocol, &LB_PROTOCOLS, "listener protocol")?;
        }
        if self.port == 0 {
            return Err("the listener port must be 1-65535".into());
        }
        self.options.check(&self.protocol)?;
        check_listener_tls(
            &self.protocol,
            self.default_tls_container_ref.as_deref(),
            &self.sni_container_refs,
        )
    }
}

impl LbPoolSpec {
    /// Known protocol, algorithm and persistence; `APP_COOKIE` needs a cookie name.
    pub fn check(&self) -> Result<(), String> {
        one_of(&self.protocol, &LB_PROTOCOLS, "pool protocol")?;
        one_of(&self.algorithm, &LB_ALGORITHMS, "algorithm")?;
        if let Some(p) = self.session_persistence.as_deref() {
            one_of(p, &LB_PERSISTENCE, "session persistence")?;
            if p == "APP_COOKIE" && self.cookie_name.as_deref().is_none_or(|c| c.trim().is_empty()) {
                return Err("APP_COOKIE persistence needs a cookie name".into());
            }
        }
        check_pool_tls(&self.protocol, self.tls_enabled, self.tls_ciphers.as_deref())
    }
}

/// Whether a listener of this protocol can insert headers (Octavia: the HTTP
/// ones, where it sees the requests).
pub fn lb_inserts_headers(protocol: &str) -> bool {
    protocol == "HTTP" || protocol == LB_TLS_PROTOCOL
}

/// A listener's certificates: `TERMINATED_HTTPS` needs one, other
/// protocols take none, and each is a Barbican URL.
pub fn check_listener_tls(protocol: &str, default_ref: Option<&str>, sni: &[String]) -> Result<(), String> {
    let default_ref = default_ref.map(str::trim).filter(|r| !r.is_empty());
    if protocol != LB_TLS_PROTOCOL {
        return if default_ref.is_some() || !sni.is_empty() {
            Err("certificates are for TERMINATED_HTTPS listeners".into())
        } else {
            Ok(())
        };
    }
    let Some(d) = default_ref else {
        return Err("a TERMINATED_HTTPS listener needs a certificate".into());
    };
    for r in std::iter::once(d).chain(sni.iter().map(|s| s.trim())) {
        if !(r.starts_with("https://") || r.starts_with("http://")) {
            return Err(format!("not a Barbican reference: {r}"));
        }
    }
    Ok(())
}

/// Octavia's rules for re-encryption: not for UDP pools, and ciphers only when
/// it is on (a colon-separated OpenSSL list, at most 2048 characters).
pub fn check_pool_tls(protocol: &str, enabled: bool, ciphers: Option<&str>) -> Result<(), String> {
    if enabled && protocol == "UDP" {
        return Err("UDP pools can't re-encrypt to their members".into());
    }
    match ciphers.map(str::trim).filter(|c| !c.is_empty()) {
        Some(_) if !enabled => Err("TLS ciphers need TLS to the members".into()),
        Some(c) if c.len() > 2048 || c.contains(char::is_whitespace) => {
            Err("TLS ciphers are a colon-separated list, at most 2048 characters".into())
        }
        _ => Ok(()),
    }
}

impl LbMemberSpec {
    /// A valid IP address, port 1-65535 and weight 0-256; the monitor address
    /// an IP and its port 1-65535 when given.
    pub fn check(&self) -> Result<(), String> {
        self.address
            .trim()
            .parse::<std::net::IpAddr>()
            .map_err(|_| format!("not an IP address: {}", self.address))?;
        if self.port == 0 {
            return Err("the member port must be 1-65535".into());
        }
        if self.weight.is_some_and(|w| w > 256) {
            return Err("the weight must be 0-256".into());
        }
        if let Some(a) = self.monitor_address.as_deref() {
            a.trim()
                .parse::<std::net::IpAddr>()
                .map_err(|_| format!("not an IP address: {a}"))?;
        }
        if self.monitor_port == Some(0) {
            return Err("the monitor port must be 1-65535".into());
        }
        Ok(())
    }
}

impl LbMonitorSpec {
    /// Octavia's rules: delay ≥ timeout, retries 1-10, HTTP fields only for HTTP(S).
    pub fn check(&self) -> Result<(), String> {
        one_of(&self.kind, &LB_MONITOR_TYPES, "monitor type")?;
        if self.delay == 0 || self.timeout == 0 || self.timeout > self.delay {
            return Err("the delay and timeout must be at least 1 s, and the timeout not longer than the delay".into());
        }
        let retries = |n: u8| (1..=10).contains(&n);
        if !retries(self.max_retries) || self.max_retries_down.is_some_and(|n| !retries(n)) {
            return Err("retries must be 1-10".into());
        }
        let http = self.kind.starts_with("HTTP");
        if !http && (self.http_method.is_some() || self.url_path.is_some() || self.expected_codes.is_some()) {
            return Err("HTTP method, URL path and expected codes are for HTTP(S) monitors only".into());
        }
        if let Some(m) = self.http_method.as_deref() {
            one_of(m, &LB_HTTP_METHODS, "HTTP method")?;
        }
        if self.url_path.as_deref().is_some_and(|p| !p.starts_with('/')) {
            return Err("the URL path must start with /".into());
        }
        Ok(())
    }
}

impl LbCreateSpec {
    /// The whole wizard: subnet given, VIP address valid, every part checked, and
    /// nothing that needs a missing parent (a pool without a listener, members
    /// without a pool). The default pool must be named.
    pub fn check(&self) -> Result<(), String> {
        if self.vip_subnet_id.trim().is_empty() {
            return Err("a subnet is required".into());
        }
        if let Some(a) = self.vip_address.as_deref() {
            a.trim()
                .parse::<std::net::IpAddr>()
                .map_err(|_| format!("not an IP address: {a}"))?;
        }
        if let Some(l) = &self.listener {
            l.check()?;
        }
        match (&self.listener, &self.pool) {
            (None, Some(_)) => return Err("a pool needs a listener".into()),
            // Octavia's single-call create rejects an unnamed default pool.
            (_, Some(p)) if p.name.as_deref().is_none_or(|n| n.trim().is_empty()) => {
                return Err("the pool needs a name".into());
            }
            (_, Some(p)) => p.check()?,
            _ => {}
        }
        if self.pool.is_none() && (!self.members.is_empty() || self.monitor.is_some()) {
            return Err("members and monitors need a pool".into());
        }
        self.members.iter().try_for_each(LbMemberSpec::check)?;
        unique_members(&self.members)?;
        if let Some(m) = &self.monitor {
            m.check()?;
        }
        Ok(())
    }
}

/// A resource referenced by ID, with its name when known (else the ID).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ref {
    pub id: String,
    pub name: String,
}

/// A listener of a load balancer (detail page, *Listeners* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbListener {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub protocol: String,
    pub port: Option<i64>,
    pub default_pool: Option<Ref>,
    /// `-1` (unlimited) → `None`.
    pub connection_limit: Option<i64>,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
    /// The *Advanced* options, for the edit dialog and the listener's row.
    #[serde(default)]
    pub options: LbListenerOptions,
    /// `TERMINATED_HTTPS`: the certificate and the SNI ones (Barbican URLs).
    #[serde(default)]
    pub default_tls_container_ref: Option<String>,
    #[serde(default)]
    pub sni_container_refs: Vec<String>,
}

/// A pool of a load balancer (detail page, *Pools* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbPool {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub algorithm: String,
    /// `SOURCE_IP`, `HTTP_COOKIE`, `APP_COOKIE (name)`.
    pub session_persistence: Option<String>,
    /// The same, split, for the edit dialog.
    #[serde(default)]
    pub persistence_type: Option<String>,
    #[serde(default)]
    pub cookie_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub listeners: Vec<Ref>,
    pub members: usize,
    pub monitor: Option<Ref>,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
    /// Re-encrypts to the members (Octavia 2.8), with these ciphers.
    #[serde(default)]
    pub tls_enabled: bool,
    #[serde(default)]
    pub tls_ciphers: Option<String>,
}

/// A member of a pool (detail page, *Members* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbMember {
    pub id: String,
    pub name: String,
    pub pool: Ref,
    pub address: String,
    pub port: Option<i64>,
    pub weight: Option<i64>,
    pub subnet: Option<Ref>,
    pub backup: bool,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
    /// Where it is health-checked instead of `address`/`port` (`None` = itself).
    #[serde(default)]
    pub monitor_address: Option<String>,
    #[serde(default)]
    pub monitor_port: Option<i64>,
}

/// An L7 policy of one of the load balancer's listeners (detail page, *L7
/// Policies* tab), with its rules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbL7Policy {
    pub id: String,
    /// The name, else the ID.
    pub name: String,
    pub description: Option<String>,
    pub listener: Ref,
    /// One of [`LB_L7_ACTIONS`] (or another Octavia action, shown as is).
    pub action: String,
    /// 1 = evaluated first.
    pub position: Option<i64>,
    pub redirect_pool: Option<Ref>,
    pub redirect_url: Option<String>,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
    pub rules: Vec<LbL7Rule>,
}

/// A rule of an L7 policy: the policy applies when all its rules match.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbL7Rule {
    pub id: String,
    /// `type`: one of [`LB_L7_RULE_TYPES`] (others shown as is).
    pub rule_type: String,
    pub compare_type: String,
    /// The header or cookie name (`HEADER`, `COOKIE`).
    pub key: Option<String>,
    pub value: String,
    pub invert: bool,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
}

/// L7 policy actions octavia-dashboard offers.
pub const LB_L7_ACTIONS: [&str; 3] = ["REDIRECT_TO_POOL", "REDIRECT_TO_URL", "REJECT"];
/// L7 rule types octavia-dashboard offers; `HEADER` and `COOKIE` need a key.
pub const LB_L7_RULE_TYPES: [&str; 5] = ["HOST_NAME", "PATH", "FILE_TYPE", "HEADER", "COOKIE"];
/// How an L7 rule compares its value; `FILE_TYPE` takes `EQUAL_TO` or `REGEX` only.
pub const LB_L7_COMPARE_TYPES: [&str; 5] = ["REGEX", "STARTS_WITH", "ENDS_WITH", "CONTAINS", "EQUAL_TO"];

/// An L7 policy to create or save.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbL7PolicySpec {
    pub name: Option<String>,
    pub description: Option<String>,
    pub action: String,
    /// `None` = last.
    pub position: Option<u32>,
    /// `REDIRECT_TO_POOL` only.
    pub redirect_pool_id: Option<String>,
    /// `REDIRECT_TO_URL` only.
    pub redirect_url: Option<String>,
    pub admin_up: bool,
}

impl LbL7PolicySpec {
    /// A known action with its target: a pool for `REDIRECT_TO_POOL`, an
    /// `http(s)://` URL for `REDIRECT_TO_URL`, neither for `REJECT`.
    pub fn check(&self) -> Result<(), String> {
        one_of(&self.action, &LB_L7_ACTIONS, "L7 action")?;
        let pool = self
            .redirect_pool_id
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty());
        let url = self.redirect_url.as_deref().map(str::trim).filter(|u| !u.is_empty());
        match self.action.as_str() {
            "REDIRECT_TO_POOL" if pool.is_none() => return Err("redirecting to a pool needs the pool".into()),
            "REDIRECT_TO_URL" => match url {
                Some(u) if u.starts_with("http://") || u.starts_with("https://") => {}
                _ => return Err("redirecting to a URL needs an http:// or https:// URL".into()),
            },
            _ => {}
        }
        if self.position == Some(0) {
            return Err("the position starts at 1".into());
        }
        Ok(())
    }

    /// The redirect target that goes with the action; the other one is `None`.
    pub fn targets(&self) -> (Option<&str>, Option<&str>) {
        fn t(v: &Option<String>) -> Option<&str> {
            v.as_deref().map(str::trim).filter(|x| !x.is_empty())
        }
        match self.action.as_str() {
            "REDIRECT_TO_POOL" => (t(&self.redirect_pool_id), None),
            "REDIRECT_TO_URL" => (None, t(&self.redirect_url)),
            _ => (None, None),
        }
    }
}

/// An L7 rule to create or save.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbL7RuleSpec {
    pub rule_type: String,
    pub compare_type: String,
    pub key: Option<String>,
    pub value: String,
    pub invert: bool,
    pub admin_up: bool,
}

impl LbL7RuleSpec {
    /// Known type and comparison, a value, a key exactly for `HEADER` and
    /// `COOKIE`, and `FILE_TYPE` compared with `EQUAL_TO` or `REGEX` (Octavia's rules).
    pub fn check(&self) -> Result<(), String> {
        one_of(&self.rule_type, &LB_L7_RULE_TYPES, "rule type")?;
        one_of(&self.compare_type, &LB_L7_COMPARE_TYPES, "compare type")?;
        if self.value.trim().is_empty() {
            return Err("the rule needs a value".into());
        }
        let keyed = matches!(self.rule_type.as_str(), "HEADER" | "COOKIE");
        let key = self.key.as_deref().is_some_and(|k| !k.trim().is_empty());
        if keyed && !key {
            return Err(format!(
                "a {} rule needs a key (the {} name)",
                self.rule_type,
                self.rule_type.to_lowercase()
            ));
        }
        if !keyed && key {
            return Err("only HEADER and COOKIE rules take a key".into());
        }
        if self.rule_type == "FILE_TYPE" && !matches!(self.compare_type.as_str(), "EQUAL_TO" | "REGEX") {
            return Err("FILE_TYPE rules compare with EQUAL_TO or REGEX".into());
        }
        Ok(())
    }
}

/// A health monitor (detail page, *Monitors* tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LbMonitor {
    pub id: String,
    pub name: String,
    /// `HTTP`, `HTTPS`, `PING`, `TCP`, `TLS-HELLO`, `UDP-CONNECT`, `SCTP`.
    pub kind: String,
    pub delay: Option<i64>,
    pub timeout: Option<i64>,
    pub max_retries: Option<i64>,
    pub max_retries_down: Option<i64>,
    /// HTTP(S) checks: `GET /healthz → 200`.
    pub http_check: Option<String>,
    /// The same, split, for the edit dialog.
    #[serde(default)]
    pub http_method: Option<String>,
    #[serde(default)]
    pub url_path: Option<String>,
    #[serde(default)]
    pub expected_codes: Option<String>,
    pub pools: Vec<Ref>,
    pub operating_status: String,
    pub provisioning_status: String,
    pub admin_up: bool,
}

/// Everything the load balancer detail page shows, fetched in two rounds of GETs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoadBalancerDetail {
    pub row: LoadBalancerRow,
    pub project_id: String,
    pub flavor_id: Option<String>,
    pub vip_port_id: Option<String>,
    pub vip_network: Option<Ref>,
    pub vip_subnet: Option<Ref>,
    pub floating_ip: Option<String>,
    /// Its ID, to disassociate it.
    #[serde(default)]
    pub floating_ip_id: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub listeners: Vec<LbListener>,
    pub pools: Vec<LbPool>,
    pub members: Vec<LbMember>,
    pub monitors: Vec<LbMonitor>,
    /// Of every listener, by listener then position.
    #[serde(default)]
    pub l7policies: Vec<LbL7Policy>,
}

/// Octavia's transitional states (`PENDING_CREATE`, `PENDING_UPDATE`,
/// `PENDING_DELETE`): the pages refetch until they end.
pub fn lb_pending(provisioning_status: &str) -> bool {
    provisioning_status.starts_with("PENDING")
}

impl LoadBalancerDetail {
    /// Whether the load balancer or any of its parts is still being changed.
    pub fn busy(&self) -> bool {
        let p = lb_pending;
        p(&self.row.provisioning_status)
            || self.listeners.iter().any(|x| p(&x.provisioning_status))
            || self.pools.iter().any(|x| p(&x.provisioning_status))
            || self.members.iter().any(|x| p(&x.provisioning_status))
            || self.monitors.iter().any(|x| p(&x.provisioning_status))
            || self
                .l7policies
                .iter()
                .any(|x| p(&x.provisioning_status) || x.rules.iter().any(|r| p(&r.provisioning_status)))
    }
}

#[cfg(test)]
mod member_rows_tests {
    use super::*;

    fn row(address: &str, port: &str) -> MemberRow {
        MemberRow {
            address: address.into(),
            subnet_id: "sub-1".into(),
            port: port.into(),
            weight: "1".into(),
            admin_up: true,
            ..Default::default()
        }
    }

    #[test]
    fn rows_become_members_with_their_own_port_and_monitor() {
        let mut ext = row(" 192.0.2.10 ", "8443");
        ext.monitor_address = "192.0.2.11".into();
        ext.monitor_port = "9000".into();
        ext.backup = true;
        ext.weight = String::new();
        let json = serde_json::to_string(&[row("10.0.0.5", "80"), ext]).unwrap();
        let m = member_specs(&json).unwrap();
        assert_eq!(
            (m[0].port, m[0].weight, m[0].monitor_address.clone()),
            (80, Some(1), None)
        );
        assert_eq!(m[1].address, "192.0.2.10", "trimmed");
        assert_eq!(
            (m[1].port, m[1].weight, m[1].backup),
            (8443, None, true),
            "empty weight = Octavia's default"
        );
        assert_eq!(
            (m[1].monitor_address.as_deref(), m[1].monitor_port),
            (Some("192.0.2.11"), Some(9000))
        );
        assert!(member_specs("").unwrap().is_empty());
    }

    #[test]
    fn bad_rows_are_refused_with_the_address() {
        let bad = |r: MemberRow| member_specs(&serde_json::to_string(&[r]).unwrap()).unwrap_err();
        assert!(bad(row("10.0.0.5", "")).contains("10.0.0.5: the port is required"));
        assert!(
            bad(MemberRow {
                typed: true,
                subnet_id: String::new(),
                ..row("192.0.2.10", "80")
            })
            .contains("subnet"),
            "an external member needs its subnet"
        );
        assert!(bad(row("10.0.0.5", "70000")).contains("port must be 1-65535"));
        assert!(bad(row("web", "80")).contains("not an IP address"));
        assert!(
            bad(MemberRow {
                weight: "300".into(),
                ..row("10.0.0.5", "80")
            })
            .contains("weight")
        );
        assert!(
            bad(MemberRow {
                monitor_address: "x".into(),
                ..row("10.0.0.5", "80")
            })
            .contains("not an IP address: x")
        );
        let twice = serde_json::to_string(&[row("10.0.0.5", "80"), row("10.0.0.5", "80")]).unwrap();
        assert_eq!(member_specs(&twice).unwrap_err(), "10.0.0.5:80 is listed twice");
        let other_port = serde_json::to_string(&[row("10.0.0.5", "80"), row("10.0.0.5", "81")]).unwrap();
        assert_eq!(
            member_specs(&other_port).unwrap().len(),
            2,
            "same address, another port: allowed"
        );
    }

    fn member(id: &str, address: &str, port: i64) -> LbMember {
        LbMember {
            id: id.into(),
            name: id.into(),
            pool: Ref {
                id: "p".into(),
                name: "p".into(),
            },
            address: address.into(),
            port: Some(port),
            weight: Some(5),
            subnet: Some(Ref {
                id: "sub-1".into(),
                name: "s".into(),
            }),
            backup: false,
            operating_status: "ONLINE".into(),
            provisioning_status: "ACTIVE".into(),
            admin_up: true,
            monitor_address: Some("10.9.9.9".into()),
            monitor_port: Some(8080),
        }
    }

    #[test]
    fn existing_members_round_trip_and_removals_match_address_and_port() {
        let m = member("m1", "10.0.0.5", 80);
        let row = MemberRow::from_member(&m);
        assert!(
            !row.typed,
            "an existing member's address isn't editable: changing it would replace the member"
        );
        assert_eq!(row.subnet_name, "s", "shown by name");
        let spec = row.spec().unwrap();
        assert_eq!(spec.name, None, "a name that is only the ID fallback isn't sent");
        assert_eq!(
            (spec.weight, spec.monitor_address.as_deref(), spec.monitor_port),
            (Some(5), Some("10.9.9.9"), Some(8080)),
            "kept as they are"
        );
        let existing = [m, member("m2", "10.0.0.6", 80)];
        let keep = [spec.clone(), LbMemberSpec { port: 81, ..spec }];
        let gone: Vec<_> = removed_members(&existing, &keep)
            .iter()
            .map(|m| m.id.as_str())
            .collect();
        assert_eq!(gone, ["m2"]);
    }
}

#[cfg(test)]
mod lb_option_tests {
    use super::*;

    /// What Octavia would refuse, refused before sending.
    #[test]
    fn listener_options_and_pool_tls_follow_octavia_rules() {
        let ok = LbListenerOptions {
            timeout_client_data: Some(1000),
            insert_headers: vec!["X-Forwarded-For".into()],
            allowed_cidrs: Some(vec!["10.0.0.0/8".into(), "2001:db8::/32".into()]),
            ..Default::default()
        };
        assert!(ok.check("HTTP").is_ok());
        assert!(ok.check("TCP").is_err(), "headers only on HTTP listeners");
        let bad = |o: LbListenerOptions| o.check("HTTP").is_err();
        assert!(bad(LbListenerOptions {
            timeout_member_data: Some(LB_TIMEOUT_MAX + 1),
            ..Default::default()
        }));
        assert!(bad(LbListenerOptions {
            insert_headers: vec!["X-Evil".into()],
            ..Default::default()
        }));
        assert!(
            bad(LbListenerOptions {
                allowed_cidrs: Some(vec!["10.0.0.0".into()]),
                ..Default::default()
            }),
            "a CIDR needs its length"
        );
        assert!(check_pool_tls("HTTP", true, Some("ECDHE-RSA-AES128-GCM-SHA256:AES256")).is_ok());
        assert!(check_pool_tls("UDP", true, None).is_err());
        assert!(
            check_pool_tls("HTTP", false, Some("AES256")).is_err(),
            "ciphers need TLS"
        );
        assert!(
            check_pool_tls("HTTP", true, Some("AES256 AES128")).is_err(),
            "colon-separated"
        );
        assert!(check_pool_tls("HTTP", false, Some(" ")).is_ok(), "blank = none");
    }

    #[test]
    fn listener_certificates_belong_to_terminated_https() {
        let r = "https://kms/v1/containers/c1".to_string();
        assert!(check_listener_tls(LB_TLS_PROTOCOL, Some(&r), &[]).is_ok());
        assert!(
            check_listener_tls(LB_TLS_PROTOCOL, Some(" "), &[]).is_err(),
            "it needs one"
        );
        assert!(
            check_listener_tls(LB_TLS_PROTOCOL, Some("c1"), &[]).is_err(),
            "a Barbican URL"
        );
        assert!(check_listener_tls("HTTP", None, std::slice::from_ref(&r)).is_err());
        assert!(check_listener_tls("HTTP", None, &[]).is_ok());
        let o = LbListenerOptions {
            tls_ciphers: Some("AES256".into()),
            ..Default::default()
        };
        assert!(
            o.check("HTTP").is_err() && o.check(LB_TLS_PROTOCOL).is_ok(),
            "listener ciphers only when TLS ends there"
        );
        assert!(
            lb_inserts_headers(LB_TLS_PROTOCOL) && !lb_inserts_headers("HTTPS"),
            "HTTPS passes through: no headers"
        );
    }

    #[test]
    fn octavia_version_gates_the_options() {
        let c = |v| Capabilities {
            known: true,
            lb_mv: v,
            ..Default::default()
        };
        assert!(needs::LB_TIMEOUTS.met(&c(Some((2, 1)))));
        assert!(!needs::LB_ALLOWED_CIDRS.met(&c(Some((2, 11)))));
        assert!(needs::LB_POOL_CIPHERS.met(&c(Some((2, 27)))));
        assert!(needs::LB_POOL_TLS.met(&c(None)), "no version found: fail open");
    }

    #[test]
    fn url_import_needs_glance_web_download() {
        let c = |m: Option<&[&str]>| Capabilities {
            known: true,
            image_import: m.map(|l| l.iter().map(|s| s.to_string()).collect()),
            ..Default::default()
        };
        assert!(needs::IMAGE_URL_IMPORT.met(&c(Some(&["glance-direct", "web-download"]))));
        assert!(
            !needs::IMAGE_URL_IMPORT.met(&c(Some(&["glance-direct"]))),
            "Glance would reject the import"
        );
        assert!(!needs::IMAGE_URL_IMPORT.met(&c(Some(&[]))), "import disabled");
        assert!(needs::IMAGE_URL_IMPORT.met(&c(None)), "list unreadable: fail open");
    }
}

#[cfg(test)]
mod lb_busy_tests {
    use super::*;

    /// The detail page polls while any part is PENDING_*.
    #[test]
    fn a_pending_part_makes_the_load_balancer_busy() {
        let d: LoadBalancerDetail = serde_json::from_value(serde_json::json!({
            "row": {"id": "lb1", "name": "lb", "description": null, "vip_address": null, "availability_zone": null,
                    "operating_status": "ONLINE", "provisioning_status": "ACTIVE", "admin_up": true, "provider": null},
            "project_id": "p", "flavor_id": null, "vip_port_id": null, "vip_network": null, "vip_subnet": null,
            "floating_ip": null, "created": null, "updated": null, "listeners": [], "pools": [], "members": [], "monitors": [],
            "l7policies": [{"id": "p1", "name": "p", "description": null, "listener": {"id": "l", "name": "l"}, "action": "REJECT",
                            "position": 1, "redirect_pool": null, "redirect_url": null, "operating_status": "ONLINE",
                            "provisioning_status": "ACTIVE", "admin_up": true,
                            "rules": [{"id": "r", "rule_type": "PATH", "compare_type": "EQUAL_TO", "key": null, "value": "/",
                                       "invert": false, "operating_status": "OFFLINE", "provisioning_status": "PENDING_CREATE", "admin_up": true}]}]
        }))
        .unwrap();
        assert!(d.busy(), "a rule being created");
        assert!(!lb_pending("ACTIVE") && lb_pending("PENDING_DELETE"));
    }
}
