//! Admin: services, metadata definitions, hypervisors, placement, aggregates, flavors, volume types, RBAC, agents.

use super::*;

/// Nova's or Cinder's `os-services` → *Compute* / *Block Storage Services*,
/// sorted by binary then host, as Horizon lists them grouped.
pub fn host_services(raw: &[Value]) -> Vec<HostService> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| HostService {
            // A UUID from compute 2.53; older microversions send a number.
            id: s(v, "id").unwrap_or_default(),
            binary: s(v, "binary").unwrap_or_default(),
            host: s(v, "host").unwrap_or_default(),
            zone: s_ne(v, "zone"),
            enabled: s(v, "status").is_some_and(|x| x.eq_ignore_ascii_case("enabled")),
            up: s(v, "state").is_some_and(|x| x.eq_ignore_ascii_case("up")),
            updated_at: s(v, "updated_at").as_deref().and_then(utc_rfc3339),
            disabled_reason: s_ne(v, "disabled_reason"),
        })
        .collect();
    out.sort_by(|a, b| (&a.binary, &a.host).cmp(&(&b.binary, &b.host)));
    out
}

/// Glance `metadefs/namespaces` → *Metadata Definitions*, sorted by name like
/// Horizon's table.
pub fn metadef_namespaces(raw: &[Value]) -> Vec<MetadefNamespace> {
    let mut out: Vec<_> = raw
        .iter()
        .filter_map(|v| {
            let namespace = s(v, "namespace")?;
            Some(MetadefNamespace {
                name: s_ne(v, "display_name").unwrap_or_else(|| namespace.clone()),
                description: s_ne(v, "description"),
                resource_types: arr(v, "resource_type_associations")
                    .iter()
                    .filter_map(|a| s(a, "name"))
                    .collect(),
                prefixes: arr(v, "resource_type_associations")
                    .iter()
                    .filter_map(|a| Some((s(a, "name")?, s_ne(a, "prefix")?)))
                    .collect(),
                public: s(v, "visibility").as_deref() == Some("public"),
                protected: b(v, "protected"),
                namespace,
            })
        })
        .collect();
    out.sort_by_key(|n| n.name.to_lowercase());
    out
}

/// One namespace's definition (`GET metadefs/namespaces/{ns}`, no `resource_type`):
/// the list's fields, dates, associations, properties and objects.
/// `None` without a `namespace`.
pub fn metadef_namespace_detail(v: &Value) -> Option<MetadefNamespaceDetail> {
    let namespace = metadef_namespaces(std::slice::from_ref(v)).into_iter().next()?;
    let text = |x: &Value| x.as_str().map_or_else(|| x.to_string(), str::to_string);
    let property = |name: &str, p: &Value| MetadefProperty {
        name: name.to_string(),
        title: s_ne(p, "title"),
        kind: s(p, "type").unwrap_or_default(),
        description: s_ne(p, "description"),
        default: p.get("default").filter(|d| !d.is_null()).map(text),
        allowed: arr(p, "enum")
            .iter()
            .chain(p.get("items").map(|i| arr(i, "enum")).unwrap_or(&[]))
            .map(text)
            .collect(),
    };
    let mut properties: Vec<MetadefProperty> = v
        .get("properties")
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, p)| property(k, p)).collect())
        .unwrap_or_default();
    properties.sort_by_key(|p| p.name.to_lowercase());
    let mut objects: Vec<MetadefObject> = arr(v, "objects")
        .iter()
        .filter_map(|o| {
            let mut props: Vec<String> = o
                .get("properties")
                .and_then(Value::as_object)
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            props.sort_by_key(|p| p.to_lowercase());
            Some(MetadefObject {
                name: s(o, "name")?,
                description: s_ne(o, "description"),
                properties: props,
            })
        })
        .collect();
    objects.sort_by_key(|o| o.name.to_lowercase());
    Some(MetadefNamespaceDetail {
        namespace,
        created_at: s_ne(v, "created_at"),
        updated_at: s_ne(v, "updated_at"),
        associations: arr(v, "resource_type_associations")
            .iter()
            .filter_map(|a| {
                Some(MetadefAssociation {
                    name: s(a, "name")?,
                    prefix: s_ne(a, "prefix"),
                    properties_target: s_ne(a, "properties_target"),
                })
            })
            .collect(),
        properties,
        objects,
        raw: serde_json::to_string_pretty(v).unwrap_or_default(),
    })
}

