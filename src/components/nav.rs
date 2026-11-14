//! The dashboards' menus (Project, Admin, Identity), mirroring Horizon's panel groups.

use crate::i18n::K;
use crate::model::{Capabilities, ErrorKind, Plugin, Requires};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    pub label: K,
    pub href: &'static str,
    /// Shown only when the cloud offers this (roadmap 19).
    pub requires: Requires,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Group {
    pub label: K,
    pub panels: &'static [Panel],
    pub requires: Requires,
}

const fn p(label: K, href: &'static str) -> Panel {
    Panel {
        label,
        href,
        requires: Requires::Always,
    }
}

const fn needs(label: K, href: &'static str, requires: Requires) -> Panel {
    Panel { label, href, requires }
}

impl Group {
    /// The panels to show; a group with none left is hidden.
    pub fn visible(&self, c: &Capabilities) -> Option<Vec<&'static Panel>> {
        if !self.requires.met(c) {
            return None;
        }
        let panels: Vec<_> = self.panels.iter().filter(|p| p.requires.met(c)).collect();
        (!panels.is_empty()).then_some(panels)
    }
}

/// Whether the page at `path` is available; unknown paths are.
pub fn available(path: &str, c: &Capabilities) -> bool {
    active(path).is_none_or(|(g, p)| g.requires.met(c) && p.requires.met(c))
}

/// Why the page at `path` is not available: `Forbidden` when the user lacks a
/// role it needs (the Admin dashboard, Identity › Groups), else
/// `ServiceUnavailable` (not in this region's catalog, an extension, a plugin).
/// `None` when it is available.
pub fn unavailable(path: &str, c: &Capabilities) -> Option<ErrorKind> {
    if available(path, c) {
        return None;
    }
    let lacks_role = active(path).is_some_and(|(g, p)| g.requires.lacks_role(c) || p.requires.lacks_role(c));
    Some(if lacks_role {
        ErrorKind::Forbidden
    } else {
        ErrorKind::ServiceUnavailable
    })
}

pub const PROJECT_NAV: &[Group] = &[
    Group {
        label: K::Project,
        panels: &[
            p(K::Overview, "/project"),
            p(K::Quotas, "/project/quotas"),
            p(K::ApiAccess, "/project/api_access"),
        ],
        requires: Requires::Always,
    },
    Group {
        label: K::Compute,
        panels: &[
            needs(K::Instances, "/project/instances", Requires::Service("compute")),
            needs(K::Images, "/project/images", Requires::Service("image")),
            needs(K::KeyPairs, "/project/key_pairs", Requires::Service("compute")),
            needs(K::ServerGroups, "/project/server_groups", Requires::Service("compute")),
        ],
        requires: Requires::Always,
    },
    Group {
        label: K::VolumesGroup,
        // Horizon's order and URLs (`volume_groups`, `vg_snapshots`); groups need volume 3.27 (roadmap 5).
        panels: &[
            p(K::Volumes, "/project/volumes"),
            p(K::Snapshots, "/project/snapshots"),
            needs(K::Backups, "/project/backups", crate::model::needs::BACKUP),
            needs(K::Groups, "/project/volume_groups", crate::model::needs::VOLUME_GROUPS),
            needs(
                K::GroupSnapshots,
                "/project/vg_snapshots",
                crate::model::needs::VOLUME_GROUPS,
            ),
        ],
        requires: Requires::Service("block-storage"),
    },
    Group {
        label: K::NetworkGroup,
        panels: &[
            p(K::NetworkTopology, "/project/network_topology"),
            p(K::Networks, "/project/networks"),
            needs(K::Routers, "/project/routers", Requires::NetExt("router")),
            needs(
                K::SecurityGroups,
                "/project/security_groups",
                Requires::NetExt("security-group"),
            ),
            needs(K::FloatingIps, "/project/floating_ips", Requires::NetExt("router")),
            needs(K::Trunks, "/project/trunks", crate::model::needs::TRUNK),
            needs(K::Firewall, "/project/firewall", crate::model::needs::FIREWALL),
            needs(K::NetworkQos, "/project/qos", crate::model::needs::QOS),
            needs(
                K::LoadBalancers,
                "/project/load_balancers",
                Requires::All(&[
                    Requires::Plugin(Plugin::LoadBalancer),
                    Requires::Service("load-balancer"),
                ]),
            ),
        ],
        requires: Requires::Service("network"),
    },
    // Barbican (roadmap 55); barbican-ui's own panel group, Horizon's name for the service.
    Group {
        label: K::KeyManager,
        panels: &[
            p(K::Secrets, "/project/key_manager/secrets"),
            p(K::Containers, "/project/key_manager/containers"),
            p(K::Orders, "/project/key_manager/orders"),
        ],
        requires: crate::model::needs::KEY_MANAGER,
    },
    // Heat (roadmap 62). Resource Types and Template Versions come next.
    Group {
        label: K::Orchestration,
        panels: &[
            p(K::Stacks, "/project/stacks"),
            p(K::ResourceTypes, "/project/resource_types"),
            p(K::TemplateVersions, "/project/template_versions"),
            p(K::TemplateGenerator, "/project/template_generator"),
        ],
        requires: crate::model::needs::ORCHESTRATION,
    },
    // Aodh (roadmap 79). Horizon has no maintained Alarming panel; the group
    // leaves room for other telemetry panels.
    Group {
        label: K::Monitoring,
        panels: &[p(K::Alarms, "/project/alarms")],
        requires: crate::model::needs::ALARMING,
    },
];

