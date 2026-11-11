//! Pure model of the stack *Topology* graph: which draw.io icon each Heat
//! resource type gets, the edges between resources, and where an edge leaves
//! and reaches the round nodes. No DOM, so it is unit-tested; the force layout
//! is [`topo_layout::force_edges`](super::topo_layout::force_edges).

use super::heat_icons::Icon;
use crate::model::StackResource;

/// The draw.io stencil for a Heat resource type; `None` for the types the
/// library has no icon for (Octavia, Barbican, software configs…), drawn as
/// [`initials`] in a ring instead. The `AWS::*` types are Heat's CFN-compatible aliases.
pub fn icon(resource_type: &str) -> Option<Icon> {
    Some(match resource_type {
        "OS::Nova::Server" | "AWS::EC2::Instance" => Icon::NovaServer,
        "OS::Nova::KeyPair" => Icon::NovaKeypair,
        "OS::Cinder::Volume" | "AWS::EC2::Volume" => Icon::CinderVolume,
        "OS::Cinder::VolumeAttachment" | "AWS::EC2::VolumeAttachment" => Icon::CinderVolumeattachment,
        "OS::Neutron::Net" | "OS::Neutron::ProviderNet" => Icon::NeutronNet,
        "OS::Neutron::Subnet" => Icon::NeutronSubnet,
        "OS::Neutron::Port" => Icon::NeutronPort,
        "OS::Neutron::Router" => Icon::NeutronRouter,
        "OS::Neutron::RouterInterface" => Icon::NeutronRouterinterface,
        "OS::Neutron::FloatingIP" | "AWS::EC2::EIP" => Icon::NeutronFloatingip,
        "OS::Neutron::FloatingIPAssociation" | "AWS::EC2::EIPAssociation" => Icon::NeutronFloatingipassociation,
        "OS::Neutron::SecurityGroup" | "OS::Neutron::SecurityGroupRule" | "AWS::EC2::SecurityGroup" => {
            Icon::NeutronSecuritygroup
        }
        "OS::Designate::Zone" => Icon::DesignateZone,
        "OS::Designate::RecordSet" => Icon::DesignateRecordset,
        "OS::Heat::AutoScalingGroup" | "AWS::AutoScaling::AutoScalingGroup" => Icon::HeatAutoscalinggroup,
        "OS::Heat::ResourceGroup" => Icon::HeatResourcegroup,
        "OS::Heat::ScalingPolicy" | "AWS::AutoScaling::ScalingPolicy" => Icon::HeatScalingpolicy,
        "OS::Swift::Container" => Icon::SwiftContainer,
        _ => return None,
    })
}

/// Up to three capitals of the type's last segment, for types without an icon:
/// `OS::Octavia::LoadBalancer` → `LB`, `OS::Heat::RandomString` → `RS`. A provider
/// template (`web.yaml`) or a nested stack shows `STK`.
pub fn initials(resource_type: &str) -> String {
    if resource_type == "OS::Heat::Stack" || !resource_type.contains("::") {
        return "STK".into();
    }
    let last = resource_type.rsplit("::").next().unwrap_or_default();
    let caps: String = last.chars().filter(char::is_ascii_uppercase).take(3).collect();
    if caps.is_empty() {
        last.chars().take(2).collect::<String>().to_uppercase()
    } else {
        caps
    }
}

/// A line of the graph, between node IDs (resource names, or the stack's ID).
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    /// `from` depends on `to` (drawn with an arrow to `to`); `false` for the
    /// links from the stack node.
    pub depends: bool,
}