/// The properties one metadef namespace offers for a resource type: the body of
/// `GET metadefs/namespaces/{ns}?resource_type=…`. Glance has already put the association's
/// `prefix` on the names in that response (`hw:cpu_policy`), also on an object's properties;
/// applying it again gave `CIM_PASD_CIM_PASD_…` on ttcp-dev. The namespace's own `properties`
/// come first, then each of its `objects`' (titled `Object › Property`, as a namespace groups
/// them). A `readonly` property is skipped, an array or object type is offered as free text, and
/// a key an earlier entry already offers is not repeated. Sorted by title.
pub fn metadef_options(ns: &Value) -> Vec<MetadataOption> {
    let namespace = s_ne(ns, "display_name")
        .or_else(|| s(ns, "namespace"))
        .unwrap_or_default();
    let text = |v: &Value| match v {
        Value::String(x) => Some(x.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    };
    let of = |props: Option<&Value>, group: Option<&str>| -> Vec<MetadataOption> {
        props
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter(|(name, p)| !name.is_empty() && !b(p, "readonly"))
            .map(|(name, p)| {
                let title = s_ne(p, "title").unwrap_or_else(|| name.clone());
                MetadataOption {
                    key: name.clone(),
                    title: group.map_or(title.clone(), |g| format!("{g} › {title}")),
                    description: s_ne(p, "description"),
                    kind: match s(p, "type").as_deref() {
                        Some("integer") => MetadataKind::Integer,
                        Some("number") => MetadataKind::Number,
                        Some("boolean") => MetadataKind::Boolean,
                        _ => MetadataKind::Text,
                    },
                    choices: arr(p, "enum").iter().filter_map(text).collect(),
                    default: p.get("default").and_then(text),
                    namespace: namespace.clone(),
                }
            })
            .collect()
    };
    let mut out = of(ns.get("properties"), None);
    for o in arr(ns, "objects") {
        let name = s_ne(o, "name").unwrap_or_default();
        let group = (!name.is_empty()).then_some(name.as_str());
        out.extend(of(o.get("properties"), group));
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|o| seen.insert(o.key.clone()));
    out.sort_by_key(|o| o.title.to_lowercase());
    out
}

/// Adds Horizon's *Project* and *Host* columns to a Project panel's rows:
/// `raw` is the API list they were mapped from (joined by `id`), `project_key`
/// and `host_key` where that API keeps them, `names` project ID → name.
pub fn owned<T>(
    rows: Vec<T>,
    id_of: impl Fn(&T) -> &str,
    raw: &[Value],
    project_key: &str,
    host_key: Option<&str>,
    names: &HashMap<String, String>,
) -> Vec<Owned<T>> {
    let by_id = index_by_id(raw);
    rows.into_iter()
        .map(|row| {
            let v = by_id.get(id_of(&row)).copied();
            let project_id = v.and_then(|v| s_ne(v, project_key));
            Owned {
                project: project_id
                    .clone()
                    .map(|p| names.get(&p).cloned().unwrap_or(p))
                    .unwrap_or_else(|| "-".into()),
                host: host_key.and_then(|k| v.and_then(|v| s_ne(v, k))),
                project_id,
                row,
            }
        })
        .collect()
}

/// Sorted `(key, value)` pairs of a JSON object; non-string values as JSON.
pub(super) fn pairs(v: &Value, k: &str) -> Vec<(String, String)> {
    let mut out: Vec<_> = v
        .get(k)
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Nova `os-hypervisors/detail` (2.53) → *Hypervisor* tab, by hostname.
pub fn hypervisors(raw: &[Value]) -> Vec<HypervisorRow> {
    let n = |v: &Value, k: &str| i(v, k).unwrap_or(0);
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| HypervisorRow {
            // An integer before 2.53, a UUID from it.
            id: s(v, "id")
                .or_else(|| i(v, "id").map(|x| x.to_string()))
                .unwrap_or_default(),
            hostname: s(v, "hypervisor_hostname").unwrap_or_default(),
            kind: s(v, "hypervisor_type").unwrap_or_default(),
            vcpus_used: n(v, "vcpus_used"),
            vcpus: n(v, "vcpus"),
            ram_used_mb: n(v, "memory_mb_used"),
            ram_mb: n(v, "memory_mb"),
            disk_used_gb: n(v, "local_gb_used"),
            disk_gb: n(v, "local_gb"),
            instances: n(v, "running_vms"),
            up: s(v, "state").as_deref() == Some("up"),
            enabled: s(v, "status").as_deref() == Some("enabled"),
        })
        .collect();
    out.sort_by(|a, b| a.hostname.cmp(&b.hostname));
    out
}

/// A Placement provider (an entry of `resource_providers`) with its
/// `inventories` and `usages` responses: VCPU, MEMORY_MB and DISK_GB. A class
/// missing from the inventory is `None`; a missing usage is 0.
pub fn resource_provider(p: &Value, inventories: &Value, usages: &Value) -> ResourceProviderRow {
    let class = |name: &str| {
        let inv = inventories.get("inventories")?.get(name)?;
        Some(ProviderResource {
            used: usages
                .get("usages")
                .and_then(|u| u.get(name))
                .and_then(Value::as_i64)
                .unwrap_or(0),
            reserved: i(inv, "reserved").unwrap_or(0),
            total: i(inv, "total").unwrap_or(0),
            allocation_ratio: inv.get("allocation_ratio").and_then(Value::as_f64).unwrap_or(1.0),
        })
    };
    const SHOWN: [&str; 4] = ["VCPU", "MEMORY_MB", "DISK_GB", "VGPU"];
    let mut others: Vec<(String, ProviderResource)> = inventories
        .get("inventories")
        .and_then(Value::as_object)
        .map(|m| {
            m.keys()
                .filter(|k| !SHOWN.contains(&k.as_str()))
                .filter_map(|k| Some((k.clone(), class(k)?)))
                .collect()
        })
        .unwrap_or_default();
    others.sort_by(|a, b| a.0.cmp(&b.0));
    let uuid = s(p, "uuid").unwrap_or_default();
    ResourceProviderRow {
        name: s_ne(p, "name").unwrap_or_else(|| uuid.clone()),
        uuid,
        vcpus: class("VCPU"),
        memory_mb: class("MEMORY_MB"),
        disk_gb: class("DISK_GB"),
        // Placement 1.14; `null` for a compute node.
        parent: s_ne(p, "parent_provider_uuid"),
        depth: 0,
        host: String::new(),
        descendants: 0,
        vgpus: class("VGPU"),
        others,
        custom_traits: vec![],
    }
}

/// A provider's `traits` response → its `CUSTOM_*` traits, sorted (the standard
/// ones, `COMPUTE_*`, `HW_*`…, say nothing about a vGPU's type).
pub fn custom_traits(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = strs(v, "traits")
        .into_iter()
        .filter(|t| t.starts_with("CUSTOM_"))
        .collect();
    out.sort();
    out
}

/// Nova `os-aggregates` → *Host Aggregates*, by name.
pub fn aggregates(raw: &[Value]) -> Vec<AggregateRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| {
            let mut hosts = strs(v, "hosts");
            hosts.sort();
            AggregateRow {
                id: s(v, "id")
                    .or_else(|| i(v, "id").map(|x| x.to_string()))
                    .unwrap_or_default(),
                name: s(v, "name").unwrap_or_default(),
                zone: s_ne(v, "availability_zone"),
                hosts,
                metadata: pairs(v, "metadata"),
            }
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Nova `os-availability-zone/detail` → Horizon's *Availability Zones* table:
/// each zone's hosts (the keys of `hosts`), by name.
pub fn zones(raw: &[Value]) -> Vec<ZoneRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| {
            let mut hosts: Vec<String> = v
                .get("hosts")
                .and_then(Value::as_object)
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            hosts.sort();
            ZoneRow {
                name: s(v, "zoneName").unwrap_or_default(),
                hosts,
                available: v.get("zoneState").is_some_and(|z| b(z, "available")),
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Nova `flavors/detail?is_public=None` → *Flavors*, by name. Extra specs
/// come with the flavor from compute 2.61.
pub fn flavor_rows(raw: &[Value]) -> Vec<FlavorRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| FlavorRow {
            id: s(v, "id").unwrap_or_default(),
            name: name_or_id(v),
            vcpus: i(v, "vcpus").unwrap_or(0),
            ram_mb: i(v, "ram").unwrap_or(0),
            disk_gb: i(v, "disk").unwrap_or(0),
            ephemeral_gb: i(v, "OS-FLV-EXT-DATA:ephemeral").unwrap_or(0),
            // `""` when there is no swap.
            swap_mb: i(v, "swap").unwrap_or(0),
            rxtx_factor: v.get("rxtx_factor").and_then(Value::as_f64),
            public: v
                .get("os-flavor-access:is_public")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            extra_specs: pairs(v, "extra_specs"),
            access: vec![],
        })
        .collect();
    out.sort_by_key(|f| f.name.to_lowercase());
    out
}

/// A flavor's `flavor_access` entries (`{"flavor_id", "tenant_id"}`) as
/// project names, sorted; unnamed projects keep their ID.
pub fn flavor_access(raw: &[Value], names: &HashMap<String, String>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = raw
        .iter()
        .filter_map(|a| s(a, "tenant_id"))
        .map(|id| (id.clone(), names.get(&id).cloned().unwrap_or(id)))
        .collect();
    out.sort_by_key(|(_, n)| n.to_lowercase());
    out
}

/// Cinder `types/{id}/encryption` → its spec; `None` for `{}` (not encrypted)
/// or a body without a provider. `key_size` may come as a number or a string.
pub fn encryption_spec(v: &Value) -> Option<EncryptionSpec> {
    let provider = s_ne(v, "provider")?;
    let key_size = v
        .get("key_size")
        .and_then(|k| k.as_u64().or_else(|| k.as_str().and_then(|s| s.parse().ok())))
        .map(|k| k as u32);
    Some(EncryptionSpec {
        provider,
        control_location: s(v, "control_location").unwrap_or_else(|| "front-end".into()),
        cipher: s_ne(v, "cipher"),
        key_size,
    })
}

/// Cinder `types` → *Volume Types*: `encryption` holds each encrypted type's
/// ID and spec, `qos` the QoS specs (for names).
pub fn volume_type_rows(raw: &[Value], encryption: &[(String, EncryptionSpec)], qos: &[Value]) -> Vec<VolumeTypeRow> {
    let qos_names = names_by_id(qos);
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| {
            let id = s(v, "id").unwrap_or_default();
            VolumeTypeRow {
                name: name_or_id(v),
                description: s_ne(v, "description"),
                qos: s_ne(v, "qos_specs_id").map(|q| Ref {
                    name: qos_names.get(&q).cloned().unwrap_or_else(|| q.clone()),
                    id: q,
                }),
                encryption: encryption.iter().find(|(t, _)| *t == id).map(|(_, e)| e.clone()),
                encrypted: encryption.iter().any(|(t, _)| *t == id),
                public: v
                    .get("os-volume-type-access:is_public")
                    .or_else(|| v.get("is_public"))
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
                extra_specs: pairs(v, "extra_specs"),
                id,
            }
        })
        .collect();
    out.sort_by_key(|t| t.name.to_lowercase());
    out
}

/// Cinder `qos-specs` → Horizon's *QoS Specs* table.
pub fn qos_specs(raw: &[Value]) -> Vec<QosSpecRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| QosSpecRow {
            id: s(v, "id").unwrap_or_default(),
            name: name_or_id(v),
            consumer: s(v, "consumer").unwrap_or_default(),
            specs: pairs(v, "specs"),
        })
        .collect();
    out.sort_by_key(|q| q.name.to_lowercase());
    out
}