/// Horizon's Identity dashboard (roadmap 57), in its panel order. Like Horizon's
/// panel policies with a project-scoped token: Projects, Users and Application
/// Credentials for everyone (their content depends on the role), Groups and
/// Roles for admins only. Domains (domain-scoped tokens) and federation are not
/// offered.
pub const IDENTITY_NAV: &[Group] = &[Group {
    label: K::Identity,
    panels: &[
        needs(K::Domains, "/identity/domains", crate::model::needs::DOMAINS),
        p(K::Projects, "/identity/projects"),
        p(K::Users, "/identity/users"),
        needs(K::Groups, "/identity/groups", crate::model::needs::IDENTITY_ADMIN),
        needs(K::Roles, "/identity/roles", crate::model::needs::IDENTITY_ADMIN),
        p(K::ApplicationCredentials, "/identity/application_credentials"),
    ],
    requires: Requires::Always,
}];

/// Horizon's Admin dashboard (roadmap 85), for the `admin` role, in Horizon's
/// order and URLs.
pub const ADMIN_NAV: &[Group] = &[
    // Overview is Nova's usage of every project.
    Group {
        label: K::Admin,
        panels: &[p(K::Overview, "/admin")],
        requires: Requires::All(&[crate::model::needs::ADMIN, Requires::Service("compute")]),
    },
    Group {
        label: K::Compute,
        panels: &[
            p(K::Hypervisors, "/admin/hypervisors"),
            p(K::HostAggregates, "/admin/aggregates"),
            p(K::Instances, "/admin/instances"),
            p(K::Flavors, "/admin/flavors"),
            needs(K::Images, "/admin/images", Requires::Service("image")),
        ],
        requires: Requires::All(&[crate::model::needs::ADMIN, Requires::Service("compute")]),
    },
    Group {
        label: K::VolumeAdminGroup,
        panels: &[
            p(K::Volumes, "/admin/volumes"),
            p(K::Snapshots, "/admin/snapshots"),
            p(K::VolumeTypes, "/admin/volume_types"),
            needs(K::Groups, "/admin/volume_groups", crate::model::needs::VOLUME_GROUPS),
            needs(
                K::GroupSnapshots,
                "/admin/vg_snapshots",
                crate::model::needs::VOLUME_GROUPS,
            ),
            needs(K::GroupTypes, "/admin/group_types", crate::model::needs::VOLUME_GROUPS),
        ],
        requires: Requires::All(&[crate::model::needs::ADMIN, Requires::Service("block-storage")]),
    },
    Group {
        label: K::NetworkGroup,
        panels: &[
            p(K::Networks, "/admin/networks"),
            needs(K::Routers, "/admin/routers", Requires::NetExt("router")),
            needs(K::FloatingIps, "/admin/floating_ips", Requires::NetExt("router")),
            needs(K::Trunks, "/admin/trunks", crate::model::needs::TRUNK),
            needs(
                K::RbacPolicies,
                "/admin/rbac_policies",
                Requires::NetExt("rbac-policies"),
            ),
        ],
        requires: Requires::All(&[crate::model::needs::ADMIN, Requires::Service("network")]),
    },
    Group {
        label: K::System,
        // Horizon's order.
        panels: &[
            p(K::QuotaDefaults, "/admin/defaults"),
            needs(
                K::MetadataDefinitions,
                "/admin/metadata_defs",
                Requires::Service("image"),
            ),
            p(K::SystemInformation, "/admin/info"),
        ],
        requires: crate::model::needs::ADMIN,
    },
];

