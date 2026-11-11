//! Form choices loaded for dialogs, and the launch rules that check them.

use super::*;

// ---------------------------------------------------------------------------
// Form choices
// ---------------------------------------------------------------------------

/// Option lists a dialog can load (`get_choices`); each is one or a few GETs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChoiceSet {
    Flavors,
    Images,
    /// Networks a port/instance can use (project + shared)
    Networks,
    ExternalNetworks,
    Subnets,
    KeyPairs,
    /// By name (Nova launch takes names)
    SecurityGroups,
    /// By ID (rules reference groups by ID)
    SecurityGroupIds,
    ServerGroups,
    ComputeZones,
    VolumeZones,
    /// Neutron's network availability zones (`resource=network`), by name
    NetworkZones,
    /// Neutron's router availability zones (`resource=router`), by name
    RouterZones,
    /// Enabled, up `nova-compute` hosts (an admin's migration targets)
    ComputeHosts,
    /// Every `nova-compute` host, whatever its state (aggregate members)
    AllComputeHosts,
    /// *Create RBAC Policy*'s *Target Project*: `*` (every project), then the projects
    RbacTargets,
    /// Every project of every domain, by name, for an admin (the Admin
    /// dashboard's *Project* fields, as Horizon lists them)
    AllProjects,
    /// Every project's networks, for an admin (*Create RBAC Policy*)
    AllNetworks,
    /// Every project's QoS policies, for an admin (*Create RBAC Policy*)
    AllQosPolicies,
    /// Cinder's back-end pools (`host@backend#pool`), an admin's volume
    /// migration targets (`scheduler-stats/get_pools`)
    CinderPools,
    /// Cinder's QoS specs (value ID, label name), for a volume type's association
    QosSpecs,
    /// Glance's metadata definition resource types (`OS::Nova::Flavor`…), by name
    MetadefResourceTypes,
    /// Every project's instance ports without a floating IP, for an admin's
    /// *Associate*. The first cell is [`associable_port_key`]: a port reached
    /// from several pools comes once per pool, and the dialog keeps the
    /// floating IP owner's ports its pool reaches with `Source::Of`
    AllAssociablePorts,
    VolumeTypes,
    AvailableVolumes,
    VolumeSnapshots,
    Servers,
    /// Instance ports without a floating IP, for association
    AssociablePorts,
    /// Fixed IPv4 addresses of instance ports, for port forwarding: value `port_id/ip`
    ForwardablePorts,
    /// QoS policies the project can use (its own and shared ones)
    QosPolicies,
    /// Load balancer members: each fixed IP of an instance port, value `ip@subnet_id`;
    /// cells `[instance, ip, subnet name]`
    LbMemberTargets,
    /// The scoped token's roles, for an application credential's roles (roadmap 57)
    TokenRoles,
    /// The domain new projects, users and groups go to (one choice: value ID,
    /// label name), shown read-only in their create dialogs like Horizon
    OperationDomain,
    /// Identity admin pickers (roadmap 57): every role (value ID, label name)
    IdentityRoles,
    /// Users visible to an admin (value ID, label name)
    IdentityUsers,
    /// Groups visible to an admin
    IdentityGroups,
    /// Projects visible to an admin (a user's primary project)
    IdentityProjects,
    /// Subnets an external member can be on, by name: those of the project's own,
    /// shared and external networks, as Horizon's members step lists them
    LbMemberSubnets,
    /// Octavia flavors that are enabled
    LbFlavors,
    /// Subnets a load balancer's VIP can be on: the project's own and shared
    /// networks' (roadmap 54, like Horizon)
    LbVipSubnets,
    /// Octavia availability zones that are enabled
    LbAvailabilityZones,
    /// Certificates a `TERMINATED_HTTPS` listener can use (Barbican URLs, roadmap 54);
    /// first cell = expiry, `""` when it never expires
    LbCertificates,
    /// Every instance port; first cell = instance ID, second = its security group IDs
    InstancePorts,
    /// Floating IPs not associated with a port
    FreeFloatingIps,
    /// Launch wizard: bootable images (not instance snapshots), like Horizon's *Image*
    LaunchImages,
    /// Launch wizard: images with `image_type=snapshot`
    InstanceSnapshots,
    /// Launch wizard: project ports not attached to anything
    FreePorts,
    /// Barbican secrets for a container (roadmap 55): value `secret_ref`, label
    /// the name; cells `[type]`
    Secrets,
    /// Cinder group types (value ID, label name), for *Create Group*
    GroupTypes,
    /// Volume types by ID (`VolumeTypes` gives names), for *Create Group*
    VolumeTypeIds,
    /// *Manage Volumes*: for each volume group, its volumes plus those it can
    /// take (no group, one of its volume types, `available` or `in-use`); a
    /// volume appears once per group, first cell = the group's ID (`Source::Of`)
    GroupVolumes,
    /// Firewall rules the project can use: its own and shared ones (roadmap 93);
    /// cells `[action, protocol]`
    FirewallRules,
    /// Firewall policies the project can use: its own and shared ones
    FirewallPolicies,
    /// Ports a firewall group can take: the project's router interfaces and
    /// instance ports no group has (Neutron refuses those); cells
    /// `[device, kind, IPs, port]`
    FirewallPorts,
}