/// Cinder `group_types` → *Group Types*.
pub fn group_type_rows(raw: &[Value]) -> Vec<GroupTypeRow> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| GroupTypeRow {
            id: s(v, "id").unwrap_or_default(),
            name: name_or_id(v),
            description: s_ne(v, "description"),
            public: v.get("is_public").and_then(Value::as_bool).unwrap_or(true),
            specs: pairs(v, "group_specs"),
        })
        .collect();
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

/// Neutron `rbac-policies` → *RBAC Policies*, projects and networks named.
/// `objects` holds, per RBAC `object_type` (`network`, `qos_policy`,
/// `security_group`, `address_scope`, `subnetpool`, `address_group`), the list
/// that names its objects; an object of another
/// type, or one the list lacks, keeps its ID.
pub fn rbac_policies(
    raw: &[Value],
    objects: &[(&str, &[Value])],
    names: &HashMap<String, String>,
) -> Vec<RbacPolicyRow> {
    let by_type: HashMap<&str, HashMap<String, String>> =
        objects.iter().map(|(ty, list)| (*ty, names_by_id(list))).collect();
    let project = |id: Option<String>| {
        id.map(|p| names.get(&p).cloned().unwrap_or(p))
            .unwrap_or_else(|| "-".into())
    };
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| {
            let object_type = s(v, "object_type").unwrap_or_default();
            let object_id = s(v, "object_id").unwrap_or_default();
            let object_name = by_type
                .get(object_type.as_str())
                .and_then(|m| m.get(&object_id))
                .cloned()
                .unwrap_or_else(|| object_id.clone());
            let target = s(v, "target_tenant").or_else(|| s(v, "target_project"));
            RbacPolicyRow {
                id: s(v, "id").unwrap_or_default(),
                project: project(s_ne(v, "project_id").or_else(|| s_ne(v, "tenant_id"))),
                object: Ref {
                    id: object_id,
                    name: object_name,
                },
                target_id: target.clone().unwrap_or_default(),
                target: match target.as_deref() {
                    Some("*") => "*".into(),
                    _ => project(target),
                },
                action: s(v, "action").unwrap_or_default(),
                object_type,
            }
        })
        .collect();
    out.sort_by(|a, b| (&a.project, &a.object_type, &a.object.name).cmp(&(&b.project, &b.object_type, &b.object.name)));
    out
}

