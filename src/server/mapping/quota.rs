//! Quotas and usage.

use super::*;

pub(super) fn limit(v: Option<i64>) -> Option<i64> {
    v.filter(|x| *x >= 0)
}

/// Nova `GET /limits` → `{"absolute": {...}}`
pub fn compute_quotas(limits: &Value) -> Vec<Quota> {
    let a = limits.get("absolute").unwrap_or(limits);
    let q = |key, used: &str, max: &str, unit| Quota {
        key,
        used: i(a, used).unwrap_or(0),
        limit: limit(i(a, max)),
        unit,
    };
    vec![
        q(
            QuotaKey::Instances,
            "totalInstancesUsed",
            "maxTotalInstances",
            QuotaUnit::Count,
        ),
        q(QuotaKey::Vcpus, "totalCoresUsed", "maxTotalCores", QuotaUnit::Count),
        q(QuotaKey::Ram, "totalRAMUsed", "maxTotalRAMSize", QuotaUnit::Mib),
    ]
}

/// Cinder `GET /limits` → `{"absolute": {...}}`
pub fn volume_quotas(limits: &Value) -> Vec<Quota> {
    let a = limits.get("absolute").unwrap_or(limits);
    let q = |key, used: &str, max: &str, unit| Quota {
        key,
        used: i(a, used).unwrap_or(0),
        limit: limit(i(a, max)),
        unit,
    };
    vec![
        q(
            QuotaKey::Volumes,
            "totalVolumesUsed",
            "maxTotalVolumes",
            QuotaUnit::Count,
        ),
        q(
            QuotaKey::VolumeSnapshots,
            "totalSnapshotsUsed",
            "maxTotalSnapshots",
            QuotaUnit::Count,
        ),
        q(
            QuotaKey::VolumeStorage,
            "totalGigabytesUsed",
            "maxTotalVolumeGigabytes",
            QuotaUnit::Gib,
        ),
    ]
}

/// Neutron `GET /quotas/{project}/details` → `{"network": {"used", "limit"}, ...}`
pub fn network_quotas(details: &Value) -> Vec<Quota> {
    let q = |key, res: &str| {
        let r = details.get(res).cloned().unwrap_or(Value::Null);
        Quota {
            key,
            used: i(&r, "used").unwrap_or(0),
            limit: limit(i(&r, "limit")),
            unit: QuotaUnit::Count,
        }
    };
    vec![
        q(QuotaKey::FloatingIps, "floatingip"),
        q(QuotaKey::SecurityGroups, "security_group"),
        q(QuotaKey::SecurityGroupRules, "security_group_rule"),
        q(QuotaKey::Networks, "network"),
        q(QuotaKey::Ports, "port"),
        q(QuotaKey::Routers, "router"),
    ]
}

/// Keys a quota set reports that are no quota of this service: Nova's
/// network proxies (gone from 2.36 on, Neutron's own quotas apply) and
/// Octavia's deprecated aliases of `load_balancer` and `health_monitor`.
const NOT_QUOTAS: &[&str] = &[
    "id",
    "project_id",
    "tenant_id",
    "floating_ips",
    "fixed_ips",
    "security_groups",
    "security_group_rules",
    "networks",
    "loadbalancer",
    "healthmonitor",
];