/// One option of a select or pick table. `cells` feed the wizard tables (one
/// per column of `ChoiceSet`'s table, raw: booleans as `true`/`false`, the UI
/// localises them); `facts` feed Horizon's sizing rules.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub cells: Vec<String>,
    #[serde(default)]
    pub facts: Facts,
}

impl Choice {
    /// A choice with only a value and a label (no table cells).
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Choice {
            value: value.into(),
            label: label.into(),
            ..Default::default()
        }
    }
}

/// The `Source::Of` key of a [`ChoiceSet::AllAssociablePorts`] choice: the
/// port's project and a floating IP pool (external network ID) that reaches it.
/// Neutron associates only within one project and through a router with its
/// gateway on the pool (roadmap 89).
pub fn associable_port_key(project_id: &str, pool_id: &str) -> String {
    format!("{project_id} {pool_id}")
}

/// Numbers the launch wizard needs about images and flavors.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Facts {
    pub size_bytes: Option<u64>,
    pub min_disk_gb: Option<i64>,
    pub min_ram_mb: Option<i64>,
    pub vcpus: Option<i64>,
    pub ram_mb: Option<i64>,
    pub disk_gb: Option<i64>,
}

/// Horizon's minimum boot-volume size for an image: the larger of its size in
/// (decimal) GB and its `min_disk`, rounded up, at least 1.
pub fn min_volume_gb(image: &Facts) -> u32 {
    let size_gb = image.size_bytes.unwrap_or(0) as f64 / 1e9;
    let min_disk = image.min_disk_gb.unwrap_or(0).max(0) as f64;
    (size_gb.max(min_disk).ceil() as u32).max(1)
}

/// Why a flavor cannot boot the selected source (Horizon disables it).
#[derive(Clone, Debug, PartialEq)]
pub enum FlavorIssue {
    /// The image needs at least this much RAM (MB).
    Ram(i64),
    /// Booting from the image without a volume needs this much root disk (GB).
    Disk(i64),
}

/// `boot_from_volume`: the root disk is a volume, so the flavor's disk does not
/// matter (Nova only checks it for local disks). Flavor disk 0 = image size.
pub fn flavor_issue(flavor: &Facts, image: Option<&Facts>, boot_from_volume: bool) -> Option<FlavorIssue> {
    let image = image?;
    let ram = flavor.ram_mb.unwrap_or(0);
    if let Some(need) = image.min_ram_mb.filter(|m| *m > ram) {
        return Some(FlavorIssue::Ram(need));
    }
    let disk = flavor.disk_gb.unwrap_or(0);
    if !boot_from_volume && disk > 0 {
        let need = min_volume_gb(image) as i64;
        if need > disk {
            return Some(FlavorIssue::Disk(need));
        }
    }
    None
}