/// Neutron `agents` → *Network Agents*, sorted by type then host.
pub fn network_agents(raw: &[Value]) -> Vec<NetworkAgent> {
    let mut out: Vec<_> = raw
        .iter()
        .map(|v| NetworkAgent {
            agent_type: s(v, "agent_type").unwrap_or_default(),
            binary: s(v, "binary").unwrap_or_default(),
            host: s(v, "host").unwrap_or_default(),
            zone: s_ne(v, "availability_zone"),
            admin_up: b(v, "admin_state_up"),
            alive: b(v, "alive"),
            heartbeat: s(v, "heartbeat_timestamp").as_deref().and_then(utc_rfc3339),
        })
        .collect();
    out.sort_by(|a, b| (&a.agent_type, &a.host).cmp(&(&b.agent_type, &b.host)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resource_types_with_or_without_descriptions() {
        let out = resource_types(&[
            json!({"resource_type": "OS::Nova::Server", "description": "A server."}),
            json!("AWS::EC2::Instance"),
            json!({"resource_type": "OS::Heat::None", "description": ""}),
        ]);
        let pairs: Vec<_> = out
            .iter()
            .map(|r| (r.name.as_str(), r.description.as_deref()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("AWS::EC2::Instance", None),
                ("OS::Heat::None", None),
                ("OS::Nova::Server", Some("A server."))
            ]
        );
    }

    #[test]
    fn os_services_map_status_state_and_zone() {
        let raw = vec![
            json!({"id": 3, "binary": "nova-scheduler", "host": "s-0", "zone": "internal", "status": "enabled", "state": "up", "updated_at": "2026-10-30T10:00:00.000000", "disabled_reason": null}),
            json!({"id": 1, "binary": "nova-compute", "host": "c-1", "zone": "nova", "status": "disabled", "state": "down", "updated_at": null, "disabled_reason": "maintenance"}),
            json!({"id": 2, "binary": "nova-compute", "host": "c-0", "zone": "", "status": "enabled", "state": "up"}),
        ];
        let out = host_services(&raw);
        assert_eq!(
            out.iter().map(|x| x.host.as_str()).collect::<Vec<_>>(),
            ["c-0", "c-1", "s-0"],
            "by binary, then host"
        );
        assert_eq!(
            (out[1].enabled, out[1].up, out[1].disabled_reason.as_deref()),
            (false, false, Some("maintenance"))
        );
        assert_eq!(out[0].zone, None, "empty zone");
        assert_eq!(out[1].updated_at, None, "never reported");
        assert_eq!(out[2].updated_at.as_deref(), Some("2026-10-30T10:00:00Z"));
    }

    #[test]
    fn network_agents_map_liveness_and_heartbeat() {
        let raw = vec![
            json!({"id": "a2", "agent_type": "OVN Metadata agent", "binary": "neutron-ovn-metadata-agent", "host": "h1", "availability_zone": null,
                   "admin_state_up": true, "alive": true, "heartbeat_timestamp": "2026-10-19 08:00:00"}),
            json!({"id": "a1", "agent_type": "OVN Controller Gateway agent", "binary": "ovn-controller", "host": "h3", "availability_zone": "",
                   "admin_state_up": false, "alive": false, "heartbeat_timestamp": "2026-10-30 10:00:00"}),
        ];
        let out = network_agents(&raw);
        assert_eq!(out[0].agent_type, "OVN Controller Gateway agent", "by type");
        assert_eq!(
            (out[0].admin_up, out[0].alive, out[0].zone.clone()),
            (false, false, None)
        );
        assert_eq!(out[1].heartbeat.as_deref(), Some("2026-10-19T08:00:00Z"));
    }

    #[test]
    fn metadef_namespaces_show_display_names_and_resource_types() {
        let raw = vec![
            json!({"namespace": "OS::Compute::Watchdog", "display_name": "Watchdog Behavior", "description": "Compute watchdog",
                   "visibility": "public", "protected": true,
                   "resource_type_associations": [{"name": "OS::Glance::Image"}, {"name": "OS::Nova::Flavor"}]}),
            json!({"namespace": "CIM::StorageAllocationSettingData", "display_name": "", "visibility": "private", "protected": false}),
            json!({"display_name": "no namespace"}),
        ];
        let out = metadef_namespaces(&raw);
        assert_eq!(
            out.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(),
            ["CIM::StorageAllocationSettingData", "Watchdog Behavior"],
            "empty display name → namespace; no namespace → skipped"
        );
        assert_eq!(out[1].resource_types, ["OS::Glance::Image", "OS::Nova::Flavor"]);
        assert_eq!(
            (
                out[1].public,
                out[1].protected,
                out[0].public,
                out[0].description.clone()
            ),
            (true, true, false, None)
        );
    }

    #[test]
    fn metadef_options_type_the_values_and_keep_the_prefixed_names() {
        // Shaped like Glance's answer for `?resource_type=OS::Glance::Image`: names already prefixed.
        let ns = json!({"namespace": "OS::Compute::CPUPinning", "display_name": "CPU Pinning",
        "resource_type_associations": [{"name": "OS::Glance::Image", "prefix": "hw_"}],
        "properties": {
            "hw_cpu_policy": {"title": "CPU Pinning policy", "type": "string", "enum": ["shared", "dedicated"], "default": "shared", "description": "Pin"},
            "hw_numa_nodes": {"type": "integer", "default": 2},
            "hw_hidden": {"type": "boolean", "readonly": true},
            "hw_flags": {"title": "Flags", "type": "array"}
        }});
        let out = metadef_options(&ns);
        assert_eq!(
            out.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(),
            ["hw_cpu_policy", "hw_flags", "hw_numa_nodes"],
            "prefix not applied twice; readonly skipped; sorted by title (no title → name)"
        );
        assert_eq!(
            (out[0].kind, out[0].choices.clone(), out[0].default.clone()),
            (
                MetadataKind::Text,
                vec!["shared".to_string(), "dedicated".to_string()],
                Some("shared".to_string())
            )
        );
        assert_eq!(
            (out[2].kind, out[2].default.clone(), out[2].title.as_str()),
            (MetadataKind::Integer, Some("2".to_string()), "hw_numa_nodes")
        );
        assert_eq!(out[1].kind, MetadataKind::Text, "arrays are free text");
        assert_eq!(out[0].namespace, "CPU Pinning");
    }

    #[test]
    fn metadef_options_include_the_properties_of_objects_once() {
        let ns = json!({"namespace": "OS::Compute::Libvirt", "display_name": "Libvirt Driver Options",
        "properties": {"hw_serial": {"title": "Serial", "type": "string"}},
        "objects": [
            {"name": "Quota", "properties": {"quota:cpu_shares": {"title": "CPU shares", "type": "integer"}, "hw_serial": {"title": "Dup"}}},
            {"name": "", "properties": {"quota:vif": {"type": "string"}}},
            {"name": "Empty"}
        ]});
        let out = metadef_options(&ns);
        let got: Vec<_> = out.iter().map(|o| (o.title.as_str(), o.key.as_str())).collect();
        assert_eq!(
            got,
            [
                ("Quota › CPU shares", "quota:cpu_shares"),
                ("quota:vif", "quota:vif"),
                ("Serial", "hw_serial")
            ],
            "object properties get their object's name; a repeated key and nameless objects are handled"
        );
        assert_eq!(out[0].kind, MetadataKind::Integer);
    }

    #[test]
    fn resource_providers_read_inventory_and_usage_per_class() {
        let p = json!({"uuid": "u1", "name": "cmp-0", "generation": 7});
        let inv = json!({"inventories": {
            "VCPU": {"total": 48, "reserved": 2, "allocation_ratio": 16.0, "min_unit": 1},
            "MEMORY_MB": {"total": 262144, "reserved": 4096, "allocation_ratio": 1.0},
            "DISK_GB": {"total": 900, "reserved": 0}
        }});
        let r = resource_provider(&p, &inv, &json!({"usages": {"VCPU": 12, "MEMORY_MB": 65536}}));
        assert_eq!(
            r.vcpus,
            Some(ProviderResource {
                used: 12,
                reserved: 2,
                total: 48,
                allocation_ratio: 16.0
            })
        );
        assert_eq!(
            r.disk_gb.map(|d| (d.used, d.allocation_ratio)),
            Some((0, 1.0)),
            "no usage → 0; no ratio → 1.0"
        );
        let nested = resource_provider(
            &json!({"uuid": "u2", "name": ""}),
            &json!({"inventories": {}}),
            &json!({}),
        );
        assert_eq!(
            (nested.name.as_str(), nested.vcpus),
            ("u2", None),
            "nameless → UUID; no inventory → None"
        );
        let gpu = resource_provider(
            &json!({"uuid": "g1", "name": "cmp-0_pci_0000_84_00_0", "parent_provider_uuid": "u1", "root_provider_uuid": "u1"}),
            &json!({"inventories": {"VGPU": {"total": 8, "reserved": 0, "allocation_ratio": 1.0}, "CUSTOM_PCI_10DE_2236": {"total": 1}}}),
            &json!({"usages": {"VGPU": 3}}),
        );
        assert_eq!(
            (gpu.parent.as_deref(), gpu.vgpus.map(|v| (v.used, v.total))),
            (Some("u1"), Some((3, 8)))
        );
        assert_eq!(
            gpu.others
                .iter()
                .map(|(k, r)| (k.as_str(), r.total))
                .collect::<Vec<_>>(),
            [("CUSTOM_PCI_10DE_2236", 1)],
            "other classes are kept"
        );
        assert_eq!(
            custom_traits(&json!({"traits": ["COMPUTE_VGPU", "CUSTOM_NVIDIA_468", "HW_GPU_API_VULKAN", "CUSTOM_A"]})),
            ["CUSTOM_A", "CUSTOM_NVIDIA_468"],
            "only CUSTOM_*, sorted"
        );
    }

    #[test]
    fn owned_rows_get_project_names_and_hosts() {
        let raw = vec![
            json!({"id": "s1", "tenant_id": "p1", "OS-EXT-SRV-ATTR:host": "c-0"}),
            json!({"id": "s2", "tenant_id": "p9"}),
        ];
        let names = HashMap::from([("p1".to_string(), "demo".to_string())]);
        let out = owned(
            vec!["s1", "s2", "s3"],
            |x| x,
            &raw,
            "tenant_id",
            Some("OS-EXT-SRV-ATTR:host"),
            &names,
        );
        let got: Vec<_> = out.iter().map(|o| (o.project.as_str(), o.host.as_deref())).collect();
        assert_eq!(
            got,
            [("demo", Some("c-0")), ("p9", None), ("-", None)],
            "named; unknown project → ID; missing → `-`"
        );
    }

    #[test]
    fn hypervisors_aggregates_and_zones() {
        let h = hypervisors(&[
            json!({"id": "6f6b…", "hypervisor_hostname": "b", "hypervisor_type": "QEMU", "vcpus": 64, "vcpus_used": 8,
            "memory_mb": 515000, "memory_mb_used": 16384, "local_gb": 900, "local_gb_used": 40, "running_vms": 3, "state": "up", "status": "enabled"}),
            json!({"id": 7, "hypervisor_hostname": "a", "state": "down", "status": "disabled"}),
        ]);
        assert_eq!(
            h.iter()
                .map(|x| (x.hostname.as_str(), x.id.as_str()))
                .collect::<Vec<_>>(),
            [("a", "7"), ("b", "6f6b…")],
            "integer IDs before 2.53"
        );
        assert_eq!(
            (h[1].ram_used_mb, h[1].instances, h[1].up, h[0].up, h[0].vcpus),
            (16384, 3, true, false, 0)
        );
        let a = aggregates(&[
            json!({"id": 3, "name": "ssd", "availability_zone": "alh1", "hosts": ["h2", "h1"], "metadata": {"ssd": "true", "availability_zone": "alh1"}}),
        ]);
        assert_eq!(
            (a[0].id.as_str(), a[0].hosts.clone(), a[0].metadata[0].0.as_str()),
            ("3", vec!["h1".to_string(), "h2".to_string()], "availability_zone")
        );
        let z = zones(&[
            json!({"zoneName": "nova", "zoneState": {"available": true}, "hosts": {"c-1": {}, "c-0": {}}}),
            json!({"zoneName": "internal", "zoneState": {"available": false}, "hosts": null}),
        ]);
        assert_eq!(
            z.iter()
                .map(|z| (z.name.as_str(), z.hosts.len(), z.available))
                .collect::<Vec<_>>(),
            [("internal", 0, false), ("nova", 2, true)]
        );
    }

    #[test]
    fn flavor_rows_read_swap_ephemeral_and_access() {
        let f = flavor_rows(&[
            json!({"id": "f1", "name": "m1.small", "vcpus": 1, "ram": 2048, "disk": 20, "OS-FLV-EXT-DATA:ephemeral": 0, "swap": "",
            "rxtx_factor": 1.0, "os-flavor-access:is_public": false, "extra_specs": {"hw:cpu_policy": "dedicated"}}),
        ]);
        assert_eq!(
            (f[0].swap_mb, f[0].public, f[0].rxtx_factor, f[0].extra_specs.len()),
            (0, false, Some(1.0), 1),
            "`\"\"` swap is none; private"
        );
    }

    #[test]
    fn flavor_access_names_projects_and_keeps_unknown_ids() {
        let raw = vec![
            json!({"flavor_id": "f1", "tenant_id": "p2"}),
            json!({"flavor_id": "f1", "tenant_id": "p1"}),
        ];
        let names = HashMap::from([("p1".to_string(), "Zeta".to_string())]);
        let names: Vec<String> = flavor_access(&raw, &names).into_iter().map(|(_, n)| n).collect();
        assert_eq!(names, ["p2", "Zeta"], "named, sorted case-insensitively; unknown → ID");
    }

    #[test]
    fn volume_types_qos_and_group_types() {
        let qos =
            vec![json!({"id": "q1", "name": "gold", "consumer": "back-end", "specs": {"total_iops_sec": "1000"}})];
        let t = volume_type_rows(
            &[
                json!({"id": "t1", "name": "ssd", "qos_specs_id": "q1", "os-volume-type-access:is_public": true}),
                json!({"id": "t2", "name": "enc", "description": "", "qos_specs_id": null, "is_public": false}),
            ],
            &[("t2".into(), luks())],
            &qos,
        );
        assert_eq!(
            t.iter()
                .map(|t| (
                    t.name.as_str(),
                    t.qos.as_ref().map(|q| q.name.as_str()),
                    t.encrypted,
                    t.public
                ))
                .collect::<Vec<_>>(),
            [("enc", None, true, false), ("ssd", Some("gold"), false, true)]
        );
        assert_eq!(
            qos_specs(&qos)[0].specs,
            [("total_iops_sec".to_string(), "1000".to_string())]
        );
        assert_eq!(t[0].encryption, Some(luks()), "the spec prefills Update Encryption");
        let g = group_type_rows(&[
            json!({"id": "g1", "name": "", "is_public": true, "group_specs": {"consistent_group_snapshot_enabled": "<is> True"}}),
        ]);
        assert_eq!(
            (g[0].name.as_str(), g[0].specs.len()),
            ("g1", 1),
            "unnamed → ID; specs read"
        );
    }

    fn luks() -> EncryptionSpec {
        EncryptionSpec {
            provider: "luks".into(),
            control_location: "front-end".into(),
            cipher: Some("aes-xts-plain64".into()),
            key_size: Some(256),
        }
    }

    #[test]
    fn encryption_specs_read_numbers_or_strings_and_empty_is_none() {
        let e = encryption_spec(
            &json!({"provider": "luks", "control_location": "front-end", "cipher": "aes-xts-plain64", "key_size": 256, "encryption_id": "e1"}),
        );
        assert_eq!(e, Some(luks()));
        assert_eq!(
            encryption_spec(
                &json!({"provider": "luks", "control_location": "back-end", "key_size": "512", "cipher": null})
            )
            .and_then(|e| e.key_size),
            Some(512)
        );
        assert_eq!(
            encryption_spec(&json!({})),
            None,
            "Cinder answers {{}} for a type without encryption"
        );
    }

    #[test]
    fn rbac_policies_name_projects_networks_and_the_wildcard() {
        let names = HashMap::from([
            ("p1".to_string(), "admin".to_string()),
            ("p2".to_string(), "demo".to_string()),
        ]);
        let nets = [json!({"id": "n1", "name": "public"})];
        let qos = [json!({"id": "q1", "name": "gold"})];
        let (pools, groups) = (
            [json!({"id": "sp1", "name": "v4-pool"})],
            [json!({"id": "ag1", "name": "web"})],
        );
        let out = rbac_policies(
            &[
                json!({"id": "r1", "project_id": "p1", "object_type": "network", "object_id": "n1", "target_tenant": "*", "action": "access_as_external"}),
                json!({"id": "r2", "project_id": "p1", "object_type": "qos_policy", "object_id": "q1", "target_tenant": "p2", "action": "access_as_shared"}),
                json!({"id": "r3", "project_id": "p1", "object_type": "qos_policy", "object_id": "q9", "target_tenant": "p2", "action": "access_as_shared"}),
                json!({"id": "r4", "project_id": "p1", "object_type": "address_scope", "object_id": "n1", "target_tenant": "p2", "action": "access_as_shared"}),
                json!({"id": "r5", "project_id": "p1", "object_type": "subnetpool", "object_id": "sp1", "target_tenant": "p2", "action": "access_as_shared"}),
                json!({"id": "r6", "project_id": "p1", "object_type": "address_group", "object_id": "ag1", "target_tenant": "p2", "action": "access_as_shared"}),
            ],
            &[
                ("network", &nets[..]),
                ("qos_policy", &qos[..]),
                ("subnetpool", &pools[..]),
                ("address_group", &groups[..]),
            ],
            &names,
        );
        assert_eq!(
            out.iter()
                .map(|r| (r.object.name.as_str(), r.target.as_str()))
                .collect::<Vec<_>>(),
            [
                ("web", "demo"),
                ("n1", "demo"),
                ("public", "*"),
                ("gold", "demo"),
                ("q9", "demo"),
                ("v4-pool", "demo")
            ],
            "QoS policies, subnet pools and address groups named like networks; an unknown one, or another type's object with the same ID, keeps its ID"
        );
        assert_eq!(
            out.iter().map(|r| r.target_id.as_str()).collect::<Vec<_>>(),
            ["p2", "p2", "*", "p2", "p2", "p2"],
            "the target's ID, for Edit Policy"
        );
    }

    #[test]
    fn metadef_namespace_detail_lists_properties_objects_and_associations() {
        // Shaped like Glance's OS::Compute::Watchdog (properties) and OS::Software::DBMS (objects).
        let watchdog = json!({
            "namespace": "OS::Compute::Watchdog", "display_name": "Watchdog Behavior", "description": "Compute drivers…",
            "visibility": "public", "protected": true, "created_at": "2026-01-02T03:04:05Z", "updated_at": "2026-01-02T03:04:05Z",
            "resource_type_associations": [
                {"name": "OS::Glance::Image"},
                {"name": "OS::Nova::Flavor", "prefix": "hw:", "properties_target": "image"}
            ],
            "properties": {"hw_watchdog_action": {"title": "Watchdog Action", "type": "string", "enum": ["disabled", "reset", "poweroff", "pause", "none"]},
                           "count": {"type": "integer", "default": 2}}
        });
        let d = metadef_namespace_detail(&watchdog).unwrap();
        assert_eq!(
            (
                d.namespace.name.as_str(),
                d.namespace.protected,
                d.created_at.as_deref()
            ),
            ("Watchdog Behavior", true, Some("2026-01-02T03:04:05Z"))
        );
        assert_eq!(
            d.associations[1],
            MetadefAssociation {
                name: "OS::Nova::Flavor".into(),
                prefix: Some("hw:".into()),
                properties_target: Some("image".into())
            }
        );
        assert_eq!(d.associations[0].prefix, None);
        assert_eq!(
            d.namespace.prefixes,
            [("OS::Nova::Flavor".to_string(), "hw:".to_string())],
            "for Update Associations"
        );
        assert_eq!(
            d.properties.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["count", "hw_watchdog_action"],
            "by name"
        );
        assert_eq!(
            (d.properties[0].kind.as_str(), d.properties[0].default.as_deref()),
            ("integer", Some("2")),
            "a non-string default as JSON"
        );
        assert_eq!(d.properties[1].allowed.len(), 5);
        assert_eq!(d.properties[1].title.as_deref(), Some("Watchdog Action"));
        assert!(d.objects.is_empty() && d.raw.contains("\"hw_watchdog_action\""));
        let dbms = json!({
            "namespace": "OS::Software::DBMS", "visibility": "public",
            "objects": [
                {"name": "PostgreSQL", "description": "Object-relational DBMS", "properties": {"sw_database_postgresql_version": {"type": "string"}, "sw_database_postgresql_listen_port": {"type": "integer"}}},
                {"name": "MySQL", "properties": {"sw_database_mysql_version": {"type": "array", "items": {"type": "string", "enum": ["5.6", "5.7"]}}}}
            ]
        });
        let d = metadef_namespace_detail(&dbms).unwrap();
        assert_eq!(d.namespace.name, "OS::Software::DBMS", "no display name: the namespace");
        assert_eq!(
            d.objects.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(),
            ["MySQL", "PostgreSQL"]
        );
        assert_eq!(
            d.objects[1].properties,
            ["sw_database_postgresql_listen_port", "sw_database_postgresql_version"]
        );
        assert!(d.properties.is_empty());
        assert!(
            metadef_namespace_detail(&json!({"properties": {}})).is_none(),
            "no namespace"
        );
    }
}