/// Project › Quotas (roadmap 82): every quota of one service's quota set, with
/// the response key stripped. Values are either `{limit, in_use|used, reserved}`
/// (Nova `os-quota-sets/{id}/detail`, Cinder `?usage=true`, Neutron
/// `quotas/{id}/details`) or a bare limit (Octavia), where `null` means the
/// cloud's default, taken from `defaults` (Octavia `lbaas/quotas/defaults`).
/// The editable keys come first, in `service.keys()` order, then the rest by name.
pub fn quota_rows(service: QuotaService, set: &Value, defaults: &Value) -> Vec<QuotaRow> {
    let Some(m) = set.as_object() else { return vec![] };
    let mut rows: Vec<QuotaRow> = m
        .iter()
        .filter(|(k, _)| !NOT_QUOTAS.contains(&k.as_str()))
        .filter_map(|(k, v)| {
            let row = match v {
                Value::Object(_) => QuotaRow {
                    key: k.clone(),
                    used: i(v, "in_use").or_else(|| i(v, "used")),
                    reserved: i(v, "reserved"),
                    limit: limit(i(v, "limit")),
                },
                Value::Number(n) => QuotaRow {
                    key: k.clone(),
                    used: None,
                    reserved: None,
                    limit: limit(n.as_i64()),
                },
                Value::Null => QuotaRow {
                    key: k.clone(),
                    used: None,
                    reserved: None,
                    limit: limit(i(defaults, k)),
                },
                _ => return None,
            };
            Some(row)
        })
        .collect();
    let keys = service.keys();
    // Cinder's per-type keys are grouped by volume type: volumes, snapshots,
    // size of `ceph`, then of the next type.
    let group = |k: &str| -> (String, usize) {
        let per_type = ["volumes_", "snapshots_", "gigabytes_"];
        per_type
            .iter()
            .enumerate()
            .find_map(|(i, p)| {
                k.strip_prefix(p)
                    .filter(|_| service == QuotaService::Volume)
                    .map(|ty| (ty.to_string(), i))
            })
            .unwrap_or_else(|| (k.to_string(), 0))
    };
    rows.sort_by(|a, b| {
        let pos = |r: &QuotaRow| keys.iter().position(|k| *k == r.key).unwrap_or(usize::MAX);
        pos(a).cmp(&pos(b)).then_with(|| group(&a.key).cmp(&group(&b.key)))
    });
    rows
}

/// Barbican's quotas (roadmap 82): `GET v1/quotas` (`{"quotas": {"secrets": 10…}}`,
/// the token's project, defaults applied, `-1` unlimited) with *Used* read
/// from the `total` of one-item list pages (`secrets`, `containers`, `orders`),
/// as Barbican reports no usage. A list that failed (`Null`) leaves `-`.
pub fn key_manager_quota_rows(quotas: &Value, totals: &[(&str, &Value)]) -> Vec<QuotaRow> {
    let set = quotas.get("quotas").unwrap_or(quotas);
    let mut rows = quota_rows(QuotaService::KeyManager, set, &Value::Null);
    for r in &mut rows {
        r.used = totals
            .iter()
            .find(|(k, _)| *k == r.key)
            .and_then(|(_, page)| i(page, "total"));
    }
    rows
}

/// Barbican's quotas of **another** project (roadmap 82, Identity › *Modify
/// Quotas*): `GET v1/project-quotas/{p}` (`target`, `{"project_quotas": {…}}`,
/// `None` on 404: nothing set) holds only the values set for it; the others
/// follow the deployment's defaults, which Barbican doesn't publish. `v1/quotas`
/// (`effective`) is the session's project with defaults applied, so it gives the
/// defaults only when that project has no own values (`mine` is `Some(None)`, a
/// 404, or all `null`); `mine` is `None` when it couldn't be read. Otherwise
/// keys without their own value are left out rather than guessed.
/// Barbican counts nothing per project: *Used* stays `-`.
pub fn key_manager_project_rows(
    target: Option<&Value>,
    mine: Option<Option<&Value>>,
    effective: &Value,
) -> Vec<QuotaRow> {
    let own = |v: Option<&Value>| {
        v.map(|v| v.get("project_quotas").unwrap_or(v).clone())
            .unwrap_or(Value::Null)
    };
    let t = own(target);
    let mine_empty = mine.is_some_and(|m| own(m).as_object().is_none_or(|o| o.values().all(Value::is_null)));
    let defaults = if mine_empty {
        effective.get("quotas").unwrap_or(effective).clone()
    } else {
        Value::Null
    };
    let set: serde_json::Map<String, Value> = QuotaService::KeyManager
        .keys()
        .iter()
        .map(|k| (k.to_string(), t.get(*k).cloned().unwrap_or(Value::Null)))
        .collect();
    let known = |k: &str| t.get(k).is_some_and(|v| !v.is_null()) || defaults.get(k).is_some_and(|v| !v.is_null());
    let mut rows = quota_rows(QuotaService::KeyManager, &Value::Object(set), &defaults);
    rows.retain(|r| known(&r.key));
    rows
}

