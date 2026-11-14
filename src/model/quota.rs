//! Project quotas (roadmap 82).

use super::*;

/// How a quota's numbers are displayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaUnit {
    Count,
    /// value is in MiB
    Mib,
    /// value is in GiB
    Gib,
}

/// The quotas the Overview shows, as in Horizon's *Limit Summary*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaKey {
    Instances,
    Vcpus,
    Ram,
    Volumes,
    VolumeSnapshots,
    VolumeStorage,
    FloatingIps,
    SecurityGroups,
    SecurityGroupRules,
    Networks,
    Ports,
    Routers,
}

/// The service a quota belongs to (one card each on the Overview).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaGroup {
    Compute,
    Volume,
    Network,
}

/// One quota: used and limit, in `unit`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quota {
    pub key: QuotaKey,
    pub used: i64,
    /// `None` = unlimited (`-1` in the APIs)
    pub limit: Option<i64>,
    pub unit: QuotaUnit,
}

/// The quotas of one service; a failing service keeps the rest of the page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuotaSection {
    pub group: QuotaGroup,
    pub quotas: Vec<Quota>,
    /// Set when that service failed: the rest of the page still renders.
    pub error: Option<String>,
}

/// A service with quotas: one card each on Project › Quotas (roadmap 82).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaService {
    Compute,
    Volume,
    Network,
    /// Octavia, with the `load-balancer` plugin.
    LoadBalancer,
    /// Barbican, with the `key-manager` plugin.
    KeyManager,
}

impl QuotaService {
    /// The keys *Edit Quotas* offers, in display order (Horizon's *Modify
    /// Quotas* fields, plus the newer Neutron and Octavia resources). Other keys
    /// the service reports (Cinder's per-type `gigabytes_<type>`…) are shown
    /// read-only after these. The server refuses any key not listed here.
    pub fn keys(self) -> &'static [&'static str] {
        match self {
            QuotaService::Compute => &[
                "instances",
                "cores",
                "ram",
                "key_pairs",
                "server_groups",
                "server_group_members",
                "metadata_items",
                "injected_files",
                "injected_file_content_bytes",
                "injected_file_path_bytes",
            ],
            QuotaService::Volume => &[
                "volumes",
                "snapshots",
                "gigabytes",
                "backups",
                "backup_gigabytes",
                "groups",
                "per_volume_gigabytes",
            ],
            QuotaService::Network => &[
                "network",
                "subnet",
                "subnetpool",
                "port",
                "router",
                "floatingip",
                "security_group",
                "security_group_rule",
                "rbac_policy",
                "trunk",
            ],
            QuotaService::LoadBalancer => &[
                "load_balancer",
                "listener",
                "pool",
                "member",
                "health_monitor",
                "l7policy",
                "l7rule",
            ],
            // Barbican's `project_quotas` (Horizon has no form for them).
            QuotaService::KeyManager => &["secrets", "containers", "orders", "consumers", "cas"],
        }
    }

    /// How `key`'s numbers read: Nova's `ram` is MiB, Cinder's `*gigabytes*` GiB.
    pub fn unit(self, key: &str) -> QuotaUnit {
        match self {
            QuotaService::Compute if key == "ram" => QuotaUnit::Mib,
            QuotaService::Volume if key.contains("gigabytes") => QuotaUnit::Gib,
            _ => QuotaUnit::Count,
        }
    }
}

/// One quota of Project › Quotas, under the API's own key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuotaRow {
    pub key: String,
    /// `None` when the service doesn't report usage (Octavia, Barbican's
    /// consumers and CAs).
    pub used: Option<i64>,
    /// Nova's and Cinder's reservations in flight; `None` when not reported.
    pub reserved: Option<i64>,
    /// `None` = unlimited (`-1` in the APIs)
    pub limit: Option<i64>,
}

/// One service's quotas; a failing service keeps the rest of the page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuotaCard {
    pub service: QuotaService,
    /// The project these quotas belong to (what *Edit Quotas* updates).
    pub project_id: String,
    pub rows: Vec<QuotaRow>,
    /// Set when that service failed.
    pub error: Option<String>,
}

/// Identity › Projects › *Modify Quotas* (roadmap 82): another project's
/// quota cards, with its name (the ID when Keystone didn't say).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectQuotas {
    pub project_id: String,
    pub project_name: String,
    pub cards: Vec<QuotaCard>,
}

