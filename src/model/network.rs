//! Neutron: networks, routers, trunks, QoS, security groups, floating IPs, routes.

use super::*;

/// A row of the Networks list: the project's own, shared and external networks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetworkRow {
    pub id: String,
    pub name: String,
    /// "name cidr"
    pub subnets: Vec<String>,
    pub shared: bool,
    pub external: bool,
    pub status: String,
    pub admin_up: bool,
    pub availability_zones: Vec<String>,
    /// The session's project owns it: only then are there write actions
    /// (external and shared networks of other projects are read-only, as in Horizon).
    #[serde(default)]
    pub owned: bool,
    /// DHCP agents hosting it (Admin › Networks, Neutron `dhcp_agent_scheduler`);
    /// `None` on Project's list or without the extension.
    #[serde(default)]
    pub dhcp_agents: Option<usize>,
    /// `provider:*`, which Neutron shows to admins only; `None` otherwise.
    #[serde(default)]
    pub provider: Option<ProviderNetwork>,
}

/// A row of the Routers list (filtered by `tenant_id`: admins see every project's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouterRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub external_network: Option<String>,
    pub external_network_id: Option<String>,
    /// The gateway's `enable_snat`; `None` without a gateway.
    pub snat: Option<bool>,
    pub admin_up: bool,
    pub availability_zones: Vec<String>,
    /// `distributed` / `ha`: Neutron shows them to admins only, and only with
    /// the `dvr` / `l3-ha` extensions; `None` when absent (roadmap 89).
    #[serde(default)]
    pub distributed: Option<bool>,
    #[serde(default)]
    pub ha: Option<bool>,
}

/// A trunk's sub-port: a port carried on the parent's VLAN `segmentation_id`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subport {
    pub port_id: String,
    /// The port's name, else its ID.
    pub port_name: String,
    pub segmentation_type: String,
    pub segmentation_id: Option<i64>,
}

/// A Neutron trunk (`trunk` extension).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrunkRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// The parent port.
    pub parent: Ref,
    pub status: String,
    pub admin_up: bool,
    pub subports: Vec<Subport>,
}

// --- Firewall (FWaaS v2, roadmap 93) --------------------------------------------

/// A sub-port to add: `vlan` needs an ID (1-4094); `inherit` takes the
/// segmentation of the port's network and no ID.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubportSpec {
    pub port_id: String,
    pub segmentation_type: String,
    pub segmentation_id: Option<u16>,
}

impl SubportSpec {
    /// The `sub_ports` entry Neutron expects; validates the port and segmentation.
    pub fn body(&self) -> Result<serde_json::Value, String> {
        use serde_json::json;
        if self.port_id.trim().is_empty() {
            return Err("a sub-port needs a port".into());
        }
        match (self.segmentation_type.as_str(), self.segmentation_id) {
            ("vlan", Some(id)) if (1..=4094).contains(&id) => {
                Ok(json!({"port_id": self.port_id.trim(), "segmentation_type": "vlan", "segmentation_id": id}))
            }
            ("vlan", _) => Err("a VLAN sub-port needs a segmentation ID, 1-4094".into()),
            ("inherit", None) => Ok(json!({"port_id": self.port_id.trim(), "segmentation_type": "inherit"})),
            ("inherit", Some(_)) => Err("an inherit sub-port takes no segmentation ID".into()),
            (other, _) => Err(format!("segmentation type must be vlan or inherit (got {other})")),
        }
    }
}

/// Neutron QoS rule types, as in the API paths (`…/bandwidth_limit_rules`).
pub const QOS_RULE_TYPES: [&str; 4] = [
    "bandwidth_limit",
    "dscp_marking",
    "minimum_bandwidth",
    "minimum_packet_rate",
];
/// The DSCP marks Neutron accepts.
pub const DSCP_MARKS: [u8; 21] = [
    0, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 46, 48, 56,
];

/// One rule of a QoS policy, its type-specific fields already formatted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QosRule {
    pub id: String,
    /// One of [`QOS_RULE_TYPES`].
    pub kind: String,
    pub direction: Option<String>,
    pub max_kbps: Option<i64>,
    pub max_burst_kbps: Option<i64>,
    pub dscp_mark: Option<i64>,
    pub min_kbps: Option<i64>,
    pub min_kpps: Option<i64>,
}