/// The dashboard a path belongs to: its name and menu.
pub fn dashboard(path: &str) -> (K, &'static [Group]) {
    if path == "/identity" || path.starts_with("/identity/") {
        (K::Identity, IDENTITY_NAV)
    } else if path == "/admin" || path.starts_with("/admin/") {
        (K::Admin, ADMIN_NAV)
    } else {
        (K::Project, PROJECT_NAV)
    }
}

fn matches(panel: &Panel, path: &str) -> bool {
    let path = path.trim_end_matches('/');
    let path = if path.is_empty() { "/" } else { path };
    // A dashboard's own page (Project and Admin › Overview) owns only its exact
    // path, not every page of the dashboard.
    if panel.href == "/project" || panel.href == "/admin" {
        path == panel.href
    } else {
        path == panel.href || path.starts_with(&format!("{}/", panel.href))
    }
}

/// (group, panel) active for a path, e.g. `/project/instances/<id>` → Compute › Instances.
pub fn active(path: &str) -> Option<(&'static Group, &'static Panel)> {
    dashboard(path)
        .1
        .iter()
        .find_map(|g| g.panels.iter().find(|p| matches(p, path)).map(|p| (g, p)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hrefs_are_unique() {
        let mut all: Vec<_> = PROJECT_NAV
            .iter()
            .chain(IDENTITY_NAV)
            .chain(ADMIN_NAV)
            .flat_map(|g| g.panels)
            .map(|p| p.href)
            .collect();
        let n = all.len();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), n);
    }

    fn caps(services: &[&str], exts: &[&str]) -> Capabilities {
        Capabilities {
            known: true,
            services: services.iter().map(|s| s.to_string()).collect(),
            network_extensions: exts.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn identity_admin_panels_need_the_admin_role() {
        let with = |roles: &[&str]| Capabilities {
            known: true,
            roles: roles.iter().map(|r| r.to_string()).collect(),
            ..Default::default()
        };
        let labels = |c: &Capabilities| {
            IDENTITY_NAV[0]
                .visible(c)
                .unwrap()
                .iter()
                .map(|p| p.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(&with(&["admin", "member"])),
            [K::Projects, K::Users, K::Groups, K::Roles, K::ApplicationCredentials]
        );
        assert_eq!(
            labels(&with(&["member", "power_user"])),
            [K::Projects, K::Users, K::ApplicationCredentials],
            "like Horizon in a project without admin"
        );
        assert!(
            !available("/identity/groups", &with(&["member"])),
            "the page itself is gated too"
        );
        assert_eq!(active("/identity/users").map(|(_, p)| p.label), Some(K::Users));
        assert_eq!(dashboard("/identity/roles").0, K::Identity);
        assert_eq!(dashboard("/project/instances").0, K::Project);
    }

    #[test]
    fn panels_follow_services_and_extensions() {
        let shown = |c: &Capabilities| -> Vec<K> {
            PROJECT_NAV
                .iter()
                .filter_map(|g| g.visible(c).map(|_| g.label))
                .collect()
        };
        let full = caps(
            &["compute", "image", "volumev3", "network", "orchestration"],
            &["router", "security-group"],
        );
        assert_eq!(shown(&full).len(), 4, "Heat in the catalog, plugin not enabled");
        assert_eq!(
            shown(&Capabilities {
                plugins: vec![Plugin::Orchestration],
                ..full
            })
            .len(),
            5
        );
        let no_heat_no_cinder = caps(&["compute", "image", "network"], &["security-group"]);
        assert_eq!(shown(&no_heat_no_cinder), [K::Project, K::Compute, K::NetworkGroup]);
        let net = PROJECT_NAV
            .iter()
            .find(|g| g.label == K::NetworkGroup)
            .unwrap()
            .visible(&no_heat_no_cinder)
            .unwrap();
        assert!(
            !net.iter().any(|p| p.label == K::Routers || p.label == K::FloatingIps),
            "no router extension"
        );
        assert!(!available("/project/routers/r1", &no_heat_no_cinder));
        assert!(!available("/project/volumes", &no_heat_no_cinder));
        assert!(available("/project/instances", &no_heat_no_cinder));
        assert!(
            available("/project/volumes", &Capabilities::default()),
            "unknown: show everything"
        );
        let heat_on = |c: Capabilities| Capabilities {
            plugins: vec![Plugin::Orchestration],
            ..c
        };
        assert_eq!(
            shown(&Capabilities::default()).len(),
            4,
            "except plugins, which are opt-in"
        );
        assert_eq!(shown(&heat_on(Capabilities::default())).len(), 5);
    }

    #[test]
    fn plugin_panels_need_the_plugin_and_the_service() {
        let lb = |c: &Capabilities| available("/project/load_balancers/lb1", c);
        let with_octavia = caps(&["network", "load-balancer"], &[]);
        assert!(!lb(&with_octavia), "in the catalog but not enabled");
        assert!(lb(&Capabilities {
            plugins: vec![Plugin::LoadBalancer],
            ..with_octavia
        }));
        assert!(
            !lb(&Capabilities {
                plugins: vec![Plugin::LoadBalancer],
                ..caps(&["network"], &[])
            }),
            "enabled, no Octavia"
        );
        // Aodh (roadmap 79): its own Monitoring group, opt-in like the others.
        let alarms = |c: &Capabilities| available("/project/alarms/a1", c);
        assert!(!alarms(&caps(&["alarming"], &[])), "in the catalog but not enabled");
        assert!(alarms(&Capabilities {
            plugins: vec![Plugin::Alarming],
            ..caps(&["alarming"], &[])
        }));
        assert!(
            !alarms(&Capabilities {
                plugins: vec![Plugin::Alarming],
                ..caps(&["compute"], &[])
            }),
            "enabled, no Aodh"
        );
        assert_eq!(
            active("/project/alarms/a1").map(|(g, p)| (g.label, p.label)),
            Some((K::Monitoring, K::Alarms))
        );
    }

    #[test]
    fn backups_need_the_backup_plugin_even_with_cinder() {
        let backups = |c: &Capabilities| available("/project/backups/b1", c);
        let cinder = caps(&["block-storage"], &[]);
        assert!(!backups(&cinder), "Cinder is there but the plugin is off (the default)");
        assert!(backups(&Capabilities {
            plugins: vec![Plugin::Backup],
            ..cinder
        }));
        assert!(
            !backups(&Capabilities {
                plugins: vec![Plugin::Backup],
                ..caps(&["compute"], &[])
            }),
            "enabled, no Cinder"
        );
        assert!(
            !backups(&Capabilities::default()),
            "unknown capabilities still hide plugins"
        );
    }

    #[test]
    fn resolves_active_group_and_panel() {
        let (g, p) = active("/project").unwrap();
        assert_eq!((g.label, p.label), (K::Project, K::Overview), "next to API Access");
        let (g, p) = active("/project/quotas").unwrap();
        assert_eq!((g.label, p.label), (K::Project, K::Quotas), "roadmap 82");
        let (g, p) = active("/project/instances/3f2a/").unwrap();
        assert_eq!((g.label, p.label), (K::Compute, K::Instances));
        let (g, _) = active("/project/floating_ips").unwrap();
        assert_eq!(g.label, K::NetworkGroup);
        let (g, p) = active("/project/stacks/s1").unwrap();
        assert_eq!((g.label, p.label), (K::Orchestration, K::Stacks));
        let (g, p) = active("/project/template_generator").unwrap();
        assert_eq!(
            (g.label, p.label),
            (K::Orchestration, K::TemplateGenerator),
            "roadmap 92"
        );
        let (_, p) = active("/project/network_topology").unwrap();
        assert_eq!(p.label, K::NetworkTopology, "not taken by the Networks panel");
        // prefix must stop at a path boundary
        assert!(active("/project/instancesX").is_none());
        assert!(active("/administrator").is_none(), "not the Admin dashboard");
    }

    #[test]
    fn the_admin_dashboard_needs_the_admin_role() {
        let with = |roles: &[&str]| Capabilities {
            known: true,
            roles: roles.iter().map(|r| r.to_string()).collect(),
            ..Default::default()
        };
        assert_eq!(dashboard("/admin/info").0, K::Admin);
        assert_eq!(
            active("/admin/info").map(|(g, p)| (g.label, p.label)),
            Some((K::System, K::SystemInformation))
        );
        let admin = Capabilities {
            services: vec!["compute".into(), "image".into()],
            ..with(&["admin"])
        };
        let system = ADMIN_NAV.iter().find(|g| g.label == K::System).unwrap();
        assert_eq!(
            system
                .visible(&admin)
                .unwrap()
                .iter()
                .map(|p| p.label)
                .collect::<Vec<_>>(),
            [K::QuotaDefaults, K::MetadataDefinitions, K::SystemInformation],
            "Horizon's System group"
        );
        let no_glance = Capabilities {
            services: vec!["compute".into()],
            ..admin
        };
        assert!(
            !available("/admin/metadata_defs", &no_glance),
            "Glance holds the namespaces"
        );
        assert_eq!(
            active("/admin/metadata_defs/OS::Compute::Watchdog").map(|(g, p)| (g.label, p.label)),
            Some((K::System, K::MetadataDefinitions)),
            "the detail (roadmap 89)"
        );
        assert!(
            !available("/admin/metadata_defs/OS::Compute::Watchdog", &no_glance),
            "the detail too"
        );
        let groups = |c: &Capabilities| {
            ADMIN_NAV
                .iter()
                .filter_map(|g| g.visible(c).map(|_| g.label))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            groups(&Capabilities {
                roles: vec!["admin".into()],
                ..Capabilities::default()
            }),
            [K::Admin, K::Compute, K::VolumeAdminGroup, K::NetworkGroup, K::System],
            "Horizon's groups"
        );
        assert_eq!(
            active("/admin").map(|(g, p)| (g.label, p.label)),
            Some((K::Admin, K::Overview)),
            "/admin is Admin › Overview"
        );
        assert_eq!(
            active("/admin/flavors").map(|(_, p)| p.label),
            Some(K::Flavors),
            "the Overview doesn't own the dashboard's pages"
        );
        assert_eq!(
            groups(&no_glance),
            [K::Admin, K::Compute, K::System],
            "no Cinder, no Neutron"
        );
        let no_nova = Capabilities {
            services: vec!["image".into()],
            ..with(&["admin"])
        };
        assert!(!groups(&no_nova).contains(&K::Admin), "Overview is Nova's usage");
        assert!(groups(&with(&["member"])).is_empty());
        assert_eq!(
            active("/admin/instances/i1").map(|(g, p)| (g.label, p.label)),
            Some((K::Compute, K::Instances))
        );
        assert_eq!(
            active("/admin/vg_snapshots").map(|(g, p)| (g.label, p.label)),
            Some((K::VolumeAdminGroup, K::GroupSnapshots))
        );
        assert!(available("/admin/info", &with(&["admin", "member"])));
        assert!(
            !available("/admin/info", &with(&["member", "power_user"])),
            "the page is gated, not only the menu"
        );
        assert!(ADMIN_NAV[3].visible(&with(&["reader"])).is_none());
        // A missing role is a refusal, not a missing service (the page said
        // "Service not available in this region" before).
        assert_eq!(
            unavailable("/admin/info", &with(&["member"])),
            Some(ErrorKind::Forbidden)
        );
        assert_eq!(
            unavailable("/identity/groups", &with(&["member"])),
            Some(ErrorKind::Forbidden)
        );
        assert_eq!(unavailable("/admin/info", &with(&["admin"])), None);
        let no_cinder = Capabilities {
            known: true,
            services: vec!["compute".into()],
            ..Default::default()
        };
        assert_eq!(
            unavailable("/project/volumes", &no_cinder),
            Some(ErrorKind::ServiceUnavailable)
        );
    }
}