/// *Edit Quotas*: the limits that changed (`-1` = unlimited), refused when one
/// is below what is used. Empty when nothing changed.
pub fn quota_changes(
    rows: &[QuotaRow],
    wanted: &[(String, i64)],
) -> Result<std::collections::BTreeMap<String, i64>, String> {
    let mut out = std::collections::BTreeMap::new();
    for (k, v) in wanted {
        let Some(row) = rows.iter().find(|r| r.key == *k) else {
            continue;
        };
        if *v < -1 {
            return Err(format!("{k}: -1 (unlimited) or more"));
        }
        if row.limit.unwrap_or(-1) == *v {
            continue;
        }
        // Horizon's check; Nova refuses it anyway, Neutron would accept it.
        if let Some(used) = row.used.filter(|u| *v != -1 && *v < *u) {
            return Err(format!("{k}: {v} is below the {used} in use"));
        }
        out.insert(k.clone(), *v);
    }
    Ok(out)
}

#[cfg(test)]
mod quota_tests {
    use super::*;

    fn row(key: &str, used: Option<i64>, limit: Option<i64>) -> QuotaRow {
        QuotaRow {
            key: key.into(),
            used,
            reserved: None,
            limit,
        }
    }

    #[test]
    fn quota_changes_keep_only_what_changed_and_refuse_below_usage() {
        let rows = [
            row("instances", Some(4), Some(10)),
            row("cores", Some(8), None),
            row("member", None, Some(50)),
        ];
        let w = |v: &[(&str, i64)]| v.iter().map(|(k, n)| (k.to_string(), *n)).collect::<Vec<_>>();
        assert!(
            quota_changes(&rows, &w(&[("instances", 10), ("cores", -1)]))
                .unwrap()
                .is_empty(),
            "unchanged, unlimited = -1"
        );
        let c = quota_changes(
            &rows,
            &w(&[("instances", 4), ("cores", 20), ("member", 0), ("unknown", 3)]),
        )
        .unwrap();
        assert_eq!(
            c.into_iter().collect::<Vec<_>>(),
            [
                ("cores".to_string(), 20),
                ("instances".to_string(), 4),
                ("member".to_string(), 0)
            ],
            "no usage reported: any value"
        );
        assert!(
            quota_changes(&rows, &w(&[("instances", 3)]))
                .unwrap_err()
                .contains("below the 4 in use")
        );
        assert!(
            quota_changes(&rows, &w(&[("instances", -1)])).is_ok(),
            "unlimited is never below usage"
        );
        assert!(quota_changes(&rows, &w(&[("instances", -2)])).is_err());
    }

    #[test]
    fn quota_units_and_load_balancer_plugin() {
        assert_eq!(QuotaService::Compute.unit("ram"), QuotaUnit::Mib);
        assert_eq!(QuotaService::Volume.unit("gigabytes_ceph"), QuotaUnit::Gib);
        assert_eq!(QuotaService::Network.unit("port"), QuotaUnit::Count);
        let lb = OsAction::QuotaUpdate {
            service: QuotaService::LoadBalancer,
            project_id: "p".into(),
            values: Default::default(),
        };
        assert_eq!(lb.plugins(), [Plugin::LoadBalancer]);
        let km = OsAction::QuotaUpdate {
            service: QuotaService::KeyManager,
            project_id: "p".into(),
            values: Default::default(),
        };
        assert_eq!(
            km.plugins(),
            [Plugin::KeyManager],
            "Barbican quotas need the key-manager plugin"
        );
    }

    #[test]
    fn another_projects_quotas_need_the_admin_role() {
        let q = |p: &str| OsAction::QuotaUpdate {
            service: QuotaService::Compute,
            project_id: p.into(),
            values: Default::default(),
        };
        assert!(
            !q("mine").other_projects_quotas("mine"),
            "the session's project: the cloud's policy decides"
        );
        assert!(q("other").other_projects_quotas("mine"));
        assert!(
            OsAction::Batch(vec![("a".into(), q("mine")), ("b".into(), q("other"))]).other_projects_quotas("mine"),
            "inside a batch too"
        );
        assert!(!OsAction::ServerStart { id: "x".into() }.other_projects_quotas("mine"));
    }

    #[test]
    fn own_quotas_are_the_session_projects_only() {
        let q = |p: &str| OsAction::QuotaUpdate {
            service: QuotaService::Network,
            project_id: p.into(),
            values: Default::default(),
        };
        assert!(q("mine").own_quotas("mine"));
        assert!(!q("other").own_quotas("mine"), "another project: the admin rule");
        assert!(
            OsAction::Batch(vec![("a".into(), q("other")), ("b".into(), q("mine"))]).own_quotas("mine"),
            "inside a batch too"
        );
        assert!(!OsAction::ServerStart { id: "x".into() }.own_quotas("mine"));
    }
}