/// A row of the Network QoS list (the project's policies and shared ones).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QosPolicyRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub shared: bool,
    pub is_default: bool,
    /// The project's own policy (shared ones of other projects are read-only).
    pub owned: bool,
    pub rules: Vec<QosRule>,
}

/// A rule to create or change; [`QosRuleSpec::body`] keeps only the fields of its type.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct QosRuleSpec {
    pub kind: String,
    pub direction: Option<String>,
    pub max_kbps: Option<u32>,
    pub max_burst_kbps: Option<u32>,
    pub dscp_mark: Option<u8>,
    pub min_kbps: Option<u32>,
    pub min_kpps: Option<u32>,
}

impl QosRuleSpec {
    /// Directions each type accepts (none for DSCP marking).
    pub fn directions(kind: &str) -> &'static [&'static str] {
        match kind {
            "bandwidth_limit" | "minimum_bandwidth" => &["egress", "ingress"],
            "minimum_packet_rate" => &["egress", "ingress", "any"],
            _ => &[],
        }
    }

    /// `{"<type>_rule": {...}}` with the fields Neutron expects for the type.
    pub fn body(&self) -> Result<serde_json::Value, String> {
        use serde_json::{Map, json};
        one_of(&self.kind, &QOS_RULE_TYPES, "rule type")?;
        let mut r = Map::new();
        let dirs = Self::directions(&self.kind);
        if let Some(d) = self.direction.as_deref() {
            if !dirs.contains(&d) {
                return Err(format!("direction {d} is not valid for {}", self.kind));
            }
            r.insert("direction".into(), json!(d));
        }
        let need = |v: Option<u32>, what: &str| {
            v.filter(|n| *n > 0)
                .ok_or_else(|| format!("{what} is required and must be above 0"))
        };
        match self.kind.as_str() {
            "bandwidth_limit" => {
                r.insert("max_kbps".into(), json!(need(self.max_kbps, "max_kbps")?));
                if let Some(b) = self.max_burst_kbps {
                    r.insert("max_burst_kbps".into(), json!(b));
                }
            }
            "dscp_marking" => {
                let m = self.dscp_mark.ok_or("dscp_mark is required")?;
                if !DSCP_MARKS.contains(&m) {
                    return Err(format!("{m} is not a valid DSCP mark"));
                }
                r.insert("dscp_mark".into(), json!(m));
            }
            "minimum_bandwidth" => {
                r.insert("min_kbps".into(), json!(need(self.min_kbps, "min_kbps")?));
            }
            _ => {
                r.insert("min_kpps".into(), json!(need(self.min_kpps, "min_kpps")?));
            }
        }
        Ok(json!({ format!("{}_rule", self.kind): r }))
    }
}

/// A row of the Security Groups list (the project's own, filtered by `tenant_id`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecurityGroupRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub rules: Vec<String>,
    /// Shared with the project through Neutron RBAC (`shared`).
    #[serde(default)]
    pub shared: bool,
}

/// A row of the Floating IPs list, with what it is associated to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloatingIpRow {
    pub id: String,
    pub ip: String,
    pub fixed_ip: Option<String>,
    /// Instance the port belongs to, when it is a Nova port.
    pub instance_id: Option<String>,
    pub instance_name: Option<String>,
    pub pool: String,
    /// The pool's network ID (`floating_network_id`): admin *Associate* offers
    /// only the ports it reaches (roadmap 89).
    #[serde(default)]
    pub pool_id: String,
    pub status: String,
    pub description: Option<String>,
    /// `dns-integration` extension; absent otherwise.
    pub dns_name: Option<String>,
    pub dns_domain: Option<String>,
}

/// A floating IP port forwarding rule (Neutron `floating-ip-port-forwarding`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortForwardingRow {
    pub id: String,
    pub protocol: String,
    /// `2222` or, with the port-ranges extension, `8080:8090`.
    pub external: String,
    pub internal_ip: String,
    pub internal: String,
    pub internal_port_id: String,
    /// The instance owning the internal port, when known.
    pub instance: Option<Ref>,
    pub description: Option<String>,
}

/// The floating IP detail page, with its port forwardings when supported.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloatingIpDetail {
    pub row: FloatingIpRow,
    pub port_id: Option<String>,
    pub router_id: Option<String>,
    pub created: Option<String>,
    pub forwardings: Vec<PortForwardingRow>,
}

/// A port or port range of a forwarding rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortSpec {
    One(u16),
    Range(u16, u16),
}

