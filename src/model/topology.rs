//! The Network Topology view.

use super::*;

// ---------------------------------------------------------------------------
// Network topology
// ---------------------------------------------------------------------------

/// What a topology node other than a network is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TopoKind {
    Router,
    Server,
}

/// A network in the Network Topology, with its subnets' CIDRs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TopoNetwork {
    pub id: String,
    pub name: String,
    pub external: bool,
    pub shared: bool,
    /// The session's project owns it (a project's own network may be `shared` too);
    /// the topology marks the others.
    #[serde(default)]
    pub owned: bool,
    pub status: String,
    pub cidrs: Vec<String>,
}

/// A router or an instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TopoDevice {
    pub id: String,
    pub name: String,
    pub kind: TopoKind,
    pub status: String,
}

/// A port joining a device to a network.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TopoLink {
    pub device_id: String,
    pub network_id: String,
    pub port_id: Option<String>,
    pub ips: Vec<String>,
    /// Floating IPs associated with the port.
    pub floating: Vec<String>,
    /// Router gateway on an external network.
    pub gateway: bool,
}

/// Networks ordered external first, then by name; devices routers first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Topology {
    pub networks: Vec<TopoNetwork>,
    pub devices: Vec<TopoDevice>,
    pub links: Vec<TopoLink>,
    /// `[network] topology_max_networks`, sent with the data so
    /// the server render and the hydrated page agree; `0`: no limit.
    #[serde(default)]
    pub max_networks: usize,
}

impl Topology {
    /// The network count when it is over [`Topology::max_networks`]: the page
    /// then shows a note instead of the drawings. At the limit it still draws.
    pub fn too_many(&self) -> Option<usize> {
        let n = self.networks.len();
        (self.max_networks > 0 && n > self.max_networks).then_some(n)
    }

    /// The links that touch `id`, a device or a network.
    pub fn links_of<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a TopoLink> + 'a {
        self.links
            .iter()
            .filter(move |l| l.device_id == id || l.network_id == id)
    }
}

#[cfg(test)]
mod topology_tests {
    use super::*;

    fn with(n: usize, max: usize) -> Topology {
        let net = |i: usize| TopoNetwork {
            id: i.to_string(),
            name: String::new(),
            external: false,
            shared: false,
            owned: true,
            status: "ACTIVE".into(),
            cidrs: vec![],
        };
        Topology {
            networks: (0..n).map(net).collect(),
            max_networks: max,
            ..Default::default()
        }
    }

    #[test]
    fn too_many_only_above_the_limit() {
        assert_eq!(with(50, 50).too_many(), None, "at the limit it still draws");
        assert_eq!(with(51, 50).too_many(), Some(51));
        assert_eq!(with(500, 0).too_many(), None, "0 = no limit");
        assert_eq!(with(0, 1).too_many(), None);
    }
}