/// `os-simple-tenant-usage/{project}` (response key already stripped).
pub fn server_usages(v: &Value) -> Vec<ServerUsage> {
    arr(v, "server_usages")
        .iter()
        .map(|u| {
            let id = s(u, "instance_id").unwrap_or_default();
            ServerUsage {
                name: s_ne(u, "name").unwrap_or_else(|| id.clone()),
                id,
                vcpus: i(u, "vcpus").unwrap_or(0),
                memory_mb: i(u, "memory_mb").unwrap_or(0),
                local_gb: i(u, "local_gb").unwrap_or(0),
                hours: u.get("hours").and_then(Value::as_f64).unwrap_or(0.0),
                uptime: i(u, "uptime").unwrap_or(0),
                state: s(u, "state").unwrap_or_default(),
            }
        })
        .collect()
}

/// Admin › Overview: Nova's detailed `tenant_usages`, from every page, one line
/// per project sorted by name. Paging is by instance, so a project can come back
/// on two pages: merged by `tenant_id`. Projects Keystone doesn't name keep their ID.
pub fn tenant_usages(raw: &[Value], names: &HashMap<String, String>) -> Vec<ProjectUsage> {
    let mut by_id: Vec<ProjectUsage> = Vec::new();
    for t in raw {
        let id = s(t, "tenant_id").unwrap_or_default();
        let servers = server_usages(t);
        match by_id.iter_mut().find(|p| p.project_id == id) {
            Some(p) => p.servers.extend(servers),
            None => by_id.push(ProjectUsage {
                project: names.get(&id).cloned().unwrap_or_else(|| id.clone()),
                // Only when Keystone answered: a failed lookup names no one.
                deleted: !names.is_empty() && !names.contains_key(&id),
                project_id: id,
                servers,
            }),
        }
    }
    by_id.sort_by_key(|a| a.project.to_lowercase());
    by_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quota_rows_of_every_service_shape() {
        let keys = |r: &[QuotaRow]| r.iter().map(|x| x.key.clone()).collect::<Vec<_>>();
        // Nova detail: the network proxies (pre-2.36) and `id` are dropped.
        let nova = json!({"id": "p1", "cores": {"in_use": 8, "limit": 20, "reserved": 1},
            "instances": {"in_use": 3, "limit": -1, "reserved": 0}, "floating_ips": {"in_use": 0, "limit": 10, "reserved": 0},
            "ram": {"in_use": 16384, "limit": 51200, "reserved": 0}});
        let r = quota_rows(QuotaService::Compute, &nova, &Value::Null);
        assert_eq!(keys(&r), ["instances", "cores", "ram"], "editable order, proxies gone");
        assert_eq!((r[0].used, r[0].limit, r[1].reserved), (Some(3), None, Some(1)));
        // Cinder with usage: per-type keys follow the known ones, by type.
        let cinder = json!({"id": "p1", "volumes_ceph": {"limit": -1, "in_use": 1, "reserved": 0, "allocated": 0},
            "gigabytes": {"limit": 1000, "in_use": 250, "reserved": 0, "allocated": 0},
            "gigabytes_ceph": {"limit": 500, "in_use": 20, "reserved": 0, "allocated": 0}, "volumes": {"limit": 10, "in_use": 2, "reserved": 0}});
        let r = quota_rows(QuotaService::Volume, &cinder, &Value::Null);
        assert_eq!(
            keys(&r),
            ["volumes", "gigabytes", "volumes_ceph", "gigabytes_ceph"],
            "grouped by type"
        );
        assert_eq!((r[1].used, r[1].limit), (Some(250), Some(1000)));
        // Neutron details say `used`.
        let r = quota_rows(
            QuotaService::Network,
            &json!({"port": {"used": 4, "limit": 500, "reserved": 0}}),
            &Value::Null,
        );
        assert_eq!((r[0].used, r[0].limit), (Some(4), Some(500)));
        // Octavia: bare limits, `null` = the default, deprecated aliases dropped, no usage.
        let lb = json!({"load_balancer": 5, "loadbalancer": 5, "listener": null, "member": -1, "health_monitor": 3, "healthmonitor": 3});
        let r = quota_rows(QuotaService::LoadBalancer, &lb, &json!({"listener": 20}));
        assert_eq!(
            keys(&r),
            ["load_balancer", "listener", "member", "health_monitor"],
            "current names, as ttcp-dev's Octavia returns"
        );
        assert_eq!(
            (r[0].used, r[0].limit, r[1].limit, r[2].limit),
            (None, Some(5), Some(20), None)
        );
        assert!(quota_rows(QuotaService::Network, &Value::Null, &Value::Null).is_empty());
    }

    #[test]
    fn quotas_treat_minus_one_as_unlimited() {
        let nova = json!({"absolute": {"maxTotalInstances": -1, "totalInstancesUsed": 3,
            "maxTotalCores": 20, "totalCoresUsed": 8, "maxTotalRAMSize": 51200, "totalRAMUsed": 16384}});
        let q = compute_quotas(&nova);
        assert_eq!((q[0].used, q[0].limit), (3, None));
        assert_eq!((q[1].used, q[1].limit), (8, Some(20)));
        assert_eq!(q[2].unit, QuotaUnit::Mib);

        let neutron =
            json!({"port": {"used": 0, "limit": 500, "reserved": 0}, "security_group": {"used": 1, "limit": -1}});
        let q = network_quotas(&neutron);
        let port = q.iter().find(|x| x.key == QuotaKey::Ports).unwrap();
        assert_eq!(port.limit, Some(500));
        let sg = q.iter().find(|x| x.key == QuotaKey::SecurityGroups).unwrap();
        assert_eq!((sg.used, sg.limit), (1, None));
        // a missing resource is reported as 0 used, unlimited
        let routers = q.iter().find(|x| x.key == QuotaKey::Routers).unwrap();
        assert_eq!((routers.used, routers.limit), (0, None));

        let cinder = json!({"absolute": {"maxTotalVolumeGigabytes": 1000, "totalGigabytesUsed": 250}});
        let v = volume_quotas(&cinder);
        assert_eq!(v[2].unit, QuotaUnit::Gib);
        assert_eq!((v[2].used, v[2].limit), (250, Some(1000)));
    }

    #[test]
    fn server_usages_fall_back_to_the_id_and_tolerate_missing_fields() {
        let v = json!({"server_usages": [
            {"instance_id": "i1", "name": "web", "vcpus": 2, "memory_mb": 4096, "local_gb": 20,
             "hours": 12.5, "uptime": 45000, "state": "active", "ended_at": null},
            {"instance_id": "i2", "name": "", "hours": 1, "state": "terminated"}
        ], "total_hours": 13.5});
        let out = server_usages(&v);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].hours, 12.5);
        assert_eq!(out[1].name, "i2");
        assert_eq!(out[1].vcpus, 0);
        assert!(!out[1].active());
        assert!(server_usages(&json!({})).is_empty());
    }

    #[test]
    fn default_quota_classes_map_like_project_quotas() {
        // Nova's `os-quota-class-sets/default` at 2.1: plain numbers, its `id`
        // and the pre-2.36 network proxies, which Horizon's Defaults don't show.
        let nova = json!({"id": "default", "instances": 10, "cores": 20, "ram": 51200, "floating_ips": 10, "fixed_ips": -1,
                          "security_groups": 10, "security_group_rules": 20, "key_pairs": 100, "server_groups": 10,
                          "server_group_members": 10, "metadata_items": 128, "injected_files": 5,
                          "injected_file_content_bytes": 10240, "injected_file_path_bytes": 255});
        let rows = quota_rows(QuotaService::Compute, &nova, &Value::Null);
        assert_eq!(rows.len(), 10, "Horizon's ten compute defaults");
        assert_eq!(
            (rows[0].key.as_str(), rows[0].limit, rows[0].used),
            ("instances", Some(10), None)
        );
        let cinder = json!({"id": "default", "volumes": -1, "gigabytes": 1000, "volumes_ceph": -1});
        let rows = quota_rows(QuotaService::Volume, &cinder, &Value::Null);
        assert_eq!(
            rows.iter().map(|r| (r.key.as_str(), r.limit)).collect::<Vec<_>>(),
            [("volumes", None), ("gigabytes", Some(1000)), ("volumes_ceph", None)],
            "-1 = unlimited"
        );
    }

    #[test]
    fn tenant_usages_merge_pages_and_name_projects() {
        let u = |id: &str, vcpus: i64, state: &str| json!({"instance_id": id, "name": id, "vcpus": vcpus, "memory_mb": 1024, "local_gb": 10, "hours": 2.0, "uptime": 7200, "state": state});
        // Page 1 ends inside p1; page 2 carries p1's last server and p2.
        let raw = vec![
            json!({"tenant_id": "p1", "server_usages": [u("a", 2, "active")]}),
            json!({"tenant_id": "p1", "server_usages": [u("b", 4, "terminated")]}),
            json!({"tenant_id": "p2", "server_usages": [u("c", 1, "active")]}),
        ];
        let names = HashMap::from([("p1".to_string(), "zeta".to_string())]);
        let out = tenant_usages(&raw, &names);
        assert_eq!(
            out.iter().map(|p| p.project.as_str()).collect::<Vec<_>>(),
            ["p2", "zeta"],
            "one line per project, by name; unnamed → ID"
        );
        assert_eq!(
            out.iter().map(|p| p.deleted).collect::<Vec<_>>(),
            [true, false],
            "Keystone names zeta, not p2: Horizon's (Deleted)"
        );
        assert!(
            tenant_usages(&raw, &HashMap::new()).iter().all(|p| !p.deleted),
            "no names at all: nothing marked"
        );
        let zeta = &out[1];
        assert_eq!(zeta.servers.len(), 2, "the split project is merged");
        assert_eq!(
            (zeta.vcpus(), zeta.memory_mb(), zeta.local_gb()),
            (2, 1024, 10),
            "sizes count only servers that still exist"
        );
        assert_eq!(zeta.vcpu_hours(), 12.0, "hours count the ended one too: 2×2 + 4×2");
    }

    #[test]
    fn another_projects_barbican_quotas_use_known_defaults_only() {
        // Roadmap 82: Barbican has no usage nor defaults per project; `v1/quotas`
        // of the session's project are the defaults only when it sets nothing.
        let target = json!({"project_quotas": {"secrets": 50, "orders": null}});
        let effective = json!({"quotas": {"secrets": -1, "containers": 20, "orders": 10, "consumers": -1, "cas": 5}});
        let rows = key_manager_project_rows(Some(&target), Some(None), &effective);
        let got: Vec<_> = rows.iter().map(|r| (r.key.as_str(), r.limit, r.used)).collect();
        assert_eq!(
            got,
            [
                ("secrets", Some(50), None),
                ("containers", Some(20), None),
                ("orders", Some(10), None),
                ("consumers", None, None),
                ("cas", Some(5), None)
            ]
        );
        let own = json!({"project_quotas": {"secrets": 3}});
        let rows = key_manager_project_rows(Some(&target), Some(Some(&own)), &effective);
        assert_eq!(
            rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(),
            ["secrets"],
            "defaults unknown: only the target's own values"
        );
        assert_eq!(
            key_manager_project_rows(None, None, &effective).len(),
            0,
            "nothing set and defaults unknown: no rows"
        );
        assert_eq!(
            key_manager_project_rows(None, Some(None), &effective).len(),
            5,
            "nothing set: every default"
        );
    }
}