impl PortSpec {
    /// `22` or `8080:8090` (also `8080-8090`); ranges only when the cloud has
    /// `floating-ip-port-forwarding-port-ranges`.
    pub fn parse(s: &str, ranges: bool) -> Result<PortSpec, String> {
        let s = s.trim();
        let port = |p: &str| match p.trim().parse::<u16>() {
            Ok(n) if n > 0 => Ok(n),
            _ => Err(format!("not a port number (1-65535): {p}")),
        };
        match s.split_once([':', '-']) {
            None => Ok(PortSpec::One(port(s)?)),
            Some(_) if !ranges => Err("this cloud does not support port ranges".into()),
            Some((a, b)) => {
                let (a, b) = (port(a)?, port(b)?);
                match a.cmp(&b) {
                    std::cmp::Ordering::Less => Ok(PortSpec::Range(a, b)),
                    std::cmp::Ordering::Equal => Ok(PortSpec::One(a)),
                    std::cmp::Ordering::Greater => Err(format!("the range must go up: {s}")),
                }
            }
        }
    }

    /// How many ports it covers.
    pub fn count(self) -> u32 {
        match self {
            PortSpec::One(_) => 1,
            PortSpec::Range(a, b) => u32::from(b - a) + 1,
        }
    }
}

impl std::fmt::Display for PortSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortSpec::One(p) => write!(f, "{p}"),
            PortSpec::Range(a, b) => write!(f, "{a}:{b}"),
        }
    }
}

/// What a forwarding rule sends, already checked by [`PortForwardingSpec::ports`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortForwardingSpec {
    pub floating_ip_id: String,
    /// `tcp` or `udp`.
    pub protocol: String,
    pub internal_port_id: String,
    pub internal_ip: String,
    pub internal: String,
    pub external: String,
    pub description: Option<String>,
    /// The cloud has `floating-ip-port-forwarding-port-ranges`.
    pub ranges: bool,
}

impl PortForwardingSpec {
    /// Validated `(internal, external)`: both single ports, or ranges of the same size.
    pub fn ports(&self) -> Result<(PortSpec, PortSpec), String> {
        if !matches!(self.protocol.as_str(), "tcp" | "udp") {
            return Err(format!("protocol must be tcp or udp (got {})", self.protocol));
        }
        self.internal_ip
            .trim()
            .parse::<std::net::Ipv4Addr>()
            .map_err(|_| format!("not an IPv4 address: {}", self.internal_ip))?;
        let (i, e) = (
            PortSpec::parse(&self.internal, self.ranges)?,
            PortSpec::parse(&self.external, self.ranges)?,
        );
        if i.count() != e.count() {
            return Err(format!(
                "internal ({i}) and external ({e}) ports must cover the same number of ports"
            ));
        }
        Ok((i, e))
    }
}

/// Checks a static route: `destination` is a CIDR and `nexthop` an address of
/// the same IP version. Returns them trimmed. Neutron still checks that the
/// next hop is inside one of the router's subnets.
pub fn validate_route(destination: &str, nexthop: &str) -> Result<(String, String), String> {
    use std::net::IpAddr;
    let (dest, hop) = (destination.trim(), nexthop.trim());
    let (net, len) = dest.split_once('/').ok_or_else(|| format!("not a CIDR: {dest}"))?;
    let net: IpAddr = net.parse().map_err(|_| format!("not a CIDR: {dest}"))?;
    let len: u8 = len.parse().map_err(|_| format!("not a CIDR: {dest}"))?;
    if len > if net.is_ipv4() { 32 } else { 128 } {
        return Err(format!("not a CIDR: {dest}"));
    }
    let hop_ip: IpAddr = hop.parse().map_err(|_| format!("not an IP address: {hop}"))?;
    if hop_ip.is_ipv4() != net.is_ipv4() {
        return Err("destination and next hop must be the same IP version".into());
    }
    Ok((dest.to_string(), hop.to_string()))
}

#[cfg(test)]
mod route_tests {
    use super::validate_route;