/// The stack's edges, as Horizon draws them: each resource to the resources it
/// depends on, and the stack (node `stack_id`) to the resources nothing requires,
/// so every resource hangs from the stack. Dependencies on names the list lacks
/// are dropped.
pub fn edges(stack_id: &str, resources: &[StackResource]) -> Vec<Edge> {
    let known = |n: &str| resources.iter().any(|r| r.name == n);
    let deps = resources.iter().flat_map(|r| {
        r.depends_on.iter().filter(|d| known(d)).map(|d| Edge {
            from: r.name.clone(),
            to: d.clone(),
            depends: true,
        })
    });
    let roots = resources
        .iter()
        .filter(|r| !r.required_by.iter().any(|n| known(n)))
        .map(|r| Edge {
            from: stack_id.to_string(),
            to: r.name.clone(),
            depends: false,
        });
    deps.chain(roots).collect()
}

/// The part of the segment `a`–`b` outside two circles of radius `ra` around
/// `a` and `rb` around `b`, so an arrow ends at the node's rim; `None` when they
/// overlap.
pub fn trim(a: (f64, f64), b: (f64, f64), ra: f64, rb: f64) -> Option<((f64, f64), (f64, f64))> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let d = (dx * dx + dy * dy).sqrt();
    if d <= ra + rb {
        return None;
    }
    let (ux, uy) = (dx / d, dy / d);
    Some(((a.0 + ux * ra, a.1 + uy * ra), (b.0 - ux * rb, b.1 - uy * rb)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn res(name: &str, depends_on: &[&str], required_by: &[&str]) -> StackResource {
        StackResource {
            name: name.into(),
            physical_id: None,
            resource_type: "OS::Nova::Server".into(),
            status: "CREATE_COMPLETE".into(),
            status_reason: None,
            updated: String::new(),
            required_by: required_by.iter().map(|s| s.to_string()).collect(),
            depends_on: depends_on.iter().map(|s| s.to_string()).collect(),
            href: None,
        }
    }

    #[test]
    fn heat_types_get_their_draw_io_icon_and_the_rest_initials() {
        assert_eq!(icon("OS::Nova::Server"), Some(Icon::NovaServer));
        assert_eq!(icon("AWS::EC2::Instance"), Some(Icon::NovaServer));
        assert_eq!(icon("OS::Neutron::SecurityGroupRule"), Some(Icon::NeutronSecuritygroup));
        assert_eq!(icon("OS::Octavia::LoadBalancer"), None);
        assert_eq!(initials("OS::Octavia::LoadBalancer"), "LB");
        assert_eq!(initials("OS::Heat::SoftwareDeploymentGroup"), "SDG");
        assert_eq!(initials("OS::Zaqar::queue"), "QU", "no capitals: first two letters");
        assert_eq!(initials("web.yaml"), "STK", "provider template");
        assert_eq!(initials("OS::Heat::Stack"), "STK");
    }

    #[test]
    fn edges_follow_dependencies_and_hang_the_roots_from_the_stack() {
        let rs = [
            res("server", &["port"], &[]),
            res("port", &["net"], &["server"]),
            res("net", &[], &["port"]),
        ];
        let e = edges("s1", &rs);
        let pairs: Vec<(&str, &str, bool)> = e.iter().map(|e| (e.from.as_str(), e.to.as_str(), e.depends)).collect();
        assert_eq!(
            pairs,
            [("server", "port", true), ("port", "net", true), ("s1", "server", false)]
        );
    }

    #[test]
    fn edges_ignore_names_missing_from_the_list() {
        // Heat lists `required_by` names of resources that are not shown (e.g. deleted).
        let rs = [res("a", &["gone"], &["gone"])];
        let e = edges("s1", &rs);
        assert_eq!(
            e,
            [Edge {
                from: "s1".into(),
                to: "a".into(),
                depends: false
            }]
        );
    }

    #[test]
    fn trim_stops_at_both_rims_and_drops_overlapping_nodes() {
        assert_eq!(trim((0.0, 0.0), (10.0, 0.0), 2.0, 3.0), Some(((2.0, 0.0), (7.0, 0.0))));
        assert_eq!(trim((0.0, 0.0), (4.0, 0.0), 2.0, 3.0), None);
    }
}
