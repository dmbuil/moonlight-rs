//! Neutron FWaaS v2.

use super::*;

/// A FWaaS v2 firewall rule. `None` addresses, ports and protocol mean *any*.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirewallRuleRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// `tcp`, `udp`, `icmp`; `None` = any protocol.
    pub protocol: Option<String>,
    /// 4 or 6.
    pub ip_version: u8,
    /// An address or CIDR.
    pub source_ip: Option<String>,
    /// A port or a `first:last` range, as FWaaS stores it.
    pub source_port: Option<String>,
    pub destination_ip: Option<String>,
    pub destination_port: Option<String>,
    /// `allow`, `deny` or `reject`.
    pub action: String,
    pub enabled: bool,
    pub shared: bool,
    /// The policies that use it (FWaaS v2 lets a rule be in several), named
    /// from the policies the project can see, else their IDs.
    pub policies: Vec<Ref>,
    /// The project's own; another project's shared rule has no edit or delete.
    pub owned: bool,
}

/// A FWaaS v2 firewall policy: an ordered list of rules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirewallPolicyRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// In evaluation order, named from the rules the project can see.
    pub rules: Vec<Ref>,
    pub audited: bool,
    pub shared: bool,
    pub owned: bool,
}

/// A firewall policy's page: the policy and its rules in full, in evaluation order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirewallPolicyDetail {
    pub policy: FirewallPolicyRow,
    /// Rules the project can't see (another project's, not shared) are left out.
    pub rules: Vec<FirewallRuleRow>,
}

/// A port a firewall group protects, with the router or instance it belongs to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirewallPort {
    /// The port, by name (else ID).
    pub port: Ref,
    /// Its fixed IPs.
    pub ips: Vec<String>,
    /// `router` or `instance` (from `device_owner`), `None` when unknown.
    pub kind: Option<String>,
    /// The router or instance, by name when known.
    pub device: Option<Ref>,
}

impl FirewallPort {
    /// Where the device's page is (`None` for an unknown kind).
    pub fn device_href(&self) -> Option<String> {
        let d = self.device.as_ref()?;
        match self.kind.as_deref()? {
            "router" => Some(format!("/project/routers/{}", d.id)),
            "instance" => Some(format!("/project/instances/{}", d.id)),
            _ => None,
        }
    }
}

/// A FWaaS v2 firewall group: an ingress and an egress policy applied to ports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirewallGroupRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// `None`: that direction is not filtered.
    pub ingress: Option<Ref>,
    pub egress: Option<Ref>,
    /// Resolved to their router or instance on the detail page; the list
    /// only names the ports.
    pub ports: Vec<FirewallPort>,
    /// `ACTIVE`, `INACTIVE` (no ports, or neither policy), `DOWN`, `PENDING_CREATE` / `_UPDATE` / `_DELETE`, `ERROR`.
    pub status: String,
    pub admin_up: bool,
    pub shared: bool,
    pub owned: bool,
}

impl FirewallGroupRow {
    /// Neutron moves it through `PENDING_*` on its own: the list polls meanwhile.
    pub fn busy(&self) -> bool {
        self.status.starts_with("PENDING")
    }
    /// Neutron's per-project `default` group (neutron-fwaas matches it by name): only its
    /// ports can change, and only an admin can delete it (`FirewallGroupCannotRemoveDefault`,
    /// `FirewallDefaultObjectUpdateRestricted`).
    pub fn is_default(&self) -> bool {
        self.name == "default"
    }
    /// Can be deleted from the Project panel: the project's own, not `default`.
    pub fn deletable(&self) -> bool {
        self.owned && !self.is_default()
    }
}

impl FirewallPolicyRow {
    /// The default group's `default ingress` / `default egress` policies (matched by name, as
    /// neutron-fwaas does): Neutron refuses to update them; their rules can still change.
    pub fn is_default(&self) -> bool {
        matches!(self.name.as_str(), "default ingress" | "default egress")
    }
    /// Can be deleted from the Project panel: the project's own, not a default policy (the
    /// default group uses it, so Neutron would refuse anyway).
    pub fn deletable(&self) -> bool {
        self.owned && !self.is_default()
    }
}