#[cfg(test)]
mod launch_rules_tests {
    use super::*;

    fn image(gb_bytes: u64, min_disk: i64, min_ram: Option<i64>) -> Facts {
        Facts {
            size_bytes: Some(gb_bytes),
            min_disk_gb: Some(min_disk),
            min_ram_mb: min_ram,
            ..Default::default()
        }
    }
    fn flavor(ram: i64, disk: i64) -> Facts {
        Facts {
            ram_mb: Some(ram),
            disk_gb: Some(disk),
            vcpus: Some(1),
            ..Default::default()
        }
    }

    #[test]
    fn min_volume_matches_horizon() {
        // ADM-RHEL9.7-ES: 7.80 GB, min_disk 20 → 20 (seen in Horizon).
        assert_eq!(min_volume_gb(&image(7_800_000_000, 20, None)), 20);
        // Debian 12: 428.69 MB, no min_disk → 1 ("must be at least 1 GB").
        assert_eq!(min_volume_gb(&image(428_690_000, 0, None)), 1);
        assert_eq!(min_volume_gb(&image(2_100_000_000, 0, None)), 3);
        assert_eq!(min_volume_gb(&Facts::default()), 1);
    }

    #[test]
    fn flavor_issues_follow_ram_and_local_disk() {
        let img = image(7_800_000_000, 20, Some(4096));
        assert_eq!(
            flavor_issue(&flavor(2048, 50), Some(&img), true),
            Some(FlavorIssue::Ram(4096))
        );
        assert_eq!(
            flavor_issue(&flavor(8192, 10), Some(&img), false),
            Some(FlavorIssue::Disk(20))
        );
        assert_eq!(
            flavor_issue(&flavor(8192, 10), Some(&img), true),
            None,
            "volume boot ignores flavor disk"
        );
        assert_eq!(
            flavor_issue(&flavor(8192, 0), Some(&img), false),
            None,
            "disk 0 = sized from the image"
        );
        assert_eq!(flavor_issue(&flavor(512, 1), None, false), None);
    }

    #[cfg(test)]
    mod capability_tests {
        use super::*;

        fn caps() -> Capabilities {
            Capabilities {
                known: true,
                services: vec!["compute".into(), "network".into(), "volumev3".into(), "image".into()],
                network_extensions: vec!["router".into(), "security-group".into()],
                lb_mv: Some((2, 27)),
                compute_mv: Some((2, 79)),
                volume_mv: Some((3, 70)),
                plugins: vec![],
                roles: vec!["member".into()],
                multidomain: false,
                image_import: None,
            }
        }

        #[test]
        fn domains_need_the_setting_even_when_detection_failed() {
            let admin = Capabilities {
                roles: vec!["admin".into()],
                ..caps()
            };
            assert!(!needs::DOMAINS.met(&admin), "[keystone] multidomain off");
            assert!(needs::DOMAINS.met(&Capabilities {
                multidomain: true,
                ..admin.clone()
            }));
            assert!(
                !needs::DOMAINS.met(&Capabilities {
                    multidomain: true,
                    ..caps()
                }),
                "members don't see it"
            );
            assert!(
                !needs::DOMAINS.met(&Capabilities::default()),
                "configuration doesn't fail open"
            );
        }

        #[test]
        fn requirements_check_services_extensions_and_versions() {
            let c = caps();
            assert!(
                Requires::Service("block-storage").met(&c),
                "volumev3 counts as block-storage"
            );
            assert!(!Requires::Service("orchestration").met(&c));
            assert!(Requires::NetExt("router").met(&c));
            assert!(!Requires::NetExt("extraroute-atomic").met(&c));
            assert!(needs::UNSHELVE_TO_ZONE.met(&c));
            assert!(!needs::REBUILD.met(&c), "2.79 < 2.94");
            assert!(needs::VOLUME_MESSAGES.met(&c));
            assert!(!Requires::All(&[Requires::Service("network"), Requires::NetExt("trunk")]).met(&c));
        }