    #[test]
    fn routes_need_a_cidr_and_a_next_hop_of_the_same_version() {
        assert_eq!(
            validate_route(" 10.9.0.0/16 ", "10.0.0.1"),
            Ok(("10.9.0.0/16".into(), "10.0.0.1".into()))
        );
        assert!(validate_route("2001:db8::/48", "2001:db8::1").is_ok());
        assert!(validate_route("10.9.0.0", "10.0.0.1").is_err());
        assert!(validate_route("10.9.0.0/33", "10.0.0.1").is_err());
        assert!(validate_route("10.9.0.0/16", "router").is_err());
        assert!(validate_route("10.9.0.0/16", "2001:db8::1").is_err());
    }
}

#[cfg(test)]
mod port_forwarding_tests {
    use super::*;

    fn spec(internal: &str, external: &str, ranges: bool) -> PortForwardingSpec {
        PortForwardingSpec {
            floating_ip_id: "f".into(),
            protocol: "tcp".into(),
            internal_port_id: "p".into(),
            internal_ip: "10.0.0.5".into(),
            internal: internal.into(),
            external: external.into(),
            description: None,
            ranges,
        }
    }

    #[test]
    fn ports_and_ranges_are_validated() {
        assert_eq!(PortSpec::parse(" 22 ", false), Ok(PortSpec::One(22)));
        assert_eq!(PortSpec::parse("8080-8090", true), Ok(PortSpec::Range(8080, 8090)));
        assert_eq!(PortSpec::parse("80:80", true), Ok(PortSpec::One(80)));
        assert!(PortSpec::parse("80:90", false).unwrap_err().contains("ranges"));
        assert!(
            PortSpec::parse("90:80", true).is_err()
                && PortSpec::parse("0", true).is_err()
                && PortSpec::parse("70000", true).is_err()
        );
        assert_eq!(PortSpec::Range(8080, 8090).to_string(), "8080:8090");
        assert_eq!(
            spec("22", "2222", false).ports(),
            Ok((PortSpec::One(22), PortSpec::One(2222)))
        );
        assert!(
            spec("8080:8081", "80:82", true)
                .ports()
                .unwrap_err()
                .contains("same number")
        );
        assert!(spec("22", "2222", false).ports().is_ok());
        assert!(
            PortForwardingSpec {
                protocol: "icmp".into(),
                ..spec("22", "22", false)
            }
            .ports()
            .is_err()
        );
        assert!(
            PortForwardingSpec {
                internal_ip: "fd00::5".into(),
                ..spec("22", "22", false)
            }
            .ports()
            .is_err(),
            "IPv4 only"
        );
    }
}

#[cfg(test)]
mod qos_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rule_bodies_keep_the_fields_of_their_type() {
        let bw = QosRuleSpec {
            kind: "bandwidth_limit".into(),
            direction: Some("ingress".into()),
            max_kbps: Some(10_000),
            max_burst_kbps: Some(1000),
            min_kbps: Some(5),
            ..Default::default()
        };
        assert_eq!(
            bw.body().unwrap(),
            json!({"bandwidth_limit_rule": {"direction": "ingress", "max_kbps": 10000, "max_burst_kbps": 1000}})
        );
        let dscp = QosRuleSpec {
            kind: "dscp_marking".into(),
            dscp_mark: Some(26),
            ..Default::default()
        };
        assert_eq!(dscp.body().unwrap(), json!({"dscp_marking_rule": {"dscp_mark": 26}}));
        assert!(
            QosRuleSpec {
                dscp_mark: Some(27),
                ..dscp.clone()
            }
            .body()
            .is_err(),
            "not a DSCP mark"
        );
        assert!(
            QosRuleSpec {
                direction: Some("egress".into()),
                ..dscp
            }
            .body()
            .is_err(),
            "no direction for DSCP"
        );
        let pps = QosRuleSpec {
            kind: "minimum_packet_rate".into(),
            direction: Some("any".into()),
            min_kpps: Some(1000),
            ..Default::default()
        };
        assert_eq!(
            pps.body().unwrap(),
            json!({"minimum_packet_rate_rule": {"direction": "any", "min_kpps": 1000}})
        );
        assert!(
            QosRuleSpec {
                kind: "minimum_bandwidth".into(),
                direction: Some("any".into()),
                min_kbps: Some(1),
                ..Default::default()
            }
            .body()
            .is_err()
        );
        assert!(
            QosRuleSpec {
                kind: "bandwidth_limit".into(),
                ..Default::default()
            }
            .body()
            .is_err(),
            "max_kbps required"
        );
        assert!(
            QosRuleSpec {
                kind: "packet_loss".into(),
                ..Default::default()
            }
            .body()
            .is_err()
        );
    }
}