/// What *Add Rule* and *Edit Rule* send. Empty strings mean *any*.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FirewallRuleSpec {
    pub name: String,
    pub description: String,
    /// `tcp`, `udp`, `icmp`, or `any` / empty.
    pub protocol: String,
    /// `allow`, `deny` or `reject`.
    pub action: String,
    /// 4 or 6.
    pub ip_version: u8,
    pub source_ip: String,
    pub source_port: String,
    pub destination_ip: String,
    pub destination_port: String,
    pub enabled: bool,
}

impl FirewallRuleSpec {
    /// The `firewall_rule` body Neutron expects, validated: protocol and action
    /// from Neutron's lists, ports (`22`, `8000:8080`) only for TCP or UDP, and
    /// addresses (an IP or a CIDR) of the rule's IP version. *Any* is `null`, so
    /// an update clears a field.
    pub fn body(&self) -> Result<serde_json::Value, String> {
        use serde_json::{Value, json};
        let protocol = match self.protocol.trim() {
            "" | "any" => None,
            p @ ("tcp" | "udp" | "icmp") => Some(p),
            other => return Err(format!("protocol must be tcp, udp, icmp or any (got {other})")),
        };
        if !matches!(self.action.as_str(), "allow" | "deny" | "reject") {
            return Err(format!("action must be allow, deny or reject (got {})", self.action));
        }
        if !matches!(self.ip_version, 4 | 6) {
            return Err(format!("IP version must be 4 or 6 (got {})", self.ip_version));
        }
        let addr = |a: &str| -> Result<Value, String> {
            let a = a.trim();
            if a.is_empty() {
                return Ok(Value::Null);
            }
            validate_ip_or_cidr(a, self.ip_version)?;
            Ok(json!(a))
        };
        // Neutron refuses ports on ICMP or *any* protocol rules.
        let port = |p: &str| -> Result<Value, String> {
            let p = p.trim();
            if p.is_empty() {
                return Ok(Value::Null);
            }
            if !matches!(protocol, Some("tcp" | "udp")) {
                return Err("ports need the TCP or UDP protocol".into());
            }
            validate_port_range(p)?;
            Ok(json!(p))
        };
        let mut r = serde_json::Map::new();
        r.insert("name".into(), json!(self.name.trim()));
        r.insert("description".into(), json!(self.description.trim()));
        r.insert("protocol".into(), json!(protocol));
        r.insert("action".into(), json!(self.action));
        r.insert("ip_version".into(), json!(self.ip_version));
        r.insert("source_ip_address".into(), addr(&self.source_ip)?);
        r.insert("source_port".into(), port(&self.source_port)?);
        r.insert("destination_ip_address".into(), addr(&self.destination_ip)?);
        r.insert("destination_port".into(), port(&self.destination_port)?);
        r.insert("enabled".into(), json!(self.enabled));
        Ok(json!({ "firewall_rule": r }))
    }
}

/// A port (`22`) or a range (`8000:8080`, low to high), 1-65535.
pub fn validate_port_range(p: &str) -> Result<(), String> {
    let one = |x: &str| x.trim().parse::<u16>().ok().filter(|n| *n > 0);
    let bad = || format!("not a port or a port range (22, 8000:8080): {p}");
    match p.split_once(':') {
        None => one(p).map(|_| ()).ok_or_else(bad),
        Some((a, b)) => match (one(a), one(b)) {
            (Some(a), Some(b)) if a <= b => Ok(()),
            _ => Err(bad()),
        },
    }
}

/// An IP address or a CIDR of IP version `version` (4 or 6).
pub fn validate_ip_or_cidr(a: &str, version: u8) -> Result<(), String> {
    use std::net::IpAddr;
    let (ip, len) = match a.split_once('/') {
        Some((ip, len)) => (ip, Some(len)),
        None => (a, None),
    };
    let ip: IpAddr = ip.parse().map_err(|_| format!("not an IP address or a CIDR: {a}"))?;
    let max = if ip.is_ipv4() { 32 } else { 128 };
    if let Some(len) = len {
        len.parse::<u8>()
            .ok()
            .filter(|l| *l <= max)
            .ok_or_else(|| format!("not an IP address or a CIDR: {a}"))?;
    }
    if (version == 4) != ip.is_ipv4() {
        return Err(format!("{a} is not an IPv{version} address"));
    }
    Ok(())
}

// --- Orchestration (Heat) ------------------------------------------