        #[test]
        fn snat_needs_the_extension_and_an_admin_role() {
            let ext = |roles: &[&str]| Capabilities {
                network_extensions: vec!["ext-gw-mode".into()],
                roles: roles.iter().map(|r| r.to_string()).collect(),
                ..caps()
            };
            assert!(!needs::SNAT.met(&caps()), "member, no extension");
            assert!(!needs::SNAT.met(&ext(&["member", "reader"])));
            assert!(needs::SNAT.met(&ext(&["member", "admin"])));
            assert!(!needs::SNAT.met(&Capabilities {
                network_extensions: vec![],
                ..ext(&["admin"])
            }));
            assert!(needs::SNAT.met(&Capabilities::default()), "unknown: the server decides");
        }

        #[test]
        fn unknown_capabilities_or_versions_fail_open() {
            assert!(Requires::Service("orchestration").met(&Capabilities::default()));
            let c = Capabilities {
                compute_mv: None,
                ..caps()
            };
            assert!(needs::REBUILD.met(&c), "no version range: don't hide");
        }

        #[test]
        fn reserved_image_keys_are_glances_and_the_operators() {
            let reserved = vec!["owner_specified.openstack.md5".to_string()];
            assert!(is_reserved_image_key("os_glance_failed_import", &[]));
            assert!(is_reserved_image_key("owner_specified.openstack.md5", &reserved));
            assert!(!is_reserved_image_key("owner_specified.openstack.md5", &[]));
            assert!(!is_reserved_image_key("hw_disk_bus", &reserved));
            let set = OsAction::ImageSetProperties {
                id: "i".into(),
                set: vec![("a".into(), "1".into())],
                remove: vec!["b".into()],
            };
            assert_eq!(set.image_property_keys(), ["a", "b"]);
            let batch = OsAction::Batch(vec![("x".into(), set)]);
            assert_eq!(batch.image_property_keys(), ["a", "b"], "inside a batch too");
        }

        #[test]
        fn plugins_are_opt_in_even_when_detection_failed() {
            let lb = Requires::All(&[
                Requires::Plugin(Plugin::LoadBalancer),
                Requires::Service("load-balancer"),
            ]);
            assert!(
                !lb.met(&Capabilities::default()),
                "not enabled: hidden although nothing is known"
            );
            let enabled = Capabilities {
                plugins: vec![Plugin::LoadBalancer],
                ..Default::default()
            };
            assert!(lb.met(&enabled), "enabled, detection failed: shown");
            assert!(
                !lb.met(&Capabilities {
                    plugins: vec![Plugin::LoadBalancer],
                    ..caps()
                }),
                "enabled but not in the catalog"
            );
            assert_eq!(Plugin::parse(" orchestration "), Some(Plugin::Orchestration));
            assert_eq!(Plugin::parse("octavia"), None);
            let batch = OsAction::Batch(vec![(
                "a".into(),
                OsAction::LoadBalancerDelete {
                    id: "a".into(),
                    cascade: true,
                },
            )]);
            assert_eq!(batch.plugins(), [Plugin::LoadBalancer]);
            assert!(OsAction::RouterDelete { id: "r".into() }.plugins().is_empty());
        }

        #[test]
        fn parses_microversions() {
            assert_eq!(parse_microversion("2.95"), Some((2, 95)));
            assert_eq!(parse_microversion(" 3.70 "), Some((3, 70)));
            assert_eq!(parse_microversion("v2"), None);
            assert!((2, 100) > (2, 95), "compared numerically, not as text");
        }
    }
}
